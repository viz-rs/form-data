use std::{
    error::Error as StdError,
    fs::File,
    io::Write,
    pin::Pin,
    task::{Context, Poll},
};

use bytes::{Bytes, BytesMut};
use futures_util::{
    io::{self, AsyncRead, AsyncWrite, AsyncWriteExt},
    stream::{Stream, TryStreamExt},
};
use http::{
    header::{CONTENT_DISPOSITION, CONTENT_TYPE},
    HeaderValue,
};
use tracing::trace;

use crate::{
    utils::{parse_content_disposition, parse_content_type, parse_part_headers},
    Error, Field, Flag, FormData, Result, State,
};

impl<T, B, E> Stream for State<T>
where
    T: Stream<Item = Result<B, E>> + Unpin,
    B: Into<Bytes>,
    E: Into<Box<dyn StdError + Send + Sync>>,
{
    type Item = Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if self.is_readable {
                // part
                trace!("attempting to decode a part");

                // field
                if let Some(data) = self.decode() {
                    trace!("part decoded from buffer");
                    return Poll::Ready(Some(Ok(data)));
                }

                // field stream is ended
                if Flag::Next == self.flag {
                    return Poll::Ready(None);
                }

                // whole stream is ended
                if Flag::Eof == self.flag {
                    self.length -= self.buffer.len() as u64;
                    self.buffer.clear();
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
                Poll::Ready(Some(Ok(b))) => {
                    let b = b.into();
                    let l = b.len() as u64;

                    if let Some(max) = self.limits.checked_stream_size(self.length + l) {
                        return Poll::Ready(Some(Err(Error::PayloadTooLarge(max))));
                    }

                    self.buffer.extend_from_slice(&b);
                    self.length += l;
                    l
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(Error::BoxError(e.into()))))
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

impl<T, B, E> Field<T>
where
    T: Stream<Item = Result<B, E>> + Unpin,
    B: Into<Bytes>,
    E: Into<Box<dyn StdError + Send + Sync>>,
{
    /// Reads field data to bytes.
    pub async fn bytes(&mut self) -> Result<Bytes> {
        let mut bytes = BytesMut::new();
        while let Some(buf) = self.try_next().await? {
            bytes.extend_from_slice(&buf);
        }
        Ok(bytes.freeze())
    }

    /// Copys large buffer to `AsyncRead`, hyper can support large buffer,
    /// 8KB <= buffer <= 512KB, so if we want to handle large buffer.
    /// `Form::set_max_buf_size(512 * 1024);`
    /// 3~4x performance improvement over the 8KB limitation of `AsyncRead`.
    pub async fn copy_to<W>(&mut self, writer: &mut W) -> Result<u64>
    where
        W: AsyncWrite + Send + Unpin + 'static,
    {
        let mut n = 0;
        while let Some(buf) = self.try_next().await? {
            writer.write_all(&buf).await?;
            n += buf.len();
        }
        writer.flush().await?;
        Ok(n as u64)
    }

    /// Copys large buffer to File, hyper can support large buffer,
    /// 8KB <= buffer <= 512KB, so if we want to handle large buffer.
    /// `Form::set_max_buf_size(512 * 1024);`
    /// 4x+ performance improvement over the 8KB limitation of `AsyncRead`.
    pub async fn copy_to_file(&mut self, file: &mut File) -> Result<u64> {
        let mut n = 0;
        while let Some(buf) = self.try_next().await? {
            n += file.write(&buf)?;
        }
        file.flush()?;
        Ok(n as u64)
    }

    /// Ignores current field data, pass it.
    pub async fn ignore(&mut self) -> Result<()> {
        while let Some(buf) = self.try_next().await? {
            drop(buf);
        }
        Ok(())
    }
}

/// Reads payload data from part, then puts them to anywhere
impl<T, B, E> AsyncRead for Field<T>
where
    T: Stream<Item = Result<B, E>> + Unpin,
    B: Into<Bytes>,
    E: Into<Box<dyn StdError + Send + Sync>>,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        match self.poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => Poll::Ready(Ok(0)),
            Poll::Ready(Some(Ok(b))) => Poll::Ready(Ok(buf.write(&b)?)),
            Poll::Ready(Some(Err(e))) => Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, e))),
        }
    }
}

/// Reads payload data from part, then yields them
impl<T, B, E> Stream for Field<T>
where
    T: Stream<Item = Result<B, E>> + Unpin,
    B: Into<Bytes>,
    E: Into<Box<dyn StdError + Send + Sync>>,
{
    type Item = Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        trace!("polling {} {}", self.index, self.state.is_some());

        let Some(state) = self.state.clone() else {
            return Poll::Ready(None);
        };

        let is_file = self.filename.is_some();
        let mut state = state
            .try_lock()
            .map_err(|e| Error::TryLockError(e.to_string()))?;

        match Pin::new(&mut *state).poll_next(cx)? {
            Poll::Pending => Poll::Pending,
            Poll::Ready(res) => match res {
                None => {
                    if let Some(waker) = state.waker_mut().take() {
                        waker.wake();
                    }
                    trace!("polled {}", self.index);
                    drop(self.state.take());
                    Poll::Ready(None)
                }
                Some(buf) => {
                    let l = buf.len();

                    if is_file {
                        if let Some(max) = state.limits.checked_file_size(self.length + l) {
                            return Poll::Ready(Some(Err(Error::FileTooLarge(max))));
                        }
                    } else if let Some(max) = state.limits.checked_field_size(self.length + l) {
                        return Poll::Ready(Some(Err(Error::FieldTooLarge(max))));
                    }

                    self.length += l;
                    trace!("polled bytes {}/{}", buf.len(), self.length);
                    Poll::Ready(Some(Ok(buf)))
                }
            },
        }
    }
}

/// Reads form-data from request payload body, then yields `Field`
impl<T, B, E> Stream for FormData<T>
where
    T: Stream<Item = Result<B, E>> + Unpin,
    B: Into<Bytes>,
    E: Into<Box<dyn StdError + Send + Sync>>,
{
    type Item = Result<Field<T>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut state = self
            .state
            .try_lock()
            .map_err(|e| Error::TryLockError(e.to_string()))?;

        if state.waker().is_some() {
            return Poll::Pending;
        }

        match Pin::new(&mut *state).poll_next(cx)? {
            Poll::Pending => Poll::Pending,
            Poll::Ready(res) => match res {
                None => {
                    trace!("parse eof");
                    Poll::Ready(None)
                }
                Some(buf) => {
                    trace!("parse part");

                    // too many parts
                    if let Some(max) = state.limits.checked_parts(state.total + 1) {
                        return Poll::Ready(Some(Err(Error::PartsTooMany(max))));
                    }

                    // invalid part header
                    let Ok(mut headers) = parse_part_headers(&buf) else {
                        return Poll::Ready(Some(Err(Error::InvalidHeader)));
                    };

                    // invalid content disposition
                    let Some((name, filename)) = headers
                        .remove(CONTENT_DISPOSITION)
                        .as_ref()
                        .map(HeaderValue::as_bytes)
                        .map(parse_content_disposition)
                        .and_then(Result::ok)
                    else {
                        return Poll::Ready(Some(Err(Error::InvalidContentDisposition)));
                    };

                    // field name is too long
                    if let Some(max) = state.limits.checked_field_name_size(name.len()) {
                        return Poll::Ready(Some(Err(Error::FieldNameTooLong(max))));
                    }

                    if filename.is_some() {
                        // files too many
                        if let Some(max) = state.limits.checked_files(state.files + 1) {
                            return Poll::Ready(Some(Err(Error::FilesTooMany(max))));
                        }
                        state.files += 1;
                    } else {
                        // fields too many
                        if let Some(max) = state.limits.checked_fields(state.fields + 1) {
                            return Poll::Ready(Some(Err(Error::FieldsTooMany(max))));
                        }
                        state.fields += 1;
                    }

                    // yields `Field`
                    let mut field = Field::empty();

                    field.name = name;
                    field.filename = filename;
                    field.index = state.index();
                    field.content_type = parse_content_type(headers.remove(CONTENT_TYPE).as_ref());
                    field.state_mut().replace(self.state());

                    if !headers.is_empty() {
                        field.headers_mut().replace(headers);
                    }

                    // clone waker, if field is polled data, wake it.
                    state.waker_mut().replace(cx.waker().clone());

                    Poll::Ready(Some(Ok(field)))
                }
            },
        }
    }
}
