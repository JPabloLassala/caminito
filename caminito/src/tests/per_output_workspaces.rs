//! Per-output workspaces (ADR 0032, 230): with `workspaces-per-output` on, two outputs on
//! different workspaces, each drawing its own workspace's windows, and input reaching only
//! the windows that show.

use caminito_test_client::dbus::debug_proxy;
use caminito_test_client::layer_shell::LayerSpec;
use caminito_theme::ButtonFunction;
use smithay::output::Output;
use smithay::utils::{Logical, Point};

use super::fixture::{
    BTN_LEFT, ClientId, ClientWindow, Fixture, KEY_LEFT, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT,
    KEY_TAB, Kind, WindowSpec, on_both_kinds,
};
use crate::output::{output_geometry, output_id};
use crate::settings::Value;
use crate::settings::translate::{CAMINITO, MARCO_GENERAL};
use crate::state::SEAT;
use crate::switcher::POPUP_DELAY;

const GREEN: u32 = 0x00ff00;

/// Two 1920 × 1080 outputs side by side with `workspaces-per-output` on, the left one the
/// primary, and a client of `kind`.
fn two_outputs(kind: Kind) -> (Fixture, ClientId, Output, Output) {
    two_outputs_with(kind, true, vec![])
}

/// `two_outputs`, with `workspaces-per-output` as `per_output` says and `settings` besides.
fn two_outputs_with(
    kind: Kind,
    per_output: bool,
    mut settings: Vec<(&'static str, String, Value)>,
) -> (Fixture, ClientId, Output, Output) {
    settings.push((
        CAMINITO,
        "workspaces-per-output".into(),
        Value::Bool(per_output),
    ));
    let mut f = Fixture::with_settings(settings);
    let left = f.add_output((1920, 1080), 1.0);
    let right = f.add_output((1920, 1080), 1.0);
    f.choose_primary_output(&left);
    assert_eq!(output_geometry(&right).loc, (1920, 0).into());
    let client = f.connect(kind);
    (f, client, left, right)
}

/// Maps a 400 × 300 window, drags it so that its geometry is at the global point `at`, and
/// puts it on `workspace`. A drag can leave it across two outputs, where a client's request
/// would be kept on one.
fn window_at(f: &mut Fixture, client: ClientId, at: (i32, i32), workspace: usize) -> ClientWindow {
    let window = f.map(client, WindowSpec::plain((400, 300)));
    let id = f.pid(window);
    let from = f.geometry(window).loc.to_f64();
    let to = Point::<i32, Logical>::from(at).to_f64();
    let policy = &mut f.state.caminito.policy;
    assert!(policy.begin_move(SEAT, id, from));
    policy.grab_motion(SEAT, to, false);
    policy.end_grab(SEAT);
    policy.move_to_workspace(id, workspace);
    f.dispatch();
    assert_eq!(
        f.geometry(window).loc,
        at.into(),
        "the window is where it was put"
    );
    window
}

/// `output`'s pixel at the global point `at`, as `0xRRGGBB`. Nothing asks for a frame but
/// what changed, so a switch shows here only if it had the outputs drawn again.
fn pixel(f: &mut Fixture, output: &Output, at: (i32, i32)) -> u32 {
    f.dispatch();
    let local = Point::<i32, Logical>::from(at) - output_geometry(output).loc;
    let pixel = f
        .state
        .backend
        .headless_mut()
        .pixel(output, local.to_physical(1));
    pixel.expect("the output is drawn") & 0xff_ffff
}

/// Whether a click at the global point `at` focuses `window`.
fn click_focuses(f: &mut Fixture, at: (i32, i32), window: ClientWindow) -> bool {
    f.wait_between_inputs(1000);
    f.click(f64::from(at.0), f64::from(at.1));
    f.state.caminito.policy.focus(SEAT) == Some(f.pid(window))
}

fn switch(f: &mut Fixture, output: &Output, workspace: u32) {
    let bus = f.start_dbus();
    let name = output.name();
    let switched = f.dbus_call(move || {
        debug_proxy(&bus)
            .unwrap()
            .switch_output_workspace(&name, workspace)
            .unwrap()
    });
    assert!(switched, "there's such an output and such a workspace");
    f.dispatch();
}

on_both_kinds! {
    fn each_output_draws_its_own_workspace_and_input_reaches_only_what_shows(kind: Kind) {
        let (mut f, client, left, right) = two_outputs(kind);
        // On the left output, workspace 0.
        let on_left = window_at(&mut f, client, (100, 100), 0);
        // On the right output, workspace 0, which it's switched away from.
        let put_away = window_at(&mut f, client, (2100, 100), 0);
        // On the right output, workspace 1, which it's switched to.
        let on_right = window_at(&mut f, client, (2500, 600), 1);
        // Three quarters on the left output, which is its Main output.
        let straddling = window_at(&mut f, client, (1620, 600), 0);

        switch(&mut f, &right, 1);

        let policy = &f.state.caminito.policy;
        assert_eq!(policy.active_workspace_on(output_id(&left)), 0);
        assert_eq!(policy.active_workspace_on(output_id(&right)), 1);
        assert_eq!(pixel(&mut f, &left, (300, 250)), GREEN, "the left output's window");
        assert_ne!(pixel(&mut f, &right, (2300, 250)), GREEN, "the put away window");
        assert_eq!(pixel(&mut f, &right, (2700, 750)), GREEN, "the right output's window");
        assert_eq!(
            pixel(&mut f, &left, (1800, 750)),
            GREEN,
            "the straddling window on its Main output"
        );
        assert_eq!(
            pixel(&mut f, &right, (1970, 750)),
            GREEN,
            "the straddling window on the other output"
        );

        assert!(click_focuses(&mut f, (2700, 750), on_right));
        assert!(!click_focuses(&mut f, (2300, 250), put_away), "a window put away");
        assert!(click_focuses(&mut f, (300, 250), on_left));
        assert!(click_focuses(&mut f, (1970, 750), straddling), "on the other output");
        // The switch of the right output left the left one's windows as they were.
        assert!(f.state.caminito.policy.is_showing(f.pid(on_left)));
    }
}

#[test]
fn the_debug_interface_refuses_an_output_or_a_workspace_there_isnt() {
    let (mut f, _, _, right) = two_outputs(Kind::Xdg);
    let bus = f.start_dbus();
    let name = right.name();
    let answers = f.dbus_call(move || {
        let debug = debug_proxy(&bus).unwrap();
        (
            debug.switch_output_workspace("NOWHERE-1", 1).unwrap(),
            debug.switch_output_workspace(&name, 4).unwrap(),
        )
    });
    assert_eq!(answers, (false, false));
    assert_eq!(
        f.state
            .caminito
            .policy
            .active_workspace_on(output_id(&right)),
        0
    );
}

#[test]
fn turning_the_key_off_and_on_applies_at_once() {
    let (mut f, client, left, right) = two_outputs(Kind::Xdg);
    let on_left = window_at(&mut f, client, (100, 100), 0);
    let on_right = window_at(&mut f, client, (2500, 600), 2);
    switch(&mut f, &right, 2);
    let active = |f: &Fixture| {
        let policy = &f.state.caminito.policy;
        (
            policy.active_workspace_on(output_id(&left)),
            policy.active_workspace_on(output_id(&right)),
        )
    };
    assert_eq!(active(&f), (0, 2));

    // Off: every output takes the Active output's, the left one's once its window is
    // focused, and no window's number changes.
    assert!(click_focuses(&mut f, (150, 150), on_left));
    f.set_setting(CAMINITO, "workspaces-per-output", Value::Bool(false));
    f.dispatch_until(|f| !f.state.caminito.policy.workspaces_per_output());
    assert_eq!(active(&f), (0, 0));
    assert_eq!(
        f.state.caminito.policy.workspace_of(f.pid(on_right)),
        Some(2)
    );
    assert_ne!(pixel(&mut f, &right, (2700, 750)), GREEN);
    assert_eq!(pixel(&mut f, &left, (300, 250)), GREEN);
    // And a switch of one output switches both.
    switch(&mut f, &right, 2);
    assert_eq!(active(&f), (2, 2));
    assert_ne!(pixel(&mut f, &left, (300, 250)), GREEN);

    // On: every output keeps what it shows, and switches by itself from there.
    f.set_setting(CAMINITO, "workspaces-per-output", Value::Bool(true));
    f.dispatch_until(|f| f.state.caminito.policy.workspaces_per_output());
    assert_eq!(active(&f), (2, 2));
    switch(&mut f, &left, 0);
    assert_eq!(active(&f), (0, 2));
    assert_eq!(pixel(&mut f, &left, (300, 250)), GREEN);
    assert_eq!(pixel(&mut f, &right, (2700, 750)), GREEN);
    assert_eq!(
        f.state.caminito.policy.workspace_of(f.pid(on_left)),
        Some(0)
    );
}

/// The debug dump.
fn dump(f: &Fixture) -> serde_json::Value {
    serde_json::from_str(&f.state.caminito.state_dump()).unwrap()
}

// On one kind: where the popup opens goes by the outputs, whatever the windows are.
#[test]
fn the_workspace_popup_is_on_the_active_output_with_the_pointer_on_the_other() {
    let (mut f, client, left, right) = two_outputs(Kind::Xdg);
    window_at(&mut f, client, (100, 100), 0);
    let on_right = window_at(&mut f, client, (2500, 600), 0);
    f.pointer_move(300., 900.);
    assert_eq!(dump(&f)["focus"], f.pid(on_right).0);
    assert_eq!(dump(&f)["active_output"], right.name());

    f.key(KEY_LEFTCTRL, true);
    f.key(KEY_LEFTALT, true);
    f.key(KEY_RIGHT, true);
    f.key(KEY_RIGHT, false);

    // The binding switched the right output, where the focused window is, and the popup
    // shows that output's workspaces there.
    let dumped = dump(&f);
    assert_eq!(dumped["workspace_popup"]["output"], right.name());
    assert_eq!(dumped["workspace_popup"]["selected"], 1);
    assert_eq!(dumped["outputs"][0]["name"], left.name());
    assert_eq!(dumped["outputs"][0]["active_workspace"], 0);
    assert_eq!(dumped["outputs"][1]["active_workspace"], 1);
    // The workspace is empty, so nothing is focused, and it's the Active output still.
    assert_eq!(dumped["focus"], serde_json::Value::Null);
    assert_eq!(dumped["active_output"], right.name());

    // A second press acts on the same output, and the popup stays.
    f.key(KEY_RIGHT, true);
    f.key(KEY_RIGHT, false);
    let dumped = dump(&f);
    assert_eq!(dumped["workspace_popup"]["output"], right.name());
    assert_eq!(dumped["workspace_popup"]["selected"], 2);
    assert_eq!(dumped["outputs"][0]["active_workspace"], 0);
    assert_eq!(dumped["outputs"][1]["active_workspace"], 2);

    f.key(KEY_LEFTALT, false);
    f.key(KEY_LEFTCTRL, false);
    assert!(dump(&f)["workspace_popup"].is_null());
}

// The Active output follows a press, and a new window opens on it (222).

/// Each output's active workspace, left then right.
fn active_workspaces(f: &Fixture, left: &Output, right: &Output) -> (usize, usize) {
    let policy = &f.state.caminito.policy;
    (
        policy.active_workspace_on(output_id(left)),
        policy.active_workspace_on(output_id(right)),
    )
}

fn switch_right(f: &mut Fixture) {
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
}

/// Where a 400 × 300 window mapped now opens: its output's name, its workspace, and
/// whether it's showing.
fn opens(f: &mut Fixture, client: ClientId) -> (String, Option<usize>, bool) {
    let window = f.map(client, WindowSpec::plain((400, 300)));
    let id = f.pid(window);
    let policy = &f.state.caminito.policy;
    let output = policy.output_showing(id);
    let outputs = &f.state.caminito.outputs;
    let output = outputs.iter().find(|o| Some(output_id(o)) == output);
    (
        output.expect("the window is on an output").name(),
        policy.workspace_of(id),
        policy.is_showing(id),
    )
}

// On one kind: a press on what's no window goes by the outputs, whatever the windows are.
#[test]
fn a_press_on_the_other_outputs_panel_or_background_makes_it_the_active_output() {
    let (mut f, client, left, right) = two_outputs(Kind::Xdg);
    window_at(&mut f, client, (100, 100), 0);
    let on_right = window_at(&mut f, client, (2500, 600), 0);
    // A panel along the top of the left output, the one the pointer is on.
    f.pointer_move(900., 900.);
    let panel = f.connect(Kind::Xdg);
    f.map_layer(panel, LayerSpec::top_bar(28), 0xff40_4040);
    assert_eq!(dump(&f)["active_output"], right.name());

    // Moving the pointer to the other output, onto its panel, changes nothing.
    f.pointer_move(900., 10.);
    assert_eq!(dump(&f)["active_output"], right.name());
    switch_right(&mut f);
    assert_eq!(active_workspaces(&f, &left, &right), (0, 1));
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFT]);
    assert_eq!(active_workspaces(&f, &left, &right), (0, 0));
    assert_eq!(dump(&f)["focus"], f.pid(on_right).0);

    // A press on the panel does, and leaves the focus where it is.
    f.wait_between_inputs(1000);
    f.click(900., 10.);
    assert_eq!(dump(&f)["active_output"], left.name());
    assert_eq!(dump(&f)["focus"], f.pid(on_right).0);
    switch_right(&mut f);
    assert_eq!(active_workspaces(&f, &left, &right), (1, 0));
    assert_eq!(dump(&f)["active_workspace"], 1);

    // As does one on the right output's bare background.
    f.wait_between_inputs(1000);
    f.click(3000., 1000.);
    assert_eq!(dump(&f)["active_output"], right.name());
    switch_right(&mut f);
    assert_eq!(active_workspaces(&f, &left, &right), (1, 1));
}

on_both_kinds! {
    fn a_window_mapped_after_a_switch_to_an_empty_workspace_opens_there(kind: Kind) {
        let (mut f, client, left, right) = two_outputs(kind);
        window_at(&mut f, client, (100, 100), 0);
        window_at(&mut f, client, (2500, 600), 0);
        // The pointer is on the left output, which decides nothing.
        f.pointer_move(300., 900.);
        switch_right(&mut f);
        assert_eq!(active_workspaces(&f, &left, &right), (0, 1));
        assert_eq!(dump(&f)["focus"], serde_json::Value::Null);

        assert_eq!(opens(&mut f, client), (right.name(), Some(1), true));
        assert_ne!(dump(&f)["focus"], serde_json::Value::Null);
    }
}

on_both_kinds! {
    fn a_window_mapped_under_pointer_placement_opens_on_the_pointers_output(kind: Kind) {
        let mode = (MARCO_GENERAL, "placement-mode".into(), Value::Str("pointer".into()));
        let (mut f, client, left, right) = two_outputs_with(kind, true, vec![mode]);
        window_at(&mut f, client, (100, 100), 0);
        window_at(&mut f, client, (2500, 600), 0);
        // The Active output is the right one, and the pointer is on the left one, which
        // shows workspace 2.
        switch(&mut f, &left, 2);
        f.pointer_move(900., 600.);
        assert_eq!(dump(&f)["active_output"], right.name());

        assert_eq!(opens(&mut f, client), (left.name(), Some(2), true));
    }
}

on_both_kinds! {
    fn with_the_key_off_a_new_window_opens_on_the_focused_windows_output(kind: Kind) {
        let (mut f, client, left, right) = two_outputs_with(kind, false, vec![]);
        window_at(&mut f, client, (100, 100), 0);
        let on_right = window_at(&mut f, client, (2500, 600), 0);
        // A press on the left output's background, which with the key on would say where.
        f.click(900., 900.);
        assert_eq!(dump(&f)["focus"], f.pid(on_right).0);

        assert_eq!(opens(&mut f, client), (right.name(), Some(0), true));

        // And on the primary output with no window left.
        switch_right(&mut f);
        assert_eq!(dump(&f)["focus"], serde_json::Value::Null);
        assert_eq!(opens(&mut f, client), (left.name(), Some(1), true));
    }
}

// On one kind: where the popup opens goes by the outputs, whatever the windows are.
#[test]
fn the_window_switchers_popup_opens_on_the_pointers_output_and_not_the_active_one() {
    let (mut f, client, left, right) = two_outputs(Kind::Xdg);
    window_at(&mut f, client, (100, 100), 0);
    window_at(&mut f, client, (2500, 600), 0);
    f.pointer_move(300., 900.);
    assert_eq!(dump(&f)["active_output"], right.name());

    f.key(KEY_LEFTALT, true);
    f.key(KEY_TAB, true);
    f.key(KEY_TAB, false);
    f.advance_clock(POPUP_DELAY);

    assert_eq!(dump(&f)["switcher"]["visible"], true);
    assert_eq!(dump(&f)["switcher"]["output"], left.name());
    f.key(KEY_LEFTALT, false);
}

on_both_kinds! {
    fn a_press_on_a_frames_button_makes_the_windows_output_the_active_output(kind: Kind) {
        let (mut f, client, left, right) = two_outputs(kind);
        let on_left = f.map(client, WindowSpec::default());
        window_at(&mut f, client, (2500, 600), 0);
        assert_eq!(dump(&f)["active_output"], right.name());

        // The press alone, which minimises nothing yet and focuses nothing.
        let minimize = f.button_rect(on_left, ButtonFunction::Minimize).to_f64();
        f.pointer_move(minimize.loc.x + 2., minimize.loc.y + 2.);
        f.wait_between_inputs(1000);
        f.pointer_button(BTN_LEFT, true);

        assert_eq!(dump(&f)["active_output"], left.name());
        f.pointer_button(BTN_LEFT, false);
    }
}
