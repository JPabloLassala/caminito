//! Which frame Marco draws for a window: its frame type and flags, from what the window is
//! and what state it's in (`core.c`'s `META_CORE_GET_FRAME_TYPE`, `frame.c`'s
//! `meta_frame_get_flags` and `window.c`'s `recalc_window_features`).

use camino_policy::{SizeHints, WindowType};
use camino_theme::{FrameFlags, FrameType};

/// What a window's frame type and flags depend on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameFacts {
    pub window_type: WindowType,
    /// It's transient for a managed window.
    pub has_parent: bool,
    pub hints: SizeHints,
    pub activated: bool,
    pub maximized: bool,
    pub fullscreen: bool,
    pub shaded: bool,
}

/// The frame type and flags Marco draws a window with, or `None` for a type it never frames.
///
/// Sticking and keeping above come in M8, so their flags stay off.
pub fn frame_style(
    facts: &FrameFacts,
    attach_modal_dialogs: bool,
) -> Option<(FrameType, FrameFlags)> {
    let attached = attach_modal_dialogs && facts.has_parent;
    let frame_type = match facts.window_type {
        WindowType::Normal => FrameType::Normal,
        WindowType::Dialog => FrameType::Dialog,
        WindowType::ModalDialog if attached => FrameType::Attached,
        WindowType::ModalDialog => FrameType::ModalDialog,
        WindowType::Menu => FrameType::Menu,
        WindowType::Utility => FrameType::Utility,
        WindowType::Desktop | WindowType::Dock | WindowType::Toolbar | WindowType::Splashscreen => {
            return None;
        }
    };
    // An attached dialog is drawn border only, and can't be resized.
    let border_only = frame_type == FrameType::Attached;

    let SizeHints { min, max } = facts.hints;
    // xdg-shell's zero maximum is unbounded.
    let grows = |min: i32, max: i32| max == 0 || min < max;
    let has_resize = !border_only && (grows(min.w, max.w) || grows(min.h, max.h));
    let normal_or_dialog = matches!(facts.window_type, WindowType::Normal | WindowType::Dialog);
    let has_minimize = normal_or_dialog;
    let has_maximize = normal_or_dialog && has_resize && !facts.fullscreen;
    let has_shade = !border_only && !facts.fullscreen;
    let resize = has_resize && !facts.maximized && !facts.fullscreen && !facts.shaded;

    let decorated = !border_only;
    let flags = FrameFlags {
        allows_delete: decorated,
        allows_menu: decorated,
        allows_appmenu: false,
        allows_minimize: decorated && has_minimize,
        allows_maximize: decorated && has_maximize,
        allows_vertical_resize: resize && grows(min.h, max.h),
        allows_horizontal_resize: resize && grows(min.w, max.w),
        has_focus: facts.activated,
        shaded: facts.shaded,
        stuck: false,
        maximized: facts.maximized,
        allows_shade: decorated && has_shade,
        allows_move: !facts.fullscreen,
        fullscreen: facts.fullscreen,
        is_flashing: false,
        above: false,
        tiled_left: false,
        tiled_right: false,
    };
    Some((frame_type, flags))
}

#[cfg(test)]
mod tests;
