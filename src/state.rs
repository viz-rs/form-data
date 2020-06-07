use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use anyhow::{anyhow, Error, Result};
use bytes::{Buf, Bytes, BytesMut};
use futures_util::stream::Stream;

use crate::utils::{read_until_ctlf, CR, DASH, LF};

#[derive(Debug, PartialEq)]
enum Flag {
    Header,
    Body,
}

struct Cached {
    flag: Flag,
    dash_boundary: Vec<u8>,
    dash_boundary_len: usize,
}

impl fmt::Debug for Cached {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cached")
            .field("flag", &self.flag)
            .field(
                "dash_boundary",
                &String::from_utf8_lossy(&self.dash_boundary),
            )
            .field("dash_boundary_len", &self.dash_boundary_len)
            .finish()
    }
}

pub struct State<T> {
    io: T,
    eof: bool,
    length: u64,
    boundary: Vec<u8>,
    index: Option<usize>,
    waker: Option<Waker>,
    buffer: Option<BytesMut>,
    catched: Cached,
}

impl<T> State<T> {
    pub fn new<B: AsRef<[u8]>>(b: B, io: T) -> Self {
        // `boundary`
        let boundary = b.as_ref().to_owned();
        // `--boundary`
        let mut dash_boundary = boundary.clone();
        dash_boundary.insert(0, DASH);
        dash_boundary.insert(0, DASH);

        Self {
            io,
            boundary,
            length: 0,
            eof: false,
            index: None,
            waker: None,
            buffer: None,
            catched: Cached {
                flag: Flag::Body,
                dash_boundary_len: dash_boundary.len(),
                dash_boundary,
            },
        }
    }

    pub fn io_mut(&mut self) -> &mut T {
        &mut self.io
    }

    pub fn waker(&self) -> Option<&Waker> {
        self.waker.as_ref()
    }

    pub fn waker_mut(&mut self) -> &mut Option<Waker> {
        &mut self.waker
    }

    pub fn buffer(&self) -> &BytesMut {
        self.buffer.as_ref().unwrap()
    }

    pub fn buffer_mut(&mut self) -> &mut BytesMut {
        self.buffer.as_mut().unwrap()
    }

    pub fn eof(&self) -> bool {
        self.eof
    }

    fn eof_mut(&mut self) -> &mut bool {
        &mut self.eof
    }

    pub fn incr_index(&mut self) -> usize {
        let total = self.index.get_or_insert_with(|| 0);
        let index = *total;
        *total += 1;
        index
    }

    pub fn len(&self) -> u64 {
        self.length
    }

    pub fn total(&self) -> usize {
        self.index.unwrap_or_default()
    }

    fn flag(&self) -> &Flag {
        &self.catched.flag
    }

    fn flag_mut(&mut self) -> &mut Flag {
        &mut self.catched.flag
    }

    /// `boundary`
    fn boundary(&self) -> &[u8] {
        &self.boundary
    }

    /// `--boundary`
    fn dash_boundary(&self) -> &[u8] {
        &self.catched.dash_boundary
    }

    fn dash_boundary_len(&self) -> usize {
        self.catched.dash_boundary_len
    }
}

impl<T> fmt::Debug for State<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("State")
            .field("boundary", &String::from_utf8_lossy(self.boundary()))
            .field("eof", &self.eof)
            .field("length", &self.length)
            .field("total", &self.index)
            .field("catched", &self.catched)
            .finish()
    }
}

impl<T, O, E> Stream for State<T>
where
    T: Stream<Item = Result<O, E>> + Unpin + Send + 'static,
    O: Into<Bytes>,
    E: Into<Error>,
{
    // 0 is EOF!
    // First: if found a boundary then returns size of headers to `Form`
    // Second: returns of payload data to `Field`
    type Item = Result<usize>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        log::debug!("poll stream");

        if self.buffer.is_none() {
            self.buffer.replace(BytesMut::new());
        }

        let mut has_boundary = false;
        let mut has_headers = 0;

        loop {
            let buffer = &self.buffer()[has_headers..];

            // `\r\n`
            if let Some(mut idx) = read_until_ctlf(&buffer) {
                idx += 2;

                // `--boundary\r\n` or `--boundary--\r\n`
                let diff: usize = idx.saturating_sub(self.dash_boundary_len());

                // `--\r\n`
                if
                // Flag::Body == *self.flag()
                diff == 4
                    && buffer[idx - 4] == DASH
                    && buffer[idx - 3] == DASH
                    && buffer[idx - 2] == CR
                    && buffer[idx - 1] == LF
                    && buffer.starts_with(self.dash_boundary())
                {
                    *self.eof_mut() = true;
                    self.buffer_mut().clear();
                    self.buffer.take();
                    return Poll::Ready(Some(Ok(0)));
                }

                // `\r\n`
                // `--boundary\r\n` is starter of current part,
                // also it is ended of previous part.
                // So flag defaults to `Body`
                if Flag::Body == *self.flag()
                    && has_boundary == false
                    && diff == 2
                    && buffer[idx - 2] == CR
                    && buffer[idx - 1] == LF
                    && buffer.starts_with(self.dash_boundary())
                {
                    // wakes previous waker of `Field`
                    if self.index.is_some() && self.waker.is_some() {
                        return Poll::Ready(None);
                    }
                    log::debug!("part {}", self.index.unwrap_or_default());
                    *self.flag_mut() = Flag::Header;
                    self.buffer_mut().advance(idx);
                    has_boundary = true;
                    idx = 0;
                }

                match self.flag() {
                    Flag::Header => {
                        // ignore
                        if idx < 2 {
                            continue;
                        }

                        // `\r\n`: end headers
                        if idx == 2 {
                            if has_headers > 0 {
                                log::debug!("part headers");
                                *self.flag_mut() = Flag::Body;
                                return Poll::Ready(Some(Ok(has_headers + idx)));
                            }

                            return Poll::Ready(Some(Err(anyhow!("missing headers"))));
                        }

                        // @TODO: check max headers, single header max size
                        // `******\r\n`: header
                        has_headers += idx;
                        continue;
                    }
                    Flag::Body => {
                        if self.index.is_some() {
                            return Poll::Ready(Some(Ok(idx)));
                        }
                        self.buffer_mut().advance(idx);
                    }
                }
            }

            if self.eof {
                return Poll::Ready(None);
            }

            match Pin::new(self.io_mut()).poll_next(cx) {
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Some(Err(e.into()))),
                Poll::Ready(Some(Ok(b))) => {
                    let b = b.into();
                    let l = b.len() as u64;
                    self.length += l;
                    self.buffer_mut().extend_from_slice(&b);
                    log::debug!("polled bytes {}/{}", l, self.length);
                }
                Poll::Ready(None) => {
                    self.eof = true;
                    log::debug!("polled total bytes: {}", self.length);
                }
                Poll::Pending => {
                    log::debug!("polled pending");
                    return Poll::Pending;
                }
            }
        }
    }
}
