//! The Workspace slide (233, ADR 0033): on a change of an output's active workspace, the
//! old workspace's windows leave and the new one's arrive, along the line between the two
//! in the Workspace grid, for 250 ms of Caminito's clock. It's Plasma's Slide effect.
//!
//! Policy has switched from the start, as it has a window minimised from the start of the
//! minimise animation (172): the focus, input and what clients are told follow it at once,
//! and only the drawing takes its time. The refresh pass takes the switches policy noted
//! and the windows that were on screen at the pass before, and the scene draws the windows
//! that left and those that came away from their own places until the slide's over. A
//! window that shows before and after is none of the slide's and stays still (234): a
//! sticky one, one that a `move-to-workspace-*` binding carries, and one being dragged,
//! which policy takes along.
//!
//! A switch while a slide runs retargets it (235): the windows go on from where they are
//! to the new workspace, for another 250 ms, and each workspace the slide has left is drawn
//! at its own place in the grid as it goes by. A workspace that a wrap round the grid
//! comes back to is drawn where it leaves and where it arrives, as Plasma draws it (239):
//! its windows are each one window to policy and to input, and are only drawn elsewhere.
//! The session lock ends a slide at once.
//!
//! With the Overview open on the output (244) a switch slides its laid-out windows: the
//! scene draws each workspace's layout where the slide has that workspace. A slide that
//! started under the Overview ends when it closes, and one that started without it when it
//! opens, since what they move isn't drawn any more.
//!
//! Each output has its own slide (236), of the windows it's the Main output of, and the
//! scene draws them away from their places on that output alone: nothing of a slide shows
//! on another output, where a window across the two is at its place or isn't drawn. Under
//! Spanning workspaces every output switches, and each slides by its own size.

use std::collections::HashSet;
use std::time::Duration;

use caminito_policy::{OutputId, OutputSwitch, PolicyWindow, WindowId};
use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::output::{output_geometry, output_id};
use crate::state::Caminito;

/// How long a slide lasts (ADR 0033).
pub(crate) const LENGTH: Duration = Duration::from_millis(250);

/// How much of its way a slide has left `elapsed` after its start, from 1 to 0: a cubic
/// ease-out, so the windows start at their fastest and slow down into their places.
fn way_left(elapsed: Duration) -> f64 {
    let time_left = 1.0 - (elapsed.as_secs_f64() / LENGTH.as_secs_f64()).min(1.0);
    time_left.powi(3)
}

/// A workspace that a slide has left, with its windows that were on screen.
#[derive(Debug)]
struct Left {
    workspace: usize,
    /// Where it ends up, from the place of the workspace that arrives: the output's width
    /// for each column of the Workspace grid the slide went between the two and its height
    /// for each row, with no gap.
    place: Point<i32, Logical>,
    windows: HashSet<WindowId>,
}

/// One output's windows on their way between two workspaces.
#[derive(Debug)]
pub(crate) struct Slide {
    /// The output that switched.
    pub(crate) output: OutputId,
    /// The workspace the last switch left.
    pub(crate) from: usize,
    /// The one whose windows arrive.
    pub(crate) to: usize,
    /// How far the arriving windows were from their own places at `start`: the whole way
    /// from the old workspace's place, or from where a slide that this one retargets had
    /// them (235).
    way: Point<i32, Logical>,
    /// When the output last switched, on Caminito's clock.
    start: Duration,
    /// The workspaces whose windows leave: the one the last switch left, which is the
    /// first, and, in a slide that was retargeted, those before it. A workspace that wraps
    /// round the grid took the output back to is here once for each of its places, and
    /// can be the one that arrives too (239).
    left: Vec<Left>,
    /// The new workspace's windows, which weren't on screen.
    arriving: HashSet<WindowId>,
    /// Whether the Overview was open on the output when it started (244): it moves the
    /// Overview's layouts then, and the windows as they are otherwise.
    under_overview: bool,
}

impl Slide {
    fn is_over(&self, now: Duration) -> bool {
        now.saturating_sub(self.start) >= LENGTH
    }

    /// A slide from `switch.from`'s place to `switch.to`'s on an output of `size`, which
    /// has no windows until `with_windows` gives it them.
    fn new(switch: &OutputSwitch, size: Size<i32, Logical>, now: Duration) -> Self {
        let distance = Point::from((switch.cells.0 * size.w, switch.cells.1 * size.h));
        Self {
            output: switch.output,
            from: switch.from,
            to: switch.to,
            way: distance,
            start: now,
            left: vec![Left {
                workspace: switch.from,
                place: Point::default() - distance,
                windows: HashSet::new(),
            }],
            arriving: HashSet::new(),
            under_overview: false,
        }
    }

    /// The slide with the windows of the workspace the last switch left, `leaving`, and
    /// of the one it went to, `arriving`.
    fn with_windows(mut self, leaving: HashSet<WindowId>, arriving: HashSet<WindowId>) -> Self {
        let last_left = self.left.first_mut();
        last_left
            .expect("a slide has the workspace it left")
            .windows = leaving;
        self.arriving = arriving;
        self
    }

    /// The slide that goes on from where `running` has the windows at `now` to
    /// `switch.to`, with the workspaces `running` had left at their places from the new
    /// one. The workspace that the output comes back to isn't one of them when that place
    /// is the one it arrives at, as it is after a switch back. After a wrap round the grid
    /// it's another place, and the workspace stays there too, so its windows are where
    /// they were (239). No two of the places of one workspace are the same: a switch
    /// moves them all by the one distance.
    fn retargeted(
        running: Slide,
        switch: &OutputSwitch,
        size: Size<i32, Logical>,
        now: Duration,
    ) -> Self {
        let mut slide = Self::new(switch, size, now);
        let distance = slide.way;
        // The place of the workspace that arrives, which the others' are from.
        let arrives_at = Point::default();
        slide.way += running.offset(now);
        let passed = running
            .left
            .into_iter()
            .map(|left| Left {
                place: left.place - distance,
                ..left
            })
            .filter(|left| left.workspace != switch.to || left.place != arrives_at);
        slide.left.extend(passed);
        slide
    }

    /// How far the arriving windows are from their own places at `now`. Those of a
    /// workspace the slide left are that far from where their workspace ends up, off the
    /// output.
    pub(crate) fn offset(&self, now: Duration) -> Point<i32, Logical> {
        let remaining = way_left(now.saturating_sub(self.start));
        let at = |way: i32| (f64::from(way) * remaining).round() as i32;
        Point::from((at(self.way.x), at(self.way.y)))
    }

    /// How far from its own place the slide draws `window` at `now`, for each place it
    /// draws it at: one that arrives, while it shows, and one that leaves, while it
    /// doesn't. One that arrives is also drawn where its workspace left, after a wrap
    /// round the grid (239). None for a window that's none of the slide's, which is drawn
    /// as policy has it.
    ///
    /// Its own place is the last: the scene then draws it there under its other drawings,
    /// so when they go at the slide's end its place in the frame's elements changes and
    /// the damage tracker draws again where they were.
    pub(crate) fn shifts(
        &self,
        window: WindowId,
        showing: bool,
        now: Duration,
    ) -> Vec<Point<i32, Logical>> {
        let arrives = showing && self.arriving.contains(&window);
        if showing && !arrives {
            return Vec::new();
        }
        let offset = self.offset(now);
        let its_own = self
            .left
            .iter()
            .filter(|left| left.windows.contains(&window));
        let leaving = its_own.map(|left| offset + left.place);
        leaving.chain(arrives.then_some(offset)).collect()
    }

    /// The workspaces the slide leaves, each one time, the one the last switch left first.
    /// The one that arrives is among them after a wrap round the grid (239).
    pub(crate) fn left_workspaces(&self) -> Vec<usize> {
        let mut workspaces = Vec::new();
        for left in &self.left {
            if !workspaces.contains(&left.workspace) {
                workspaces.push(left.workspace);
            }
        }
        workspaces
    }

    /// How far from its own place the slide draws what's on `workspace` at `now`, for each
    /// place it draws that workspace at, as `shifts` has it for a window: where it ended up
    /// for each time the slide left it, and last where it arrives, if it's the one that
    /// does. None for a workspace that's none of the slide's. The Overview's layouts are
    /// drawn so (244).
    pub(crate) fn workspace_shifts(
        &self,
        workspace: usize,
        now: Duration,
    ) -> Vec<Point<i32, Logical>> {
        let offset = self.offset(now);
        let left = self.left.iter().filter(|left| left.workspace == workspace);
        let leaving = left.map(|left| offset + left.place);
        leaving
            .chain((workspace == self.to).then_some(offset))
            .collect()
    }

    /// The windows the slide moves, or would if they showed as they did at its start,
    /// each one time.
    fn windows(&self) -> HashSet<WindowId> {
        let left = self.left.iter().flat_map(|left| &left.windows);
        left.chain(&self.arriving).copied().collect()
    }
}

/// The slides that run, and what the refresh pass needs to start the next one.
#[derive(Debug, Default)]
pub(crate) struct WorkspaceSlides {
    /// The windows that were on screen at the last refresh pass.
    shown: HashSet<WindowId>,
    /// One for an output, at the most.
    running: Vec<Slide>,
}

impl WorkspaceSlides {
    /// When the first of the running slides is over: `deadlines` then draws its output at
    /// rest (174).
    pub(crate) fn deadline(&self) -> Option<Duration> {
        self.running.iter().map(|slide| slide.start + LENGTH).min()
    }

    /// The slide of `output` at `now`, while one runs.
    pub(crate) fn on(&self, output: OutputId, now: Duration) -> Option<&Slide> {
        self.running(now).find(|slide| slide.output == output)
    }

    /// The slides that run at `now`.
    fn running(&self, now: Duration) -> impl Iterator<Item = &Slide> {
        self.running.iter().filter(move |slide| !slide.is_over(now))
    }
}

/// The switches of one refresh pass as one for an output: from the workspace it showed at
/// the pass before to the one it shows now, the whole way the switches went. An output
/// that's back where it was has none.
fn whole_switches(switches: Vec<OutputSwitch>) -> Vec<OutputSwitch> {
    let mut whole: Vec<OutputSwitch> = Vec::new();
    for switch in switches {
        match whole.iter_mut().find(|first| first.output == switch.output) {
            Some(first) => {
                first.to = switch.to;
                first.cells.0 += switch.cells.0;
                first.cells.1 += switch.cells.1;
            }
            None => whole.push(switch),
        }
    }
    whole.retain(|switch| switch.from != switch.to);
    whole
}

impl Caminito {
    /// Starts the slide of each output that switched its workspace since the last refresh
    /// pass. One that still runs on the output is retargeted: the new one goes on from
    /// where the windows are (235). With `workspace-slide` off, or one of the minimise
    /// animation's three keys against it, the switch is a cut, which ends a running slide
    /// too; a change of those keys alone lets it finish. The session lock ends every slide
    /// at once and makes each switch under it a cut, and a slide ends with its output. So
    /// does the Overview on its output (241): it opens at once, with the slide cut to its
    /// end, and a switch under it is a cut until its bar has the workspaces (244).
    pub(crate) fn refresh_workspace_slides(&mut self) {
        let now = self.now();
        let switches = self.policy.take_output_switches();
        let locked = self.lock.is_locked();
        let overview = self.policy.overview().map(|open| open.output);
        let outputs = &self.outputs;
        self.workspace_slides.running.retain(|slide| {
            !locked
                && slide.under_overview == (Some(slide.output) == overview)
                && outputs
                    .iter()
                    .any(|output| output_id(output) == slide.output)
        });
        let policy = &self.policy;
        let shown: HashSet<WindowId> = policy
            .windows()
            .map(|mapped| mapped.id())
            .filter(|&id| policy.is_showing(id))
            .collect();
        let was_shown = std::mem::replace(&mut self.workspace_slides.shown, shown);
        let shown = &self.workspace_slides.shown;

        let mut started = Vec::new();
        for switch in whole_switches(switches) {
            let running = &mut self.workspace_slides.running;
            let retargeted = running
                .iter()
                .position(|slide| slide.output == switch.output)
                .map(|index| running.swap_remove(index))
                // One that's over, or that a switch with no slide took the output from,
                // has nothing to go on from.
                .filter(|slide| !slide.is_over(now) && slide.to == switch.from);
            let mut outputs = self.outputs.iter();
            let Some(output) = outputs.find(|output| output_id(output) == switch.output) else {
                continue;
            };
            if !self.settings.workspace_slide || locked {
                continue;
            }
            let size = output_geometry(output).size;
            let mut slide = match retargeted {
                Some(running) => Slide::retargeted(running, &switch, size, now),
                None => Slide::new(&switch, size, now),
            };
            slide.under_overview = Some(switch.output) == overview;
            // Those drawn there. A dialog dragged to another output leaves and arrives
            // with its root (223), and isn't the switched output's to slide.
            let on_output = |id: &&WindowId| policy.main_output_of(**id) == Some(switch.output);
            let leaving = was_shown
                .difference(shown)
                .filter(on_output)
                // One that was minimised or shows the desktop since isn't on its way to
                // another workspace.
                .filter(|id| {
                    policy.workspace_of(**id) == Some(switch.from) && !policy.is_minimized(**id)
                })
                .copied()
                .collect();
            // One that maps or is restored on another workspace in the same pass isn't the
            // new workspace's.
            let arriving = shown
                .difference(&was_shown)
                .filter(on_output)
                .filter(|id| policy.workspace_of(**id) == Some(switch.to))
                .copied();
            running.push(slide.with_windows(leaving, arriving.collect()));
            started.push(output.clone());
        }
        if started.is_empty() {
            return;
        }
        for output in started {
            self.queue_redraw(&output);
        }
        // The Overview lays out the workspaces a slide leaves, which it's drawn with from
        // this pass on.
        self.refresh_overview();
    }

    /// Ends the slides that are over at `now`, and draws their outputs at rest, for
    /// `deadlines`. While one runs, its output is drawn again at each of its frames
    /// (`State::redraw_queued_outputs`), so there's no step to wait for.
    pub(crate) fn workspace_slide_elapse(&mut self, now: Duration) {
        let running = &mut self.workspace_slides.running;
        let over: Vec<OutputId> = running
            .iter()
            .filter(|slide| slide.is_over(now))
            .map(|slide| slide.output)
            .collect();
        if over.is_empty() {
            return;
        }
        running.retain(|slide| !slide.is_over(now));
        let outputs = self.outputs.clone();
        for output in outputs {
            if over.contains(&output_id(&output)) {
                self.queue_redraw(&output);
            }
        }
    }

    /// Each window `slide` moves at `now`, in the order of their ids, with where its frame
    /// is drawn, globally, for the debug dump: one time for each place the slide draws it
    /// at, from the left and from the top (239).
    pub(crate) fn sliding_windows(
        &self,
        slide: &Slide,
        now: Duration,
    ) -> Vec<(WindowId, Rectangle<i32, Logical>)> {
        let policy = &self.policy;
        let mut windows: Vec<_> = slide
            .windows()
            .into_iter()
            .filter_map(|id| Some((id, policy.frame_rect(id)?)))
            .flat_map(|(id, frame)| {
                let shifts = slide.shifts(id, policy.is_showing(id), now);
                let drawn = move |shift| (id, Rectangle::new(frame.loc + shift, frame.size));
                shifts.into_iter().map(drawn)
            })
            .collect();
        windows.sort_by_key(|(id, drawn)| (*id, drawn.loc.x, drawn.loc.y));
        windows
    }

    /// The windows on `slide`'s output that it leaves in their places, in the order of
    /// their ids, for the debug dump: those that are the same on both workspaces, as a
    /// sticky window, one a move carries and one being dragged are (234).
    pub(crate) fn still_windows(&self, slide: &Slide) -> Vec<WindowId> {
        let policy = &self.policy;
        let mut windows: Vec<WindowId> = policy
            .windows()
            .map(|mapped| mapped.id())
            .filter(|&id| policy.is_showing(id) && policy.main_output_of(id) == Some(slide.output))
            .filter(|id| !slide.arriving.contains(id))
            .collect();
        windows.sort();
        windows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The output, which each slide here is on.
    const SIZE: (i32, i32) = (800, 600);

    /// A slide over `cells` of the grid from workspace 0 to workspace 1, with window 1
    /// leaving and window 2 arriving.
    fn slide(cells: (i32, i32)) -> Slide {
        Slide::new(&switch(1, 0, 1, cells), SIZE.into(), after(0))
            .with_windows(windows([1]), windows([2]))
    }

    fn windows<const N: usize>(ids: [u64; N]) -> HashSet<WindowId> {
        ids.into_iter().map(WindowId).collect()
    }

    fn after(millis: u64) -> Duration {
        Duration::from_secs(10) + Duration::from_millis(millis)
    }

    #[test]
    fn the_windows_start_a_whole_output_away_and_slow_down_into_their_places() {
        let slide = slide((1, 0));

        assert_eq!(slide.offset(after(0)), Point::from((800, 0)));
        // A cubic ease-out: half the time leaves an eighth of the way.
        assert_eq!(slide.offset(after(125)), Point::from((100, 0)));
        assert_eq!(slide.offset(after(250)), Point::from((0, 0)));
        assert_eq!(slide.offset(after(900)), Point::from((0, 0)));
    }

    #[test]
    fn a_jump_is_one_straight_line() {
        let slide = slide((-2, 1));

        assert_eq!(slide.offset(after(125)), Point::from((-200, 75)));
    }

    #[test]
    fn a_leaving_window_is_the_whole_distance_behind_an_arriving_one() {
        let slide = slide((1, 0));
        let now = after(125);

        assert_eq!(slide.shifts(WindowId(2), true, now), [(100, 0).into()]);
        assert_eq!(slide.shifts(WindowId(1), false, now), [(-700, 0).into()]);
    }

    #[test]
    fn a_window_that_isnt_the_slides_is_left_where_it_is() {
        let slide = slide((1, 0));
        let now = after(125);

        // One that showed before and shows after, as a sticky window does.
        assert_eq!(slide.shifts(WindowId(3), true, now), []);
        // One that left and shows again, and one that came and went.
        assert_eq!(slide.shifts(WindowId(1), true, now), []);
        assert_eq!(slide.shifts(WindowId(2), false, now), []);
    }

    #[test]
    fn a_workspace_is_drawn_where_its_windows_are() {
        let slide = slide((1, 0));
        let now = after(125);

        assert_eq!(slide.workspace_shifts(1, now), [(100, 0).into()]);
        assert_eq!(slide.workspace_shifts(0, now), [(-700, 0).into()]);
        assert_eq!(slide.workspace_shifts(2, now), []);
        assert_eq!(slide.left_workspaces(), [0]);
    }

    #[test]
    fn a_workspace_a_wrap_comes_back_to_is_drawn_where_it_leaves_and_arrives() {
        let now = after(125);

        let round = wrapped_round(now);

        assert_eq!(
            round.workspace_shifts(0, now),
            [(-700, 0).into(), (900, 0).into()]
        );
        assert_eq!(round.left_workspaces(), [1, 0]);
    }

    #[test]
    fn a_slide_is_over_after_a_quarter_second() {
        let slides = WorkspaceSlides {
            shown: HashSet::new(),
            running: vec![slide((1, 0))],
        };

        assert_eq!(slides.deadline(), Some(after(250)));
        assert!(slides.on(OutputId(1), after(249)).is_some());
        assert!(slides.on(OutputId(1), after(250)).is_none());
        assert!(slides.on(OutputId(2), after(0)).is_none());
    }

    fn switch(output: u64, from: usize, to: usize, cells: (i32, i32)) -> OutputSwitch {
        OutputSwitch {
            output: OutputId(output),
            from,
            to,
            cells,
        }
    }

    #[test]
    fn a_retargeted_slide_goes_on_from_where_the_windows_are() {
        let now = after(125);
        let first = slide((1, 0));
        let before = [
            first.shifts(WindowId(1), false, now),
            first.shifts(WindowId(2), true, now),
        ];

        // On to workspace 2, on the right, where window 3 is.
        let second = Slide::retargeted(first, &switch(1, 1, 2, (1, 0)), SIZE.into(), now)
            .with_windows(windows([2]), windows([3]));

        let after_it = [
            second.shifts(WindowId(1), false, now),
            second.shifts(WindowId(2), false, now),
        ];
        assert_eq!(after_it, before);
        assert_eq!(after_it, [[(-700, 0).into()], [(100, 0).into()]]);
        // The new workspace is a whole output behind the one that was arriving.
        assert_eq!(second.shifts(WindowId(3), true, now), [(900, 0).into()]);
        // And it takes its own quarter second, to each workspace's place in the grid.
        let end = now + LENGTH;
        assert!(!second.is_over(after(250)) && second.is_over(end));
        assert_eq!(second.shifts(WindowId(1), false, end), [(-1600, 0).into()]);
        assert_eq!(second.shifts(WindowId(2), false, end), [(-800, 0).into()]);
        assert_eq!(second.shifts(WindowId(3), true, end), [(0, 0).into()]);
    }

    #[test]
    fn a_slide_retargeted_to_the_workspace_it_left_returns_from_where_it_is() {
        let now = after(125);
        let out = slide((1, 0));

        let back = Slide::retargeted(out, &switch(1, 1, 0, (-1, 0)), SIZE.into(), now)
            .with_windows(windows([2]), windows([1]));

        // The first workspace is the one that arrives, and isn't one that leaves too.
        assert_eq!(back.left.len(), 1);
        assert_eq!(back.shifts(WindowId(1), true, now), [(-700, 0).into()]);
        assert_eq!(back.shifts(WindowId(2), false, now), [(100, 0).into()]);
        let end = now + LENGTH;
        assert_eq!(back.shifts(WindowId(1), true, end), [(0, 0).into()]);
        assert_eq!(back.shifts(WindowId(2), false, end), [(800, 0).into()]);
    }

    /// Right from workspace 0, and right again part-way from the last of two workspaces,
    /// which wraps to the first: one cell the way of the key.
    fn wrapped_round(now: Duration) -> Slide {
        Slide::retargeted(slide((1, 0)), &switch(1, 1, 0, (1, 0)), SIZE.into(), now)
            .with_windows(windows([2]), windows([1]))
    }

    #[test]
    fn a_wrap_back_to_the_workspace_it_left_draws_that_one_where_it_leaves_and_arrives() {
        let now = after(125);

        let round = wrapped_round(now);

        assert_eq!(round.left.len(), 2);
        assert_eq!(round.shifts(WindowId(2), false, now), [(100, 0).into()]);
        // It's 700 to the left, on its way out as it was, and comes in from the right.
        assert_eq!(
            round.shifts(WindowId(1), true, now),
            [(-700, 0).into(), (900, 0).into()]
        );
        let end = now + LENGTH;
        assert_eq!(
            round.shifts(WindowId(1), true, end),
            [(-1600, 0).into(), (0, 0).into()]
        );
        // The slide's windows are each there one time.
        assert_eq!(round.windows(), windows([1, 2]));
    }

    #[test]
    fn each_wrap_more_keeps_the_places_a_workspace_left() {
        let now = after(125);
        let later = after(200);

        // Right a third time, to the second workspace again, and a fourth.
        let third = Slide::retargeted(
            wrapped_round(now),
            &switch(1, 0, 1, (1, 0)),
            SIZE.into(),
            later,
        )
        .with_windows(windows([1]), windows([2]));
        let before = third.shifts(WindowId(1), false, later);
        let fourth = Slide::retargeted(third, &switch(1, 1, 0, (1, 0)), SIZE.into(), later)
            .with_windows(windows([2]), windows([1]));

        // The first workspace has left two times, and arrives.
        let places = |slide: &Slide, workspace| {
            let left = slide.left.iter().filter(|left| left.workspace == workspace);
            left.map(|left| left.place.x).collect::<Vec<_>>()
        };
        assert_eq!(places(&fourth, 0), [-1600, -3200]);
        assert_eq!(places(&fourth, 1), [-800, -2400]);
        let drawn = fourth.shifts(WindowId(1), true, later);
        assert_eq!(drawn.len(), 3);
        assert_eq!(drawn[..2], before[..]);
        assert_eq!(drawn[2], drawn[0] + Point::from((1600, 0)));
    }

    #[test]
    fn a_wrap_and_a_switch_back_leave_the_workspace_at_the_place_it_didnt_return_to() {
        let now = after(125);

        // Left from the first workspace, which the wrap came back to: to the second, where
        // the slide has it on the left.
        let back = Slide::retargeted(
            wrapped_round(now),
            &switch(1, 0, 1, (-1, 0)),
            SIZE.into(),
            now,
        )
        .with_windows(windows([1]), windows([2]));

        // The first workspace is on each side of the second, and the second arrives alone.
        let places: Vec<_> = back.left.iter().map(|l| (l.workspace, l.place.x)).collect();
        assert_eq!(places, [(0, 800), (0, -800)]);
        assert_eq!(back.shifts(WindowId(2), true, now), [(100, 0).into()]);
        assert_eq!(
            back.shifts(WindowId(1), false, now),
            [(900, 0).into(), (-700, 0).into()]
        );
    }

    #[test]
    fn the_switches_of_one_pass_are_one_slide_the_whole_way() {
        let switches = vec![
            switch(1, 0, 1, (1, 0)),
            switch(2, 0, 2, (2, 0)),
            switch(1, 1, 3, (0, 1)),
        ];

        assert_eq!(
            whole_switches(switches),
            [switch(1, 0, 3, (1, 1)), switch(2, 0, 2, (2, 0))]
        );
    }

    #[test]
    fn an_output_that_is_back_where_it_was_has_no_slide() {
        let switches = vec![switch(1, 0, 1, (1, 0)), switch(1, 1, 0, (-1, 0))];

        assert_eq!(whole_switches(switches), []);
    }
}
