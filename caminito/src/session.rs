//! The Session (08, ADR 0004): `caminito --session`, which runs mate-session, or the command
//! it's given, as Caminito's child, and ends with it.
//!
//! Start-up is in 08's order. The backend, the socket, Xwayland, the palette helper and the
//! bus names (62) are `run`'s, before [`Caminito::start_session`]. From there the refresh
//! pass waits for Xwayland to be ready, or to be given up on. Then the environment goes
//! into `Caminito::environment`, and from there to the D-Bus activation environment and to
//! systemd's user manager (`dbus::systemd`), which then restarts the portal (232), and only
//! once the bus has answered is the command spawned, with `GDK_BACKEND=x11` for itself alone.
//!
//! The Session ends when the command exits: Caminito stops its helpers and Xwayland, closes
//! its socket, has systemd forget the variables, and stops the loop, for `run` to exit with
//! the command's status. SIGTERM or SIGINT, and the winit window closing, are passed to the
//! command as SIGTERM first ([`Caminito::quit`]).
//!
//! mate-session restarts the user's bus as it exits, if the session is the user's last
//! (`maybe_restart_user_bus` in its `gsm-manager.c`), so Caminito's connection may be to a bus
//! that has stopped, and zbus's executor for it stops with its last task (74). systemd is
//! therefore asked over a connection made for the request, on a thread of the request's
//! own, and asked again until it answers: the bus may stop after it took the request, and
//! the one that takes its place may not be there yet.
//!
//! Nothing here waits in the loop: the bus calls run on zbus's executor, or on a thread,
//! and report through a `calloop::channel`, and the command's exit comes through a
//! `SIGCHLD` source, as the helpers' do. The one wait is for a command Caminito has just
//! killed.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::{self, Read};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::UnixStream;
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::Duration;

use rustix::process::{Pid, Signal, kill_process};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::{Interest, Mode, PostAction, RegistrationToken, channel};

use crate::dbus::systemd::{self, Exported};
use crate::settings::translate::Mouse;
use crate::state::{Caminito, State};

/// How long a command that was sent SIGTERM has to exit (08), and how long systemd has to
/// forget the variables before Caminito exits anyway.
const WAIT: Duration = Duration::from_secs(5);

/// How many requests to forget the variables systemd can leave unanswered within the wait
/// (74). One it hasn't answered in its share of the wait is followed by another.
const REQUESTS_IN_WAIT: u32 = 5;

/// How long after a request to systemd fails the next is made (74): a bus that's being
/// replaced is there again this soon, and one that isn't is asked this often until `WAIT`.
const PAUSE_AFTER_FAILURE: Duration = Duration::from_millis(50);

/// The status Caminito exits with when the Session ends without one from its command.
const FAILURE: u8 = 1;

/// The variables systemd's user manager is asked to forget as the Session ends, so they
/// can't leak into the next login (08). `XDG_SESSION_TYPE` and `XDG_CURRENT_DESKTOP` are
/// the login's as much as Caminito's, and stay.
const UNSET: [&str; 6] = [
    "WAYLAND_DISPLAY",
    "DISPLAY",
    "MATE_WM_NAME",
    "MATE_WM_KEYBINDINGS",
    "XCURSOR_THEME",
    "XCURSOR_SIZE",
];

/// Where `caminito-git` puts what a Session's programs are to find before the stock
/// packages' own, as the XDG base directories have it (84): the desktop entry that takes
/// the Display capplet's place, with the capplet's id, so that the control center and the
/// menus show caminito-displays where they showed the capplet.
const DATA_DIR: &str = "/usr/share/caminito/xdg";

/// What the specification has for an `XDG_DATA_DIRS` that isn't set.
const DEFAULT_DATA_DIRS: &str = "/usr/local/share:/usr/share";

/// The session command's `XDG_DATA_DIRS`: Caminito's directory, then what Caminito `has`.
///
/// It's the command's alone, and what the command starts inherits it. The bus and systemd
/// aren't given it: systemd's user manager outlives the Session, and an X11 MATE login
/// after it must find the capplet's own entry.
fn data_dirs(has: Option<&OsStr>) -> OsString {
    let has = has.filter(|dirs| !dirs.is_empty());
    let mut dirs = OsString::from(DATA_DIR);
    dirs.push(":");
    dirs.push(has.unwrap_or(OsStr::new(DEFAULT_DATA_DIRS)));
    dirs
}

/// The GTK module that exports a classic `GtkMenuBar`, as most of MATE's apps have theirs,
/// for the global menu (111, ADR 0017): the name GTK 3 finds it by in its modules
/// directory.
const MENU_MODULE: &str = "appmenu-gtk-module";

/// Where pacman keeps a directory for each installed package, named by the package and its
/// version.
const PACKAGES: &str = "/var/lib/pacman/local";

/// What the directory of the patched module's package starts with. The stock package has
/// the same file, and isn't enough: it tells Caminito nothing of a menubar in a window
/// that isn't a `GtkApplicationWindow`, which then has no menu at all while an applet is
/// there. The stock one is enough once it has the fix.
const MENU_MODULE_PACKAGE: &str = "appmenu-gtk-module-caminito-";

/// Whether the patched menu module is installed.
fn menu_module_is_installed() -> bool {
    let Ok(packages) = std::fs::read_dir(PACKAGES) else {
        return false;
    };
    let mut names = packages.flatten().map(|package| package.file_name());
    names.any(|name| name.as_bytes().starts_with(MENU_MODULE_PACKAGE.as_bytes()))
}

/// The session command's `GTK_MODULES`, if it isn't what Caminito `has`: the menu module
/// after those, if it's `installed` and not among them already.
///
/// It's given to the command alone, which hands it to what it starts: Caminito's own
/// helpers have no use for it.
fn gtk_modules(has: Option<&OsStr>, installed: bool) -> Option<OsString> {
    let has = has.filter(|modules| !modules.is_empty());
    let listed = has.is_some_and(|modules| {
        let mut modules = modules.as_bytes().split(|byte| *byte == b':');
        modules.any(|module| module == MENU_MODULE.as_bytes())
    });
    if !installed || listed {
        return None;
    }
    let mut modules = OsString::new();
    if let Some(has) = has {
        modules.push(has);
        modules.push(":");
    }
    modules.push(MENU_MODULE);
    Some(modules)
}

/// What `--session` runs as Caminito's child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionCommand {
    program: OsString,
    args: Vec<OsString>,
}

impl SessionCommand {
    /// mate-session, without its acceleration check: Caminito needs working GL already, and
    /// the check can only get it wrong, for the whole session (08).
    pub fn mate_session() -> Self {
        SessionCommand {
            program: "mate-session".into(),
            args: vec!["--disable-acceleration-check".into()],
        }
    }

    /// `--session=CMD`'s command: its words, split at whitespace, are the program and its
    /// arguments. There's no shell and no quoting. Returns `None` if it has no word.
    pub fn parse(command: &OsStr) -> Option<Self> {
        let mut words = command
            .as_bytes()
            .split(u8::is_ascii_whitespace)
            .filter(|word| !word.is_empty())
            .map(|word| OsStr::from_bytes(word).to_owned());
        Some(SessionCommand {
            program: words.next()?,
            args: words.collect(),
        })
    }

    /// The program to run, which `PATH` finds.
    pub fn program(&self) -> &OsStr {
        &self.program
    }

    /// The program's arguments.
    pub fn args(&self) -> impl Iterator<Item = &OsStr> {
        self.args.iter().map(OsString::as_os_str)
    }
}

/// The variables a Session exports (08): Caminito's socket, Xwayland's display if it has one,
/// and what tells MATE it runs on Caminito. `XDG_CURRENT_DESKTOP` is the display manager's to
/// set, from `DesktopNames`, so it's `MATE` only when `desktop_is_set` says nobody did.
fn environment(
    socket: &OsStr,
    display: Option<u32>,
    mouse: &Mouse,
    desktop_is_set: bool,
) -> BTreeMap<OsString, OsString> {
    let mut environment = BTreeMap::new();
    let mut set = |name: &str, value: OsString| environment.insert(OsString::from(name), value);
    set("WAYLAND_DISPLAY", socket.to_owned());
    if let Some(display) = display {
        set("DISPLAY", format!(":{display}").into());
    }
    set("XDG_SESSION_TYPE", "wayland".into());
    if !desktop_is_set {
        set("XDG_CURRENT_DESKTOP", "MATE".into());
    }
    set("MATE_WM_NAME", "Caminito".into());
    set("MATE_WM_KEYBINDINGS", "Caminito".into());
    // Empty is the default theme, which a client finds without the variable.
    if !mouse.cursor_theme.is_empty() {
        set("XCURSOR_THEME", mouse.cursor_theme.clone().into());
    }
    set("XCURSOR_SIZE", mouse.cursor_size.to_string().into());
    environment
}

/// The status Caminito exits with for its command's: the command's own, or, as a shell has
/// it, 128 and the signal that ended it.
fn exit_status(status: ExitStatus) -> u8 {
    let code = match (status.code(), status.signal()) {
        (Some(code), _) => code,
        (None, Some(signal)) => 128 + signal,
        (None, None) => FAILURE.into(),
    };
    u8::try_from(code).unwrap_or(FAILURE)
}

/// A Session, from `--session` to Caminito's exit.
pub(crate) struct Session {
    command: SessionCommand,
    /// Caminito's socket, the Session's `WAYLAND_DISPLAY`.
    socket: OsString,
    phase: Phase,
    /// What the bus tasks answer on.
    events: channel::Sender<Event>,
    /// The export, which holds Caminito's connection while it runs. It stops when dropped.
    bus_task: Option<zbus::Task<()>>,
    /// Whether systemd's user manager took the environment, and so is asked to forget it.
    systemd_has_environment: bool,
    /// The timer that asks systemd again to forget the environment (74).
    ask_again: Option<RegistrationToken>,
    /// Why systemd's last request to forget failed, if it did.
    unset_error: Option<zbus::Error>,
    /// The `SIGCHLD` source, from the command's spawn on.
    sigchld: Option<(signal_hook::SigId, RegistrationToken)>,
    /// `WAIT`, which a test makes shorter.
    wait: Duration,
}

enum Phase {
    /// Xwayland is neither ready nor given up on.
    WaitingForXwayland,
    /// The bus has the environment to take. A `quit` that comes now waits for its answer,
    /// which says whether systemd has variables to forget.
    Exporting { quit: bool },
    /// The command runs. `stop` is the timer that gives up on it, once it was sent SIGTERM.
    Running {
        child: Child,
        stop: Option<RegistrationToken>,
    },
    /// Caminito has torn down, and systemd has the variables to forget, until
    /// the timer `give_up` runs.
    Ending {
        status: u8,
        give_up: Option<RegistrationToken>,
    },
    /// The loop was told to stop, and Caminito exits with this status.
    Over(u8),
}

/// What a bus task answers.
enum Event {
    Exported(Box<Exported>),
    Unset(zbus::Result<()>),
}

impl Drop for Session {
    /// The command is never left running without Caminito.
    fn drop(&mut self) {
        if let Phase::Running { child, .. } = &mut self.phase {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some((id, _)) = self.sigchld.take() {
            signal_hook::low_level::unregister(id);
        }
    }
}

impl Caminito {
    /// Makes this run a Session on `command`, with `socket` as its `WAYLAND_DISPLAY`. The
    /// command is spawned by the refresh pass, once Xwayland is ready and the bus has the
    /// environment.
    pub(crate) fn start_session(&mut self, command: SessionCommand, socket: OsString) {
        let (events, answers) = channel::channel();
        self.loop_handle
            .insert_source(answers, |event, _, state: &mut State| match event {
                channel::Event::Msg(event) => state.caminito.session_event(event),
                channel::Event::Closed => {}
            })
            .expect("the Session's channel joins the event loop");
        self.session = Some(Session {
            command,
            socket,
            phase: Phase::WaitingForXwayland,
            events,
            bus_task: None,
            systemd_has_environment: false,
            ask_again: None,
            unset_error: None,
            sigchld: None,
            wait: WAIT,
        });
    }

    /// The status to exit with, once the Session is over.
    pub(crate) fn session_exit(&self) -> Option<u8> {
        match self.session.as_ref()?.phase {
            Phase::Over(status) => Some(status),
            _ => None,
        }
    }

    /// Ends Caminito, as SIGTERM, SIGINT and the winit window closing ask. A bare `caminito`
    /// stops its loop. A Session sends its command SIGTERM first, and ends when the command
    /// exits or after `WAIT` (08).
    pub(crate) fn quit(&mut self) {
        let Some(session) = &mut self.session else {
            self.loop_signal.stop();
            return;
        };
        match &mut session.phase {
            // There's no command to wait for, and none is spawned.
            Phase::WaitingForXwayland => self.end_session(FAILURE),
            Phase::Exporting { quit } => *quit = true,
            Phase::Running { child, stop } if stop.is_none() => {
                tracing::info!("stopping the session command");
                if let Err(err) = kill_process(Pid::from_child(child), Signal::TERM) {
                    tracing::warn!("can't send the session command SIGTERM: {err}");
                }
                let timer = Timer::from_duration(session.wait);
                let timer = self.loop_handle.insert_source(timer, |_, _, state| {
                    state.caminito.give_up_on_session_command();
                    TimeoutAction::Drop
                });
                match timer {
                    Ok(token) => *stop = Some(token),
                    Err(err) => {
                        tracing::warn!("can't wait for the session command: {}", err.error);
                        self.give_up_on_session_command();
                    }
                }
            }
            Phase::Running { .. } | Phase::Ending { .. } | Phase::Over(_) => {}
        }
    }

    /// The refresh pass's part of start-up (08): once Xwayland is ready, with its root
    /// properties published (38), or Caminito runs without X, the environment is exported.
    pub(crate) fn advance_session(&mut self) {
        let Some(session) = &self.session else {
            return;
        };
        if !matches!(session.phase, Phase::WaitingForXwayland) {
            return;
        }
        let display = self.xwayland_display();
        if display.is_none() {
            if !self.xwayland_restart.without_x() {
                return;
            }
            tracing::error!("Xwayland didn't start, so the session starts with no DISPLAY");
        }
        let desktop_is_set = std::env::var_os("XDG_CURRENT_DESKTOP").is_some();
        let environment = environment(
            &session.socket,
            display,
            &self.settings.mouse,
            desktop_is_set,
        );
        self.environment.extend(environment);
        self.export_session_environment();
    }

    /// Gives the bus and systemd `Caminito::environment`, and spawns the command when they've
    /// answered, or at once if Caminito isn't on the bus.
    fn export_session_environment(&mut self) {
        let Some(dbus) = &self.dbus else {
            tracing::warn!(
                "Caminito isn't on the session bus, so nothing there has the environment"
            );
            self.spawn_session_command();
            return;
        };
        let session = self.session.as_mut().expect("a Session is starting");
        let mut variables = Vec::new();
        for (name, value) in &self.environment {
            match (name.to_str(), value.to_str()) {
                (Some(name), Some(value)) => variables.push((name.to_owned(), value.to_owned())),
                _ => tracing::warn!("can't export {name:?}, which isn't UTF-8, to the bus"),
            }
        }
        let connection = dbus.connection().clone();
        let events = session.events.clone();
        let task = dbus.connection().executor().spawn(
            async move {
                let exported = systemd::export(&connection, &variables).await;
                let _ = events.send(Event::Exported(Box::new(exported)));
            },
            "session environment",
        );
        session.bus_task = Some(task);
        session.phase = Phase::Exporting { quit: false };
    }

    /// An answer from the bus.
    fn session_event(&mut self, event: Event) {
        let Some(session) = &mut self.session else {
            return;
        };
        match (event, &session.phase) {
            (Event::Exported(exported), &Phase::Exporting { quit }) => {
                if let Err(err) = exported.activation {
                    tracing::warn!("can't export the environment to D-Bus activation: {err}");
                }
                match exported.systemd {
                    Ok(true) => session.systemd_has_environment = true,
                    // A nested run's private bus (14).
                    Ok(false) => tracing::info!(
                        "{} isn't on the bus, so the environment isn't exported to systemd",
                        systemd::NAME
                    ),
                    Err(err) => tracing::warn!("can't export the environment to systemd: {err}"),
                }
                if let Err(err) = exported.portal {
                    tracing::warn!("can't restart the portal, which may have no display: {err}");
                }
                if quit {
                    self.end_session(FAILURE);
                } else {
                    self.spawn_session_command();
                }
            }
            (Event::Unset(Ok(())), &Phase::Ending { status, give_up }) => {
                if let Some(give_up) = give_up {
                    self.loop_handle.remove(give_up);
                }
                self.session_over(status);
            }
            // The bus has stopped, or the one in its place isn't there yet (74).
            (Event::Unset(Err(err)), Phase::Ending { .. }) => {
                tracing::debug!("can't unset the environment in systemd yet: {err}");
                session.unset_error = Some(err);
                self.ask_systemd_again_after(PAUSE_AFTER_FAILURE);
            }
            // systemd's answer after Caminito gave up on it, or its second to one request.
            _ => {}
        }
    }

    /// Spawns the command with `Caminito::environment`, and `GDK_BACKEND=x11` for itself
    /// alone: mate-session isn't Wayland-safe, and its carried patch unsets the variable
    /// for what it starts (08). Caminito's data directory is first in its `XDG_DATA_DIRS`,
    /// and so in those of what it starts (84), and the menu module is in its `GTK_MODULES`
    /// if it's installed (111).
    fn spawn_session_command(&mut self) {
        let sigchld = match self.watch_session_command() {
            Ok(sigchld) => sigchld,
            Err(err) => {
                tracing::error!("can't watch for the session command's exit: {err}");
                self.end_session(FAILURE);
                return;
            }
        };
        let session = self.session.as_mut().expect("a Session is starting");
        session.sigchld = Some(sigchld);
        let program = &session.command.program;
        let mut command = Command::new(program);
        command
            .args(&session.command.args)
            .envs(&self.environment)
            .env("GDK_BACKEND", "x11")
            .env(
                "XDG_DATA_DIRS",
                data_dirs(std::env::var_os("XDG_DATA_DIRS").as_deref()),
            )
            .stdin(Stdio::null());
        let has = std::env::var_os("GTK_MODULES");
        if let Some(modules) = gtk_modules(has.as_deref(), menu_module_is_installed()) {
            command.env("GTK_MODULES", modules);
        }
        // A nested run has its host's, which isn't the Session's.
        if !self.environment.contains_key(OsStr::new("DISPLAY")) {
            command.env_remove("DISPLAY");
        }
        match command.spawn() {
            Ok(child) => {
                tracing::info!(
                    "started the session command {} as process {}",
                    program.display(),
                    child.id()
                );
                session.phase = Phase::Running { child, stop: None };
            }
            Err(err) => {
                tracing::error!(
                    "can't start the session command {}: {err}",
                    program.display()
                );
                self.end_session(FAILURE);
            }
        }
    }

    /// Adds the `SIGCHLD` source that sees the command exit: signal-hook's pipe, as the
    /// helpers have it (`helpers.rs`).
    fn watch_session_command(&self) -> io::Result<(signal_hook::SigId, RegistrationToken)> {
        let (read, write) = UnixStream::pair()?;
        read.set_nonblocking(true)?;
        write.set_nonblocking(true)?;
        let id = signal_hook::low_level::pipe::register(signal_hook::consts::SIGCHLD, write)?;
        let source = Generic::new(read, Interest::READ, Mode::Level);
        let token = self
            .loop_handle
            .insert_source(source, |_, read, state| {
                let mut buffer = [0; 64];
                while matches!((&**read).read(&mut buffer), Ok(n) if n > 0) {}
                state.caminito.reap_session_command();
                Ok(PostAction::Continue)
            })
            .map_err(|err| io::Error::other(err.error))?;
        Ok((id, token))
    }

    /// Handles a `SIGCHLD`, which may be for another child.
    fn reap_session_command(&mut self) {
        let Some(session) = &mut self.session else {
            return;
        };
        let Phase::Running { child, .. } = &mut session.phase else {
            return;
        };
        match child.try_wait() {
            Ok(Some(status)) => {
                tracing::info!("the session command exited: {status}");
                self.end_session(exit_status(status));
            }
            Ok(None) => {}
            Err(err) => tracing::warn!("can't wait for the session command: {err}"),
        }
    }

    /// The command was sent SIGTERM and still runs after the wait. It's killed, so it
    /// doesn't outlive the Session, and the Session ends without its status.
    fn give_up_on_session_command(&mut self) {
        let Some(session) = &mut self.session else {
            return;
        };
        let Phase::Running { child, .. } = &mut session.phase else {
            return;
        };
        tracing::warn!(
            "the session command didn't exit within {:?} of SIGTERM, so it's killed",
            session.wait
        );
        let _ = child.kill();
        let _ = child.wait();
        self.end_session(FAILURE);
    }

    /// Tears Caminito down (08): its helpers and Xwayland are stopped, its socket is closed,
    /// and systemd forgets the variables, if it had them. Then the loop stops, and `run`
    /// gives the VT back as it drops the backend and exits with `status`. A systemd that
    /// hasn't done so within `WAIT`, as one that's shutting down may not, isn't waited for.
    fn end_session(&mut self, status: u8) {
        let session = self.session.as_mut().expect("a Session is ending");
        let ending = Phase::Ending {
            status,
            give_up: None,
        };
        let was = std::mem::replace(&mut session.phase, ending);
        if let Phase::Running {
            stop: Some(stop), ..
        } = was
        {
            self.loop_handle.remove(stop);
        }
        if let Some((id, token)) = session.sigchld.take() {
            signal_hook::low_level::unregister(id);
            self.loop_handle.remove(token);
        }
        let forget = session.systemd_has_environment;
        let wait = session.wait;

        self.palette.stop();
        self.locker.stop();
        self.end_xwayland();
        self.close_socket();

        if !forget || self.dbus.is_none() {
            self.session_over(status);
            return;
        }
        let timer = Timer::from_duration(wait);
        let timer = self.loop_handle.insert_source(timer, move |_, _, state| {
            state.caminito.give_up_on_systemd(status);
            TimeoutAction::Drop
        });
        match timer {
            Ok(token) => {
                let session = self.session.as_mut().expect("a Session is ending");
                session.phase = Phase::Ending {
                    status,
                    give_up: Some(token),
                };
                self.ask_systemd_to_forget();
            }
            Err(err) => {
                tracing::warn!("can't wait for systemd: {}", err.error);
                self.session_over(status);
            }
        }
    }

    /// Asks systemd to forget the variables, over a new connection to the bus that's there
    /// now. If it doesn't answer in its share of the wait, as on a bus that stopped once it
    /// had the request, it's asked again (74).
    ///
    /// Each request has a thread, since zbus needs something to run the connection being
    /// made. One that's never answered keeps its thread until Caminito exits, which is soon.
    fn ask_systemd_to_forget(&mut self) {
        let (Some(session), Some(dbus)) = (&mut self.session, &self.dbus) else {
            return;
        };
        let events = session.events.clone();
        let builder = dbus.builder();
        let thread = std::thread::Builder::new().name("session environment".into());
        let thread = thread.spawn(move || {
            let unset = builder.and_then(|bus| zbus::block_on(systemd::unset(bus, &UNSET)));
            // The Session may be over by now.
            let _ = events.send(Event::Unset(unset));
        });
        if let Err(err) = thread {
            tracing::warn!("can't start the thread that asks systemd: {err}");
        }
        let share = session.wait / REQUESTS_IN_WAIT;
        self.ask_systemd_again_after(share);
    }

    /// Has systemd asked again to forget the variables once `delay` has passed, and not
    /// when it was to be before.
    fn ask_systemd_again_after(&mut self, delay: Duration) {
        let session = self.session.as_mut().expect("a Session is ending");
        if let Some(ask_again) = session.ask_again.take() {
            self.loop_handle.remove(ask_again);
        }
        let timer = Timer::from_duration(delay);
        let timer = self.loop_handle.insert_source(timer, |_, _, state| {
            let caminito = &mut state.caminito;
            if let Some(session) = &mut caminito.session {
                // The timer is dropped as this returns.
                session.ask_again = None;
                caminito.ask_systemd_to_forget();
            }
            TimeoutAction::Drop
        });
        match timer {
            Ok(token) => session.ask_again = Some(token),
            // The timer that gives up on systemd still ends the Session.
            Err(err) => tracing::warn!("can't ask systemd again: {}", err.error),
        }
    }

    /// systemd didn't forget the variables within the wait, and the Session ends anyway.
    fn give_up_on_systemd(&mut self, status: u8) {
        let session = self.session.as_mut().expect("a Session is ending");
        let wait = session.wait;
        match session.unset_error.take() {
            Some(err) => {
                tracing::warn!("systemd didn't unset the environment within {wait:?}: {err}");
            }
            None => tracing::warn!("systemd didn't unset the environment within {wait:?}"),
        }
        self.session_over(status);
    }

    /// The Session has ended, and Caminito exits with `status`.
    fn session_over(&mut self, status: u8) {
        let session = self.session.as_mut().expect("a Session is ending");
        session.phase = Phase::Over(status);
        session.bus_task = None;
        if let Some(ask_again) = session.ask_again.take() {
            self.loop_handle.remove(ask_again);
        }
        self.loop_signal.stop();
    }

    #[cfg(test)]
    pub(crate) fn set_session_wait(&mut self, wait: Duration) {
        self.session.as_mut().expect("a Session runs").wait = wait;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mouse(theme: &str, size: u32) -> Mouse {
        Mouse {
            cursor_theme: theme.into(),
            cursor_size: size,
            double_click_time: Duration::from_millis(400),
            drag_threshold: 8,
        }
    }

    /// The environment as `NAME=value` lines, in the map's order.
    fn lines(environment: &BTreeMap<OsString, OsString>) -> Vec<String> {
        environment
            .iter()
            .map(|(name, value)| format!("{}={}", name.display(), value.display()))
            .collect()
    }

    #[test]
    fn the_environment_has_the_displays_the_desktop_and_the_cursor() {
        let environment = environment(
            "wayland-1".as_ref(),
            Some(2),
            &mouse("mate-black", 32),
            false,
        );
        assert_eq!(
            lines(&environment),
            [
                "DISPLAY=:2",
                "MATE_WM_KEYBINDINGS=Caminito",
                "MATE_WM_NAME=Caminito",
                "WAYLAND_DISPLAY=wayland-1",
                "XCURSOR_SIZE=32",
                "XCURSOR_THEME=mate-black",
                "XDG_CURRENT_DESKTOP=MATE",
                "XDG_SESSION_TYPE=wayland",
            ]
        );
    }

    #[test]
    fn the_display_managers_current_desktop_is_left_alone() {
        let environment = environment("wayland-1".as_ref(), Some(0), &mouse("", 24), true);
        assert!(!environment.contains_key(OsStr::new("XDG_CURRENT_DESKTOP")));
    }

    #[test]
    fn without_xwayland_there_is_no_display() {
        let environment = environment("wayland-1".as_ref(), None, &mouse("", 24), false);
        assert!(!environment.contains_key(OsStr::new("DISPLAY")));
        assert_eq!(environment[OsStr::new("WAYLAND_DISPLAY")], "wayland-1");
    }

    #[test]
    fn the_default_cursor_theme_isnt_named() {
        let environment = environment("wayland-1".as_ref(), Some(0), &mouse("", 24), false);
        assert!(!environment.contains_key(OsStr::new("XCURSOR_THEME")));
        assert_eq!(environment[OsStr::new("XCURSOR_SIZE")], "24");
    }

    #[test]
    fn caminitos_data_directory_is_first_among_the_commands() {
        let dirs = data_dirs(Some("/usr/share:/var/lib/flatpak/exports/share".as_ref()));
        assert_eq!(
            dirs,
            "/usr/share/caminito/xdg:/usr/share:/var/lib/flatpak/exports/share"
        );
    }

    #[test]
    fn without_data_directories_set_the_command_has_caminito_s_and_the_default_ones() {
        let expected = "/usr/share/caminito/xdg:/usr/local/share:/usr/share";
        assert_eq!(data_dirs(None), expected);
        assert_eq!(data_dirs(Some("".as_ref())), expected);
    }

    #[test]
    fn the_menu_module_is_added_to_the_commands_gtk_modules_when_its_installed() {
        assert_eq!(gtk_modules(None, true).unwrap(), "appmenu-gtk-module");
        assert_eq!(
            gtk_modules(Some("".as_ref()), true).unwrap(),
            "appmenu-gtk-module"
        );
        assert_eq!(
            gtk_modules(Some("canberra-gtk-module".as_ref()), true).unwrap(),
            "canberra-gtk-module:appmenu-gtk-module"
        );
    }

    #[test]
    fn the_commands_gtk_modules_are_left_alone_without_the_menu_module() {
        assert_eq!(gtk_modules(None, false), None);
        assert_eq!(
            gtk_modules(Some("canberra-gtk-module".as_ref()), false),
            None
        );
    }

    #[test]
    fn a_menu_module_thats_listed_already_isnt_listed_twice() {
        let has = "appmenu-gtk-module:canberra-gtk-module";
        assert_eq!(gtk_modules(Some(has.as_ref()), true), None);
        // Another module whose name only starts as the menu module's does.
        assert_eq!(
            gtk_modules(Some("appmenu-gtk-module-wayland".as_ref()), true).unwrap(),
            "appmenu-gtk-module-wayland:appmenu-gtk-module"
        );
    }

    #[test]
    fn caminito_exits_with_its_commands_status_or_its_signal() {
        assert_eq!(exit_status(ExitStatus::from_raw(3 << 8)), 3);
        assert_eq!(exit_status(ExitStatus::from_raw(0)), 0);
        // Ended by SIGTERM, 15.
        assert_eq!(exit_status(ExitStatus::from_raw(15)), 143);
    }
}
