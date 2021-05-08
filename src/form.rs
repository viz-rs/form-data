use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

use anyhow::{anyhow, Error, Result};
use bytes::Bytes;
use futures_util::stream::Stream;
use http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};

use crate::{
    utils::{parse_content_disposition, parse_content_type, parse_part_headers},
    Field, State,
};

/// FormData
pub struct FormData<T> {
    state: Arc<Mutex<State<T>>>,
}

impl<T> FormData<T> {
    /// Creates new FormData with boundary and stream.
    pub fn new(boundary: &str, t: T) -> Self {
        Self {
            state: Arc::new(Mutex::new(State::new(boundary.as_bytes(), t))),
        }
    }

    /// Gets the state.
    pub fn state(&self) -> Arc<Mutex<State<T>>> {
        self.state.clone()
    }

    /// Sets Buffer max size for reading.
    pub fn set_max_buf_size(&mut self, max: usize) -> Result<()> {
        self.state
            .try_lock()
            .map_err(|e| anyhow!(e.to_string()))?
            .set_max_buf_size(max);
        Ok(())
    }
}

/// Reads form-data from request payload body, then yields `Field`
impl<T, E> Stream for FormData<T>
where
    T: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Into<Error>,
{
    type Item = Result<Field<T>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut state = self.state.try_lock().map_err(|e| anyhow!(e.to_string()))?;

        if state.waker().is_some() {
            return Poll::Pending;
        }

        match Pin::new(&mut *state).poll_next(cx)? {
            Poll::Pending => Poll::Pending,
            Poll::Ready(res) => match res {
                None => {
                    tracing::trace!("parse eof");
                    return Poll::Ready(None);
                }
                Some(buf) => {
                    tracing::trace!("parse part");

                    dbg!(&buf.len());
                    let mut headers = parse_part_headers(&buf)?;
                    dbg!(&headers);

                    let names = headers.remove(CONTENT_DISPOSITION).map_or_else(
                        || Err(anyhow!("invalid content disposition")),
                        |v| parse_content_disposition(&v.as_bytes()),
                    )?;

                    // yields `Field`
                    let mut field = Field::<T>::empty();

                    field.name = names.0;
                    field.filename = names.1;
                    field.index = state.index();
                    field.content_type = parse_content_type(headers.remove(CONTENT_TYPE).as_ref());
                    field.state_mut().replace(self.state());

                    if headers.len() > 0 {
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
