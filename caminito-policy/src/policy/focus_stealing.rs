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
//!
//! XWayland's windows carry no token (X11 startup notification is 05's), so they take the
//! path of a window with none (166). `_NET_ACTIVE_WINDOW` is judged by its source
//! indication, since X's timestamp isn't on Caminito's clock.

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
    /// Nothing is checked, and the window takes the focus as every window did before 165.
    #[default]
    Unchecked,
    /// A native window with no token, or with one Caminito doesn't know, and every
    /// XWayland window (166).
    NoToken,
    /// A native window with a token.
    Token(ActivationToken),
}

/// Who asks to activate a window with no token: `_NET_ACTIVE_WINDOW`'s source indication
/// (166).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationSource {
    /// A pager or a window list, which acts for the user: honoured.
    Pager,
    /// The window's own client: honoured only if that client has the focus already.
    Application,
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
        } else {
            self.set_demands_attention(id, true);
        }
    }

    /// A client asks to activate `id` with no token, as `source`. A pager's request
    /// activates it as `activate` does, and so does an application's while its client has
    /// the focus, or nothing has; otherwise the window is made urgent.
    ///
    /// Marco judges both by the request's timestamp against the user's last input
    /// (window.c:3177-3188), and looks at the source only to raise the window.
    pub fn request_activation_from(
        &mut self,
        seat: SeatId,
        id: WindowId,
        source: ActivationSource,
        now: Duration,
    ) {
        if !self.windows.contains_key(&id) {
            return;
        }
        let honoured = match source {
            ActivationSource::Pager => true,
            ActivationSource::Application => self
                .focus(seat)
                .is_none_or(|focused| focused == id || self.same_client(id, focused)),
        };
        if honoured {
            self.activate(seat, id, now);
        } else {
            self.set_demands_attention(id, true);
        }
    }

    /// Sets or unsets the window's demand for attention, as its client asks with
    /// `_NET_WM_STATE_DEMANDS_ATTENTION`, or as policy does for a window it refuses the
    /// focus. A client can take back a demand policy made, as in Marco (window.c:5370-5378).
    /// A window that has the focus demands none.
    pub fn set_demands_attention(&mut self, id: WindowId, demands_attention: bool) {
        let focused = self.focus.values().any(|focused| *focused == id);
        if let Some(managed) = self.windows.get_mut(&id) {
            managed.demands_attention = demands_attention && !focused;
        }
    }

    /// Whether the window demands attention: policy refused it the focus, or its client
    /// asked, and it hasn't had the focus since. It's what `_NET_WM_STATE_DEMANDS_ATTENTION`
    /// says of an XWayland window.
    pub fn demands_attention(&self, id: WindowId) -> bool {
        self.windows
            .get(&id)
            .is_some_and(|managed| managed.demands_attention)
    }

    /// Whether the window asks for the user's attention: it demands attention, or its
    /// client's urgency hint says so.
    pub fn is_urgent(&self, id: WindowId) -> bool {
        self.windows
            .get(&id)
            .is_some_and(|managed| managed.demands_attention || managed.window.urgent())
    }

    /// Whether the two windows are one client's, which policy tells by the process.
    fn same_client(&self, a: WindowId, b: WindowId) -> bool {
        let pid = self.windows[&a].window.pid();
        pid.is_some() && self.windows[&b].window.pid() == pid
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
        // Marco never refuses a transient of the focused window (window.c:2155).
        if self.same_client(id, focused) || self.is_ancestor(focused, id) {
            return true;
        }
        // MATE's launchers pass no token yet (05).
        self.config.focus_new_windows == FocusNewWindows::Smart
            && !self
                .windows
                .keys()
                .any(|other| *other != id && self.same_client(id, *other))
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
