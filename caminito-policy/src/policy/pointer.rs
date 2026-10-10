//! The pointer focus modes, `raise-on-click` and auto-raise (153): what the pointer
//! crossing into a window or onto the desktop does, as Marco's `EnterNotify` handling does
//! it (display.c:2093-2183), and auto-raise's deadline, which the compositor runs with one
//! loop timer, as it runs idle's (128).

use std::collections::BTreeMap;
use std::time::Duration;

use crate::{Config, PolicyWindow, SeatId, WindowId, WindowType};

use super::Policy;

/// `focus-mode`: what gives a window the focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusMode {
    /// A click, Marco's default.
    #[default]
    Click,
    /// The pointer entering the window; leaving it for the desktop keeps the focus there.
    Sloppy,
    /// As `Sloppy`, but leaving the window for the desktop takes the focus away.
    Mouse,
}

/// What the pointer is over, as the compositor reports it each time that changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerTarget {
    /// A managed window: one of its surfaces, or its Frame.
    Window(WindowId),
    /// The desktop: no window and no Layer surface, or a Layer surface in the bottom or
    /// background layer, such as caja's desktop or the wallpaper.
    Desktop,
    /// Anything else: a Layer surface above the windows, such as the panel, an
    /// override-redirect window, a Layer surface's popup, the Window switcher or the lock.
    Other,
}

/// What made the pointer cross onto a new target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Crossing {
    /// The pointer moved.
    Motion,
    /// What's under a still pointer changed: a window mapped, went, moved or was restacked
    /// there, or the workspace changed.
    Scene,
}

/// An auto-raise waiting for its delay.
#[derive(Debug, Clone, Copy)]
struct AutoRaise {
    seat: SeatId,
    /// The window the pointer entered, which is raised.
    window: WindowId,
    /// The window that took the focus, which is `window` or its modal dialog.
    focused: WindowId,
    due: Duration,
}

/// Policy's state for the pointer focus modes.
pub(super) struct PointerFocus {
    mode: FocusMode,
    raise_on_click: bool,
    /// `auto-raise-delay`, while `auto-raise` is on.
    auto_raise: Option<Duration>,
    /// What each seat's pointer was last reported over.
    targets: BTreeMap<SeatId, PointerTarget>,
    /// Marco's `mouse_mode` (display.c:391): off once the keyboard has moved the focus, as
    /// the Window switcher does (keybindings.c:3069, 3100), and on again at the next
    /// crossing the pointer's motion makes. While it's off, a crossing of the scene's
    /// changes the focus no more than Marco's focus sentinel lets the enters that showing
    /// windows make (window.c:1682-1698).
    mouse_mode: bool,
    pending: Option<AutoRaise>,
}

impl Default for PointerFocus {
    fn default() -> Self {
        Self {
            mode: FocusMode::default(),
            raise_on_click: true,
            auto_raise: None,
            targets: BTreeMap::new(),
            mouse_mode: true,
            pending: None,
        }
    }
}

impl PointerFocus {
    /// Takes the settings, which apply from the next crossing. Leaving the pointer modes or
    /// turning auto-raise off drops a waiting auto-raise.
    pub(super) fn settings_changed(&mut self, config: &Config) {
        self.mode = config.focus_mode;
        self.raise_on_click = config.raise_on_click;
        self.auto_raise = config.auto_raise.then_some(config.auto_raise_delay);
        if self.mode == FocusMode::Click || self.auto_raise.is_none() {
            self.pending = None;
        }
    }

    pub(super) fn raise_on_click(&self) -> bool {
        self.raise_on_click
    }

    /// The keyboard moved the focus: crossings of the scene's leave it until the pointer
    /// moves.
    pub(super) fn keyboard_focused(&mut self) {
        self.mouse_mode = false;
    }
}

/// What a crossing gives the focus to.
enum Entered {
    Window(WindowId),
    Desktop,
    Nothing,
}

impl<W: PolicyWindow> Policy<W> {
    /// `seat`'s pointer crossed onto `target`, leaving what it was over. With a pointer
    /// focus mode, entering a window focuses it, or the modal dialog over it, and queues its
    /// auto-raise; entering the desktop, or a desktop window, takes the focus away under
    /// `mouse`. A dock or anything else isn't focused, as in Marco. The Window switcher and
    /// a move or resize hold the modes off while they last.
    pub fn pointer_entered(
        &mut self,
        seat: SeatId,
        target: PointerTarget,
        crossing: Crossing,
        now: Duration,
    ) {
        let pointer = &mut self.pointer_focus;
        if pointer.targets.insert(seat, target) == Some(target) {
            return;
        }
        // Leaving the window drops its auto-raise.
        pointer
            .pending
            .take_if(|p| p.seat == seat && target != PointerTarget::Window(p.window));
        if pointer.mode == FocusMode::Click
            || self.pointer_modes_held(seat)
            || (crossing == Crossing::Scene && !self.pointer_focus.mouse_mode)
        {
            return;
        }
        self.pointer_focus.mouse_mode = true;
        let entered = match target {
            PointerTarget::Window(id) if self.is_showing(id) => {
                match self.windows[&id].window_type {
                    WindowType::Desktop => Entered::Desktop,
                    WindowType::Dock => Entered::Nothing,
                    _ => Entered::Window(id),
                }
            }
            PointerTarget::Window(_) | PointerTarget::Other => Entered::Nothing,
            PointerTarget::Desktop => Entered::Desktop,
        };
        match entered {
            Entered::Window(id) => {
                self.set_last_seat(seat);
                if self.focus_window(seat, id, now)
                    && let Some(delay) = self.pointer_focus.auto_raise
                {
                    self.pointer_focus.pending = Some(AutoRaise {
                        seat,
                        window: id,
                        focused: self.focus[&seat],
                        due: now + delay,
                    });
                }
            }
            Entered::Desktop if self.pointer_focus.mode == FocusMode::Mouse => {
                self.set_last_seat(seat);
                self.unfocus(seat, now);
            }
            Entered::Desktop | Entered::Nothing => return,
        }
        self.apply();
    }

    /// When policy next needs `elapse` called: an auto-raise's deadline.
    pub fn deadline(&self) -> Option<Duration> {
        self.pointer_focus.pending.map(|p| p.due)
    }

    /// Runs what's due by `now`: an auto-raise, which raises its window if the pointer is
    /// still in it and it still has the focus, as Marco's
    /// `window_raise_with_delay_callback` checks (display.c:1311-1361).
    pub fn elapse(&mut self, now: Duration) {
        let Some(p) = self.pointer_focus.pending.take_if(|p| p.due <= now) else {
            return;
        };
        let still = self.pointer_focus.targets.get(&p.seat)
            == Some(&PointerTarget::Window(p.window))
            && self.focus(p.seat) == Some(p.focused)
            && self.is_showing(p.window)
            && !self.pointer_modes_held(p.seat);
        if still {
            self.raise_family(p.window);
            self.apply();
        }
    }

    /// Whether the pointer focus modes are held off for `seat`: while the Window switcher
    /// is open (108), and during the seat's move or resize, whose window has the pointer.
    /// The workspace popup is to hold them off too when it comes, with 152.
    fn pointer_modes_held(&self, seat: SeatId) -> bool {
        self.switcher.is_some() || self.grabs.contains_key(&seat)
    }

    /// Takes `seat`'s focus away on user input at `now`, as Marco's
    /// `meta_display_focus_the_no_focus_window` does, unless the request is older than the
    /// seat's last focus change.
    fn unfocus(&mut self, seat: SeatId, now: Duration) {
        if self.focus_time.get(&seat).is_some_and(|last| now < *last) {
            return;
        }
        self.focus_time.insert(seat, now);
        self.focus.remove(&seat);
        self.pointer_focus.pending.take_if(|p| p.seat == seat);
    }
}
