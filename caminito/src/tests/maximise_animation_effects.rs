//! The Maximise animation beside the other effects (CMN-177). During a Workspace slide, a
//! window that's on its way to its new frame on the workspace leaving or the one arriving
//! moves with its workspace as well. None starts while the Overview is open or during its
//! transition, and one that runs ends when it opens. The session lock ends every one at once
//! and none starts under it, and an output that goes ends those on it, as with the Opening
//! and the Closing animations (254). Each runs with one output and with two, under
//! Per-output workspaces and under Spanning workspaces (ADR 0032), read from the debug dump.
//!
//! The windows are xdg toplevels alone: what's here is the effects', the same for both
//! kinds, and `maximise_animation.rs` has each kind maximised.

use std::time::Duration;

use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{ClientId, ClientWindow, Fixture, Kind};
use super::maximise_animation::{DONE_PART_WAY, between, maximizing, near};
use super::open_close_animation::Settings;
use super::open_close_animation_effects::overview_transition_runs;
use super::overview;
use super::workspace_slide::{LOW_RIGHT, OUTPUT, WIDE, put_at, slides};
use crate::maximize_animation::LENGTH;
use crate::output::output_geometry;
use crate::settings::Value as Setting;
use crate::settings::translate::CAMINITO;

/// A window clear of the edges of the left output wherever it's placed.
const SMALL: (i32, i32) = (300, 200);

/// Where a `SMALL` window is on the left output, away from its corner.
const AWAY: (i32, i32) = (150, 120);

/// The time between two steps of a test.
const STEP: Duration = Duration::from_millis(50);

/// How far the box has gone one `STEP` after its start: `1 - (1 - 50/250)³`.
const DONE_ONE_STEP: f64 = 0.488;

/// The two workspace modes, as `workspaces-per-output` has them: Per-output workspaces,
/// and Spanning workspaces.
const MODES: [bool; 2] = [true, false];

/// Each workspace mode with one output and with two.
const DESKS: [(bool, usize); 4] = [(true, 1), (false, 1), (true, 2), (false, 2)];

/// `outputs` outputs, the left one of `OUTPUT` and the primary, and one of `WIDE` on its
/// right if there are two, with `workspaces-per-output` as `per_output` says,
/// `maximize-animation` and `workspace-slide` on, `settings` and a Wayland client. The clock
/// is stopped.
fn desk(
    per_output: bool,
    outputs: usize,
    mut settings: Settings,
) -> (Fixture, ClientId, Output, Option<Output>) {
    for key in ["maximize-animation", "workspace-slide"] {
        settings.push((CAMINITO, key.into(), Setting::Bool(true)));
    }
    let mode = Setting::Bool(per_output);
    settings.push((CAMINITO, "workspaces-per-output".into(), mode));
    let mut f = Fixture::with_settings(settings);
    let left = f.add_output(OUTPUT, 1.0);
    let right = (outputs == 2).then(|| f.add_output(WIDE, 1.0));
    f.choose_primary_output(&left);
    if let Some(right) = &right {
        assert_eq!(output_geometry(right).loc, (OUTPUT.0, 0).into());
    }
    let id = f.connect(Kind::Xdg);
    f.stop_clock();
    (f, id, left, right)
}

/// Maximises or unmaximises `window` both ways, as policy's own maximise, which a client's
/// request, a binding or a button ends in.
fn maximise(f: &mut Fixture, window: ClientWindow, maximized: bool) {
    let pid = f.pid(window);
    f.state.caminito.policy.set_maximized(pid, maximized);
    f.dispatch();
}

/// The frame policy asks `window` to be at.
fn asked_frame(f: &Fixture, window: ClientWindow) -> Rectangle<i32, Logical> {
    let policy = &f.state.caminito.policy;
    policy.asked_frame_rect(f.pid(window)).unwrap()
}

/// `window`'s box, at the one place it's drawn.
#[track_caller]
fn the_box(f: &Fixture, window: ClientWindow) -> Rectangle<i32, Logical> {
    let pid = f.pid(window).0;
    let maximizing = maximizing(f);
    let its_own: Vec<_> = maximizing.iter().filter(|(id, _, _)| *id == pid).collect();
    match its_own[..] {
        [(_, rect, _)] => *rect,
        _ => panic!("{window:?} is on its way at one place: {maximizing:?}"),
    }
}

/// The windows on their way, by policy's id, in the order of their ids.
fn on_their_way(f: &Fixture) -> Vec<u64> {
    let mut ids: Vec<u64> = maximizing(f).iter().map(|(id, _, _)| *id).collect();
    ids.sort_unstable();
    ids
}

/// Checks that `drawn` is `done` of the way from `from` to `to`, moved by `shift`.
#[track_caller]
fn assert_on_its_way_and_moved(
    drawn: Rectangle<i32, Logical>,
    (from, to): (Rectangle<i32, Logical>, Rectangle<i32, Logical>),
    done: f64,
    shift: Point<i32, Logical>,
) {
    let place = between(from, to, done);
    let expected = Rectangle::new(place.loc + shift, place.size);
    assert!(
        near(drawn, expected),
        "{done} of the way, moved by {shift:?} with the slide: {drawn:?}, not {expected:?}"
    );
}

#[test]
fn during_a_slide_a_window_on_its_way_on_the_leaving_workspace_and_one_on_the_arriving_one_move_as_they_grow()
 {
    for (per_output, outputs) in DESKS {
        let (mut f, id, left, _) = desk(per_output, outputs, Vec::new());
        let leaving = put_at(&mut f, id, AWAY, 0, SMALL);
        let arriving = put_at(&mut f, id, AWAY, 1, SMALL);
        let leaving_from = f.visible_frame(leaving);
        let arriving_from = f.visible_frame(arriving);
        maximise(&mut f, leaving, true);
        let leaving_to = asked_frame(&f, leaving);
        f.advance_clock(STEP);

        let caminito = &mut f.state.caminito;
        assert!(caminito.switch_output_workspace(&left.name(), 1));
        f.dispatch();
        maximise(&mut f, arriving, true);
        let arriving_to = asked_frame(&f, arriving);
        f.advance_clock(STEP);

        let slide = slides(&f)
            .into_iter()
            .find(|slide| slide.output == left.name());
        let offset = slide.expect("the left output slides").offset;
        // The workspace that left ends up the left output's width to the left.
        let away = Point::from((OUTPUT.0, 0));
        let leaving_drawn = the_box(&f, leaving);
        let leaving_way = (leaving_from, leaving_to);
        assert_on_its_way_and_moved(leaving_drawn, leaving_way, DONE_PART_WAY, offset - away);
        let arriving_drawn = the_box(&f, arriving);
        let arriving_way = (arriving_from, arriving_to);
        assert_on_its_way_and_moved(arriving_drawn, arriving_way, DONE_ONE_STEP, offset);

        f.advance_clock(LENGTH);
        assert!(slides(&f).is_empty(), "the slide is over");
        assert!(maximizing(&f).is_empty(), "over: {:?}", maximizing(&f));
    }
}

#[test]
fn none_runs_while_the_overview_is_open_and_a_window_maximised_in_it_comes_out_at_its_new_place() {
    for (per_output, outputs) in DESKS {
        let (mut f, id, _, _) = desk(per_output, outputs, Vec::new());
        let first = put_at(&mut f, id, AWAY, 0, SMALL);
        let second = put_at(&mut f, id, AWAY, 0, SMALL);
        maximise(&mut f, first, true);
        f.advance_clock(STEP);
        assert_eq!(on_their_way(&f), [f.pid(first).0]);

        overview::open(&mut f);
        assert!(
            maximizing(&f).is_empty(),
            "the Overview ended it: {:?}",
            maximizing(&f)
        );
        maximise(&mut f, second, true);
        assert!(maximizing(&f).is_empty(), "none starts in it");
        f.advance_clock(STEP);

        overview::press_shortcut(&mut f);
        assert!(!overview::is_open(&f), "the shortcut closes it");
        assert!(
            maximizing(&f).is_empty(),
            "it comes out at its new place: {:?}",
            maximizing(&f)
        );
        let pid = f.pid(second);
        assert!(f.state.caminito.policy.is_maximized(pid));
    }
}

#[test]
fn one_that_runs_ends_as_the_overviews_transition_starts_and_none_starts_during_it_either_way() {
    for (per_output, outputs) in DESKS {
        let animation = (CAMINITO, "overview-animation".into(), Setting::Bool(true));
        let (mut f, id, _, _) = desk(per_output, outputs, vec![animation]);
        let running = put_at(&mut f, id, AWAY, 0, SMALL);
        let window = put_at(&mut f, id, AWAY, 0, SMALL);
        maximise(&mut f, running, true);
        f.advance_clock(STEP);
        assert_eq!(on_their_way(&f), [f.pid(running).0]);

        overview::open(&mut f);
        assert!(overview_transition_runs(&f), "it opens with its transition");
        assert!(
            maximizing(&f).is_empty(),
            "the Overview ended it: {:?}",
            maximizing(&f)
        );
        maximise(&mut f, window, true);
        f.advance_clock(STEP);
        assert!(maximizing(&f).is_empty(), "{:?}", maximizing(&f));
        f.advance_clock(crate::workspace_slide::LENGTH);
        assert!(!overview_transition_runs(&f), "it's open at rest");

        overview::press_shortcut(&mut f);
        assert!(!overview::is_open(&f), "the shortcut closes it");
        assert!(
            overview_transition_runs(&f),
            "it closes with its transition"
        );
        maximise(&mut f, window, false);
        f.advance_clock(STEP);
        assert!(maximizing(&f).is_empty(), "{:?}", maximizing(&f));
    }
}

#[test]
fn locking_part_way_ends_every_one_at_once_and_none_starts_under_the_lock() {
    for (per_output, outputs) in DESKS {
        let (mut f, id, _, _) = desk(per_output, outputs, Vec::new());
        let first = put_at(&mut f, id, AWAY, 0, SMALL);
        let second = put_at(&mut f, id, AWAY, 0, SMALL);
        let later = put_at(&mut f, id, AWAY, 0, SMALL);
        maximise(&mut f, first, true);
        maximise(&mut f, second, true);
        f.advance_clock(STEP);
        assert_eq!(on_their_way(&f).len(), 2);
        let locker = f.connect(Kind::Xdg);

        let lock = f.client(locker).lock_session();
        f.roundtrip(locker);
        assert_ne!(f.lock_state(), "unlocked");
        assert!(
            maximizing(&f).is_empty(),
            "the lock ended them: {:?}",
            maximizing(&f)
        );

        maximise(&mut f, later, true);
        f.advance_clock(STEP);
        assert!(maximizing(&f).is_empty(), "none starts under it");
        f.client(locker).unlock_and_destroy(lock);
        f.roundtrip(locker);
        assert_eq!(f.lock_state(), "unlocked");
        assert!(
            maximizing(&f).is_empty(),
            "it isn't animated after the lock: {:?}",
            maximizing(&f)
        );
    }
}

#[test]
fn an_output_removed_part_way_ends_the_ones_on_it() {
    for per_output in MODES {
        let (mut f, id, _, right) = desk(per_output, 2, Vec::new());
        let right = right.expect("two outputs");
        let on_left = put_at(&mut f, id, AWAY, 0, SMALL);
        let on_right = put_at(&mut f, id, LOW_RIGHT, 0, SMALL);
        maximise(&mut f, on_left, true);
        maximise(&mut f, on_right, true);
        f.advance_clock(STEP);
        let (left_id, right_id) = (f.pid(on_left).0, f.pid(on_right).0);
        assert_eq!(on_their_way(&f), [left_id, right_id]);

        f.unplug(&right);
        assert_eq!(on_their_way(&f), [left_id], "the right one's ended with it");
        f.advance_clock(LENGTH);
        assert!(maximizing(&f).is_empty(), "over: {:?}", maximizing(&f));
    }
}
