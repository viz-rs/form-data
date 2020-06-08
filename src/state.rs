use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use anyhow::{Error, Result};
use bytes::{Buf, Bytes, BytesMut};
use futures_util::stream::Stream;

use crate::utils::{read_until, CR, CRLF, CRLFCRLF, CRLF_DASH_DASH, DASH, LF};

#[derive(Debug, PartialEq)]
enum Flag {
    Header,
    Body,
}

struct Cursor {
    flag: Flag,
    crlf_d_b_crlf: Vec<u8>,
    crlf_d_b_d_crlf: Vec<u8>,
    x: Option<usize>,
    y: Option<usize>,
    z: bool,
}

impl Cursor {
    pub(crate) fn new(boundary: Vec<u8>) -> Self {
        // `\r\n--boundary\r\n`
        let mut crlf_d_b_crlf = boundary.clone();
        crlf_d_b_crlf.insert(0, DASH);
        crlf_d_b_crlf.insert(0, DASH);
        crlf_d_b_crlf.insert(0, LF);
        crlf_d_b_crlf.insert(0, CR);

        // `\r\n--boundary--\r\n`
        let mut crlf_d_b_d_crlf = crlf_d_b_crlf.clone();

        crlf_d_b_crlf.push(CR);
        crlf_d_b_crlf.push(LF);

        crlf_d_b_d_crlf.push(DASH);
        crlf_d_b_d_crlf.push(DASH);
        crlf_d_b_d_crlf.push(CR);
        crlf_d_b_d_crlf.push(LF);

        Self {
            flag: Flag::Body,
            crlf_d_b_crlf,
            crlf_d_b_d_crlf,
            x: None,
            y: None,
            z: false,
        }
    }
}

impl fmt::Debug for Cursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cursor")
            .field("flag", &self.flag)
            .field("x", &self.x)
            .field("y", &self.y)
            .field("z", &self.z)
            .field(
                "crlf_dash_boundary_crlf",
                &String::from_utf8_lossy(&self.crlf_d_b_crlf),
            )
            .field("crlf_dash_boundary_crlf_len", &self.crlf_d_b_crlf.len())
            .field(
                "crlf_dash_boundary_dash_crlf",
                &String::from_utf8_lossy(&self.crlf_d_b_d_crlf),
            )
            .field(
                "crlf_dash_boundary_dash_crlf_len",
                &self.crlf_d_b_d_crlf.len(),
            )
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
    cursor: Cursor,
}

impl<T> State<T> {
    pub fn new<B: AsRef<[u8]>>(b: B, io: T) -> Self {
        // `boundary`
        let boundary = b.as_ref().to_owned();
        let cursor = Cursor::new(boundary.to_owned());

        Self {
            io,
            boundary,
            length: 0,
            eof: false,
            index: None,
            waker: None,
            buffer: None,
            cursor,
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

    pub fn buffer_drop(&mut self) {
        if let Some(b) = self.buffer.take() {
            drop(b);
        }
    }

    pub fn eof(&self) -> bool {
        self.eof
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

    /// `boundary`
    fn boundary(&self) -> &[u8] {
        &self.boundary
    }

    /// `\r\n--boundary\r\n`
    fn crlf_d_b_crlf(&self) -> &[u8] {
        &self.cursor.crlf_d_b_crlf
    }

    /// 4: `\r\n--\r\n`
    fn crlf_d_b_crlf_len(&self) -> usize {
        // self.boundary.len() + 2 + 2 + 2
        self.cursor.crlf_d_b_crlf.len()
    }

    /// `\r\n--boundary--\r\n`
    fn crlf_d_b_d_crlf(&self) -> &[u8] {
        &self.cursor.crlf_d_b_d_crlf
    }

    /// 4: `\r\n----\r\n`
    fn crlf_d_b_d_crlf_len(&self) -> usize {
        // self.boundary.len() + 2 + 2 + 2 + 2
        self.cursor.crlf_d_b_d_crlf.len()
    }
}

impl<T> fmt::Debug for State<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("State")
            .field("boundary", &String::from_utf8_lossy(self.boundary()))
            .field("eof", &self.eof)
            .field("length", &self.length)
            .field("total", &self.index)
            .field("cursor", &self.cursor)
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
    type Item = Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        log::debug!("poll stream");

        if self.buffer.is_none() {
            // placeholder `\r\n` , let first boundary is `\r\n--boundary`
            self.buffer.replace(BytesMut::from(&CRLF[..]));
        }

        loop {
            if Flag::Body == self.cursor.flag {
                // `\r\n--`
                if self.cursor.x == None {
                    self.cursor.x = read_until(self.buffer(), CRLF_DASH_DASH);
                }

                if let Some(mut x) = self.cursor.x {
                    if self.index == None && x > 0 {
                        self.buffer_mut().advance(x);
                        x = 0;
                        self.cursor.x.replace(x);
                    }

                    // `\r\n--boundary\r\n`
                    if self.cursor.y == None {
                        self.cursor.y = read_until(self.buffer(), self.crlf_d_b_crlf());
                    }

                    // found new part
                    if let Some(y) = self.cursor.y {
                        self.cursor.x = None;
                        self.cursor.flag = Flag::Header;

                        if self.index.is_some() {
                            // previous part is end
                            if y == 0 {
                                return Poll::Ready(None);
                            }

                            // last data of previous part
                            self.cursor.z = true;
                            self.cursor.y = None;
                            return Poll::Ready(Some(Ok(self.buffer_mut().split_to(y).freeze())));
                        }
                    }

                    if Flag::Body == self.cursor.flag {
                        // keep consume data of current part
                        if x > 0 {
                            self.cursor.x = None;
                            return Poll::Ready(Some(Ok(self.buffer_mut().split_to(x).freeze())));
                        }

                        // payload data is end
                        if let Some(z) = read_until(self.buffer(), self.crlf_d_b_d_crlf()) {
                            self.eof = true;
                            self.cursor.x = None;
                            self.cursor.y = None;
                            self.cursor.flag = Flag::Body;

                            if z == 0 {
                                let n = self.crlf_d_b_d_crlf_len();
                                self.buffer_mut().advance(n);
                                self.length -= self.buffer().len() as u64;
                                self.buffer_mut().clear();
                                return Poll::Ready(None);
                            } else {
                                // last data of last part
                                return Poll::Ready(Some(Ok(self
                                    .buffer_mut()
                                    .split_to(z)
                                    .freeze())));
                            }
                        }
                    }
                }
            }

            if Flag::Header == self.cursor.flag {
                if self.cursor.z {
                    self.cursor.z = false;
                    return Poll::Ready(None);
                }

                if let Some(h) = read_until(self.buffer(), CRLFCRLF) {
                    self.cursor.x = None;
                    self.cursor.y = None;
                    self.cursor.flag = Flag::Body;
                    return Poll::Ready(Some(Ok(self
                        .buffer_mut()
                        .split_to(h + 4)
                        .split_off(self.crlf_d_b_crlf_len())
                        .freeze())));
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
                    // @TODO: need check field payload data length
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
