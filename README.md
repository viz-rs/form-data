<h1 align="center">form-data</h1>

<div align="center">
  <p><strong>AsyncRead/AsyncWrite/Stream for `multipart/form-data` <sup>rfc7578</sup></strong></p>
</div>

<div align="center">
  <img src="https://img.shields.io/badge/-safety!-success?style=flat-square" alt="Safety!" />
  <!-- Docs.rs docs -->
  <a href="https://docs.rs/form-data">
    <img src="https://img.shields.io/badge/docs-latest-blue.svg?style=flat-square"
      alt="Docs.rs docs" /></a>
  <!-- Crates version -->
  <a href="https://crates.io/crates/form-data">
    <img src="https://img.shields.io/crates/v/form-data.svg?style=flat-square"
    alt="Crates.io version" /></a>
  <!-- Downloads -->
  <a href="https://crates.io/crates/form-data">
    <img src="https://img.shields.io/crates/d/form-data.svg?style=flat-square"
      alt="Download" /></a>
  <!-- Twitter -->
  <a href="https://twitter.com/_fundon">
    <img src="https://img.shields.io/badge/twitter-@__fundon-blue.svg?style=flat-square" alt="Twitter: @_fundon" /></a>
</div>

## Features

* Stream

* Preparse headers of part

## Example

```rust
let body = Body::wrap_stream(stream);
let mut form = FormData::new("----WebKitFormBoundaryWLHCs9qmcJJoyjKR", body);

while let Some(mut field) = form.try_next().await? {
    assert!(!field.consumed());
    assert_eq!(field.length, 0);

    match field.index {
        Some(0) => {
            assert_eq!(field.name, "_method");
            assert_eq!(field.filename, None);
            assert_eq!(field.content_type, None);
        }
        Some(1) => {
            assert_eq!(field.name, "profile[blog]");
            assert_eq!(field.filename, None);
            assert_eq!(field.content_type, None);
        }
        Some(2) => {
            assert_eq!(field.name, "profile[public_email]");
            assert_eq!(field.filename, None);
            assert_eq!(field.content_type, None);
        }
        Some(3) => {
            assert_eq!(field.name, "profile[interests]");
            assert_eq!(field.filename, None);
            assert_eq!(field.content_type, None);
        }
        Some(4) => {
            assert_eq!(field.name, "profile[bio]");
            assert_eq!(field.filename, None);
            assert_eq!(field.content_type, None);
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
        }
        _ => {}
    }

    let buffer = field.bytes().await?;

    match field.index {
        Some(0) => {
            assert_eq!(buffer, "put");
        }
        Some(1) => {
            assert_eq!(buffer, "");
        }
        Some(2) => {
            assert_eq!(buffer, "");
        }
        Some(3) => {
            assert_eq!(buffer, "");
        }
        Some(4) => {
            assert_eq!(buffer, "hello\r\n\r\n\"quote\"");
        }
        Some(5) => {
            assert_eq!(buffer, "");
        }
        Some(6) => {
            assert_eq!(buffer, "Save");
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
```

## License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
