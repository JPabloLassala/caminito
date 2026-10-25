//! caminito-notifications, the notification daemon (ADR 0051, CMN-258). A Session starts it
//! before its command, on Caminito's socket with the variables the command has (08), so that
//! it owns `org.freedesktop.Notifications` before anything the command starts could have
//! another daemon D-Bus-activated. It's supervised as a helper, started again if it exits,
//! and stopped as the Session ends. A run that isn't a Session starts nothing.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;

use smithay::reexports::calloop::LoopHandle;

use crate::helpers::{Helper, HelperCommand, HelperEvent, RestartPolicy, StderrTarget};
use crate::state::State;

/// The daemon, and its helper once a Session started it.
pub(crate) struct Notifications {
    /// What runs: `CAMINITO_NOTIFICATIONS`, or `caminito-notifications` on the path, as the
    /// package installs it. The tests run none but the one a test names, so that a daemon
    /// installed on the machine never takes the tests' bus's name.
    pub(crate) program: Option<PathBuf>,
    helper: Option<Helper<State>>,
}

impl Notifications {
    pub(crate) fn new() -> Self {
        let program = std::env::var_os("CAMINITO_NOTIFICATIONS")
            .unwrap_or("caminito-notifications".into())
            .into();
        Notifications {
            program: (!cfg!(test)).then_some(program),
            helper: None,
        }
    }

    /// Spawns the daemon with `environment`, which has the socket, before this returns, and
    /// supervises it. Nothing waits for it to own the name: one that D-Bus activation
    /// started meanwhile gives it back (CMN-258).
    pub(crate) fn start(
        &mut self,
        handle: LoopHandle<'static, State>,
        environment: &BTreeMap<OsString, OsString>,
        stderr: StderrTarget,
    ) {
        let Some(program) = &self.program else {
            return;
        };
        let mut command = HelperCommand::new("caminito-notifications", program);
        for (name, value) in environment {
            command = command.env(name, value);
        }
        let mut helper = Helper::new(
            handle,
            command,
            RestartPolicy::default(),
            stderr,
            |state: &mut State| {
                state
                    .caminito
                    .notifications
                    .helper
                    .as_mut()
                    .expect("the daemon's helper is made before it's started")
            },
            daemon_event,
        );
        helper.start_at_once();
        self.helper = Some(helper);
    }

    /// Stops the daemon for good, as the Session ends (08).
    pub(crate) fn stop(&mut self) {
        if let Some(helper) = &mut self.helper {
            helper.stop();
        }
    }
}

fn daemon_event(_: &mut State, event: HelperEvent<'_>) {
    if let HelperEvent::GaveUp = event {
        tracing::warn!(
            "caminito-notifications exited too often, so another daemon may take the notifications"
        );
    }
}
