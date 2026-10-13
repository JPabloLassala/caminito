//! The Workspace slide (233, ADR 0033): on a change of an output's active workspace, the
//! old workspace's windows leave and the new one's arrive, along the line between the two
//! in the Workspace grid, for 250 ms of Caminito's clock. It's Plasma's Slide effect.
//!
//! Policy has switched from the start, as it has a window minimised from the start of the
//! minimise animation (172): the focus, input and what clients are told follow it at once,
//! and only the drawing takes its time. The refresh pass takes the switches policy noted
//! and the windows that were on screen at the pass before, and the scene draws the windows
//! that left and those that came away from their own places until the slide's over. A
//! window that shows before and after, as a sticky one does, is none of the slide's.

use std::collections::HashSet;
use std::time::Duration;

use caminito_policy::{OutputId, OutputSwitch, PolicyWindow, WindowId};
use smithay::utils::{Logical, Point, Rectangle};

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

/// One output's windows on their way between two workspaces.
#[derive(Debug)]
pub(crate) struct Slide {
    /// The output that switched.
    pub(crate) output: OutputId,
    /// The workspace whose windows leave.
    pub(crate) from: usize,
    /// The one whose windows arrive.
    pub(crate) to: usize,
    /// From the old workspace's place to the new one's: the output's width for each column
    /// of the Workspace grid between them and its height for each row, with no gap.
    distance: Point<i32, Logical>,
    /// When the output switched, on Caminito's clock.
    start: Duration,
    /// The old workspace's windows that were on screen.
    leaving: HashSet<WindowId>,
    /// The new workspace's windows, which weren't.
    arriving: HashSet<WindowId>,
}

impl Slide {
    fn is_over(&self, now: Duration) -> bool {
        now.saturating_sub(self.start) >= LENGTH
    }

    /// How far the arriving windows are from their own places at `now`. The leaving ones
    /// are that far from where the old workspace ends up, the whole distance away.
    pub(crate) fn offset(&self, now: Duration) -> Point<i32, Logical> {
        let left = way_left(now.saturating_sub(self.start));
        let at = |distance: i32| (f64::from(distance) * left).round() as i32;
        Point::from((at(self.distance.x), at(self.distance.y)))
    }

    /// How far from its own place the slide draws `window` at `now`: one that arrives, while
    /// it shows, and one that leaves, while it doesn't. `None` for a window that's none of
    /// the slide's, which is drawn as policy has it.
    pub(crate) fn shift(
        &self,
        window: WindowId,
        showing: bool,
        now: Duration,
    ) -> Option<Point<i32, Logical>> {
        if showing {
            self.arriving.contains(&window).then(|| self.offset(now))
        } else {
            self.leaving
                .contains(&window)
                .then(|| self.offset(now) - self.distance)
        }
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
    /// pass, in place of the one that ran on it, if any: the first ends at once (a switch
    /// during a slide retargets it in ticket 235). With `workspace-slide` off, or one of the
    /// minimise animation's three keys against it, the switch is a cut.
    pub(crate) fn refresh_workspace_slides(&mut self) {
        let now = self.now();
        let switches = self.policy.take_output_switches();
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
            running.retain(|slide| slide.output != switch.output);
            let mut outputs = self.outputs.iter();
            let Some(output) = outputs.find(|output| output_id(output) == switch.output) else {
                continue;
            };
            if !self.settings.workspace_slide {
                continue;
            }
            let size = output_geometry(output).size;
            let on_output = |id: &&WindowId| policy.output_showing(**id) == Some(switch.output);
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
            running.push(Slide {
                output: switch.output,
                from: switch.from,
                to: switch.to,
                distance: Point::from((switch.cells.0 * size.w, switch.cells.1 * size.h)),
                start: now,
                leaving,
                arriving: arriving.collect(),
            });
            started.push(output.clone());
        }
        for output in started {
            self.queue_redraw(&output);
        }
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
    /// is drawn, globally, for the debug dump.
    pub(crate) fn sliding_windows(
        &self,
        slide: &Slide,
        now: Duration,
    ) -> Vec<(WindowId, Rectangle<i32, Logical>)> {
        let policy = &self.policy;
        let mut windows: Vec<_> = slide
            .leaving
            .iter()
            .chain(&slide.arriving)
            .filter_map(|&id| {
                let shift = slide.shift(id, policy.is_showing(id), now)?;
                let frame = policy.frame_rect(id)?;
                Some((id, Rectangle::new(frame.loc + shift, frame.size)))
            })
            .collect();
        windows.sort_by_key(|(id, _)| *id);
        windows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slide(distance: (i32, i32)) -> Slide {
        Slide {
            output: OutputId(1),
            from: 0,
            to: 1,
            distance: distance.into(),
            start: Duration::from_secs(10),
            leaving: HashSet::from([WindowId(1)]),
            arriving: HashSet::from([WindowId(2)]),
        }
    }

    fn after(millis: u64) -> Duration {
        Duration::from_secs(10) + Duration::from_millis(millis)
    }

    #[test]
    fn the_windows_start_a_whole_output_away_and_slow_down_into_their_places() {
        let slide = slide((800, 0));

        assert_eq!(slide.offset(after(0)), Point::from((800, 0)));
        // A cubic ease-out: half the time leaves an eighth of the way.
        assert_eq!(slide.offset(after(125)), Point::from((100, 0)));
        assert_eq!(slide.offset(after(250)), Point::from((0, 0)));
        assert_eq!(slide.offset(after(900)), Point::from((0, 0)));
    }

    #[test]
    fn a_jump_is_one_straight_line() {
        let slide = slide((-1600, 600));

        assert_eq!(slide.offset(after(125)), Point::from((-200, 75)));
    }

    #[test]
    fn a_leaving_window_is_the_whole_distance_behind_an_arriving_one() {
        let slide = slide((800, 0));
        let now = after(125);

        assert_eq!(slide.shift(WindowId(2), true, now), Some((100, 0).into()));
        assert_eq!(slide.shift(WindowId(1), false, now), Some((-700, 0).into()));
    }

    #[test]
    fn a_window_that_isnt_the_slides_is_left_where_it_is() {
        let slide = slide((800, 0));
        let now = after(125);

        // One that showed before and shows after, as a sticky window does.
        assert_eq!(slide.shift(WindowId(3), true, now), None);
        // One that left and shows again, and one that came and went.
        assert_eq!(slide.shift(WindowId(1), true, now), None);
        assert_eq!(slide.shift(WindowId(2), false, now), None);
    }

    #[test]
    fn a_slide_is_over_after_a_quarter_second() {
        let slides = WorkspaceSlides {
            shown: HashSet::new(),
            running: vec![slide((800, 0))],
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
