//! The bindings that move the focused window without a grab (160): to a corner, a side or
//! the centre of its output's work area, and to the output beside its own. Each is set in
//! the test, as Marco's defaults are `disabled`, and each test runs on both kinds of window
//! (45): an X client is also told where its window went.

use caminito_test_client::layer_shell::LayerSpec;
use smithay::utils::{Logical, Point};

use super::fixture::{
    ClientId, ClientWindow, Fixture, KEY_A, KEY_D, KEY_DOWN, KEY_E, KEY_F1, KEY_F2, KEY_LEFT,
    KEY_LEFTMETA, KEY_Q, KEY_R, KEY_RIGHT, KEY_T, KEY_UP, KEY_W, Kind, WindowSpec, on_both_kinds,
};
use crate::settings::Value as Setting;
use crate::settings::translate::MARCO_WINDOW_KEYBINDINGS;

/// The thirteen bindings, each on Super with a key of its own.
const KEYS: [(&str, &str, u32); 13] = [
    ("move-to-corner-nw", "q", KEY_Q),
    ("move-to-corner-ne", "w", KEY_W),
    ("move-to-corner-sw", "e", KEY_E),
    ("move-to-corner-se", "r", KEY_R),
    ("move-to-side-n", "t", KEY_T),
    ("move-to-side-s", "a", KEY_A),
    ("move-to-side-e", "d", KEY_D),
    ("move-to-side-w", "F1", KEY_F1),
    ("move-to-center", "F2", KEY_F2),
    ("move-to-monitor-n", "Up", KEY_UP),
    ("move-to-monitor-s", "Down", KEY_DOWN),
    ("move-to-monitor-e", "Right", KEY_RIGHT),
    ("move-to-monitor-w", "Left", KEY_LEFT),
];

/// A fixture with the bindings set as `KEYS` has them, and no output yet.
fn keys_bound() -> Fixture {
    let initial = KEYS
        .iter()
        .map(|(key, name, _)| {
            (
                MARCO_WINDOW_KEYBINDINGS,
                (*key).to_owned(),
                Setting::Str(format!("<Super>{name}")),
            )
        })
        .collect();
    Fixture::with_settings(initial)
}

/// Presses the key `binding` is set to, with Super, and returns where the window's Frame
/// then is. An X client is checked to see its window where policy has it.
fn press(f: &mut Fixture, window: ClientWindow, binding: &str) -> Point<i32, Logical> {
    let (_, _, key) = KEYS
        .iter()
        .find(|(name, _, _)| *name == binding)
        .expect("the binding is one of KEYS");
    f.chord(&[KEY_LEFTMETA, *key]);
    f.sync(window);
    if let ClientWindow::X11(..) = window {
        let id = window.x11();
        let seen = f
            .x11_call(window.client(), move |x| x.position(id))
            .unwrap();
        let at = f.geometry(window).loc;
        assert_eq!(seen, (at.x, at.y), "{binding}: the X window's position");
    }
    f.visible_frame(window).loc
}

/// A client of `kind` with one window, on a 1920 × 1080 output under a panel 28 high, so
/// that the work area is 1920 × 1052 at 0,28.
fn under_a_panel(kind: Kind) -> (Fixture, ClientId, ClientWindow) {
    let mut f = keys_bound();
    f.add_output((1920, 1080), 1.0);
    let client = f.connect(kind);
    let panel = f.connect(Kind::Xdg);
    f.map_layer(panel, LayerSpec::top_bar(28), 0xff20_2020);
    let window = f.map(client, WindowSpec::default());
    (f, client, window)
}

on_both_kinds! {
    fn each_corner_side_and_centre_binding_once_set_moves_the_focused_window_there(kind: Kind) {
        let (mut f, _, window) = under_a_panel(kind);
        let size = f.visible_frame(window).size;
        // Where the Frame's left and top are with its right at 1920 and its bottom at 1080.
        let (right, bottom) = (1920 - size.w, 1080 - size.h);

        assert_eq!(press(&mut f, window, "move-to-corner-nw"), (0, 28).into());
        assert_eq!(press(&mut f, window, "move-to-corner-ne"), (right, 28).into());
        assert_eq!(press(&mut f, window, "move-to-corner-sw"), (0, bottom).into());
        assert_eq!(press(&mut f, window, "move-to-corner-se"), (right, bottom).into());
        let centre = Point::from((right / 2, 28 + (bottom - 28) / 2));
        assert_eq!(press(&mut f, window, "move-to-center"), centre);
        // A side moves it along one axis alone.
        assert_eq!(press(&mut f, window, "move-to-side-n"), (centre.x, 28).into());
        assert_eq!(press(&mut f, window, "move-to-side-e"), (right, 28).into());
        assert_eq!(press(&mut f, window, "move-to-side-s"), (right, bottom).into());
        assert_eq!(press(&mut f, window, "move-to-side-w"), (0, bottom).into());
        // And it keeps its size.
        assert_eq!(f.visible_frame(window).size, size);
    }
}

on_both_kinds! {
    fn each_monitor_binding_once_set_moves_the_focused_window_to_the_output_that_way(kind: Kind) {
        // Three outputs of one size: the second on the right of the first, and the third
        // below it.
        let mut f = keys_bound();
        f.add_output((1920, 1080), 1.0);
        f.add_output((1920, 1080), 1.0);
        let below = f.add_output((1920, 1080), 1.0);
        f.set_output_position(&below, (0, 1080));
        let client = f.connect(kind);
        let window = f.map(client, WindowSpec::default());
        press(&mut f, window, "move-to-corner-nw");
        press(&mut f, window, "move-to-center");
        let start = f.visible_frame(window).loc;
        assert!(start.x < 1920 && start.y < 1080, "it's on the first output");

        // With no output that way it stays.
        assert_eq!(press(&mut f, window, "move-to-monitor-w"), start);
        assert_eq!(press(&mut f, window, "move-to-monitor-n"), start);

        let east = start + Point::from((1920, 0));
        assert_eq!(press(&mut f, window, "move-to-monitor-e"), east);
        assert_eq!(press(&mut f, window, "move-to-monitor-s"), east);
        assert_eq!(press(&mut f, window, "move-to-monitor-w"), start);
        let south = start + Point::from((0, 1080));
        assert_eq!(press(&mut f, window, "move-to-monitor-s"), south);
        assert_eq!(press(&mut f, window, "move-to-monitor-n"), start);
    }
}
