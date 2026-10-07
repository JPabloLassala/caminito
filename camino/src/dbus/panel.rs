//! Camino's calls to `org.mate.Panel` (06, 95): the main menu for Alt+F1 and the Run dialog
//! for Alt+F2, which Marco asked the panel for with `_MATE_PANEL_ACTION`. mate-panel's
//! carried patch exports the name under Camino (ADR 0011).
//!
//! The panel never saw the key, so it has no serial for its menu's grab. Camino accepts one
//! popup grab without a valid serial from the panel's client, for about a second after the
//! call. The panel's client is the one of the process that owns the name: the bus knows
//! the owner's pid, and Camino each Wayland client's. If several clients are of that
//! process, any of them may take the grab.
//!
//! An unpatched panel has no such name. Then Alt+F2 spawns `mate-panel --run-dialog`, and
//! Alt+F1, which has nothing of the kind, logs once.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use futures_channel::oneshot;
use smithay::reexports::calloop::LoopHandle;
use smithay::reexports::calloop::channel;
use smithay::reexports::wayland_server::Client;

use crate::state::{Camino, State};

/// The well-known name, which mate-panel's carried patch owns under Camino.
const NAME: &str = "org.mate.Panel";
/// The object the interface is at.
const PATH: &str = "/org/mate/Panel";
const INTERFACE: &str = "org.mate.Panel";

/// How long after a call the panel's client may take its grab (06).
const GRAB_TIME: Duration = Duration::from_secs(1);

/// What Camino asks the panel for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelAction {
    /// Its main menu: dropped from the Menu Bar, else anchored to the Menu Button, else
    /// opened at the pointer.
    MainMenu,
    /// Its Run dialog.
    RunDialog,
}

impl PanelAction {
    /// The method of `org.mate.Panel` that does it. Both take the key event's time, `u`.
    fn method(self) -> &'static str {
        match self {
            PanelAction::MainMenu => "MainMenu",
            PanelAction::RunDialog => "RunDialog",
        }
    }
}

/// What a call's task tells the event loop.
enum Event {
    /// The name's owner is the process `pid`, and is called for its menu once `go` is
    /// answered: the event loop knows whose grab to accept before the panel can ask for
    /// it.
    Calling { pid: u32, go: oneshot::Sender<()> },
    /// The name has no owner, or the call failed, with the cause.
    Failed(PanelAction, String),
}

/// What Camino keeps of its calls to the panel.
pub(crate) struct Panel {
    events: channel::Sender<Event>,
    /// The process whose client may take one popup grab without a valid serial, and until
    /// when, on Camino's clock.
    grab: Option<(u32, Duration)>,
    /// Whether Alt+F1 has logged that no panel opens its menu.
    main_menu_logged: bool,
    /// The program run with `--run-dialog` when the panel can't be asked.
    program: PathBuf,
    /// The Run dialogs spawned, until they're seen to have exited.
    run_dialogs: Vec<Child>,
}

impl Panel {
    pub(crate) fn new(loop_handle: &LoopHandle<'static, State>) -> Self {
        let (events, answers) = channel::channel();
        loop_handle
            .insert_source(answers, |event, _, state: &mut State| match event {
                channel::Event::Msg(event) => state.camino.panel_event(event),
                channel::Event::Closed => {}
            })
            .expect("the panel's channel joins the event loop");
        Panel {
            events,
            grab: None,
            main_menu_logged: false,
            program: "mate-panel".into(),
            run_dialogs: Vec::new(),
        }
    }
}

impl Camino {
    /// Asks the panel for `action`, for the key event at `time`, in milliseconds. The call
    /// is made on zbus's thread, and the main thread never waits on it (15).
    pub(crate) fn ask_panel(&mut self, action: PanelAction, time: u32) {
        let Some(dbus) = &self.dbus else {
            self.panel_failed(action, "Camino isn't on the session bus");
            return;
        };
        let connection = dbus.connection().clone();
        let events = self.panel.events.clone();
        let call = async move {
            let called = async {
                let bus = zbus::fdo::DBusProxy::new(&connection).await?;
                // The call goes to the owner the pid is of, even if the name changes hands.
                let owner = bus.get_name_owner(NAME.try_into()?).await?;
                // Only the menu takes a grab.
                if action == PanelAction::MainMenu {
                    let pid = bus
                        .get_connection_unix_process_id(owner.clone().into())
                        .await?;
                    let (go, gone) = oneshot::channel();
                    // Camino is stopping if either fails, and nothing is owed an answer.
                    if events.send(Event::Calling { pid, go }).is_err() || gone.await.is_err() {
                        return Ok(());
                    }
                }
                connection
                    .call_method(
                        Some(&owner),
                        PATH,
                        Some(INTERFACE),
                        action.method(),
                        &(time,),
                    )
                    .await?;
                zbus::Result::Ok(())
            };
            if let Err(err) = called.await {
                let _ = events.send(Event::Failed(action, err.to_string()));
            }
        };
        dbus.connection()
            .executor()
            .spawn(call, "panel call")
            .detach();
    }

    fn panel_event(&mut self, event: Event) {
        match event {
            Event::Calling { pid, go } => {
                self.panel.grab = Some((pid, self.now() + GRAB_TIME));
                // The task is only dropped with the connection.
                let _ = go.send(());
            }
            Event::Failed(action, err) => self.panel_failed(action, &err),
        }
    }

    /// What an unpatched panel leaves: Alt+F2 has `mate-panel --run-dialog`, and Alt+F1
    /// nothing (02).
    fn panel_failed(&mut self, action: PanelAction, err: &str) {
        match action {
            PanelAction::MainMenu => {
                // A panel that failed the call opens no menu.
                self.panel.grab = None;
                if !std::mem::replace(&mut self.panel.main_menu_logged, true) {
                    tracing::warn!("can't open the panel's main menu: {err}");
                }
            }
            PanelAction::RunDialog => {
                tracing::debug!("can't ask the panel for its run dialog, so it's spawned: {err}");
                self.spawn_run_dialog();
            }
        }
    }

    /// Spawns `mate-panel --run-dialog`, a process that lasts as long as its dialog.
    fn spawn_run_dialog(&mut self) {
        // No source watches for their exits: the next one spawned sees to the last.
        self.panel
            .run_dialogs
            .retain_mut(|child| matches!(child.try_wait(), Ok(None)));
        let spawned = Command::new(&self.panel.program)
            .arg("--run-dialog")
            .envs(&self.environment)
            .stdin(Stdio::null())
            .spawn();
        match spawned {
            Ok(child) => self.panel.run_dialogs.push(child),
            Err(err) => tracing::warn!(
                "can't spawn {} --run-dialog: {err}",
                self.panel.program.display()
            ),
        }
    }

    /// Takes the one popup grab without a valid serial that the panel's client has for a
    /// while after a call, if `client` is of the panel's process. Whether it was there to
    /// take.
    pub(crate) fn take_panel_grab(&mut self, client: &Client) -> bool {
        let Some((pid, until)) = self.panel.grab else {
            return false;
        };
        if self.now() >= until {
            self.panel.grab = None;
            return false;
        }
        if self.client_pid(client) != Some(pid) {
            return false;
        }
        self.panel.grab = None;
        true
    }

    /// The process `client` is of, from its socket's credentials.
    fn client_pid(&self, client: &Client) -> Option<u32> {
        crate::state::client_process(client, &self.display_handle)
    }

    /// Runs `program` in place of `mate-panel` for the Run dialog.
    #[cfg(test)]
    pub(crate) fn set_panel_program(&mut self, program: PathBuf) {
        self.panel.program = program;
    }
}
