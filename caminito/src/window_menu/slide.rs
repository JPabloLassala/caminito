//! The Window menu's Popup slide (ADR 0037, CMN-181): the menu slides down out of where it
//! was asked for, or up when its layout turned it to fit its output, and its submenu slides
//! out of its parent's side, right, or left when it opened to the left. Each is cut off at
//! its own rectangle, on the xdg_popups' motion (`popup_slide`).
//!
//! Policy knows nothing of it: the menu's keys, clicks, selection and choice are at its real
//! place from its first frame, and a selection that changes draws the pane again without
//! touching its slide. As the menu or its submenu closes, by a choice, Escape, a click
//! outside or Left, its buffer is kept as its picture and slides back into its edge, and
//! policy has let it go. One shown again on its way back, as a submenu opened again with
//! Right, turns round from where it is.

use std::time::Duration;

use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};

use super::{Pane, Shown};
use crate::output::output_geometry;
use crate::popup_slide::{DrawnSlide, Edge, Motion, Slid};
use crate::state::Caminito;

/// The menu, or its submenu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Part {
    Main,
    Sub,
}

/// A part of the menu on screen: its edge, and its slide while that runs.
#[derive(Debug, Clone, Copy)]
struct Showing {
    edge: Edge,
    motion: Option<Motion>,
}

/// A closed part's picture on its way back into its edge.
#[derive(Debug)]
pub(crate) struct ClosingPane {
    pub(crate) part: Part,
    /// The output it was on, which takes it away when it goes.
    pub(crate) output: Output,
    /// Its last buffer, at its place.
    pub(crate) pane: Pane,
    motion: Motion,
}

impl ClosingPane {
    /// How it's drawn at `now`.
    pub(crate) fn slid(&self, now: Duration) -> Slid {
        self.motion.slid(now)
    }
}

/// The Window menu's parts on screen and the pictures of those on their way back.
#[derive(Debug, Default)]
pub(crate) struct MenuSlides {
    main: Option<Showing>,
    sub: Option<Showing>,
    closing: Vec<ClosingPane>,
}

impl MenuSlides {
    fn showing(&mut self, part: Part) -> &mut Option<Showing> {
        match part {
            Part::Main => &mut self.main,
            Part::Sub => &mut self.sub,
        }
    }

    /// `part`'s slide out of its edge, while it runs at `now`.
    fn sliding(&self, part: Part, now: Duration) -> Option<Motion> {
        let showing = match part {
            Part::Main => self.main,
            Part::Sub => self.sub,
        };
        showing?.motion.filter(|motion| !motion.is_over(now))
    }

    /// How `part`, on screen, is drawn at `now`, while it slides.
    pub(crate) fn slid(&self, part: Part, now: Duration) -> Option<Slid> {
        Some(self.sliding(part, now)?.slid(now))
    }

    /// The pictures on their way back at `now`, those that closed last first.
    pub(crate) fn closing(&self, now: Duration) -> impl Iterator<Item = &ClosingPane> {
        self.closing
            .iter()
            .rev()
            .filter(move |closing| !closing.motion.is_over(now))
    }

    /// When the first of the slides is over.
    pub(crate) fn deadline(&self) -> Option<Duration> {
        let shown = [self.main, self.sub].into_iter().flatten();
        let opening = shown.filter_map(|showing| showing.motion).map(|m| m.end());
        let closing = self.closing.iter().map(|closing| closing.motion.end());
        opening.chain(closing).min()
    }

    /// Ends the slides that are over at `now`. Returns whether any ended.
    pub(crate) fn elapse(&mut self, now: Duration) -> bool {
        let mut ended = false;
        for showing in [&mut self.main, &mut self.sub].into_iter().flatten() {
            if showing.motion.is_some_and(|motion| motion.is_over(now)) {
                showing.motion = None;
                ended = true;
            }
        }
        let before = self.closing.len();
        self.closing.retain(|closing| !closing.motion.is_over(now));
        ended || self.closing.len() != before
    }

    /// Ends every slide when `stops`, and drops the pictures whose output is no longer one
    /// of `outputs`. Returns whether any ended.
    pub(crate) fn refresh(&mut self, stops: bool, outputs: &[Output]) -> bool {
        let mut ended = false;
        for showing in [&mut self.main, &mut self.sub].into_iter().flatten() {
            if stops && showing.motion.take().is_some() {
                ended = true;
            }
        }
        let before = self.closing.len();
        self.closing
            .retain(|closing| !stops && outputs.contains(&closing.output));
        ended || self.closing.len() != before
    }
}

/// The menu's edge: its top, as it slides down out of where it was asked for at `asked`,
/// from the output's corner, or its bottom when its layout turned it up to fit the output.
fn main_edge(main: &Pane, asked: Point<i32, Logical>) -> Edge {
    if main.layout.rect.loc.y < asked.y {
        Edge::Bottom
    } else {
        Edge::Top
    }
}

/// The submenu's edge: the side it shares with its parent, its left as it opens to the
/// right of the menu, or its right as it opens to the left.
fn sub_edge(main: &Pane, sub: &Pane) -> Edge {
    if sub.layout.rect.loc.x < main.layout.rect.loc.x {
        Edge::Right
    } else {
        Edge::Left
    }
}

impl Caminito {
    /// Follows the menu from `before`, as it was shown, to its panes `main` and `sub` just
    /// drawn on `output`, asked for at `asked` from the output's corner: a menu newly shown
    /// slides out, and so does a submenu that opens, and one that closes slides back.
    pub(super) fn window_menu_slides_follow(
        &mut self,
        before: Option<Shown>,
        output: &Output,
        asked: Point<i32, Logical>,
        main: &Pane,
        sub: Option<&Pane>,
    ) {
        let had_sub = match before {
            None => {
                self.window_menu_part_opens(Part::Main, output, main, main_edge(main, asked));
                None
            }
            Some(before) => before.submenu.map(|pane| (before.output, pane)),
        };
        match (had_sub, sub) {
            (None, Some(sub)) => {
                self.window_menu_part_opens(Part::Sub, output, sub, sub_edge(main, sub));
            }
            (Some((output, pane)), None) => self.window_menu_part_closes(Part::Sub, output, pane),
            _ => {}
        }
    }

    /// The menu `shown` is gone from policy: its submenu and itself slide back.
    pub(super) fn window_menu_closes(&mut self, shown: Option<Shown>) {
        let Some(shown) = shown else {
            return;
        };
        if let Some(sub) = shown.submenu {
            self.window_menu_part_closes(Part::Sub, shown.output.clone(), sub);
        }
        self.window_menu_part_closes(Part::Main, shown.output, shown.main);
    }

    /// `part`, drawn as `pane` on `output`, is shown, with its edge: it slides out, or turns
    /// round if its picture is on its way back from the same place.
    fn window_menu_part_opens(&mut self, part: Part, output: &Output, pane: &Pane, edge: Edge) {
        let now = self.now();
        let motion = self.popup_slides_run().then(|| {
            let slides = &mut self.popup_slides.window_menu;
            let back = slides.closing.iter().position(|closing| {
                closing.part == part
                    && closing.output == *output
                    && closing.pane.layout.rect == pane.layout.rect
                    && !closing.motion.is_over(now)
            });
            match back {
                Some(index) => slides.closing.remove(index).motion.turned(now),
                None => self.motion(edge, pane.layout.rect.size, false),
            }
        });
        *self.popup_slides.window_menu.showing(part) = Some(Showing { edge, motion });
        self.queue_redraw_all();
    }

    /// `part`, last drawn as `pane` on `output`, is gone: its buffer slides back into its
    /// edge, turning round if it was on its way out. With no buffer, it goes at once.
    fn window_menu_part_closes(&mut self, part: Part, output: Output, pane: Pane) {
        let showing = self.popup_slides.window_menu.showing(part).take();
        self.queue_redraw_all();
        let Some(showing) = showing else {
            return;
        };
        if !self.popup_slides_run() || pane.chrome.is_none() {
            return;
        }
        let now = self.now();
        let motion = match showing.motion.filter(|motion| !motion.is_over(now)) {
            Some(opening) => opening.turned(now),
            None => self.motion(showing.edge, pane.layout.rect.size, true),
        };
        self.popup_slides.window_menu.closing.push(ClosingPane {
            part,
            output,
            pane,
            motion,
        });
    }

    /// Each part of the menu that slides at `now`, out and then back, as it's drawn, for the
    /// debug dump, with whether it's on its way back: the menu before its submenu, and the
    /// pictures in the order they closed.
    pub(crate) fn drawn_window_menu_slides(&self, now: Duration) -> Vec<(bool, DrawnSlide)> {
        let slides = &self.popup_slides.window_menu;
        let mut drawn = Vec::new();
        if let Some(shown) = self.shown_window_menu() {
            let origin = output_geometry(&shown.output).loc;
            let panes = [
                (Part::Main, Some(&shown.main)),
                (Part::Sub, shown.submenu.as_ref()),
            ];
            for (part, pane) in panes {
                let (Some(pane), Some(motion)) = (pane, slides.sliding(part, now)) else {
                    continue;
                };
                let rect = pane.layout.rect;
                drawn.push((
                    false,
                    DrawnSlide {
                        rect: Rectangle::new(origin + rect.loc, rect.size),
                        edge: motion.edge(),
                        slid: motion.slid(now),
                    },
                ));
            }
        }
        for closing in slides.closing.iter().filter(|c| !c.motion.is_over(now)) {
            let rect = closing.pane.layout.rect;
            let origin = output_geometry(&closing.output).loc;
            drawn.push((
                true,
                DrawnSlide {
                    rect: Rectangle::new(origin + rect.loc, rect.size),
                    edge: closing.motion.edge(),
                    slid: closing.slid(now),
                },
            ));
        }
        drawn
    }
}
