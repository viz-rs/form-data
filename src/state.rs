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

use crate::utils::{CRLF, CRLFS, DASHES, DEFAULT_BUF_SIZE};

#[derive(Debug, PartialEq)]
enum Flag {
    Delimiting(bool),
    Heading(usize),
    Headed,
    Header,
    Next,
    Eof,
}
/// IO State
pub struct State<T> {
    io: T,
    eof: bool,
    flag: Flag,
    length: u64,
    total: usize,
    buffer: BytesMut,
    delimiter: Bytes,
    is_readable: bool,
    max_buf_size: usize,
    waker: Option<Waker>,
}

impl<T> State<T> {
    /// Creates new State.
    pub fn new(boundary: &[u8], io: T) -> Self {
        // `\r\n--boundary`
        let mut delimiter = BytesMut::with_capacity(4 + boundary.len());
        delimiter.extend_from_slice(&CRLF);
        delimiter.extend_from_slice(&DASHES);
        delimiter.extend_from_slice(&boundary);

        // `\r\n`
        let mut buffer = BytesMut::with_capacity(DEFAULT_BUF_SIZE);
        buffer.extend_from_slice(&CRLF);

        Self {
            io,
            total: 0,
            length: 0,

            waker: None,
            eof: false,
            is_readable: false,

            buffer,
            flag: Flag::Delimiting(false),
            delimiter: delimiter.freeze(),

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

    /// Gets the boundary.
    pub fn boundary(&self) -> &[u8] {
        &self.delimiter[4..]
    }

    fn decode(&mut self) -> Result<Option<Bytes>> {
        if let Flag::Delimiting(boding) = self.flag {
            let mut heading = false;
            if let Some(n) = memmem::find(&self.buffer, &self.delimiter) {
                heading = true;
                self.flag = Flag::Heading(n);
            }

            if !heading {
                // Empty Request Body
                if self.eof && self.buffer.len() == 2 && self.buffer[..2] == CRLF {
                    self.buffer.advance(2);
                    self.flag = Flag::Eof;
                }

                // Empty Part Body
                if memmem::find(&self.buffer, &self.delimiter[2..]).is_some() {
                    self.flag = Flag::Next;
                    self.buffer.advance(self.delimiter.len() - 2);
                    return Ok(None);
                }

                // Reading Part Body
                if boding {
                    // Returns buffer with `max_buf_size`
                    if self.max_buf_size + self.delimiter.len() < self.buffer.len() {
                        return Ok(Some(self.buffer.split_to(self.max_buf_size).freeze()));
                    }
                }
            }
        }

        if let Flag::Heading(ref mut n) = self.flag {
            // first part
            if self.total == 0 {
                if *n > 0 {
                    // consume data
                    self.buffer.advance(*n);
                }
                self.buffer.advance(self.delimiter.len());
                self.flag = Flag::Headed;
            } else {
                // prev part is ended
                if *n == 0 {
                    // field'stream need to stop
                    self.flag = Flag::Next;
                    self.buffer.advance(self.delimiter.len());
                    return Ok(None);
                } else {
                    // prev part last data
                    let buf = self.buffer.split_to(*n).freeze();
                    *n = 0;
                    return Ok(Some(buf));
                }
            }
        }

        if Flag::Next == self.flag {
            self.flag = Flag::Headed;
        }

        if Flag::Headed == self.flag {
            if self.buffer.len() > 1 {
                if self.buffer[..2] == CRLF {
                    self.buffer.advance(2);
                    self.flag = Flag::Header;
                } else if self.buffer[..2] == DASHES {
                    self.buffer.advance(2);
                    self.flag = Flag::Eof;
                } else {
                    // We dont parse other format, like `\n`
                    self.length -= (self.delimiter.len() - 2) as u64;
                    self.flag = Flag::Eof;
                }
            }
        }

        if Flag::Header == self.flag {
            if let Some(n) = memmem::find(&self.buffer, &CRLFS) {
                self.flag = Flag::Delimiting(true);
                return Ok(Some(self.buffer.split_to(n + CRLFS.len()).freeze()));
            }
        }

        Ok(None)
    }
}

impl<T> fmt::Debug for State<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("State")
            .field("eof", &self.eof)
            .field("flag", &self.flag)
            .field("total", &self.total)
            .field("length", &self.length)
            .field("is_readable", &self.is_readable)
            .field("boundary", &String::from_utf8_lossy(self.boundary()))
            .finish()
    }
}

impl<T, E> Stream for State<T>
where
    T: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Into<Error>,
{
    type Item = Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if self.is_readable {
                // part
                trace!("attempting to decode a part");

                if let Some(data) = self.decode()? {
                    trace!("part decoded from buffer");
                    return Poll::Ready(Some(Ok(data)));
                }

                // field stream is ended
                if Flag::Next == self.flag {
                    return Poll::Ready(None);
                }

                // whole stream is ended
                if Flag::Eof == self.flag {
                    if self.buffer.len() > 0 {
                        self.length -= self.buffer.len() as u64;
                        self.buffer.clear();
                    }
                    self.eof = true;
                    return Poll::Ready(None);
                }

                self.is_readable = false;
            }

            trace!("polling data from stream");

            if self.eof {
                self.is_readable = true;
                continue;
            }

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
                self.eof = true;
            }

            self.is_readable = true;
        }
    }
}
