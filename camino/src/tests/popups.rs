//! xdg popups: placed by their positioner and kept on their parent's output (22).

use camino_test_client::{
    Anchor, ConstraintAdjustment, Gravity, Positioner, format_popup_configures,
};
use insta::assert_snapshot;
use smithay::utils::{Logical, Point};

use super::fixture::{ClientId, Fixture, Kind, WindowSpec};

const POPUP_COLOR: u32 = 0xffff0000;

/// A 1900 × 1000 window, near enough the output's edges to push popups off it, and where
/// policy placed it.
fn big_window(
    f: &mut Fixture,
    id: ClientId,
) -> (camino_test_client::WindowId, Point<i32, Logical>) {
    let window = f.map(id, WindowSpec::plain((1900, 1000)));
    (window.xdg(), f.geometry(window).loc)
}

/// A 300 × 200 popup hanging off the bottom-right corner of `anchor_rect`.
fn bottom_right(anchor_rect: (i32, i32, i32, i32), adjustment: ConstraintAdjustment) -> Positioner {
    Positioner {
        size: (300, 200),
        anchor_rect,
        anchor: Anchor::BottomRight,
        gravity: Gravity::BottomRight,
        constraint_adjustment: adjustment,
    }
}

#[test]
fn a_popup_that_fits_goes_where_its_positioner_says() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let window = f.map(id, WindowSpec::plain((400, 300)));

    let popup = f.client(id).create_popup(
        window.xdg(),
        bottom_right((10, 20, 30, 40), ConstraintAdjustment::None),
    );
    f.roundtrip(id);

    assert_snapshot!(format_popup_configures(&f.client(id).popup(popup).configures), @"at 40, 60, size: 300 × 200");
}

#[test]
fn a_popup_flips_to_stay_on_its_output() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let (window, _) = big_window(&mut f, id);

    let flip = ConstraintAdjustment::FlipX | ConstraintAdjustment::FlipY;
    let popup = f
        .client(id)
        .create_popup(window, bottom_right((1850, 950, 50, 50), flip));
    f.roundtrip(id);

    assert_snapshot!(format_popup_configures(&f.client(id).popup(popup).configures), @"at 1550, 750, size: 300 × 200");
}

#[test]
fn a_popup_slides_to_stay_on_its_output() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let (window, loc) = big_window(&mut f, id);

    let slide = ConstraintAdjustment::SlideX | ConstraintAdjustment::SlideY;
    let popup = f
        .client(id)
        .create_popup(window, bottom_right((1850, 950, 50, 50), slide));
    f.roundtrip(id);

    // Slid back until its bottom-right corner meets the output's.
    let configure = f.client(id).popup(popup).configures[0].clone();
    assert_eq!(configure.size, (300, 200));
    assert_eq!(
        (loc.x + configure.x, loc.y + configure.y),
        (1920 - 300, 1080 - 200)
    );
}

#[test]
fn a_repositioned_popup_stays_on_its_output() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let (window, _) = big_window(&mut f, id);
    let popup = f.client(id).create_popup(
        window,
        bottom_right((10, 20, 30, 40), ConstraintAdjustment::None),
    );
    f.roundtrip(id);
    f.client(id).map_popup(popup, POPUP_COLOR);

    let flip = ConstraintAdjustment::FlipX | ConstraintAdjustment::FlipY;
    f.client(id)
        .reposition_popup(popup, bottom_right((1850, 950, 50, 50), flip), 7);
    f.roundtrip(id);

    assert_snapshot!(format_popup_configures(&f.client(id).popup(popup).configures), @r"
    at 40, 60, size: 300 × 200
    at 1550, 750, size: 300 × 200, repositioned: 7
    ");
}

#[test]
fn a_mapped_popup_is_drawn_over_its_parent() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let output = f.state.camino.outputs[0].clone();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let popup = f.client(id).create_popup(
        window.xdg(),
        bottom_right((10, 20, 30, 40), ConstraintAdjustment::None),
    );
    f.roundtrip(id);

    f.client(id).map_popup(popup, POPUP_COLOR);
    f.roundtrip(id);

    let parent = f.geometry(window);
    let pixels = f.state.backend.headless_mut().pixels(&output).unwrap();
    let (x, y) = (parent.loc.x + 40, parent.loc.y + 60);
    assert_eq!(pixels[(y * 1920 + x) as usize], POPUP_COLOR);
    assert_eq!(pixels[((y + 199) * 1920 + x + 299) as usize], POPUP_COLOR);
}

#[test]
fn a_grab_without_the_serial_of_an_input_event_is_refused() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let geometry = f.geometry(window);
    f.click(geometry.loc.x as f64 + 10., geometry.loc.y as f64 + 10.);
    f.roundtrip(id);
    let press = f.client(id).state.last_press_serial.unwrap();
    let menu = || bottom_right((10, 20, 30, 40), ConstraintAdjustment::None);

    // A serial Camino never sent the client.
    let refused = f.client(id).create_popup(window.xdg(), menu());
    f.client(id).grab_popup(refused, press.wrapping_add(1000));
    let granted = f.client(id).create_popup(window.xdg(), menu());
    f.client(id).grab_popup(granted, press);
    f.roundtrip(id);

    assert!(f.client(id).popup(refused).done);
    assert!(!f.client(id).popup(granted).done);
}

#[test]
fn a_submenus_grab_needs_no_serial_of_its_own() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let geometry = f.geometry(window);
    f.click(geometry.loc.x as f64 + 10., geometry.loc.y as f64 + 10.);
    f.roundtrip(id);
    let press = f.client(id).state.last_press_serial.unwrap();
    let place = || bottom_right((10, 20, 30, 40), ConstraintAdjustment::None);
    let menu = f.client(id).create_popup(window.xdg(), place());
    f.client(id).grab_popup(menu, press);
    f.roundtrip(id);
    f.client(id).map_popup(menu, POPUP_COLOR);
    f.roundtrip(id);

    // A serial Camino never sent the client, as one long past is.
    let stale = press.wrapping_add(1000);
    let submenu = f.client(id).create_nested_popup(menu, place(), stale);
    f.roundtrip(id);

    assert!(!f.client(id).popup(menu).done);
    assert!(!f.client(id).popup(submenu).done);
}
