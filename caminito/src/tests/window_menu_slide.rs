//! The Window menu's Popup slide (ADR 0037, CMN-181): the menu slides down out of where it
//! was asked for, or up when it was turned to fit its output, its submenu out of its
//! parent's side, and each slides back as it closes, read from the debug dump's
//! `popup_slides` and `closing_popups` and from the output's pixels. Policy has the menu at
//! its real place from the start.

use smithay::utils::{Logical, Point};

use super::fixture::{
    BTN_LEFT, BTN_RIGHT, ClientId, ClientWindow, Fixture, KEY_3, KEY_DOWN, KEY_ESC, KEY_LEFT,
    KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, KEY_TAB, KEY_W, KEY_X, WindowSpec,
};
use super::popup_slide::{
    PART_WAY, Sliding, closing, desk, last_pixel, pixel, sliding, the_closing, the_slide,
};
use super::window_menu::{alt_space, has_menu_layer, is_open, menu, rect, tap};
use crate::popup_slide::LENGTH;
use crate::settings::Value as Setting;
use crate::settings::translate::CAMINITO;
use crate::switcher::POPUP_DELAY;

/// One 1280 × 800 output with `popup-slide` on, a stopped clock, and a Wayland window.
fn with_window() -> (Fixture, ClientId, ClientWindow) {
    let (mut f, id) = desk(Vec::new());
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    (f, id, window)
}

/// Asks for `window`'s menu at `at`, globally, as its client does.
fn menu_at(f: &mut Fixture, id: ClientId, window: ClientWindow, at: Point<i32, Logical>) {
    let geometry = f.geometry(window);
    f.pointer_move(
        f64::from(geometry.loc.x + 30),
        f64::from(geometry.loc.y + 10),
    );
    f.pointer_button(BTN_RIGHT, true);
    f.roundtrip(id);
    let serial = f.client(id).state.last_press_serial.unwrap();
    f.pointer_button(BTN_RIGHT, false);
    let from_window = at - geometry.loc;
    f.client(id)
        .show_window_menu(window.xdg(), serial, from_window.x, from_window.y);
    f.roundtrip(id);
    assert!(is_open(f));
}

/// Whether `slide` is on its way from `from`, where it started, to its place: nearer, and
/// from the same side.
#[track_caller]
fn on_its_way(slide: &Sliding, from: Point<i32, Logical>) {
    let now = slide.offset;
    assert!(
        now.x.signum() == from.x.signum()
            && now.y.signum() == from.y.signum()
            && now.x.abs() < from.x.abs().max(1)
            && now.y.abs() < from.y.abs().max(1),
        "on its way: {now:?} from {from:?}"
    );
}

#[test]
fn alt_spaces_menu_slides_down_out_of_where_it_was_asked_for() {
    let (mut f, _, _) = with_window();

    alt_space(&mut f);

    let whole = rect(&menu(&f)["rect"]);
    let start = the_slide(&f);
    assert_eq!(start.rect, whole);
    assert_eq!(start.edge, "top");
    assert!(start.offset.x == 0 && start.offset.y < 0, "{start:?}");
    // Taller than eight lines, it fades in as it slides.
    assert!(-start.offset.y < whole.size.h, "{start:?}");
    assert_eq!(start.alpha, 0.0);

    f.advance_clock(PART_WAY);
    let part_way = the_slide(&f);
    on_its_way(&part_way, start.offset);
    assert!(part_way.alpha > 0.0 && part_way.alpha < 1.0);
    assert_eq!(part_way.rect, whole, "its place doesn't move");

    f.advance_clock(LENGTH - PART_WAY);
    assert!(sliding(&f).is_empty(), "over at 200 ms");
    assert!(has_menu_layer(&mut f));
}

#[test]
fn a_menu_asked_for_near_the_bottom_and_turned_up_slides_up() {
    let (mut f, id, window) = with_window();

    menu_at(&mut f, id, window, (100, 790).into());

    let whole = rect(&menu(&f)["rect"]);
    assert!(whole.loc.y < 790, "turned up to fit: {whole:?}");
    let start = the_slide(&f);
    assert_eq!(start.edge, "bottom");
    assert!(start.offset.x == 0 && start.offset.y > 0, "{start:?}");

    f.advance_clock(PART_WAY);
    on_its_way(&the_slide(&f), start.offset);
    f.advance_clock(LENGTH - PART_WAY);
    assert!(sliding(&f).is_empty());
}

#[test]
fn the_submenu_slides_out_of_its_parents_side() {
    let (mut f, _, _) = with_window();
    alt_space(&mut f);
    f.advance_clock(LENGTH);

    tap(&mut f, KEY_W);

    let sub = rect(&menu(&f)["submenu"]["rect"]);
    let start = the_slide(&f);
    assert_eq!(start.rect, sub);
    assert_eq!(start.edge, "left", "it slides right, out of the menu");
    assert!(start.offset.x < 0 && start.offset.y == 0, "{start:?}");

    f.advance_clock(PART_WAY);
    on_its_way(&the_slide(&f), start.offset);
    f.advance_clock(LENGTH - PART_WAY);
    assert!(sliding(&f).is_empty());
}

#[test]
fn a_submenu_turned_to_the_left_slides_left() {
    let (mut f, id, window) = with_window();
    menu_at(&mut f, id, window, (1270, 100).into());
    f.advance_clock(LENGTH);

    tap(&mut f, KEY_W);

    let whole = rect(&menu(&f)["rect"]);
    let sub = rect(&menu(&f)["submenu"]["rect"]);
    assert!(sub.loc.x < whole.loc.x, "it opens to the left: {sub:?}");
    let start = the_slide(&f);
    assert_eq!(start.edge, "right");
    assert!(start.offset.x > 0, "{start:?}");
}

#[test]
fn an_item_chosen_part_way_open_is_done_at_once_and_the_menu_turns_round() {
    let (mut f, _, window) = with_window();
    let id = f.pid(window);
    alt_space(&mut f);
    f.advance_clock(PART_WAY);
    let out = the_slide(&f);

    tap(&mut f, KEY_X);

    assert!(f.state.caminito.policy.is_maximized(id), "at once");
    assert!(!is_open(&f));
    assert!(sliding(&f).is_empty());
    let back = the_closing(&f);
    assert_eq!(back.rect, out.rect);
    assert_eq!(back.edge, "top");
    assert_eq!(back.offset, out.offset, "from where it was, no jump");
    assert_eq!(back.alpha, out.alpha);

    // Back as fast as it came: 50 ms to its edge.
    f.advance_clock(PART_WAY / 2);
    let halfway = the_closing(&f);
    assert!(halfway.offset.y < out.offset.y, "{halfway:?}");
    f.advance_clock(PART_WAY / 2);
    assert!(closing(&f).is_empty());
    assert!(!has_menu_layer(&mut f));
}

#[test]
fn a_key_part_way_moves_the_selection_and_the_slide_goes_on() {
    let (mut f, _, _) = with_window();
    alt_space(&mut f);
    f.advance_clock(PART_WAY);
    let before = the_slide(&f);

    tap(&mut f, KEY_DOWN);

    assert_eq!(menu(&f)["selected"], 0);
    assert_eq!(the_slide(&f), before, "drawn again, where it was");
    f.advance_clock(PART_WAY);
    on_its_way(&the_slide(&f), before.offset);
}

#[test]
fn escape_and_a_click_outside_slide_the_menu_back() {
    let (mut f, _, _) = with_window();
    alt_space(&mut f);
    f.advance_clock(LENGTH);
    let whole = rect(&menu(&f)["rect"]);

    tap(&mut f, KEY_ESC);

    assert!(!is_open(&f));
    let back = the_closing(&f);
    assert_eq!((back.rect, back.edge.as_str()), (whole, "top"));
    assert_eq!(back.offset, (0, 0).into(), "from its place");
    f.advance_clock(PART_WAY);
    assert!(the_closing(&f).offset.y < 0);
    assert!(has_menu_layer(&mut f), "its picture is drawn");
    f.advance_clock(LENGTH - PART_WAY);
    assert!(closing(&f).is_empty(), "into its edge at 200 ms");

    alt_space(&mut f);
    f.advance_clock(LENGTH);
    f.pointer_move(1200., 700.);
    f.pointer_button(BTN_LEFT, true);
    f.pointer_button(BTN_LEFT, false);
    assert!(!is_open(&f));
    assert_eq!(the_closing(&f).rect, whole);
}

#[test]
fn the_submenu_closing_alone_slides_back_into_its_parent_and_turns_round_when_opened_again() {
    let (mut f, _, _) = with_window();
    alt_space(&mut f);
    tap(&mut f, KEY_W);
    f.advance_clock(LENGTH);
    let sub = rect(&menu(&f)["submenu"]["rect"]);

    tap(&mut f, KEY_LEFT);

    assert!(is_open(&f));
    assert!(menu(&f)["submenu"].is_null());
    let back = the_closing(&f);
    assert_eq!((back.rect, back.edge.as_str()), (sub, "left"));
    assert!(sliding(&f).is_empty(), "the menu stays");

    f.advance_clock(PART_WAY);
    let part_way = the_closing(&f);
    tap(&mut f, KEY_RIGHT);
    assert!(closing(&f).is_empty());
    let out = the_slide(&f);
    assert_eq!(
        out.offset, part_way.offset,
        "it turns round from where it is"
    );
    f.advance_clock(PART_WAY);
    assert!(sliding(&f).is_empty(), "as fast as it went");
}

#[test]
fn a_menu_and_its_open_submenu_chosen_from_slide_back_together() {
    let (mut f, _, _) = with_window();
    alt_space(&mut f);
    tap(&mut f, KEY_W);
    f.advance_clock(LENGTH);
    let whole = rect(&menu(&f)["rect"]);
    let sub = rect(&menu(&f)["submenu"]["rect"]);

    tap(&mut f, KEY_3);

    assert!(!is_open(&f));
    let back: Vec<_> = closing(&f).into_iter().map(|c| (c.rect, c.edge)).collect();
    assert_eq!(
        back,
        [(sub, "left".into()), (whole, "top".into())],
        "each into its own edge"
    );
}

#[test]
fn part_way_nothing_of_the_menu_is_drawn_outside_its_place() {
    let (mut f, _, _) = with_window();
    alt_space(&mut f);
    let whole = rect(&menu(&f)["rect"]);
    tap(&mut f, KEY_ESC);
    f.advance_clock(LENGTH);
    let above = whole.loc + Point::from((20, -1));
    let inside = whole.loc + Point::from((20, 2));
    let bottom = last_pixel(whole);
    let beneath = [above, inside, bottom].map(|at| pixel(&mut f, at));

    alt_space(&mut f);
    f.advance_clock(PART_WAY);

    assert_eq!(
        pixel(&mut f, above),
        beneath[0],
        "nothing over what's above it"
    );
    assert_ne!(pixel(&mut f, inside), beneath[1], "its top is drawn");
    assert_eq!(
        pixel(&mut f, bottom),
        beneath[2],
        "its bottom rows aren't out yet"
    );
}

#[test]
fn the_window_switcher_and_the_workspace_popup_show_at_once() {
    let (mut f, id, _) = with_window();
    f.map(id, WindowSpec::plain((400, 300)));

    f.key(KEY_LEFTALT, true);
    tap(&mut f, KEY_TAB);
    f.advance_clock(POPUP_DELAY);
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    assert!(!dump["switcher"].is_null());
    assert!(sliding(&f).is_empty());
    f.key(KEY_LEFTALT, false);
    assert!(closing(&f).is_empty());

    f.key(KEY_LEFTCTRL, true);
    f.key(KEY_LEFTALT, true);
    tap(&mut f, KEY_RIGHT);
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    assert!(!dump["workspace_popup"].is_null());
    assert!(sliding(&f).is_empty());
    f.key(KEY_LEFTALT, false);
    f.key(KEY_LEFTCTRL, false);
    assert!(closing(&f).is_empty());
}

#[test]
fn with_the_slide_off_the_menu_is_there_and_gone_in_one_frame() {
    let (mut f, id) = desk(vec![(CAMINITO, "popup-slide".into(), Setting::Bool(false))]);
    f.map(id, WindowSpec::default());

    alt_space(&mut f);
    assert!(sliding(&f).is_empty());
    assert!(has_menu_layer(&mut f));
    tap(&mut f, KEY_W);
    assert!(sliding(&f).is_empty());
    tap(&mut f, KEY_LEFT);
    assert!(closing(&f).is_empty());

    tap(&mut f, KEY_ESC);
    assert!(closing(&f).is_empty());
    assert!(!has_menu_layer(&mut f));
}
