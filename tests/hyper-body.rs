use std::fs::{self, File};

use anyhow::{anyhow, Result};
use bytes::BytesMut;
use hyper::Body;

use futures_util::io::{self, AsyncReadExt, AsyncWriteExt};
use futures_util::stream::TryStreamExt;

use form_data::*;

mod limited;
use limited::Limited;

#[test]
fn hyper_body() -> Result<()> {
    pretty_env_logger::try_init()?;

    smol::block_on(async {
        let payload = smol::reader(File::open("tests/fixtures/graphql.txt")?);
        let stream = Limited::random_with(payload, 256);

        let body = Body::wrap_stream(stream);
        let mut form = FormData::new("------------------------627436eaefdbc285", body);

        while let Some(mut field) = form.try_next().await? {
            assert!(!field.consumed());
            assert_eq!(field.length, 0);

            match field.index {
                Some(0) => {
                    assert_eq!(field.name, "operations");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);

                    // reads chunks
                    let mut buffer = BytesMut::new();
                    while let Some(buf) = field.try_next().await? {
                        buffer.extend_from_slice(&buf);
                    }

                    assert_eq!(buffer, "[{ \"query\": \"mutation ($file: Upload!) { singleUpload(file: $file) { id } }\", \"variables\": { \"file\": null } }, { \"query\": \"mutation($files: [Upload!]!) { multipleUpload(files: $files) { id } }\", \"variables\": { \"files\": [null, null] } }]");
                    assert_eq!(field.length, buffer.len() as u64);

                    assert!(field.consumed());

                    log::info!("{:#?}", field);
                }
                Some(1) => {
                    assert_eq!(field.name, "map");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);

                    // reads bytes
                    let buffer = field.bytes().await?;

                    assert_eq!(buffer, "{ \"0\": [\"0.variables.file\"], \"1\": [\"1.variables.files.0\"], \"2\": [\"1.variables.files.1\"] }");
                    assert_eq!(field.length, buffer.len() as u64);

                    assert!(field.consumed());

                    log::info!("{:#?}", field);
                }
                Some(2) => {
                    log::info!("{:#?}", field);

                    assert_eq!(field.name, "0");
                    assert_eq!(field.filename, Some("a.txt".into()));
                    assert_eq!(field.content_type, Some(mime::TEXT_PLAIN));

                    fs::create_dir("tests/fixtures/tmp")?;

                    let filename = field.filename.as_ref().unwrap();
                    let filepath = format!("tests/fixtures/tmp/{}", filename);

                    let mut writer = smol::writer(File::create(&filepath)?);

                    let bytes = io::copy(field, &mut writer).await?;
                    writer.close().await?;

                    // async ?
                    let metadata = fs::metadata(&filepath)?;
                    assert_eq!(metadata.len(), bytes);

                    let mut reader = smol::reader(File::open(&filepath)?);
                    let mut contents = Vec::new();
                    reader.read_to_end(&mut contents).await?;
                    assert_eq!(contents, "Alpha file content.\r\n".as_bytes());

                    fs::remove_file(filepath)?;
                    fs::remove_dir("tests/fixtures/tmp")?;
                }
                Some(3) => {
                    assert_eq!(field.name, "1");
                    assert_eq!(field.filename, Some("b.txt".into()));
                    assert_eq!(field.content_type, Some(mime::TEXT_PLAIN));

                    let mut buffer = Vec::with_capacity(4);
                    let bytes = field.read_to_end(&mut buffer).await?;

                    assert_eq!(buffer, "Bravo file content.\r\n".as_bytes());
                    assert_eq!(field.length, bytes as u64);
                    assert_eq!(field.length, buffer.len() as u64);

                    log::info!("{:#?}", field);
                }
                Some(4) => {
                    assert_eq!(field.name, "2");
                    assert_eq!(field.filename, Some("c.txt".into()));
                    assert_eq!(field.content_type, Some(mime::TEXT_PLAIN));

                    let mut string = String::new();
                    let bytes = field.read_to_string(&mut string).await?;

                    assert_eq!(string, "Charlie file content.\r\n");
                    assert_eq!(field.length, bytes as u64);
                    assert_eq!(field.length, string.len() as u64);

                    log::info!("{:#?}", field);
                }
                _ => {}
            }
        }

        let state = form.state();
        let state = state.try_lock().map_err(|e| anyhow!(e.to_string()))?;

        assert_eq!(state.eof(), true);
        assert_eq!(state.total(), 5);
        assert_eq!(state.len(), 1029);

        Ok(())
    })
}
