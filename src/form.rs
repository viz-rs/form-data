#![allow(clippy::module_name_repetitions)]

use std::sync::{Arc, Mutex};

use crate::{Error, Limits, Result, State};

/// `FormData`
pub struct FormData<T> {
    pub(crate) state: Arc<Mutex<State<T>>>,
}

impl<T> FormData<T> {
    /// Creates new `FormData` with boundary.
    #[must_use]
    pub fn new(t: T, boundary: &str) -> Self {
        Self {
            state: Arc::new(Mutex::new(State::new(
                t,
                boundary.as_bytes(),
                Limits::default(),
            ))),
        }
    }

    /// Creates new `FormData` with boundary and limits.
    #[must_use]
    pub fn with_limits(t: T, boundary: &str, limits: Limits) -> Self {
        Self {
            state: Arc::new(Mutex::new(State::new(t, boundary.as_bytes(), limits))),
        }
    }

    /// Gets the state.
    #[must_use]
    pub fn state(&self) -> Arc<Mutex<State<T>>> {
        self.state.clone()
    }

    /// Sets Buffer max size for reading.
    pub fn set_max_buf_size(&self, max: usize) -> Result<()> {
        self.state
            .try_lock()
            .map_err(|e| Error::TryLockError(e.to_string()))?
            .limits_mut()
            .buffer_size = max;

        Ok(())
    }
}
