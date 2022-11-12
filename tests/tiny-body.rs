//!
//! ```
//! RUST_LOG=trace cargo test --test tiny-body --no-default-features --features="sync" -- --nocapture
//! ```

#![cfg(feature = "sync")]

use std::{fs::File, io::Read, str::FromStr};

use anyhow::Result;

use form_data::*;

#[path = "./lib/mod.rs"]
mod lib;

use lib::{tracing_init, Limited};

#[test]
fn tiny_body() -> Result<()> {
    tracing_init()?;

    let payload = File::open("tests/fixtures/issue-6.txt")?;
    let stream = Limited::random_with(payload, 256);
    let limit = stream.limit();
    tracing::trace!(limit = limit);

    let mut form = FormData::new(
        stream,
        "---------------------------187056119119472771921673485771",
    );
    form.set_max_buf_size(limit)?;

    while let Some(item) = form.next() {
        let mut field = item?;
        assert!(!field.consumed());
        assert_eq!(field.length, 0);
        tracing::trace!("{:?}", field);

        match field.index {
            0 => {
                assert_eq!(field.name, "upload_file");
                assert_eq!(field.filename, Some("font.py".into()));
                assert_eq!(
                    field.content_type,
                    Some(mime::Mime::from_str("text/x-python")?)
                );
            }
            1 => {
                assert_eq!(field.name, "expire");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);

                let mut value = String::new();
                let size = field.read_to_string(&mut value)?;

                tracing::trace!("value: {}, size: {}", value, size);

                assert_eq!(value, "on");
            }
            2 => {
                assert_eq!(field.name, "expireDays");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);

                let mut value = String::new();
                let size = field.read_to_string(&mut value)?;

                tracing::trace!("value: {}, size: {}", value, size);

                assert_eq!(value, "2");
            }
            3 => {
                assert_eq!(field.name, "expireHours");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);

                let mut value = String::new();
                let size = field.read_to_string(&mut value)?;

                tracing::trace!("value: {}, size: {}", value, size);

                assert_eq!(value, "0");
            }
            4 => {
                assert_eq!(field.name, "expireMins");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);

                let mut value = String::new();
                let size = field.read_to_string(&mut value)?;

                tracing::trace!("value: {}, size: {}", value, size);

                assert_eq!(value, "2");
            }
            5 => {
                assert_eq!(field.name, "expireSecs");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);

                let mut value = String::new();
                let size = field.read_to_string(&mut value)?;

                tracing::trace!("value: {}, size: {}", value, size);

                assert_eq!(value, "0");
            }
            _ => {}
        }

        field.ignore()?;
    }

    let state = form.state();
    let state = state
        .try_lock()
        .map_err(|e| Error::TryLockError(e.to_string()))?;

    assert_eq!(state.eof(), true);
    assert_eq!(state.total(), 6);
    assert_eq!(state.len(), 1415);

    Ok(())
}
