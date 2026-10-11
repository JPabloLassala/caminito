//! A window that doesn't close (168), as Marco's `core/delete.c` handles it, at `8522ed1`.
//! A request to close a window goes with a ping to its client: xdg_wm_base's, or
//! `_NET_WM_PING` for an XWayland window that takes it. A ping with no answer in five
//! seconds shows Marco's "not responding" dialog, with zenity as 167's are, which offers to
//! wait or to force the application to quit. Force Quit kills the client's process, and
//! cuts an X client off.
//!
//! The ping waits on Caminito's clock, so it's a holder of `deadlines`; the dialog is a
//! process, and has a source of its own.
//!
//! Not as in Marco:
//!
//! - An answer to the ping takes the dialog away, as the window's closing does. Marco's
//!   `delete_ping_reply_func` does nothing, and its dialog stays.
//! - Escape in the dialog, or closing it, is *Wait*. zenity's status for both is 1, as for
//!   *Force Quit*, which Marco takes them for, though its comment says that status 1
//!   "means the user pressed Force Quit".
//! - The dialog isn't transient for the window: Marco's `WINDOWID` names an X window to a
//!   zenity that's an X client, which zenity 4 on Caminito isn't.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::process::{Child, Command};
use std::time::Duration;

use caminito_policy::{PolicyWindow, WindowId};
use gio::glib;
use rustix::process::{Pid, Signal, kill_process};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{Interest, Mode, PostAction, RegistrationToken};
use smithay::utils::SERIAL_COUNTER;
use smithay::wayland::shell::xdg::ShellClient;

use crate::dialogs::TITLE;
use crate::state::{Caminito, SEAT};
use crate::window::Mapped;

/// How long a client has to answer a ping: Marco's `PING_TIMEOUT_DELAY`
/// (`core/display.c:4369`).
const PING_TIMEOUT: Duration = Duration::from_secs(5);

/// zenity's status for *Force Quit*, its cancel button.
const FORCE_QUIT: i32 = 1;

/// The status zenity is given for Escape and for its window closed, in `ZENITY_ESC`, to
/// tell them from *Force Quit*.
const ESCAPE: &str = "2";

/// The windows asked to close whose clients owe an answer to a ping, or have the dialog.
#[derive(Default)]
pub(crate) struct NotResponding {
    windows: BTreeMap<WindowId, Waiting>,
}

/// What a window waits for, which is one of the two at least.
#[derive(Default)]
struct Waiting {
    /// When the ping it was last sent is taken to have no answer.
    due: Option<Duration>,
    /// Its "not responding" dialog, while it shows.
    dialog: Option<Dialog>,
}

struct Dialog {
    zenity: Child,
    /// The source that sees zenity end.
    source: RegistrationToken,
}

impl NotResponding {
    /// When the earliest ping is taken to have no answer.
    pub(crate) fn deadline(&self) -> Option<Duration> {
        self.windows
            .values()
            .filter_map(|waiting| waiting.due)
            .min()
    }

    /// How many dialogs show.
    #[cfg(test)]
    pub(crate) fn dialogs(&self) -> usize {
        let dialogs = self.windows.values().filter_map(|w| w.dialog.as_ref());
        dialogs.count()
    }
}

/// Why a process isn't Force Quit's to kill.
enum Spared {
    Caminito,
    Xwayland,
}

impl Caminito {
    /// Pings the client of each window policy asked to close since the last refresh pass,
    /// as Marco's `meta_window_delete` does (`core/delete.c:126-159`). A window that takes
    /// no ping is taken to have answered.
    pub(crate) fn refresh_close_pings(&mut self) {
        let due = self.now() + PING_TIMEOUT;
        for mapped in self.policy.windows() {
            if mapped.take_close_asked() && mapped.ping(SERIAL_COUNTER.next_serial()) {
                let waiting = self.not_responding.windows.entry(mapped.id()).or_default();
                waiting.due = Some(due);
            }
        }
    }

    /// The pings with no answer at `now`: each shows its window's dialog, or brings the
    /// one that shows forward (`delete_ping_timeout_func`, `core/delete.c:75-123`).
    pub(crate) fn close_pings_elapse(&mut self, now: Duration) {
        let windows = &mut self.not_responding.windows;
        let unanswered: Vec<(WindowId, Option<u32>)> = windows
            .iter_mut()
            .filter(|(_, waiting)| waiting.due.is_some_and(|due| due <= now))
            .map(|(id, waiting)| {
                waiting.due = None;
                (
                    *id,
                    waiting.dialog.as_ref().map(|dialog| dialog.zenity.id()),
                )
            })
            .collect();
        for (id, dialog) in unanswered {
            match dialog {
                Some(zenity) => self.present_dialog(zenity, now),
                None => self.show_not_responding_dialog(id),
            }
        }
    }

    /// The window's client answered its ping, or the window has gone: nothing waits for
    /// it now, and its dialog is taken away, as Marco's is from a window that goes
    /// (`meta_window_free_delete_dialog`, `core/delete.c:202-209`).
    pub(crate) fn window_responds(&mut self, id: WindowId) {
        let Some(waiting) = self.not_responding.windows.remove(&id) else {
            return;
        };
        if let Some(mut dialog) = waiting.dialog {
            self.loop_handle.remove(dialog.source);
            // Killed, as Marco's is, so the wait is short.
            let ended = dialog.zenity.kill().and_then(|()| dialog.zenity.wait());
            if let Err(err) = ended {
                tracing::warn!("can't take a \"not responding\" dialog away: {err}");
            }
        }
    }

    /// An xdg client answered its ping, which is for every window of its.
    pub(crate) fn xdg_client_answered(&mut self, client: &ShellClient) {
        let waiting: Vec<WindowId> = self.not_responding.windows.keys().copied().collect();
        for id in waiting {
            let toplevel = self.policy.window(id).and_then(|m| m.window().toplevel());
            if toplevel.is_some_and(|toplevel| toplevel.client() == *client) {
                self.window_responds(id);
            }
        }
    }

    /// Shows Marco's dialog for the window, whose arguments are `delete_ping_timeout_func`'s
    /// through `meta_show_dialog` (`core/util.c:535-630`), but `--display` and `--class`,
    /// as 167's. The title is escaped for zenity's markup, which Marco's isn't.
    fn show_not_responding_dialog(&mut self, id: WindowId) {
        let Some(mapped) = self.policy.window(id) else {
            self.not_responding.windows.remove(&id);
            return;
        };
        let title = glib::markup_escape_text(&mapped.title());
        let text = format!(
            "<big><b><tt>{title}</tt> is not responding.</b></big>\n\n\
             <i>You may choose to wait a short while for it to continue or force the \
             application to quit entirely.</i>"
        );
        let mut zenity = Command::new("zenity");
        zenity
            .args(["--question", "--title", TITLE, "--text", &text])
            .args(["--ok-label", "_Wait", "--cancel-label", "_Force Quit"])
            .env("ZENITY_ESC", ESCAPE);
        let source = self.spawn_dialog(zenity).and_then(|(zenity, stdout)| {
            let source = self.watch_dialog(id, stdout)?;
            Ok(Dialog { zenity, source })
        });
        match source {
            Ok(dialog) => {
                let waiting = self.not_responding.windows.entry(id).or_default();
                waiting.dialog = Some(dialog);
            }
            Err(err) => {
                tracing::warn!(
                    "can't show Marco's dialog for a window that isn't responding: zenity: {err}"
                );
                self.not_responding.windows.remove(&id);
            }
        }
    }

    /// Has the loop see the dialog of the window `id` end: zenity closes `stdout` as it
    /// does, and a question prints nothing before.
    fn watch_dialog(&self, id: WindowId, stdout: File) -> std::io::Result<RegistrationToken> {
        let source = Generic::new(stdout, Interest::READ, Mode::Level);
        let inserted = self
            .loop_handle
            .insert_source(source, move |_, stdout, state| {
                let mut buffer = [0; 512];
                if let Ok(1..) = (&**stdout).read(&mut buffer) {
                    return Ok(PostAction::Continue);
                }
                state.caminito.not_responding_dialog_ended(id);
                Ok(PostAction::Remove)
            });
        inserted.map_err(|err| err.error.into())
    }

    /// The dialog of the window `id` ended by itself: with *Force Quit*, which kills, or
    /// with anything else, which waits (`dialog_exited`, `core/delete.c:61-73`).
    fn not_responding_dialog_ended(&mut self, id: WindowId) {
        let windows = &mut self.not_responding.windows;
        let Some(mut dialog) = windows.get_mut(&id).and_then(|w| w.dialog.take()) else {
            return;
        };
        // A close while the dialog showed may wait for its ping still.
        if windows
            .get(&id)
            .is_some_and(|waiting| waiting.due.is_none())
        {
            windows.remove(&id);
        }
        // zenity closes its stdout as it ends, so the wait is short.
        match dialog.zenity.wait() {
            Ok(status) if status.code() == Some(FORCE_QUIT) => self.force_quit(id),
            Ok(_) => tracing::debug!("a \"not responding\" dialog ended with a wait"),
            Err(err) => tracing::warn!("can't read a \"not responding\" dialog's answer: {err}"),
        }
    }

    /// Brings the dialog that shows forward, as Marco's
    /// `meta_window_present_delete_dialog` does (`core/delete.c:211-248`), which finds it
    /// by its class and its parent: here it's the window of zenity's process.
    fn present_dialog(&mut self, zenity: u32, now: Duration) {
        let is_dialog = |mapped: &&Mapped| mapped.pid() == Some(zenity);
        let dialog = self.policy.windows().find(is_dialog).map(Mapped::id);
        if let Some(id) = dialog {
            self.policy.activate(SEAT, id, now);
        }
    }

    /// Force Quit, as Marco's `meta_window_kill` (`core/delete.c:162-200`): kills the
    /// process of the window's client, and cuts an X client off whether it named a process
    /// or not. The process is the Wayland client's by its socket's credentials, or an X
    /// window's `_NET_WM_PID` if `WM_CLIENT_MACHINE` is this machine. Caminito's own process
    /// and Xwayland's are never killed.
    fn force_quit(&mut self, id: WindowId) {
        let Some(mapped) = self.policy.window(id) else {
            return;
        };
        let (title, pid) = (mapped.title(), mapped.pid());
        let x11 = mapped.window().x11_surface().cloned();
        let pid = match &x11 {
            Some(window) => pid.filter(|_| self.x_client_is_of_this_machine(window)),
            None => pid,
        };
        // A process id that isn't one, or that names a group to `kill`, is nobody's.
        let pid = pid.and_then(|pid| Pid::from_raw(i32::try_from(pid).ok()?));
        match pid.map(|pid| (pid, self.spared(pid))) {
            Some((pid, None)) => {
                tracing::info!("forcing {title:?} to quit: killing process {pid:?}");
                if let Err(err) = kill_process(pid, Signal::KILL) {
                    tracing::warn!("can't force {title:?} to quit: {err}");
                }
            }
            Some((_, Some(Spared::Caminito))) => {
                tracing::warn!("can't force {title:?} to quit: its process is Caminito's own");
            }
            Some((_, Some(Spared::Xwayland))) => {
                tracing::warn!("can't force {title:?} to quit: its process is Xwayland's");
            }
            None if x11.is_none() => {
                tracing::warn!("can't force {title:?} to quit: its client has no process");
            }
            None => {}
        }
        if let Some(window) = x11 {
            self.kill_x_client(&window);
        }
    }

    /// Whether `pid` is a process Force Quit never kills, and whose.
    fn spared(&self, pid: Pid) -> Option<Spared> {
        if pid.as_raw_nonzero().get().cast_unsigned() == std::process::id() {
            return Some(Spared::Caminito);
        }
        let xwayland = self.xwayland_display().and_then(crate::xwayland::process);
        (xwayland == Some(pid)).then_some(Spared::Xwayland)
    }
}
