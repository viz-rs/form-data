use std::fs::File;
// use std::thread;

use anyhow::{anyhow, Result};

use bytes::BytesMut;
use http::HeaderMap;

use futures_util::stream::TryStreamExt;

use form_data::*;

mod limited;

use limited::Limited;

#[test]
fn main() -> Result<()> {
    assert!(pretty_env_logger::try_init().is_ok());

    Ok(())
}

#[test]
fn empty() -> Result<()> {
    // dont use `smol::run`, we need Multi-threaded
    smol::block_on(async {
        let body = Limited::random(smol::reader(File::open("tests/fixtures/empty.txt")?));

        let mut form = FormData::new("", body);

        while let Some(mut field) = form.try_next().await? {
            let mut buffer = BytesMut::new();
            while let Some(buf) = field.try_next().await? {
                buffer.extend_from_slice(&buf);
            }
            assert_eq!(buffer.len(), 0);
        }

        let state = form.state();
        let state = state.try_lock().map_err(|e| anyhow!(e.to_string()))?;

        assert_eq!(state.eof(), true);
        assert_eq!(state.total(), 0);
        assert_eq!(state.len(), 0);

        Ok(())
    })
}

#[test]
fn many() -> Result<()> {
    // dont use `smol::run`, we need Multi-threaded
    smol::block_on(async {
        let body = Limited::random(smol::reader(File::open("tests/fixtures/many.txt")?));

        let mut form = FormData::new("----WebKitFormBoundaryWLHCs9qmcJJoyjKR", body);

        while let Some(mut field) = form.try_next().await? {
            assert!(!field.consumed());
            assert_eq!(field.length, 0);

            let mut buffer = BytesMut::new();
            while let Some(buf) = field.try_next().await? {
                buffer.extend_from_slice(&buf);
            }

            match field.index {
                Some(0) => {
                    assert_eq!(field.name, "_method");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(field.length, 5);
                    assert_eq!(buffer, "put\r\n");
                }
                Some(1) => {
                    assert_eq!(field.name, "profile[blog]");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(field.length, 2);
                    assert_eq!(buffer, "\r\n");
                }
                Some(2) => {
                    assert_eq!(field.name, "profile[public_email]");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(field.length, 2);
                    assert_eq!(buffer, "\r\n");
                }
                Some(3) => {
                    assert_eq!(field.name, "profile[interests]");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(field.length, 2);
                    assert_eq!(buffer, "\r\n");
                }
                Some(4) => {
                    assert_eq!(field.name, "profile[bio]");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(field.length, 18);
                    assert_eq!(buffer, "hello\r\n\r\n\"quote\"\r\n");
                }
                Some(5) => {
                    assert_eq!(field.name, "media");
                    assert_eq!(field.filename, Some(String::new()));
                    assert_eq!(field.content_type, Some(mime::APPLICATION_OCTET_STREAM));
                    assert_eq!(field.length, 2);
                    assert_eq!(buffer, "\r\n");
                }
                Some(6) => {
                    assert_eq!(field.name, "commit");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(field.length, 6);
                    assert_eq!(buffer, "Save\r\n");
                }
                _ => {}
            }

            assert_eq!(field.length, buffer.len() as u64);
            assert!(field.consumed());

            log::info!("{:#?}", field);
        }

        let state = form.state();
        let state = state.try_lock().map_err(|e| anyhow!(e.to_string()))?;

        assert_eq!(state.eof(), true);
        assert_eq!(state.total(), 7);
        assert_eq!(state.len(), 809);

        Ok(())
    })
}

#[test]
fn many_noend() -> Result<()> {
    // dont use `smol::run`, we need Multi-threaded
    smol::block_on(async {
        let body = Limited::random(smol::reader(File::open("tests/fixtures/many-noend.txt")?));

        let mut form = FormData::new("----WebKitFormBoundaryWLHCs9qmcJJoyjKR", body);

        while let Some(mut field) = form.try_next().await? {
            assert!(!field.consumed());
            assert_eq!(field.length, 0);

            let mut buffer = BytesMut::new();
            while let Some(buf) = field.try_next().await? {
                buffer.extend_from_slice(&buf);
            }

            match field.index {
                Some(0) => {
                    assert_eq!(field.name, "_method");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(field.length, 5);
                    assert_eq!(buffer, "put\r\n");
                }
                Some(1) => {
                    assert_eq!(field.name, "profile[blog]");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(field.length, 2);
                    assert_eq!(buffer, "\r\n");
                }
                Some(2) => {
                    assert_eq!(field.name, "profile[public_email]");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(field.length, 2);
                    assert_eq!(buffer, "\r\n");
                }
                Some(3) => {
                    assert_eq!(field.name, "profile[interests]");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(field.length, 2);
                    assert_eq!(buffer, "\r\n");
                }
                Some(4) => {
                    assert_eq!(field.name, "profile[bio]");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(field.length, 18);
                    assert_eq!(buffer, "hello\r\n\r\n\"quote\"\r\n");
                }
                Some(5) => {
                    assert_eq!(field.name, "commit");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(field.length, 6);
                    assert_eq!(buffer, "Save\r\n");
                }
                Some(6) => {
                    assert_eq!(field.name, "media");
                    assert_eq!(field.filename, Some(String::new()));
                    assert_eq!(field.content_type, Some(mime::APPLICATION_OCTET_STREAM));
                    assert_eq!(field.length, 2);
                    assert_eq!(buffer, "\r\n");
                }
                _ => {}
            }

            assert_eq!(field.length, buffer.len() as u64);
            assert!(field.consumed());

            log::info!("{:#?}", field);
        }

        let state = form.state();
        let state = state.try_lock().map_err(|e| anyhow!(e.to_string()))?;

        assert_eq!(state.eof(), true);
        assert_eq!(state.total(), 7);
        assert_eq!(state.len(), 767);

        Ok(())
    })
}

#[test]
fn headers() -> Result<()> {
    // dont use `smol::run`, we need Multi-threaded
    smol::block_on(async {
        let body = Limited::random(smol::reader(File::open("tests/fixtures/headers.txt")?));

        let mut form = FormData::new("boundary", body);

        while let Some(mut field) = form.try_next().await? {
            assert!(!field.consumed());
            assert_eq!(field.length, 0);

            let mut buffer = BytesMut::new();
            while let Some(buf) = field.try_next().await? {
                buffer.extend_from_slice(&buf);
            }

            match field.index {
                Some(0) => {
                    assert_eq!(field.name, "operations");
                    assert_eq!(field.filename, Some("graphql.json".into()));
                    assert_eq!(field.content_type, Some(mime::APPLICATION_JSON));
                    assert_eq!(field.length, 15);
                    let mut headers = HeaderMap::new();
                    headers.append(http::header::CONTENT_LENGTH, 13.into());
                    assert_eq!(field.headers, Some(headers));
                    assert_eq!(buffer, "{\"query\": \"\"}\r\n");
                }
                _ => {}
            }

            assert_eq!(field.length, buffer.len() as u64);
            assert!(field.consumed());

            log::info!("{:#?}", field);
        }

        let state = form.state();
        let state = state.try_lock().map_err(|e| anyhow!(e.to_string()))?;

        assert_eq!(state.eof(), true);
        assert_eq!(state.total(), 1);
        assert_eq!(state.len(), 177);

        Ok(())
    })
}

#[test]
fn sample() -> Result<()> {
    // dont use `smol::run`, we need Multi-threaded
    smol::block_on(async {
        let body = Limited::random(smol::reader(File::open("tests/fixtures/sample.txt")?));

        let mut form = FormData::new("--------------------------434049563556637648550474", body);

        while let Some(mut field) = form.try_next().await? {
            assert!(!field.consumed());
            assert_eq!(field.length, 0);

            let mut buffer = BytesMut::new();
            while let Some(buf) = field.try_next().await? {
                buffer.extend_from_slice(&buf);
            }

            match field.index {
                Some(0) => {
                    assert_eq!(field.name, "foo");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, Some(mime::APPLICATION_OCTET_STREAM));
                    assert_eq!(field.length, 5);
                    assert_eq!(buffer, "foo\r\n");
                }
                Some(1) => {
                    assert_eq!(field.name, "bar");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, Some(mime::APPLICATION_OCTET_STREAM));
                    assert_eq!(field.length, 5);
                    assert_eq!(buffer, "bar\r\n");
                }
                Some(2) => {
                    assert_eq!(field.name, "file");
                    assert_eq!(field.filename, Some("tsconfig.json".into()));
                    assert_eq!(field.content_type, Some(mime::APPLICATION_OCTET_STREAM));
                    assert_eq!(field.length, 235);
                    assert_eq!(
                        String::from_utf8_lossy(&buffer).replacen("\r\n", "\n", 12),
                        r#"{
  "compilerOptions": {
    "target": "es2018",
    "baseUrl": ".",
    "paths": {
      "deno": ["./deno.d.ts"],
      "https://*": ["../../.deno/deps/https/*"],
      "http://*": ["../../.deno/deps/http/*"]
    }
  }
}

"#
                    );
                }
                Some(3) => {
                    assert_eq!(field.name, "file2");
                    assert_eq!(field.filename, Some("中文.json".into()));
                    assert_eq!(field.content_type, Some(mime::APPLICATION_OCTET_STREAM));
                    assert_eq!(field.length, 30);
                    assert_eq!(buffer, "{\r\n  \"test\": \"filename\"\r\n}\r\n\r\n");
                }
                Some(4) => {
                    assert_eq!(field.name, "crab");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(field.length, 0);
                    assert_eq!(buffer, "");
                }
                _ => {}
            }

            assert_eq!(field.length, buffer.len() as u64);
            assert!(field.consumed());

            log::info!("{:#?}", field);
        }

        let state = form.state();
        let state = state.try_lock().map_err(|e| anyhow!(e.to_string()))?;

        assert_eq!(state.eof(), true);
        assert_eq!(state.total(), 5);
        assert_eq!(state.len(), 1047);

        Ok(())
    })
}

#[test]
fn sample_lf() -> Result<()> {
    // dont use `smol::run`, we need Multi-threaded
    smol::block_on(async {
        let body = Limited::random(smol::reader(File::open("tests/fixtures/sample.lf.txt")?));

        let mut form = FormData::new("--------------------------434049563556637648550474", body);

        while let Some(mut field) = form.try_next().await? {
            assert!(!field.consumed());
            assert_eq!(field.length, 0);

            let mut buffer = BytesMut::new();
            while let Some(buf) = field.try_next().await? {
                buffer.extend_from_slice(&buf);
            }

            assert_eq!(field.length, buffer.len() as u64);
            assert!(field.consumed());

            log::info!("{:#?}", field);
        }

        let state = form.state();
        let state = state.try_lock().map_err(|e| anyhow!(e.to_string()))?;

        assert_eq!(state.eof(), true);
        assert_eq!(state.total(), 0);
        assert_eq!(state.len(), 1008);

        Ok(())
    })
}

#[test]
fn graphql() -> Result<()> {
    // dont use `smol::run`, we need Multi-threaded
    smol::block_on(async {
        let body = Limited::random(smol::reader(File::open("tests/fixtures/graphql.txt")?));

        let mut form = FormData::new("------------------------627436eaefdbc285", body);

        while let Some(mut field) = form.try_next().await? {
            assert!(!field.consumed());
            assert_eq!(field.length, 0);

            let mut buffer = BytesMut::new();
            while let Some(buf) = field.try_next().await? {
                buffer.extend_from_slice(&buf);
            }

            match field.index {
                Some(0) => {
                    assert_eq!(field.name, "operations");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(field.length, 238);
                    assert_eq!(buffer, "[{ \"query\": \"mutation ($file: Upload!) { singleUpload(file: $file) { id } }\", \"variables\": { \"file\": null } }, { \"query\": \"mutation($files: [Upload!]!) { multipleUpload(files: $files) { id } }\", \"variables\": { \"files\": [null, null] } }]\r\n");
                }
                Some(1) => {
                    assert_eq!(field.name, "map");
                    assert_eq!(field.filename, None);
                    assert_eq!(field.content_type, None);
                    assert_eq!(field.length, 91);
                    assert_eq!(buffer, "{ \"0\": [\"0.variables.file\"], \"1\": [\"1.variables.files.0\"], \"2\": [\"1.variables.files.1\"] }\r\n");
                }
                Some(2) => {
                    assert_eq!(field.name, "0");
                    assert_eq!(field.filename, Some("a.txt".into()));
                    assert_eq!(field.content_type, Some(mime::TEXT_PLAIN));
                    assert_eq!(field.length, 23);
                    assert_eq!(buffer, "Alpha file content.\r\n\r\n");
                }
                Some(3) => {
                    assert_eq!(field.name, "1");
                    assert_eq!(field.filename, Some("b.txt".into()));
                    assert_eq!(field.content_type, Some(mime::TEXT_PLAIN));
                    assert_eq!(field.length, 23);
                    assert_eq!(buffer, "Bravo file content.\r\n\r\n");
                }
                Some(4) => {
                    assert_eq!(field.name, "2");
                    assert_eq!(field.filename, Some("c.txt".into()));
                    assert_eq!(field.content_type, Some(mime::TEXT_PLAIN));
                    assert_eq!(field.length, 25);
                    assert_eq!(buffer, "Charlie file content.\r\n\r\n");
                }
                _ => {}
            }

            assert_eq!(field.length, buffer.len() as u64);
            assert!(field.consumed());

            log::info!("{:#?}", field);
        }

        let state = form.state();
        let state = state.try_lock().map_err(|e| anyhow!(e.to_string()))?;

        assert_eq!(state.eof(), true);
        assert_eq!(state.total(), 5);
        assert_eq!(state.len(), 1031);

        Ok(())
    })
}
