//! Docking, snapping, healing and resizing, by example and by property. The checks of the
//! properties are written apart from `is_contiguous`, pair by pair, so they don't share
//! its mistakes.

use proptest::prelude::*;

use crate::{MIN_SHARED_EDGE, Rect, at_origin, at_the_right, dock, heal, is_contiguous, resized};

const fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
    Rect { x, y, w, h }
}

/// A 1920×1080 output at the origin.
const A: Rect = rect(0, 0, 1920, 1080);

/// The pairs of outputs that are on top of each other.
fn overlapping(outputs: &[Rect]) -> Vec<(Rect, Rect)> {
    let mut pairs = Vec::new();
    for (i, a) in outputs.iter().enumerate() {
        for b in &outputs[i + 1..] {
            let across = a.x.max(b.x) < (a.x + a.w).min(b.x + b.w);
            let down = a.y.max(b.y) < (a.y + a.h).min(b.y + b.h);
            if across && down {
                pairs.push((*a, *b));
            }
        }
    }
    pairs
}

/// Whether two outputs lie side by side, or one above the other, along enough edge.
fn share_enough_edge(a: Rect, b: Rect) -> bool {
    let shared_down = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
    let shared_across = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
    let side_by_side = a.x + a.w == b.x || b.x + b.w == a.x;
    let stacked = a.y + a.h == b.y || b.y + b.h == a.y;
    (side_by_side && shared_down >= MIN_SHARED_EDGE.min(a.h).min(b.h))
        || (stacked && shared_across >= MIN_SHARED_EDGE.min(a.w).min(b.w))
}

/// The outputs that can't be reached from the first through shared edges.
fn cut_off(outputs: &[Rect]) -> Vec<Rect> {
    let mut reached = vec![false; outputs.len()];
    let mut queue = Vec::from_iter(outputs.first().map(|_| 0));
    while let Some(i) = queue.pop() {
        if std::mem::replace(&mut reached[i], true) {
            continue;
        }
        queue.extend((0..outputs.len()).filter(|&j| share_enough_edge(outputs[i], outputs[j])));
    }
    let unreached = outputs.iter().zip(reached).filter(|(_, reached)| !reached);
    unreached.map(|(output, _)| *output).collect()
}

/// Outputs anywhere, of any size, some shorter than the edge they must share.
fn any_rect() -> impl Strategy<Value = Rect> {
    let side = prop_oneof![1 => 1..MIN_SHARED_EDGE, 4 => MIN_SHARED_EDGE..4000];
    (-6000..6000, -4000..4000, side.clone(), side).prop_map(|(x, y, w, h)| rect(x, y, w, h))
}

/// A layout that may have gaps and overlaps, and an output of it to hold still.
fn any_layout() -> impl Strategy<Value = (Vec<Rect>, usize)> {
    proptest::collection::vec(any_rect(), 1..7)
        .prop_flat_map(|outputs| (0..outputs.len(), Just(outputs)))
        .prop_map(|(anchor, outputs)| (outputs, anchor))
}

proptest! {
    #[test]
    fn a_healed_layout_has_no_gap_no_overlap_and_no_output_cut_off((outputs, anchor) in any_layout()) {
        let healed = heal(&outputs, anchor);

        prop_assert_eq!(overlapping(&healed), []);
        prop_assert_eq!(cut_off(&healed), []);
        prop_assert!(is_contiguous(&healed));
    }

    #[test]
    fn healing_moves_outputs_without_resizing_them_and_holds_the_anchor_still(
        (outputs, anchor) in any_layout()
    ) {
        let healed = heal(&outputs, anchor);

        prop_assert_eq!(healed[anchor], outputs[anchor]);
        let sizes = |layout: &[Rect]| layout.iter().map(|o| (o.w, o.h)).collect::<Vec<_>>();
        prop_assert_eq!(sizes(&healed), sizes(&outputs));
    }

    #[test]
    fn healing_a_healed_layout_changes_nothing(
        (outputs, anchor) in any_layout(),
        second_anchor in any::<prop::sample::Index>(),
    ) {
        let healed = heal(&outputs, anchor);

        // Whichever output is held still the second time.
        prop_assert_eq!(&heal(&healed, second_anchor.index(healed.len())), &healed);
    }

    #[test]
    fn a_healed_layout_at_the_origin_is_still_healed_with_its_corner_at_0_0(
        (outputs, anchor) in any_layout()
    ) {
        let moved = at_origin(&heal(&outputs, anchor));

        prop_assert_eq!(moved.iter().map(|o| o.x).min(), Some(0));
        prop_assert_eq!(moved.iter().map(|o| o.y).min(), Some(0));
        prop_assert_eq!(&heal(&moved, anchor), &moved);
    }

    #[test]
    fn a_docked_output_leaves_the_layout_contiguous(
        (others, anchor) in any_layout(),
        output in any_rect(),
        snap in prop_oneof![Just(0), 1..300],
    ) {
        let mut layout = heal(&others, anchor);

        let docked = dock(output, &layout, snap);

        prop_assert_eq!((docked.w, docked.h), (output.w, output.h));
        layout.push(docked);
        prop_assert_eq!(overlapping(&layout), []);
        prop_assert_eq!(cut_off(&layout), []);
        prop_assert!(is_contiguous(&layout));
    }

    #[test]
    fn an_output_already_against_the_outline_stays_where_it_is(
        (others, anchor) in any_layout(),
        output in any_rect(),
    ) {
        let layout = heal(&others, anchor);
        let docked = dock(output, &layout, 0);

        prop_assert_eq!(dock(docked, &layout, 0), docked);
    }
}

#[test]
fn an_output_docks_to_the_nearest_place_on_the_outline() {
    // Wanted with a gap to A's right, and then partly on top of it.
    assert_eq!(
        dock(rect(2500, 200, 1280, 720), &[A], 0),
        rect(1920, 200, 1280, 720)
    );
    assert_eq!(
        dock(rect(1800, 200, 1280, 720), &[A], 0),
        rect(1920, 200, 1280, 720)
    );
    assert_eq!(
        dock(rect(300, -900, 1280, 720), &[A], 0),
        rect(300, -720, 1280, 720)
    );
    assert_eq!(
        dock(rect(-1500, 200, 1280, 720), &[A], 0),
        rect(-1280, 200, 1280, 720)
    );
    assert_eq!(
        dock(rect(300, 1300, 1280, 720), &[A], 0),
        rect(300, 1080, 1280, 720)
    );
}

#[test]
fn a_docked_output_shares_at_least_100_pixels_of_edge() {
    // Wanted well right of A, below its corner and above it.
    assert_eq!(
        dock(rect(3000, 1500, 1280, 720), &[A], 0),
        rect(1920, 980, 1280, 720)
    );
    assert_eq!(
        dock(rect(3000, -1500, 1280, 720), &[A], 0),
        rect(1920, -620, 1280, 720)
    );
}

#[test]
fn an_output_shorter_than_100_pixels_shares_its_whole_edge() {
    assert_eq!(
        dock(rect(3000, 1500, 800, 60), &[A], 0),
        rect(1920, 1020, 800, 60)
    );
    assert_eq!(
        dock(rect(3000, -1500, 800, 60), &[A], 0),
        rect(1920, 0, 800, 60)
    );
}

#[test]
fn an_output_doesnt_dock_on_top_of_a_third() {
    let right = rect(1920, 0, 1920, 1080);

    // Below A it would reach under `right` too, which is fine. Beside A it would cover it.
    let docked = dock(rect(1900, 300, 1280, 720), &[A, right], 0);

    assert_eq!(docked, rect(1900, 1080, 1280, 720));
}

#[test]
fn the_first_output_stays_where_it_is_wanted() {
    assert_eq!(
        dock(rect(40, -7, 1280, 720), &[], 12),
        rect(40, -7, 1280, 720)
    );
}

#[test]
fn a_docked_output_snaps_to_the_matching_edge_of_the_output_it_lies_against() {
    // Top with top, and bottom with bottom: 1080 - 720 = 360.
    assert_eq!(
        dock(rect(1925, 9, 1280, 720), &[A], 12),
        rect(1920, 0, 1280, 720)
    );
    assert_eq!(
        dock(rect(1925, 351, 1280, 720), &[A], 12),
        rect(1920, 360, 1280, 720)
    );
    // Left with left, and right with right: 1920 - 1280 = 640.
    assert_eq!(
        dock(rect(-11, 1090, 1280, 720), &[A], 12),
        rect(0, 1080, 1280, 720)
    );
    assert_eq!(
        dock(rect(652, 1090, 1280, 720), &[A], 12),
        rect(640, 1080, 1280, 720)
    );
}

#[test]
fn an_edge_further_than_the_snap_distance_doesnt_pull() {
    assert_eq!(
        dock(rect(1925, 13, 1280, 720), &[A], 12),
        rect(1920, 13, 1280, 720)
    );
    assert_eq!(
        dock(rect(1925, 9, 1280, 720), &[A], 0),
        rect(1920, 9, 1280, 720)
    );
}

#[test]
fn the_nearer_of_two_edges_within_the_snap_distance_wins() {
    // 20 pixels shorter than A, so both its top and its bottom can line up near y = 10.
    assert_eq!(
        dock(rect(1925, 4, 1280, 1060), &[A], 30),
        rect(1920, 0, 1280, 1060)
    );
    assert_eq!(
        dock(rect(1925, 16, 1280, 1060), &[A], 30),
        rect(1920, 20, 1280, 1060)
    );
}

#[test]
fn an_output_snaps_to_one_it_lies_against_on_its_far_side() {
    // The docked output fills the 1280 pixels between A and `far`.
    let far = rect(3200, 300, 1920, 1080);

    let docked = dock(rect(1925, 308, 1280, 720), &[A, far], 12);

    assert_eq!(docked, rect(1920, 300, 1280, 720));
}

#[test]
fn an_output_it_doesnt_touch_doesnt_pull() {
    // `left` has its top at 300 too, but A is between it and the docked output.
    let left = rect(-1920, 300, 1920, 1080);

    let docked = dock(rect(1925, 308, 1280, 720), &[A, left], 12);

    assert_eq!(docked, rect(1920, 308, 1280, 720));
}

#[test]
fn centres_dont_snap() {
    // Centred against A it would be at y = 180.
    assert_eq!(
        dock(rect(1925, 183, 1280, 720), &[A], 12),
        rect(1920, 183, 1280, 720)
    );
}

#[test]
fn a_snap_never_puts_an_output_on_top_of_another() {
    // `low`'s top would pull the docked output's top to 1000, where it would cover `low`.
    let low = rect(1920, 1000, 1920, 1080);

    let docked = dock(rect(1925, 270, 1280, 720), &[A, low], 12);

    assert_eq!(docked, rect(1920, 270, 1280, 720));
}

#[test]
fn healing_closes_the_gap_an_output_left() {
    // Three in a row, without the middle one.
    let healed = heal(&[A, rect(3840, 0, 1920, 1080)], 0);

    assert_eq!(healed, [A, rect(1920, 0, 1920, 1080)]);
}

#[test]
fn healing_moves_the_others_to_the_anchor() {
    let healed = heal(&[A, rect(3840, 0, 1920, 1080)], 1);

    assert_eq!(
        healed,
        [rect(1920, 0, 1920, 1080), rect(3840, 0, 1920, 1080)]
    );
}

#[test]
fn healing_moves_an_output_off_another() {
    let healed = heal(&[A, rect(1800, 100, 1920, 1080)], 0);

    assert_eq!(healed, [A, rect(1920, 100, 1920, 1080)]);
}

#[test]
fn healing_joins_outputs_that_only_touch_at_a_corner_or_along_too_little() {
    assert_eq!(
        heal(&[A, rect(1920, 1080, 1920, 1080)], 0),
        [A, rect(1920, 980, 1920, 1080)]
    );
    assert_eq!(
        heal(&[A, rect(1920, 1030, 1920, 1080)], 0),
        [A, rect(1920, 980, 1920, 1080)]
    );
}

#[test]
fn healing_brings_a_chain_of_outputs_along() {
    // Three in a row after the first of four is gone from x = 0: the nearest is docked
    // first, and the next follows it.
    let row = [
        rect(-1920, 0, 1920, 1080),
        rect(1920, 0, 1920, 1080),
        rect(3840, 0, 1920, 1080),
    ];

    let healed = heal(&row, 0);

    assert_eq!(
        healed,
        [
            rect(-1920, 0, 1920, 1080),
            rect(0, 0, 1920, 1080),
            rect(1920, 0, 1920, 1080),
        ]
    );
}

#[test]
fn a_contiguous_layout_heals_to_itself() {
    let layout = [A, rect(1920, 200, 2560, 1440), rect(500, 1080, 1280, 720)];

    assert_eq!(heal(&layout, 2), layout);
}

#[test]
fn no_outputs_heal_to_none() {
    assert_eq!(heal(&[], 0), []);
}

#[test]
fn outputs_right_of_and_below_a_resized_one_keep_their_side_of_it() {
    let layout = [
        A,
        rect(1920, 0, 1920, 1080),
        rect(0, 1080, 1920, 1080),
        rect(-1280, 0, 1280, 720),
    ];

    // A grows by 640 and 360, as from a scale of 2 to 1.5.
    let grown = resized(&layout, 0, 2560, 1440);

    assert_eq!(
        grown,
        [
            rect(0, 0, 2560, 1440),
            rect(2560, 0, 1920, 1080),
            rect(0, 1440, 1920, 1080),
            rect(-1280, 0, 1280, 720),
        ]
    );
    assert_eq!(resized(&grown, 0, 1920, 1080), layout);
}

#[test]
fn the_origin_is_the_layouts_top_left_corner() {
    let layout = [rect(100, 50, 1920, 1080), rect(-1180, 410, 1280, 720)];

    assert_eq!(
        at_origin(&layout),
        [rect(1280, 0, 1920, 1080), rect(0, 360, 1280, 720)]
    );
    assert_eq!(at_origin(&[]), []);
}

#[test]
fn a_new_output_goes_right_of_the_rightmost_at_the_layouts_top() {
    let layout = [rect(0, 200, 1920, 1080), rect(1920, 0, 2560, 1440)];

    assert_eq!(at_the_right(1280, 720, &layout), rect(4480, 0, 1280, 720));
    assert_eq!(at_the_right(1280, 720, &[]), rect(0, 0, 1280, 720));
}

#[test]
fn a_layout_with_a_gap_an_overlap_or_too_little_shared_edge_isnt_contiguous() {
    let beside = |x, y| [A, rect(x, y, 1920, 1080)];

    assert!(is_contiguous(&beside(1920, 0)));
    assert!(is_contiguous(&beside(1920, 980)));
    assert!(!is_contiguous(&beside(1920, 981)), "99 pixels of edge");
    assert!(!is_contiguous(&beside(1920, 1080)), "a corner");
    assert!(!is_contiguous(&beside(1921, 0)), "a gap");
    assert!(!is_contiguous(&beside(1919, 0)), "an overlap");
    assert!(is_contiguous(&[A]));
    assert!(is_contiguous(&[]));
}

#[test]
fn outputs_shorter_than_100_pixels_are_contiguous_along_the_whole_shorter_edge() {
    assert!(is_contiguous(&[A, rect(1920, 500, 800, 60)]));
    assert!(!is_contiguous(&[A, rect(1920, 1021, 800, 60)]));
}
