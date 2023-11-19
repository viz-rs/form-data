#![allow(dead_code)]
#![allow(unused_imports)]

#[cfg(feature = "async")]
mod incoming_body;
#[cfg(feature = "async")]
pub use incoming_body::IncomingBody;

mod limited;
pub use limited::Limited;

pub fn tracing_init() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        // From env var: `RUST_LOG`
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .map_err(|e| anyhow::anyhow!(e))
}
