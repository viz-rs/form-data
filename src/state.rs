use std::fmt;

#[cfg(feature = "async")]
use std::task::Waker;

use bytes::{Buf, Bytes, BytesMut};
use memchr::memmem;

use crate::{
    utils::{CRLF, CRLFS, DASHES},
    Limits,
};

#[derive(Debug, PartialEq)]
pub(crate) enum Flag {
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
    pub(crate) eof: bool,
    pub(crate) flag: Flag,
    pub(crate) length: u64,
    pub(crate) buffer: BytesMut,
    delimiter: Bytes,
    pub(crate) is_readable: bool,
    #[cfg(feature = "async")]
    waker: Option<Waker>,
    pub(crate) total: usize,
    pub(crate) files: usize,
    pub(crate) fields: usize,
    pub(crate) limits: Limits,
}

impl<T> State<T> {
    /// Creates new State.
    pub fn new(io: T, boundary: &[u8], limits: Limits) -> Self {
        // `\r\n--boundary`
        let mut delimiter = BytesMut::with_capacity(4 + boundary.len());
        delimiter.extend_from_slice(&CRLF);
        delimiter.extend_from_slice(&DASHES);
        delimiter.extend_from_slice(boundary);

        // `\r\n`
        let mut buffer = BytesMut::with_capacity(limits.buffer_size);
        buffer.extend_from_slice(&CRLF);

        Self {
            io,
            limits,
            total: 0,
            files: 0,
            fields: 0,
            length: 0,

            #[cfg(feature = "async")]
            waker: None,
            eof: false,
            is_readable: false,

            buffer,
            flag: Flag::Delimiting(false),
            delimiter: delimiter.freeze(),
        }
    }

    /// Gets io.
    pub fn io_mut(&mut self) -> &mut T {
        &mut self.io
    }

    /// Gets waker.
    #[cfg(feature = "async")]
    pub fn waker(&self) -> Option<&Waker> {
        self.waker.as_ref()
    }

    /// Gets waker.
    #[cfg(feature = "async")]
    pub fn waker_mut(&mut self) -> &mut Option<Waker> {
        &mut self.waker
    }

    /// Gets limits.
    pub fn limits_mut(&mut self) -> &mut Limits {
        &mut self.limits
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

    /// Gets bool of the form-data's length is zero.
    pub fn is_empty(&self) -> bool {
        self.length == 0
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

    pub(crate) fn decode(&mut self) -> Option<Bytes> {
        if let Flag::Delimiting(boding) = self.flag {
            if let Some(n) = memmem::find(&self.buffer, &self.delimiter) {
                self.flag = Flag::Heading(n);
            } else {
                // Empty Request Body
                if self.eof && self.buffer.len() == 2 && self.buffer[..2] == CRLF {
                    self.buffer.advance(2);
                    self.flag = Flag::Eof;
                    return None;
                }

                // Empty Part Body
                if memmem::find(&self.buffer, &self.delimiter[2..]).is_some() {
                    self.flag = Flag::Next;
                    self.buffer.advance(self.delimiter.len() - 2);
                    return None;
                }

                // Reading Part Body
                if boding {
                    // Returns buffer with `max_buf_size`
                    if self.limits.buffer_size + self.delimiter.len() < self.buffer.len() {
                        return Some(self.buffer.split_to(self.limits.buffer_size).freeze());
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
                    return None;
                }
                // prev part last data
                let buf = self.buffer.split_to(*n).freeze();
                *n = 0;
                return Some(buf);
            }
        }

        if Flag::Next == self.flag {
            self.flag = Flag::Headed;
        }

        if Flag::Headed == self.flag && self.buffer.len() > 1 {
            if self.buffer[..2] == CRLF {
                self.buffer.advance(2);
                self.flag = Flag::Header;
            } else if self.buffer[..2] == DASHES {
                self.buffer.advance(2);
                self.flag = Flag::Eof;
                return None;
            } else {
                // We dont parse other format, like `\n`
                self.length -= (self.delimiter.len() - 2) as u64;
                self.flag = Flag::Eof;
                return None;
            }
        }

        if Flag::Header == self.flag {
            if let Some(n) = memmem::find(&self.buffer, &CRLFS) {
                self.flag = Flag::Delimiting(true);
                return Some(self.buffer.split_to(n + CRLFS.len()).freeze());
            }
        }

        None
    }
}

impl<T> fmt::Debug for State<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("State")
            .field("eof", &self.eof)
            .field("flag", &self.flag)
            .field("total", &self.total)
            .field("files", &self.files)
            .field("fields", &self.fields)
            .field("length", &self.length)
            .field("limits", &self.limits)
            .field("is_readable", &self.is_readable)
            .field("boundary", &String::from_utf8_lossy(self.boundary()))
            .finish_non_exhaustive()
    }
}
