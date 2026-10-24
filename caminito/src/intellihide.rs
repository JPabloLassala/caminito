//! Intellihide on the compositor's side (CMN-224, ADR 0047): the layer surfaces that ask for
//! it through `caminito_intellihide_v1` are told to policy with where they are, policy says
//! which are hidden, and this slides each out of its edge or back with the Popup slide's
//! motion (ADR 0037). The panel is never told: the scene draws its surface moved off its
//! place, and a pointer finds it there, so a hidden panel is out of the output and takes no
//! input, as its input region goes with the surface.
//!
//! The pointer's rest at the edge is policy's, whose deadline joins auto-raise's. The slide's
//! own deadline, its end, is a holder of `deadlines`.

use std::collections::BTreeMap;
use std::time::Duration;

use caminito_policy::{IntellihidePanel, LayerSurfaceId, OutputId, PanelEdge};
use smithay::desktop::{LayerSurface, PopupManager};
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};
use smithay::wayland::shell::wlr_layer::Anchor;

use crate::layer::{is_mapped, policy_name};
use crate::output::{output_geometry, output_id};
use crate::popup_slide::{Edge, Motion};
use crate::state::Caminito;

/// The edge of its output a layer surface anchored as `anchor` is on, if it is on one: the
/// one it's anchored to alone, with or without both ends of the edge. A surface anchored
/// to a corner, or to two opposite edges, is on none.
fn edge_of(anchor: Anchor) -> Option<PanelEdge> {
    let has = |side| anchor.contains(side);
    let (top, bottom) = (has(Anchor::TOP), has(Anchor::BOTTOM));
    let (left, right) = (has(Anchor::LEFT), has(Anchor::RIGHT));
    let along_x = left == right;
    let along_y = top == bottom;
    match (top, bottom, left, right) {
        (true, false, ..) if along_x => Some(PanelEdge::Top),
        (false, true, ..) if along_x => Some(PanelEdge::Bottom),
        (_, _, true, false) if along_y => Some(PanelEdge::Left),
        (_, _, false, true) if along_y => Some(PanelEdge::Right),
        _ => None,
    }
}

/// How far a panel at `rect` goes to leave `output` through `edge`: all of it, and the gap
/// between it and the edge.
fn distance(
    output: Rectangle<i32, Logical>,
    edge: PanelEdge,
    rect: Rectangle<i32, Logical>,
) -> i32 {
    let far = |origin: i32, size: i32| origin + size;
    match edge {
        PanelEdge::Top => far(rect.loc.y, rect.size.h) - output.loc.y,
        PanelEdge::Bottom => far(output.loc.y, output.size.h) - rect.loc.y,
        PanelEdge::Left => far(rect.loc.x, rect.size.w) - output.loc.x,
        PanelEdge::Right => far(output.loc.x, output.size.w) - rect.loc.x,
    }
    .max(0)
}

fn slide_edge(edge: PanelEdge) -> Edge {
    match edge {
        PanelEdge::Top => Edge::Top,
        PanelEdge::Bottom => Edge::Bottom,
        PanelEdge::Left => Edge::Left,
        PanelEdge::Right => Edge::Right,
    }
}

/// One panel's slide.
#[derive(Debug)]
struct Slide {
    output: OutputId,
    edge: Edge,
    /// How far out of its place it is when it's hidden.
    distance: i32,
    /// Whether policy has it hidden, which it is once its motion is over.
    hidden: bool,
    /// The motion it's in, until it's over.
    motion: Option<Motion>,
}

impl Slide {
    /// Where the panel is when it's hidden: out of its edge, all of it.
    fn hidden_offset(&self) -> Point<i32, Logical> {
        let d = self.distance;
        match self.edge {
            Edge::Top => (0, -d),
            Edge::Bottom => (0, d),
            Edge::Left => (-d, 0),
            Edge::Right => (d, 0),
        }
        .into()
    }

    /// Where the panel is drawn at `now`, from its place.
    fn offset(&self, now: Duration) -> Point<i32, Logical> {
        match &self.motion {
            Some(motion) if !motion.is_over(now) => motion.offset(now),
            _ if self.hidden => self.hidden_offset(),
            _ => Point::default(),
        }
    }
}

/// The panels that ask for intellihide, with how far each has slid.
#[derive(Debug, Default)]
pub(crate) struct IntellihideSlides {
    panels: BTreeMap<LayerSurfaceId, Slide>,
}

impl IntellihideSlides {
    /// Where the panel `id` is drawn at `now`, from its place: nowhere off it unless it
    /// asks for intellihide.
    pub(crate) fn offset(&self, id: LayerSurfaceId, now: Duration) -> Point<i32, Logical> {
        self.panels
            .get(&id)
            .map_or_else(Point::default, |slide| slide.offset(now))
    }

    /// Whether the panel `id` is hidden or on its way, for the debug dump.
    pub(crate) fn is_hidden(&self, id: LayerSurfaceId) -> Option<bool> {
        self.panels.get(&id).map(|slide| slide.hidden)
    }

    /// When the first slide is over: `deadlines` then draws its panel at rest or out of
    /// the output (174).
    pub(crate) fn deadline(&self) -> Option<Duration> {
        self.panels
            .values()
            .filter_map(|slide| slide.motion.as_ref().map(Motion::end))
            .min()
    }

    /// Whether a panel on `output` slides at `now`, which is then drawn again at its next
    /// frame.
    pub(crate) fn sliding_on(&self, output: OutputId, now: Duration) -> bool {
        self.panels.values().any(|slide| {
            slide.output == output && slide.motion.as_ref().is_some_and(|m| !m.is_over(now))
        })
    }
}

/// A layer surface that asks for intellihide, with the output it's on.
struct Asking {
    output: Output,
    panel: IntellihidePanel,
}

impl Caminito {
    /// Where the layer surface is drawn from its place at `now`: moved toward its edge while
    /// it's hidden or on its way.
    pub(crate) fn layer_slide_offset(&self, layer: &LayerSurface) -> Point<i32, Logical> {
        let Some(id) = policy_name(layer) else {
            return Point::default();
        };
        self.intellihide_slides.offset(id, self.now())
    }

    /// The layer surfaces that ask for intellihide: mapped, on an edge of their output, with
    /// their places.
    fn intellihide_askers(&self) -> Vec<Asking> {
        let mut askers = Vec::new();
        for output in &self.outputs {
            for (layer, rect) in self.layer_surfaces(output) {
                let Some(id) = policy_name(&layer) else {
                    continue;
                };
                if !is_mapped(&layer)
                    || !self
                        .intellihide_state
                        .asks(layer.layer_surface().shell_surface())
                {
                    continue;
                }
                let Some(edge) = edge_of(layer.cached_state().anchor) else {
                    continue;
                };
                // A popup of its own, a menu or a tooltip, keeps it where the user is using it.
                let held = PopupManager::popups_for_surface(layer.wl_surface())
                    .next()
                    .is_some();
                askers.push(Asking {
                    output: output.clone(),
                    panel: IntellihidePanel {
                        id,
                        output: output_id(output),
                        rect,
                        edge,
                        held,
                    },
                });
            }
        }
        askers
    }

    /// Tells policy the panels that ask for intellihide and slides each to where policy has
    /// it, from the refresh pass after the last step that changes the windows, and before the
    /// pointer's focus, which finds a panel where it is drawn. A slide starts when policy's
    /// answer changes, runs the Popup slide's motion, and turns round from where it is. With
    /// the slide off, or the session locked, or the Overview open, a panel goes at once.
    pub(crate) fn refresh_intellihide(&mut self) {
        let askers = self.intellihide_askers();
        let panels: Vec<IntellihidePanel> = askers.iter().map(|asker| asker.panel).collect();
        self.policy.set_intellihide_panels(&panels);
        let now = self.now();
        let animate = self.popup_slides_run();
        let mut redraw: Vec<Output> = Vec::new();
        for Asking { output, panel } in &askers {
            let hidden = self.policy.is_intellihide_hidden(panel.id);
            let edge = slide_edge(panel.edge);
            let distance = distance(output_geometry(output), panel.edge, panel.rect);
            let slides = &mut self.intellihide_slides;
            match slides.panels.get_mut(&panel.id) {
                // A panel seen for the first time is where policy has it.
                None => {
                    let motion = None;
                    slides.panels.insert(
                        panel.id,
                        Slide {
                            output: panel.output,
                            edge,
                            distance,
                            hidden,
                            motion,
                        },
                    );
                    if hidden {
                        redraw.push(output.clone());
                    }
                }
                Some(slide) => {
                    let moved = slide.output != panel.output
                        || slide.edge != edge
                        || slide.distance != distance;
                    let turned = slide.hidden != hidden;
                    slide.output = panel.output;
                    slide.edge = edge;
                    slide.distance = distance;
                    slide.hidden = hidden;
                    if !animate {
                        let was_sliding = slide.motion.take().is_some();
                        if turned || moved || was_sliding {
                            redraw.push(output.clone());
                        }
                    } else if turned {
                        slide.motion = Some(match slide.motion.take() {
                            Some(motion) if !motion.is_over(now) => motion.turned(now),
                            _ => Motion::across(edge, distance, hidden, now),
                        });
                        redraw.push(output.clone());
                    } else if moved {
                        redraw.push(output.clone());
                    }
                }
            }
        }
        let gone: Vec<LayerSurfaceId> = self
            .intellihide_slides
            .panels
            .keys()
            .filter(|id| !panels.iter().any(|panel| panel.id == **id))
            .copied()
            .collect();
        for id in gone {
            if let Some(slide) = self.intellihide_slides.panels.remove(&id) {
                // A panel that stopped asking, or went, is drawn where it is, with the other
                // layer surfaces of its output.
                if let Some(output) = self.outputs.iter().find(|o| output_id(o) == slide.output) {
                    redraw.push(output.clone());
                }
            }
        }
        for output in redraw {
            self.queue_redraw(&output);
        }
    }

    /// Ends the slides that are over at `now`, for `deadlines`: the panel is drawn at rest
    /// or hidden. While one runs, the output it's on is drawn again at each of its frames.
    pub(crate) fn intellihide_elapse(&mut self, now: Duration) {
        let mut done = Vec::new();
        for slide in self.intellihide_slides.panels.values_mut() {
            if slide
                .motion
                .as_ref()
                .is_some_and(|motion| motion.is_over(now))
            {
                slide.motion = None;
                done.push(slide.output);
            }
        }
        for id in done {
            if let Some(output) = self.outputs.iter().find(|o| output_id(o) == id).cloned() {
                self.queue_redraw(&output);
            }
        }
    }

    /// Whether a panel slides on `output` at `now`.
    pub(crate) fn intellihide_slide_on(&self, output: &Output, now: Duration) -> bool {
        self.intellihide_slides.sliding_on(output_id(output), now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_surface_anchored_to_one_edge_is_on_it_with_or_without_the_whole_edge() {
        assert_eq!(edge_of(Anchor::BOTTOM), Some(PanelEdge::Bottom));
        let bar = Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT;
        assert_eq!(edge_of(bar), Some(PanelEdge::Bottom));
        let bar = Anchor::TOP | Anchor::LEFT | Anchor::RIGHT;
        assert_eq!(edge_of(bar), Some(PanelEdge::Top));
        let bar = Anchor::LEFT | Anchor::TOP | Anchor::BOTTOM;
        assert_eq!(edge_of(bar), Some(PanelEdge::Left));
        assert_eq!(edge_of(Anchor::RIGHT), Some(PanelEdge::Right));
    }

    #[test]
    fn a_surface_anchored_to_a_corner_or_to_none_or_to_all_is_on_no_edge() {
        assert_eq!(edge_of(Anchor::BOTTOM | Anchor::LEFT), None);
        assert_eq!(edge_of(Anchor::empty()), None);
        assert_eq!(edge_of(Anchor::all()), None);
        assert_eq!(edge_of(Anchor::TOP | Anchor::BOTTOM), None);
    }

    #[test]
    fn a_panel_goes_its_own_size_and_the_gap_to_its_edge() {
        let output = Rectangle::new((0, 0).into(), (1920, 1080).into());
        let floating = Rectangle::new((460, 1032).into(), (1000, 40).into());

        assert_eq!(distance(output, PanelEdge::Bottom, floating), 48);
        assert_eq!(distance(output, PanelEdge::Top, floating), 1072);
        assert_eq!(distance(output, PanelEdge::Left, floating), 1460);
        assert_eq!(distance(output, PanelEdge::Right, floating), 1460);
    }
}
