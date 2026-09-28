use smithay::input::pointer::CursorIcon;

use super::*;

fn edges(top: bool, bottom: bool, left: bool, right: bool) -> FrameControl {
    FrameControl::Resize(ResizeEdges {
        top,
        bottom,
        left,
        right,
    })
}

#[test]
fn each_zone_has_marcos_cursor() {
    // `frames.c`'s `meta_frames_update_prelit_control`.
    let cases = [
        (FrameControl::ClientArea, CursorIcon::Default),
        (FrameControl::None, CursorIcon::Default),
        (FrameControl::Title, CursorIcon::Default),
        (
            FrameControl::Button(ButtonFunction::Close),
            CursorIcon::Default,
        ),
        (
            FrameControl::Button(ButtonFunction::Menu),
            CursorIcon::Default,
        ),
        (edges(true, false, false, false), CursorIcon::NResize),
        (edges(false, true, false, false), CursorIcon::SResize),
        (edges(false, false, true, false), CursorIcon::WResize),
        (edges(false, false, false, true), CursorIcon::EResize),
        (edges(true, false, true, false), CursorIcon::NwResize),
        (edges(true, false, false, true), CursorIcon::NeResize),
        (edges(false, true, true, false), CursorIcon::SwResize),
        (edges(false, true, false, true), CursorIcon::SeResize),
    ];
    for (control, cursor) in cases {
        assert_eq!(control.cursor(), cursor, "{control:?}");
    }
}
