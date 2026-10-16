//! The Window menu's Popup slide (ADR 0037, CMN-181): the menu slides down out of where it
//! was asked for, or up when its layout turned it to fit its output, and its submenu slides
//! out of its parent's side, right, or left when it opened to the left. Each is cut off at
//! its own rectangle, on the xdg_popups' motion (`popup_slide`).
//!
//! `Layout::at` turns a menu by moving it up as far as it has to, as Marco's does, rather
//! than flipping it over the point: a menu that lies above where it was asked for is turned,
//! and comes up out of its bottom edge.
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
    Menu,
    Submenu,
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
    menu: Option<Showing>,
    submenu: Option<Showing>,
    closing: Vec<ClosingPane>,
}

impl MenuSlides {
    fn showing(&mut self, part: Part) -> &mut Option<Showing> {
        match part {
            Part::Menu => &mut self.menu,
            Part::Submenu => &mut self.submenu,
        }
    }

    /// `part`'s slide out of its edge, while it runs at `now`.
    fn sliding(&self, part: Part, now: Duration) -> Option<Motion> {
        let showing = match part {
            Part::Menu => self.menu,
            Part::Submenu => self.submenu,
        };
        showing?.motion.filter(|motion| !motion.is_over(now))
    }

    /// How `part`, on screen, is drawn at `now`, while it slides.
    pub(crate) fn slid(&self, part: Part, now: Duration) -> Option<Slid> {
        Some(self.sliding(part, now)?.slid(now))
    }

    /// The pictures on their way back at `now`, in the order they closed.
    fn closing_now(&self, now: Duration) -> impl DoubleEndedIterator<Item = &ClosingPane> {
        self.closing
            .iter()
            .filter(move |closing| !closing.motion.is_over(now))
    }

    /// The pictures on their way back at `now`, those that closed last first.
    pub(crate) fn closing(&self, now: Duration) -> impl Iterator<Item = &ClosingPane> {
        self.closing_now(now).rev()
    }

    /// Takes `part`'s picture on its way back at `now` from `rect` on `output`, and returns
    /// its slide turned round, for the part shown there again.
    fn take_returning(
        &mut self,
        part: Part,
        output: &Output,
        rect: Rectangle<i32, Logical>,
        now: Duration,
    ) -> Option<Motion> {
        let index = self.closing.iter().position(|closing| {
            closing.part == part
                && closing.output == *output
                && closing.pane.layout.rect == rect
                && !closing.motion.is_over(now)
        })?;
        Some(self.closing.remove(index).motion.turned(now))
    }

    /// When the first of the slides is over.
    pub(crate) fn deadline(&self) -> Option<Duration> {
        let shown = [self.menu, self.submenu].into_iter().flatten();
        let opening = shown.filter_map(|showing| showing.motion).map(|m| m.end());
        let closing = self.closing.iter().map(|closing| closing.motion.end());
        opening.chain(closing).min()
    }

    /// Ends the slides out that `ends` and drops the pictures it doesn't `keep`. Returns
    /// whether any ended.
    fn end(&mut self, ends: impl Fn(&Motion) -> bool, keep: impl Fn(&ClosingPane) -> bool) -> bool {
        let mut ended = false;
        for showing in [&mut self.menu, &mut self.submenu].into_iter().flatten() {
            if showing.motion.as_ref().is_some_and(&ends) {
                showing.motion = None;
                ended = true;
            }
        }
        let before = self.closing.len();
        self.closing.retain(keep);
        ended || self.closing.len() != before
    }

    /// Ends the slides that are over at `now`. Returns whether any ended.
    pub(crate) fn elapse(&mut self, now: Duration) -> bool {
        self.end(
            |motion| motion.is_over(now),
            |closing| !closing.motion.is_over(now),
        )
    }

    /// Ends every slide when `stops`, and drops the pictures whose output is no longer one
    /// of `outputs`. Returns whether any ended.
    pub(crate) fn refresh(&mut self, stops: bool, outputs: &[Output]) -> bool {
        self.end(
            |_| stops,
            |closing| !stops && outputs.contains(&closing.output),
        )
    }
}

/// The menu's edge: its top, as it slides down out of where it was asked for at `asked`,
/// from the output's corner, or its bottom when its layout turned it up to fit the output.
fn menu_edge(menu: &Pane, asked: Point<i32, Logical>) -> Edge {
    if menu.layout.rect.loc.y < asked.y {
        Edge::Bottom
    } else {
        Edge::Top
    }
}

/// The submenu's edge: the side it shares with its parent, its left as it opens to the
/// right of the menu, or its right as it opens to the left. One kept on a narrow output over
/// the menu goes by which side of the menu's middle its own is.
fn submenu_edge(menu: &Pane, submenu: &Pane) -> Edge {
    let twice_middle = |pane: &Pane| 2 * pane.layout.rect.loc.x + pane.layout.rect.size.w;
    if twice_middle(submenu) < twice_middle(menu) {
        Edge::Right
    } else {
        Edge::Left
    }
}

/// A part's slide at `now` as it's drawn, at `rect` on `output`, for the debug dump.
fn drawn(
    output: &Output,
    rect: Rectangle<i32, Logical>,
    motion: Motion,
    now: Duration,
) -> DrawnSlide {
    DrawnSlide {
        rect: Rectangle::new(output_geometry(output).loc + rect.loc, rect.size),
        edge: motion.edge(),
        slid: motion.slid(now),
    }
}

impl Caminito {
    /// Follows the menu from `before`, as it was shown, to its panes `menu` and `submenu`
    /// just drawn on `output`, asked for at `asked` from the output's corner: a menu newly
    /// shown slides out, and so does a submenu that opens, and one that closes slides back.
    pub(super) fn window_menu_slides_follow(
        &mut self,
        before: Option<Shown>,
        output: &Output,
        asked: Point<i32, Logical>,
        menu: &Pane,
        submenu: Option<&Pane>,
    ) {
        let had_submenu = match before {
            None => {
                let edge = menu_edge(menu, asked);
                self.window_menu_part_opens(Part::Menu, output, menu, edge);
                None
            }
            Some(before) => before.submenu.map(|pane| (before.output, pane)),
        };
        match (had_submenu, submenu) {
            (None, Some(submenu)) => {
                let edge = submenu_edge(menu, submenu);
                self.window_menu_part_opens(Part::Submenu, output, submenu, edge);
            }
            (Some((output, pane)), None) => {
                self.window_menu_part_closes(Part::Submenu, output, pane);
            }
            _ => {}
        }
    }

    /// The menu `shown` is gone from policy: its submenu and itself slide back.
    pub(super) fn window_menu_closes(&mut self, shown: Option<Shown>) {
        let Some(shown) = shown else {
            return;
        };
        if let Some(submenu) = shown.submenu {
            self.window_menu_part_closes(Part::Submenu, shown.output.clone(), submenu);
        }
        self.window_menu_part_closes(Part::Menu, shown.output, shown.main);
    }

    /// `part`, drawn as `pane` on `output`, is shown, with its edge: it slides out, or turns
    /// round if its picture is on its way back from the same place.
    fn window_menu_part_opens(&mut self, part: Part, output: &Output, pane: &Pane, edge: Edge) {
        let now = self.now();
        let rect = pane.layout.rect;
        let motion = self.popup_slides_run().then(|| {
            let slides = &mut self.popup_slides.window_menu;
            slides
                .take_returning(part, output, rect, now)
                .unwrap_or_else(|| self.motion(edge, rect.size, false))
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

    /// Each part of the menu on screen that slides out at `now`, the menu before its
    /// submenu, as it's drawn, for the debug dump.
    pub(crate) fn drawn_window_menu_slides(&self, now: Duration) -> Vec<DrawnSlide> {
        let slides = &self.popup_slides.window_menu;
        let Some(shown) = self.shown_window_menu() else {
            return Vec::new();
        };
        [Part::Menu, Part::Submenu]
            .into_iter()
            .filter_map(|part| {
                let rect = shown.pane(part)?.layout.rect;
                Some(drawn(&shown.output, rect, slides.sliding(part, now)?, now))
            })
            .collect()
    }

    /// Each closed part's picture that slides back at `now`, in the order they closed, as
    /// it's drawn, for the debug dump.
    pub(crate) fn drawn_window_menu_pictures(&self, now: Duration) -> Vec<DrawnSlide> {
        let slides = &self.popup_slides.window_menu;
        slides
            .closing_now(now)
            .map(|closing| {
                drawn(
                    &closing.output,
                    closing.pane.layout.rect,
                    closing.motion,
                    now,
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window_menu::layout::Layout;

    fn pane(x: i32, y: i32, w: i32, h: i32) -> Pane {
        let rect = Rectangle::new((x, y).into(), (w, h).into());
        Pane {
            layout: Layout {
                rect,
                rows: Vec::new(),
            },
            chrome: None,
        }
    }

    #[test]
    fn a_menu_slides_down_unless_it_was_moved_up_to_fit() {
        let asked = Point::from((100, 300));
        assert_eq!(menu_edge(&pane(100, 300, 200, 300), asked), Edge::Top);
        // Moved left to fit is still down.
        assert_eq!(menu_edge(&pane(60, 300, 200, 300), asked), Edge::Top);
        assert_eq!(menu_edge(&pane(100, 250, 200, 300), asked), Edge::Bottom);
    }

    #[test]
    fn a_submenu_slides_out_of_the_side_of_its_menu_it_lies_on() {
        let menu = pane(400, 100, 200, 300);
        assert_eq!(submenu_edge(&menu, &pane(598, 150, 120, 100)), Edge::Left);
        assert_eq!(submenu_edge(&menu, &pane(282, 150, 120, 100)), Edge::Right);
        // Kept over the menu on a narrow output, right of its middle.
        assert_eq!(submenu_edge(&menu, &pane(500, 150, 120, 100)), Edge::Left);
    }
}
