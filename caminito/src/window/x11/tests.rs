use caminito_policy::WindowType;
use caminito_theme::FrameType;
use smithay::xwayland::xwm::WmWindowType;

use super::marco_type;
use crate::decoration::style::{FrameFacts, frame_style};
use crate::window::{Skips, skips};

#[test]
fn ewmh_types_map_to_marcos_one_to_one() {
    // window.c:6576-6593.
    for (ewmh, marco) in [
        (WmWindowType::Normal, WindowType::Normal),
        (WmWindowType::Desktop, WindowType::Desktop),
        (WmWindowType::Dock, WindowType::Dock),
        (WmWindowType::Toolbar, WindowType::Toolbar),
        (WmWindowType::Menu, WindowType::Menu),
        (WmWindowType::Dialog, WindowType::Dialog),
        (WmWindowType::Utility, WindowType::Utility),
        (WmWindowType::Splash, WindowType::Splashscreen),
    ] {
        assert_eq!(marco_type(Some(ewmh), false, false), marco, "{ewmh:?}");
        assert_eq!(
            marco_type(Some(ewmh), true, false),
            marco,
            "{ewmh:?}, transient"
        );
    }
}

#[test]
fn a_window_with_no_type_is_a_dialog_if_transient_and_normal_if_not() {
    assert_eq!(marco_type(None, false, false), WindowType::Normal);
    assert_eq!(marco_type(None, true, false), WindowType::Dialog);
}

#[test]
fn only_a_dialog_with_the_modal_state_is_a_modal_dialog() {
    // window.c:6605-6607.
    assert_eq!(
        marco_type(Some(WmWindowType::Dialog), false, true),
        WindowType::ModalDialog
    );
    assert_eq!(marco_type(None, true, true), WindowType::ModalDialog);
    assert_eq!(
        marco_type(Some(WmWindowType::Normal), true, true),
        WindowType::Normal
    );
    assert_eq!(marco_type(None, false, true), WindowType::Normal);
}

#[test]
fn override_redirect_types_on_a_managed_window_count_as_no_type() {
    // Marco doesn't know their atoms (atomnames.h), so they're skipped.
    for ewmh in [
        WmWindowType::DropdownMenu,
        WmWindowType::PopupMenu,
        WmWindowType::Tooltip,
        WmWindowType::Notification,
        WmWindowType::Combo,
        WmWindowType::Dnd,
    ] {
        assert_eq!(marco_type(Some(ewmh), false, false), WindowType::Normal);
        assert_eq!(marco_type(Some(ewmh), true, false), WindowType::Dialog);
    }
}

/// Marco's frame for a window of `window_type`, if it gets one.
fn frame(window_type: WindowType) -> Option<FrameType> {
    let facts = FrameFacts {
        window_type,
        has_parent: false,
        hints: Default::default(),
        activated: true,
        maximized: false,
        fullscreen: false,
        shaded: false,
    };
    frame_style(&facts, false).map(|(frame_type, _)| frame_type)
}

#[test]
fn each_type_is_framed_and_skips_the_taskbar_and_pager_as_in_marco() {
    let none = Skips::default();
    // recalc_window_features: frames (window.c:6772-6806) and skipping (6861-6891).
    let table = [
        (WindowType::Normal, Some(FrameType::Normal), (false, false)),
        (WindowType::Dialog, Some(FrameType::Dialog), (false, false)),
        (
            WindowType::ModalDialog,
            Some(FrameType::ModalDialog),
            (false, false),
        ),
        (WindowType::Menu, Some(FrameType::Menu), (true, true)),
        (WindowType::Utility, Some(FrameType::Utility), (true, true)),
        (WindowType::Toolbar, None, (true, true)),
        (WindowType::Splashscreen, None, (true, true)),
        (WindowType::Dock, None, (true, true)),
        (WindowType::Desktop, None, (true, true)),
    ];
    for (window_type, framed, (taskbar, pager)) in table {
        assert_eq!(frame(window_type), framed, "{window_type:?}");
        assert_eq!(
            skips(window_type, false, none),
            Skips { taskbar, pager },
            "{window_type:?}"
        );
    }
}

#[test]
fn a_dialog_with_a_transient_parent_skips_only_the_taskbar() {
    for window_type in [WindowType::Dialog, WindowType::ModalDialog] {
        let expected = Skips {
            taskbar: true,
            pager: false,
        };
        assert_eq!(skips(window_type, true, Skips::default()), expected);
    }
    assert_eq!(
        skips(WindowType::Normal, true, Skips::default()),
        Skips::default()
    );
}

#[test]
fn a_client_can_skip_the_taskbar_or_pager_itself() {
    let asked = Skips {
        taskbar: true,
        pager: false,
    };
    assert_eq!(skips(WindowType::Normal, false, asked), asked);
}
