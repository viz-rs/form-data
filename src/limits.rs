/// Various limits on incoming data
#[derive(Debug)]
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
            // 100
            field_name_size: Some(100),
            // 100kb
            field_size: Some(100 * 1024),
            fields: None,
            // 10mb
            file_size: Some(10 * 1024 * 1024),
            files: None,
            parts: None,
            // 200mb
            stream_size: Some(200 * 1024 * 1024),
            buffer_size: Self::DEFAULT_BUFFER_SIZE,
        }
    }
}

impl Limits {
    /// Max number of buffer size
    pub const DEFAULT_BUFFER_SIZE: usize = 8 * 1024;

    /// Max field name size
    pub fn field_name_size(mut self, max: usize) -> Self {
        self.field_name_size.replace(max);
        self
    }

    /// Max field value size
    pub fn field_size(mut self, max: usize) -> Self {
        self.field_size.replace(max);
        self
    }

    /// Max number of non-file fields
    pub fn fields(mut self, max: usize) -> Self {
        self.fields.replace(max);
        self
    }

    /// Max file size
    pub fn file_size(mut self, max: usize) -> Self {
        self.file_size.replace(max);
        self
    }

    /// Max number of file fields
    pub fn files(mut self, max: usize) -> Self {
        self.files.replace(max);
        self
    }

    /// Max number of parts (fields + files)
    pub fn parts(mut self, max: usize) -> Self {
        self.parts.replace(max);
        self
    }

    /// Max number of buffer size
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
    pub fn stream_size(mut self, max: u64) -> Self {
        self.stream_size.replace(max);
        self
    }

    /// Check parts
    pub fn checked_parts(&self, rhs: usize) -> bool {
        matches!(self.parts, Some(max) if rhs > max)
    }

    /// Check fields
    pub fn checked_fields(&self, rhs: usize) -> bool {
        matches!(self.fields, Some(max) if rhs > max)
    }

    /// Check files
    pub fn checked_files(&self, rhs: usize) -> bool {
        matches!(self.files, Some(max) if rhs > max)
    }

    /// Check stream size
    pub fn checked_stream_size(&self, rhs: u64) -> bool {
        matches!(self.stream_size, Some(max) if rhs > max)
    }

    /// Check file size
    pub fn checked_file_size(&self, rhs: usize) -> bool {
        matches!(self.file_size, Some(max) if rhs > max)
    }

    /// Check field size
    pub fn checked_field_size(&self, rhs: usize) -> bool {
        matches!(self.field_size, Some(max) if rhs > max)
    }

    /// Check field name size
    pub fn checked_field_name_size(&self, rhs: usize) -> bool {
        matches!(self.field_name_size, Some(max) if rhs > max)
    }
}
