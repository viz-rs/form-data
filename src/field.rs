use std::{
    fmt,
    fs::File,
    io::Write,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

use anyhow::{anyhow, Error, Result};
use bytes::{Bytes, BytesMut};
use futures_util::{
    io::{self, AsyncRead, AsyncWrite, AsyncWriteExt},
    stream::{Stream, TryStreamExt},
};

use crate::{FormDataError, State};

/// Field
pub struct Field<T> {
    /// The payload size of Field.
    pub length: usize,
    /// The index of Field.
    pub index: usize,
    /// The name of Field.
    pub name: String,
    /// The filename of Field, optinal.
    pub filename: Option<String>,
    /// The content_type of Field, optinal.
    pub content_type: Option<mime::Mime>,
    /// The extras headers of Field, optinal.
    pub headers: Option<http::HeaderMap>,
    state: Option<Arc<Mutex<State<T>>>>,
}

impl<T, E> Field<T>
where
    T: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Into<Error>,
{
    /// Creates an empty field.
    pub fn empty() -> Self {
        Self {
            index: 0,
            length: 0,
            name: String::new(),
            filename: None,
            content_type: None,
            headers: None,
            state: None,
        }
    }

    /// Gets mutable headers.
    pub fn headers_mut(&mut self) -> &mut Option<http::HeaderMap> {
        &mut self.headers
    }

    /// Gets mutable state.
    pub fn state_mut(&mut self) -> &mut Option<Arc<Mutex<State<T>>>> {
        &mut self.state
    }

    /// Gets the status of state.
    pub fn consumed(&self) -> bool {
        self.state.is_none()
    }

    /// Reads field data to bytes.
    pub async fn bytes(&mut self) -> Result<Bytes> {
        let mut bytes = BytesMut::new();
        while let Some(buf) = self.try_next().await? {
            bytes.extend_from_slice(&buf);
        }
        Ok(bytes.freeze())
    }

    /// Copys large buffer to AsyncRead, hyper can support large buffer,
    /// 8KB <= buffer <= 512KB, so if we want to handle large buffer.
    /// `Form::set_max_buf_size(512 * 1024);`
    /// 3~4x performance improvement over the 8KB limitation of AsyncRead.
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
    /// 4x+ performance improvement over the 8KB limitation of AsyncRead.
    pub async fn copy_to_file(&mut self, mut file: File) -> Result<u64> {
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

impl<T> fmt::Debug for Field<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Field")
            .field("name", &self.name)
            .field("filename", &self.filename)
            .field("content_type", &self.content_type)
            .field("index", &self.index)
            .field("length", &self.length)
            .field("headers", &self.headers)
            .field("consumed", &self.state.is_none())
            .finish()
    }
}

/// Reads payload data from part, then puts them to anywhere
impl<T, E> AsyncRead for Field<T>
where
    T: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Into<Error>,
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
impl<T, E> Stream for Field<T>
where
    T: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Into<Error>,
{
    type Item = Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        tracing::trace!("polling {} {}", self.index, self.state.is_some());

        let state = match self.state.clone() {
            None => return Poll::Ready(None),
            Some(state) => state,
        };

        let is_file = self.filename.is_some();
        let mut state = state.try_lock().map_err(|e| anyhow!(e.to_string()))?;

        match Pin::new(&mut *state).poll_next(cx)? {
            Poll::Pending => Poll::Pending,
            Poll::Ready(res) => match res {
                None => {
                    if let Some(waker) = state.waker_mut().take() {
                        waker.wake();
                    }
                    tracing::trace!("polled {}", self.index);
                    drop(self.state.take());
                    Poll::Ready(None)
                }
                Some(buf) => {
                    let l = buf.len();

                    if is_file {
                        if let Some(max) = state.limits.checked_file_size(self.length + l) {
                            return Poll::Ready(Some(Err(FormDataError::FileTooLarge(max).into())));
                        }
                    } else if let Some(max) = state.limits.checked_field_size(self.length + l) {
                        return Poll::Ready(Some(
                            Err(FormDataError::FieldTooLarge(max).into()),
                        ));
                    }

                    self.length += l;
                    tracing::trace!("polled bytes {}/{}", buf.len(), self.length);
                    Poll::Ready(Some(Ok(buf)))
                }
            },
        }
    }
}
