//! Read/write `multipart/form-data`, implemented rfc7578
//! Supports `Stream`, `Future`, `AsyncRead`, `AsyncWrite`
//!
//! Links:
//!     https://tools.ietf.org/html/rfc7578
//!     https://developer.mozilla.org/en-US/docs/Web/API/FormData
//!     https://github.com/jaydenseric/graphql-multipart-request-spec
//!     https://ec.haxx.se/http/http-multipart

#![forbid(unsafe_code, rust_2018_idioms)]
#![deny(nonstandard_style)]
#![warn(missing_docs, missing_doc_code_examples, unreachable_pub)]

mod form;
mod state;
mod field;
mod utils;

pub const MAX_HEADERS: usize = 8 * 2;

pub use form::FormData;
pub use state::State;
pub use field::Field;
