#![deny(warnings)]

use anyhow::{Error, Result};
use async_fs::File;
use bytes::{Buf, Bytes};
use form_data::FormData;
use futures_util::{
    io::{self, AsyncWriteExt},
    ready,
    stream::{Stream, TryStreamExt},
};
use hyper::{header::HeaderValue, Body, Response};
use std::pin::Pin;
use std::task::{Context, Poll};
use tempfile::tempdir;
use warp::Filter;

struct MyStream<T> {
    body: T,
}

impl<T, B> MyStream<T>
where
    T: Stream<Item = Result<B, warp::Error>> + Unpin,
    B: Buf,
{
    pub fn new(body: T) -> Self {
        Self { body }
    }
}

impl<T, B> Stream for MyStream<T>
where
    T: Stream<Item = Result<B, warp::Error>> + Unpin,
    B: Buf,
{
    type Item = Result<Bytes>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let opt_item = ready!(Pin::new(&mut self.get_mut().body).poll_next(cx));

        match opt_item {
            None => Poll::Ready(None),
            Some(item) => {
                let stream_buf = item
                    .map(|mut b| b.copy_to_bytes(b.remaining()))
                    .map_err(Error::new);

                Poll::Ready(Some(stream_buf))
            }
        }
    }
}

async fn form(
    header: HeaderValue,
    body: impl Stream<Item = Result<impl Buf, warp::Error>> + Unpin,
) -> Result<impl warp::Reply, anyhow::Error> {
    let dir = tempdir()?;
    let mut txt = String::new();

    txt.push_str(&dir.path().to_string_lossy());
    txt.push_str("\r\n");

    let m = header
        .to_str()
        .ok()
        .and_then(|v| v.parse::<mime::Mime>().ok())
        .unwrap();

    let mut form = FormData::new(
        MyStream::new(body),
        m.get_param(mime::BOUNDARY).unwrap().as_str(),
    );

    // 512KB for hyper lager buffer
    // form.set_max_buf_size(size)?;

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
                    bytes = io::copy(&mut field, &mut writer).await?;
                    writer.close().await?;
                }
                Some("iso") => {
                    field.ignore().await?;
                }
                _ => {
                    // 8KB <= buffer <= 512KB
                    // let mut writer = File::create(&filepath).await?;
                    // bytes = field.copy_to(&mut writer).await?;

                    let writer = std::fs::File::create(&filepath)?;
                    bytes = field.copy_to_file(writer).await?;
                }
            }

            tracing::info!("file {} {}", name, bytes);
            txt.push_str(&format!("file {} {}\r\n", name, bytes));
        } else {
            let buffer = field.bytes().await?;
            bytes = buffer.len() as u64;
            tracing::info!("text {} {}", name, bytes);
            txt.push_str(&format!("text {} {}\r\n", name, bytes));
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
async fn main() {
    let routes = warp::post()
        .and(warp::header::value("Content-Type"))
        .and(warp::body::stream())
        .and_then(|h, b| async {
            let r = form(h, b).await;
            r.map_err(|e| {
                dbg!(e);
                warp::reject::reject()
            })
        });

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}
