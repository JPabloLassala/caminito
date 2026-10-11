//! Focus-stealing prevention (165): whether a new window, or a client's request to activate
//! one, takes the focus as `focus-new-windows` allows, and the urgency of a window refused
//! it.
//!
//! Marco compares a new window's `_NET_WM_USER_TIME` with the focused window's
//! (window.c:1835-1931): a window whose launch is older than the user's last interaction
//! with the focused window is refused, and a window with no time at all, or mapped while
//! nothing has the focus, takes it. Policy maps that onto xdg-activation tokens: a token's
//! time is when Caminito issued it, and the focused window's user time is when the seat's
//! focus last changed. *strict* differs from *smart* in Marco only for terminals; here it
//! asks for a token tied to user input, as 05 has it.

use std::time::Duration;

use crate::{PolicyWindow, SeatId, WindowId};

use super::Policy;

/// How long after Caminito issued it a token still gives the focus.
pub const TOKEN_LIFETIME: Duration = Duration::from_secs(10);

/// `focus-new-windows`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusNewWindows {
    /// A valid and recent token gives the focus, and a client's first window takes it with
    /// none. Marco's default.
    #[default]
    Smart,
    /// Only a recent token tied to user input gives the focus.
    Strict,
}

/// `alt-tab-urgent-placement`, Marco master's key: where the Window switcher lists an urgent
/// window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UrgentPlacement {
    /// First, before the others: master's default.
    Start,
    /// In its place by when it last had the focus, as 108 has it and Marco 1.29.0, which has
    /// no key, does.
    #[default]
    Mru,
}

/// An xdg-activation token Caminito issued, as policy judges it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivationToken {
    /// When Caminito issued it, on the clock policy is given `now` by.
    pub issued: Duration,
    /// Whether it's tied to user input: asked for with a serial of an input event of the
    /// client that had the keyboard.
    pub input: bool,
}

/// What a window maps with to claim the focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusClaim {
    /// Nothing is checked, and the window takes the focus as every window did before 165:
    /// XWayland's windows, until 166.
    #[default]
    Unchecked,
    /// A native window with no token, or with one Caminito doesn't know.
    NoToken,
    /// A native window with a token.
    Token(ActivationToken),
}

impl<W: PolicyWindow> Policy<W> {
    /// A client asks to activate `id` with a token, or with one Caminito doesn't know. A
    /// token `focus-new-windows` takes activates it as `activate` does; otherwise it's
    /// made urgent, unless it has the focus already.
    pub fn request_activation(
        &mut self,
        seat: SeatId,
        id: WindowId,
        token: Option<ActivationToken>,
        now: Duration,
    ) {
        if !self.windows.contains_key(&id) {
            return;
        }
        if token.is_some_and(|token| self.token_gives_focus(seat, token, now)) {
            self.activate(seat, id, now);
        } else if self.focus(seat) != Some(id)
            && let Some(managed) = self.windows.get_mut(&id)
        {
            managed.demands_attention = true;
        }
    }

    /// Whether the window asks for the user's attention: it was refused the focus and hasn't
    /// had it since, or its client says so.
    pub fn is_urgent(&self, id: WindowId) -> bool {
        self.windows
            .get(&id)
            .is_some_and(|managed| managed.demands_attention || managed.window.urgent())
    }

    /// Whether `id`, which has just mapped, may take the focus with `claim`.
    pub(super) fn claim_gives_focus(&self, id: WindowId, claim: FocusClaim, now: Duration) -> bool {
        let seat = self.last_seat;
        let token = match claim {
            FocusClaim::Unchecked => return true,
            FocusClaim::NoToken => None,
            FocusClaim::Token(token) => Some(token),
        };
        if token.is_some_and(|token| self.token_gives_focus(seat, token, now)) {
            return true;
        }
        // Marco focuses a window mapped while nothing has the focus.
        let Some(focused) = self.focus(seat) else {
            return true;
        };
        // A window of the focused window's client is as tied to the user's input as a token
        // of that client's would be: Marco's toolkits give it the app's last user time.
        let pid = self.windows[&id].window.pid();
        let same_client =
            |other: &WindowId| pid.is_some() && self.windows[other].window.pid() == pid;
        if same_client(&focused) {
            return true;
        }
        // MATE's launchers pass no token yet (05).
        self.config.focus_new_windows == FocusNewWindows::Smart
            && !self
                .windows
                .keys()
                .any(|other| *other != id && same_client(other))
    }

    /// Whether `token` gives the focus for `seat` now: it's recent, no older than the seat's
    /// last focus change, and with *strict* tied to user input.
    fn token_gives_focus(&self, seat: SeatId, token: ActivationToken, now: Duration) -> bool {
        let recent = now.saturating_sub(token.issued) <= TOKEN_LIFETIME
            && !self
                .focus_time
                .get(&seat)
                .is_some_and(|last| token.issued < *last);
        recent && (self.config.focus_new_windows == FocusNewWindows::Smart || token.input)
    }

    /// Refuses a window that has just mapped the focus, as Marco's `meta_window_show` does
    /// (window.c:2190-2237): it goes just below the focused window rather than over it, and
    /// is urgent.
    pub(super) fn refuse_focus(&mut self, id: WindowId) {
        self.windows.get_mut(&id).unwrap().demands_attention = true;
        let Some(focused) = self.focus(self.last_seat) else {
            return;
        };
        let put_below = |stack: &mut Vec<WindowId>| {
            stack.retain(|w| *w != id);
            if let Some(below) = stack.iter().position(|w| *w == focused) {
                stack.insert(below, id);
            }
        };
        put_below(&mut self.stack);
        // The stacking an open Window switcher puts back has it there too.
        if let Some(saved) = self.switcher_saved_stack() {
            put_below(saved);
        }
    }
}
