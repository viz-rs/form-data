use anyhow::Result;
use async_fs::File;

use bytes::BytesMut;
use http::HeaderMap;

use futures_util::stream::TryStreamExt;

use form_data::*;

mod lib;

use lib::Limited;

#[tokio::test]

async fn from_bytes_stream() -> Result<()> {
    let body = Limited::random(File::open("tests/fixtures/rfc7578-example.txt").await?);
    let mut form = FormData::new(body, "AaB03x");

    while let Some(mut field) = form.try_next().await? {
        let mut buffer = BytesMut::new();
        while let Some(buf) = field.try_next().await? {
            buffer.extend_from_slice(&buf);
        }
        assert_eq!(buffer.len(), "Joe owes =E2=82=AC100.".len());
    }

    let state = form.state();
    let state = state
        .try_lock()
        .map_err(|e| Error::TryLockError(e.to_string()))?;

    assert!(state.eof());
    assert_eq!(state.total(), 1);
    assert_eq!(state.len(), 178);

    Ok(())
}

#[tokio::test]
async fn empty() -> Result<()> {
    let body = Limited::random(File::open("tests/fixtures/empty.txt").await?);

    let mut form = FormData::new(body, "");

    while let Some(mut field) = form.try_next().await? {
        let mut buffer = BytesMut::new();
        while let Some(buf) = field.try_next().await? {
            buffer.extend_from_slice(&buf);
        }
        assert_eq!(buffer.len(), 0);
    }

    let state = form.state();
    let state = state
        .try_lock()
        .map_err(|e| Error::TryLockError(e.to_string()))?;

    assert!(state.eof());
    assert_eq!(state.total(), 0);
    assert_eq!(state.len(), 0);

    Ok(())
}

#[tokio::test]
async fn filename_with_space() -> Result<()> {
    let body = Limited::random(File::open("tests/fixtures/filename-with-space.txt").await?);
    let limit = body.limit();

    let mut form = FormData::new(body, "------------------------d74496d66958873e");
    form.set_max_buf_size(limit)?;

    while let Some(mut field) = form.try_next().await? {
        assert!(!field.consumed());
        assert_eq!(field.length, 0);

        let mut buffer = BytesMut::new();
        while let Some(buf) = field.try_next().await? {
            buffer.extend_from_slice(&buf);

            match field.index {
                0 => {
                    assert_eq!(field.name, "person");
                    assert_eq!(field.content_type, None);
                    assert_eq!(field.length, 9);
                    assert_eq!(buffer, "anonymous");
                }
                1 => {
                    assert_eq!(field.name, "secret");
                    assert_eq!(field.filename, Some("foo bar.txt".to_string()));
                    assert_eq!(field.content_type, Some(mime::TEXT_PLAIN));
                    assert_eq!(field.length, 20);
                    assert_eq!(buffer, "contents of the file");
                }
                _ => {}
            }
        }
    }

    let state = form.state();
    let state = state
        .try_lock()
        .map_err(|e| Error::TryLockError(e.to_string()))?;

    assert!(state.eof());
    assert_eq!(state.total(), 2);
    assert_eq!(state.len(), 313);

    Ok(())
}

#[tokio::test]
async fn many() -> Result<()> {
    let body = Limited::random(File::open("tests/fixtures/many.txt").await?);
    let limit = body.limit();

    let mut form = FormData::new(body, "----WebKitFormBoundaryWLHCs9qmcJJoyjKR");
    form.set_max_buf_size(limit)?;

    while let Some(mut field) = form.try_next().await? {
        assert!(!field.consumed());
        assert_eq!(field.length, 0);

        let mut buffer = BytesMut::new();
        while let Some(buf) = field.try_next().await? {
            buffer.extend_from_slice(&buf);
        }

        match field.index {
            0 => {
                assert_eq!(field.name, "_method");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(field.length, 3);
                assert_eq!(buffer, "put");
            }
            1 => {
                assert_eq!(field.name, "profile[blog]");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(field.length, 0);
                assert_eq!(buffer, "");
            }
            2 => {
                assert_eq!(field.name, "profile[public_email]");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(field.length, 0);
                assert_eq!(buffer, "");
            }
            3 => {
                assert_eq!(field.name, "profile[interests]");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(field.length, 0);
                assert_eq!(buffer, "");
            }
            4 => {
                assert_eq!(field.name, "profile[bio]");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(field.length, 16);
                assert_eq!(buffer, "hello\r\n\r\n\"quote\"");
            }
            5 => {
                assert_eq!(field.name, "media");
                assert_eq!(field.filename, Some(String::new()));
                assert_eq!(field.content_type, Some(mime::APPLICATION_OCTET_STREAM));
                assert_eq!(field.length, 0);
                assert_eq!(buffer, "");
            }
            6 => {
                assert_eq!(field.name, "commit");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(field.length, 4);
                assert_eq!(buffer, "Save");
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

    assert!(state.eof());
    assert_eq!(state.total(), 7);
    assert_eq!(state.len(), 809);

    Ok(())
}

#[tokio::test]
async fn many_noend() -> Result<()> {
    let body = Limited::random(File::open("tests/fixtures/many-noend.txt").await?);
    let limit = body.limit();

    let mut form = FormData::new(body, "----WebKitFormBoundaryWLHCs9qmcJJoyjKR");
    form.set_max_buf_size(limit)?;

    while let Some(mut field) = form.try_next().await? {
        assert!(!field.consumed());
        assert_eq!(field.length, 0);

        let mut buffer = BytesMut::new();
        while let Some(buf) = field.try_next().await? {
            buffer.extend_from_slice(&buf);
        }

        match field.index {
            0 => {
                assert_eq!(field.name, "_method");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(field.length, 3);
                assert_eq!(buffer, "put");
            }
            1 => {
                assert_eq!(field.name, "profile[blog]");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(field.length, 0);
                assert_eq!(buffer, "");
            }
            2 => {
                assert_eq!(field.name, "profile[public_email]");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(field.length, 0);
                assert_eq!(buffer, "");
            }
            3 => {
                assert_eq!(field.name, "profile[interests]");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(field.length, 0);
                assert_eq!(buffer, "");
            }
            4 => {
                assert_eq!(field.name, "profile[bio]");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(field.length, 16);
                assert_eq!(buffer, "hello\r\n\r\n\"quote\"");
            }
            5 => {
                assert_eq!(field.name, "commit");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(field.length, 4);
                assert_eq!(buffer, "Save");
            }
            6 => {
                assert_eq!(field.name, "media");
                assert_eq!(field.filename, Some(String::new()));
                assert_eq!(field.content_type, Some(mime::APPLICATION_OCTET_STREAM));
                assert_eq!(field.length, 0);
                assert_eq!(buffer, "");
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

    assert!(state.eof());
    assert_eq!(state.total(), 7);
    assert_eq!(state.len(), 767);

    Ok(())
}

#[tokio::test]
async fn headers() -> Result<()> {
    let body = Limited::random(File::open("tests/fixtures/headers.txt").await?);
    let limit = body.limit();

    let mut form = FormData::new(body, "boundary");
    form.set_max_buf_size(limit)?;

    while let Some(mut field) = form.try_next().await? {
        assert!(!field.consumed());
        assert_eq!(field.length, 0);

        let mut buffer = BytesMut::new();
        while let Some(buf) = field.try_next().await? {
            buffer.extend_from_slice(&buf);
        }

        match field.index {
            0 => {
                assert_eq!(field.name, "operations");
                assert_eq!(field.filename, Some("graphql.json".into()));
                assert_eq!(field.content_type, Some(mime::APPLICATION_JSON));
                assert_eq!(field.length, 13);
                let mut headers = HeaderMap::new();
                headers.append(http::header::CONTENT_LENGTH, 13.into());
                assert_eq!(field.headers, Some(headers));
                assert_eq!(buffer, "{\"query\": \"\"}");
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

    assert!(state.eof());
    assert_eq!(state.total(), 1);
    assert_eq!(state.len(), 175);

    Ok(())
}

#[tokio::test]
async fn sample() -> Result<()> {
    let body = Limited::random(File::open("tests/fixtures/sample.txt").await?);
    let limit = body.limit();

    let mut form = FormData::new(body, "--------------------------434049563556637648550474");
    form.set_max_buf_size(limit)?;

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
                assert_eq!(field.content_type, Some(mime::APPLICATION_OCTET_STREAM));
                assert_eq!(field.length, 3);
                assert_eq!(buffer, "foo");
            }
            1 => {
                assert_eq!(field.name, "bar");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, Some(mime::APPLICATION_OCTET_STREAM));
                assert_eq!(field.length, 3);
                assert_eq!(buffer, "bar");
            }
            2 => {
                assert_eq!(field.name, "file");
                assert_eq!(field.filename, Some("tsconfig.json".into()));
                assert_eq!(field.content_type, Some(mime::APPLICATION_OCTET_STREAM));
                assert_eq!(field.length, 233);
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
            3 => {
                assert_eq!(field.name, "file2");
                assert_eq!(field.filename, Some("中文.json".into()));
                assert_eq!(field.content_type, Some(mime::APPLICATION_OCTET_STREAM));
                assert_eq!(field.length, 28);
                assert_eq!(buffer, "{\r\n  \"test\": \"filename\"\r\n}\r\n");
            }
            4 => {
                assert_eq!(field.name, "crab");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(buffer, "");
                assert_eq!(field.length, 0);
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

    assert!(state.eof());
    assert_eq!(state.total(), 5);
    assert_eq!(state.len(), 1043);

    Ok(())
}

#[tokio::test]
async fn sample_lf() -> Result<()> {
    let body = Limited::random(File::open("tests/fixtures/sample.lf.txt").await?);
    let limit = body.limit();

    let mut form = FormData::new(body, "--------------------------434049563556637648550474");
    form.set_max_buf_size(limit)?;

    while let Some(mut field) = form.try_next().await? {
        assert!(!field.consumed());
        assert_eq!(field.length, 0);

        let mut buffer = BytesMut::new();
        while let Some(buf) = field.try_next().await? {
            buffer.extend_from_slice(&buf);
        }

        assert_eq!(field.length, buffer.len());
        assert!(field.consumed());

        tracing::info!("{:#?}", field);
    }

    let state = form.state();
    let state = state
        .try_lock()
        .map_err(|e| Error::TryLockError(e.to_string()))?;

    assert!(state.eof());
    assert_eq!(state.total(), 0);
    assert_eq!(state.len(), 0);

    Ok(())
}

#[tokio::test]
async fn graphql_random() -> Result<()> {
    let body = Limited::random(File::open("tests/fixtures/graphql.txt").await?);
    let limit = body.limit();

    let mut form = FormData::new(body, "------------------------627436eaefdbc285");
    form.set_max_buf_size(limit)?;

    while let Some(mut field) = form.try_next().await? {
        assert!(!field.consumed());
        assert_eq!(field.length, 0);

        let mut buffer = BytesMut::new();
        while let Some(buf) = field.try_next().await? {
            buffer.extend_from_slice(&buf);
        }

        match field.index {
            0 => {
                assert_eq!(field.name, "operations");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(field.length, 236);
                assert_eq!(buffer, "[{ \"query\": \"mutation ($file: Upload!) { singleUpload(file: $file) { id } }\", \"variables\": { \"file\": null } }, { \"query\": \"mutation($files: [Upload!]!) { multipleUpload(files: $files) { id } }\", \"variables\": { \"files\": [null, null] } }]");
            }
            1 => {
                assert_eq!(field.name, "map");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(field.length, 89);
                assert_eq!(buffer, "{ \"0\": [\"0.variables.file\"], \"1\": [\"1.variables.files.0\"], \"2\": [\"1.variables.files.1\"] }");
            }
            2 => {
                assert_eq!(field.name, "0");
                assert_eq!(field.filename, Some("a.txt".into()));
                assert_eq!(field.content_type, Some(mime::TEXT_PLAIN));
                assert_eq!(field.length, 21);
                assert_eq!(buffer, "Alpha file content.\r\n");
            }
            3 => {
                assert_eq!(field.name, "1");
                assert_eq!(field.filename, Some("b.txt".into()));
                assert_eq!(field.content_type, Some(mime::TEXT_PLAIN));
                assert_eq!(field.length, 21);
                assert_eq!(buffer, "Bravo file content.\r\n");
            }
            4 => {
                assert_eq!(field.name, "2");
                assert_eq!(field.filename, Some("c.txt".into()));
                assert_eq!(field.content_type, Some(mime::TEXT_PLAIN));
                assert_eq!(field.length, 23);
                assert_eq!(buffer, "Charlie file content.\r\n");
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

    assert!(state.eof());
    assert_eq!(state.total(), 5);
    assert_eq!(state.len(), 1027);

    Ok(())
}

#[tokio::test]
async fn graphql_1024() -> Result<()> {
    let body = Limited::random_with(File::open("tests/fixtures/graphql.txt").await?, 1024);
    // let body = Limited::new(File::open("tests/fixtures/graphql.txt").await?, 1033);
    let limit = body.limit();

    let mut form = FormData::new(body, "------------------------627436eaefdbc285");
    form.set_max_buf_size(limit)?;

    while let Some(mut field) = form.try_next().await? {
        assert!(!field.consumed());
        assert_eq!(field.length, 0);

        let mut buffer = BytesMut::new();
        while let Some(buf) = field.try_next().await? {
            buffer.extend_from_slice(&buf);
        }

        match field.index {
            0 => {
                assert_eq!(field.name, "operations");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(field.length, 236);
                assert_eq!(buffer, "[{ \"query\": \"mutation ($file: Upload!) { singleUpload(file: $file) { id } }\", \"variables\": { \"file\": null } }, { \"query\": \"mutation($files: [Upload!]!) { multipleUpload(files: $files) { id } }\", \"variables\": { \"files\": [null, null] } }]");
            }
            1 => {
                assert_eq!(field.name, "map");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(field.length, 89);
                assert_eq!(buffer, "{ \"0\": [\"0.variables.file\"], \"1\": [\"1.variables.files.0\"], \"2\": [\"1.variables.files.1\"] }");
            }
            2 => {
                assert_eq!(field.name, "0");
                assert_eq!(field.filename, Some("a.txt".into()));
                assert_eq!(field.content_type, Some(mime::TEXT_PLAIN));
                assert_eq!(field.length, 21);
                assert_eq!(buffer, "Alpha file content.\r\n");
            }
            3 => {
                assert_eq!(field.name, "1");
                assert_eq!(field.filename, Some("b.txt".into()));
                assert_eq!(field.content_type, Some(mime::TEXT_PLAIN));
                assert_eq!(field.length, 21);
                assert_eq!(buffer, "Bravo file content.\r\n");
            }
            4 => {
                assert_eq!(field.name, "2");
                assert_eq!(field.filename, Some("c.txt".into()));
                assert_eq!(field.content_type, Some(mime::TEXT_PLAIN));
                assert_eq!(field.length, 23);
                assert_eq!(buffer, "Charlie file content.\r\n");
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

    assert!(state.eof());
    assert_eq!(state.total(), 5);
    assert_eq!(state.len(), 1027);

    Ok(())
}

#[tokio::test]
async fn graphql_1033() -> Result<()> {
    let body = Limited::new(File::open("tests/fixtures/graphql.txt").await?, 1033);
    let limit = body.limit();

    let mut form = FormData::new(body, "------------------------627436eaefdbc285");
    form.set_max_buf_size(limit)?;

    while let Some(mut field) = form.try_next().await? {
        assert!(!field.consumed());
        assert_eq!(field.length, 0);

        let mut buffer = BytesMut::new();
        while let Some(buf) = field.try_next().await? {
            buffer.extend_from_slice(&buf);
        }

        match field.index {
            0 => {
                assert_eq!(field.name, "operations");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(field.length, 236);
                assert_eq!(buffer, "[{ \"query\": \"mutation ($file: Upload!) { singleUpload(file: $file) { id } }\", \"variables\": { \"file\": null } }, { \"query\": \"mutation($files: [Upload!]!) { multipleUpload(files: $files) { id } }\", \"variables\": { \"files\": [null, null] } }]");
            }
            1 => {
                assert_eq!(field.name, "map");
                assert_eq!(field.filename, None);
                assert_eq!(field.content_type, None);
                assert_eq!(field.length, 89);
                assert_eq!(buffer, "{ \"0\": [\"0.variables.file\"], \"1\": [\"1.variables.files.0\"], \"2\": [\"1.variables.files.1\"] }");
            }
            2 => {
                assert_eq!(field.name, "0");
                assert_eq!(field.filename, Some("a.txt".into()));
                assert_eq!(field.content_type, Some(mime::TEXT_PLAIN));
                assert_eq!(field.length, 21);
                assert_eq!(buffer, "Alpha file content.\r\n");
            }
            3 => {
                assert_eq!(field.name, "1");
                assert_eq!(field.filename, Some("b.txt".into()));
                assert_eq!(field.content_type, Some(mime::TEXT_PLAIN));
                assert_eq!(field.length, 21);
                assert_eq!(buffer, "Bravo file content.\r\n");
            }
            4 => {
                assert_eq!(field.name, "2");
                assert_eq!(field.filename, Some("c.txt".into()));
                assert_eq!(field.content_type, Some(mime::TEXT_PLAIN));
                assert_eq!(field.length, 23);
                assert_eq!(buffer, "Charlie file content.\r\n");
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

    assert!(state.eof());
    assert_eq!(state.total(), 5);
    assert_eq!(state.len(), 1027);

    Ok(())
}
