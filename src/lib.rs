//! form-data implemented [rfc7578]
//!
//! # Example
//!
//! ```no_run
//! #![deny(warnings)]
//!
//! use std::{env, net::SocketAddr};
//!
//! use anyhow::Result;
//! use async_fs::File;
//! use bytes::Bytes;
//! use futures_util::{
//!     io::{copy, AsyncWriteExt},
//!     stream::TryStreamExt,
//! };
//! use http_body_util::Full;
//! use hyper::{body::Incoming, header, server::conn::http1, service::service_fn, Request, Response};
//! use hyper_util::rt::TokioIo;
//! use tempfile::tempdir;
//! use tokio::net::TcpListener;
//!
//! use form_data::{Error, FormData};
//!
//! #[path = "../tests/lib/mod.rs"]
//! mod lib;
//!
//! use lib::IncomingBody;
//!
//! async fn hello(size: usize, req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Error> {
//!     let dir = tempdir()?;
//!     let mut txt = String::new();
//!
//!     txt.push_str(&dir.path().to_string_lossy());
//!     txt.push_str("\r\n");
//!
//!     let m = req
//!         .headers()
//!         .get(header::CONTENT_TYPE)
//!         .and_then(|val| val.to_str().ok())
//!         .and_then(|val| val.parse::<mime::Mime>().ok())
//!         .ok_or(Error::InvalidHeader)?;
//!
//!     let mut form = FormData::new(
//!         req.map(|body| IncomingBody::new(Some(body))).into_body(),
//!         m.get_param(mime::BOUNDARY).unwrap().as_str(),
//!     );
//!
//!     // 512KB for hyper lager buffer
//!     form.set_max_buf_size(size)?;
//!
//!     while let Some(mut field) = form.try_next().await? {
//!         let name = field.name.to_owned();
//!         let mut bytes: u64 = 0;
//!
//!         assert_eq!(bytes as usize, field.length);
//!
//!         if let Some(filename) = &field.filename {
//!             let filepath = dir.path().join(filename);
//!
//!             match filepath.extension().and_then(|s| s.to_str()) {
//!                 Some("txt") => {
//!                     // buffer <= 8KB
//!                     let mut writer = File::create(&filepath).await?;
//!                     bytes = copy(&mut field, &mut writer).await?;
//!                     writer.close().await?;
//!                 }
//!                 Some("iso") => {
//!                     field.ignore().await?;
//!                 }
//!                 _ => {
//!                     // 8KB <= buffer <= 512KB
//!                     // let mut writer = File::create(&filepath).await?;
//!                     // bytes = field.copy_to(&mut writer).await?;
//!
//!                     let mut writer = std::fs::File::create(&filepath)?;
//!                     bytes = field.copy_to_file(&mut writer).await?;
//!                 }
//!             }
//!
//!             tracing::info!("file {} {}", name, bytes);
//!             txt.push_str(&format!("file {name} {bytes}\r\n"));
//!         } else {
//!             let buffer = field.bytes().await?;
//!             bytes = buffer.len() as u64;
//!             tracing::info!("text {} {}", name, bytes);
//!             txt.push_str(&format!("text {name} {bytes}\r\n"));
//!         }
//!
//!         tracing::info!("{:?}", field);
//!
//!         assert_eq!(
//!             bytes,
//!             match name.as_str() {
//!                 "empty" => 0,
//!                 "tiny1" => 7,
//!                 "tiny0" => 122,
//!                 "small1" => 315,
//!                 "small0" => 1_778,
//!                 "medium" => 13_196,
//!                 "large" => 2_413_677,
//!                 "book" => 400_797_393,
//!                 "crate" => 9,
//!                 _ => bytes,
//!             }
//!         );
//!     }
//!
//!     dir.close()?;
//!
//!     Ok(Response::new(Full::from(Into::<String>::into(txt))))
//! }
//!
//! #[tokio::main]
//! pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     tracing_subscriber::fmt()
//!         // From env var: `RUST_LOG`
//!         .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
//!         .try_init()
//!         .map_err(|e| anyhow::anyhow!(e))?;
//!
//!     let mut arg = env::args()
//!         .find(|a| a.starts_with("--size="))
//!         .unwrap_or_else(|| "--size=8".to_string());
//!
//!     // 512
//!     // 8 * 2
//!     // 8
//!     let size = arg.split_off(7).parse::<usize>().unwrap_or(8) * 1024;
//!     let addr: SocketAddr = ([127, 0, 0, 1], 3000).into();
//!
//!     println!("Listening on http://{addr}");
//!     println!("FormData max buffer size is {}KB", size / 1024);
//!
//!     let listener = TcpListener::bind(addr).await?;
//!
//!     loop {
//!         let (stream, _) = listener.accept().await?;
//!
//!         tokio::task::spawn(async move {
//!             if let Err(err) = http1::Builder::new()
//!                 .max_buf_size(size)
//!                 .serve_connection(
//!                     TokioIo::new(stream),
//!                     service_fn(|req: Request<Incoming>| hello(size, req)),
//!                 )
//!                 .await
//!             {
//!                 println!("Error serving connection: {:?}", err);
//!             }
//!         });
//!     }
//! }
//! ```
//!
//! [rfc7578]: <https://tools.ietf.org/html/rfc7578>

#![forbid(unsafe_code)]
#![deny(nonstandard_style)]
#![warn(missing_docs, unreachable_pub)]
#![allow(clippy::missing_errors_doc)]

mod error;
pub use error::Error;

mod field;
pub use field::Field;

mod form;
pub use form::FormData;

mod limits;
pub use limits::Limits;

mod state;
pub use state::*;

mod utils;

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(all(feature = "async", not(feature = "sync")))]
mod r#async;
#[cfg(all(feature = "sync", not(feature = "async")))]
mod sync;
