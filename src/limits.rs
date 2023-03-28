use serde::{Deserialize, Serialize};

/// Various limits on incoming data
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Limits {
    /// Max field name size
    pub field_name_size: Option<usize>,
    /// Max field value size
    pub field_size: Option<usize>,
    /// Max number of non-file fields
    pub fields: Option<usize>,
    /// Max file size
    pub file_size: Option<usize>,
    /// Max number of file fields
    pub files: Option<usize>,
    /// Max number of parts (fields + files)
    pub parts: Option<usize>,
    /// Max number of whole stream
    pub stream_size: Option<u64>,
    /// Max number of buffer size
    pub buffer_size: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            field_name_size: Some(Self::DEFAULT_FIELD_NAME_SIZE),
            field_size: Some(Self::DEFAULT_FIELD_SIZE),
            fields: None,
            file_size: Some(Self::DEFAULT_FILE_SIZE),
            files: None,
            parts: None,
            stream_size: Some(Self::DEFAULT_STREAM_SIZE),
            buffer_size: Self::DEFAULT_BUFFER_SIZE,
        }
    }
}

impl Limits {
    /// Max number of field name size, defaults to 100.
    pub const DEFAULT_FIELD_NAME_SIZE: usize = 100;

    /// Max number of field value size, defaults to 100KB.
    pub const DEFAULT_FIELD_SIZE: usize = 100 * 1024;

    /// Max number of file size, defaults to 10MB.
    pub const DEFAULT_FILE_SIZE: usize = 10 * 1024 * 1024;

    /// Max number of stream size, defaults to 200MB.
    pub const DEFAULT_STREAM_SIZE: u64 = 200 * 1024 * 1024;

    /// Max number of buffer size, defaults to 8KB
    pub const DEFAULT_BUFFER_SIZE: usize = 8 * 1024;

    /// Max field name size
    #[must_use]
    pub fn field_name_size(mut self, max: usize) -> Self {
        self.field_name_size.replace(max);
        self
    }

    /// Max field value size
    #[must_use]
    pub fn field_size(mut self, max: usize) -> Self {
        self.field_size.replace(max);
        self
    }

    /// Max number of non-file fields
    #[must_use]
    pub fn fields(mut self, max: usize) -> Self {
        self.fields.replace(max);
        self
    }

    /// Max file size
    #[must_use]
    pub fn file_size(mut self, max: usize) -> Self {
        self.file_size.replace(max);
        self
    }

    /// Max number of file fields
    #[must_use]
    pub fn files(mut self, max: usize) -> Self {
        self.files.replace(max);
        self
    }

    /// Max number of parts (fields + files)
    #[must_use]
    pub fn parts(mut self, max: usize) -> Self {
        self.parts.replace(max);
        self
    }

    /// Max number of buffer size
    ///
    /// # Panics
    ///
    /// If `max` is greater than or equal to `Limits::DEFAULT_BUFFER_SIZE`.
    #[must_use]
    pub fn buffer_size(mut self, max: usize) -> Self {
        assert!(
            max >= Self::DEFAULT_BUFFER_SIZE,
            "The max_buffer_size cannot be smaller than {}.",
            Self::DEFAULT_BUFFER_SIZE,
        );

        self.buffer_size = max;
        self
    }

    /// Max number of whole stream size
    #[must_use]
    pub fn stream_size(mut self, max: u64) -> Self {
        self.stream_size.replace(max);
        self
    }

    /// Check parts
    #[must_use]
    pub fn checked_parts(&self, rhs: usize) -> Option<usize> {
        self.parts.filter(|max| rhs > *max)
    }

    /// Check fields
    #[must_use]
    pub fn checked_fields(&self, rhs: usize) -> Option<usize> {
        self.fields.filter(|max| rhs > *max)
    }

    /// Check files
    #[must_use]
    pub fn checked_files(&self, rhs: usize) -> Option<usize> {
        self.files.filter(|max| rhs > *max)
    }

    /// Check stream size
    #[must_use]
    pub fn checked_stream_size(&self, rhs: u64) -> Option<u64> {
        self.stream_size.filter(|max| rhs > *max)
    }

    /// Check file size
    #[must_use]
    pub fn checked_file_size(&self, rhs: usize) -> Option<usize> {
        self.file_size.filter(|max| rhs > *max)
    }

    /// Check field size
    #[must_use]
    pub fn checked_field_size(&self, rhs: usize) -> Option<usize> {
        self.field_size.filter(|max| rhs > *max)
    }

    /// Check field name size
    #[must_use]
    pub fn checked_field_name_size(&self, rhs: usize) -> Option<usize> {
        self.field_name_size.filter(|max| rhs > *max)
    }
}
