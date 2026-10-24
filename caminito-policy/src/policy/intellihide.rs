//! Intellihide (CMN-224, ADR 0047): when a panel that asks for it is hidden. It hides while a
//! window on its output's active workspace overlaps it, and shows when none does, or when
//! the pointer has rested at its edge for `REST`. It hides again when the pointer leaves
//! and something still overlaps. How the hiding is drawn is the compositor's.
//!
//! The overlap is worked out from the windows when it's asked, so a window that moves,
//! maps, goes or changes workspace needs no event. Only the pointer's rest has a deadline,
//! which joins auto-raise's in `Policy::deadline`.

use std::time::Duration;

use smithay::utils::{Logical, Point, Rectangle};

use crate::{LayerSurfaceId, OutputId, PolicyWindow};

use super::Policy;

/// How long the pointer rests at a hidden panel's edge before the panel shows: about a
/// quarter of a second, as the spec has it.
const REST: Duration = Duration::from_millis(250);

/// The edge of its output that a panel is on, which it hides off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelEdge {
    /// The output's top edge.
    Top,
    /// The output's bottom edge.
    Bottom,
    /// The output's left edge.
    Left,
    /// The output's right edge.
    Right,
}

/// A Layer surface that asks for intellihide, as the compositor describes it each time they
/// change. Policy manages none: it only says which are hidden.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntellihidePanel {
    /// The layer surface.
    pub id: LayerSurfaceId,
    /// The output the panel is on.
    pub output: OutputId,
    /// Where the panel is when it's shown, globally. A panel that floats off its edge has
    /// the gap between it and the edge outside this.
    pub rect: Rectangle<i32, Logical>,
    /// The edge it's on, and slides off.
    pub edge: PanelEdge,
    /// Whether the panel has a popup open, a menu or a tooltip, which keeps it shown.
    pub held: bool,
}

/// Policy's state for the panels that ask for intellihide.
#[derive(Debug, Default)]
pub(super) struct Intellihide {
    panels: Vec<Tracked>,
    /// Where the pointer was when the compositor last said, for a popup that closes with
    /// no motion after it.
    pointer: Option<Point<f64, Logical>>,
}

#[derive(Debug)]
struct Tracked {
    panel: IntellihidePanel,
    /// Whether the pointer keeps the panel shown over what overlaps it: it rested at the
    /// panel's edge, or it is on the panel while nothing overlaps, until it leaves.
    revealed: bool,
    /// Since when the pointer has rested at the edge of the panel while it was hidden.
    resting: Option<Duration>,
}

/// The part of `output` along `edge` that the pointer rests in: its last row or column.
fn edge_strip(
    output: Rectangle<i32, Logical>,
    edge: PanelEdge,
    rect: Rectangle<i32, Logical>,
) -> Rectangle<f64, Logical> {
    let output = output.to_f64();
    let rect = rect.to_f64();
    let (loc, size) = match edge {
        PanelEdge::Top => ((rect.loc.x, output.loc.y), (rect.size.w, 1.)),
        PanelEdge::Bottom => (
            (rect.loc.x, output.loc.y + output.size.h - 1.),
            (rect.size.w, 1.),
        ),
        PanelEdge::Left => ((output.loc.x, rect.loc.y), (1., rect.size.h)),
        PanelEdge::Right => (
            (output.loc.x + output.size.w - 1., rect.loc.y),
            (1., rect.size.h),
        ),
    };
    Rectangle::new(loc.into(), size.into())
}

/// The panel and the gap between it and its edge, which a pointer there is still on.
fn zone(
    output: Rectangle<i32, Logical>,
    edge: PanelEdge,
    rect: Rectangle<i32, Logical>,
) -> Rectangle<f64, Logical> {
    let strip = edge_strip(output, edge, rect);
    let rect = rect.to_f64();
    let (left, top) = (rect.loc.x.min(strip.loc.x), rect.loc.y.min(strip.loc.y));
    let right = (rect.loc.x + rect.size.w).max(strip.loc.x + strip.size.w);
    let bottom = (rect.loc.y + rect.size.h).max(strip.loc.y + strip.size.h);
    Rectangle::new((left, top).into(), (right - left, bottom - top).into())
}

impl<W: PolicyWindow> Policy<W> {
    /// The panels that ask for intellihide now, replacing the last list. A panel that's in
    /// both keeps what the pointer has done for it, and one that's new starts hidden if
    /// something overlaps it. A panel whose popup closes with the pointer off it hides again,
    /// as it would have when the pointer left it, since no motion may follow.
    pub fn set_intellihide_panels(&mut self, panels: &[IntellihidePanel]) {
        let mut old = std::mem::take(&mut self.intellihide.panels);
        let pointer = self.intellihide.pointer;
        self.intellihide.panels = panels
            .iter()
            .map(|panel| {
                let kept = old.iter().position(|tracked| tracked.panel.id == panel.id);
                let mut tracked = kept.map(|i| old.swap_remove(i)).unwrap_or(Tracked {
                    panel: *panel,
                    revealed: false,
                    resting: None,
                });
                let released = tracked.panel.held && !panel.held;
                tracked.panel = *panel;
                if released && !pointer.is_some_and(|at| self.is_on(panel, at)) {
                    tracked.revealed = false;
                }
                tracked
            })
            .collect();
    }

    /// Whether the panel is hidden: something overlaps it, nothing holds it shown and the
    /// pointer hasn't revealed it. A panel that doesn't ask for intellihide is never hidden.
    pub fn is_intellihide_hidden(&self, id: LayerSurfaceId) -> bool {
        self.intellihide
            .panels
            .iter()
            .find(|tracked| tracked.panel.id == id)
            .is_some_and(|tracked| self.hides(tracked))
    }

    /// The pointer is at `at` at `now`, which the compositor says at each motion. A pointer
    /// that leaves a revealed panel hides it again, and one that arrives at a hidden panel's
    /// edge starts the rest that shows it.
    pub fn intellihide_pointer(&mut self, at: Point<f64, Logical>, now: Duration) {
        self.intellihide.pointer = Some(at);
        let panels = std::mem::take(&mut self.intellihide.panels);
        self.intellihide.panels = panels
            .into_iter()
            .map(|mut tracked| {
                let panel = tracked.panel;
                let output = self.output_rect(&panel);
                let on_it = self.is_on(&panel, at);
                if !on_it && !panel.held {
                    tracked.revealed = false;
                } else if on_it && !self.overlapped(&panel) {
                    // On a panel nothing overlaps: a window that moves over it later
                    // doesn't take it out from under the pointer.
                    tracked.revealed = true;
                }
                let at_the_edge = edge_strip(output, panel.edge, panel.rect).contains(at);
                tracked.resting = match tracked.resting {
                    _ if !self.hides(&tracked) || !at_the_edge => None,
                    resting => resting.or(Some(now)),
                };
                tracked
            })
            .collect();
    }

    /// When the first rest at a hidden panel's edge is over.
    pub(super) fn intellihide_deadline(&self) -> Option<Duration> {
        self.intellihide
            .panels
            .iter()
            .filter(|tracked| self.hides(tracked))
            .filter_map(|tracked| tracked.resting)
            .map(|since| since + REST)
            .min()
    }

    /// Shows the panels whose rest at their edge is over at `now`.
    pub(super) fn intellihide_elapse(&mut self, now: Duration) {
        let due: Vec<LayerSurfaceId> = self
            .intellihide
            .panels
            .iter()
            .filter(|tracked| self.hides(tracked))
            .filter(|tracked| tracked.resting.is_some_and(|since| since + REST <= now))
            .map(|tracked| tracked.panel.id)
            .collect();
        for tracked in &mut self.intellihide.panels {
            if due.contains(&tracked.panel.id) {
                tracked.revealed = true;
            }
            // A rest only counts while the panel is hidden.
            if tracked.revealed {
                tracked.resting = None;
            }
        }
    }

    fn hides(&self, tracked: &Tracked) -> bool {
        !tracked.revealed && !tracked.panel.held && self.overlapped(&tracked.panel)
    }

    /// Whether a pointer at `at` is on the panel or in the gap between it and its edge.
    fn is_on(&self, panel: &IntellihidePanel, at: Point<f64, Logical>) -> bool {
        zone(self.output_rect(panel), panel.edge, panel.rect).contains(at)
    }

    /// The output of a panel, or its own rectangle if policy has no such output.
    fn output_rect(&self, panel: &IntellihidePanel) -> Rectangle<i32, Logical> {
        self.outputs
            .iter()
            .find(|output| output.id == panel.output)
            .map_or(panel.rect, |output| output.geometry)
    }

    /// Whether a window on the panel's output's active workspace overlaps the panel: one
    /// that's shown by that workspace, not minimised, and not the desktop or a dock. A
    /// window is where it's asked to be, so a panel starts to go as a window starts to grow
    /// over it, and not once its client has committed the new size.
    fn overlapped(&self, panel: &IntellihidePanel) -> bool {
        let workspace = self.active_workspace_on(panel.output);
        self.windows.keys().any(|&id| {
            !self.is_desktop_or_dock(id)
                && self.is_on_workspace(id, workspace)
                && self.is_visible_on_workspace(id)
                && self
                    .asked_frame_rect(id)
                    .is_some_and(|frame| frame.overlaps(panel.rect))
        })
    }
}
