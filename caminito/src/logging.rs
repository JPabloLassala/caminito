//! `tracing` set-up (14): to journald in a Session whose stderr isn't a terminal, as the
//! display manager starts one, and to stderr otherwise.

use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, Ordering};

use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::helpers::StderrTarget;

/// Whether `init` sent the logs to the journal.
static IN_JOURNAL: AtomicBool = AtomicBool::new(false);

/// Whether logs are to go to the journal: under `--session`, unless someone reads stderr on a
/// terminal, as on `caminitotest`'s tty and in a nested run.
fn wants_journal(session: bool) -> bool {
    session && !std::io::stderr().is_terminal()
}

/// Logs to the journal or to stderr, filtered by `RUST_LOG` and at `info` by default.
pub fn init(session: bool) {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    let journal = wants_journal(session).then(tracing_journald::layer);
    match journal {
        Some(Ok(journal)) => {
            tracing_subscriber::registry()
                .with(filter)
                .with(journal)
                .init();
            IN_JOURNAL.store(true, Ordering::Relaxed);
        }
        other => {
            tracing_subscriber::fmt()
                .with_writer(std::io::stderr)
                .with_env_filter(filter)
                .init();
            if let Some(Err(err)) = other {
                tracing::warn!("can't log to the journal, so logs go to stderr: {err}");
            }
        }
    }
}

/// Where the helpers' stderr goes: where `init` sent Caminito's logs.
pub(crate) fn helper_stderr() -> StderrTarget {
    if IN_JOURNAL.load(Ordering::Relaxed) {
        StderrTarget::Journal
    } else {
        StderrTarget::Prefixed
    }
}
