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

pub struct Limited<T> {
    io: T,
    limit: usize,
    length: u64,
    eof: bool,
}

impl<T> Limited<T> {
    #[allow(dead_code)]
    pub fn random(io: T) -> Self {
        let mut rng = rand::thread_rng();
        let limit = rng.gen_range(1, 8 * 1024);

        log::info!("Limited stream by {}", limit);

        Self {
            io,
            limit,
            length: 0,
            eof: false,
        }
    }

    #[allow(dead_code)]
    pub fn random_with(io: T, max: usize) -> Self {
        let mut rng = rand::thread_rng();
        let limit = rng.gen_range(1, max);

        log::info!("Limited stream by {}", limit);

        Self {
            io,
            limit,
            length: 0,
            eof: false,
        }
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
        buf.resize(self.limit, 0);

        match Pin::new(&mut self.io).poll_read(cx, &mut buf[..])? {
            Poll::Ready(n) => {
                if n == 0 {
                    self.eof = true;
                    return Poll::Ready(None);
                }
                if n < self.limit {
                    buf.truncate(n);
                }
                self.length += n as u64;
                return Poll::Ready(Some(Ok(buf.freeze())));
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
