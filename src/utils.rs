use anyhow::{anyhow, Result};

use http::header::{HeaderMap, HeaderName, HeaderValue};
use httparse::{parse_headers, Status, EMPTY_HEADER};

pub(crate) const MAX_HEADERS: usize = 8 * 2;
pub(crate) const DASHES: [u8; 2] = [b'-', b'-']; // `--`
pub(crate) const CRLF: [u8; 2] = [b'\r', b'\n']; // `\r\n`
pub(crate) const CRLFS: [u8; 4] = [b'\r', b'\n', b'\r', b'\n']; // `\r\n\r\n`

const NAME: &[u8; 4] = b"name";
const FILE_NAME: &[u8; 8] = b"filename";
const FORM_DATA: &[u8; 9] = b"form-data";

pub(crate) fn parse_content_type(header: Option<&http::HeaderValue>) -> Option<mime::Mime> {
    header
        .and_then(|val| val.to_str().ok())
        .and_then(|val| val.parse::<mime::Mime>().ok())
}

pub(crate) fn parse_part_headers(bytes: &[u8]) -> Result<HeaderMap> {
    let mut headers = [EMPTY_HEADER; MAX_HEADERS];
    match parse_headers(&bytes, &mut headers) {
        Ok(Status::Complete((_, hs))) => {
            let len = hs.len();
            let mut header_map = HeaderMap::with_capacity(len);
            for h in hs.iter().take(len) {
                header_map.append(
                    HeaderName::from_bytes(h.name.as_bytes())?,
                    HeaderValue::from_bytes(&h.value)?,
                );
            }
            Ok(header_map)
        }
        Ok(Status::Partial) => Err(anyhow!("invaild headers")),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn parse_content_disposition(hv: &[u8]) -> Result<(String, Option<String>)> {
    if hv.len() < 20 {
        return Err(anyhow!("invalid content disposition"));
    }

    let mut i = 9;
    let form_data = &hv[0..i];

    if form_data != FORM_DATA {
        return Err(anyhow!("invalid content disposition"));
    }

    let mut j = i;
    let mut p = 0;
    let mut v: Vec<(&[u8], &[u8])> = Vec::new();

    v.push((form_data, &[]));

    loop {
        if i == hv.len() {
            if p == 1 {
                if let Some(mut e) = v.last_mut() {
                    e.1 = &hv[if hv[j] == b'"' && hv[i - 1] == b'"' {
                        j + 1..i - 1
                    } else {
                        j..i
                    }];
                }
            }
            break;
        }

        let b = hv[i];

        match b {
            b';' => {
                if p == 1 {
                    if let Some(mut e) = v.last_mut() {
                        e.1 = &hv[if hv[j] == b'"' && hv[i - 1] == b'"' {
                            j + 1..i - 1
                        } else {
                            j..i
                        }];
                    }
                    p = 0;
                }
                i += 1;
                j = i;
            }
            b' ' => {
                i += 1;
                if p == 0 {
                    j = i;
                }
            }
            b'=' => {
                v.push((&hv[j..i], &[]));
                i += 1;
                j = i;
                p = 1;
            }
            // b'\r' => {
            //     if p == 1 {
            //         if let Some(mut e) = v.last_mut() {
            //             e.1 = &hv[j..i];
            //         }
            //         p = 0;
            //     }
            //     i += 1;
            // }
            // b'\n' => {
            //     if i - j == 1 {
            //         break;
            //     }
            // }
            _ => {
                i += 1;
            }
        }
    }

    // name
    if v[1].0 == NAME && v[1].1.len() > 0 {
        return Ok((
            String::from_utf8_lossy(v[1].1).to_string(),
            if v.len() > 2 && v[2].0 == FILE_NAME {
                Some(String::from_utf8_lossy(v[2].1).to_string())
            } else {
                None
            },
        ));
    }

    Err(anyhow!("invalid content disposition"))
}
