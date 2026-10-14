//! The Overview (241): the shortcut that opens it, the windows it lists and where it lays
//! them out, what it draws in place of the output's own, and the keys and the clicks that
//! close it, read from the debug dump and from the output's pixels. It opens and closes in
//! one frame. Its selection, the arrow keys and Enter, the middle click, and windows that
//! map or go while it's open are 242's.
//!
//! What both kinds of window share runs on both. The tests of what a client sees of the
//! keys and the clicks, and of what only the Overview's own state shows (the shortcut, the
//! lock, the Window menu), map xdg toplevels only: the keys are checked as a Wayland client
//! sees them (45), and the rest doesn't depend on the kind.

use caminito_test_client::InputEvent;
use caminito_test_client::layer_shell::{KeyboardInteractivity, Layer, LayerAnchor, LayerSpec};
use caminito_test_client::session_lock::LockId;
use smithay::utils::{Logical, Point, Rectangle};

use super::accelerators::Caller;
use super::fixture::{
    BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, ClientId, ClientWindow, Fixture, KEY_A, KEY_DOWN, KEY_ENTER,
    KEY_ESC, KEY_LEFT, KEY_LEFTALT, KEY_LEFTCTRL, KEY_LEFTMETA, KEY_MUTE, KEY_RIGHT, KEY_SPACE,
    KEY_TAB, KEY_UP, KEY_W, Kind, WINDOW_COLOR, WindowSpec, on_both_kinds,
};
use crate::scene::SceneLayer;
use crate::settings::Value;
use crate::settings::translate::CAMINITO;
use crate::state::SEAT;
use crate::workspace_slide::LENGTH;

/// The window's colour, as `pixel` reads it.
const WINDOW: u32 = WINDOW_COLOR & 0xff_ffff;

/// The output every test here has.
const OUTPUT: (i32, i32) = (1920, 1080);

/// The debug dump's `overview` member: `null` while the Overview is closed.
fn overview(f: &Fixture) -> serde_json::Value {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    dump["overview"].clone()
}

fn is_open(f: &Fixture) -> bool {
    !overview(f).is_null()
}

/// Presses the Overview's default shortcut, Super+W.
fn press_shortcut(f: &mut Fixture) {
    f.chord(&[KEY_LEFTMETA, KEY_W]);
}

/// Opens the Overview with its shortcut.
#[track_caller]
fn open(f: &mut Fixture) {
    press_shortcut(f);
    assert!(is_open(f), "the shortcut opens the Overview");
}

fn rect(dump: &serde_json::Value) -> Rectangle<i32, Logical> {
    let field = |name: &str| dump[name].as_i64().unwrap() as i32;
    Rectangle::new(
        (field("x"), field("y")).into(),
        (field("width"), field("height")).into(),
    )
}

/// The windows the open Overview lists, by policy's ids.
fn listed(f: &Fixture) -> Vec<u64> {
    let windows = overview(f)["windows"].as_array().unwrap().clone();
    windows
        .iter()
        .map(|window| window["window"].as_u64().unwrap())
        .collect()
}

/// One of the two rectangles the dump has for `window` in the layout: `rect` or `label`.
#[track_caller]
fn part(f: &Fixture, window: ClientWindow, part: &str) -> Rectangle<i32, Logical> {
    let id = f.pid(window).0;
    let windows = overview(f)["windows"].as_array().unwrap().clone();
    let listed = windows.iter().find(|listed| listed["window"] == id);
    rect(&listed.expect("the window is listed")[part])
}

/// Where the layout draws `window`.
#[track_caller]
fn place(f: &Fixture, window: ClientWindow) -> Rectangle<i32, Logical> {
    part(f, window, "rect")
}

fn middle(rect: Rectangle<i32, Logical>) -> Point<i32, Logical> {
    rect.loc + Point::from((rect.size.w / 2, rect.size.h / 2))
}

/// Clicks the left button at `at`.
fn click(f: &mut Fixture, at: Point<i32, Logical>) {
    f.click(f64::from(at.x), f64::from(at.y));
}

/// The output's pixel at `at`, as `0xRRGGBB`.
fn pixel(f: &mut Fixture, at: Point<i32, Logical>) -> u32 {
    let output = f.state.caminito.outputs[0].clone();
    let pixel = f
        .state
        .backend
        .headless_mut()
        .pixel(&output, at.to_physical(1));
    pixel.expect("the output is drawn") & 0xff_ffff
}

/// Whether `pixel` is `color` under the Overview's dimming, which is half of black: each
/// channel halved, give or take the rounding.
fn is_dimmed(pixel: u32, color: u32) -> bool {
    [16, 8, 0].into_iter().all(|shift| {
        let (drawn, whole) = ((pixel >> shift) & 0xff, (color >> shift) & 0xff);
        drawn.abs_diff(whole / 2) <= 1
    })
}

fn focus(f: &Fixture) -> Option<u64> {
    f.state.caminito.policy.focus(SEAT).map(|id| id.0)
}

fn stacking(f: &Fixture) -> Vec<u64> {
    let stacking = f.state.caminito.policy.active_stacking();
    stacking.iter().map(|id| id.0).collect()
}

fn minimise(f: &mut Fixture, window: ClientWindow) {
    let id = f.pid(window);
    f.state.caminito.policy.minimize(id);
    f.dispatch();
}

/// Three windows of different sizes, the last one focused and on top.
fn map_three(f: &mut Fixture, id: ClientId) -> [ClientWindow; 3] {
    [(400, 300), (640, 480), (300, 500)].map(|size| f.map(id, WindowSpec::plain(size)))
}

/// The keys an xdg client that records its input saw since it last forgot it.
fn keys_seen(f: &mut Fixture, client: ClientId) -> Vec<(u32, bool)> {
    f.roundtrip(client);
    let input = std::mem::take(&mut f.client(client).state.input);
    input
        .into_iter()
        .filter_map(|event| match event {
            InputEvent::Key { key, pressed } => Some((key, pressed)),
            _ => None,
        })
        .collect()
}

// Opening, and what the dump has.

on_both_kinds! {
    fn the_shortcut_opens_it_with_each_window_of_the_workspace_in_the_layout(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let windows = map_three(&mut f, id);
        assert!(!is_open(&f));

        press_shortcut(&mut f);

        let dump = overview(&f);
        let output = f.state.caminito.outputs[0].name();
        assert_eq!(dump["output"], output.as_str());
        assert_eq!(listed(&f), stacking(&f));
        let area = Rectangle::<i32, Logical>::from_size(OUTPUT.into());
        let places = windows.map(|window| place(&f, window));
        for (index, (window, drawn)) in windows.into_iter().zip(places).enumerate() {
            let frame = f.state.caminito.policy.frame_rect(f.pid(window)).unwrap();
            // They all fit at their own sizes.
            assert_eq!(drawn.size, frame.size, "window {index}");
            assert!(area.contains_rect(drawn), "window {index}: {drawn:?}");
            let label = part(&f, window, "label");
            assert_eq!(label.loc.x, drawn.loc.x);
            assert!(label.loc.y >= drawn.loc.y + drawn.size.h);
            for other in &places[index + 1..] {
                assert!(!drawn.overlaps(*other), "{drawn:?} is over {other:?}");
            }
        }
    }
}

on_both_kinds! {
    fn the_layout_shows_each_window_at_its_place(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let windows = map_three(&mut f, id);

        open(&mut f);

        for window in windows {
            let at = middle(place(&f, window));
            assert_eq!(pixel(&mut f, at), WINDOW, "{window:?} at {at:?}");
        }
        // And nothing of them where they are.
        assert!(f.state.caminito.under((10.0, 10.0).into()).is_none());
        let layers = f.state.scene_layers(&f.state.caminito.outputs[0].clone());
        assert!(layers.contains(&SceneLayer::Overview));
        assert!(!layers.contains(&SceneLayer::Windows));
    }
}

#[test]
fn it_opens_on_a_workspace_with_no_windows() {
    let (mut f, _) = Fixture::connected(Kind::Xdg);

    press_shortcut(&mut f);

    assert!(is_open(&f));
    assert_eq!(listed(&f), Vec::<u64>::new());
}

on_both_kinds! {
    fn a_focused_fullscreen_window_is_laid_out_like_any_other(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let small = f.map(id, WindowSpec::plain((400, 300)));
        let full = f.map(id, WindowSpec::plain((400, 300)));
        let pid = f.pid(full);
        f.state.caminito.policy.set_fullscreen(pid, true, None);
        f.dispatch();
        f.draw_at_asked_size(full, OUTPUT);
        let output = f.state.caminito.outputs[0].clone();
        assert!(f.state.scene_layers(&output).contains(&SceneLayer::FullscreenWindows));

        open(&mut f);

        let drawn = place(&f, full);
        assert!(drawn.size.w < OUTPUT.0 && drawn.size.h < OUTPUT.1, "{drawn:?}");
        assert!(!drawn.overlaps(place(&f, small)));
        assert!(!f.state.scene_layers(&output).contains(&SceneLayer::FullscreenWindows));
    }
}

/// Only an X11 client asks to be left out of the taskbar itself.
#[test]
fn a_window_that_skips_the_taskbar_isnt_listed() {
    let (mut f, id) = Fixture::connected(Kind::X11);
    let listed_one = f.map(id, WindowSpec::plain((400, 300)));
    let skipping = f.map(
        id,
        WindowSpec {
            states: vec!["_NET_WM_STATE_SKIP_TASKBAR"],
            ..WindowSpec::plain((300, 200))
        },
    );

    open(&mut f);

    assert_eq!(listed(&f), [f.pid(listed_one).0]);
    assert!(!listed(&f).contains(&f.pid(skipping).0));
}

on_both_kinds! {
    fn another_workspaces_window_isnt_listed(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let [here, away, _] = map_three(&mut f, id);
        let pid = f.pid(away);
        f.state.caminito.policy.move_to_workspace(pid, 1);
        f.dispatch();

        open(&mut f);

        assert!(listed(&f).contains(&f.pid(here).0));
        assert!(!listed(&f).contains(&pid.0));
        assert_eq!(listed(&f).len(), 2);
    }
}

// The rest of the output.

/// The wallpaper, caja's desktop and the panel, in colours of their own.
const WALLPAPER: u32 = 0xff40_80c0;
const DESKTOP: u32 = 0xffc0_4040;
const PANEL: u32 = 0xff20_c020;
const PANEL_HEIGHT: i32 = 30;

/// Where the desktop's surface is, which doesn't cover the output as caja's does, so that
/// the wallpaper shows beside it.
fn desktop_spot() -> Point<i32, Logical> {
    Point::from((100, 540))
}

fn panel_spot() -> Point<i32, Logical> {
    Point::from((960, PANEL_HEIGHT / 2))
}

fn wallpaper_spot() -> Point<i32, Logical> {
    Point::from((1500, 900))
}

/// Maps the wallpaper, a desktop of 300 × 300 at the left and the panel along the top.
fn map_the_desktop(f: &mut Fixture) {
    let shell = f.connect(Kind::Xdg);
    f.client(shell).bind_outputs();
    // Under the panel too, as the wallpaper's drawer has it.
    let wallpaper = LayerSpec {
        exclusive_zone: -1,
        ..LayerSpec::covering(Layer::Background, "mate-background")
    };
    f.map_layer(shell, wallpaper, WALLPAPER);
    let desktop = LayerSpec {
        layer: Layer::Background,
        namespace: "desktop",
        output: None,
        anchor: LayerAnchor::Left,
        size: (300, 300),
        exclusive_zone: 0,
        keyboard: KeyboardInteractivity::None,
    };
    f.map_layer(shell, desktop, DESKTOP);
    f.map_layer(shell, LayerSpec::top_bar(PANEL_HEIGHT as u32), PANEL);
    f.wait_for_pixel(panel_spot().x, panel_spot().y, PANEL & 0xff_ffff);
}

#[test]
fn the_panel_and_cajas_desktop_arent_drawn_and_the_wallpaper_is_dimmed() {
    let (mut f, _) = Fixture::connected(Kind::Xdg);
    map_the_desktop(&mut f);
    let output = f.state.caminito.outputs[0].clone();
    let rgb = |argb: u32| argb & 0xff_ffff;
    assert_eq!(pixel(&mut f, desktop_spot()), rgb(DESKTOP));
    assert_eq!(pixel(&mut f, wallpaper_spot()), rgb(WALLPAPER));

    open(&mut f);

    let dump = overview(&f);
    let hidden: Vec<&str> = dump["hidden_layer_surfaces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|namespace| namespace.as_str().unwrap())
        .collect();
    assert_eq!(hidden, ["desktop", "panel"]);
    assert_eq!(dump["dim"], 0.5);
    for spot in [desktop_spot(), panel_spot(), wallpaper_spot()] {
        let drawn = pixel(&mut f, spot);
        assert!(is_dimmed(drawn, rgb(WALLPAPER)), "{spot:?} is {drawn:06x}");
    }
    let layers = f.state.scene_layers(&output);
    assert!(!layers.contains(&SceneLayer::Top), "{layers:?}");
    assert_eq!(
        layers
            .iter()
            .filter(|l| **l == SceneLayer::Background)
            .count(),
        1,
        "the wallpaper alone: {layers:?}"
    );

    // And all as it was after it closes.
    press_shortcut(&mut f);
    assert!(!is_open(&f));
    assert_eq!(pixel(&mut f, desktop_spot()), rgb(DESKTOP));
    assert_eq!(pixel(&mut f, panel_spot()), rgb(PANEL));
    assert_eq!(pixel(&mut f, wallpaper_spot()), rgb(WALLPAPER));
    assert!(f.state.scene_layers(&output).contains(&SceneLayer::Top));
}

on_both_kinds! {
    fn the_windows_are_where_they_were_after_it_closes(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let windows = map_three(&mut f, id);
        minimise(&mut f, windows[0]);
        let before = (focus(&f), stacking(&f), windows.map(|w| f.geometry(w)));
        let shown = middle(f.geometry(windows[2]));
        assert_eq!(pixel(&mut f, shown), WINDOW);

        open(&mut f);
        press_shortcut(&mut f);

        assert!(!is_open(&f));
        assert_eq!((focus(&f), stacking(&f), windows.map(|w| f.geometry(w))), before);
        assert!(f.state.caminito.policy.is_minimized(f.pid(windows[0])));
        assert_eq!(pixel(&mut f, shown), WINDOW);
        let output = f.state.caminito.outputs[0].clone();
        assert!(!f.state.scene_layers(&output).contains(&SceneLayer::Overview));
    }
}

// The pointer.

on_both_kinds! {
    fn a_click_on_a_window_activates_it_and_closes_the_overview(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let [first, _, last] = map_three(&mut f, id);
        assert_eq!(focus(&f), Some(f.pid(last).0));
        open(&mut f);

        let at = middle(place(&f, first));
        click(&mut f, at);

        assert!(!is_open(&f));
        assert_eq!(focus(&f), Some(f.pid(first).0));
        assert_eq!(stacking(&f).last(), Some(&f.pid(first).0));
        assert!(f.has_keyboard(first));
    }
}

on_both_kinds! {
    fn a_click_on_a_windows_label_chooses_it_too(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let [first, ..] = map_three(&mut f, id);
        open(&mut f);

        let at = middle(part(&f, first, "label"));
        click(&mut f, at);

        assert!(!is_open(&f));
        assert_eq!(focus(&f), Some(f.pid(first).0));
    }
}

on_both_kinds! {
    fn a_click_on_a_minimised_window_unminimises_it(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let [first, ..] = map_three(&mut f, id);
        minimise(&mut f, first);
        open(&mut f);
        // It's in the layout with what it last drew.
        let at = middle(place(&f, first));
        assert_eq!(pixel(&mut f, at), WINDOW);

        click(&mut f, at);

        assert!(!is_open(&f));
        assert!(!f.state.caminito.policy.is_minimized(f.pid(first)));
        assert_eq!(focus(&f), Some(f.pid(first).0));
    }
}

on_both_kinds! {
    fn a_click_on_the_empty_area_closes_it_and_changes_nothing(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        map_three(&mut f, id);
        let before = (focus(&f), stacking(&f));
        open(&mut f);

        click(&mut f, Point::from((5, 5)));

        assert!(!is_open(&f));
        assert_eq!((focus(&f), stacking(&f)), before);
    }
}

#[test]
fn no_client_sees_the_overviews_clicks() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let [first, ..] = map_three(&mut f, id);
    open(&mut f);
    f.roundtrip(id);
    f.client(id).state.input.clear();

    // The right button on a window does nothing, and the left one chooses it.
    let at = middle(place(&f, first));
    f.pointer_move(f64::from(at.x), f64::from(at.y));
    f.pointer_button(BTN_RIGHT, true);
    f.pointer_button(BTN_RIGHT, false);
    assert!(is_open(&f));
    f.pointer_button(BTN_LEFT, true);
    f.pointer_button(BTN_LEFT, false);
    assert!(!is_open(&f));

    f.roundtrip(id);
    let input = &f.client(id).state.input;
    let buttons = input
        .iter()
        .filter(|e| matches!(e, InputEvent::Button { .. }));
    assert_eq!(buttons.count(), 0, "{input:?}");
}

// The keys.

on_both_kinds! {
    fn esc_closes_it_and_changes_nothing(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        map_three(&mut f, id);
        let before = (focus(&f), stacking(&f));
        open(&mut f);

        f.chord(&[KEY_ESC]);

        assert!(!is_open(&f));
        assert_eq!((focus(&f), stacking(&f)), before);
    }
}

#[test]
fn a_key_reaches_no_window_and_alt_tab_opens_no_window_switcher() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    map_three(&mut f, id);
    let before = focus(&f);
    open(&mut f);
    keys_seen(&mut f, id);

    f.chord(&[KEY_A]);
    f.chord(&[KEY_LEFTALT, KEY_TAB]);
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_A]);

    assert!(is_open(&f));
    assert!(f.state.caminito.policy.switcher().is_none());
    assert_eq!(focus(&f), before);
    // The modifiers alone, which go on to the client as with the Window switcher open.
    let seen = keys_seen(&mut f, id);
    let modifiers = [KEY_LEFTALT, KEY_LEFTCTRL];
    assert!(
        seen.iter().all(|(key, _)| modifiers.contains(key)),
        "{seen:?}"
    );

    // And its keys are the client's again once it's closed.
    f.chord(&[KEY_ESC]);
    f.chord(&[KEY_A]);
    assert_eq!(keys_seen(&mut f, id), [(KEY_A, true), (KEY_A, false)]);
}

#[test]
fn a_media_key_works_and_another_accelerator_is_swallowed() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    map_three(&mut f, id);
    let bus = f.start_dbus();
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["XF86AudioMute", "<Control><Alt>a"]);
    let (mute, ctrl_alt_a) = (actions[0], actions[1]);
    assert!(mute != 0 && ctrl_alt_a != 0);
    open(&mut f);

    f.chord(&[KEY_MUTE]);
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_A]);
    f.chord(&[KEY_MUTE]);
    assert!(is_open(&f));
    // The other accelerator ends them once the Overview is closed: nothing else was
    // signalled before it.
    f.chord(&[KEY_ESC]);
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_A]);

    let signalled = [0; 3].map(|_| caller.next_activation(&mut f).0);
    assert_eq!(signalled, [mute, mute, ctrl_alt_a]);
}

// The shortcut.

#[test]
fn with_the_shortcut_removed_nothing_opens_the_overview() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    map_three(&mut f, id);
    let bound = |f: &Fixture| {
        let bindings = &f.state.caminito.settings.bindings;
        bindings.label_of("overview")
    };
    assert_eq!(bound(&f).as_deref(), Some("Super+W"));

    f.set_setting(CAMINITO, "overview", Value::Str("disabled".into()));
    f.dispatch_until(|f| bound(f).is_none());
    press_shortcut(&mut f);
    assert!(!is_open(&f));

    f.set_setting(CAMINITO, "overview", Value::Str("<Super>Tab".into()));
    f.dispatch_until(|f| bound(f).is_some());
    press_shortcut(&mut f);
    assert!(!is_open(&f));
    f.chord(&[KEY_LEFTMETA, KEY_TAB]);
    assert!(is_open(&f));
}

#[test]
fn the_shortcut_does_nothing_while_the_window_menu_is_open() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.map(id, WindowSpec::default());
    f.chord(&[KEY_LEFTALT, KEY_SPACE]);
    assert!(f.state.caminito.policy.window_menu().is_some());

    press_shortcut(&mut f);

    assert!(!is_open(&f));
    assert!(f.state.caminito.policy.window_menu().is_some());
}

#[test]
fn no_window_menu_opens_while_the_overview_is_open() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.map(id, WindowSpec::default());
    open(&mut f);

    f.chord(&[KEY_LEFTALT, KEY_SPACE]);

    assert!(is_open(&f));
    assert!(f.state.caminito.policy.window_menu().is_none());
}

// Live windows.

/// The test's Wayland client draws its windows again while the Overview is open; the
/// fixture's X clients draw once (45).
#[test]
fn a_window_in_the_layout_gets_frame_callbacks_and_shows_what_it_draws() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let shown = f.map(id, WindowSpec::plain((400, 300)));
    let minimised = f.map(id, WindowSpec::plain((400, 300)));
    f.map(id, WindowSpec::plain((400, 300)));
    minimise(&mut f, minimised);
    open(&mut f);

    for (window, argb) in [(shown, 0xff00_00ff), (minimised, 0xffff_0000)] {
        let at = middle(place(&f, window));
        assert_eq!(pixel(&mut f, at), WINDOW);
        let frames = f.client(id).window(window.xdg()).frames_done;

        f.client(id).attach_buffer(window.xdg(), (400, 300), argb);
        f.client(id).request_frame(window.xdg());
        f.client(id).commit(window.xdg());
        f.roundtrip(id);

        f.wait_for_pixel(at.x, at.y, argb & 0xff_ffff);
        f.dispatch_until_mut(|f| f.client(id).window(window.xdg()).frames_done > frames);
        assert!(is_open(&f));
    }
}

// The session lock.

/// Locks the session as a new client's ext-session-lock. Returns the locker and its lock.
fn lock_the_session(f: &mut Fixture) -> (ClientId, LockId) {
    let locker = f.connect(Kind::Xdg);
    let lock = f.client(locker).lock_session();
    f.roundtrip(locker);
    assert_eq!(f.lock_state(), "locked");
    (locker, lock)
}

#[test]
fn the_session_lock_ends_the_overview_and_lets_none_open() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    map_three(&mut f, id);
    let before = (focus(&f), stacking(&f));
    open(&mut f);

    let (locker, lock) = lock_the_session(&mut f);

    assert!(!is_open(&f));
    assert!(f.state.caminito.policy.overview().is_none());
    press_shortcut(&mut f);
    assert!(!is_open(&f));

    f.client(locker).unlock_and_destroy(lock);
    f.roundtrip(locker);
    assert!(!is_open(&f));
    assert_eq!((focus(&f), stacking(&f)), before);
    press_shortcut(&mut f);
    assert!(is_open(&f));
}

// The Workspace slide.

/// Whether a Workspace slide runs, from the debug dump's `workspace_slides`.
fn slides(f: &Fixture) -> bool {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    dump["workspace_slides"]
        .as_array()
        .is_some_and(|slides| !slides.is_empty())
}

on_both_kinds! {
    fn the_shortcut_during_a_workspace_slide_opens_it_at_once_and_cuts_the_slide(kind: Kind) {
        let mut f = Fixture::with_settings(vec![(
            CAMINITO,
            "workspace-slide".into(),
            Value::Bool(true),
        )]);
        f.add_output(OUTPUT, 1.0);
        let id = f.connect(kind);
        let leaving = f.map(id, WindowSpec::plain((700, 500)));
        let arriving = f.map(id, WindowSpec::plain((600, 400)));
        let pid = f.pid(arriving);
        f.state.caminito.policy.move_to_workspace(pid, 1);
        f.dispatch();
        // As a pager's switch, which shows no Workspace popup.
        f.state.caminito.policy.switch_workspace(1);
        f.dispatch();
        f.advance_clock(LENGTH / 5);
        assert!(slides(&f), "the switch slides");

        press_shortcut(&mut f);

        assert!(is_open(&f));
        assert!(!slides(&f), "the slide is cut to its end");
        assert_eq!(listed(&f), [pid.0]);
        let at = middle(place(&f, arriving));
        assert_eq!(pixel(&mut f, at), WINDOW);
        // Nothing of the workspace it left is drawn.
        let left = middle(f.geometry(leaving));
        assert!(!place(&f, arriving).contains(left));
        assert_ne!(pixel(&mut f, left), WINDOW);

        // And none runs once it's closed.
        f.chord(&[KEY_ESC]);
        assert!(!slides(&f));
        assert_eq!(f.state.caminito.policy.active_workspace(), 1);
    }
}

// The selection, the arrow keys and Enter (242).

/// The open Overview's selected window, by policy's id.
fn selected(f: &Fixture) -> Option<u64> {
    overview(f)["selected"].as_u64()
}

/// A spot inside `window`'s label, in its padding before the icon.
fn label_spot(f: &Fixture, window: ClientWindow) -> Point<i32, Logical> {
    let label = part(f, window, "label");
    label.loc + Point::from((2, label.size.h / 2))
}

on_both_kinds! {
    fn the_focused_window_is_selected_at_opening_and_its_label_is_marked(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let [first, second, third] = map_three(&mut f, id);

        open(&mut f);

        assert_eq!(selected(&f), Some(f.pid(third).0));
        assert_eq!(focus(&f), selected(&f));
        let (first_spot, third_spot) = (label_spot(&f, first), label_spot(&f, third));
        let second_spot = label_spot(&f, second);
        let unmarked = pixel(&mut f, first_spot);
        assert_eq!(pixel(&mut f, second_spot), unmarked);
        assert_ne!(pixel(&mut f, third_spot), unmarked);

        // The mark moves with the selection.
        f.chord(&[KEY_LEFT]);
        f.chord(&[KEY_LEFT]);
        assert_eq!(selected(&f), Some(f.pid(first).0));
        assert_eq!(pixel(&mut f, third_spot), unmarked);
        assert_ne!(pixel(&mut f, first_spot), unmarked);
    }
}

#[test]
fn with_no_window_focused_none_is_selected() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    for window in map_three(&mut f, id) {
        minimise(&mut f, window);
    }
    assert_eq!(focus(&f), None);

    open(&mut f);

    assert_eq!(selected(&f), None);
    // Enter does nothing then.
    f.chord(&[KEY_ENTER]);
    assert!(is_open(&f));
    assert_eq!(focus(&f), None);
}

on_both_kinds! {
    fn the_arrow_keys_and_enter_activate_the_window_chosen_and_close_it(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        // One row, in the order they were mapped from the left; the third is focused.
        let [first, second, third] = map_three(&mut f, id);
        open(&mut f);
        let pid = |f: &Fixture, window: ClientWindow| Some(f.pid(window).0);

        f.chord(&[KEY_LEFT]);
        assert_eq!(selected(&f), pid(&f, second));
        f.chord(&[KEY_LEFT]);
        assert_eq!(selected(&f), pid(&f, first));
        // At the layout's edge, and on its one row, the selection stays.
        for key in [KEY_LEFT, KEY_UP, KEY_DOWN] {
            f.chord(&[key]);
            assert_eq!(selected(&f), pid(&f, first), "{key}");
        }
        f.chord(&[KEY_RIGHT]);
        f.chord(&[KEY_RIGHT]);
        f.chord(&[KEY_RIGHT]);
        assert_eq!(selected(&f), pid(&f, third));
        f.chord(&[KEY_LEFT]);
        assert!(is_open(&f));

        f.chord(&[KEY_ENTER]);

        assert!(!is_open(&f));
        assert_eq!(focus(&f), pid(&f, second));
        assert_eq!(stacking(&f).last().copied(), pid(&f, second));
    }
}

#[test]
fn an_arrow_key_with_none_selected_selects_the_first_and_enter_unminimises_it() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let windows = map_three(&mut f, id);
    for window in windows {
        minimise(&mut f, window);
    }
    open(&mut f);

    f.chord(&[KEY_DOWN]);
    assert_eq!(selected(&f), Some(f.pid(windows[0]).0));
    f.chord(&[KEY_ENTER]);

    assert!(!is_open(&f));
    let chosen = f.pid(windows[0]);
    assert!(!f.state.caminito.policy.is_minimized(chosen));
    assert_eq!(focus(&f), Some(chosen.0));
}

#[test]
fn no_client_sees_the_arrow_keys_or_enter() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    map_three(&mut f, id);
    open(&mut f);
    keys_seen(&mut f, id);

    f.chord(&[KEY_LEFT]);
    f.chord(&[KEY_ENTER]);

    assert!(!is_open(&f));
    assert_eq!(keys_seen(&mut f, id), []);
}

// The middle click (242).

on_both_kinds! {
    fn a_middle_click_on_a_window_asks_it_to_close_and_the_overview_stays_open(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let [first, second, _] = map_three(&mut f, id);
        open(&mut f);
        assert!(!f.close_requested(first));

        let at = middle(place(&f, first));
        f.pointer_move(f64::from(at.x), f64::from(at.y));
        f.pointer_button(BTN_MIDDLE, true);
        f.pointer_button(BTN_MIDDLE, false);

        assert!(f.close_requested(first));
        assert!(!f.close_requested(second));
        assert!(is_open(&f));
        assert!(listed(&f).contains(&f.pid(first).0));
    }
}

#[test]
fn no_client_sees_the_middle_click() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let [first, ..] = map_three(&mut f, id);
    open(&mut f);
    f.roundtrip(id);
    f.client(id).state.input.clear();

    let at = middle(place(&f, first));
    f.pointer_move(f64::from(at.x), f64::from(at.y));
    f.pointer_button(BTN_MIDDLE, true);
    f.pointer_button(BTN_MIDDLE, false);

    f.roundtrip(id);
    let input = &f.client(id).state.input;
    let buttons = input
        .iter()
        .filter(|e| matches!(e, InputEvent::Button { .. }));
    assert_eq!(buttons.count(), 0, "{input:?}");
}

// Windows that come and go (242).

/// Has a window's client destroy it, and waits until Caminito no longer manages it.
fn destroy(f: &mut Fixture, window: ClientWindow) {
    match window {
        ClientWindow::Xdg(id, xdg) => {
            f.client(id).destroy_window(xdg);
            f.roundtrip(id);
        }
        ClientWindow::X11(id, x11) => {
            f.x11_call(id, move |x| x.destroy(x11)).unwrap();
            f.dispatch_until(|f| f.policy_id(window).is_none());
        }
    }
}

on_both_kinds! {
    fn a_window_that_maps_while_it_is_open_is_in_the_layouts_next_frame(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let first = f.map(id, WindowSpec::plain((400, 300)));
        let second = f.map(id, WindowSpec::plain((640, 480)));
        open(&mut f);
        let before = place(&f, first);

        let third = f.map(id, WindowSpec::plain((300, 500)));

        assert!(is_open(&f));
        assert_eq!(listed(&f), stacking(&f));
        assert!(listed(&f).contains(&f.pid(third).0));
        let places = [first, second, third].map(|window| place(&f, window));
        assert_ne!(places[0], before, "the others take their new places");
        for (index, drawn) in places.iter().enumerate() {
            assert_eq!(pixel(&mut f, middle(*drawn)), WINDOW, "window {index}");
            for other in &places[index + 1..] {
                assert!(!drawn.overlaps(*other), "{drawn:?} is over {other:?}");
            }
        }
    }
}

on_both_kinds! {
    fn a_window_that_goes_leaves_the_layout_and_the_selection_with_it(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let [first, second, third] = map_three(&mut f, id);
        open(&mut f);
        let gone = f.pid(third).0;
        assert_eq!(selected(&f), Some(gone));
        let before = place(&f, first);

        destroy(&mut f, third);

        assert!(is_open(&f));
        assert_eq!(listed(&f), [f.pid(first).0, f.pid(second).0]);
        assert_eq!(selected(&f), None);
        let places = [first, second].map(|window| place(&f, window));
        assert_ne!(places[0], before, "the others take their new places");
        for drawn in places {
            assert_eq!(pixel(&mut f, middle(drawn)), WINDOW, "{drawn:?}");
        }

        // An arrow key selects the first again.
        f.chord(&[KEY_RIGHT]);
        assert_eq!(selected(&f), Some(f.pid(first).0));
    }
}
