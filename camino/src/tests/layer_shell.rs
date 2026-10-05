//! Layer surfaces (63): the panel, caja's desktop, the wallpaper and notification bubbles,
//! as a wlr-layer-shell client makes them. Only Wayland clients make them. Where a test
//! is about a window beside them, it runs with an xdg toplevel and an XWayland window
//! (45); the tests that read the keyboard's or the pointer's events as the window's client
//! had them map an xdg toplevel, since only a Wayland test client records those.

use camino_test_client::layer_shell::{
    KeyboardInteractivity, Layer, LayerAnchor, LayerSpec, format_layer_configures,
};
use camino_test_client::{
    Anchor, ConstraintAdjustment, Gravity, InputEvent, PopupId, Positioner, SurfaceRef,
    format_input, format_popup_configures,
};
use insta::assert_snapshot;
use smithay::utils::{Logical, Point};

use super::fixture::{BTN_LEFT, ClientId, ClientWindow, Fixture, Kind, WindowSpec, on_both_kinds};
use crate::scene::SceneLayer;
use crate::state::SEAT;

const BAR_COLOR: u32 = 0xff00ff00;
const RED: u32 = 0xffff0000;
const BLUE: u32 = 0xff0000ff;

/// Each output's layer surfaces from the dump, one per line.
fn layers(f: &Fixture) -> String {
    let dump: serde_json::Value = serde_json::from_str(&f.state.camino.state_dump()).unwrap();
    let mut lines = Vec::new();
    for output in dump["outputs"].as_array().unwrap() {
        for layer in output["layer_surfaces"].as_array().unwrap() {
            let g = &layer["geometry"];
            lines.push(format!(
                "{}: {} {} at {},{} {}x{} zone {} keyboard {}",
                output["name"].as_str().unwrap(),
                layer["layer"].as_str().unwrap(),
                layer["namespace"].as_str().unwrap(),
                g["x"],
                g["y"],
                g["width"],
                g["height"],
                layer["exclusive_zone"],
                layer["keyboard_interactivity"].as_str().unwrap(),
            ));
        }
    }
    lines.join("\n")
}

#[test]
fn a_top_anchored_bar_is_configured_to_the_width_of_the_output_it_names() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.add_output((1280, 720), 1.0);
    f.roundtrip(id);
    f.client(id).bind_outputs();
    f.roundtrip(id);

    let bar = f.map_layer(
        id,
        LayerSpec {
            output: Some(1),
            ..LayerSpec::top_bar(28)
        },
        BAR_COLOR,
    );

    assert_snapshot!(
        format_layer_configures(&f.client(id).layer_surface(bar).configures),
        @"size: 1280 × 28"
    );
    assert_snapshot!(layers(&f), @"headless-2: top panel at 1920,0 1280x28 zone 28 keyboard none");
}

#[test]
fn a_layer_surface_that_names_no_output_goes_on_the_one_under_the_pointer() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.add_output((1280, 720), 1.0);
    f.pointer_move(2000., 100.);

    f.map_layer(id, LayerSpec::top_bar(28), BAR_COLOR);

    assert_snapshot!(layers(&f), @"headless-2: top panel at 1920,0 1280x28 zone 28 keyboard none");
}

#[test]
fn a_layer_surface_is_told_the_scale_of_its_output_before_it_draws() {
    let mut f = Fixture::new();
    f.add_output((3840, 2160), 2.0);
    let id = f.connect(Kind::Xdg);

    let bar = f.client(id).create_layer_surface(&LayerSpec::top_bar(28));
    f.roundtrip(id);

    let bar = f.client(id).layer_surface(bar);
    assert_eq!(bar.output_events, ["buffer scale 2"]);
    assert_snapshot!(format_layer_configures(&bar.configures), @"size: 1920 × 28");
}

#[test]
fn two_exclusive_surfaces_on_one_edge_sit_one_after_the_other() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);

    f.map_layer(id, LayerSpec::top_bar(28), BAR_COLOR);
    f.map_layer(id, LayerSpec::top_bar(40), BAR_COLOR);

    assert_snapshot!(layers(&f), @"
    headless-1: top panel at 0,0 1920x28 zone 28 keyboard none
    headless-1: top panel at 0,28 1920x40 zone 40 keyboard none
    ");
}

#[test]
fn an_outputs_mode_change_reconfigures_its_layer_surfaces() {
    let mut f = Fixture::new();
    let output = f.plug_with_modes((527, 296), &[(1920, 1080), (1280, 720)]);
    let id = f.connect(Kind::Xdg);
    let bar = f.map_layer(id, LayerSpec::top_bar(28), BAR_COLOR);

    f.change_mode(&output, (1280, 720));
    f.roundtrip(id);

    assert_snapshot!(format_layer_configures(&f.client(id).layer_surface(bar).configures), @"
    size: 1920 × 28
    size: 1280 × 28
    ");
}

#[test]
fn unplugging_an_output_closes_its_layer_surfaces() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let second = f.add_output((1280, 720), 1.0);
    f.roundtrip(id);
    f.client(id).bind_outputs();
    f.roundtrip(id);
    let on = |output| LayerSpec {
        output: Some(output),
        ..LayerSpec::top_bar(28)
    };
    let stays = f.map_layer(id, on(0), BAR_COLOR);
    let goes = f.map_layer(id, on(1), BAR_COLOR);

    f.unplug(&second);
    f.roundtrip(id);

    assert!(f.client(id).layer_surface(goes).closed);
    assert!(!f.client(id).layer_surface(stays).closed);
    assert_snapshot!(layers(&f), @"headless-1: top panel at 0,0 1920x28 zone 28 keyboard none");
}

/// The scene's layers that have something in them on the first output, front to back.
fn scene_layers(f: &mut Fixture) -> Vec<SceneLayer> {
    let output = f.state.camino.outputs[0].clone();
    let mut layers = f.state.scene_layers(&output);
    layers.dedup();
    layers
}

/// A surface on each of the four layer-shell layers, each over its whole output.
fn cover_every_layer(f: &mut Fixture, id: ClientId) {
    for layer in [Layer::Background, Layer::Bottom, Layer::Top, Layer::Overlay] {
        f.map_layer(id, LayerSpec::covering(layer, "test"), BAR_COLOR);
    }
}

on_both_kinds! {
    fn the_scene_has_the_windows_between_the_top_and_bottom_layers(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let shell = f.connect(Kind::Xdg);
        cover_every_layer(&mut f, shell);
        f.map(client, WindowSpec::default());

        assert_eq!(
            scene_layers(&mut f),
            [
                SceneLayer::Cursor,
                SceneLayer::Overlay,
                SceneLayer::Top,
                SceneLayer::Windows,
                SceneLayer::Bottom,
                SceneLayer::Background,
            ]
        );
    }
}

/// Makes a window fullscreen as its kind of client asks for it, and has it draw at the
/// size of the 1920 × 1080 output.
fn make_fullscreen(f: &mut Fixture, window: ClientWindow) {
    let client = window.client();
    match window {
        ClientWindow::Xdg(_, xdg) => f.client(client).set_fullscreen(xdg, None),
        ClientWindow::X11(_, id) => {
            f.x11_call(client, move |x| {
                let fullscreen = x.atom("_NET_WM_STATE_FULLSCREEN")?;
                x.send_client_message(id, "_NET_WM_STATE", [1, fullscreen, 0, 1, 0])
            })
            .unwrap();
        }
    }
    f.dispatch_until(|f| f.state.camino.policy.is_fullscreen(f.pid(window)));
    f.draw_at_asked_size(window, (1920, 1080));
}

on_both_kinds! {
    fn a_fullscreen_window_covers_the_top_layer_but_not_the_overlay_layer(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let shell = f.connect(Kind::Xdg);
        cover_every_layer(&mut f, shell);
        let window = f.map(client, WindowSpec::plain((400, 300)));

        make_fullscreen(&mut f, window);

        assert_eq!(
            scene_layers(&mut f),
            [
                SceneLayer::Cursor,
                SceneLayer::Overlay,
                SceneLayer::FullscreenWindows,
                SceneLayer::Top,
                SceneLayer::Bottom,
                SceneLayer::Background,
            ]
        );
    }
}

#[test]
fn the_pointer_enters_the_topmost_layer_surface_or_window_under_it() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let bottom = f.map_layer(id, LayerSpec::covering(Layer::Bottom, "test"), BAR_COLOR);
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let inside = f.geometry(window).loc + Point::from((200, 150));
    let entered = |f: &mut Fixture| {
        f.roundtrip(id);
        let entered = f
            .client(id)
            .state
            .input
            .iter()
            .rev()
            .find_map(|event| match event {
                InputEvent::PointerEnter { surface, .. } => Some(*surface),
                _ => None,
            });
        entered.unwrap()
    };

    // Beside the window, on the bottom layer.
    f.pointer_move(1900., 1000.);
    assert_eq!(entered(&mut f), SurfaceRef::Layer(bottom));
    f.pointer_move(f64::from(inside.x), f64::from(inside.y));
    assert_eq!(entered(&mut f), SurfaceRef::Window(window.xdg()));
    let top = f.map_layer(id, LayerSpec::covering(Layer::Top, "test"), BAR_COLOR);
    assert_eq!(entered(&mut f), SurfaceRef::Layer(top));
}

/// Maps a background surface with each of `namespaces` in turn, each over the whole
/// output: the desktop red, and the wallpaper blue.
fn map_backgrounds(f: &mut Fixture, id: ClientId, namespaces: [&'static str; 2]) {
    for namespace in namespaces {
        let color = if namespace == "desktop" { RED } else { BLUE };
        f.map_layer(id, LayerSpec::covering(Layer::Background, namespace), color);
    }
}

#[test]
fn the_desktop_shows_above_the_wallpaper_that_mapped_before_it() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);

    map_backgrounds(&mut f, id, ["mate-background", "desktop"]);

    f.wait_for_pixel(960, 540, RED & 0xffffff);
}

#[test]
fn the_desktop_shows_above_the_wallpaper_that_mapped_after_it() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);

    map_backgrounds(&mut f, id, ["desktop", "mate-background"]);

    f.wait_for_pixel(960, 540, RED & 0xffffff);
}

on_both_kinds! {
    fn an_on_demand_top_surface_gives_the_keyboard_back_to_a_window_that_covers_it(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let shell = f.connect(Kind::Xdg);
        let window = f.map(client, WindowSpec::plain((400, 300)));
        f.map_layer(
            shell,
            in_the_corner(KeyboardInteractivity::OnDemand),
            BAR_COLOR,
        );
        f.click(1900., 1060.);
        assert!(!f.has_keyboard(window));

        make_fullscreen(&mut f, window);

        assert!(f.has_keyboard(window));
    }
}

/// The keyboard's enter and leave events `id` has had since they were last asked for.
fn keyboard_events(f: &mut Fixture, id: ClientId) -> String {
    f.roundtrip(id);
    let events: Vec<InputEvent> = std::mem::take(&mut f.client(id).state.input)
        .into_iter()
        .filter(|event| {
            matches!(
                event,
                InputEvent::KeyboardEnter { .. } | InputEvent::KeyboardLeave { .. }
            )
        })
        .collect();
    format_input(&events)
}

/// A client with a focused window that has the keyboard, and the window.
fn with_a_focused_window() -> (Fixture, ClientId, ClientWindow) {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    assert_eq!(keyboard_events(&mut f, id), "keyboard enters window 0");
    (f, id, window)
}

/// A 200 × 100 surface in the bottom-right corner of the top layer, away from where policy
/// places a first window, with keyboard interactivity `keyboard`.
fn in_the_corner(keyboard: KeyboardInteractivity) -> LayerSpec {
    LayerSpec {
        anchor: LayerAnchor::Bottom | LayerAnchor::Right,
        keyboard,
        ..LayerSpec::floating(Layer::Top, (200, 100))
    }
}

/// `spec` with `exclusive` keyboard interactivity.
fn exclusive(spec: LayerSpec) -> LayerSpec {
    LayerSpec {
        keyboard: KeyboardInteractivity::Exclusive,
        ..spec
    }
}

#[test]
fn an_exclusive_overlay_surface_takes_the_keyboard_and_gives_it_back_on_unmap() {
    let (mut f, id, window) = with_a_focused_window();

    let layer = f.map_layer(
        id,
        exclusive(LayerSpec::floating(Layer::Overlay, (200, 100))),
        BAR_COLOR,
    );
    assert_snapshot!(keyboard_events(&mut f, id), @"
    keyboard leaves window 0
    keyboard enters layer surface 0
    ");
    // The window keeps policy's focus, so its frame stays drawn focused.
    assert_eq!(f.state.camino.policy.focus(SEAT), Some(f.pid(window)));

    f.client(id).unmap_layer_surface(layer);
    assert_snapshot!(keyboard_events(&mut f, id), @"
    keyboard leaves layer surface 0
    keyboard enters window 0
    ");
}

on_both_kinds! {
    fn an_exclusive_top_surface_under_a_fullscreen_window_leaves_it_the_keyboard(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let shell = f.connect(Kind::Xdg);
        let window = f.map(client, WindowSpec::plain((400, 300)));
        f.map_layer(
            shell,
            exclusive(LayerSpec::floating(Layer::Top, (200, 100))),
            BAR_COLOR,
        );
        assert!(!f.has_keyboard(window));

        make_fullscreen(&mut f, window);

        assert!(f.has_keyboard(window));
    }
}

on_both_kinds! {
    fn an_on_demand_surface_takes_the_keyboard_on_a_click_and_a_click_on_a_window_gives_it_back(
        kind: Kind
    ) {
        let (mut f, client) = Fixture::connected(kind);
        let shell = f.connect(Kind::Xdg);
        f.client(shell).bind_input();
        let window = f.map(client, WindowSpec::plain((400, 300)));
        let inside = f.geometry(window).loc + Point::from((200, 150));

        f.map_layer(
            shell,
            in_the_corner(KeyboardInteractivity::OnDemand),
            BAR_COLOR,
        );
        assert_eq!(keyboard_events(&mut f, shell), "");
        assert!(f.has_keyboard(window));

        f.click(1900., 1060.);
        assert_eq!(keyboard_events(&mut f, shell), "keyboard enters layer surface 0");
        assert!(!f.has_keyboard(window));
        assert_eq!(f.state.camino.policy.focus(SEAT), Some(f.pid(window)));

        f.click(f64::from(inside.x), f64::from(inside.y));
        assert_eq!(keyboard_events(&mut f, shell), "keyboard leaves layer surface 0");
        assert!(f.has_keyboard(window));
    }
}

#[test]
fn a_surface_with_no_keyboard_interactivity_never_takes_the_keyboard() {
    let (mut f, id, window) = with_a_focused_window();

    f.map_layer(id, in_the_corner(KeyboardInteractivity::None), BAR_COLOR);
    f.click(1900., 1060.);

    assert_snapshot!(keyboard_events(&mut f, id), @"");
    assert!(f.has_keyboard(window));
}

#[test]
fn a_layer_surfaces_popup_maps_and_takes_a_grab() {
    let (mut f, id, window) = with_a_focused_window();
    let bar = f.map_layer(id, LayerSpec::top_bar(28), BAR_COLOR);
    f.pointer_move(100., 10.);
    f.pointer_button(BTN_LEFT, true);
    f.roundtrip(id);
    let serial = f.client(id).state.last_press_serial.unwrap();

    // A menu hanging below the bar, as mate-panel's are.
    let menu = Positioner {
        size: (300, 200),
        anchor_rect: (90, 0, 20, 28),
        anchor: Anchor::BottomLeft,
        gravity: Gravity::BottomRight,
        constraint_adjustment: ConstraintAdjustment::None,
    };
    let popup = f.client(id).create_layer_popup(bar, menu, Some(serial));
    f.roundtrip(id);
    f.client(id).map_popup(popup, RED);
    f.pointer_button(BTN_LEFT, false);

    assert_snapshot!(
        format_popup_configures(&f.client(id).popup(popup).configures),
        @"at 90, 28, size: 300 × 200"
    );
    f.wait_for_pixel(100, 100, RED & 0xffffff);
    assert_snapshot!(keyboard_events(&mut f, id), @"
    keyboard leaves window 0
    keyboard enters popup 0
    ");

    // A press outside the client's surfaces ends the grab and dismisses the menu. The bar,
    // the grab's root, hears nothing of the keyboard on its way back to the window.
    f.click(1900., 1060.);
    assert_snapshot!(keyboard_events(&mut f, id), @"
    keyboard leaves popup 0
    keyboard enters window 0
    ");
    assert!(f.client(id).popup(popup).done);
    assert!(f.has_keyboard(window));
}

/// How far from the window's frame the press that opens the desktop's menu is: to its
/// right, and below its top.
const BESIDE: (i32, i32) = (20, 100);

/// A red 200 × 200 menu of caja's desktop, which is blue, that lies over the right side of
/// a window of `kind`'s client. A press on the desktop beside the window opened it, as
/// caja's is opened. Returns the desktop's client, the menu and the window.
fn a_desktop_menu_over_a_window(kind: Kind) -> (Fixture, ClientId, PopupId, ClientWindow) {
    let (mut f, client) = Fixture::connected(kind);
    let shell = f.connect(Kind::Xdg);
    f.client(shell).bind_input();
    let desktop = f.map_layer(shell, LayerSpec::covering(Layer::Bottom, "desktop"), BLUE);
    let window = f.map(client, WindowSpec::plain((400, 300)));
    let frame = f.visible_frame(window);
    let beside = frame.loc + Point::from((frame.size.w + BESIDE.0, BESIDE.1));
    f.pointer_move(f64::from(beside.x), f64::from(beside.y));
    f.pointer_button(BTN_LEFT, true);
    f.roundtrip(shell);
    let serial = f.client(shell).state.last_press_serial.unwrap();

    // The desktop covers the output, so its coordinates are the output's.
    let menu = Positioner {
        size: (200, 200),
        anchor_rect: (beside.x, beside.y, 1, 1),
        anchor: Anchor::TopLeft,
        gravity: Gravity::BottomLeft,
        constraint_adjustment: ConstraintAdjustment::None,
    };
    let popup = f
        .client(shell)
        .create_layer_popup(desktop, menu, Some(serial));
    f.roundtrip(shell);
    f.client(shell).map_popup(popup, RED);
    f.pointer_button(BTN_LEFT, false);
    f.roundtrip(shell);
    (f, shell, popup, window)
}

/// A point of `a_desktop_menu_over_a_window`'s window that its menu lies over.
fn under_the_menu(f: &Fixture, window: ClientWindow) -> Point<i32, Logical> {
    let frame = f.visible_frame(window);
    frame.loc + Point::from((frame.size.w - 50, BESIDE.1 + 100))
}

on_both_kinds! {
    fn a_bottom_surfaces_popup_is_above_a_window_it_overlaps_and_takes_a_press_there(kind: Kind) {
        let (mut f, shell, popup, window) = a_desktop_menu_over_a_window(kind);
        let covered = under_the_menu(&f, window);

        assert_eq!(
            scene_layers(&mut f),
            [
                SceneLayer::Cursor,
                SceneLayer::LayerPopups,
                SceneLayer::Windows,
                SceneLayer::Bottom,
            ]
        );
        f.wait_for_pixel(covered.x, covered.y, RED & 0xffffff);

        f.client(shell).state.input.clear();
        f.click(f64::from(covered.x), f64::from(covered.y));
        f.roundtrip(shell);
        // The press is the menu's, and so didn't dismiss it.
        let input = &f.client(shell).state.input;
        assert!(input.contains(&InputEvent::Button { button: BTN_LEFT, pressed: true }));
        assert!(input.iter().any(|event| matches!(
            event,
            InputEvent::PointerEnter { surface, .. } if *surface == SurfaceRef::Popup(popup)
        )));
        assert!(!f.client(shell).popup(popup).done);
    }
}

on_both_kinds! {
    fn a_fullscreen_window_with_the_focus_covers_a_bottom_surfaces_popup(kind: Kind) {
        let (mut f, _, _, window) = a_desktop_menu_over_a_window(kind);

        make_fullscreen(&mut f, window);

        assert_eq!(
            scene_layers(&mut f),
            [
                SceneLayer::Cursor,
                SceneLayer::FullscreenWindows,
                SceneLayer::LayerPopups,
                SceneLayer::Bottom,
            ]
        );
    }
}

on_both_kinds! {
    fn a_top_surfaces_popup_stays_in_the_top_layer(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let shell = f.connect(Kind::Xdg);
        let bar = f.map_layer(shell, LayerSpec::top_bar(28), BAR_COLOR);
        f.map(client, WindowSpec::plain((400, 300)));
        let menu = Positioner {
            size: (300, 200),
            anchor_rect: (90, 0, 20, 28),
            anchor: Anchor::BottomLeft,
            gravity: Gravity::BottomRight,
            constraint_adjustment: ConstraintAdjustment::None,
        };

        let popup = f.client(shell).create_layer_popup(bar, menu, None);
        f.roundtrip(shell);
        f.client(shell).map_popup(popup, RED);

        f.wait_for_pixel(100, 100, RED & 0xffffff);
        assert_eq!(
            scene_layers(&mut f),
            [SceneLayer::Cursor, SceneLayer::Top, SceneLayer::Windows]
        );
    }
}
