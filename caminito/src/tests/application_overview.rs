//! The Application overview (CMN-185) on two outputs, under Per-output workspaces and under
//! Spanning workspaces: its shortcut opens it on both, each listing the focused window's
//! application's windows alone while the bars' cells draw every window, opens nothing with
//! no window focused, and changes to the Overview in place and back, the windows that join
//! fading in and those that leave fading out, read from the debug dump. A hot corner opens
//! it on its own output and closes it.
//!
//! The outputs are `two_outputs_with`'s. The two applications are two Wayland clients, one
//! of them taken for another process, so the windows are xdg toplevels only: the fixture's
//! X clients set no `_NET_WM_PID`, and which windows are listed is policy's, the same for
//! both kinds.

use std::time::Duration;

use smithay::output::Output;

use super::fixture::{ClientWindow, Fixture, KEY_A, KEY_LEFTMETA, KEY_W, Kind};
use super::overview::{is_open, minimise, overview, rect};
use super::overview_outputs::{ids, listed_on, on};
use super::per_output_workspaces::{two_outputs_with, window_at};
use crate::settings::Value;
use crate::settings::translate::CAMINITO;
use crate::state::SEAT;
use crate::workspace_slide::LENGTH;

/// The middle of the easing, where the ease-out has the windows seven eighths of the way.
const MIDDLE: Duration = Duration::from_millis(125);

/// Two applications' windows on the two outputs, with `workspaces-per-output` as
/// `per_output` says, the Application overview's shortcut Super+A, `overview-animation`
/// on and `settings` set, and Caminito's clock stopped. On the left output are `a` of the
/// one at (100, 100) and `b` of the other at (900, 500), and on the right one `c` of the
/// one at (2300, 200) and `d` of the other at (2900, 300). `a` is focused.
fn desk(
    per_output: bool,
    mut settings: Vec<(&'static str, String, Value)>,
) -> (Fixture, Output, Output, [ClientWindow; 4]) {
    settings.push((
        CAMINITO,
        "application-overview".into(),
        Value::Str("<Super>a".into()),
    ));
    settings.push((CAMINITO, "overview-animation".into(), Value::Bool(true)));
    let (mut f, one, left, right) = two_outputs_with(Kind::Xdg, per_output, settings);
    let other = f.connect_as_process(1);
    let a = window_at(&mut f, one, (100, 100), 0);
    let b = window_at(&mut f, other, (900, 500), 0);
    let c = window_at(&mut f, one, (2300, 200), 0);
    let d = window_at(&mut f, other, (2900, 300), 0);
    let id = f.pid(a);
    let now = f.state.caminito.now();
    f.state.caminito.policy.activate(SEAT, id, now);
    f.stop_clock();
    f.dispatch();
    (f, left, right, [a, b, c, d])
}

fn press_shortcut(f: &mut Fixture) {
    f.chord(&[KEY_LEFTMETA, KEY_A]);
}

fn press_overviews_shortcut(f: &mut Fixture) {
    f.chord(&[KEY_LEFTMETA, KEY_W]);
}

fn scope(f: &Fixture) -> String {
    overview(f)["scope"].as_str().unwrap().to_owned()
}

/// `windows`' ids, in order.
fn sorted(mut windows: Vec<u64>) -> Vec<u64> {
    windows.sort_unstable();
    windows
}

/// The windows the cell of `workspace` of `output`'s bar draws.
fn pictured(f: &Fixture, output: &Output, workspace: u64) -> Vec<u64> {
    let bar = on(f, output)["bar"].as_array().unwrap().clone();
    let cell = bar
        .iter()
        .find(|cell| cell["workspace"] == workspace)
        .unwrap();
    let windows = cell["windows"].as_array().unwrap().iter();
    sorted(windows.map(|id| id.as_u64().unwrap()).collect())
}

/// The windows fading on `output`'s layout, with how opaque each is drawn.
fn fading(f: &Fixture, output: &Output) -> Vec<(u64, f32)> {
    let fading = on(f, output)["fading"].as_array().unwrap().clone();
    fading
        .iter()
        .map(|window| {
            let alpha = window["alpha"].as_f64().unwrap() as f32;
            (window["window"].as_u64().unwrap(), alpha)
        })
        .collect()
}

#[test]
fn its_shortcut_opens_it_on_both_outputs_with_the_focused_applications_windows_alone() {
    for per_output in [true, false] {
        let (mut f, left, right, [a, b, c, d]) = desk(per_output, vec![]);

        press_shortcut(&mut f);

        assert_eq!(scope(&f), "application", "per output: {per_output}");
        assert_eq!(listed_on(&f, &left), ids(&f, &[a]));
        assert_eq!(listed_on(&f, &right), ids(&f, &[c]));
        assert_eq!(overview(&f)["selected"], f.pid(a).0);
        // The bars draw each workspace as it is.
        assert_eq!(pictured(&f, &left, 0), sorted(ids(&f, &[a, b])));
        assert_eq!(pictured(&f, &right, 0), sorted(ids(&f, &[c, d])));
        // Each output's layout has its windows' places.
        assert!(!rect(&on(&f, &right)["windows"][0]["rect"]).is_empty());
    }
}

#[test]
fn with_no_window_focused_its_shortcut_opens_nothing() {
    let (mut f, _, _, windows) = desk(true, vec![]);
    for window in windows {
        minimise(&mut f, window);
    }
    assert_eq!(f.state.caminito.policy.focus(SEAT), None);

    press_shortcut(&mut f);

    assert!(!is_open(&f));
}

#[test]
fn the_overviews_shortcut_changes_it_in_place_and_its_own_closes_it() {
    let (mut f, left, right, [a, b, c, d]) = desk(true, vec![]);
    press_shortcut(&mut f);
    f.advance_clock(LENGTH);

    press_overviews_shortcut(&mut f);

    assert_eq!(scope(&f), "overview");
    assert_eq!(sorted(listed_on(&f, &left)), sorted(ids(&f, &[a, b])));
    assert_eq!(sorted(listed_on(&f, &right)), sorted(ids(&f, &[c, d])));
    assert_eq!(overview(&f)["selected"], f.pid(a).0);
    // The windows that join fade in.
    assert_eq!(fading(&f, &left), [(f.pid(b).0, 0.0)]);
    assert_eq!(fading(&f, &right), [(f.pid(d).0, 0.0)]);
    f.advance_clock(MIDDLE);
    assert_eq!(fading(&f, &left), [(f.pid(b).0, 0.875)]);
    f.advance_clock(LENGTH);
    assert_eq!(fading(&f, &left), []);

    press_shortcut(&mut f);

    assert_eq!(scope(&f), "application");
    assert_eq!(listed_on(&f, &left), ids(&f, &[a]));
    // Those that leave fade out where they were drawn.
    assert_eq!(fading(&f, &left), [(f.pid(b).0, 1.0)]);
    f.advance_clock(MIDDLE);
    assert_eq!(fading(&f, &left), [(f.pid(b).0, 0.125)]);

    press_shortcut(&mut f);

    assert!(!is_open(&f));
}

#[test]
fn its_shortcut_opens_it_with_the_selection_on_the_pointers_output() {
    let (mut f, left, right, [a, _, c, _]) = desk(true, vec![]);
    f.pointer_move(3500., 900.);

    press_shortcut(&mut f);

    // The focused window's application's, with the selection on the pointer's output
    // (ADR 0046), where nothing is selected since the focused window isn't there.
    assert_eq!(scope(&f), "application");
    assert_eq!(overview(&f)["output"], right.name());
    assert_eq!(overview(&f)["selected"], serde_json::Value::Null);
    assert_eq!(listed_on(&f, &left), ids(&f, &[a]));
    assert_eq!(listed_on(&f, &right), ids(&f, &[c]));
}

#[test]
fn a_hot_corner_set_to_it_opens_it_on_its_output_and_closes_it() {
    let corner = (
        CAMINITO,
        "hot-corner-top-right".into(),
        Value::Str("application-overview".into()),
    );
    let (mut f, _, right, [_, _, c, _]) = desk(true, vec![corner]);

    f.pointer_move(3839., 0.);
    f.advance_clock(Duration::from_millis(150));

    assert_eq!(scope(&f), "application");
    assert_eq!(overview(&f)["output"], right.name());
    assert_eq!(listed_on(&f, &right), ids(&f, &[c]));

    f.pointer_move(3000., 500.);
    f.advance_clock(Duration::from_millis(350));
    f.pointer_move(3839., 0.);
    f.advance_clock(Duration::from_millis(150));

    assert!(!is_open(&f));
}
