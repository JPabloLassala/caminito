//! The Overview's transition (243): it opens and closes with each window moving between
//! its real place and its place in the layout for 250 ms of Caminito's clock, while the
//! minimised windows, the dimming, the panel and caja's desktop fade, read from the debug
//! dump and from the output's pixels. Policy has it open or closed from the first frame.
//!
//! Each test stops Caminito's clock, so the transition is where `advance_clock` takes it.

use std::time::Duration;

use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{ClientId, ClientWindow, Fixture, Kind, on_both_kinds};
use super::overview::{
    DESKTOP, OUTPUT, PANEL, WALLPAPER, WINDOW, click, desktop_spot, focus, is_dimmed, is_open,
    map_the_desktop, map_three, middle, minimise, open, overview, panel_spot, pixel, place,
    press_shortcut, rect, stacking, wallpaper_spot,
};
use crate::scene::SceneLayer;
use crate::settings::Value;
use crate::settings::translate::{CAMINITO, INTERFACE, MARCO_GENERAL};
use crate::workspace_slide::LENGTH;

/// The transition's middle, where the ease-out has the drawing seven eighths of the way.
const MIDDLE: Duration = Duration::from_millis(125);
const MIDDLE_PART: f64 = 0.875;

type Settings = Vec<(&'static str, String, Value)>;

/// One output with `overview-animation` on, unless `settings` has it, and a client of
/// `kind`, with Caminito's clock stopped. The fixture has the key off, for the tests
/// written when the Overview opened at once.
fn desk(kind: Kind, mut settings: Settings) -> (Fixture, ClientId) {
    if !settings
        .iter()
        .any(|(_, key, _)| key == "overview-animation")
    {
        settings.push((CAMINITO, "overview-animation".into(), Value::Bool(true)));
    }
    let mut f = Fixture::with_settings(settings);
    f.add_output(OUTPUT, 1.0);
    let id = f.connect(kind);
    f.stop_clock();
    (f, id)
}

/// The debug dump's `overview_transition` member: `null` while none runs.
fn transition(f: &Fixture) -> serde_json::Value {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    dump["overview_transition"].clone()
}

#[track_caller]
fn direction(f: &Fixture) -> String {
    let transition = transition(f);
    let direction = transition["direction"].as_str();
    direction.expect("a transition runs").to_owned()
}

#[track_caller]
fn part(f: &Fixture) -> f64 {
    transition(f)["part"].as_f64().expect("a transition runs")
}

/// The windows the transition draws, bottom to top, by policy's ids.
fn drawn_order(f: &Fixture) -> Vec<u64> {
    let windows = transition(f)["windows"].as_array().unwrap().clone();
    windows
        .iter()
        .map(|w| w["window"].as_u64().unwrap())
        .collect()
}

/// Where the transition draws `window`, and how opaque.
#[track_caller]
fn drawn(f: &Fixture, window: ClientWindow) -> (Rectangle<i32, Logical>, f64) {
    let id = f.pid(window).0;
    let windows = transition(f)["windows"].as_array().unwrap().clone();
    let drawn = windows.iter().find(|drawn| drawn["window"] == id);
    let drawn = drawn.expect("the transition draws the window");
    (rect(&drawn["rect"]), drawn["alpha"].as_f64().unwrap())
}

#[track_caller]
fn drawn_rect(f: &Fixture, window: ClientWindow) -> Rectangle<i32, Logical> {
    drawn(f, window).0
}

/// The window's real place, its frame as policy has it.
fn real(f: &Fixture, window: ClientWindow) -> Rectangle<i32, Logical> {
    f.state.caminito.policy.frame_rect(f.pid(window)).unwrap()
}

/// Whether `drawn` is strictly between `from` and `to`, at a place that's neither.
fn is_between(
    drawn: Rectangle<i32, Logical>,
    from: Rectangle<i32, Logical>,
    to: Rectangle<i32, Logical>,
) -> bool {
    let within = |value: i32, a: i32, b: i32| a.min(b) <= value && value <= a.max(b);
    within(drawn.loc.x, from.loc.x, to.loc.x)
        && within(drawn.loc.y, from.loc.y, to.loc.y)
        && within(drawn.size.w, from.size.w, to.size.w)
        && within(drawn.size.h, from.size.h, to.size.h)
        && drawn.loc != from.loc
        && drawn.loc != to.loc
}

fn overview_is_drawn(f: &mut Fixture) -> bool {
    let output = f.state.caminito.outputs[0].clone();
    f.state
        .scene_layers(&output)
        .contains(&SceneLayer::Overview)
}

/// Opens the Overview and lets its transition finish.
fn open_at_rest(f: &mut Fixture) {
    open(f);
    f.advance_clock(LENGTH);
    assert!(transition(f).is_null(), "it's open at rest");
}

// Opening and closing.

on_both_kinds! {
    fn opening_moves_each_window_from_its_real_place_to_the_layout(kind: Kind) {
        let (mut f, id) = desk(kind, Vec::new());
        let windows = map_three(&mut f, id);
        let reals = windows.map(|window| real(&f, window));

        press_shortcut(&mut f);

        // Policy has it open from the first frame, which has every window where it is.
        assert!(is_open(&f));
        assert_eq!(direction(&f), "opening");
        assert_eq!(part(&f), 0.0);
        let places = windows.map(|window| place(&f, window));
        for (window, (real, place)) in windows.into_iter().zip(reals.into_iter().zip(places)) {
            assert_ne!(real.loc, place.loc, "{window:?} moves");
            assert_eq!(drawn_rect(&f, window), real, "{window:?}");
        }

        f.advance_clock(MIDDLE);

        assert_eq!(part(&f), MIDDLE_PART);
        for (window, (real, place)) in windows.into_iter().zip(reals.into_iter().zip(places)) {
            let at = drawn_rect(&f, window);
            assert!(is_between(at, real, place), "{window:?}: {at:?}");
            assert_eq!(pixel(&mut f, middle(at)), WINDOW, "{window:?} is drawn there");
        }

        f.advance_clock(LENGTH - MIDDLE);

        assert!(transition(&f).is_null(), "it's over");
        assert!(is_open(&f));
        for (window, place) in windows.into_iter().zip(places) {
            assert_eq!(pixel(&mut f, middle(place)), WINDOW, "{window:?}");
        }
    }
}

on_both_kinds! {
    fn closing_moves_each_window_back_to_its_real_place(kind: Kind) {
        let (mut f, id) = desk(kind, Vec::new());
        let windows = map_three(&mut f, id);
        let reals = windows.map(|window| real(&f, window));
        open_at_rest(&mut f);
        let places = windows.map(|window| place(&f, window));

        press_shortcut(&mut f);

        // Policy has it closed from the first frame, which has every window in the layout.
        assert!(!is_open(&f));
        assert_eq!(direction(&f), "closing");
        assert_eq!(part(&f), 1.0);
        for (window, place) in windows.into_iter().zip(places) {
            assert_eq!(drawn_rect(&f, window), place, "{window:?}");
        }

        f.advance_clock(MIDDLE);

        assert_eq!(part(&f), 1.0 - MIDDLE_PART);
        for (window, (real, place)) in windows.into_iter().zip(reals.into_iter().zip(places)) {
            let at = drawn_rect(&f, window);
            assert!(is_between(at, place, real), "{window:?}: {at:?}");
        }

        f.advance_clock(LENGTH - MIDDLE);

        assert!(transition(&f).is_null(), "it's over");
        assert!(!overview_is_drawn(&mut f));
        for (window, real) in windows.into_iter().zip(reals) {
            assert_eq!(pixel(&mut f, middle(real)), WINDOW, "{window:?}");
        }
    }
}

// The fades.

/// The channels of `pixel`, `0xRRGGBB`.
fn channels(pixel: u32) -> [u32; 3] {
    [16, 8, 0].map(|shift| (pixel >> shift) & 0xff)
}

/// Whether `pixel` is part-way between `from` and `to` in each channel that differs, and
/// neither.
fn is_part_way(pixel: u32, from: u32, to: u32) -> bool {
    let (pixel, from, to) = (channels(pixel), channels(from), channels(to));
    let within = (0..3).all(|c| from[c].min(to[c]) <= pixel[c] && pixel[c] <= from[c].max(to[c]));
    within && pixel != from && pixel != to
}

#[test]
fn the_minimised_windows_the_dimming_the_panel_and_cajas_desktop_fade() {
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    map_the_desktop(&mut f);
    let [hidden, ..] = map_three(&mut f, id);
    minimise(&mut f, hidden);
    let rgb = |argb: u32| argb & 0xff_ffff;
    let dimmed = |color: u32| {
        let [r, g, b] = channels(color).map(|c| c / 2);
        (r << 16) | (g << 8) | b
    };

    press_shortcut(&mut f);
    f.advance_clock(MIDDLE);

    let dump = transition(&f);
    let at_middle = MIDDLE_PART as f32;
    assert_eq!(dump["dim"].as_f64().unwrap() as f32, 0.5 * at_middle);
    assert_eq!(
        dump["layer_surfaces"].as_f64().unwrap() as f32,
        1.0 - at_middle
    );
    let place = place(&f, hidden);
    // A minimised window stays at its place in the layout, and fades in there.
    let (rect, alpha) = drawn(&f, hidden);
    assert_eq!(rect, place);
    assert_eq!(alpha as f32, at_middle);
    // On the pixels: none of them is as it was, nor as it is once open.
    let wallpaper = pixel(&mut f, wallpaper_spot());
    let whole = rgb(WALLPAPER);
    assert!(
        is_part_way(wallpaper, whole, dimmed(whole)),
        "{wallpaper:06x}"
    );
    for (spot, color) in [(desktop_spot(), DESKTOP), (panel_spot(), PANEL)] {
        let drawn = pixel(&mut f, spot);
        assert_ne!(drawn, rgb(color), "{spot:?}");
        assert!(!is_dimmed(drawn, whole), "{spot:?} is {drawn:06x}");
    }
    let window = pixel(&mut f, middle(place));
    assert_ne!(window, WINDOW);

    // And out again as it closes.
    f.advance_clock(LENGTH - MIDDLE);
    press_shortcut(&mut f);
    f.advance_clock(MIDDLE);

    let dump = transition(&f);
    let at_middle = (1.0 - MIDDLE_PART) as f32;
    assert_eq!(dump["dim"].as_f64().unwrap() as f32, 0.5 * at_middle);
    assert_eq!(
        dump["layer_surfaces"].as_f64().unwrap() as f32,
        1.0 - at_middle
    );
    assert_eq!(drawn(&f, hidden), (place, f64::from(at_middle)));
    f.advance_clock(LENGTH - MIDDLE);
    assert_eq!(pixel(&mut f, desktop_spot()), rgb(DESKTOP));
    assert_eq!(pixel(&mut f, panel_spot()), rgb(PANEL));
    assert_eq!(pixel(&mut f, wallpaper_spot()), whole);
}

// Reversed part-way.

on_both_kinds! {
    fn the_shortcut_part_way_through_the_opening_returns_each_window_from_where_it_is(kind: Kind) {
        let (mut f, id) = desk(kind, Vec::new());
        let windows = map_three(&mut f, id);
        let reals = windows.map(|window| real(&f, window));
        press_shortcut(&mut f);
        f.advance_clock(MIDDLE);
        let before = windows.map(|window| drawn_rect(&f, window));
        let part_way = part(&f);

        press_shortcut(&mut f);

        assert!(!is_open(&f));
        assert_eq!(direction(&f), "closing");
        assert_eq!(part(&f), part_way, "no jump");
        assert_eq!(windows.map(|window| drawn_rect(&f, window)), before);

        f.advance_clock(MIDDLE);
        for (window, (real, before)) in windows.into_iter().zip(reals.into_iter().zip(before)) {
            let at = drawn_rect(&f, window);
            assert!(is_between(at, before, real), "{window:?}: {at:?}");
        }
        f.advance_clock(LENGTH - MIDDLE);
        assert!(transition(&f).is_null());
        assert!(!overview_is_drawn(&mut f));
    }
}

#[test]
fn the_shortcut_part_way_through_the_closing_opens_it_again_from_where_it_is() {
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    let windows = map_three(&mut f, id);
    open_at_rest(&mut f);
    press_shortcut(&mut f);
    f.advance_clock(MIDDLE);
    let before = windows.map(|window| drawn_rect(&f, window));
    let part_way = part(&f);

    press_shortcut(&mut f);

    assert!(is_open(&f));
    assert_eq!(direction(&f), "opening");
    assert_eq!(part(&f), part_way, "no jump");
    assert_eq!(windows.map(|window| drawn_rect(&f, window)), before);
    f.advance_clock(LENGTH);
    assert!(transition(&f).is_null());
    assert!(is_open(&f));
}

// A choice.

on_both_kinds! {
    fn a_chosen_window_is_at_the_top_from_the_closings_first_frame(kind: Kind) {
        let (mut f, id) = desk(kind, Vec::new());
        let [first, _, last] = map_three(&mut f, id);
        open_at_rest(&mut f);
        let top = f.pid(last).0;
        assert_eq!(stacking(&f).last(), Some(&top));

        let at = middle(place(&f, first));
        click(&mut f, at);

        assert!(!is_open(&f));
        assert_eq!(direction(&f), "closing");
        let chosen = f.pid(first).0;
        assert_eq!(drawn_order(&f).last(), Some(&chosen));
        // It's over the others on the pixels too, where it's drawn: away from the pointer,
        // whose cursor is at the middle.
        let at = middle(drawn_rect(&f, first)) - Point::from((40, 40));
        assert_eq!(pixel(&mut f, at), WINDOW);
    }
}

#[test]
fn a_chosen_minimised_window_moves_back_and_the_others_fade_out() {
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    let [chosen, other, _] = map_three(&mut f, id);
    minimise(&mut f, chosen);
    minimise(&mut f, other);
    open_at_rest(&mut f);
    let (chosen_place, other_place) = (place(&f, chosen), place(&f, other));

    click(&mut f, middle(chosen_place));
    f.advance_clock(MIDDLE);

    let (at, alpha) = drawn(&f, chosen);
    assert_eq!(alpha, 1.0);
    assert!(is_between(at, chosen_place, real(&f, chosen)), "{at:?}");
    let (at, alpha) = drawn(&f, other);
    assert_eq!(at, other_place);
    assert_eq!(alpha as f32, (1.0 - MIDDLE_PART) as f32);
}

// Policy, from the first frame.

on_both_kinds! {
    fn a_click_part_way_through_the_opening_reaches_the_window_at_its_place_in_the_layout(
        kind: Kind
    ) {
        let (mut f, id) = desk(kind, Vec::new());
        let [first, _, last] = map_three(&mut f, id);
        assert_eq!(focus(&f), Some(f.pid(last).0));
        press_shortcut(&mut f);
        f.advance_clock(Duration::from_millis(20));
        let target = place(&f, first);
        // Where the drawing has it isn't there yet.
        let at = middle(target);
        assert!(!drawn_rect(&f, first).contains(at), "{:?}", drawn_rect(&f, first));

        click(&mut f, at);

        assert!(!is_open(&f));
        assert_eq!(focus(&f), Some(f.pid(first).0));
    }
}

#[test]
fn a_click_part_way_through_the_closing_reaches_the_window_at_its_real_place() {
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    let [first, _, last] = map_three(&mut f, id);
    open_at_rest(&mut f);
    press_shortcut(&mut f);
    f.advance_clock(Duration::from_millis(20));
    let real = real(&f, first);
    let at = middle(real);
    assert!(
        !drawn_rect(&f, first).contains(at),
        "it's drawn elsewhere yet"
    );
    assert_eq!(focus(&f), Some(f.pid(last).0));

    click(&mut f, at);

    assert!(!is_open(&f));
    assert_eq!(focus(&f), Some(f.pid(first).0));
}

// With no transition.

/// Opens and closes the Overview, and checks that each is drawn in one frame.
#[track_caller]
fn assert_at_once(mut f: Fixture, id: ClientId) {
    let windows = map_three(&mut f, id);

    open(&mut f);

    assert!(transition(&f).is_null(), "nothing runs");
    for window in windows {
        let at = middle(place(&f, window));
        assert_eq!(pixel(&mut f, at), WINDOW);
    }

    press_shortcut(&mut f);

    assert!(transition(&f).is_null(), "nothing runs");
    assert!(!overview_is_drawn(&mut f));
    for window in windows {
        let at = middle(real(&f, window));
        assert_eq!(pixel(&mut f, at), WINDOW);
    }
}

#[test]
fn with_its_key_or_one_of_the_three_others_against_it_it_opens_and_closes_at_once() {
    for (schema, key, value) in [
        (CAMINITO, "overview-animation", false),
        (MARCO_GENERAL, "compositing-manager", false),
        (MARCO_GENERAL, "reduced-resources", true),
        (INTERFACE, "enable-animations", false),
    ] {
        let (f, id) = desk(Kind::Xdg, vec![(schema, key.into(), Value::Bool(value))]);
        assert_at_once(f, id);
    }
}

#[test]
fn the_key_turned_off_leaves_the_overview_on() {
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    f.set_setting(CAMINITO, "overview-animation", Value::Bool(false));
    f.dispatch_until(|f| !f.state.caminito.settings.overview_animation);

    assert_at_once(f, id);
}

// The lock.

#[test]
fn the_session_lock_ends_a_transition_at_once() {
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    map_three(&mut f, id);
    let locker = f.connect(Kind::Xdg);
    press_shortcut(&mut f);
    f.advance_clock(MIDDLE);
    assert_eq!(direction(&f), "opening");

    let lock = f.client(locker).lock_session();
    f.roundtrip(locker);

    assert_ne!(f.lock_state(), "unlocked");
    assert!(overview(&f).is_null());
    assert!(transition(&f).is_null(), "the lock ended it");
    assert!(!overview_is_drawn(&mut f));
    f.client(locker).unlock_and_destroy(lock);
    f.roundtrip(locker);
    assert!(transition(&f).is_null());
}
