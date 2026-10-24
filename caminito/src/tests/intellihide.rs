//! Intellihide (CMN-224, ADR 0047): a panel that asks for it through
//! `caminito_intellihide_v1` is slid out of its edge while a window overlaps it, and back
//! when none does or the pointer rests at the edge. The panel is never told, so what shows is
//! the debug dump's account of where it is drawn, and the pixels. The policy's rules are
//! tested in `caminito-policy`; these are about the protocol and the slide.

use std::time::Duration;

use caminito_test_client::layer_shell::{
    KeyboardInteractivity, Layer, LayerAnchor, LayerId, LayerSpec,
};
use smithay::utils::Point;

use super::fixture::{
    ClientId, ClientWindow, Fixture, KEY_F10, KEY_LEFTALT, Kind, WINDOW_COLOR, WindowSpec,
};
use crate::input::Under;
use crate::settings::Value as Setting;
use crate::settings::translate::CAMINITO;

const PANEL_COLOR: u32 = 0xff0000ff;
const PANEL: u32 = 0x0000ff;
const WINDOW: u32 = WINDOW_COLOR & 0x00ff_ffff;

/// A panel along the bottom of its output, 28 high, that reserves nothing, as the patched
/// mate-panel's is with Intellihide on.
fn bottom_panel() -> LayerSpec {
    LayerSpec {
        layer: Layer::Top,
        namespace: "panel",
        output: None,
        anchor: LayerAnchor::Bottom | LayerAnchor::Left | LayerAnchor::Right,
        size: (0, 28),
        exclusive_zone: 0,
        keyboard: KeyboardInteractivity::None,
    }
}

/// Caminito with one 1920×1080 output, the Popup slide on or off, and a client. The clock is
/// stopped, so the slide is read where `advance_clock` has it.
fn desk(slide: bool) -> (Fixture, ClientId) {
    let mut f =
        Fixture::with_settings(vec![(CAMINITO, "popup-slide".into(), Setting::Bool(slide))]);
    f.add_output((1920, 1080), 1.0);
    let id = f.connect(Kind::Xdg);
    f.stop_clock();
    f.roundtrip(id);
    (f, id)
}

/// A window high on the output and a panel along its bottom that asks for intellihide.
fn a_window_and_a_panel_with(slide: bool) -> (Fixture, ClientId, ClientWindow, LayerId) {
    let (mut f, id) = desk(slide);
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let panel = f.map_layer(id, bottom_panel(), PANEL_COLOR);
    f.client(id).ask_for_intellihide(panel);
    f.roundtrip(id);
    (f, id, window, panel)
}

fn a_window_and_a_panel() -> (Fixture, ClientId, ClientWindow, LayerId) {
    a_window_and_a_panel_with(true)
}

/// What the dump says of the panel's intellihide: whether it is hidden, and where it is
/// drawn.
fn panel(f: &Fixture) -> (bool, (i64, i64)) {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    let layers = &dump["outputs"][0]["layer_surfaces"];
    let intellihide = &layers[0]["intellihide"];
    let drawn = &intellihide["drawn"];
    (
        intellihide["hidden"]
            .as_bool()
            .expect("the panel asks for intellihide"),
        (drawn["x"].as_i64().unwrap(), drawn["y"].as_i64().unwrap()),
    )
}

/// Maximises the focused window and has its client draw at the work area's size.
fn maximise(f: &mut Fixture, window: ClientWindow) {
    f.chord(&[KEY_LEFTALT, KEY_F10]);
    f.draw_at_asked_size(window, (1920, 1080));
}

/// Lets the slide run its 200 ms.
fn slide(f: &mut Fixture) {
    f.advance_clock(Duration::from_millis(200));
}

#[test]
fn the_compositor_offers_the_protocol() {
    let (mut f, id, _, _) = a_window_and_a_panel();
    f.roundtrip(id);

    assert!(f.client(id).has_intellihide_manager());
}

#[test]
fn a_panel_nothing_overlaps_is_where_it_asked_to_be() {
    let (f, _, _, _) = a_window_and_a_panel();

    assert_eq!(panel(&f), (false, (0, 1052)));
}

#[test]
fn a_maximised_window_hides_an_intellihide_panel_and_the_edge_reveals_it() {
    let (mut f, _, window, _) = a_window_and_a_panel();
    f.pointer_move(960., 500.);
    f.wait_for_pixel(960, 1066, PANEL);

    maximise(&mut f, window);
    slide(&mut f);

    // Out of the output, below its bottom edge, and the window shows where it was.
    assert_eq!(panel(&f), (true, (0, 1080)));
    f.wait_for_pixel(960, 1066, WINDOW);

    // The pointer rests at the edge for a quarter of a second.
    f.pointer_move(960., 1079.);
    f.advance_clock(Duration::from_millis(250));
    slide(&mut f);
    assert_eq!(panel(&f), (false, (0, 1052)));
    f.wait_for_pixel(960, 1066, PANEL);

    // It leaves, and the window still overlaps the panel.
    f.pointer_move(960., 500.);
    slide(&mut f);
    assert_eq!(panel(&f), (true, (0, 1080)));
    f.wait_for_pixel(960, 1066, WINDOW);
}

#[test]
fn unmaximising_shows_the_panel_again() {
    let (mut f, _, window, _) = a_window_and_a_panel();
    f.pointer_move(960., 500.);
    maximise(&mut f, window);
    slide(&mut f);
    assert!(panel(&f).0);

    f.chord(&[KEY_LEFTALT, KEY_F10]);
    f.draw_at_asked_size(window, (400, 300));
    slide(&mut f);

    assert_eq!(panel(&f), (false, (0, 1052)));
}

#[test]
fn the_pointer_resting_less_than_250_ms_reveals_nothing() {
    let (mut f, _, window, _) = a_window_and_a_panel();
    f.pointer_move(960., 500.);
    maximise(&mut f, window);
    slide(&mut f);

    f.pointer_move(960., 1079.);
    f.advance_clock(Duration::from_millis(240));

    assert!(panel(&f).0);
}

#[test]
fn the_panel_slides_over_200_ms_and_a_slide_turned_round_goes_back_from_where_it_is() {
    let (mut f, _, window, _) = a_window_and_a_panel();
    f.pointer_move(960., 500.);
    f.stop_clock();

    maximise(&mut f, window);
    // A slide into the edge is the one out played backwards, so it starts slowly.
    f.advance_clock(Duration::from_millis(150));
    let (hidden, (_, y)) = panel(&f);
    assert!(hidden);
    assert!(
        1052 < y && y < 1080,
        "{y} is between its place and the edge"
    );

    // The window goes back before the panel is out: it turns round.
    f.chord(&[KEY_LEFTALT, KEY_F10]);
    f.draw_at_asked_size(window, (400, 300));
    let (hidden, (_, turned)) = panel(&f);
    assert!(!hidden);
    assert_eq!(turned, y);
    f.advance_clock(Duration::from_millis(50));
    let (_, (_, later)) = panel(&f);
    assert!(later < turned, "{later} is nearer its place than {turned}");
    slide(&mut f);
    assert_eq!(panel(&f), (false, (0, 1052)));
}

#[test]
fn a_hidden_panel_takes_no_pointer() {
    let (mut f, _, window, _) = a_window_and_a_panel();
    f.pointer_move(960., 500.);
    let at = Point::from((960., 1066.));
    assert!(matches!(f.state.caminito.under(at), Some(Under::Layer(..))));

    maximise(&mut f, window);
    slide(&mut f);

    assert!(matches!(
        f.state.caminito.under(at),
        Some(Under::Surface(..))
    ));
}

#[test]
fn a_panel_that_stops_asking_is_shown_where_it_is() {
    let (mut f, id, window, panel_id) = a_window_and_a_panel();
    f.pointer_move(960., 500.);
    maximise(&mut f, window);
    slide(&mut f);
    assert!(panel(&f).0);

    f.client(id).stop_asking_for_intellihide(panel_id);
    f.roundtrip(id);
    f.wait_for_pixel(960, 1066, PANEL);

    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    assert!(
        dump["outputs"][0]["layer_surfaces"][0]
            .get("intellihide")
            .is_none()
    );
}

#[test]
fn a_panel_that_never_asked_is_never_hidden() {
    let (mut f, id) = desk(true);
    let window = f.map(id, WindowSpec::plain((400, 300)));
    f.map_layer(id, bottom_panel(), PANEL_COLOR);
    f.pointer_move(960., 500.);

    maximise(&mut f, window);
    slide(&mut f);

    f.wait_for_pixel(960, 1066, PANEL);
}

#[test]
fn with_the_popup_slide_off_a_panel_goes_at_once() {
    let (mut f, _, window, _) = a_window_and_a_panel_with(false);
    f.pointer_move(960., 500.);

    maximise(&mut f, window);

    assert_eq!(panel(&f), (true, (0, 1080)));
}

#[test]
#[should_panic(expected = "Protocol error 0 on object caminito_intellihide_manager_v1")]
fn a_second_object_for_a_layer_surface_is_a_protocol_error() {
    let (mut f, id, _, panel_id) = a_window_and_a_panel();

    f.client(id).ask_for_intellihide(panel_id);

    f.roundtrip(id);
}
