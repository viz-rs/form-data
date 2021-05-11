use std::io;
use thiserror::Error;

/// Form-data Error
#[derive(Debug, Error)]
pub enum FormDataError {
    /// IO Error
    #[error(transparent)]
    Stream(#[from] io::Error),

    /// Invalid part header
    #[error("invalid part header")]
    InvalidHeader,

    /// Invalid content disposition
    #[error("invalid content disposition")]
    InvalidContentDisposition,

    /// Payload too large
    #[error("payload is too large, limit to `{0}`")]
    PayloadTooLarge(u64),

    /// File too large
    #[error("file is too large, limit to `{0}`")]
    FileTooLarge(usize),

    /// Field too large
    #[error("field is too large, limit to `{0}`")]
    FieldTooLarge(usize),

    /// Parts too many
    #[error("parts is too many, limit to `{0}`")]
    PartsTooMany(usize),

    /// Fields too many
    #[error("fields is too many, limit to `{0}`")]
    FieldsTooMany(usize),

    /// Files too many
    #[error("files is too many, limit to `{0}`")]
    FilesTooMany(usize),

    /// Field name is too long
    #[error("field name is too long, limit to `{0}`")]
    FieldNameTooLong(usize),
}
