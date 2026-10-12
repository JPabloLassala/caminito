//! caminito-wallpaperd, the drawer of the wallpaper (210, ADR 0031). A Session starts it with
//! its command, on Caminito's socket with the variables the command has (08), supervises it
//! as a helper so that it's started again if it exits, and stops it as the Session ends. A
//! run that isn't a Session starts nothing: the wallpaper there is whoever draws it.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;

use smithay::reexports::calloop::LoopHandle;

use crate::helpers::{Helper, HelperCommand, HelperEvent, RestartPolicy, StderrTarget};
use crate::state::State;

/// The drawer, and its helper once a Session started it.
pub(crate) struct Wallpaper {
    /// What runs: `CAMINITO_WALLPAPERD`, or `caminito-wallpaperd` on the path, as the
    /// package installs it. The tests run none but the one a test names, so that a drawer
    /// installed on the machine never runs against a test's Caminito.
    pub(crate) program: Option<PathBuf>,
    helper: Option<Helper<State>>,
}

impl Wallpaper {
    pub(crate) fn new() -> Self {
        let program = std::env::var_os("CAMINITO_WALLPAPERD")
            .unwrap_or("caminito-wallpaperd".into())
            .into();
        Wallpaper {
            program: (!cfg!(test)).then_some(program),
            helper: None,
        }
    }

    /// Starts the drawer with `environment`, which has the socket, and supervises it.
    pub(crate) fn start(
        &mut self,
        handle: LoopHandle<'static, State>,
        environment: &BTreeMap<OsString, OsString>,
        stderr: StderrTarget,
    ) {
        let Some(program) = &self.program else {
            return;
        };
        let mut command = HelperCommand::new("caminito-wallpaperd", program);
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
                    .wallpaper
                    .helper
                    .as_mut()
                    .expect("the drawer's helper is made before it's started")
            },
            drawer_event,
        );
        helper.start();
        self.helper = Some(helper);
    }

    /// Stops the drawer for good, as the Session ends (08).
    pub(crate) fn stop(&mut self) {
        if let Some(helper) = &mut self.helper {
            helper.stop();
        }
    }
}

fn drawer_event(_: &mut State, event: HelperEvent<'_>) {
    if let HelperEvent::GaveUp = event {
        tracing::warn!("caminito-wallpaperd exited too often, so the wallpaper isn't drawn");
    }
}
