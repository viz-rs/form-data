//! form-data implemented [rfc7578]
//!
//! # Example
//!
//! ```rust
//! use std::{convert::Infallible, env};
//!
//! use anyhow::Result;
//! use tempfile::tempdir;
//!
//! use futures_util::{
//!     io::{copy, AsyncWriteExt},
//!     stream::TryStreamExt,
//! };
//!
//! use hyper::{
//!     header,
//!     service::{make_service_fn, service_fn},
//!     Body, Request, Response, Server,
//! };
//!
//! use async_fs::File;
//!
//! use form_data::{Error, FormData};
//!
//! async fn hello(size: usize, req: Request<Body>) -> Result<Response<Body>, Error> {
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
//!         .unwrap();
//!
//!     let mut form = FormData::new(
//!         req.into_body(),
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
//!             txt.push_str(&format!("file {} {}\r\n", name, bytes));
//!         } else {
//!             let buffer = field.bytes().await?;
//!             bytes = buffer.len() as u64;
//!             tracing::info!("text {} {}", name, bytes);
//!             txt.push_str(&format!("text {} {}\r\n", name, bytes));
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
//!     Ok(Response::new(Body::from(txt)))
//! }
//! ```
//!
//! [rfc7578]: <https://tools.ietf.org/html/rfc7578>

#![forbid(unsafe_code)]
#![deny(nonstandard_style)]
#![warn(missing_docs, rustdoc::missing_doc_code_examples, unreachable_pub)]

mod error;
mod field;
mod form;
mod limits;
mod state;
mod utils;

pub use form::FormData;

pub use field::Field;

pub use state::*;

pub use limits::Limits;

pub use error::Error;

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(all(feature = "async", not(feature = "sync")))]
mod r#async;
#[cfg(all(feature = "async", not(feature = "sync")))]
pub use r#async::*;
#[cfg(all(feature = "sync", not(feature = "async")))]
mod sync;
#[cfg(all(feature = "sync", not(feature = "async")))]
pub use sync::*;
