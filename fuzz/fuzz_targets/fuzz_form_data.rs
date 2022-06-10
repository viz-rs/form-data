#![no_main]
use libfuzzer_sys::fuzz_target;

use std::convert::Infallible;
use std::time::Duration;

use bytes::Bytes;
use form_data::FormData;
use futures_util::stream::{once, TryStreamExt};
use tokio::{runtime, time::timeout};

const FIELD_TIMEOUT: Duration = Duration::from_millis(10);

fuzz_target!(|data: &[u8]| {
    let data = data.to_vec();
    let stream = once(async move { Result::<Bytes, Infallible>::Ok(Bytes::from(data)) });

    let body = hyper::Body::wrap_stream(stream);
    let rt = runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime");

    let form_data = FormData::new(body, "BOUNDARY");

    async fn run(mut form_data: FormData<hyper::Body>) -> Result<(), Infallible> {
        while let Ok(Some(mut field)) = form_data.try_next().await {
            let _ = timeout(FIELD_TIMEOUT, field.ignore()).await;
        }
        Ok(())
    }

    rt.block_on(async move { run(form_data).await.unwrap() })
});
