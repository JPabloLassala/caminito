use camino_policy::{SizeHints, WindowType};
use camino_theme::{FrameFlags, FrameType};

use super::{FrameFacts, frame_style};

fn normal() -> FrameFacts {
    FrameFacts {
        window_type: WindowType::Normal,
        has_parent: false,
        hints: SizeHints::default(),
        activated: false,
        maximized: false,
        fullscreen: false,
        shaded: false,
    }
}

/// What Marco allows a normal, resizable window.
fn allows_all() -> FrameFlags {
    FrameFlags {
        allows_delete: true,
        allows_menu: true,
        allows_minimize: true,
        allows_maximize: true,
        allows_vertical_resize: true,
        allows_horizontal_resize: true,
        allows_shade: true,
        allows_move: true,
        ..FrameFlags::default()
    }
}

#[test]
fn a_normal_window_gets_the_normal_style_with_everything_allowed() {
    assert_eq!(
        frame_style(&normal(), false),
        Some((FrameType::Normal, allows_all()))
    );
}

#[test]
fn a_window_with_a_parent_is_a_dialog_which_may_still_minimise() {
    let facts = FrameFacts {
        window_type: WindowType::Dialog,
        has_parent: true,
        ..normal()
    };
    assert_eq!(
        frame_style(&facts, false),
        Some((FrameType::Dialog, allows_all()))
    );
}

#[test]
fn a_modal_dialog_is_attached_only_with_a_parent_and_the_key_on() {
    let modal = FrameFacts {
        window_type: WindowType::ModalDialog,
        ..normal()
    };
    let (frame_type, flags) = frame_style(&modal, false).unwrap();
    assert_eq!(frame_type, FrameType::ModalDialog);
    // Marco lets only normal windows and dialogs minimise and maximise.
    assert!(!flags.allows_minimize && !flags.allows_maximize);

    assert_eq!(frame_style(&modal, true).unwrap().0, FrameType::ModalDialog);
    let with_parent = FrameFacts {
        has_parent: true,
        ..modal
    };
    assert_eq!(
        frame_style(&with_parent, false).unwrap().0,
        FrameType::ModalDialog
    );
    assert_eq!(
        frame_style(&with_parent, true),
        Some((
            FrameType::Attached,
            // Border only, and not resizable.
            FrameFlags {
                allows_move: true,
                ..FrameFlags::default()
            }
        ))
    );
}

#[test]
fn a_window_that_cant_resize_cant_maximise() {
    let facts = FrameFacts {
        hints: SizeHints {
            min: (300, 200).into(),
            max: (300, 200).into(),
        },
        ..normal()
    };
    let (_, flags) = frame_style(&facts, false).unwrap();
    assert_eq!(
        flags,
        FrameFlags {
            allows_maximize: false,
            allows_vertical_resize: false,
            allows_horizontal_resize: false,
            ..allows_all()
        }
    );
}

#[test]
fn a_window_resizable_one_way_allows_only_that_way() {
    let facts = FrameFacts {
        hints: SizeHints {
            min: (300, 200).into(),
            max: (300, 0).into(),
        },
        ..normal()
    };
    let (_, flags) = frame_style(&facts, false).unwrap();
    assert!(flags.allows_maximize);
    assert!(flags.allows_vertical_resize);
    assert!(!flags.allows_horizontal_resize);
}

#[test]
fn focus_and_maximising_show_in_the_flags_and_a_maximised_frame_doesnt_resize() {
    let facts = FrameFacts {
        activated: true,
        maximized: true,
        ..normal()
    };
    let (_, flags) = frame_style(&facts, false).unwrap();
    assert_eq!(
        flags,
        FrameFlags {
            has_focus: true,
            maximized: true,
            allows_vertical_resize: false,
            allows_horizontal_resize: false,
            ..allows_all()
        }
    );
}

#[test]
fn a_fullscreen_frame_neither_moves_resizes_shades_nor_maximises() {
    let facts = FrameFacts {
        fullscreen: true,
        ..normal()
    };
    let (_, flags) = frame_style(&facts, false).unwrap();
    assert_eq!(
        flags,
        FrameFlags {
            allows_delete: true,
            allows_menu: true,
            allows_minimize: true,
            fullscreen: true,
            ..FrameFlags::default()
        }
    );
}

#[test]
fn a_shaded_frame_is_drawn_shaded_and_cant_be_resized() {
    let facts = FrameFacts {
        shaded: true,
        ..normal()
    };
    let (_, flags) = frame_style(&facts, false).unwrap();
    assert_eq!(
        flags,
        FrameFlags {
            allows_vertical_resize: false,
            allows_horizontal_resize: false,
            shaded: true,
            ..allows_all()
        }
    );
}

#[test]
fn a_utility_window_gets_its_own_style_without_minimise_or_maximise() {
    let facts = FrameFacts {
        window_type: WindowType::Utility,
        ..normal()
    };
    let (frame_type, flags) = frame_style(&facts, false).unwrap();
    assert_eq!(frame_type, FrameType::Utility);
    assert!(!flags.allows_minimize && !flags.allows_maximize);
}

#[test]
fn marco_frames_no_desktop_dock_toolbar_or_splash_screen() {
    for window_type in [
        WindowType::Desktop,
        WindowType::Dock,
        WindowType::Toolbar,
        WindowType::Splashscreen,
    ] {
        let facts = FrameFacts {
            window_type,
            ..normal()
        };
        assert_eq!(frame_style(&facts, false), None, "{window_type:?}");
    }
}
