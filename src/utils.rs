use anyhow::{anyhow, Result};

use http::header::{HeaderMap, HeaderName, HeaderValue};
use httparse::{parse_headers, Status, EMPTY_HEADER};

pub(crate) const CR: u8 = b'\r';
pub(crate) const LF: u8 = b'\n';
pub(crate) const DASH: u8 = b'-';
pub(crate) const CRLF: &[u8; 2] = &[CR, LF]; // `\r\n`

pub(crate) fn read_until_ctlf(bytes: &[u8]) -> Option<usize> {
    twoway::find_bytes(bytes, CRLF)
}

pub(crate) fn parse_content_type(header: Option<&http::HeaderValue>) -> Option<mime::Mime> {
    header
        .and_then(|val| val.to_str().ok())
        .and_then(|val| val.parse::<mime::Mime>().ok())
}

pub(crate) fn parse_part_headers(bytes: &[u8]) -> Result<HeaderMap> {
    let mut headers = [EMPTY_HEADER; crate::MAX_HEADERS];
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

    if form_data != b"form-data" {
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
                j = i;
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
    if v[1].0 == b"name" && v[1].1.len() > 0 {
        return Ok((
            String::from_utf8_lossy(v[1].1).to_string(),
            if v.len() > 2 && v[2].0 == b"filename" {
                Some(String::from_utf8_lossy(v[2].1).to_string())
            } else {
                None
            },
        ));
    }

    Err(anyhow!("invalid content disposition"))
}
