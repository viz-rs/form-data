use std::{
    fmt,
    sync::{Arc, Mutex},
};

use crate::State;

/// Field
pub struct Field<T> {
    /// The payload size of Field.
    pub length: usize,
    /// The index of Field.
    pub index: usize,
    /// The name of Field.
    pub name: String,
    /// The filename of Field, optinal.
    pub filename: Option<String>,
    /// The `content_type` of Field, optinal.
    pub content_type: Option<mime::Mime>,
    /// The extras headers of Field, optinal.
    pub headers: Option<http::HeaderMap>,
    pub(crate) state: Option<Arc<Mutex<State<T>>>>,
}

impl<T> Field<T> {
    /// Creates an empty field.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            index: 0,
            length: 0,
            name: String::new(),
            filename: None,
            content_type: None,
            headers: None,
            state: None,
        }
    }

    /// Gets mutable headers.
    #[must_use]
    pub fn headers_mut(&mut self) -> &mut Option<http::HeaderMap> {
        &mut self.headers
    }

    /// Gets mutable state.
    #[must_use]
    pub fn state_mut(&mut self) -> &mut Option<Arc<Mutex<State<T>>>> {
        &mut self.state
    }

    /// Gets the status of state.
    #[must_use]
    pub fn consumed(&self) -> bool {
        self.state.is_none()
    }
}

impl<T> fmt::Debug for Field<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Field")
            .field("name", &self.name)
            .field("filename", &self.filename)
            .field("content_type", &self.content_type)
            .field("index", &self.index)
            .field("length", &self.length)
            .field("headers", &self.headers)
            .field("consumed", &self.state.is_none())
            .finish()
    }
}
