//!
//! Run with
//!
//! Max buffer size is 8KB by defaults.
//!
//! ```
//! // 8KB
//! $ RUST_LOG=info cargo run --example hyper -- --nocapture --size=8
//!
//! // 64KB
//! $ RUST_LOG=info cargo run --example hyper -- --nocapture --size=64
//!
//! // 512KB
//! $ RUST_LOG=info cargo run --example hyper -- --nocapture --size=512
//! ```
//!
//! Fish shell
//! ```
//! $ set files tests/fixtures/files/*; for i in (seq (count $files) | sort -R); echo "-F "(string split . (basename $files[$i]))[1]=@$files[$i]; end | string join ' ' | xargs time curl -vvv http://127.0.0.1:3000 -F crate=form-data
//! ```

#![deny(warnings)]

use std::{convert::Infallible, env};

use anyhow::Result;
use tempfile::tempdir;

use futures_util::{
    io::{copy, AsyncWriteExt},
    stream::TryStreamExt,
};

use hyper::{
    header,
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server,
};

use async_fs::File;

use form_data::{Error, FormData};

async fn hello(size: usize, req: Request<Body>) -> Result<Response<Body>, Error> {
    let dir = tempdir()?;
    let mut txt = String::new();

    txt.push_str(&dir.path().to_string_lossy());
    txt.push_str("\r\n");

    let m = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|val| val.to_str().ok())
        .and_then(|val| val.parse::<mime::Mime>().ok())
        .unwrap();

    let mut form = FormData::new(
        req.into_body(),
        m.get_param(mime::BOUNDARY).unwrap().as_str(),
    );

    // 512KB for hyper lager buffer
    form.set_max_buf_size(size)?;

    while let Some(mut field) = form.try_next().await? {
        let name = field.name.to_owned();
        let mut bytes: u64 = 0;

        assert_eq!(bytes as usize, field.length);

        if let Some(filename) = &field.filename {
            let filepath = dir.path().join(filename);

            match filepath.extension().and_then(|s| s.to_str()) {
                Some("txt") => {
                    // buffer <= 8KB
                    let mut writer = File::create(&filepath).await?;
                    bytes = copy(&mut field, &mut writer).await?;
                    writer.close().await?;
                }
                Some("iso") => {
                    field.ignore().await?;
                }
                _ => {
                    // 8KB <= buffer <= 512KB
                    // let mut writer = File::create(&filepath).await?;
                    // bytes = field.copy_to(&mut writer).await?;

                    let mut writer = std::fs::File::create(&filepath)?;
                    bytes = field.copy_to_file(&mut writer).await?;
                }
            }

            tracing::info!("file {} {}", name, bytes);
            txt.push_str(&format!("file {name} {bytes}\r\n"));
        } else {
            let buffer = field.bytes().await?;
            bytes = buffer.len() as u64;
            tracing::info!("text {} {}", name, bytes);
            txt.push_str(&format!("text {name} {bytes}\r\n"));
        }

        tracing::info!("{:?}", field);

        assert_eq!(
            bytes,
            match name.as_str() {
                "empty" => 0,
                "tiny1" => 7,
                "tiny0" => 122,
                "small1" => 315,
                "small0" => 1_778,
                "medium" => 13_196,
                "large" => 2_413_677,
                "book" => 400_797_393,
                "crate" => 9,
                _ => bytes,
            }
        );
    }

    dir.close()?;

    Ok(Response::new(Body::from(txt)))
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        // From env var: `RUST_LOG`
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .map_err(|e| anyhow::anyhow!(e))?;

    let mut arg = env::args()
        .find(|a| a.starts_with("--size="))
        .unwrap_or_else(|| "--size=8".to_string());

    // 512
    // 8 * 2
    // 8
    let size = arg.split_off(7).parse::<usize>().unwrap_or(8) * 1024;

    // For every connection, we must make a `Service` to handle all
    // incoming HTTP requests on said connection.
    let make_svc = make_service_fn(|_conn| {
        // This is the `Service` that will handle the connection.
        // `service_fn` is a helper to convert a function that
        // returns a Response into a `Service`.
        async move { Ok::<_, Infallible>(service_fn(move |req| hello(size, req))) }
    });

    let addr = ([127, 0, 0, 1], 3000).into();

    let server = Server::bind(&addr).http1_max_buf_size(size).serve(make_svc);

    println!("Listening on http://{addr}");
    println!("FormData max buffer size is {}KB", size / 1024);

    server.await?;

    Ok(())
}
