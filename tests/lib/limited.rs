use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::Result;
use bytes::{Bytes, BytesMut};
use futures_util::{
    io::{self, AsyncRead},
    stream::Stream,
};
use rand::Rng;

pub const LIMITED: usize = 8 * 1024;

pub struct Limited<T> {
    io: T,
    limit: usize,
    length: u64,
    eof: bool,
}

#[allow(dead_code)]
impl<T> Limited<T> {
    pub fn new(io: T, limit: usize) -> Self {
        tracing::info!("Limited stream by {}", limit);

        Self {
            io,
            limit,
            length: 0,
            eof: false,
        }
    }

    pub fn random(io: T) -> Self {
        Self::new(io, rand::thread_rng().gen_range(1..LIMITED))
    }

    pub fn random_with(io: T, max: usize) -> Self {
        Self::new(io, rand::thread_rng().gen_range(1..max))
    }
}

impl<T> fmt::Debug for Limited<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Limited")
            .field("eof", &self.eof)
            .field("limit", &self.limit)
            .field("length", &self.length)
            .finish()
    }
}

impl<T: AsyncRead + Unpin + Send + 'static> Stream for Limited<T> {
    type Item = Result<Bytes, io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut buf = BytesMut::new();
        // zero-fills the space in the read buffer
        buf.resize(self.limit, 0);

        match Pin::new(&mut self.io).poll_read(cx, &mut buf[..])? {
            Poll::Ready(0) => {
                self.eof = true;
                return Poll::Ready(None);
            }
            Poll::Ready(n) => {
                self.length += n as u64;
                buf.truncate(n);
                return Poll::Ready(Some(Ok(buf.freeze())));
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
