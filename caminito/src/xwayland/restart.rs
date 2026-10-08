//! Xwayland's restart (08, 11, 42): once it has been ready, Caminito keeps it on that display.
//! When it goes, its windows are forgotten, since X clients don't survive it, and it's
//! spawned again on the same display, so `DISPLAY` stays valid, with the helpers' backoff.
//! After too many exits Caminito logs once and runs without X. It's one code path in every
//! mode.
//!
//! Between an exit and the restart Caminito holds the display's lock file itself (49), so no
//! other X server takes the number in the delay. Smithay's `XWayland::spawn` always grabs
//! the lock afresh and keeps its `X11Lock` private, so Caminito takes its own at the end of
//! the dispatch that saw the exit, and lets it go just before the spawn. Another server
//! could still take the display in those two gaps, but not in the restart's delay. The
//! listening sockets aren't held, since the spawn binds its own.
//!
//! Known limitation (11): in a MATE on Caminito session, an Xwayland crash logs the user out.
//! mate-session is an X11 GTK client, GTK exits when it loses its display, and mate-session
//! exiting ends the session. The restart doesn't hold up that teardown.

use std::ffi::OsStr;
use std::fmt::Display;
use std::io::{self, Write};
use std::time::{Duration, Instant};

use rustix::process::{Pid, Signal, kill_process};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::wayland::selection::SelectionTarget;
use smithay::xwayland::xwm::XwmId;

use crate::helpers::{RestartPolicy, Restarts};
use crate::state::{Caminito, State};
use crate::window::WindowKey;

/// What Caminito keeps of Xwayland from one spawn to the next, and when it spawns it again.
#[derive(Debug, Default)]
pub(crate) struct Restart {
    /// The display Xwayland was first ready on, which every restart takes again. Until
    /// then, a failure to start means Caminito runs without X.
    display: Option<u32>,
    exits: Restarts,
    /// The display's lock, while Xwayland is down for a restart.
    display_lock: Option<DisplayLock>,
    /// The helpers' backoff and give-up rule (08), which a test makes quicker.
    pub(crate) policy: RestartPolicy,
    /// Whether Caminito runs without X: Xwayland is down, and isn't started again.
    without_x: bool,
}

/// What follows an exit of Xwayland.
enum AfterExit {
    /// It has never been ready, so it isn't tried again.
    WithoutX,
    /// It has exited too often.
    GiveUp,
    /// It's spawned again on its display after the delay.
    Respawn { display: u32, delay: Duration },
}

impl Restart {
    /// The display a spawn must take, if Xwayland has been ready before.
    pub(super) fn display(&self) -> Option<u32> {
        self.display
    }

    /// Takes the display's lock until the restart, unless Caminito holds it already.
    fn hold_display_lock(&mut self, number: u32) {
        if self.display_lock.is_some() {
            return;
        }
        match DisplayLock::grab(number) {
            Ok(lock) => self.display_lock = Some(lock),
            Err(err) => {
                tracing::warn!("can't hold :{number}'s lock until Xwayland restarts: {err}");
            }
        }
    }

    /// Whether Caminito runs without X, which a Session's start-up doesn't wait for (61).
    pub(crate) fn without_x(&self) -> bool {
        self.without_x
    }

    /// Lets the display's lock go, for a spawn to take.
    pub(super) fn release_display_lock(&mut self) {
        self.display_lock = None;
    }

    /// Xwayland is ready on `display`.
    pub(super) fn ready(&mut self, display: u32) {
        self.display.get_or_insert(display);
    }

    /// Records an exit at `now`.
    fn exited(&mut self, now: Instant) -> AfterExit {
        let Some(display) = self.display else {
            return AfterExit::WithoutX;
        };
        match self.exits.exited(now, &self.policy) {
            Some(delay) => AfterExit::Respawn { display, delay },
            None => AfterExit::GiveUp,
        }
    }
}

/// `/tmp/.X<n>-lock`, taken as X servers take it, with Caminito's pid in X's format. Every
/// X server leaves a lock naming a live process alone. It's removed when dropped.
#[derive(Debug)]
struct DisplayLock {
    display: u32,
}

impl DisplayLock {
    fn path(display: u32) -> String {
        format!("/tmp/.X{display}-lock")
    }

    fn grab(display: u32) -> io::Result<Self> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(Self::path(display))?;
        // Built first, so a failed write removes the file again.
        let lock = DisplayLock { display };
        writeln!(file, "{:>10}", std::process::id())?;
        Ok(lock)
    }
}

impl Drop for DisplayLock {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_file(Self::path(self.display)) {
            tracing::warn!("can't remove :{}'s lock: {err}", self.display);
        }
    }
}

impl State {
    /// `X11Wm` lost its connection: Xwayland has gone. `X11Wm` doesn't call
    /// `destroyed_window` as it goes, so its XWayland windows are forgotten here. A late
    /// word from an `X11Wm` that's already gone is ignored.
    pub(super) fn xwayland_disconnected(&mut self, xwm: XwmId) {
        let caminito = &mut self.caminito;
        if caminito.xwm().map(|wm| wm.id()) != Some(xwm) {
            return;
        }
        // `X11Wm`, Caminito's X connection and the override-redirect windows go first, as
        // 11 orders it, then the display's lock and socket with the `XWayland` source.
        caminito.stop_xwayland();
        let xwayland_windows: Vec<WindowKey> = caminito
            .known_windows()
            .filter(|window| window.x11_surface().is_some())
            .map(WindowKey::of)
            .collect();
        for key in &xwayland_windows {
            caminito.window_destroyed(key);
        }
        caminito.app_menu_registrations.xwayland_gone();
        // X's selections, which the seat holds as Caminito's, went with it.
        for target in [SelectionTarget::Clipboard, SelectionTarget::Primary] {
            self.x_selection_cleared(target);
        }
        let caminito = &mut self.caminito;
        caminito.queue_redraw_all();
        caminito.xwayland_exited("Xwayland exited");
    }
}

impl Caminito {
    /// Xwayland exited, or couldn't start, for `why`, and is stopped if it isn't yet. It's
    /// spawned again after the backoff if it has been ready before, and otherwise, or once
    /// it has exited too often, Caminito runs without X.
    pub(super) fn xwayland_exited(&mut self, why: impl Display) {
        self.stop_xwayland();
        // Given up on, or ended with the Session: there's nothing to start again.
        if self.xwayland_restart.without_x {
            tracing::debug!("{why}");
            return;
        }
        match self.xwayland_restart.exited(Instant::now()) {
            AfterExit::WithoutX => {
                tracing::warn!("{why}, so Caminito runs without X");
                self.xwayland_restart.without_x = true;
            }
            AfterExit::GiveUp => {
                self.xwayland_restart.without_x = true;
                let policy = &self.xwayland_restart.policy;
                tracing::warn!(
                    "{why}, {} times within {:?}, so Caminito runs without X",
                    policy.give_up_after,
                    policy.window
                );
                self.environment.remove(OsStr::new("DISPLAY"));
            }
            AfterExit::Respawn {
                display: number,
                delay,
            } => {
                tracing::warn!("{why}, so it's started again on :{number} in {delay:?}");
                let timer =
                    self.loop_handle
                        .insert_source(Timer::from_duration(delay), |_, _, state| {
                            // A Session that ended in the delay has no use for it.
                            if !state.caminito.xwayland_restart.without_x {
                                state.caminito.start_xwayland();
                            }
                            TimeoutAction::Drop
                        });
                match timer {
                    // Taken once this dispatch is over, since an exit reported by the
                    // `XWayland` source's own callback leaves it, and Smithay's lock, alive
                    // until the callback returns.
                    Ok(_) => {
                        self.loop_handle.insert_idle(move |state| {
                            let caminito = &mut state.caminito;
                            if caminito.xwayland.is_none() {
                                caminito.xwayland_restart.hold_display_lock(number);
                            }
                        });
                    }
                    Err(err) => {
                        tracing::warn!(
                            "can't schedule Xwayland's restart, so Caminito runs without X: {}",
                            err.error
                        );
                        self.xwayland_restart.without_x = true;
                    }
                };
            }
        }
    }

    /// Ends Xwayland for good with SIGTERM, as a Session tears down (08). `X11Wm` stays
    /// until it sees X go, since it calls back for as long as X talks to it, and Caminito
    /// doesn't wait for that.
    pub(crate) fn end_xwayland(&mut self) {
        self.xwayland_restart.without_x = true;
        self.xwayland_restart.release_display_lock();
        let Some(xwayland) = &self.xwayland else {
            return;
        };
        let Some(pid) = process(xwayland.display) else {
            tracing::warn!("can't find Xwayland's process to send it SIGTERM");
            return;
        };
        if let Err(err) = kill_process(pid, Signal::TERM) {
            tracing::warn!("can't send Xwayland SIGTERM: {err}");
        }
    }

    /// Notices Xwayland dying before it's ready. Smithay's `XWayland` source takes the end
    /// of the display fd for "not ready yet" (`xserver.rs:330-345`), so it's Xwayland's
    /// Wayland connection going that tells.
    pub(crate) fn notice_xwayland_died_starting(&mut self) {
        let Some(xwayland) = &self.xwayland else {
            return;
        };
        let gone = self
            .display_handle
            .backend_handle()
            .get_client_data(xwayland.client.id())
            .is_err();
        if xwayland.wm.is_none() && gone {
            self.xwayland_exited("Xwayland exited while starting");
        }
    }
}

/// The Xwayland this process spawned for `:display`, found among its children in `/proc`:
/// Smithay spawns it and keeps its `Child` to itself (`xserver.rs:199-224`).
pub(crate) fn process(display: u32) -> Option<Pid> {
    let parent = std::process::id().to_string();
    let display = format!(":{display}");
    std::fs::read_dir("/proc").ok()?.find_map(|entry| {
        let dir = entry.ok()?.path();
        // `pid (comm) state ppid …`
        let stat = std::fs::read_to_string(dir.join("stat")).ok()?;
        let (comm, rest) = stat.split_once(") ")?;
        let mut fields = rest.split(' ');
        let (state, ppid) = (fields.next()?, fields.next()?);
        // One that has exited may not be reaped yet.
        if !comm.ends_with("(Xwayland") || ppid != parent || state == "Z" {
            return None;
        }
        let cmdline = std::fs::read(dir.join("cmdline")).ok()?;
        cmdline
            .split(|&b| b == 0)
            .any(|arg| arg == display.as_bytes())
            .then(|| Pid::from_raw(dir.file_name()?.to_str()?.parse().ok()?))?
    })
}
