//! The Popup slide (ADR 0037): an xdg_popup that's shown slides out of the edge it's
//! attached to over 200 ms of Caminito's clock, cut off at its own final rectangle, and its
//! picture slides back as it closes, read from the debug dump and from the output's pixels.
//! Policy has it at its real place from the start, and lets it go at once as it closes.
//!
//! Only Wayland clients have xdg_popups: an X11 menu is an override-redirect window, whose
//! slide is `override_redirect_slide`'s.

use std::time::Duration;

use caminito_test_client::layer_shell::{KeyboardInteractivity, Layer, LayerAnchor, LayerSpec};
use caminito_test_client::{Anchor, ConstraintAdjustment, Gravity, Positioner, format_input};
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{BTN_LEFT, ClientId, ClientWindow, Fixture, Kind, WINDOW_COLOR, WindowSpec};
use super::overview;
use crate::popup_slide::LENGTH;
use crate::settings::Value as Setting;
use crate::settings::translate::{CAMINITO, INTERFACE, MARCO_GENERAL};

const POPUP_COLOR: u32 = 0xffff_0000;

/// The colours as `0xRRGGBB`.
const POPUP: u32 = POPUP_COLOR & 0xff_ffff;
const WINDOW: u32 = WINDOW_COLOR & 0xff_ffff;

/// How far into the slide the tests read it.
pub(super) const PART_WAY: Duration = Duration::from_millis(50);

/// The rectangle in the window the popups are anchored to, from its window geometry's corner.
const ANCHOR: (i32, i32, i32, i32) = (150, 100, 100, 20);

type Settings = Vec<(&'static str, String, Setting)>;

/// One output with `popup-slide` on, unless `settings` has it, and a Wayland client. The
/// fixture has the key off, for the tests written when a popup was there at once. The clock
/// is stopped, so the slide is read where `advance_clock` has it.
fn desk(mut settings: Settings) -> (Fixture, ClientId) {
    if !settings.iter().any(|(_, key, _)| key == "popup-slide") {
        settings.push((CAMINITO, "popup-slide".into(), Setting::Bool(true)));
    }
    let mut f = Fixture::with_settings(settings);
    f.add_output((1280, 800), 1.0);
    let id = f.connect(Kind::Xdg);
    f.stop_clock();
    (f, id)
}

/// A Popup slide as the debug dump's `popup_slides` has it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Sliding {
    pub rect: Rectangle<i32, Logical>,
    pub edge: String,
    pub offset: Point<i32, Logical>,
    pub alpha: f64,
}

/// Each Popup slide that runs, from the debug dump, which has none while none does.
pub(super) fn sliding(f: &Fixture) -> Vec<Sliding> {
    dumped(f, "popup_slides")
}

/// Each closed popup's picture that slides back, from the debug dump.
pub(super) fn closing(f: &Fixture) -> Vec<Sliding> {
    dumped(f, "closing_popups")
}

/// The one closed popup's picture that slides back.
#[track_caller]
pub(super) fn the_closing(f: &Fixture) -> Sliding {
    match &closing(f)[..] {
        [closing] => closing.clone(),
        other => panic!("one popup slides back: {other:?}"),
    }
}

/// The slides the debug dump has under `key`.
fn dumped(f: &Fixture, key: &str) -> Vec<Sliding> {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    let int = |value: &serde_json::Value| value.as_i64().unwrap() as i32;
    let Some(popups) = dump[key].as_array() else {
        return Vec::new();
    };
    let popups = popups.iter().map(|popup| {
        let rect = &popup["rect"];
        Sliding {
            rect: Rectangle::new(
                (int(&rect["x"]), int(&rect["y"])).into(),
                (int(&rect["width"]), int(&rect["height"])).into(),
            ),
            edge: popup["edge"].as_str().unwrap().to_owned(),
            offset: (int(&popup["offset"]["x"]), int(&popup["offset"]["y"])).into(),
            alpha: popup["alpha"].as_f64().unwrap(),
        }
    });
    popups.collect()
}

/// The one popup that slides.
#[track_caller]
pub(super) fn the_slide(f: &Fixture) -> Sliding {
    match &sliding(f)[..] {
        [slide] => slide.clone(),
        other => panic!("one popup slides: {other:?}"),
    }
}

/// A `size` popup placed at `anchor` and `gravity` on `ANCHOR`, as it is wherever it falls.
fn place(size: (i32, i32), anchor: Anchor, gravity: Gravity) -> Positioner {
    Positioner {
        size,
        anchor_rect: ANCHOR,
        anchor,
        gravity,
        constraint_adjustment: ConstraintAdjustment::None,
    }
}

/// A 400 × 300 window with a popup placed by `positioner`, which is shown, and the window's
/// corner.
fn window_with_popup(
    f: &mut Fixture,
    id: ClientId,
    positioner: Positioner,
) -> (caminito_test_client::PopupId, Point<i32, Logical>) {
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let corner = f.geometry(window).loc;
    let popup = f.client(id).create_popup(window.xdg(), positioner);
    f.roundtrip(id);
    f.client(id).map_popup(popup, POPUP_COLOR);
    f.roundtrip(id);
    (popup, corner)
}

/// The output's pixel at the global point `at`, as `0xRRGGBB`.
pub(super) fn pixel(f: &mut Fixture, at: Point<i32, Logical>) -> u32 {
    let output = f.state.caminito.outputs[0].clone();
    let pixel = f
        .state
        .backend
        .headless_mut()
        .pixel(&output, at.to_physical(1));
    pixel.expect("the output is drawn") & 0xff_ffff
}

/// The bottom right pixel of `rect`.
pub(super) fn last_pixel(rect: Rectangle<i32, Logical>) -> Point<i32, Logical> {
    rect.loc + rect.size.to_point() - Point::from((1, 1))
}

#[test]
fn a_popup_slides_out_of_the_side_of_its_anchor_it_lies_on() {
    for (anchor, edge, start) in [
        (Anchor::Bottom, "top", (0, -60)),
        (Anchor::Top, "bottom", (0, 60)),
        (Anchor::Right, "left", (-80, 0)),
        (Anchor::Left, "right", (80, 0)),
    ] {
        let (mut f, id) = desk(Vec::new());
        // Gravity the anchor's way puts the popup on that side of the anchor.
        let gravity = match anchor {
            Anchor::Bottom => Gravity::Bottom,
            Anchor::Top => Gravity::Top,
            Anchor::Right => Gravity::Right,
            _ => Gravity::Left,
        };
        window_with_popup(&mut f, id, place((80, 60), anchor, gravity));

        let slide = the_slide(&f);
        assert_eq!(slide.edge, edge, "{anchor:?}");
        assert_eq!(
            slide.offset,
            start.into(),
            "its whole size away: {anchor:?}"
        );
        assert_eq!(slide.alpha, 1.0, "a short popup doesn't fade: {anchor:?}");

        f.advance_clock(PART_WAY);
        let part_way = the_slide(&f);
        let (from, now): (Point<i32, Logical>, _) = (start.into(), part_way.offset);
        assert!(
            now.x.signum() == from.x.signum()
                && now.y.signum() == from.y.signum()
                && now.x.abs() < from.x.abs().max(1)
                && now.y.abs() < from.y.abs().max(1),
            "on its way: {now:?} from {from:?}, {anchor:?}"
        );
        assert_eq!(part_way.rect, slide.rect, "its place doesn't move");

        f.advance_clock(LENGTH - PART_WAY);
        assert!(sliding(&f).is_empty(), "over at 200 ms: {anchor:?}");
        assert_eq!(pixel(&mut f, slide.rect.loc), POPUP, "{anchor:?}");
        assert_eq!(pixel(&mut f, last_pixel(slide.rect)), POPUP, "{anchor:?}");
    }
}

#[test]
fn a_combo_box_list_over_its_anchor_slides_down() {
    let (mut f, id) = desk(Vec::new());
    // Over its button, as GTK places a combo box's list.
    let list = Positioner {
        anchor_rect: ANCHOR,
        ..place((100, 80), Anchor::TopLeft, Gravity::BottomRight)
    };
    window_with_popup(&mut f, id, list);

    assert_eq!(the_slide(&f).edge, "top");
}

#[test]
fn a_submenu_slides_out_of_its_menus_side() {
    let (mut f, id) = desk(Vec::new());
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let corner = f.geometry(window).loc;
    f.click(f64::from(corner.x + 10), f64::from(corner.y + 10));
    f.roundtrip(id);
    let press = f.client(id).state.last_press_serial.unwrap();
    let menu = place((100, 120), Anchor::BottomLeft, Gravity::BottomRight);
    let menu = f.client(id).create_popup(window.xdg(), menu);
    f.client(id).grab_popup(menu, press);
    f.roundtrip(id);
    f.client(id).map_popup(menu, POPUP_COLOR);
    f.roundtrip(id);
    f.advance_clock(LENGTH);

    // Beside its second row, as a menu's submenu is.
    let row = Positioner {
        size: (100, 80),
        anchor_rect: (0, 20, 100, 20),
        anchor: Anchor::TopRight,
        gravity: Gravity::BottomRight,
        constraint_adjustment: ConstraintAdjustment::None,
    };
    let submenu = f.client(id).create_nested_popup(menu, row, press);
    f.roundtrip(id);
    f.client(id).map_popup(submenu, POPUP_COLOR);
    f.roundtrip(id);

    assert!(!f.client(id).popup(submenu).done);
    let slide = the_slide(&f);
    assert_eq!(slide.edge, "left", "it slides right, out of the menu");
    assert_eq!(slide.offset, (-100, 0).into());
}

#[test]
fn a_bottom_panels_popup_slides_up() {
    let (mut f, id) = desk(Vec::new());
    let bottom_bar = LayerSpec {
        layer: Layer::Top,
        namespace: "panel",
        output: None,
        anchor: LayerAnchor::Bottom | LayerAnchor::Left | LayerAnchor::Right,
        size: (0, 28),
        exclusive_zone: 28,
        keyboard: KeyboardInteractivity::None,
    };
    let bar = f.map_layer(id, bottom_bar, 0xff00_00ff);
    // Above the bar, as mate-panel's menus are on a bottom panel.
    let menu = Positioner {
        size: (100, 120),
        anchor_rect: (90, 0, 20, 28),
        anchor: Anchor::TopLeft,
        gravity: Gravity::TopRight,
        constraint_adjustment: ConstraintAdjustment::None,
    };
    let popup = f.client(id).create_layer_popup(bar, menu, None);
    f.roundtrip(id);
    f.client(id).map_popup(popup, POPUP_COLOR);
    f.roundtrip(id);

    let slide = the_slide(&f);
    assert_eq!(slide.edge, "bottom");
    assert_eq!(
        slide.rect,
        Rectangle::new((90, 800 - 28 - 120).into(), (100, 120).into())
    );
    assert_eq!(slide.offset, (0, 120).into());
}

#[test]
fn a_tall_popup_slides_eight_lines_and_fades_and_a_short_one_doesnt_fade() {
    let (mut f, id) = desk(Vec::new());
    window_with_popup(
        &mut f,
        id,
        place((100, 60), Anchor::Bottom, Gravity::Bottom),
    );
    let short = the_slide(&f);
    assert_eq!((short.offset, short.alpha), ((0, -60).into(), 1.0));

    let (mut f, id) = desk(Vec::new());
    window_with_popup(
        &mut f,
        id,
        place((100, 500), Anchor::Bottom, Gravity::Bottom),
    );
    let tall = the_slide(&f);
    assert_eq!(tall.alpha, 0.0, "it fades in from nothing");
    let lines = -tall.offset.y;
    assert!(
        lines > 0 && lines < 500 && lines % 8 == 0,
        "eight lines of the interface font: {lines}"
    );
    f.advance_clock(PART_WAY);
    let alpha = the_slide(&f).alpha;
    assert!(alpha > 0.0 && alpha < 1.0, "fainter part-way: {alpha}");
}

#[test]
fn part_way_nothing_of_a_popup_is_drawn_outside_its_place() {
    let (mut f, id) = desk(Vec::new());
    window_with_popup(&mut f, id, place((80, 60), Anchor::Bottom, Gravity::Bottom));
    f.advance_clock(PART_WAY);
    let slide = the_slide(&f);
    let place = slide.rect;
    assert!(slide.offset.y < 0, "it comes down: {slide:?}");

    // Its top rows are drawn at the top of its place, and none above it, on its anchor.
    let above = place.loc - Point::from((0, 1));
    assert_eq!(pixel(&mut f, place.loc), POPUP);
    assert_eq!(pixel(&mut f, above), WINDOW, "nothing over its parent");
    let left = place.loc + Point::from((-1, 5));
    let right = place.loc + Point::from((place.size.w, 5));
    assert_eq!(pixel(&mut f, left), WINDOW);
    assert_eq!(pixel(&mut f, right), WINDOW);
    // Its bottom rows aren't out yet.
    assert_eq!(pixel(&mut f, last_pixel(place)), WINDOW);
}

#[test]
fn a_reposition_part_way_keeps_the_edge_and_the_way_done() {
    let (mut f, id) = desk(Vec::new());
    let (popup, corner) =
        window_with_popup(&mut f, id, place((80, 60), Anchor::Bottom, Gravity::Bottom));
    f.advance_clock(PART_WAY);
    let before = the_slide(&f);

    let moved = Positioner {
        anchor_rect: (20, 150, 100, 20),
        ..place((80, 60), Anchor::Bottom, Gravity::Bottom)
    };
    f.client(id).reposition_popup(popup, moved, 1);
    f.roundtrip(id);
    f.client(id).map_popup(popup, POPUP_COLOR);
    f.roundtrip(id);

    let after = the_slide(&f);
    assert_eq!(after.edge, before.edge);
    assert_eq!(after.offset, before.offset);
    assert_eq!(after.rect.loc, corner + Point::from((30, 170)));
}

// What a client is sent is read from a Wayland client, as `input.rs` reads it.
#[test]
fn a_click_part_way_on_a_row_at_its_real_place_reaches_it() {
    let (mut f, id) = desk(Vec::new());
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let corner = f.geometry(window).loc;
    f.click(f64::from(corner.x + 10), f64::from(corner.y + 10));
    f.roundtrip(id);
    let press = f.client(id).state.last_press_serial.unwrap();
    let menu = f.client(id).create_popup(
        window.xdg(),
        place((80, 60), Anchor::Bottom, Gravity::Bottom),
    );
    f.client(id).grab_popup(menu, press);
    f.roundtrip(id);
    f.client(id).map_popup(menu, POPUP_COLOR);
    f.roundtrip(id);
    f.advance_clock(PART_WAY);

    // Its last row, which isn't drawn there yet.
    let row = last_pixel(the_slide(&f).rect) - Point::from((40, 0));
    assert_ne!(pixel(&mut f, row), POPUP);
    f.client(id).state.input.clear();
    f.pointer_move(f64::from(row.x), f64::from(row.y));
    f.pointer_button(BTN_LEFT, true);
    f.pointer_button(BTN_LEFT, false);
    f.roundtrip(id);

    let input = format_input(&f.client(id).state.input);
    assert!(
        input.contains("pointer enters popup 0 at 39, 59"),
        "{input}"
    );
    assert!(
        input.contains(&format!("button {BTN_LEFT:#x} pressed")),
        "{input}"
    );
}

#[test]
fn with_its_key_or_one_of_the_minimise_animations_three_against_it_a_popup_is_there_at_once() {
    for (schema, key, value) in [
        (CAMINITO, "popup-slide", false),
        (MARCO_GENERAL, "compositing-manager", false),
        (MARCO_GENERAL, "reduced-resources", true),
        (INTERFACE, "enable-animations", false),
    ] {
        let (mut f, id) = desk(vec![(schema, key.into(), Setting::Bool(value))]);
        let (_, corner) =
            window_with_popup(&mut f, id, place((80, 60), Anchor::Bottom, Gravity::Bottom));

        assert!(sliding(&f).is_empty(), "{key}: {:?}", sliding(&f));
        // Centred below the anchor: its bottom right pixel.
        let last = corner + Point::from((200 + 40 - 1, 120 + 60 - 1));
        assert_eq!(pixel(&mut f, last), POPUP, "{key}");
    }
}

#[test]
fn no_popup_slides_in_the_overview() {
    let (mut f, id) = desk(Vec::new());
    let window = f.map(id, WindowSpec::plain((400, 300)));
    overview::open(&mut f);

    let popup = f.client(id).create_popup(
        window.xdg(),
        place((80, 60), Anchor::Bottom, Gravity::Bottom),
    );
    f.roundtrip(id);
    f.client(id).map_popup(popup, POPUP_COLOR);
    f.roundtrip(id);

    assert!(sliding(&f).is_empty(), "{:?}", sliding(&f));
}

#[test]
fn no_popup_slides_under_the_lock_and_the_lock_ends_a_slide() {
    let (mut f, id) = desk(Vec::new());
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let menu = place((80, 60), Anchor::Bottom, Gravity::Bottom);
    let popup = f.client(id).create_popup(window.xdg(), menu);
    f.roundtrip(id);
    f.client(id).map_popup(popup, POPUP_COLOR);
    f.roundtrip(id);
    assert_eq!(sliding(&f).len(), 1);

    let locker = f.connect(Kind::Xdg);
    f.client(locker).lock_session();
    f.roundtrip(locker);
    assert!(f.state.caminito.lock.is_locked());
    assert!(
        sliding(&f).is_empty(),
        "the lock ends it: {:?}",
        sliding(&f)
    );

    let popup = f.client(id).create_popup(
        window.xdg(),
        place((80, 60), Anchor::Bottom, Gravity::Bottom),
    );
    f.roundtrip(id);
    f.client(id).map_popup(popup, POPUP_COLOR);
    f.roundtrip(id);
    assert!(sliding(&f).is_empty(), "{:?}", sliding(&f));
}

#[test]
fn a_popup_of_a_window_on_its_way_open_is_inside_its_zoom_and_doesnt_slide() {
    let open_close = (CAMINITO, "open-close-animation".into(), Setting::Bool(true));
    let (mut f, id) = desk(vec![open_close]);
    window_with_popup(&mut f, id, place((80, 60), Anchor::Bottom, Gravity::Bottom));

    assert!(sliding(&f).is_empty(), "{:?}", sliding(&f));
}

/// A window with a menu below `ANCHOR` that the window's client grabbed with a click on it,
/// shown and slid out, with the click's serial.
fn window_with_menu(
    f: &mut Fixture,
    id: ClientId,
) -> (ClientWindow, caminito_test_client::PopupId, u32) {
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let corner = f.geometry(window).loc;
    f.click(f64::from(corner.x + 10), f64::from(corner.y + 10));
    f.roundtrip(id);
    let press = f.client(id).state.last_press_serial.unwrap();
    let menu = f.client(id).create_popup(
        window.xdg(),
        place((80, 60), Anchor::Bottom, Gravity::Bottom),
    );
    f.client(id).grab_popup(menu, press);
    f.roundtrip(id);
    f.client(id).map_popup(menu, POPUP_COLOR);
    f.roundtrip(id);
    f.advance_clock(LENGTH);
    (window, menu, press)
}

/// Checks that `slide` is on its way back into its `top` edge from `place`, up.
#[track_caller]
fn going_back_up(slide: &Sliding, place: Rectangle<i32, Logical>) {
    assert_eq!(slide.rect, place);
    assert_eq!(slide.edge, "top");
    assert!(
        slide.offset.x == 0 && slide.offset.y < 0 && slide.offset.y > -60,
        "part-way back: {slide:?}"
    );
}

#[test]
fn a_popup_its_client_destroys_slides_back_into_its_edge() {
    let (mut f, id) = desk(Vec::new());
    let (popup, _) =
        window_with_popup(&mut f, id, place((80, 60), Anchor::Bottom, Gravity::Bottom));
    f.advance_clock(LENGTH);
    let place = rest_place(&f);

    f.client(id).destroy_popup(popup);
    f.roundtrip(id);
    let start = the_closing(&f);
    assert_eq!((start.rect, start.offset), (place, (0, 0).into()));
    assert_eq!(start.alpha, 1.0, "a short popup doesn't fade");
    assert_eq!(
        pixel(&mut f, last_pixel(place)),
        POPUP,
        "its picture is there"
    );

    f.advance_clock(PART_WAY);
    let part_way = the_closing(&f);
    going_back_up(&part_way, place);
    // Its bottom rows have gone back up, and its top rows are drawn at the top of its place.
    assert_eq!(pixel(&mut f, last_pixel(place)), WINDOW);
    assert_eq!(pixel(&mut f, place.loc), POPUP);
    assert_eq!(
        pixel(&mut f, place.loc - Point::from((0, 1))),
        WINDOW,
        "cut off"
    );

    f.advance_clock(LENGTH - PART_WAY);
    assert!(closing(&f).is_empty(), "gone at 200 ms: {:?}", closing(&f));
    assert_eq!(pixel(&mut f, place.loc), WINDOW);
}

/// Where the one window's popup centred below `ANCHOR` is, at rest, globally.
#[track_caller]
fn rest_place(f: &Fixture) -> Rectangle<i32, Logical> {
    assert!(sliding(f).is_empty(), "at rest: {:?}", sliding(f));
    let policy = &f.state.caminito.policy;
    let corner = policy.geometry(policy.stacking_all()[0]).unwrap().loc;
    Rectangle::new(corner + Point::from((160, 120)), (80, 60).into())
}

#[test]
fn a_popup_dismissed_by_a_click_outside_slides_back_and_its_grab_has_ended() {
    let (mut f, id) = desk(Vec::new());
    let (_, menu, _) = window_with_menu(&mut f, id);
    let place = rest_place(&f);

    f.click(1270.0, 790.0);
    f.roundtrip(id);
    assert!(f.client(id).popup(menu).done, "popup_done");
    assert_eq!(the_closing(&f).offset, (0, 0).into());

    f.advance_clock(PART_WAY);
    going_back_up(&the_closing(&f), place);
    f.advance_clock(LENGTH - PART_WAY);
    assert!(closing(&f).is_empty());
}

// What a client is sent is read from a Wayland client, as `input.rs` reads it.
#[test]
fn a_click_part_way_on_a_closing_picture_reaches_the_window_beneath() {
    let (mut f, id) = desk(Vec::new());
    let (_, menu, _) = window_with_menu(&mut f, id);
    let place = rest_place(&f);
    f.client(id).destroy_popup(menu);
    f.roundtrip(id);
    f.advance_clock(PART_WAY);
    assert_eq!(
        pixel(&mut f, place.loc),
        POPUP,
        "the picture is drawn there"
    );

    f.client(id).state.input.clear();
    f.pointer_move(f64::from(place.loc.x), f64::from(place.loc.y));
    f.pointer_button(BTN_LEFT, true);
    f.pointer_button(BTN_LEFT, false);
    f.roundtrip(id);

    // The pointer was on the window, under the popup's grab, so it's there still.
    let input = format_input(&f.client(id).state.input);
    assert!(!input.contains("popup"), "{input}");
    assert!(
        input.contains(&format!("button {BTN_LEFT:#x} pressed")),
        "{input}"
    );
}

#[test]
fn a_menu_and_its_open_submenu_dismissed_together_slide_back_together() {
    let (mut f, id) = desk(Vec::new());
    let (_, menu, press) = window_with_menu(&mut f, id);
    let row = Positioner {
        size: (100, 80),
        anchor_rect: (0, 20, 80, 20),
        anchor: Anchor::TopRight,
        gravity: Gravity::BottomRight,
        constraint_adjustment: ConstraintAdjustment::None,
    };
    let submenu = f.client(id).create_nested_popup(menu, row, press);
    f.roundtrip(id);
    f.client(id).map_popup(submenu, POPUP_COLOR);
    f.roundtrip(id);
    f.advance_clock(LENGTH);

    f.click(1270.0, 790.0);
    f.roundtrip(id);
    assert!(f.client(id).popup(menu).done && f.client(id).popup(submenu).done);
    let edges: Vec<String> = closing(&f).into_iter().map(|c| c.edge).collect();
    assert_eq!(edges.len(), 2, "{edges:?}");
    assert!(edges.contains(&"top".to_owned()) && edges.contains(&"left".to_owned()));

    f.advance_clock(PART_WAY);
    let part_way = closing(&f);
    assert_eq!(part_way.len(), 2);
    for slide in &part_way {
        let back = match slide.edge.as_str() {
            "top" => slide.offset.y < 0 && slide.offset.x == 0,
            _ => slide.offset.x < 0 && slide.offset.y == 0,
        };
        assert!(back, "each into its own edge: {slide:?}");
    }
    f.advance_clock(LENGTH - PART_WAY);
    assert!(closing(&f).is_empty());
}

#[test]
fn a_popup_closed_on_its_way_out_turns_round_from_where_it_is() {
    let (mut f, id) = desk(Vec::new());
    let (popup, _) =
        window_with_popup(&mut f, id, place((80, 60), Anchor::Bottom, Gravity::Bottom));
    f.advance_clock(PART_WAY);
    let out = the_slide(&f);

    f.client(id).destroy_popup(popup);
    f.roundtrip(id);
    let back = the_closing(&f);
    assert!(sliding(&f).is_empty());
    assert_eq!(back.offset, out.offset, "no jump");
    assert_eq!(back.rect, out.rect);

    // At the same speed: back at its edge after as long as it took to get there.
    f.advance_clock(PART_WAY / 2);
    let further = the_closing(&f).offset.y;
    assert!(further < out.offset.y && further > -60, "{further}");
    f.advance_clock(PART_WAY / 2);
    assert!(closing(&f).is_empty(), "{:?}", closing(&f));
}

#[test]
fn a_popup_of_a_closing_window_leaves_inside_its_closing_picture() {
    let open_close = (CAMINITO, "open-close-animation".into(), Setting::Bool(true));
    let (mut f, id) = desk(vec![open_close]);
    let window = f.map(id, WindowSpec::plain((400, 300)));
    f.advance_clock(LENGTH);
    let popup = f.client(id).create_popup(
        window.xdg(),
        place((80, 60), Anchor::Bottom, Gravity::Bottom),
    );
    f.roundtrip(id);
    f.client(id).map_popup(popup, POPUP_COLOR);
    f.roundtrip(id);
    f.advance_clock(LENGTH);
    let place = rest_place(&f);

    f.client(id).destroy_popup(popup);
    f.client(id).destroy_window(window.xdg());
    f.roundtrip(id);

    assert!(
        closing(&f).is_empty(),
        "no slide of its own: {:?}",
        closing(&f)
    );
    assert!(
        f.state.caminito.state_dump().contains("closing_windows"),
        "the window closes"
    );
    assert_eq!(pixel(&mut f, last_pixel(place)), POPUP, "in its picture");
}

/// A desk with a popup that its client destroyed after it slid out, whose picture slides
/// back.
fn closing_popup(f: &mut Fixture, id: ClientId) {
    let (popup, _) = window_with_popup(f, id, place((80, 60), Anchor::Bottom, Gravity::Bottom));
    f.advance_clock(LENGTH);
    f.client(id).destroy_popup(popup);
    f.roundtrip(id);
    assert_eq!(closing(f).len(), 1);
}

#[test]
fn the_lock_ends_a_slide_back() {
    let (mut f, id) = desk(Vec::new());
    closing_popup(&mut f, id);

    let locker = f.connect(Kind::Xdg);
    f.client(locker).lock_session();
    f.roundtrip(locker);

    assert!(closing(&f).is_empty(), "{:?}", closing(&f));
}

#[test]
fn the_overview_opening_drops_a_closing_picture() {
    let (mut f, id) = desk(Vec::new());
    closing_popup(&mut f, id);

    overview::open(&mut f);

    assert!(closing(&f).is_empty(), "{:?}", closing(&f));
}

#[test]
fn an_output_that_goes_drops_the_closing_pictures_on_it() {
    let (mut f, id) = desk(Vec::new());
    // Another output, which the window goes to.
    f.add_output((1280, 800), 1.0);
    closing_popup(&mut f, id);

    let first = f.state.caminito.outputs[0].clone();
    f.unplug(&first);

    assert!(closing(&f).is_empty(), "{:?}", closing(&f));
}

#[test]
fn with_the_slide_off_a_popup_that_closes_is_gone_in_one_frame() {
    let off = (CAMINITO, "popup-slide".into(), Setting::Bool(false));
    let (mut f, id) = desk(vec![off]);
    let (popup, _) =
        window_with_popup(&mut f, id, place((80, 60), Anchor::Bottom, Gravity::Bottom));
    let place = rest_place(&f);

    f.client(id).destroy_popup(popup);
    f.roundtrip(id);

    assert!(closing(&f).is_empty(), "{:?}", closing(&f));
    assert_eq!(pixel(&mut f, place.loc), WINDOW);
    assert_eq!(pixel(&mut f, last_pixel(place)), WINDOW);
}

// The picture is drawn at the output's scale, so it starts as the popup was, pixel for pixel,
// and not a corner of it blown up: its subsurface in its far corner stays there.
#[test]
fn at_an_output_scale_of_2_a_closing_popups_picture_is_the_whole_popup() {
    let settings = vec![(CAMINITO, "popup-slide".into(), Setting::Bool(true))];
    let mut f = Fixture::with_settings(settings);
    let output = f.add_output((1280, 800), 2.0);
    let id = f.connect(Kind::Xdg);
    f.stop_clock();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let popup = f.client(id).create_popup(
        window.xdg(),
        place((80, 60), Anchor::Bottom, Gravity::Bottom),
    );
    f.roundtrip(id);
    f.client(id)
        .add_popup_subsurface(popup, (60, 40), (20, 20), 0xff00_ff00);
    f.client(id).map_popup(popup, POPUP_COLOR);
    f.roundtrip(id);
    f.advance_clock(LENGTH);
    let place = rest_place(&f);
    let corner = last_pixel(place).to_physical(2);
    let green = |f: &mut Fixture| {
        let pixel = f.state.backend.headless_mut().pixel(&output, corner);
        pixel.expect("the output is drawn") & 0xff_ffff
    };
    f.draw(&output);
    assert_eq!(green(&mut f), 0x00_ff00, "its subsurface at rest");

    f.client(id).destroy_popup(popup);
    f.roundtrip(id);
    f.draw(&output);

    assert_eq!(the_closing(&f).offset, (0, 0).into());
    assert_eq!(green(&mut f), 0x00_ff00, "its subsurface in its picture");
}
