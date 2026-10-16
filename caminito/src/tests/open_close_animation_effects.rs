//! The Opening and the Closing animations beside the other effects (254). During a
//! Workspace slide, a window that opens on the workspace arriving and a closing window's
//! picture of the one leaving move with their workspace as they zoom and fade. Neither
//! animation runs while the Overview is open or during its transition, and one that runs
//! when it opens is dropped. The session lock ends both at once and none starts under it,
//! and an output that goes drops the pictures on it, as the slide has them (235). Each
//! runs with one output and with two, under Per-output workspaces and under Spanning
//! workspaces (ADR 0032), read from the debug dump.
//!
//! The windows are xdg toplevels alone: what's here is the animations' and the effects',
//! the same for both kinds, and `open_close_animation.rs` has each kind open and close.

use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{ClientId, ClientWindow, Fixture, Kind, WindowSpec};
use super::open_close_animation::{
    Close, DrawnPicture, PART_WAY, Settings, close, closing, map_opened, opening,
};
use super::overview;
use super::workspace_slide::{
    LOW_RIGHT, OUTPUT, Sliding, WIDE, WINDOW, clear, pixel_on, put_at, slides,
};
use crate::open_close_animation::LENGTH;
use crate::output::output_geometry;
use crate::settings::Value as Setting;
use crate::settings::translate::CAMINITO;

/// A window clear of the edges of the left output wherever it's placed.
const SMALL: (i32, i32) = (300, 200);

/// Where a `SMALL` window is on the left output, near its right edge.
const RIGHT_OF_LEFT: (i32, i32) = (450, 300);

/// The two workspace modes, as `workspaces-per-output` has them: Per-output workspaces,
/// and Spanning workspaces.
const MODES: [bool; 2] = [true, false];

/// Each workspace mode with one output and with two.
const DESKS: [(bool, usize); 4] = [(true, 1), (false, 1), (true, 2), (false, 2)];

/// `outputs` outputs, the left one of `OUTPUT` and the primary, and one of `WIDE` on its
/// right if there are two, with `workspaces-per-output` as `per_output` says,
/// `open-close-animation` and `workspace-slide` on unless `settings` has them, `settings`
/// and a Wayland client. The clock is stopped.
fn desk(
    per_output: bool,
    outputs: usize,
    mut settings: Settings,
) -> (Fixture, ClientId, Output, Option<Output>) {
    for key in ["open-close-animation", "workspace-slide"] {
        if !settings.iter().any(|(_, set, _)| set == key) {
            settings.push((CAMINITO, key.into(), Setting::Bool(true)));
        }
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

/// Maps a `SMALL` window on the left output and lets its opening finish.
fn map_left(f: &mut Fixture, id: ClientId) -> ClientWindow {
    let window = map_opened(f, id, WindowSpec::plain(SMALL));
    let frame = f.visible_frame(window);
    assert!(Rectangle::from_size(OUTPUT.into()).contains_rect(frame));
    window
}

/// The slide that runs on `output`.
#[track_caller]
fn slide_on(f: &Fixture, output: &Output) -> Sliding {
    let slides = slides(f);
    let name = output.name();
    let slide = slides.into_iter().find(|slide| slide.output == name);
    slide.unwrap_or_else(|| panic!("{name} slides"))
}

/// Switches `output` to `workspace`, as the debug interface does: with Spanning workspaces
/// every output switches.
#[track_caller]
fn switch_output(f: &mut Fixture, output: &Output, workspace: usize) {
    let caminito = &mut f.state.caminito;
    assert!(caminito.switch_output_workspace(&output.name(), workspace));
    f.dispatch();
}

/// The middle of `rect`.
fn middle(rect: Rectangle<i32, Logical>) -> Point<i32, Logical> {
    rect.loc + Point::from((rect.size.w / 2, rect.size.h / 2))
}

/// Twice the middle of `rect`, which is then whole.
fn twice_middle(rect: Rectangle<i32, Logical>) -> Point<i32, Logical> {
    rect.loc + rect.loc + rect.size.to_point()
}

/// Checks that `drawn` is smaller than `frame` and has its middle `shift` from `frame`'s.
#[track_caller]
fn assert_scaled_and_moved(
    drawn: Rectangle<i32, Logical>,
    frame: Rectangle<i32, Logical>,
    shift: Point<i32, Logical>,
) {
    assert!(
        drawn.size.w < frame.size.w && drawn.size.h < frame.size.h,
        "scaled part-way: {drawn:?} of {frame:?}"
    );
    let four_fifths = |n: i32| (f64::from(n) * 0.8).round() as i32;
    assert!(drawn.size.w > four_fifths(frame.size.w), "{drawn:?}");
    let (centre, moved) = (twice_middle(drawn), twice_middle(frame) + shift + shift);
    assert!(
        (centre.x - moved.x).abs() <= 2 && (centre.y - moved.y).abs() <= 2,
        "moved by {shift:?} with the slide: {drawn:?} of {frame:?}"
    );
}

/// `window`'s closing picture, the one there is.
#[track_caller]
fn the_picture(f: &Fixture, id: u64) -> DrawnPicture {
    let mut pictures = closing(f);
    assert_eq!(pictures.len(), 1, "one picture: {pictures:?}");
    let picture = pictures.remove(0);
    assert_eq!(picture.id, id);
    picture
}

/// Where `window`'s opening draws its frame, at the one place it's drawn.
#[track_caller]
fn the_opening(f: &Fixture, window: ClientWindow) -> Rectangle<i32, Logical> {
    let pid = f.pid(window).0;
    let opening = opening(f);
    match opening[..] {
        [(id, rect, _)] if id == pid => rect,
        _ => panic!("{window:?} alone opens: {opening:?}"),
    }
}

#[test]
fn during_a_slide_a_window_opening_on_the_arriving_workspace_and_one_closing_on_the_leaving_one_move_as_they_zoom()
 {
    for (per_output, outputs) in DESKS {
        let (mut f, id, left, _) = desk(per_output, outputs, Vec::new());
        let old = map_left(&mut f, id);
        let old_frame = f.visible_frame(old);
        let old_id = f.pid(old).0;
        switch_output(&mut f, &left, 1);
        f.advance_clock(PART_WAY);

        close(&mut f, old, Close::Destroy);
        let new = f.map(id, WindowSpec::plain(SMALL));
        assert_eq!(f.state.caminito.policy.workspace_of(f.pid(new)), Some(1));
        let new_frame = f.visible_frame(new);
        f.advance_clock(PART_WAY);

        let slide = slide_on(&f, &left);
        // The workspace that left ends up the left output's width to the left.
        let away = Point::from((OUTPUT.0, 0));
        let picture = the_picture(&f, old_id);
        assert_scaled_and_moved(picture.rect, old_frame, slide.offset - away);
        assert!(picture.alpha > 0.0 && picture.alpha < 1.0);
        assert_scaled_and_moved(the_opening(&f, new), new_frame, slide.offset);

        f.advance_clock(LENGTH);
        assert!(slides(&f).is_empty() && closing(&f).is_empty() && opening(&f).is_empty());
    }
}

#[test]
fn with_the_animation_off_a_window_that_maps_part_way_isnt_the_slides() {
    for (per_output, outputs) in DESKS {
        let off = (
            CAMINITO,
            "open-close-animation".into(),
            Setting::Bool(false),
        );
        let (mut f, id, left, _) = desk(per_output, outputs, vec![off]);
        map_left(&mut f, id);
        switch_output(&mut f, &left, 1);
        f.advance_clock(PART_WAY);

        let new = f.map(id, WindowSpec::plain(SMALL));
        let pid = f.pid(new).0;
        let slide = slide_on(&f, &left);
        let moved = slide.windows.iter().any(|(id, _)| *id == pid);
        assert!(
            !moved,
            "it's at its place, as the slide has it (235): {slide:?}"
        );
    }
}

#[test]
fn a_closing_windows_picture_slides_away_with_its_workspace() {
    for (per_output, outputs) in DESKS {
        let (mut f, id, left, _) = desk(per_output, outputs, Vec::new());
        // On the right of the left output, so that part-way its picture is still on it.
        let old = put_at(&mut f, id, RIGHT_OF_LEFT, 0, SMALL);
        f.advance_clock(LENGTH);
        let old_frame = f.visible_frame(old);
        let old_id = f.pid(old).0;
        close(&mut f, old, Close::Destroy);
        f.advance_clock(PART_WAY);

        switch_output(&mut f, &left, 1);
        f.advance_clock(PART_WAY);

        let slide = slide_on(&f, &left);
        let away = Point::from((OUTPUT.0, 0));
        let picture = the_picture(&f, old_id);
        assert_scaled_and_moved(picture.rect, old_frame, slide.offset - away);
        // The scene draws it there, faint, and nothing of it where it was.
        let drawn = middle(picture.rect);
        assert!(
            Rectangle::from_size(OUTPUT.into()).contains(drawn),
            "{drawn:?}"
        );
        let faint = pixel_on(&mut f, &left, drawn);
        assert!(
            faint != WINDOW && faint != clear(),
            "drawn faint: {faint:#08x}"
        );
        assert_eq!(pixel_on(&mut f, &left, middle(old_frame)), clear());
    }
}

/// Checks that neither a window that maps nor one that closes now is animated.
#[track_caller]
fn assert_neither_animates(f: &mut Fixture, id: ClientId, doomed: ClientWindow) {
    let new = f.map(id, WindowSpec::plain(SMALL));
    assert!(opening(f).is_empty(), "it doesn't open: {:?}", opening(f));
    close(f, doomed, Close::Destroy);
    assert!(closing(f).is_empty(), "it doesn't close: {:?}", closing(f));
    f.advance_clock(PART_WAY);
    assert!(opening(f).is_empty() && closing(f).is_empty());
    close(f, new, Close::Destroy);
}

#[test]
fn neither_runs_while_the_overview_is_open_and_one_that_runs_is_gone_in_the_next_frame() {
    for (per_output, outputs) in DESKS {
        let (mut f, id, _, _) = desk(per_output, outputs, Vec::new());
        let doomed = map_left(&mut f, id);
        let kept = map_left(&mut f, id);
        let _opened = f.map(id, WindowSpec::plain(SMALL));
        close(&mut f, doomed, Close::Destroy);
        f.advance_clock(PART_WAY);
        assert_eq!((opening(&f).len(), closing(&f).len()), (1, 1));

        overview::open(&mut f);
        assert!(
            opening(&f).is_empty(),
            "the Overview ended it: {:?}",
            opening(&f)
        );
        assert!(
            closing(&f).is_empty(),
            "the Overview ended it: {:?}",
            closing(&f)
        );

        assert_neither_animates(&mut f, id, kept);
    }
}

#[test]
fn neither_runs_during_the_overviews_transition_either_way() {
    for (per_output, outputs) in DESKS {
        let animation = (CAMINITO, "overview-animation".into(), Setting::Bool(true));
        let (mut f, id, _, _) = desk(per_output, outputs, vec![animation]);
        let first = map_left(&mut f, id);
        let second = map_left(&mut f, id);

        overview::open(&mut f);
        assert!(overview_transition_runs(&f), "it opens with its transition");
        assert_neither_animates(&mut f, id, first);
        f.advance_clock(crate::workspace_slide::LENGTH);
        assert!(!overview_transition_runs(&f), "it's open at rest");

        overview::press_shortcut(&mut f);
        assert!(!overview::is_open(&f), "the shortcut closes it");
        assert!(
            overview_transition_runs(&f),
            "it closes with its transition"
        );
        assert_neither_animates(&mut f, id, second);
    }
}

/// Whether the Overview's transition runs, from the debug dump.
fn overview_transition_runs(f: &Fixture) -> bool {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    !dump["overview_transition"].is_null()
}

#[test]
fn locking_part_way_ends_both_at_once_and_a_window_that_opens_under_the_lock_isnt_animated() {
    for (per_output, outputs) in DESKS {
        let (mut f, id, _, _) = desk(per_output, outputs, Vec::new());
        let doomed = map_left(&mut f, id);
        let later = map_left(&mut f, id);
        let _opened = f.map(id, WindowSpec::plain(SMALL));
        close(&mut f, doomed, Close::Destroy);
        f.advance_clock(PART_WAY);
        assert_eq!((opening(&f).len(), closing(&f).len()), (1, 1));
        let locker = f.connect(Kind::Xdg);

        let lock = f.client(locker).lock_session();
        f.roundtrip(locker);
        assert_ne!(f.lock_state(), "unlocked");
        assert!(
            opening(&f).is_empty(),
            "the lock ended it: {:?}",
            opening(&f)
        );
        assert!(
            closing(&f).is_empty(),
            "the lock ended it: {:?}",
            closing(&f)
        );

        let _under = f.map(id, WindowSpec::plain(SMALL));
        close(&mut f, later, Close::Destroy);
        assert!(opening(&f).is_empty() && closing(&f).is_empty());
        f.client(locker).unlock_and_destroy(lock);
        f.roundtrip(locker);
        assert_eq!(f.lock_state(), "unlocked");
        assert!(opening(&f).is_empty(), "it isn't animated after the lock");
    }
}

#[test]
fn an_output_removed_part_way_drops_the_pictures_on_it() {
    for per_output in MODES {
        let (mut f, id, _, right) = desk(per_output, 2, Vec::new());
        let right = right.expect("two outputs");
        let on_left = map_left(&mut f, id);
        let on_right = put_at(&mut f, id, LOW_RIGHT, 0, SMALL);
        f.advance_clock(LENGTH);
        let (left_id, right_id) = (f.pid(on_left).0, f.pid(on_right).0);
        close(&mut f, on_left, Close::Destroy);
        close(&mut f, on_right, Close::Destroy);
        f.advance_clock(PART_WAY);
        let ids = |f: &Fixture| closing(f).iter().map(|p| p.id).collect::<Vec<_>>();
        assert_eq!(ids(&f), [left_id, right_id]);

        f.unplug(&right);
        assert_eq!(ids(&f), [left_id], "the right one's picture went with it");
    }
}
