use std::{
    fmt,
    pin::Pin,
    task::{Context, Poll},
};

use anyhow::Result;
use bytes::Bytes;
use futures_util::Stream;
use http_body::{Body, Frame, SizeHint};
use hyper::body::Incoming;

/// Incoming Body from request.
pub struct IncomingBody(Option<Incoming>);

#[allow(dead_code)]
impl IncomingBody {
    /// Creates new Incoming Body
    pub fn new(inner: Option<Incoming>) -> Self {
        Self(inner)
    }

    /// Incoming body has been used
    pub fn used() -> Self {
        Self(None)
    }
}

impl Default for IncomingBody {
    fn default() -> Self {
        Self::used()
    }
}

impl fmt::Debug for IncomingBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IncomingBody").finish()
    }
}

impl Body for IncomingBody {
    type Data = Bytes;
    type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

    #[inline]
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        self.get_mut()
            .0
            .as_mut()
            .map_or(Poll::Ready(None), |inner| {
                Pin::new(inner).poll_frame(cx).map_err(Into::into)
            })
    }

    fn is_end_stream(&self) -> bool {
        self.0.as_ref().map_or(true, |inner| inner.is_end_stream())
    }

    fn size_hint(&self) -> SizeHint {
        self.0
            .as_ref()
            .map_or(SizeHint::with_exact(0), |inner| inner.size_hint())
    }
}

impl Stream for IncomingBody {
    type Item = Result<Bytes, Box<dyn std::error::Error + Send + Sync + 'static>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut()
            .0
            .as_mut()
            .map_or(Poll::Ready(None), |inner| {
                match Pin::new(inner).poll_frame(cx)? {
                    Poll::Ready(Some(f)) => Poll::Ready(f.into_data().map(Ok).ok()),
                    Poll::Ready(None) => Poll::Ready(None),
                    Poll::Pending => Poll::Pending,
                }
            })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.as_ref().map_or((0, None), |inner| {
            let sh = inner.size_hint();
            (sh.lower() as usize, sh.upper().map(|s| s as usize))
        })
    }
}
