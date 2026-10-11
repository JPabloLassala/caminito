//! A window's functions: what Marco lets it do, from what it is (177). This is Caminito's
//! one port of `recalc_window_features` (window.c:6670-6830, 1.29.0). Policy's setters
//! refuse a state the window has no function for, and its Frame shows them in its buttons
//! and borders.

use crate::{SizeHints, WindowType};

/// What a window's functions are worked out from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionFacts {
    pub window_type: WindowType,
    pub hints: SizeHints,
    /// It's transient for a managed window.
    pub has_parent: bool,
    /// Caminito draws its frame.
    pub ssd: bool,
    pub fullscreen: bool,
}

/// What Marco lets a window do (`has_*_func`), and whether its frame is a border alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Functions {
    pub minimize: bool,
    /// Maximising both ways.
    pub maximize: bool,
    /// Resizing each way. Marco's `has_resize_func` is either, and its frame's
    /// `META_WINDOW_ALLOWS_HORIZONTAL_RESIZE` and `_VERTICAL_RESIZE` are these
    /// (window-private.h:437-441).
    pub resize_horizontally: bool,
    pub resize_vertically: bool,
    pub fullscreen: bool,
    pub shade: bool,
    /// The frame has no titlebar, as an attached modal dialog's.
    pub border_only: bool,
}

impl Functions {
    /// Marco's `has_resize_func`, which maximising one way and a keyboard resize need.
    pub fn resize(&self) -> bool {
        self.resize_horizontally || self.resize_vertically
    }
}

/// The functions of a window of `facts`, with `attach-modal-dialogs` on or off, as
/// `recalc_window_features` works them out, but for fullscreen: a normal window or a dialog
/// always has that function, where Marco refuses one that can't resize unless its least
/// size is the screen's, which an X11 game arranges and a Wayland client can't (177).
///
/// Caminito reads no `_MOTIF_WM_HINTS` functions, so Marco's `mwm_has_*_func` are all on.
pub fn functions(facts: &FunctionFacts, attach_modal_dialogs: bool) -> Functions {
    let window_type = facts.window_type;
    let border_only =
        window_type == WindowType::ModalDialog && attach_modal_dialogs && facts.has_parent;
    let desktop_dock_or_splash = matches!(
        window_type,
        WindowType::Desktop | WindowType::Dock | WindowType::Splashscreen
    );
    // Marco's `decorated`, which those and a toolbar never are.
    let decorated = facts.ssd && !desktop_dock_or_splash && window_type != WindowType::Toolbar;
    let normal_or_dialog = matches!(window_type, WindowType::Normal | WindowType::Dialog);

    let SizeHints { min, max } = facts.hints;
    // A zero maximum is unbounded.
    let grows = |min: i32, max: i32| max == 0 || min < max;
    let resizes = !border_only && !desktop_dock_or_splash && !facts.fullscreen;
    let resize_horizontally = resizes && grows(min.w, max.w);
    let resize_vertically = resizes && grows(min.h, max.h);
    Functions {
        minimize: normal_or_dialog,
        maximize: normal_or_dialog && (resize_horizontally || resize_vertically),
        resize_horizontally,
        resize_vertically,
        fullscreen: normal_or_dialog,
        shade: decorated && !border_only && !facts.fullscreen,
        border_only,
    }
}
