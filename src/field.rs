use std::fmt;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll};

use anyhow::{anyhow, Error, Result};
use bytes::{Bytes, BytesMut};
use futures_util::stream::{Stream, TryStreamExt};

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

/// Reads payload data from part, then yields them
impl<T, O, E> Stream for Field<T>
where
    T: Stream<Item = Result<O, E>> + Unpin + Send + 'static,
    O: Into<Bytes>,
    E: Into<Error>,
{
    type Item = Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        log::debug!("polling {} {}", self.index.unwrap_or_default(), self.state.is_some());

        if self.state.is_none() {
            return Poll::Ready(None);
        }

        let state = self.state.clone().unwrap();
        let mut state = state.try_lock().map_err(|e| anyhow!(e.to_string()))?;

        match Pin::new(&mut *state).poll_next(cx)? {
            Poll::Ready(res) => match res {
                Some(0) | None => {
                    if let Some(waker) = state.waker_mut().take() {
                        waker.wake();
                    }
                    log::debug!("polled {}", self.index.unwrap_or_default());
                    drop(self.state.take());
                    Poll::Ready(None)
                }
                Some(len) => {
                    // @TODO: need check field payload data length
                    self.length += len as u64;
                    log::debug!("polled bytes {}/{}", len, self.length);
                    Poll::Ready(Some(Ok(state.buffer_mut().split_to(len).freeze())))
                }
            },
            Poll::Pending => Poll::Pending,
        }
    }
}
