use std::{
    env,
    fs::File,
    io::{copy, Cursor, Read, Write},
    sync::Arc,
    thread::spawn,
};

use anyhow::Result;
use form_data::{Field, FormData};
use tempfile::tempdir;
use tiny_http::{Header, Response, Server};

fn hello(size: usize, boundary: &str, reader: &mut dyn Read) -> Result<Response<Cursor<Vec<u8>>>> {
    let dir = tempdir()?;
    let mut txt = String::new();

    txt.push_str(&dir.path().to_string_lossy());
    txt.push_str("\r\n");

    let mut form = FormData::new(reader, boundary);

    // 512KB for hyper lager buffer
    form.set_max_buf_size(size)?;

    while let Some(item) = form.next() {
        let mut field = item?;
        let name = field.name.to_owned();
        let mut bytes: u64 = 0;

        assert_eq!(bytes as usize, field.length);

        if let Some(filename) = &field.filename {
            let filepath = dir.path().join(filename);

            match filepath.extension().and_then(|s| s.to_str()) {
                Some("txt") => {
                    // buffer <= 8KB
                    let mut writer = File::create(&filepath)?;
                    bytes = copy(&mut field, &mut writer)?;
                    writer.flush()?;
                }
                Some("iso") => {
                    field.ignore()?;
                }
                _ => {
                    // 8KB <= buffer <= 512KB
                    // let mut writer = File::create(&filepath).await?;
                    // bytes = field.copy_to(&mut writer).await?;

                    let mut writer = File::create(&filepath)?;
                    bytes = field.copy_to_file(&mut writer)?;
                }
            }

            tracing::info!("file {} {}", name, bytes);
            txt.push_str(&format!("file {} {}\r\n", name, bytes));
        } else {
            let buffer = Field::bytes(&mut field)?;
            bytes = buffer.len() as u64;
            tracing::info!("text {} {}", name, bytes);
            txt.push_str(&format!("text {} {}\r\n", name, bytes));
        }

        tracing::info!("{:?}", field);

        assert_eq!(
            bytes,
            match name.as_str() {
                "empty" => 0,
                "tiny1" => 7,
                "tiny0" => 122,
                "small1" => 315,
                "small0" => 1_778,
                "medium" => 13_196,
                "large" => 2_413_677,
                "book" => 400_797_393,
                "crate" => 9,
                _ => bytes,
            }
        );
    }

    dir.close()?;

    Ok(Response::from_string(txt))
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        // From env var: `RUST_LOG`
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .map_err(|e| anyhow::anyhow!(e))?;

    let mut arg = env::args()
        .find(|a| a.starts_with("--size="))
        .unwrap_or_else(|| "--size=8".to_string());

    // 512
    // 8 * 2
    // 8
    let size = arg.split_off(7).parse::<usize>().unwrap_or(8) * 1024;
    let server = Arc::new(Server::http("0.0.0.0:3000").unwrap());
    println!("Now listening on port 3000");

    for mut request in server.incoming_requests() {
        spawn(move || {
            let m = request
                .headers()
                .iter()
                .find(|h: &&Header| h.field.equiv("Content-Type"))
                .map(|h| h.value.clone())
                .and_then(|val| val.as_str().parse::<mime::Mime>().ok())
                .unwrap();
            let boundary = m.get_param(mime::BOUNDARY).unwrap().as_str();
            let reader = request.as_reader();
            let response = hello(size, boundary, reader).unwrap();
            let _ = request.respond(response);
        });
    }

    Ok(())
}
