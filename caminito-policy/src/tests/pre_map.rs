//! What a Pending window is first configured with (190): the bounds of the output it'll map
//! on, and its size if it'll map maximised or fullscreen, which policy answers before the
//! map as it'll decide them at the map.

use smithay::utils::Size;

use super::*;
use crate::{FrameState, PlacementMode, PreMapConfigure, PreMapWindow, SizeHints};

/// A 1920×1080 output, and a 1280×720 one right of it; window 1 on the second, and window 2
/// on the first, focused; the pointer on the second.
fn two_outputs(placement_mode: PlacementMode) -> Policy<TestWindow> {
    let mut policy = Policy::new(Config {
        placement_mode,
        ..Config::default()
    });
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.add_output(output(2, 1920, 0, 1280, 720));
    policy.map_window(
        TestWindow::new(1, 400, 300).with_requested_position(2100, 100),
        ms(1),
    );
    policy.map_window(
        TestWindow::new(2, 400, 300).with_requested_position(100, 100),
        ms(2),
    );
    policy.set_pointer(Point::new(2500., 400.));
    policy
}

/// A CSD window of `window_type`, transient for `parent`.
fn pending(window_type: WindowType, parent: Option<u64>) -> PreMapWindow {
    PreMapWindow {
        window_type,
        hints: SizeHints::default(),
        parent: parent.map(WindowId),
        ssd: false,
    }
}

fn no_frame(_: FrameState) -> FrameExtents<i32, Logical> {
    FrameExtents::default()
}

#[test]
fn the_bounds_are_those_of_the_output_the_window_will_be_placed_on() {
    use PlacementMode::{Automatic, Manual, Pointer};
    use WindowType::{Dialog, ModalDialog, Normal, Splashscreen};
    let maximized = MapStates {
        maximized: true,
        ..MapStates::default()
    };
    let (first, second) = (Size::new(1920, 1080), Size::new(1280, 720));
    #[rustfmt::skip]
    let cases = [
        // A dialog goes over its parent, on the parent's output, in every mode.
        ("a dialog with a parent", Automatic, Dialog, Some(1), MapStates::default(), second),
        ("a modal dialog with a parent", Automatic, ModalDialog, Some(1), MapStates::default(), second),
        ("a dialog with a parent, pointer", Pointer, Dialog, Some(1), MapStates::default(), second),
        // One with no parent is centred on the focused window's output, in every mode.
        ("a dialog with none", Automatic, Dialog, None, MapStates::default(), first),
        ("a dialog with none, pointer", Pointer, Dialog, None, MapStates::default(), first),
        ("a splash screen, manual", Manual, Splashscreen, None, MapStates::default(), first),
        // The pointer modes put a window on the pointer's output.
        ("pointer", Pointer, Normal, None, MapStates::default(), second),
        ("manual", Manual, Normal, None, MapStates::default(), second),
        // The automatic mode on the focused window's.
        ("automatic", Automatic, Normal, None, MapStates::default(), first),
        // A window that maps maximised keeps the automatic choice (154).
        ("maximised, pointer", Pointer, Normal, None, maximized, first),
        ("maximised, manual", Manual, Normal, None, maximized, first),
    ];
    for (case, mode, window_type, parent, states, bounds) in cases {
        let policy = two_outputs(mode);
        let answer = policy.pre_map_configure(&pending(window_type, parent), &states, no_frame);
        assert_eq!(answer.bounds, Some(bounds), "{case}");
    }
}

#[test]
fn a_window_that_asks_for_fullscreen_is_configured_with_the_whole_output() {
    let mut policy = policy();
    policy.set_outputs(vec![reserving(output(1, 0, 0, 1920, 1080), 0, 0, 28, 0)]);
    let states = MapStates {
        fullscreen: Some(None),
        ..MapStates::default()
    };

    let answer = policy.pre_map_configure(&pending(WindowType::Normal, None), &states, no_frame);

    assert_eq!(
        answer,
        PreMapConfigure {
            bounds: Some(Size::new(1920, 1052)),
            size: Some(Size::new(1920, 1080)),
            maximized: false,
            fullscreen: true,
        }
    );
}

#[test]
fn a_window_that_asks_for_fullscreen_on_an_output_is_configured_with_that_one() {
    let policy = two_outputs(PlacementMode::Automatic);
    let states = MapStates {
        fullscreen: Some(Some(OutputId(2))),
        ..MapStates::default()
    };

    let answer = policy.pre_map_configure(&pending(WindowType::Normal, None), &states, no_frame);

    assert_eq!(answer.size, Some(Size::new(1280, 720)));
}

#[test]
fn an_output_policy_doesnt_have_is_the_one_the_window_is_placed_on() {
    // As at the map, where the output it's on is the one laid out on.
    let policy = two_outputs(PlacementMode::Automatic);
    let states = MapStates {
        fullscreen: Some(Some(OutputId(7))),
        ..MapStates::default()
    };

    let answer = policy.pre_map_configure(&pending(WindowType::Normal, None), &states, no_frame);

    assert_eq!(answer.size, Some(Size::new(1920, 1080)));
}

#[test]
fn a_maximised_window_fills_the_work_area_inside_its_maximised_frame_focused() {
    let mut policy = policy();
    policy.set_outputs(vec![reserving(output(1, 0, 0, 1920, 1080), 0, 0, 28, 0)]);
    let window = PreMapWindow {
        ssd: true,
        ..pending(WindowType::Normal, None)
    };
    let states = MapStates {
        maximized: true,
        ..MapStates::default()
    };
    // A theme whose frame is wider unfocused, and thinner maximised.
    let extents = |state: FrameState| {
        let border = if state.focused { 5 } else { 6 };
        if state.maximized {
            FrameExtents::new(0, 0, 22, 0)
        } else {
            FrameExtents::new(border, border, 25, border)
        }
    };

    let answer = policy.pre_map_configure(&window, &states, extents);

    assert_eq!(
        answer,
        PreMapConfigure {
            bounds: Some(Size::new(1910, 1052 - 30)),
            size: Some(Size::new(1920, 1052 - 22)),
            maximized: true,
            fullscreen: false,
        }
    );
}

#[test]
fn a_window_without_the_function_for_a_state_is_configured_without_it() {
    // A fixed size: no maximise function (177).
    let policy = policy();
    let window = PreMapWindow {
        hints: SizeHints {
            min: (400, 300).into(),
            max: (400, 300).into(),
        },
        ..pending(WindowType::Normal, None)
    };
    let states = MapStates {
        maximized: true,
        ..MapStates::default()
    };
    let answer = policy.pre_map_configure(&window, &states, no_frame);
    assert!(!answer.maximized);
    assert_eq!(answer.size, None);

    // A utility window: no fullscreen function.
    let states = MapStates {
        fullscreen: Some(None),
        ..MapStates::default()
    };
    let answer = policy.pre_map_configure(&pending(WindowType::Utility, None), &states, no_frame);
    assert!(!answer.fullscreen);
    assert_eq!(answer.size, None);
}

#[test]
fn a_frame_is_asked_for_as_a_new_window_has_it_focused() {
    let policy = policy();
    let state = policy.pre_map_frame_state(&pending(WindowType::Normal, None), true);
    assert!(state.focused);
    assert!(state.maximized);
    assert!(state.resizes_horizontally && state.resizes_vertically);
}
