//! `tracing` set-up (14). Logs go to stderr until the session's journald logging (M5).

use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;

/// Logs to stderr, filtered by `RUST_LOG` and at `info` by default.
pub fn init() {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .init();
}
