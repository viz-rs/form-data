use std::fmt;
use std::fs::File;
use std::future::Future;
use std::io::{IoSlice, IoSliceMut, Write};
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll};

use anyhow::{anyhow, Error, Result};
use bytes::{Bytes, BytesMut};
use futures_util::{
    io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    stream::{Stream, TryStreamExt},
};

use crate::State;

pub struct Field<T> {
    pub name: String,
    pub index: Option<usize>,
    pub filename: Option<String>,
    pub content_type: Option<mime::Mime>,
    pub headers: Option<http::HeaderMap>,
    pub length: u64,
    state: Option<Arc<Mutex<State<T>>>>,
}

impl<T> Field<T> {
    pub fn empty() -> Self {
        Self {
            name: String::new(),
            content_type: None,
            filename: None,
            headers: None,
            state: None,
            index: None,
            length: 0,
        }
    }

    pub fn headers_mut(&mut self) -> &mut Option<http::HeaderMap> {
        &mut self.headers
    }

    pub fn state_mut(&mut self) -> &mut Option<Arc<Mutex<State<T>>>> {
        &mut self.state
    }

    pub fn state(&self) -> Result<MutexGuard<'_, State<T>>> {
        self.state
            .as_ref()
            .unwrap()
            .try_lock()
            .map_err(|e| anyhow!(e.to_string()))
    }

    pub fn consumed(&self) -> bool {
        self.state.is_none()
    }

    /// Reads field data to bytes
    pub async fn bytes<O, E>(&mut self) -> Result<Bytes>
    where
        T: Stream<Item = Result<O, E>> + Unpin + Send + 'static,
        O: Into<Bytes>,
        E: Into<Error>,
    {
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
    pub async fn copy_to<O, E, W>(mut self, writer: &mut W) -> Result<u64>
    where
        T: Stream<Item = Result<O, E>> + Unpin + Send + 'static,
        O: Into<Bytes>,
        E: Into<Error>,
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
    pub async fn copy_to_file<O, E>(mut self, mut file: File) -> Result<u64>
    where
        T: Stream<Item = Result<O, E>> + Unpin + Send + 'static,
        O: Into<Bytes>,
        E: Into<Error>,
    {
        // smol::blocking!(async move {
        let mut n = 0;
        while let Some(buf) = self.try_next().await? {
            n += file.write(&buf)?;
        }
        file.flush()?;
        Ok(n as u64)
        // }).await
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
impl<T, O, E> AsyncRead for Field<T>
where
    T: Stream<Item = Result<O, E>> + Unpin + Send + 'static,
    O: Into<Bytes>,
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
impl<T, O, E> Stream for Field<T>
where
    T: Stream<Item = Result<O, E>> + Unpin + Send + 'static,
    O: Into<Bytes>,
    E: Into<Error>,
{
    type Item = Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        log::debug!(
            "polling {} {}",
            self.index.unwrap_or_default(),
            self.state.is_some()
        );

        if self.state.is_none() {
            return Poll::Ready(None);
        }

        let state = self.state.clone().unwrap();
        let mut state = state.try_lock().map_err(|e| anyhow!(e.to_string()))?;

        match Pin::new(&mut *state).poll_next(cx)? {
            Poll::Pending => Poll::Pending,
            Poll::Ready(res) => match res {
                None => {
                    if let Some(waker) = state.waker_mut().take() {
                        waker.wake();
                    }
                    log::debug!("polled {}", self.index.unwrap_or_default());
                    drop(self.state.take());
                    Poll::Ready(None)
                }
                Some(buf) => {
                    // @TODO: need check field payload data length
                    self.length += buf.len() as u64;
                    log::debug!("polled bytes {}/{}", buf.len(), self.length);
                    // Poll::Ready(Some(Ok(state.buffer_mut().split_to(len).freeze())))
                    Poll::Ready(Some(Ok(buf)))
                }
            },
        }
    }
}
