// SPDX-FileCopyrightText: 2001 Havoc Pennington
// SPDX-FileCopyrightText: 2002-2003 Red Hat, Inc.
// SPDX-FileCopyrightText: 2003 Rob Adams
// SPDX-FileCopyrightText: 2005 Elijah Newren
// SPDX-FileCopyrightText: 2025-2026 Juan Pablo Lassala
// SPDX-License-Identifier: GPL-3.0-or-later AND GPL-2.0-or-later

//! Marco's placement (`place.c`), on frame rectangles: the automatic, pointer and manual
//! modes, and `center-new-windows` (154).

use smithay::utils::{Logical, Point, Rectangle, Size};

/// Marco's `placement-mode`: where a new window goes when nothing else places it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlacementMode {
    /// The first fit, or a cascade.
    #[default]
    Automatic,
    /// Centred on the pointer.
    Pointer,
    /// Centred on the pointer, then moved with it until it's put down.
    Manual,
}

type Rect = Rectangle<i32, Logical>;

/// How close a window's corner must be to a cascade point to be in its way.
const CASCADE_FUZZ: i32 = 15;
/// How far each new cascade starts right of the last one.
const CASCADE_INTERVAL: i32 = 50;

/// A window already on screen, as placement sees it.
pub(crate) struct Obstacle {
    pub frame: Rect,
    /// The window geometry's origin, where Marco cascades the next frame to.
    pub client: Point<i32, Logical>,
    /// Whether placing over it counts as overlap: normal, utility, toolbar and menu windows.
    pub blocks: bool,
}

/// Marco's `center_tile_rect_in_area`: the spare space is split so that a screen full of
/// windows tiled this way would be centred as a group.
pub(crate) fn center_tile(size: Size<i32, Logical>, area: Rect) -> Point<i32, Logical> {
    Point::new(
        area.loc.x + (area.size.w % (size.w + 1)) / 2,
        area.loc.y + (area.size.h % (size.h + 1)) / 3,
    )
}

/// Marco's `center_rect_in_area`, for `center-new-windows`: the middle of `area`, if the
/// frame fits there. Windows already there don't matter.
pub(crate) fn center(size: Size<i32, Logical>, area: Rect) -> Option<Point<i32, Logical>> {
    let loc = Point::new(
        area.loc.x + (area.size.w - size.w) / 2,
        area.loc.y + (area.size.h - size.h) / 2,
    );
    area.contains_rect(Rectangle::new(loc, size)).then_some(loc)
}

/// Marco's `place_by_pointer`: the frame centred on the pointer.
pub(crate) fn at_pointer(
    size: Size<i32, Logical>,
    pointer: Point<i32, Logical>,
) -> Point<i32, Logical> {
    Point::new(pointer.x - size.w / 2, pointer.y - size.h / 2)
}

/// Marco's `find_first_fit`: the centre tile, then below each window, then right of each, for
/// the first frame position inside `area` that overlaps no blocking window.
pub(crate) fn first_fit(
    size: Size<i32, Logical>,
    area: Rect,
    others: &[Obstacle],
) -> Option<Point<i32, Logical>> {
    let fits = |loc: Point<i32, Logical>| {
        let rect = Rectangle::new(loc, size);
        area.contains_rect(rect) && !others.iter().any(|o| o.blocks && o.frame.overlaps(rect))
    };

    let center = center_tile(size, area);
    if fits(center) {
        return Some(center);
    }

    let mut by_top: Vec<&Rect> = others.iter().map(|o| &o.frame).collect();
    by_top.sort_by_key(|r| (r.loc.y, r.loc.x));
    let below = by_top
        .iter()
        .map(|r| Point::new(r.loc.x, r.loc.y + r.size.h))
        .find(|loc| fits(*loc));
    if below.is_some() {
        return below;
    }

    let mut by_left = by_top;
    by_left.sort_by_key(|r| (r.loc.x, r.loc.y));
    by_left
        .iter()
        .map(|r| Point::new(r.loc.x + r.size.w, r.loc.y))
        .find(|loc| fits(*loc))
}

/// Marco's `find_next_cascade`: a fuzzy diagonal cascade from the corner of `area`, stepping
/// past each window whose corner is near the cascade point.
///
/// Marco steps to the client origin of the window in the way. A frameless window has its
/// client origin at its corner, so there the step is the fuzz instead.
pub(crate) fn cascade(
    size: Size<i32, Logical>,
    extents_left_top: (i32, i32),
    area: Rect,
    others: &[Obstacle],
) -> Point<i32, Logical> {
    let x_threshold = extents_left_top.0.max(CASCADE_FUZZ);
    let y_threshold = extents_left_top.1.max(CASCADE_FUZZ);

    let mut sorted: Vec<&Obstacle> = others.iter().collect();
    sorted.sort_by_key(|o| {
        let (x, y) = (i64::from(o.frame.loc.x), i64::from(o.frame.loc.y));
        x * x + y * y
    });

    let right = area.loc.x + area.size.w;
    let bottom = area.loc.y + area.size.h;
    let (mut x, mut y) = (area.loc.x, area.loc.y);
    let mut stage = 0;
    let mut i = 0;
    while i < sorted.len() {
        let o = sorted[i];
        let (wx, wy) = (o.frame.loc.x, o.frame.loc.y);
        if (wx - x).abs() < x_threshold && (wy - y).abs() < y_threshold {
            if o.client == o.frame.loc {
                (x, y) = (wx + CASCADE_FUZZ, wy + CASCADE_FUZZ);
            } else {
                (x, y) = (o.client.x, o.client.y);
            }
            if x + size.w > right || y + size.h > bottom {
                // Off the area: start a new cascade further right, if there's room.
                stage += 1;
                (x, y) = (area.loc.x + CASCADE_INTERVAL * stage, area.loc.y);
                if x + size.w < right {
                    i = 0;
                    continue;
                }
                x = area.loc.x;
                break;
            }
        }
        i += 1;
    }
    Point::new(x, y)
}

/// Moves a frame inside `area`, or to its top-left corner along an axis it doesn't fit.
pub(crate) fn constrain(frame: Rect, area: Rect) -> Point<i32, Logical> {
    let axis =
        |loc: i32, len: i32, start: i32, area_len: i32| loc.min(start + area_len - len).max(start);
    Point::new(
        axis(frame.loc.x, frame.size.w, area.loc.x, area.size.w),
        axis(frame.loc.y, frame.size.h, area.loc.y, area.size.h),
    )
}
