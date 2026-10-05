//! What's reserved along the outputs' edges (64): layer surfaces' exclusive zones and
//! XWayland windows' struts, combined here as Marco combines struts into each monitor's
//! work area. Policy takes the result as each output's reserved edges, and doesn't know
//! where they come from.

use camino_policy::ReservedEdges;
use smithay::desktop::layer_map_for_output;
use smithay::utils::{Logical, Rectangle};

use crate::output::{output_geometry, output_id};
use crate::state::Camino;
use crate::window::{Side, Strut, with_x_facts};

type Rect = Rectangle<i32, Logical>;

/// Marco's work area for `base`, a monitor or the X screen: the largest rectangle of it
/// that none of `reserved` overlaps. Marco splits `base` around each strut into the
/// largest rectangles clear of it (`meta_rectangle_get_minimal_spanning_set_for_region`,
/// boxes.c:524-664), and clips `base` to the one of them it overlaps most, which is the
/// largest (`meta_rectangle_clip_to_region`, boxes.c:909-970; workspace.c:656-729). Of
/// two as large, Marco takes the one its list has first and this the one found first,
/// which may differ.
///
/// The ticket (64) asked to ignore a strut wider than half its output, as Marco was thought
/// to. Marco has no such rule, so on purpose there's none here: a large strut leaves the
/// largest rectangle beside it, as any other does.
///
/// A `base` that's wholly reserved has no work area in Marco. Here nothing is reserved on
/// it then, so windows still have somewhere to go.
fn largest_free(base: Rect, reserved: &[Rect]) -> Rect {
    let mut free = vec![base];
    for strut in reserved {
        let mut split = Vec::new();
        for rect in free {
            if !rect.overlaps(*strut) {
                split.push(rect);
                continue;
            }
            let (left, top) = (rect.loc.x, rect.loc.y);
            let (right, bottom) = (left + rect.size.w, top + rect.size.h);
            let (strut_right, strut_bottom) =
                (strut.loc.x + strut.size.w, strut.loc.y + strut.size.h);
            if left < strut.loc.x {
                split.push(Rect::new(
                    rect.loc,
                    (strut.loc.x - left, rect.size.h).into(),
                ));
            }
            if right > strut_right {
                split.push(Rect::new(
                    (strut_right, top).into(),
                    (right - strut_right, rect.size.h).into(),
                ));
            }
            if top < strut.loc.y {
                split.push(Rect::new(rect.loc, (rect.size.w, strut.loc.y - top).into()));
            }
            if bottom > strut_bottom {
                split.push(Rect::new(
                    (left, strut_bottom).into(),
                    (rect.size.w, bottom - strut_bottom).into(),
                ));
            }
        }
        free = split;
    }
    let area = |rect: &Rect| i64::from(rect.size.w) * i64::from(rect.size.h);
    // `max_by_key` gives the last of the largest, so the first is the last in reverse.
    free.iter()
        .rev()
        .max_by_key(|rect| area(rect))
        .copied()
        .unwrap_or(base)
}

/// The rectangle of the X `screen` a strut reserves, as Marco's `meta_window_update_struts`
/// makes it (window.c:6400-6520): the strut's width in from the screen's edge, along its
/// span, or along the whole edge for `_NET_WM_STRUT`. It's the X screen's edge, not a
/// monitor's, so the strut counts on the monitors this rectangle is on.
fn strut_rect(strut: Strut, screen: Rect) -> Rect {
    let (far_x, far_y) = (screen.loc.x + screen.size.w, screen.loc.y + screen.size.h);
    let along = |start: i32, length: i32| match strut.span {
        Some((first, last)) => (first, last - first + 1),
        None => (start, length),
    };
    let (y, height) = along(screen.loc.y, screen.size.h);
    let (x, width) = along(screen.loc.x, screen.size.w);
    let (loc, size) = match strut.side {
        Side::Left => ((screen.loc.x, y), (strut.width, height)),
        Side::Right => ((far_x - strut.width, y), (strut.width, height)),
        Side::Top => ((x, screen.loc.y), (width, strut.width)),
        Side::Bottom => ((x, far_y - strut.width), (width, strut.width)),
    };
    Rect::new(loc.into(), size.into())
}

/// The strips along `geometry`'s edges that `reserved` takes from it.
fn edge_rects(geometry: Rect, reserved: ReservedEdges) -> Vec<Rect> {
    let Rect { loc, size } = geometry;
    [
        Rect::new(loc, (reserved.left, size.h).into()),
        Rect::new(
            (loc.x + size.w - reserved.right, loc.y).into(),
            (reserved.right, size.h).into(),
        ),
        Rect::new(loc, (size.w, reserved.top).into()),
        Rect::new(
            (loc.x, loc.y + size.h - reserved.bottom).into(),
            (size.w, reserved.bottom).into(),
        ),
    ]
    .into_iter()
    .filter(|rect| !rect.is_empty())
    .collect()
}

/// What's reserved along `geometry`'s edges when `area` of it is left.
fn edges_around(geometry: Rect, area: Rect) -> ReservedEdges {
    ReservedEdges {
        left: area.loc.x - geometry.loc.x,
        right: geometry.loc.x + geometry.size.w - area.loc.x - area.size.w,
        top: area.loc.y - geometry.loc.y,
        bottom: geometry.loc.y + geometry.size.h - area.loc.y - area.size.h,
    }
}

/// The X screen for outputs at `geometries`: from the origin to their far edges, as
/// Xwayland makes it.
fn screen(geometries: impl IntoIterator<Item = Rect>) -> Rect {
    let far = geometries.into_iter().fold((0, 0), |(w, h), geometry| {
        let edge = geometry.loc + geometry.size;
        (w.max(edge.x), h.max(edge.y))
    });
    Rect::from_size(far.into())
}

/// `_NET_WORKAREA`'s one rectangle: Marco's is the X screen's work area, the largest
/// rectangle of the screen clear of every strut (workspace.c:656-663; screen.c:2396-2435).
/// What policy has reserved along each output's edges stands for the struts here.
pub(crate) fn screen_work_area(outputs: &[camino_policy::Output]) -> Rect {
    let reserved: Vec<Rect> = outputs
        .iter()
        .flat_map(|output| edge_rects(output.geometry, output.reserved))
        .collect();
    largest_free(screen(outputs.iter().map(|o| o.geometry)), &reserved)
}

impl Camino {
    /// Policy's descriptor of each output, with what its layer surfaces' exclusive zones
    /// and the XWayland windows' struts reserve along its edges.
    pub(crate) fn policy_outputs(&self) -> Vec<camino_policy::Output> {
        let screen = screen(self.outputs.iter().map(output_geometry));
        // Of every managed XWayland window, on any workspace: policy's work area is the
        // same on each. Marco counts a workspace's windows, and a dock is on them all.
        let struts: Vec<Rect> = self
            .policy
            .windows()
            .filter_map(|mapped| mapped.window().x11_surface())
            .flat_map(|surface| with_x_facts(surface, |facts| facts.struts.clone()))
            .map(|strut| strut_rect(strut, screen))
            .collect();
        self.outputs
            .iter()
            .map(|output| {
                let geometry = output_geometry(output);
                // What Smithay's arrangement leaves of the output, from its origin.
                let mut zone = layer_map_for_output(output).non_exclusive_zone();
                zone.loc += geometry.loc;
                let mut reserved = edge_rects(geometry, edges_around(geometry, zone));
                reserved.extend(&struts);
                let area = largest_free(geometry, &reserved);
                camino_policy::Output {
                    id: output_id(output),
                    geometry,
                    scale: output.current_scale().fractional_scale(),
                    reserved: edges_around(geometry, area),
                }
            })
            .collect()
    }

    /// Tells policy what's reserved along the outputs' edges now, if that changed: a layer
    /// surface or a dock mapped, changed what it reserves, or went. Maximised windows then
    /// fill the new work areas.
    pub(crate) fn refresh_work_areas(&mut self) {
        let outputs = self.policy_outputs();
        if outputs != self.policy.outputs() {
            self.policy.set_outputs(outputs);
            self.queue_redraw_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect::new((x, y).into(), (w, h).into())
    }

    /// A 1920 × 1080 output at the origin.
    fn output() -> Rect {
        rect(0, 0, 1920, 1080)
    }

    #[test]
    fn nothing_reserved_leaves_the_whole() {
        assert_eq!(largest_free(output(), &[]), output());
    }

    #[test]
    fn struts_on_two_edges_leave_the_rectangle_between_them() {
        let reserved = [rect(0, 0, 1920, 28), rect(0, 0, 64, 1080)];
        assert_eq!(largest_free(output(), &reserved), rect(64, 28, 1856, 1052));
    }

    #[test]
    fn a_strut_along_part_of_an_edge_leaves_the_larger_side_of_it() {
        // Below it there's more than beside it.
        assert_eq!(
            largest_free(output(), &[rect(0, 0, 960, 30)]),
            rect(0, 30, 1920, 1050)
        );
        // Beside it there's more than below it.
        assert_eq!(
            largest_free(output(), &[rect(0, 0, 480, 600)]),
            rect(480, 0, 1440, 1080)
        );
    }

    #[test]
    fn a_strut_off_the_rectangle_reserves_nothing_of_it() {
        assert_eq!(largest_free(output(), &[rect(1920, 0, 1280, 28)]), output());
    }

    #[test]
    fn a_wholly_reserved_rectangle_is_left_whole() {
        assert_eq!(
            largest_free(output(), &[rect(-10, -10, 3000, 3000)]),
            output()
        );
    }

    #[test]
    fn a_strut_is_measured_from_the_edge_of_the_x_screen() {
        let screen = rect(0, 0, 3200, 1080);
        let strut = |side, span| Strut {
            side,
            width: 40,
            span,
        };
        assert_eq!(
            strut_rect(strut(Side::Left, Some((100, 199))), screen),
            rect(0, 100, 40, 100)
        );
        assert_eq!(
            strut_rect(strut(Side::Right, Some((0, 1023))), screen),
            rect(3160, 0, 40, 1024)
        );
        assert_eq!(
            strut_rect(strut(Side::Top, Some((1920, 3199))), screen),
            rect(1920, 0, 1280, 40)
        );
        assert_eq!(
            strut_rect(strut(Side::Bottom, None), screen),
            rect(0, 1040, 3200, 40)
        );
    }

    #[test]
    fn edges_and_their_strips_agree() {
        let geometry = rect(1920, 0, 1280, 1024);
        let reserved = ReservedEdges {
            left: 0,
            right: 50,
            top: 28,
            bottom: 0,
        };
        let strips = edge_rects(geometry, reserved);
        assert_eq!(strips, [rect(3150, 0, 50, 1024), rect(1920, 0, 1280, 28)]);
        assert_eq!(
            edges_around(geometry, largest_free(geometry, &strips)),
            reserved
        );
    }

    #[test]
    fn the_screens_work_area_is_its_largest_rectangle_clear_of_every_outputs_edges() {
        let with_top = |id, geometry, top| camino_policy::Output {
            id: camino_policy::OutputId(id),
            geometry,
            scale: 1.,
            reserved: ReservedEdges {
                top,
                ..ReservedEdges::default()
            },
        };
        // A bar on the first of two outputs: the screen below it is larger than the
        // second output.
        let outputs = [
            with_top(1, output(), 28),
            with_top(2, rect(1920, 0, 1280, 1024), 0),
        ];
        assert_eq!(screen_work_area(&outputs), rect(0, 28, 3200, 1052));
        assert_eq!(screen_work_area(&outputs[..1]), rect(0, 28, 1920, 1052));
        assert_eq!(screen_work_area(&[]), rect(0, 0, 0, 0));
    }
}
