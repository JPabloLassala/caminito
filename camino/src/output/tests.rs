//! The X11 scale (ADR 0008): the largest of the outputs' rounded-up scales.

use super::x11_scale;

#[test]
fn a_single_output_gives_its_scale_rounded_up() {
    assert_eq!(x11_scale([1.0]), 1);
    assert_eq!(x11_scale([1.25]), 2);
    assert_eq!(x11_scale([1.5]), 2);
    assert_eq!(x11_scale([2.0]), 2);
}

#[test]
fn mixed_outputs_give_the_largest_rounded_up_scale() {
    assert_eq!(x11_scale([1.0, 2.0]), 2);
    assert_eq!(x11_scale([2.0, 1.0]), 2);
    assert_eq!(x11_scale([1.0, 1.25]), 2);
    assert_eq!(x11_scale([1.5, 2.25, 1.0]), 3);
}

#[test]
fn no_outputs_give_1() {
    assert_eq!(x11_scale([]), 1);
}
