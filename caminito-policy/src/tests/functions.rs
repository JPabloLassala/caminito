//! A window's functions (177): the table Marco's `recalc_window_features` gives, and the
//! setters that refuse a state the window has no function for.

use super::*;

use crate::{FunctionFacts, Functions, SizeHints, functions};

fn facts(window_type: WindowType) -> FunctionFacts {
    FunctionFacts {
        window_type,
        hints: SizeHints::default(),
        has_parent: false,
        ssd: true,
        fullscreen: false,
    }
}

fn fixed() -> SizeHints {
    SizeHints {
        min: (300, 200).into(),
        max: (300, 200).into(),
    }
}

/// The functions as a word of letters, one for each the window has: mi(n)imise,
/// ma(x)imise, resize (h)orizontally and (v)ertically, (f)ullscreen, (s)hade and (b)order
/// only.
fn letters(functions: Functions) -> String {
    [
        (functions.minimize, 'n'),
        (functions.maximize, 'x'),
        (functions.resize_horizontally, 'h'),
        (functions.resize_vertically, 'v'),
        (functions.fullscreen, 'f'),
        (functions.shade, 's'),
        (functions.border_only, 'b'),
    ]
    .into_iter()
    .filter_map(|(has, letter)| has.then_some(letter))
    .collect()
}

#[test]
fn each_window_type_has_the_functions_marco_gives_it() {
    // An SSD window that can resize, then one whose size hints fix its size.
    for (window_type, resizable, fixed_size) in [
        (WindowType::Normal, "nxhvfs", "nfs"),
        (WindowType::Dialog, "nxhvfs", "nfs"),
        (WindowType::ModalDialog, "hvs", "s"),
        (WindowType::Utility, "hvs", "s"),
        (WindowType::Menu, "hvs", "s"),
        // Marco never frames a toolbar, so it has no shade function.
        (WindowType::Toolbar, "hv", ""),
        (WindowType::Desktop, "", ""),
        (WindowType::Dock, "", ""),
        (WindowType::Splashscreen, "", ""),
    ] {
        assert_eq!(
            letters(functions(&facts(window_type), false)),
            resizable,
            "{window_type:?}"
        );
        let facts = FunctionFacts {
            hints: fixed(),
            ..facts(window_type)
        };
        assert_eq!(
            letters(functions(&facts, false)),
            fixed_size,
            "a fixed-size {window_type:?}"
        );
    }
}

#[test]
fn a_window_that_resizes_one_way_has_that_way_alone_and_still_maximises() {
    let facts = FunctionFacts {
        hints: SizeHints {
            min: (300, 200).into(),
            // A zero maximum is unbounded.
            max: (300, 0).into(),
        },
        ..facts(WindowType::Normal)
    };
    assert_eq!(letters(functions(&facts, false)), "nxvfs");
}

#[test]
fn a_fullscreen_window_only_minimises_and_leaves_fullscreen() {
    for window_type in [WindowType::Normal, WindowType::Dialog] {
        let facts = FunctionFacts {
            fullscreen: true,
            ..facts(window_type)
        };
        assert_eq!(letters(functions(&facts, false)), "nf");
    }
}

#[test]
fn a_csd_window_has_no_shade_function() {
    let facts = FunctionFacts {
        ssd: false,
        ..facts(WindowType::Normal)
    };
    assert_eq!(letters(functions(&facts, false)), "nxhvf");
}

#[test]
fn a_modal_dialog_has_a_border_only_with_a_parent_and_attach_modal_dialogs_on() {
    let modal = facts(WindowType::ModalDialog);
    let with_parent = FunctionFacts {
        has_parent: true,
        ..modal
    };
    assert_eq!(letters(functions(&modal, true)), "hvs");
    assert_eq!(letters(functions(&with_parent, false)), "hvs");
    // A border alone: no shade or resize function, and so no maximise one.
    assert_eq!(letters(functions(&with_parent, true)), "b");

    // The key attaches no other type.
    let dialog = FunctionFacts {
        has_parent: true,
        ..facts(WindowType::Dialog)
    };
    assert_eq!(letters(functions(&dialog, true)), "nxhvfs");
}

#[test]
fn policy_gives_a_managed_window_its_functions_from_its_own_state() {
    let mut policy = Policy::new(Config {
        attach_modal_dialogs: true,
        ..Config::default()
    });
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.map_window(TestWindow::new(1, 400, 300).with_frame(4, 4, 24, 4), ms(1));
    policy.map_window(
        TestWindow::new(2, 300, 200)
            .with_frame(4, 4, 24, 4)
            .with_type(WindowType::ModalDialog)
            .with_parent(1),
        ms(2),
    );

    assert_eq!(letters(policy.functions(id(1))), "nxhvfs");
    assert_eq!(letters(policy.functions(id(2))), "b");
    // A window policy doesn't manage has none.
    assert_eq!(letters(policy.functions(id(3))), "");

    policy.set_fullscreen(id(1), true, None);
    assert_eq!(letters(policy.functions(id(1))), "nf");

    // The key is read again when the settings change.
    policy.settings_changed(Config::default());
    assert_eq!(letters(policy.functions(id(2))), "hvs");
}

#[test]
fn policy_sends_a_window_its_functions_when_they_change() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300).with_frame(4, 4, 24, 4), ms(1));
    assert_eq!(letters(window(&policy, 1).functions), "nxhvfs");

    policy.window_mut(id(1)).unwrap().hints = fixed();
    policy.commit(id(1));
    assert_eq!(letters(window(&policy, 1).functions), "nfs");
}

#[test]
fn the_frame_extents_are_asked_for_with_border_only_and_the_ways_the_window_resizes() {
    let mut policy = Policy::new(Config {
        attach_modal_dialogs: true,
        ..Config::default()
    });
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.map_window(TestWindow::new(1, 400, 300).with_frame(4, 4, 24, 4), ms(1));
    let mut modal = TestWindow::new(2, 300, 200)
        .with_frame(4, 4, 24, 4)
        .with_type(WindowType::ModalDialog)
        .with_parent(1);
    modal.border_only_extents = Some(FrameExtents::new(2, 2, 2, 2));
    policy.map_window(modal, ms(2));

    assert_eq!(policy.frame_extents(id(1)), FrameExtents::new(4, 4, 24, 4));
    assert_eq!(policy.frame_extents(id(2)), FrameExtents::new(2, 2, 2, 2));

    // A window that can't resize is asked for with neither way, and stays so while it's
    // fullscreen, which is no state a frame is asked for in.
    let mut fixed_size = TestWindow::new(3, 300, 200)
        .with_frame(4, 4, 24, 4)
        .with_hints((300, 200), (300, 200));
    fixed_size.unresizable_extents = Some(FrameExtents::new(1, 1, 24, 1));
    policy.map_window(fixed_size, ms(3));
    assert_eq!(policy.frame_extents(id(3)), FrameExtents::new(1, 1, 24, 1));
}

/// A normal window whose size hints fix its size, focused.
fn fixed_size_window(policy: &mut Policy<TestWindow>) {
    policy.map_window(
        TestWindow::new(1, 400, 300)
            .with_frame(4, 4, 24, 4)
            .with_hints((400, 300), (400, 300)),
        ms(1),
    );
}

#[test]
fn set_maximized_refuses_a_window_with_no_maximise_function() {
    let mut policy = policy();
    fixed_size_window(&mut policy);

    policy.set_maximized(id(1), true);
    policy.set_maximized_horizontally(id(1), true);
    policy.set_maximized_vertically(id(1), true);

    assert!(!policy.is_maximized_horizontally(id(1)));
    assert!(!policy.is_maximized_vertically(id(1)));
}

#[test]
fn maximising_one_way_needs_only_the_resize_function() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_type(WindowType::Utility),
        ms(1),
    );

    // Marco's `handle_maximize_vertically` and `_horizontally` ask only that the window
    // resizes (keybindings.c:2529-2559).
    policy.set_maximized_vertically(id(1), true);
    assert!(policy.is_maximized_vertically(id(1)));
    policy.set_maximized(id(1), true);
    assert!(!policy.is_maximized_horizontally(id(1)));
}

#[test]
fn a_window_that_loses_the_maximise_function_can_still_unmaximise() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.set_maximized(id(1), true);
    policy.window_mut(id(1)).unwrap().hints = fixed();
    policy.commit(id(1));
    assert!(policy.is_maximized(id(1)));

    policy.set_maximized_horizontally(id(1), false);
    assert!(!policy.is_maximized_horizontally(id(1)));
    assert!(policy.is_maximized_vertically(id(1)));
    policy.set_maximized(id(1), false);
    assert!(!policy.is_maximized_vertically(id(1)));
}

#[test]
fn set_fullscreen_refuses_a_window_with_no_fullscreen_function_and_lets_any_leave() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_type(WindowType::Utility),
        ms(1),
    );
    policy.set_fullscreen(id(1), true, None);
    assert!(!policy.is_fullscreen(id(1)));

    // A window that became a utility window while fullscreen leaves it.
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.set_fullscreen(id(2), true, None);
    policy.window_mut(id(2)).unwrap().window_type = WindowType::Utility;
    policy.commit(id(2));
    assert!(policy.is_fullscreen(id(2)));
    policy.set_fullscreen(id(2), false, None);
    assert!(!policy.is_fullscreen(id(2)));
}

#[test]
fn a_fixed_size_normal_window_is_made_fullscreen() {
    // Not as in Marco, which refuses one unless its least size is the screen's (177).
    let mut policy = policy();
    fixed_size_window(&mut policy);

    policy.set_fullscreen(id(1), true, None);

    assert!(policy.is_fullscreen(id(1)));
}

#[test]
fn set_shaded_refuses_a_window_with_no_shade_function_and_lets_any_unshade() {
    let mut policy = Policy::new(Config {
        attach_modal_dialogs: true,
        ..Config::default()
    });
    policy.add_output(output(1, 0, 0, 1920, 1080));
    policy.map_window(TestWindow::new(1, 400, 300).with_frame(4, 4, 24, 4), ms(1));
    // A modal dialog with no parent yet shades.
    policy.map_window(
        TestWindow::new(2, 300, 200)
            .with_frame(4, 4, 24, 4)
            .with_type(WindowType::ModalDialog),
        ms(2),
    );
    policy.set_shaded(id(2), true);
    assert!(policy.is_shaded(id(2)));

    // Attached, it has a border only, and it can still unshade.
    policy.window_mut(id(2)).unwrap().parent = Some(id(1));
    policy.commit(id(2));
    assert!(policy.is_shaded(id(2)));
    policy.set_shaded(id(2), false);
    assert!(!policy.is_shaded(id(2)));
    policy.set_shaded(id(2), true);
    assert!(!policy.is_shaded(id(2)));

    // Nor does a fullscreen window shade.
    policy.set_fullscreen(id(1), true, None);
    policy.set_shaded(id(1), true);
    assert!(!policy.is_shaded(id(1)));
}

#[test]
fn minimize_refuses_a_window_with_no_minimise_function() {
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 400, 300).with_type(WindowType::ModalDialog),
        ms(1),
    );
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));

    policy.minimize(id(1));
    policy.minimize(id(2));

    assert!(!policy.is_minimized(id(1)));
    assert!(policy.is_minimized(id(2)));
}

#[test]
fn a_window_maps_without_a_state_it_asks_for_and_has_no_function_for() {
    let mut policy = policy();
    let maximized = MapStates {
        maximized: true,
        ..MapStates::default()
    };
    policy.map_window_with(
        TestWindow::new(1, 400, 300).with_hints((400, 300), (400, 300)),
        maximized,
        ms(1),
    );
    assert!(!policy.is_maximized_horizontally(id(1)));
    assert!(!policy.is_maximized_vertically(id(1)));

    let fullscreen = MapStates {
        fullscreen: Some(None),
        ..MapStates::default()
    };
    policy.map_window_with(
        TestWindow::new(2, 400, 300).with_type(WindowType::Utility),
        fullscreen,
        ms(2),
    );
    assert!(!policy.is_fullscreen(id(2)));
}

#[test]
fn a_framed_window_too_big_for_its_work_area_isnt_maximised_without_the_function() {
    // Marco's placement maximises one only if it has the function (place.c:913-918).
    let mut policy = policy();
    policy.map_window(
        TestWindow::new(1, 1920, 1080)
            .with_frame(4, 4, 24, 4)
            .with_hints((1920, 1080), (1920, 1080)),
        ms(1),
    );

    assert!(!policy.is_maximized_horizontally(id(1)));
    assert!(!policy.is_maximized_vertically(id(1)));
}
