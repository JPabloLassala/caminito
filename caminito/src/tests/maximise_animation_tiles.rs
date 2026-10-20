//! The Maximise animation for tiles (CMN-175, ADR 0036): a window tiled by a drop at a side
//! or by a tiling key, given another tile's width by the key pressed again, and untiled,
//! goes from its frame as it was to its new one as a maximised window does, read from the
//! debug dump. A drop goes from where the window was let go. A titlebar drag that pulls a
//! window off its maximised or tiled place doesn't animate: the window is under the pointer
//! at once.

use caminito_policy::ResizeEdges;
use smithay::utils::Point;

use super::fixture::{
    BTN_LEFT, ClientId, ClientWindow, Fixture, KEY_LEFTMETA, KEY_RIGHT, KEY_W, Kind, on_both_kinds,
};
use super::maximise_animation::{OUTPUT, asked_size, assert_goes_from, desk, floating, maximizing};
use super::tiling::title_point;
use crate::maximize_animation::LENGTH;
use crate::settings::Value as Setting;
use crate::settings::translate::{MARCO_GENERAL, MARCO_WINDOW_KEYBINDINGS};
use crate::state::SEAT;

/// Presses the window's titlebar and moves the pointer by `dx`,`dy`, past the drag
/// threshold.
fn grab_title_by(f: &mut Fixture, window: ClientWindow, dx: f64, dy: f64) {
    let title = title_point(f, window);
    f.pointer_move(title.x, title.y);
    f.pointer_button(BTN_LEFT, true);
    f.pointer_move(title.x + dx, title.y + dy);
}

/// Drags the window by its titlebar to `x`,`y`, past the drag threshold, and holds it there.
fn drag_title_to(f: &mut Fixture, window: ClientWindow, x: f64, y: f64) {
    grab_title_by(f, window, 20., 20.);
    f.sync(window);
    f.pointer_move(x, y);
    f.sync(window);
}

on_both_kinds! {
    fn a_window_dropped_at_a_side_goes_into_its_tile_from_where_it_was_let_go(kind: Kind) {
        let (mut f, id) = desk(kind, Vec::new());
        let window = floating(&mut f, id);
        drag_title_to(&mut f, window, 0., 300.);
        let let_go = f.visible_frame(window);

        f.pointer_button(BTN_LEFT, false);

        assert!(f.state.caminito.policy.tile(f.pid(window)).is_some(), "tiled");
        assert_goes_from(&mut f, window, let_go);
    }
}

/// `desk`'s output and client, with `tile-to-side-e` on Super+W and `move-to-monitor-e` on
/// Super+Right, and a window of 400×300 away from the output's corner, drawn at rest.
/// Without `cycling`, the tiling key pressed again untiles the window.
fn east_tile_key_bound(kind: Kind, cycling: bool) -> (Fixture, ClientId, ClientWindow) {
    let key = |name: &str, accelerator: &str| {
        let accelerator = Setting::Str(accelerator.into());
        (MARCO_WINDOW_KEYBINDINGS, name.to_owned(), accelerator)
    };
    let settings = vec![
        key("tile-to-side-e", "<Super>w"),
        key("move-to-monitor-e", "<Super>Right"),
        (
            MARCO_GENERAL,
            "allow-tile-cycling".into(),
            Setting::Bool(cycling),
        ),
    ];
    let (mut f, id) = desk(kind, settings);
    let window = floating(&mut f, id);
    (f, id, window)
}

/// Has the window's client draw at the size it was asked for, and ends its animation.
fn at_rest(f: &mut Fixture, window: ClientWindow) {
    let size = asked_size(f, window);
    f.draw_at_asked_size(window, size);
    f.advance_clock(LENGTH);
}

on_both_kinds! {
    fn a_window_tiled_by_its_key_goes_into_its_tile(kind: Kind) {
        let (mut f, _, window) = east_tile_key_bound(kind, true);
        let from = f.visible_frame(window);

        f.chord(&[KEY_LEFTMETA, KEY_W]);

        assert!(f.state.caminito.policy.tile(f.pid(window)).is_some(), "tiled");
        assert_goes_from(&mut f, window, from);
    }
}

on_both_kinds! {
    fn a_tile_given_another_width_by_its_key_goes_to_it(kind: Kind) {
        let (mut f, _, window) = east_tile_key_bound(kind, true);
        f.chord(&[KEY_LEFTMETA, KEY_W]);
        at_rest(&mut f, window);
        let half = f.visible_frame(window);

        // A third of the width, on the same side.
        f.chord(&[KEY_LEFTMETA, KEY_W]);

        assert_goes_from(&mut f, window, half);
        assert_eq!(f.visible_frame(window).size.w, half.size.w * 2 / 3);
    }
}

on_both_kinds! {
    fn a_window_untiled_by_its_key_goes_back_to_where_it_was(kind: Kind) {
        let (mut f, _, window) = east_tile_key_bound(kind, false);
        let floating_frame = f.visible_frame(window);
        f.chord(&[KEY_LEFTMETA, KEY_W]);
        at_rest(&mut f, window);
        let tiled = f.visible_frame(window);

        f.chord(&[KEY_LEFTMETA, KEY_W]);

        assert!(f.state.caminito.policy.tile(f.pid(window)).is_none(), "untiled");
        assert_goes_from(&mut f, window, tiled);
        assert_eq!(f.visible_frame(window), floating_frame);
    }
}

on_both_kinds! {
    fn a_window_dropped_at_the_top_is_maximised_from_where_it_was_let_go(kind: Kind) {
        let (mut f, id) = desk(kind, Vec::new());
        let window = floating(&mut f, id);
        drag_title_to(&mut f, window, 400., 0.);
        let let_go = f.visible_frame(window);

        f.pointer_button(BTN_LEFT, false);

        assert!(f.state.caminito.policy.is_maximized(f.pid(window)), "maximised");
        assert_goes_from(&mut f, window, let_go);
    }
}

on_both_kinds! {
    fn a_tiled_window_maximised_goes_from_its_tile(kind: Kind) {
        let (mut f, _, window) = east_tile_key_bound(kind, true);
        f.chord(&[KEY_LEFTMETA, KEY_W]);
        at_rest(&mut f, window);
        let tiled = f.visible_frame(window);

        f.state.caminito.policy.set_maximized(f.pid(window), true);
        f.dispatch();

        assert_goes_from(&mut f, window, tiled);
    }
}

/// Drags `window`, at rest, by its titlebar past the drag threshold, which pulls it off its
/// place, and checks that it's under the pointer at once, with no animation.
#[track_caller]
fn assert_pulled_off_at_once(f: &mut Fixture, window: ClientWindow) {
    // Read in the pass that pulls it off, which draws a frame.
    grab_title_by(f, window, 50., 100.);

    let pid = f.pid(window);
    let policy = &f.state.caminito.policy;
    assert!(
        !policy.is_maximized(pid) && policy.tile(pid).is_none(),
        "pulled off"
    );
    let none = maximizing(f);
    assert!(none.is_empty(), "{none:?}");
    let size = asked_size(f, window);
    f.draw_at_asked_size(window, size);
    let none = maximizing(f);
    assert!(none.is_empty(), "drawn: {none:?}");
    let pointer = f
        .state
        .caminito
        .seat
        .get_pointer()
        .unwrap()
        .current_location();
    assert!(
        f.visible_frame(window).to_f64().contains(pointer),
        "under the pointer: {:?}, {pointer:?}",
        f.visible_frame(window)
    );
    f.pointer_button(BTN_LEFT, false);
    let none = maximizing(f);
    assert!(none.is_empty(), "let go: {none:?}");
}

on_both_kinds! {
    fn a_maximised_window_dragged_off_is_under_the_pointer_at_once(kind: Kind) {
        let (mut f, id) = desk(kind, Vec::new());
        let window = floating(&mut f, id);
        f.state.caminito.policy.set_maximized(f.pid(window), true);
        f.dispatch();
        at_rest(&mut f, window);

        assert_pulled_off_at_once(&mut f, window);
    }
}

on_both_kinds! {
    fn a_tiled_window_dragged_off_is_under_the_pointer_at_once(kind: Kind) {
        let (mut f, _, window) = east_tile_key_bound(kind, true);
        f.chord(&[KEY_LEFTMETA, KEY_W]);
        at_rest(&mut f, window);

        assert_pulled_off_at_once(&mut f, window);
    }
}

on_both_kinds! {
    fn a_tile_resized_by_its_inner_side_and_given_its_width_again_by_its_key_goes_to_it(
        kind: Kind
    ) {
        let (mut f, _, window) = east_tile_key_bound(kind, true);
        // Tiled by a drop, at half the width before any press of the key.
        drag_title_to(&mut f, window, 799., 300.);
        f.pointer_button(BTN_LEFT, false);
        at_rest(&mut f, window);
        let pid = f.pid(window);
        let half = f.visible_frame(window);
        // Policy's resize by the tile's inner side, which a drag of its left edge ends in.
        let from = half.loc.to_f64();
        let policy = &mut f.state.caminito.policy;
        let left = ResizeEdges { left: true, ..ResizeEdges::default() };
        assert!(policy.begin_resize(SEAT, pid, left, from));
        policy.grab_motion(SEAT, from - Point::from((100., 0.)), false);
        policy.end_grab(SEAT);
        f.dispatch();
        at_rest(&mut f, window);
        let resized = f.visible_frame(window);
        assert_eq!(resized.size.w, half.size.w + 100, "resized");

        // Half the width again.
        f.chord(&[KEY_LEFTMETA, KEY_W]);

        assert_goes_from(&mut f, window, resized);
        assert_eq!(f.visible_frame(window), half);
    }
}

on_both_kinds! {
    fn a_tiled_window_moved_to_another_output_goes_there_at_once(kind: Kind) {
        let (mut f, _, window) = east_tile_key_bound(kind, true);
        let left = f.state.caminito.outputs[0].clone();
        let right = f.add_output(OUTPUT, 1.0);
        f.set_output_position(&left, (0, 0));
        f.set_output_position(&right, (OUTPUT.0, 0));
        f.chord(&[KEY_LEFTMETA, KEY_W]);
        at_rest(&mut f, window);
        let tile = f.state.caminito.policy.tile(f.pid(window));

        f.chord(&[KEY_LEFTMETA, KEY_RIGHT]);

        assert_eq!(f.state.caminito.policy.tile(f.pid(window)), tile, "the same tile");
        assert!(f.visible_frame(window).loc.x >= OUTPUT.0, "on the right output");
        let none = maximizing(&f);
        assert!(none.is_empty(), "{none:?}");
    }
}
