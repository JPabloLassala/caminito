//! xdg popups: placed by their positioner and kept on their parent's output (22).

use camino_test_client::{
    Anchor, ConstraintAdjustment, Gravity, Positioner, format_popup_configures,
};
use insta::assert_snapshot;
use smithay::utils::{Logical, Point};

use super::fixture::{ClientId, Fixture};

const POPUP_COLOR: u32 = 0xffff0000;

/// A 1900 × 1000 window, near enough the output's edges to push popups off it, and where
/// policy placed it.
fn big_window(
    f: &mut Fixture,
    id: ClientId,
) -> (camino_test_client::WindowId, Point<i32, Logical>) {
    let window = f.map_window(id, (1900, 1000));
    let policy_id = f.policy_id(id, window).unwrap();
    let geometry = f.state.camino.policy.geometry(policy_id).unwrap();
    (window, geometry.loc)
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
    let (mut f, id) = Fixture::connected();
    let window = f.map_window(id, (400, 300));

    let popup = f.client(id).create_popup(
        window,
        bottom_right((10, 20, 30, 40), ConstraintAdjustment::None),
    );
    f.roundtrip(id);

    assert_snapshot!(format_popup_configures(&f.client(id).popup(popup).configures), @"at 40, 60, size: 300 × 200");
}

#[test]
fn a_popup_flips_to_stay_on_its_output() {
    let (mut f, id) = Fixture::connected();
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
    let (mut f, id) = Fixture::connected();
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
    let (mut f, id) = Fixture::connected();
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
    let (mut f, id) = Fixture::connected();
    let output = f.state.camino.outputs[0].clone();
    let window = f.map_window(id, (400, 300));
    let popup = f.client(id).create_popup(
        window,
        bottom_right((10, 20, 30, 40), ConstraintAdjustment::None),
    );
    f.roundtrip(id);

    f.client(id).map_popup(popup, POPUP_COLOR);
    f.roundtrip(id);
    f.dispatch();

    let policy_id = f.policy_id(id, window).unwrap();
    let parent = f.state.camino.policy.geometry(policy_id).unwrap();
    let pixels = f.state.backend.headless_mut().pixels(&output).unwrap();
    let (x, y) = (parent.loc.x + 40, parent.loc.y + 60);
    assert_eq!(pixels[(y * 1920 + x) as usize], POPUP_COLOR);
    assert_eq!(pixels[((y + 199) * 1920 + x + 299) as usize], POPUP_COLOR);
}
