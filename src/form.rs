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
    Field, FormDataError, Limits, State,
};

/// FormData
pub struct FormData<T> {
    state: Arc<Mutex<State<T>>>,
}

impl<T> FormData<T> {
    /// Creates new FormData with boundary.
    pub fn new(t: T, boundary: &str) -> Self {
        Self {
            state: Arc::new(Mutex::new(State::new(
                t,
                boundary.as_bytes(),
                Limits::default(),
            ))),
        }
    }

    /// Creates new FormData with boundary and limits.
    pub fn with_limits(t: T, boundary: &str, limits: Limits) -> Self {
        Self {
            state: Arc::new(Mutex::new(State::new(t, boundary.as_bytes(), limits))),
        }
    }

    /// Gets the state.
    pub fn state(&self) -> Arc<Mutex<State<T>>> {
        self.state.clone()
    }

    /// Sets Buffer max size for reading.
    pub fn set_max_buf_size(&self, max: usize) -> Result<()> {
        self.state
            .try_lock()
            .map_err(|e| anyhow!(e.to_string()))?
            .limits_mut()
            .buffer_size = max;

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
                    Poll::Ready(None)
                }
                Some(buf) => {
                    tracing::trace!("parse part");

                    // too many parts
                    if let Some(max) = state.limits.checked_parts(state.total + 1) {
                        return Poll::Ready(Some(Err(FormDataError::PartsTooMany(max).into())));
                    }

                    // invalid part header
                    let mut headers = match parse_part_headers(&buf) {
                        Ok(h) => h,
                        Err(_) => {
                            return Poll::Ready(Some(Err(FormDataError::InvalidHeader.into())))
                        }
                    };

                    // invalid content disposition
                    let (name, filename) = match headers
                        .remove(CONTENT_DISPOSITION)
                        .and_then(|v| parse_content_disposition(v.as_bytes()).ok())
                    {
                        Some(n) => n,
                        None => {
                            return Poll::Ready(Some(Err(
                                FormDataError::InvalidContentDisposition.into()
                            )))
                        }
                    };

                    // field name is too long
                    if let Some(max) = state.limits.checked_field_name_size(name.len()) {
                        return Poll::Ready(Some(Err(FormDataError::FieldNameTooLong(max).into())));
                    }

                    if filename.is_some() {
                        // files too many
                        if let Some(max) = state.limits.checked_files(state.files + 1) {
                            return Poll::Ready(Some(Err(FormDataError::FilesTooMany(max).into())));
                        }
                        state.files += 1;
                    } else {
                        // fields too many
                        if let Some(max) = state.limits.checked_fields(state.fields + 1) {
                            return Poll::Ready(Some(
                                Err(FormDataError::FieldsTooMany(max).into()),
                            ));
                        }
                        state.fields += 1;
                    }

                    // yields `Field`
                    let mut field = Field::<T>::empty();

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
