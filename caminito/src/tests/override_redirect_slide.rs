//! The Popup slide of override-redirect windows (ADR 0037): an X11 menu or tooltip slides
//! out of the edge away from where the pointer was when it mapped, and its picture slides
//! back as X unmaps or destroys it, read from the debug dump and the output's pixels. A
//! Utility or a DND one doesn't slide. Only X clients make override-redirect windows, so
//! these tests map X11 windows alone; an xdg_popup's slide is `popup_slide`'s.

use std::time::Duration;

use caminito_test_client::x11::XWindowSpec;
use smithay::utils::{Logical, Point, Rectangle};
use x11rb::protocol::Event;
use x11rb::protocol::xproto::GrabStatus;

use super::fixture::{ClientId, Fixture, KEY_A, Kind};
use super::popup_slide::{
    PART_WAY, Sliding, closing, last_pixel, pixel, sliding, the_closing, the_slide,
};
use crate::popup_slide::LENGTH;
use crate::settings::Value as Setting;
use crate::settings::translate::CAMINITO;

const RED: u32 = 0xff0000;

/// Where the menus are: a 150 × 200 window, taller than eight lines of the font.
const MENU: (i32, i32, i32, i32) = (300, 200, 150, 200);

fn menu_place() -> Rectangle<i32, Logical> {
    let (x, y, w, h) = MENU;
    Rectangle::new((x, y).into(), (w, h).into())
}

/// The menu's top right pixel, which the pointer's cursor at its corner doesn't cover.
fn top_right() -> Point<i32, Logical> {
    let place = menu_place();
    place.loc + Point::from((place.size.w - 1, 0))
}

/// One output with `popup-slide` as `on` says, and an X client. The clock is stopped, so
/// the slide is read where `advance_clock` has it.
fn desk(on: bool) -> (Fixture, ClientId) {
    let key = (CAMINITO, "popup-slide".into(), Setting::Bool(on));
    let mut f = Fixture::with_settings(vec![key]);
    f.add_output((1280, 800), 1.0);
    let x = f.connect(Kind::X11);
    f.stop_clock();
    (f, x)
}

/// A red override-redirect window of `types` at `MENU`, mapped with the pointer at
/// `pointer`.
fn menu(f: &mut Fixture, x: ClientId, pointer: (f64, f64), types: Vec<&'static str>) -> u32 {
    f.pointer_move(pointer.0, pointer.1);
    let (px, py, w, h) = MENU;
    let spec = XWindowSpec {
        size: (w as u16, h as u16),
        position: Some((px as i16, py as i16)),
        background: Some(RED),
        types,
        ..XWindowSpec::default()
    };
    f.map_override_redirect_as(x, spec)
}

/// How X closes an override-redirect window.
#[derive(Debug, Clone, Copy)]
enum Close {
    Unmap,
    Destroy,
}

/// Has X close override-redirect window `window` as `how` says, and waits until Caminito
/// no longer shows it.
fn close(f: &mut Fixture, x: ClientId, window: u32, how: Close) {
    f.x11_call(x, move |x| match how {
        Close::Unmap => x.unmap(window),
        Close::Destroy => x.destroy(window),
    })
    .unwrap();
    f.wait_until_override_redirect_gone(window);
}

/// Whether `slide` is on its way, with `from` the offset it started at.
fn on_its_way(slide: &Sliding, from: Point<i32, Logical>) -> bool {
    let now = slide.offset;
    now.x.signum() == from.x.signum()
        && now.y.signum() == from.y.signum()
        && now.x.abs() < from.x.abs().max(1)
        && now.y.abs() < from.y.abs().max(1)
}

#[test]
fn a_menu_mapped_under_the_pointer_slides_down_and_one_above_it_slides_up() {
    // Under the pointer, at the menu's corner, as a menubar's or a context menu opens; and
    // above it, as a bottom panel's opens.
    for (pointer, edge, down) in [
        ((300.0, 200.0), "top", true),
        ((310.0, 790.0), "bottom", false),
    ] {
        let (mut f, x) = desk(true);
        menu(&mut f, x, pointer, Vec::new());

        let start = the_slide(&f);
        assert_eq!(start.rect, menu_place(), "{edge}");
        assert_eq!(start.edge, edge);
        assert_eq!(start.offset.x, 0, "{edge}");
        // Eight lines of the font, less than its height, so it fades as well.
        assert!(start.offset.y.abs() < 200, "{start:?}");
        assert_eq!(start.offset.y < 0, down, "{start:?}");
        assert_eq!(start.alpha, 0.0, "{edge}");

        f.advance_clock(PART_WAY);
        let part_way = the_slide(&f);
        assert!(on_its_way(&part_way, start.offset), "{part_way:?}");
        assert!(0.0 < part_way.alpha && part_way.alpha < 1.0, "{part_way:?}");
        assert_eq!(part_way.rect, menu_place());
        // The rows it has yet to slide into are empty, and nothing of it is drawn outside
        // its place.
        let (far, outside) = if down {
            let far = last_pixel(menu_place());
            (far, top_right() - Point::from((0, 1)))
        } else {
            (top_right(), last_pixel(menu_place()) + Point::from((0, 1)))
        };
        assert_ne!(pixel(&mut f, far), RED, "{edge}");
        assert_ne!(pixel(&mut f, outside), RED, "{edge}");

        f.advance_clock(LENGTH - PART_WAY);
        assert!(sliding(&f).is_empty(), "over at 200 ms: {edge}");
        assert_eq!(pixel(&mut f, top_right()), RED, "{edge}");
        assert_eq!(pixel(&mut f, last_pixel(menu_place())), RED, "{edge}");
    }
}

#[test]
fn a_menu_beside_the_pointer_slides_away_from_it_sideways() {
    for (pointer, edge) in [((100.0, 300.0), "left"), ((600.0, 300.0), "right")] {
        let (mut f, x) = desk(true);
        menu(&mut f, x, pointer, Vec::new());

        assert_eq!(the_slide(&f).edge, edge);
    }
}

#[test]
fn a_menu_unmapped_or_destroyed_slides_back_from_its_picture() {
    for how in [Close::Unmap, Close::Destroy] {
        let (mut f, x) = desk(true);
        let window = menu(&mut f, x, (300.0, 200.0), Vec::new());
        f.advance_clock(LENGTH);

        close(&mut f, x, window, how);
        let start = the_closing(&f);
        assert_eq!(start.rect, menu_place(), "{how:?}");
        assert_eq!(start.edge, "top");
        assert_eq!((start.offset, start.alpha), ((0, 0).into(), 1.0));
        assert_eq!(pixel(&mut f, last_pixel(menu_place())), RED, "its picture");

        f.advance_clock(PART_WAY);
        let part_way = the_closing(&f);
        assert!(
            part_way.offset.y < 0 && part_way.offset.x == 0,
            "{part_way:?}"
        );
        assert!(part_way.alpha < 1.0, "{part_way:?}");
        assert_ne!(pixel(&mut f, last_pixel(menu_place())), RED, "gone back up");

        f.advance_clock(LENGTH - PART_WAY);
        assert!(closing(&f).is_empty(), "gone at 200 ms: {:?}", closing(&f));
        assert_ne!(pixel(&mut f, top_right()), RED);
    }
}

#[test]
fn a_menu_unmapped_part_way_open_turns_round_and_mapped_again_turns_round_again() {
    let (mut f, x) = desk(true);
    let window = menu(&mut f, x, (300.0, 200.0), Vec::new());
    f.advance_clock(PART_WAY);
    let out = the_slide(&f);

    close(&mut f, x, window, Close::Unmap);
    let back = the_closing(&f);
    assert!(sliding(&f).is_empty());
    assert_eq!((back.offset, back.rect), (out.offset, out.rect), "no jump");

    f.advance_clock(PART_WAY / 2);
    let further = the_closing(&f);
    assert!(further.offset.y < out.offset.y, "back up: {further:?}");

    // X maps it again where it was while its picture slides back.
    f.x11_call(x, move |x| x.map(window)).unwrap();
    f.wait_for_override_redirect(window);
    assert!(closing(&f).is_empty(), "{:?}", closing(&f));
    let again = the_slide(&f);
    assert_eq!(again.offset, further.offset, "no jump");
    assert_eq!(again.edge, "top");

    // At the same speed: it has as far to go as it had been when it was unmapped, less the
    // way it went back.
    let left = LENGTH - PART_WAY / 2;
    f.advance_clock(left - Duration::from_millis(1));
    assert_eq!(sliding(&f).len(), 1);
    f.advance_clock(Duration::from_millis(1));
    assert!(sliding(&f).is_empty(), "{:?}", sliding(&f));
}

#[test]
fn a_utility_or_a_dnd_override_redirect_window_doesnt_slide() {
    for kind in ["_NET_WM_WINDOW_TYPE_UTILITY", "_NET_WM_WINDOW_TYPE_DND"] {
        let (mut f, x) = desk(true);
        let window = menu(&mut f, x, (300.0, 200.0), vec![kind]);

        assert!(sliding(&f).is_empty(), "{kind}: {:?}", sliding(&f));
        assert_eq!(pixel(&mut f, last_pixel(menu_place())), RED, "{kind}");

        close(&mut f, x, window, Close::Unmap);
        assert!(closing(&f).is_empty(), "{kind}: {:?}", closing(&f));
    }
}

#[test]
fn part_way_the_keyboard_grab_and_a_click_reach_a_menu_at_its_real_place() {
    let (mut f, x) = desk(true);
    let window = menu(&mut f, x, (300.0, 200.0), Vec::new());
    f.advance_clock(PART_WAY);
    assert_eq!(sliding(&f).len(), 1);

    let status = f.x11_call(x, move |x| x.grab_keyboard(window)).unwrap();
    assert_eq!(status, GrabStatus::SUCCESS);
    f.dispatch_until(|f| f.override_redirect_with_keyboard() == Some(window));
    f.key(KEY_A, true);
    f.key(KEY_A, false);
    let events = f.x11_call(x, |x| x.events()).unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::KeyPress(key) if key.event == window)),
        "{events:?}"
    );

    // On its bottom row, which it's drawn above part-way.
    let row = last_pixel(menu_place()).to_f64();
    assert_ne!(pixel(&mut f, last_pixel(menu_place())), RED);
    f.click(row.x, row.y);
    assert_eq!(f.override_redirect_under_pointer(), Some(window));
}

#[test]
fn with_the_slide_off_a_menu_is_there_and_gone_in_one_frame() {
    let (mut f, x) = desk(false);
    let window = menu(&mut f, x, (300.0, 200.0), Vec::new());

    assert!(sliding(&f).is_empty(), "{:?}", sliding(&f));
    assert_eq!(pixel(&mut f, top_right()), RED);
    assert_eq!(pixel(&mut f, last_pixel(menu_place())), RED);

    close(&mut f, x, window, Close::Unmap);
    assert!(closing(&f).is_empty(), "{:?}", closing(&f));
    assert_ne!(pixel(&mut f, top_right()), RED);
    assert_ne!(pixel(&mut f, last_pixel(menu_place())), RED);
}
