use anyhow::Result;
use async_fs::File;
use bytes::BytesMut;
use hyper::Body;
use tempfile::tempdir;

use futures_util::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    stream::{self, TryStreamExt},
};

use form_data::*;

mod lib;

use lib::{tracing_init, Limited};

#[tokio::test]
async fn hyper_body() -> Result<()> {
    tracing_init()?;

    let payload = File::open("tests/fixtures/graphql.txt").await?;
    let stream = Limited::random_with(payload, 256);
    let limit = stream.limit();

    let body = Body::wrap_stream(stream);
    let mut form = FormData::new(body, "------------------------627436eaefdbc285");
    form.set_max_buf_size(limit)?;

    while let Some(mut field) = form.try_next().await? {
        assert!(!field.consumed());
        assert_eq!(field.length, 0);

        match field.index {
            0 => {
                assert_eq!(field.name, "operations");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);

                // reads chunks
                let mut buffer = BytesMut::new();
                while let Some(buf) = field.try_next().await? {
                    buffer.extend_from_slice(&buf);
                }

                assert_eq!(buffer, "[{ \"query\": \"mutation ($file: Upload!) { singleUpload(file: $file) { id } }\", \"variables\": { \"file\": null } }, { \"query\": \"mutation($files: [Upload!]!) { multipleUpload(files: $files) { id } }\", \"variables\": { \"files\": [null, null] } }]");
                assert_eq!(field.length, buffer.len());

                assert!(field.consumed());

                tracing::info!("{:#?}", field);
            }
            1 => {
                assert_eq!(field.name, "map");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);

                // reads bytes
                let buffer = field.bytes().await?;

                assert_eq!(buffer, "{ \"0\": [\"0.variables.file\"], \"1\": [\"1.variables.files.0\"], \"2\": [\"1.variables.files.1\"] }");
                assert_eq!(field.length, buffer.len());

                assert!(field.consumed());

                tracing::info!("{:#?}", field);
            }
            2 => {
                tracing::info!("{:#?}", field);

                assert_eq!(field.name, "0");
                assert_eq!(field.filename, Some("a.txt".into()));
                assert_eq!(field.content_type, Some(mime::TEXT_PLAIN));

                let dir = tempdir()?;

                let filename = field.filename.as_ref().unwrap();
                let filepath = dir.path().join(filename);

                let mut writer = File::create(&filepath).await?;

                let bytes = io::copy(field, &mut writer).await?;
                writer.close().await?;

                // async ?
                let metadata = std::fs::metadata(&filepath)?;
                assert_eq!(metadata.len(), bytes);

                let mut reader = File::open(&filepath).await?;
                let mut contents = Vec::new();
                reader.read_to_end(&mut contents).await?;
                assert_eq!(contents, "Alpha file content.\r\n".as_bytes());

                dir.close()?;
            }
            3 => {
                assert_eq!(field.name, "1");
                assert_eq!(field.filename, Some("b.txt".into()));
                assert_eq!(field.content_type, Some(mime::TEXT_PLAIN));

                let mut buffer = Vec::with_capacity(4);
                let bytes = field.read_to_end(&mut buffer).await?;

                assert_eq!(buffer, "Bravo file content.\r\n".as_bytes());
                assert_eq!(field.length, bytes);
                assert_eq!(field.length, buffer.len());

                tracing::info!("{:#?}", field);
            }
            4 => {
                assert_eq!(field.name, "2");
                assert_eq!(field.filename, Some("c.txt".into()));
                assert_eq!(field.content_type, Some(mime::TEXT_PLAIN));

                let mut string = String::new();
                let bytes = field.read_to_string(&mut string).await?;

                assert_eq!(string, "Charlie file content.\r\n");
                assert_eq!(field.length, bytes);
                assert_eq!(field.length, string.len());

                tracing::info!("{:#?}", field);
            }
            _ => {}
        }
    }

    let state = form.state();
    let state = state
        .try_lock()
        .map_err(|e| Error::TryLockError(e.to_string()))?;

    assert_eq!(state.eof(), true);
    assert_eq!(state.total(), 5);
    assert_eq!(state.len(), 1027);

    Ok(())
}

#[tokio::test]
async fn stream_iter() -> Result<()> {
    let chunks: Vec<Result<_, std::io::Error>> = vec![
        Ok("--00252461d3ab8ff5"),
        Ok("c25834e0bffd6f70"),
        Ok("\r\n"),
        Ok(r#"Content-Disposition: form-data; name="foo""#),
        Ok("\r\n"),
        Ok("\r\n"),
        Ok("bar"),
        Ok("\r\n"),
        Ok("--00252461d3ab8ff5c25834e0bffd6f70"),
        Ok("\r\n"),
        Ok(r#"Content-Disposition: form-data; name="name""#),
        Ok("\r\n"),
        Ok("\r\n"),
        Ok("web"),
        Ok("\r\n"),
        Ok("--00252461d3ab8ff5c25834e0bffd6f70"),
        Ok("--"),
    ];
    let body = hyper::Body::wrap_stream(stream::iter(chunks));
    let mut form = FormData::new(body, "00252461d3ab8ff5c25834e0bffd6f70");

    while let Some(mut field) = form.try_next().await? {
        assert!(!field.consumed());
        assert_eq!(field.length, 0);

        let mut buffer = BytesMut::new();
        while let Some(buf) = field.try_next().await? {
            buffer.extend_from_slice(&buf);
        }

        match field.index {
            0 => {
                assert_eq!(field.name, "foo");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(field.length, 3);
                assert_eq!(buffer, "bar");
            }
            1 => {
                assert_eq!(field.name, "name");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(field.length, 3);
                assert_eq!(buffer, "web");
            }
            _ => {}
        }

        assert_eq!(field.length, buffer.len());
        assert!(field.consumed());

        tracing::info!("{:#?}", field);
    }

    let state = form.state();
    let state = state
        .try_lock()
        .map_err(|e| Error::TryLockError(e.to_string()))?;

    assert_eq!(state.eof(), true);
    assert_eq!(state.total(), 2);
    assert_eq!(state.len(), 211);

    Ok(())
}
