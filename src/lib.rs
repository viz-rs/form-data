//! Read/write `multipart/form-data`, implemented rfc7578
//! Supports `Stream`, `Future`, `AsyncRead`, `AsyncWrite`
//!
//! Links:
//!     https://tools.ietf.org/html/rfc7578
//!     https://developer.mozilla.org/en-US/docs/Web/API/FormData
//!     https://github.com/jaydenseric/graphql-multipart-request-spec

mod form;
mod state;
mod field;
mod utils;

pub const MAX_HEADERS: usize = 8 * 2;

pub use form::FormData;
pub use state::State;
pub use field::Field;
