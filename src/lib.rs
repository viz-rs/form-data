//! Read/write `multipart/form-data`, implemented rfc7578
//! Supports `Stream`, `Sink`, `Future`, `AsyncRead`, `AsyncWrite`
//!
//! AsyncRead limit 8KB.
//! https://docs.rs/futures-util/0.3/src/futures_util/io/mod.rs.html#37-40
//! But hyper is ~ 400kb by defaults.
//! https://docs.rs/hyper/0.14/hyper/server/struct.Builder.html#method.http1_max_buf_size
//!
//! Links:
//!     https://tools.ietf.org/html/rfc7578
//!     https://developer.mozilla.org/en-US/docs/Web/API/FormData
//!     https://github.com/jaydenseric/graphql-multipart-request-spec
//!     https://ec.haxx.se/http/http-multipart

#![forbid(unsafe_code)]
#![deny(nonstandard_style)]
#![warn(missing_docs, rustdoc::missing_doc_code_examples, unreachable_pub)]

mod error;
mod field;
mod form;
mod limits;
mod state;
mod utils;

pub use form::FormData;

pub use field::Field;

pub use state::*;

pub use limits::Limits;

pub use error::Error;

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(feature = "async")]
mod r#async;
#[cfg(feature = "async")]
pub use r#async::*;
#[cfg(feature = "sync")]
mod sync;
#[cfg(feature = "sync")]
pub use sync::*;
