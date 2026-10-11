use caminito_policy::{Functions, WindowType};
use caminito_theme::{FrameFlags, FrameType};

use super::{FrameFacts, frame_style};

/// Every function but a border only: what Policy gives a normal SSD window that resizes.
fn all_functions() -> Functions {
    Functions {
        minimize: true,
        maximize: true,
        resize_horizontally: true,
        resize_vertically: true,
        fullscreen: true,
        shade: true,
        border_only: false,
    }
}

fn normal() -> FrameFacts {
    FrameFacts {
        window_type: WindowType::Normal,
        functions: all_functions(),
        activated: false,
        maximized: false,
        fullscreen: false,
        shaded: false,
        tiled_left: false,
        tiled_right: false,
    }
}

/// The flags of a frame whose window has every function.
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
fn a_window_with_every_function_has_everything_allowed() {
    assert_eq!(
        frame_style(&normal()),
        Some((FrameType::Normal, allows_all()))
    );
}

#[test]
fn each_function_the_window_lacks_is_a_flag_its_frame_lacks() {
    let flags = |functions| {
        let facts = FrameFacts {
            functions,
            ..normal()
        };
        frame_style(&facts).unwrap().1
    };
    let all = all_functions();

    assert_eq!(
        flags(Functions {
            minimize: false,
            ..all
        }),
        FrameFlags {
            allows_minimize: false,
            ..allows_all()
        }
    );
    assert_eq!(
        flags(Functions {
            maximize: false,
            ..all
        }),
        FrameFlags {
            allows_maximize: false,
            ..allows_all()
        }
    );
    assert_eq!(
        flags(Functions {
            resize_horizontally: false,
            ..all
        }),
        FrameFlags {
            allows_horizontal_resize: false,
            ..allows_all()
        }
    );
    assert_eq!(
        flags(Functions {
            resize_vertically: false,
            ..all
        }),
        FrameFlags {
            allows_vertical_resize: false,
            ..allows_all()
        }
    );
    assert_eq!(
        flags(Functions {
            shade: false,
            ..all
        }),
        FrameFlags {
            allows_shade: false,
            ..allows_all()
        }
    );
    // The fullscreen function has no flag.
    assert_eq!(
        flags(Functions {
            fullscreen: false,
            ..all
        }),
        allows_all()
    );
}

#[test]
fn a_frame_thats_a_border_only_allows_only_the_move_and_the_resizes() {
    let facts = FrameFacts {
        functions: Functions {
            border_only: true,
            ..all_functions()
        },
        ..normal()
    };
    // `meta_frame_get_flags` leaves out the titlebar's flags alone (frame.c:260-281).
    assert_eq!(
        frame_style(&facts).unwrap().1,
        FrameFlags {
            allows_move: true,
            allows_vertical_resize: true,
            allows_horizontal_resize: true,
            ..FrameFlags::default()
        }
    );
}

#[test]
fn each_framed_type_has_its_frame_type() {
    for (window_type, frame_type) in [
        (WindowType::Normal, FrameType::Normal),
        (WindowType::Dialog, FrameType::Dialog),
        (WindowType::ModalDialog, FrameType::ModalDialog),
        (WindowType::Menu, FrameType::Menu),
        (WindowType::Utility, FrameType::Utility),
    ] {
        let facts = FrameFacts {
            window_type,
            ..normal()
        };
        assert_eq!(frame_style(&facts).unwrap().0, frame_type);
    }
}

#[test]
fn a_modal_dialog_with_a_border_only_is_attached() {
    let facts = FrameFacts {
        window_type: WindowType::ModalDialog,
        functions: Functions {
            border_only: true,
            ..Functions::default()
        },
        ..normal()
    };
    assert_eq!(
        frame_style(&facts),
        Some((
            FrameType::Attached,
            FrameFlags {
                allows_move: true,
                ..FrameFlags::default()
            }
        ))
    );
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
        assert_eq!(frame_style(&facts), None, "{window_type:?}");
    }
}

#[test]
fn focus_and_maximising_show_in_the_flags_and_a_maximised_frame_doesnt_resize() {
    let facts = FrameFacts {
        activated: true,
        maximized: true,
        ..normal()
    };
    let (_, flags) = frame_style(&facts).unwrap();
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
fn a_fullscreen_frame_neither_moves_nor_resizes() {
    let facts = FrameFacts {
        fullscreen: true,
        ..normal()
    };
    let (_, flags) = frame_style(&facts).unwrap();
    assert_eq!(
        flags,
        FrameFlags {
            allows_move: false,
            allows_vertical_resize: false,
            allows_horizontal_resize: false,
            fullscreen: true,
            ..allows_all()
        }
    );
}

#[test]
fn a_shaded_frame_is_drawn_shaded_and_cant_be_resized() {
    let facts = FrameFacts {
        shaded: true,
        ..normal()
    };
    let (_, flags) = frame_style(&facts).unwrap();
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
fn a_side_tiled_frame_is_flagged_tiled_and_resizes_only_across() {
    let facts = FrameFacts {
        tiled_right: true,
        ..normal()
    };
    let (_, flags) = frame_style(&facts).unwrap();
    assert_eq!(
        flags,
        FrameFlags {
            tiled_right: true,
            allows_vertical_resize: false,
            ..allows_all()
        }
    );
}
