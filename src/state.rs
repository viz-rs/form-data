use std::{
    fmt,
    pin::Pin,
    task::{Context, Poll, Waker},
};

use anyhow::{Error, Result};
use bytes::{Buf, Bytes, BytesMut};
use futures_util::stream::Stream;
use memchr::memmem;
use tracing::trace;

use crate::utils::{CR, CRLF, CRLFS, CRLF_DASHES, DASH, DASHES, DEFAULT_BUF_SIZE, LF};

/// IO State
pub struct State<'a, T> {
    io: T,
    eof: bool,
    length: u64,
    total: usize,
    buffer: BytesMut,
    is_readable: bool,
    boundary: &'a [u8],
    waker: Option<Waker>,
    max_buf_size: usize,
}

impl<'a, T> State<'a, T> {
    /// Creates new State.
    pub fn new(boundary: &'a [u8], io: T) -> Self {
        Self {
            io,
            boundary,
            total: 0,
            length: 0,
            eof: false,
            waker: None,
            // placeholder `\r\n` , let first boundary is `\r\n--boundary`
            buffer: BytesMut::from(&CRLF[..]),
            is_readable: false,
            max_buf_size: DEFAULT_BUF_SIZE,
        }
    }

    /// Sets max buffer size.
    pub fn set_max_buf_size(&mut self, max: usize) {
        assert!(
            max >= DEFAULT_BUF_SIZE,
            "The max_buf_size cannot be smaller than {}.",
            DEFAULT_BUF_SIZE,
        );
        self.max_buf_size = max;
    }

    /// Gets io.
    pub fn io_mut(&mut self) -> &mut T {
        &mut self.io
    }

    /// Gets waker.
    pub fn waker(&self) -> Option<&Waker> {
        self.waker.as_ref()
    }

    /// Gets waker.
    pub fn waker_mut(&mut self) -> &mut Option<Waker> {
        &mut self.waker
    }

    /// Splits buffer.
    pub fn split_buffer(&mut self, n: usize) -> Bytes {
        self.buffer.split_to(n).freeze()
    }

    /// Gets the index of the field.
    pub fn index(&mut self) -> usize {
        let index = self.total;
        self.total += 1;
        index
    }

    /// Gets the length of the form-data.
    pub fn len(&self) -> u64 {
        self.length
    }

    /// Gets EOF.
    pub fn eof(&self) -> bool {
        self.eof
    }

    /// Counts the fields.
    pub fn total(&self) -> usize {
        self.total
    }

    /// `boundary`
    fn boundary(&self) -> &[u8] {
        &self.boundary
    }

    fn decode_eof(&mut self) -> Result<Option<Bytes>> {
        Ok(Some(Bytes::new()))
    }

    fn decode(&mut self) -> Result<Option<Bytes>> {
        // `\r\n--boundary\r\n` or // `\r\n--boundary--`
        let min_size = 2 + 2 + self.boundary.len() + 2;
        let max_buf_size = self.max_buf_size;

        Ok(Some(Bytes::new()))
    }
}

impl<'a, T> fmt::Debug for State<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("State")
            .field("boundary", &String::from_utf8_lossy(self.boundary()))
            .field("length", &self.length)
            .field("total", &self.total)
            .field("eof", &self.eof)
            .field("is_readable", &self.is_readable)
            .finish()
    }
}

impl<'a, T, E> Stream for State<'a, T>
where
    T: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Into<Error>,
{
    // 0 is EOF!
    // First: if found a boundary then returns size of headers to `Form`
    // Second: returns of payload data to `Field`
    // Find `--boundary` ->
    // Find part headers -> return headers buffer -> return Field
    // Find part payload -> return payload buffer -> return to Field Stream
    // Find part headers -> if with prev Field -> return None to prev Field Stream, Field Stream EOF
    // Find part headers -> if with `--` stuffix -> return None to FormData Stream EOF
    type Item = Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if self.is_readable {
                if self.eof {
                    // decode_eof
                    let data = self.decode_eof()?;
                    if data.is_none() {
                        self.is_readable = false;
                    }
                    return Poll::Ready(data.map(Ok));
                }

                // part
                trace!("attempting to decode a part");

                if let Some(data) = self.decode()? {
                    trace!("part decoded from buffer");
                    return Poll::Ready(Some(Ok(data)));
                }

                self.is_readable = false;
            }

            trace!("polling data from stream");

            self.buffer.reserve(1);
            let bytect = match Pin::new(self.io_mut()).poll_next(cx) {
                Poll::Pending => {
                    return Poll::Pending;
                }
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Some(Err(e.into()))),
                Poll::Ready(Some(Ok(b))) => {
                    let l = b.len();
                    // @TODO: need check payload data length
                    self.length += l as u64;
                    self.buffer.extend_from_slice(&b);
                    l
                }
                Poll::Ready(None) => 0,
            };

            if bytect == 0 {
                if self.eof {
                    return Poll::Ready(None);
                }

                self.eof = true;
            } else {
                self.eof = false;
            }

            self.is_readable = true;
        }
    }
}
