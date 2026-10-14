//! What X11 clients are told of Per-output workspaces (ADR 0032, 228): the Active output's
//! active workspace as `_NET_CURRENT_DESKTOP`, a window's number among its own output's as
//! `_NET_WM_DESKTOP`, the requests to change them, and X's stacking order. They're X
//! properties and requests, so these tests map XWayland windows only.

use smithay::output::Output;

use super::fixture::{ClientId, ClientWindow, Fixture, Kind};
use super::per_output_workspaces::{active_workspaces, click_focuses, two_outputs_with, window_at};
use super::root_properties::root_properties;
use super::window_properties::{property, send};
use super::xwayland_windows::client_list_stacking;
use crate::output::output_id;

/// The root's `_NET_CURRENT_DESKTOP`, as the X client `x` reads it.
fn current_desktop(f: &mut Fixture, x: ClientId) -> u32 {
    let line = root_properties(f, x, &["_NET_CURRENT_DESKTOP"]);
    let value = line.strip_prefix("_NET_CURRENT_DESKTOP(CARDINAL) = ");
    value.and_then(|n| n.parse().ok()).expect(&line)
}

/// Asks for `desktop` as the current one, as a pager does.
fn request_current_desktop(f: &mut Fixture, x: ClientId, desktop: u32) {
    f.x11_call(x, move |x| {
        x.send_client_message(x.root(), "_NET_CURRENT_DESKTOP", [desktop, 0, 0, 0, 0])
    })
    .unwrap();
}

/// The windows of `two_outputs_on_different_workspaces`.
struct Windows {
    /// On the left output's workspace 0, which it shows.
    on_left: ClientWindow,
    /// On the right output's workspace 1, which it shows.
    on_right: ClientWindow,
    /// On the right output's workspace 0.
    put_away_right: ClientWindow,
    /// On the left output's workspace 1.
    put_away_left: ClientWindow,
}

/// Two outputs with `workspaces-per-output` as `per_output` says, a window on each one's
/// workspaces 0 and 1, and then the right output switched to workspace 1: by itself, or
/// with the left one where the key is off. The windows of the workspaces put away mapped
/// last, so they're the top ones of policy's one stack.
fn two_outputs_on_different_workspaces(
    per_output: bool,
) -> (Fixture, ClientId, Windows, Output, Output) {
    let (mut f, x, left, right) = two_outputs_with(Kind::X11, per_output, vec![]);
    let on_left = window_at(&mut f, x, (100, 100), 0);
    let on_right = window_at(&mut f, x, (2500, 600), 1);
    let put_away_right = window_at(&mut f, x, (2100, 100), 0);
    let put_away_left = window_at(&mut f, x, (600, 500), 1);
    let policy = &mut f.state.caminito.policy;
    policy.switch_output_workspace(output_id(&right), 1);
    f.dispatch();
    let windows = Windows {
        on_left,
        on_right,
        put_away_right,
        put_away_left,
    };
    (f, x, windows, left, right)
}

/// A press on the bare background of the right output.
fn press_on_the_right_output(f: &mut Fixture) {
    f.wait_between_inputs(1000);
    f.click(3500., 1000.);
}

#[test]
fn net_current_desktop_is_the_active_outputs_and_follows_a_click_with_no_switch() {
    let (mut f, x, windows, left, right) = two_outputs_on_different_workspaces(true);
    assert_eq!(active_workspaces(&f, &left, &right), (0, 1));

    assert!(click_focuses(&mut f, (300, 250), windows.on_left));
    assert_eq!(current_desktop(&mut f, x), 0);

    // The press makes the right output the Active output, and switches nothing.
    press_on_the_right_output(&mut f);
    assert_eq!(active_workspaces(&f, &left, &right), (0, 1));
    assert_eq!(current_desktop(&mut f, x), 1);

    assert!(click_focuses(&mut f, (300, 250), windows.on_left));
    assert_eq!(current_desktop(&mut f, x), 0);
}

#[test]
fn a_current_desktop_message_switches_the_active_output_only() {
    let (mut f, x, windows, left, right) = two_outputs_on_different_workspaces(true);
    assert!(click_focuses(&mut f, (300, 250), windows.on_left));
    press_on_the_right_output(&mut f);

    request_current_desktop(&mut f, x, 2);
    let right_id = output_id(&right);
    f.dispatch_until(|f| f.state.caminito.policy.active_workspace_on(right_id) == 2);

    assert_eq!(active_workspaces(&f, &left, &right), (0, 2));
    assert_eq!(current_desktop(&mut f, x), 2);
}

#[test]
fn a_desktop_message_moves_a_window_of_the_other_output_within_that_output() {
    let (mut f, _, windows, left, right) = two_outputs_on_different_workspaces(true);
    let on_right = windows.on_right;
    // The Active output is the left one, which shows workspace 0.
    assert!(click_focuses(&mut f, (300, 250), windows.on_left));
    assert_eq!(
        property(&mut f, on_right, "_NET_WM_DESKTOP"),
        "_NET_WM_DESKTOP(CARDINAL) = 1"
    );

    // Source indication 2: a pager.
    send(&mut f, on_right, "_NET_WM_DESKTOP", [2, 2, 0, 0, 0]);
    f.dispatch_until(|f| f.state.caminito.policy.workspace_of(f.pid(on_right)) == Some(2));

    let policy = &f.state.caminito.policy;
    assert_eq!(
        policy.main_output_of(f.pid(on_right)),
        Some(output_id(&right))
    );
    assert!(!policy.is_showing(f.pid(on_right)));
    assert_eq!(active_workspaces(&f, &left, &right), (0, 1));
    f.sync(on_right);
    assert_eq!(
        property(&mut f, on_right, "_NET_WM_DESKTOP"),
        "_NET_WM_DESKTOP(CARDINAL) = 2"
    );

    // Every desktop makes it sticky there, on its own output.
    send(
        &mut f,
        on_right,
        "_NET_WM_DESKTOP",
        [0xFFFF_FFFF, 2, 0, 0, 0],
    );
    f.dispatch_until(|f| f.state.caminito.policy.is_sticky(f.pid(on_right)));

    let policy = &f.state.caminito.policy;
    assert_eq!(
        policy.main_output_of(f.pid(on_right)),
        Some(output_id(&right))
    );
    assert!(policy.is_showing(f.pid(on_right)));
    f.sync(on_right);
    assert_eq!(
        property(&mut f, on_right, "_NET_WM_DESKTOP"),
        "_NET_WM_DESKTOP(CARDINAL) = 4294967295"
    );
}

/// The XWayland windows put away and those showing, each sorted, as X's stacking order has
/// them: the bottom two and the top two.
fn stacked(f: &mut Fixture, x: ClientId) -> (Vec<u32>, Vec<u32>) {
    let stacking = client_list_stacking(f, x);
    assert_eq!(stacking.len(), 4, "{stacking:?}");
    let (mut below, mut above) = (stacking[..2].to_vec(), stacking[2..].to_vec());
    below.sort();
    above.sort();
    (below, above)
}

/// The X ids of `windows`, sorted as `stacked` sorts each half.
fn sorted(windows: [ClientWindow; 2]) -> Vec<u32> {
    let mut ids = windows.map(ClientWindow::x11).to_vec();
    ids.sort();
    ids
}

#[test]
fn xs_stacking_order_has_every_showing_window_above_every_hidden_one() {
    let (mut f, x, windows, _, right) = two_outputs_on_different_workspaces(true);

    assert_eq!(
        stacked(&mut f, x),
        (
            sorted([windows.put_away_right, windows.put_away_left]),
            sorted([windows.on_left, windows.on_right])
        )
    );

    // And as the right output switches back, with the left one as it was.
    let policy = &mut f.state.caminito.policy;
    policy.switch_output_workspace(output_id(&right), 0);
    f.dispatch();
    assert_eq!(
        stacked(&mut f, x),
        (
            sorted([windows.on_right, windows.put_away_left]),
            sorted([windows.on_left, windows.put_away_right])
        )
    );
}

#[test]
fn with_the_key_off_x11_clients_are_told_of_the_one_active_workspace() {
    let (mut f, x, windows, left, right) = two_outputs_on_different_workspaces(false);
    assert_eq!(active_workspaces(&f, &left, &right), (1, 1));
    assert_eq!(current_desktop(&mut f, x), 1);
    assert_eq!(
        stacked(&mut f, x),
        (
            sorted([windows.on_left, windows.put_away_right]),
            sorted([windows.on_right, windows.put_away_left])
        )
    );

    // A message switches every output.
    request_current_desktop(&mut f, x, 0);
    f.dispatch_until(|f| f.state.caminito.policy.active_workspace() == 0);
    assert_eq!(active_workspaces(&f, &left, &right), (0, 0));
    // And a press on the other output changes nothing.
    press_on_the_right_output(&mut f);
    assert_eq!(current_desktop(&mut f, x), 0);

    let put_away_right = windows.put_away_right;
    send(&mut f, put_away_right, "_NET_WM_DESKTOP", [1, 2, 0, 0, 0]);
    f.dispatch_until(|f| f.state.caminito.policy.workspace_of(f.pid(put_away_right)) == Some(1));
    f.sync(put_away_right);
    assert_eq!(
        property(&mut f, put_away_right, "_NET_WM_DESKTOP"),
        "_NET_WM_DESKTOP(CARDINAL) = 1"
    );
    assert_eq!(active_workspaces(&f, &left, &right), (0, 0));
}
