//! What each part of a frame does under the pointer (07): Marco's frame controls, where
//! they are (`frames.c`'s `get_control`), and the cursor each shows.

use camino_policy::ResizeEdges;
use camino_theme::{ButtonFunction, FrameFlags, FrameGeometry, Rect};
use smithay::input::pointer::CursorIcon;

/// How far into the frame the bottom and side resize areas, and the corners, reach past
/// the borders (`RESIZE_EXTENDS`).
const RESIZE_EXTENDS: i32 = 15;
/// How far down from the invisible border the top resize area reaches
/// (`TOP_RESIZE_HEIGHT`). Marco multiplies it by the scale in device pixels, which is the
/// same in logical ones.
const TOP_RESIZE_HEIGHT: i32 = 4;

/// The buttons `get_control` looks for before it reads the flags, in its order, and
/// the ones after the title.
const BUTTONS_BEFORE_TITLE: [ButtonFunction; 4] = [
    ButtonFunction::Close,
    ButtonFunction::Minimize,
    ButtonFunction::Menu,
    ButtonFunction::Appmenu,
];
const BUTTONS_AFTER_TITLE: [ButtonFunction; 7] = [
    ButtonFunction::Maximize,
    ButtonFunction::Shade,
    ButtonFunction::Unshade,
    ButtonFunction::Above,
    ButtonFunction::Unabove,
    ButtonFunction::Stick,
    ButtonFunction::Unstick,
];

/// A part of a frame the pointer can be on (`MetaFrameControl`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameControl {
    /// Inside the frame, where the client is.
    ClientArea,
    /// The titlebar outside its buttons.
    Title,
    Button(ButtonFunction),
    /// A border or corner that resizes these edges.
    Resize(ResizeEdges),
    /// The frame, doing nothing.
    None,
}

impl FrameControl {
    /// The cursor Marco shows over the control (`meta_frames_update_prelit_control`): a
    /// resize cursor on a border or corner, and the default one everywhere else.
    pub fn cursor(self) -> CursorIcon {
        let FrameControl::Resize(edges) = self else {
            return CursorIcon::Default;
        };
        resize_cursor(edges)
    }
}

/// The control at `(x, y)` in logical pixels from the frame's top left corner, the
/// invisible border included, as Marco's `get_control` finds it. Maximise and unmaximise
/// are both [`ButtonFunction::Maximize`].
pub fn control_at(geometry: &FrameGeometry, flags: &FrameFlags, x: i32, y: i32) -> FrameControl {
    let total = geometry.borders.total;
    let client = Rect {
        x: total.left,
        y: total.top,
        width: geometry.width - total.right - total.left,
        height: geometry.height - total.bottom - total.top,
    };
    if contains(client, x, y) {
        return FrameControl::ClientArea;
    }
    let button_at = |buttons: &[ButtonFunction]| {
        buttons.iter().copied().find(|&function| {
            geometry
                .button(function)
                .is_some_and(|space| contains(space.clickable, x, y))
        })
    };
    if let Some(function) = button_at(&BUTTONS_BEFORE_TITLE) {
        return FrameControl::Button(function);
    }

    let vertical = flags.allows_vertical_resize;
    let horizontal = flags.allows_horizontal_resize;
    if contains(geometry.title_rect, x, y) {
        return if vertical && y <= TOP_RESIZE_HEIGHT {
            resize(true, false, false, false)
        } else {
            FrameControl::Title
        };
    }
    if let Some(function) = button_at(&BUTTONS_AFTER_TITLE) {
        return FrameControl::Button(function);
    }

    // The south resize areas win over the north ones where they overlap.
    let invisible_top = geometry.borders.invisible.top;
    let south = y >= geometry.height - total.bottom - RESIZE_EXTENDS;
    let north = y < invisible_top + RESIZE_EXTENDS;
    let west = x <= total.left + RESIZE_EXTENDS;
    let east = x >= geometry.width - total.right - RESIZE_EXTENDS;
    // A corner resizes both ways if it can, else whichever way it can.
    let corner = |top: bool, left: bool| match (vertical, horizontal) {
        (true, true) => Some(resize(top, !top, left, !left)),
        (true, false) => Some(resize(top, !top, false, false)),
        (false, true) => Some(resize(false, false, left, !left)),
        (false, false) => None,
    };
    let control = if south && east {
        corner(false, false)
    } else if south && west {
        corner(false, true)
    } else if north && west {
        corner(true, true)
    } else if north && east {
        corner(true, false)
    } else if y < invisible_top + TOP_RESIZE_HEIGHT {
        vertical.then(|| resize(true, false, false, false))
    } else if south {
        vertical.then(|| resize(false, true, false, false))
    } else if west {
        horizontal.then(|| resize(false, false, true, false))
    } else if east {
        horizontal.then(|| resize(false, false, false, true))
    } else {
        None
    };
    control.unwrap_or(if y >= total.top {
        FrameControl::None
    } else {
        FrameControl::Title
    })
}

/// Whether `(x, y)`, from the frame's top left corner, takes input: it's in the frame,
/// and not in a pixel the rounded corners cut out (07).
pub fn in_input_region(geometry: &FrameGeometry, x: i32, y: i32) -> bool {
    let frame = Rect {
        x: 0,
        y: 0,
        width: geometry.width,
        height: geometry.height,
    };
    contains(frame, x, y)
        && !geometry
            .corner_cutouts(1)
            .into_iter()
            .any(|cutout| contains(cutout, x, y))
}

fn resize(top: bool, bottom: bool, left: bool, right: bool) -> FrameControl {
    FrameControl::Resize(ResizeEdges {
        top,
        bottom,
        left,
        right,
    })
}

/// Marco's `POINT_IN_RECT`.
fn contains(rect: Rect, x: i32, y: i32) -> bool {
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}

/// The cursor for resizing `edges`.
pub fn resize_cursor(edges: ResizeEdges) -> CursorIcon {
    match (edges.top, edges.bottom, edges.left, edges.right) {
        (true, _, true, _) => CursorIcon::NwResize,
        (true, _, _, true) => CursorIcon::NeResize,
        (_, true, true, _) => CursorIcon::SwResize,
        (_, true, _, true) => CursorIcon::SeResize,
        (true, ..) => CursorIcon::NResize,
        (_, true, ..) => CursorIcon::SResize,
        (_, _, true, _) => CursorIcon::WResize,
        (_, _, _, true) => CursorIcon::EResize,
        _ => CursorIcon::Default,
    }
}

#[cfg(test)]
mod tests;
