//! Which frame Marco draws for a window: its frame type and flags, from its type, the
//! functions Policy gives it (177) and the state it's in (`core.c`'s
//! `META_CORE_GET_FRAME_TYPE` and `frame.c`'s `meta_frame_get_flags`).

use caminito_policy::{Functions, WindowType};
use caminito_theme::{FrameFlags, FrameType};

/// What a window's frame type and flags depend on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameFacts {
    pub window_type: WindowType,
    /// What the window can do, as Policy has it.
    pub functions: Functions,
    pub activated: bool,
    pub maximized: bool,
    pub fullscreen: bool,
    pub shaded: bool,
    /// Tiled to the left or right half of a work area (158).
    pub tiled_left: bool,
    pub tiled_right: bool,
}

/// The frame type and flags Marco draws a window with, or `None` for a type it never frames.
///
/// Whether the window is sticky or kept above isn't a fact here: those flags are set as the
/// frame is drawn (155), and its extents don't depend on them.
pub fn frame_style(facts: &FrameFacts) -> Option<(FrameType, FrameFlags)> {
    let functions = facts.functions;
    let frame_type = match facts.window_type {
        WindowType::Normal => FrameType::Normal,
        WindowType::Dialog => FrameType::Dialog,
        // An attached dialog is the one modal dialog with a border only.
        WindowType::ModalDialog if functions.border_only => FrameType::Attached,
        WindowType::ModalDialog => FrameType::ModalDialog,
        WindowType::Menu => FrameType::Menu,
        WindowType::Utility => FrameType::Utility,
        WindowType::Desktop | WindowType::Dock | WindowType::Toolbar | WindowType::Splashscreen => {
            return None;
        }
    };
    // `META_WINDOW_ALLOWS_RESIZE_EXCEPT_HINTS` (window-private.h:441).
    let resize = !facts.maximized && !facts.fullscreen && !facts.shaded;
    // A side tile fills its work area's height (window-private.h:446).
    let side_tiled = facts.tiled_left || facts.tiled_right;

    // A frame that's a border only has no titlebar to act on the window from.
    let decorated = !functions.border_only;
    let flags = FrameFlags {
        allows_delete: decorated,
        allows_menu: decorated,
        allows_appmenu: false,
        allows_minimize: decorated && functions.minimize,
        allows_maximize: decorated && functions.maximize,
        allows_vertical_resize: resize && functions.resize_vertically && !side_tiled,
        allows_horizontal_resize: resize && functions.resize_horizontally,
        has_focus: facts.activated,
        shaded: facts.shaded,
        stuck: false,
        maximized: facts.maximized,
        allows_shade: decorated && functions.shade,
        allows_move: !facts.fullscreen,
        fullscreen: facts.fullscreen,
        is_flashing: false,
        above: false,
        tiled_left: facts.tiled_left,
        tiled_right: facts.tiled_right,
    };
    Some((frame_type, flags))
}

#[cfg(test)]
mod tests;
