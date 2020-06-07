use std::fs::File;

use anyhow::{anyhow, Result};
use hyper::Body;

use futures_util::{
    // io::{copy, Cursor},
    stream::TryStreamExt,
};

use form_data::*;

mod limited;

use limited::Limited;

#[test]
fn hyper_body() -> Result<()> {
    // pretty_env_logger::try_init()?;

    // dont use `smol::run`, we need Multi-threaded
    smol::block_on(async {
        let txt = smol::reader(File::open("tests/fixtures/many.txt")?);
        // let mut cursor = Cursor::new(Vec::<u8>::new());
        // copy(txt, &mut cursor).await?;

        // cursor.set_position(0);

        // let stream = Limited::random(cursor);
        let stream = Limited::random_with(txt, 256);

        // let chunks = cursor
        //     .into_inner()
        //     .chunks(1)
        //     // .chunks(8 * 1024)
        //     .collect::<Vec<_>>()
        //     .iter()
        //     .map(|v| Ok(v.to_vec()))
        //     .collect::<Vec<Result<_, Error>>>();
        // let stream = stream::iter(chunks);

        let body = Body::wrap_stream(stream);

        let mut form = FormData::new("----WebKitFormBoundaryWLHCs9qmcJJoyjKR", body);

        while let Some(mut field) = form.try_next().await? {
            assert!(!field.consumed());
            assert_eq!(field.length, 0);

            let buffer = field.bytes().await?;

            match field.index {
                Some(0) => {
                    assert_eq!(field.name, "_method");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(buffer, "put\r\n");
                }
                Some(1) => {
                    assert_eq!(field.name, "profile[blog]");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(buffer, "\r\n");
                }
                Some(2) => {
                    assert_eq!(field.name, "profile[public_email]");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(buffer, "\r\n");
                }
                Some(3) => {
                    assert_eq!(field.name, "profile[interests]");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(buffer, "\r\n");
                }
                Some(4) => {
                    assert_eq!(field.name, "profile[bio]");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(buffer, "hello\r\n\r\n\"quote\"\r\n");
                }
                Some(5) => {
                    assert_eq!(field.name, "media");
                    assert_eq!(field.filename, Some(String::new()));
                    assert_eq!(field.content_type, Some(mime::APPLICATION_OCTET_STREAM));
                }
                Some(6) => {
                    assert_eq!(field.name, "commit");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(buffer, "Save\r\n");
                }
                _ => {}
            }

            assert_eq!(field.length, buffer.len() as u64);
            assert!(field.consumed());
        }

        let state = form.state();
        let state = state.try_lock().map_err(|e| anyhow!(e.to_string()))?;

        assert_eq!(state.eof(), true);
        assert_eq!(state.total(), 7);

        Ok(())
    })
}
