//! Marco's `placement-mode` and `center-new-windows` (154), from their keys to where a new
//! window shows. Policy's tests work the positions out; these check that the keys, the
//! pointer, a click and the keyboard reach it, on both kinds of window.

use caminito_policy::PlacementMode;
use smithay::utils::{Logical, Point};

use super::fixture::{
    BTN_LEFT, ClientId, ClientWindow, Fixture, KEY_ENTER, KEY_ESC, KEY_RIGHT, Kind, WindowSpec,
    on_both_kinds,
};
use crate::settings::Value;
use crate::settings::translate::MARCO_GENERAL;

/// Caminito on one 1920×1080 output with `placement-mode` and `center-new-windows` set,
/// and a client of `kind`.
fn connected_with(kind: Kind, mode: &str, center: bool) -> (Fixture, ClientId) {
    let mut f = Fixture::with_settings(vec![
        (
            MARCO_GENERAL,
            "placement-mode".into(),
            Value::Str(mode.into()),
        ),
        (
            MARCO_GENERAL,
            "center-new-windows".into(),
            Value::Bool(center),
        ),
    ]);
    f.add_output((1920, 1080), 1.0);
    let id = f.connect(kind);
    (f, id)
}

/// The top left corner of a window's visible frame.
fn frame_corner(f: &Fixture, window: ClientWindow) -> Point<i32, Logical> {
    f.visible_frame(window).loc
}

/// Where a window's visible frame goes when it's centred on `(x, y)`.
fn centred_on(f: &Fixture, window: ClientWindow, x: i32, y: i32) -> Point<i32, Logical> {
    let size = f.visible_frame(window).size;
    Point::from((x - size.w / 2, y - size.h / 2))
}

fn placing(f: &Fixture) -> bool {
    f.state
        .caminito
        .policy
        .placing(crate::state::SEAT)
        .is_some()
}

on_both_kinds! {
    fn pointer_placement_puts_a_new_window_on_the_pointer(kind: Kind) {
        let (mut f, id) = connected_with(kind, "pointer", false);
        f.pointer_move(1000., 500.);

        let window = f.map(id, WindowSpec::plain((400, 300)));

        assert_eq!(frame_corner(&f, window), centred_on(&f, window, 1000, 500));
        assert!(!placing(&f));
    }
}

on_both_kinds! {
    fn center_new_windows_puts_a_new_window_in_the_middle_of_the_work_area(kind: Kind) {
        let (mut f, id) = connected_with(kind, "automatic", true);
        f.pointer_move(100., 100.);

        let window = f.map(id, WindowSpec::plain((400, 300)));

        assert_eq!(frame_corner(&f, window), centred_on(&f, window, 960, 540));
    }
}

on_both_kinds! {
    fn a_manual_placement_follows_the_pointer_until_a_click(kind: Kind) {
        let (mut f, id) = connected_with(kind, "manual", false);
        f.pointer_move(1000., 500.);
        let window = f.map(id, WindowSpec::plain((400, 300)));
        let placed = centred_on(&f, window, 1000, 500);
        assert_eq!(frame_corner(&f, window), placed);
        assert!(placing(&f));

        f.pointer_move(1100., 550.);
        assert_eq!(frame_corner(&f, window), placed + Point::from((100, 50)));

        f.pointer_button(BTN_LEFT, true);
        f.pointer_button(BTN_LEFT, false);
        assert!(!placing(&f));
        f.pointer_move(1300., 700.);

        assert_eq!(frame_corner(&f, window), placed + Point::from((100, 50)));
    }
}

on_both_kinds! {
    fn a_manual_placement_is_moved_by_the_arrows_and_put_down_with_return(kind: Kind) {
        let (mut f, id) = connected_with(kind, "manual", false);
        f.pointer_move(1000., 500.);
        let window = f.map(id, WindowSpec::plain((400, 300)));
        let placed = centred_on(&f, window, 1000, 500);

        f.key(KEY_RIGHT, true);
        f.key(KEY_RIGHT, false);
        assert_eq!(frame_corner(&f, window), placed + Point::from((10, 0)));
        assert!(placing(&f));

        f.key(KEY_ENTER, true);
        f.key(KEY_ENTER, false);
        assert!(!placing(&f));
        f.pointer_move(1300., 700.);

        assert_eq!(frame_corner(&f, window), placed + Point::from((10, 0)));
    }
}

on_both_kinds! {
    fn escape_puts_a_manual_placement_back_where_it_began(kind: Kind) {
        let (mut f, id) = connected_with(kind, "manual", false);
        f.pointer_move(1000., 500.);
        let window = f.map(id, WindowSpec::plain((400, 300)));
        let placed = centred_on(&f, window, 1000, 500);
        f.pointer_move(1200., 600.);

        f.key(KEY_ESC, true);
        f.key(KEY_ESC, false);

        assert!(!placing(&f));
        assert_eq!(frame_corner(&f, window), placed);
    }
}

on_both_kinds! {
    fn a_placement_key_changed_applies_to_the_next_window(kind: Kind) {
        let (mut f, id) = connected_with(kind, "automatic", false);
        f.pointer_move(1000., 500.);
        let first = f.map(id, WindowSpec::plain((400, 300)));
        let automatic = frame_corner(&f, first);
        assert_ne!(automatic, centred_on(&f, first, 1000, 500));

        f.set_setting(MARCO_GENERAL, "placement-mode", Value::Str("pointer".into()));
        f.dispatch_until(|f| {
            f.state.caminito.settings.policy.placement_mode == PlacementMode::Pointer
        });
        assert_eq!(frame_corner(&f, first), automatic);
        let second = f.map(id, WindowSpec::plain((400, 300)));

        assert_eq!(frame_corner(&f, second), centred_on(&f, second, 1000, 500));
    }
}
