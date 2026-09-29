//! Converting between logical space and X client space at the X11 scale.

use smithay::utils::{FrameExtents, Rectangle};

use super::X11Scale;
use crate::window::struts;

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, smithay::utils::Logical> {
    Rectangle::new((x, y).into(), (w, h).into())
}

#[test]
fn logical_values_are_multiplied_into_x_client_space() {
    assert_eq!(
        X11Scale(2).to_x(rect(10, 20, 300, 200)),
        rect(20, 40, 600, 400)
    );
    assert_eq!(
        X11Scale(2).extents_to_x(FrameExtents::new(1, 2, 24, 4)),
        FrameExtents::new(2, 4, 48, 8)
    );
    assert_eq!(
        X11Scale(1).to_x(rect(10, 20, 300, 200)),
        rect(10, 20, 300, 200)
    );
}

#[test]
fn a_rectangle_from_x_becomes_the_logical_one_covering_it() {
    assert_eq!(
        X11Scale(2).rect_to_logical(rect(20, 40, 600, 400)),
        rect(10, 20, 300, 200)
    );
    // 11..=24 covers logical pixels 5 to 12.
    assert_eq!(
        X11Scale(2).rect_to_logical(rect(11, 0, 14, 3)),
        rect(5, 0, 8, 2)
    );
    assert_eq!(
        X11Scale(2).rect_to_logical(rect(-3, -3, 2, 2)),
        rect(-2, -2, 2, 2)
    );
}

#[test]
fn a_strut_from_x_reserves_at_least_what_it_asked_over_the_pixels_it_names() {
    // `_NET_WM_STRUT_PARTIAL`: 49 pixels along the top, from 0 to 3839.
    let partial = [0, 0, 49, 0, 0, 0, 0, 0, 0, 3839, 0, 0];
    let [strut] = struts(Some(&partial), None)[..] else {
        unreachable!("one strut");
    };
    let logical = X11Scale(2).strut_to_logical(strut);
    assert_eq!((logical.width, logical.span), (25, Some((0, 1919))));
    assert_eq!(X11Scale(1).strut_to_logical(strut), strut);
}

#[test]
fn a_place_from_x_rounds_as_smithay_does() {
    assert_eq!(X11Scale(2).to_logical(100), 50);
    assert_eq!(X11Scale(2).to_logical(101), 51);
    assert_eq!(X11Scale(2).to_logical(-101), -51);
}
