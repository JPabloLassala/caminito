// SPDX-FileCopyrightText: 2005-2006 Elijah Newren
// SPDX-FileCopyrightText: 2025-2026 Juan Pablo Lassala
// SPDX-License-Identifier: GPL-3.0-or-later AND GPL-2.0-or-later

//! Edge resistance and snap-move (157): the edges a window being moved or resized stops at,
//! or with Shift snaps to, ported from Marco's `edge-resistance.c`.
//!
//! The edges are the work areas' (Marco's screen and Xinerama edges: the outputs' edges
//! where nothing's reserved, and the reserved strips' inner edges) and the frames of the
//! other windows showing on the workspace, less the parts windows above them cover. Marco's
//! timeouts are all 0 (edge-resistance.c:348-350), so resistance is by distance alone and
//! needs no time.
//!
//! A keyboard move or resize (156) steps by a key at a time, and Marco treats it apart
//! (`keyboard_op`): a step stops at any edge it would cross, and a snap goes on to the next
//! edge its way.

use smithay::utils::{Logical, Point, Rectangle};

/// How far past an edge, in the direction it holds back, the pointer must take a window's
/// side before the side goes on (edge-resistance.c:342-347). Moving away from an edge,
/// the threshold is 0 there, so nothing holds the window.
const WINDOW_THRESHOLD: i32 = 16;
const OUTPUT_THRESHOLD: i32 = 32;

/// How far a snap-move's pointer must go before a side jumps more than this far, so a
/// slight slip of the hand doesn't snap the window away (edge-resistance.c:518-529).
const SNAP_SLIP: i32 = 8;

/// What drives a grab's window, which Marco's edges treat apart (`keyboard_op`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum By {
    Pointer,
    /// The keys of a keyboard move or resize (156).
    Keys,
}

/// The way along its axis an edge holds a window's side back, Marco's `MetaSide`: a `Left`
/// edge stops a side moving left (an output's left edge, a window's right one).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    fn holds_back(self, increasing: bool) -> bool {
        match self {
            Side::Left | Side::Top => !increasing,
            Side::Right | Side::Bottom => increasing,
        }
    }
}

/// One edge: a segment at `position` across its axis, from `start` to `end` along it.
#[derive(Debug, Clone, Copy)]
struct Edge {
    position: i32,
    start: i32,
    end: i32,
    side: Side,
    threshold: i32,
}

impl Edge {
    /// Whether a rectangle's side could meet the edge: their extents along it overlap or
    /// touch, as Marco's `meta_rectangle_edge_aligns` has it (boxes.c).
    fn aligns(&self, rect: Rectangle<i32, Logical>) -> bool {
        let (low, high) = match self.side {
            Side::Left | Side::Right => (rect.loc.y, rect.loc.y + rect.size.h),
            Side::Top | Side::Bottom => (rect.loc.x, rect.loc.x + rect.size.w),
        };
        low <= self.end && self.start <= high
    }
}

/// A rectangle by its four sides' positions.
#[derive(Debug, Clone, Copy)]
struct Sides {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

impl From<Rectangle<i32, Logical>> for Sides {
    fn from(r: Rectangle<i32, Logical>) -> Self {
        Sides {
            left: r.loc.x,
            right: r.loc.x + r.size.w,
            top: r.loc.y,
            bottom: r.loc.y + r.size.h,
        }
    }
}

/// A window whose frame the moving window meets, bottom to top.
#[derive(Debug, Clone, Copy)]
pub(crate) struct EdgeWindow {
    pub frame: Rectangle<i32, Logical>,
    /// Whether its own edges count. A dock's don't, since its strut's are the work area's,
    /// but it still covers the edges of windows below it (edge-resistance.c:1016-1020).
    pub has_edges: bool,
}

/// The edges a grab's window meets, across each axis.
#[derive(Debug, Default)]
pub(crate) struct Edges {
    /// Edges at an x: what the window's left and right sides meet.
    vertical: Vec<Edge>,
    /// Edges at a y: what its top and bottom meet.
    horizontal: Vec<Edge>,
}

impl Edges {
    /// The edges of the work areas, and of `windows`, listed bottom to top, within the
    /// outputs' bounding box `screen` (edge-resistance.c:1026-1032).
    pub(crate) fn new(
        work_areas: impl IntoIterator<Item = Rectangle<i32, Logical>>,
        windows: &[EdgeWindow],
        screen: Rectangle<i32, Logical>,
    ) -> Self {
        let mut edges = Edges::default();
        for area in work_areas {
            edges.add_rect(area, OUTPUT_THRESHOLD, true, &[]);
        }
        for (i, window) in windows.iter().enumerate() {
            let Some(frame) = window.frame.intersection(screen) else {
                continue;
            };
            if window.has_edges {
                edges.add_rect(frame, WINDOW_THRESHOLD, false, &windows[i + 1..]);
            }
        }
        edges
    }

    /// Adds a rectangle's four edges, less what `above` covers. A work area's edges hold a
    /// side in (its left edge stops a side moving left); a window's hold one out.
    fn add_rect(
        &mut self,
        r: Rectangle<i32, Logical>,
        threshold: i32,
        inside: bool,
        above: &[EdgeWindow],
    ) {
        let s = Sides::from(r);
        let (left, right, top, bottom) = if inside {
            (Side::Left, Side::Right, Side::Top, Side::Bottom)
        } else {
            (Side::Right, Side::Left, Side::Bottom, Side::Top)
        };
        let vertical = [(s.left, left), (s.right, right)];
        let horizontal = [(s.top, top), (s.bottom, bottom)];
        for (position, side) in vertical {
            let edge = Edge {
                position,
                start: s.top,
                end: s.bottom,
                side,
                threshold,
            };
            self.vertical.extend(uncovered(edge, above));
        }
        for (position, side) in horizontal {
            let edge = Edge {
                position,
                start: s.left,
                end: s.right,
                side,
                threshold,
            };
            self.horizontal.extend(uncovered(edge, above));
        }
    }

    /// Where a window's outer rectangle moves to from `old`, when the pointer would take
    /// it to `proposed`: each side resists or snaps on its own, then the window moves by
    /// the smaller change each way, so it keeps its size, as Marco's
    /// `meta_window_edge_resistance_for_move` does (edge-resistance.c:1176-1216). A key's
    /// snap moves the window by the side that found an edge, if only one did.
    pub(crate) fn for_move(
        &self,
        old: Rectangle<i32, Logical>,
        proposed: Rectangle<i32, Logical>,
        snap: bool,
        by: By,
    ) -> Point<i32, Logical> {
        let moved = self.each_side(old, proposed, snap, by);
        // A pointer's snap measures from where the pointer would put the window; the rest
        // from where it was.
        let reference = Sides::from(if snap && by == By::Pointer {
            proposed
        } else {
            old
        });
        let key_snap = snap && by == By::Keys;
        let smaller = |a: i32, b: i32| {
            if key_snap && a == 0 {
                b
            } else if (key_snap && b == 0) || a.abs() < b.abs() {
                a
            } else {
                b
            }
        };
        let dx = smaller(moved.left - reference.left, moved.right - reference.right);
        let dy = smaller(moved.top - reference.top, moved.bottom - reference.bottom);
        (reference.left + dx, reference.top + dy).into()
    }

    /// The outer width and height a resize takes a window to from `old`, when the pointer
    /// would take it to `proposed`: the sides that move resist or snap on their own. A side
    /// can snap past the opposite one, as in Marco, so either can be 0 or less, for the
    /// size hints to clamp.
    pub(crate) fn for_resize(
        &self,
        old: Rectangle<i32, Logical>,
        proposed: Rectangle<i32, Logical>,
        snap: bool,
        by: By,
    ) -> (i32, i32) {
        let s = self.each_side(old, proposed, snap, by);
        (s.right - s.left, s.bottom - s.top)
    }

    /// Marco's `apply_edge_resistance_to_each_side` (edge-resistance.c:541-669). Its exception for resizing a window
    /// with size increments doesn't arise: policy's size hints have none.
    fn each_side(
        &self,
        old: Rectangle<i32, Logical>,
        new: Rectangle<i32, Logical>,
        snap: bool,
        by: By,
    ) -> Sides {
        let (o, n) = (Sides::from(old), Sides::from(new));
        let side = |edges: &[Edge], from: i32, to: i32| match (snap, by) {
            (true, By::Pointer) => snap_position(edges, from, to, new),
            (false, By::Pointer) => resist(edges, from, to, old, new),
            (true, By::Keys) => snap_ahead(edges, from, to, new),
            (false, By::Keys) => stop_short(edges, from, to, old, new),
        };
        Sides {
            left: side(&self.vertical, o.left, n.left),
            right: side(&self.vertical, o.right, n.right),
            top: side(&self.horizontal, o.top, n.top),
            bottom: side(&self.horizontal, o.bottom, n.bottom),
        }
    }
}

/// The parts of `edge` that no window in `above` covers. An edge along a window's own
/// border isn't covered by it (edge-resistance.c:1081-1092, boxes.c's
/// `meta_rectangle_remove_intersections_with_boxes_from_edges`).
fn uncovered(edge: Edge, above: &[EdgeWindow]) -> Vec<Edge> {
    let mut parts = vec![edge];
    for window in above {
        let s = Sides::from(window.frame);
        let (across, along) = match edge.side {
            Side::Left | Side::Right => ((s.left, s.right), (s.top, s.bottom)),
            Side::Top | Side::Bottom => ((s.top, s.bottom), (s.left, s.right)),
        };
        if !(across.0 < edge.position && edge.position < across.1) {
            continue;
        }
        parts = parts
            .into_iter()
            .flat_map(|part| {
                let before = (part.start < along.0).then_some(Edge {
                    end: along.0.min(part.end),
                    ..part
                });
                let after = (along.1 < part.end).then_some(Edge {
                    start: along.1.max(part.start),
                    ..part
                });
                before.into_iter().chain(after)
            })
            .collect();
    }
    parts
}

/// Where a side going from `from` to `to` stops: at the first edge it crosses (or starts
/// on) that holds it back and that it hasn't gone `threshold` past, as Marco's
/// `apply_edge_resistance` does for a pointer (edge-resistance.c:326-495).
fn resist(
    edges: &[Edge],
    from: i32,
    to: i32,
    old: Rectangle<i32, Logical>,
    new: Rectangle<i32, Logical>,
) -> i32 {
    if from == to {
        return to;
    }
    let increasing = to > from;
    let (low, high) = (from.min(to), from.max(to));
    let mut crossed: Vec<&Edge> = edges
        .iter()
        .filter(|e| (low..=high).contains(&e.position))
        .filter(|e| e.aligns(old) || e.aligns(new))
        .collect();
    crossed.sort_by_key(|e| if increasing { e.position } else { -e.position });
    crossed
        .into_iter()
        .find(|e| e.side.holds_back(increasing) && (e.position - to).abs() < e.threshold)
        .map_or(to, |e| e.position)
}

/// Where a side going from `from` to `to` snaps: the edge nearest `to` of any kind that the
/// window's new rectangle meets, as Marco's `apply_edge_snapping` does for a pointer
/// (edge-resistance.c:499-530).
fn snap_position(edges: &[Edge], from: i32, to: i32, new: Rectangle<i32, Logical>) -> i32 {
    if from == to {
        return to;
    }
    let nearest = edges
        .iter()
        .filter(|e| e.aligns(new))
        .map(|e| e.position)
        .min_by_key(|p| (p - to).abs())
        .unwrap_or(from);
    if (nearest - from).abs() >= SNAP_SLIP && (to - from).abs() < SNAP_SLIP {
        from
    } else {
        nearest
    }
}

/// Where a key's step of a side from `from` to `to` stops: at the first edge of any kind
/// that it would cross, short of `to`. One it starts on doesn't hold it, so the next key
/// goes on (edge-resistance.c:402-408).
fn stop_short(
    edges: &[Edge],
    from: i32,
    to: i32,
    old: Rectangle<i32, Logical>,
    new: Rectangle<i32, Logical>,
) -> i32 {
    let (low, high) = (from.min(to), from.max(to));
    edges
        .iter()
        .filter(|e| low < e.position && e.position < high)
        .filter(|e| e.aligns(old) || e.aligns(new))
        .map(|e| e.position)
        .min_by_key(|p| (p - from).abs())
        .unwrap_or(to)
}

/// Where a key's snap of a side from `from` towards `to` goes: the edge the window's new
/// rectangle meets nearest `to`, at it or beyond it, or nowhere if there's none, as
/// Marco's `find_nearest_position` with `only_forward` (edge-resistance.c:181-293).
fn snap_ahead(edges: &[Edge], from: i32, to: i32, new: Rectangle<i32, Logical>) -> i32 {
    if from == to {
        return to;
    }
    edges
        .iter()
        .filter(|e| e.aligns(new))
        .map(|e| e.position)
        .filter(|p| (p - to) * (from - to) <= 0)
        .min_by_key(|p| (p - to).abs())
        .unwrap_or(from)
}
