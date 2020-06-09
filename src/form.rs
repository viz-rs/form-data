use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use anyhow::{anyhow, Error, Result};
use bytes::Bytes;
use futures_util::stream::Stream;
use http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};

use crate::utils::{parse_content_disposition, parse_content_type, parse_part_headers};
use crate::Field;
use crate::State;

pub struct FormData<T> {
    state: Arc<Mutex<State<T>>>,
}

impl<T> FormData<T> {
    pub fn new<B: AsRef<[u8]>>(b: B, t: T) -> Self {
        // @TODO: check boundary max size
        Self {
            state: Arc::new(Mutex::new(State::new(b, t))),
        }
    }

    pub fn state(&self) -> Arc<Mutex<State<T>>> {
        self.state.clone()
    }
}

/// Reads form-data from request payload body, then yields `Field`
impl<T, O, E> Stream for FormData<T>
where
    T: Stream<Item = Result<O, E>> + Unpin + Send + 'static,
    O: Into<Bytes>,
    E: Into<Error>,
{
    type Item = Result<Field<T>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let state = self.state();
        let mut state = state.try_lock().map_err(|e| anyhow!(e.to_string()))?;

        if state.waker().is_some() {
            return Poll::Pending;
        }

        match Pin::new(&mut *state).poll_next(cx)? {
            Poll::Pending => Poll::Pending,
            Poll::Ready(res) => match res {
                None => {
                    state.buffer_drop();
                    return Poll::Ready(None);
                }
                Some(buf) => {
                    let mut headers = parse_part_headers(&buf)?;

                    log::debug!("parse headers {:#?}", &buf);

                    let names = headers.remove(CONTENT_DISPOSITION).map_or_else(
                        || Err(anyhow!("invalid content disposition")),
                        |v| parse_content_disposition(&v.as_bytes()),
                    )?;

                    // yields `Field`
                    let mut field = Field::<T>::empty();

                    field.name = names.0;
                    field.filename = names.1;
                    field.content_type = parse_content_type(headers.remove(CONTENT_TYPE).as_ref());
                    field.index.replace(state.incr_index());
                    field.state_mut().replace(self.state.clone());

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
