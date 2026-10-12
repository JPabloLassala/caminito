//! Per-output workspaces (ADR 0032, 230): with `workspaces-per-output` on, two outputs on
//! different workspaces, each drawing its own workspace's windows, and input reaching only
//! the windows that show.

use caminito_test_client::dbus::debug_proxy;
use smithay::output::Output;
use smithay::utils::{Logical, Point};

use super::fixture::{ClientId, ClientWindow, Fixture, Kind, WindowSpec, on_both_kinds};
use crate::output::{output_geometry, output_id};
use crate::settings::Value;
use crate::settings::translate::CAMINITO;
use crate::state::SEAT;

const GREEN: u32 = 0x00ff00;

/// Two 1920 × 1080 outputs side by side with `workspaces-per-output` on, the left one the
/// primary, and a client of `kind`.
fn two_outputs(kind: Kind) -> (Fixture, ClientId, Output, Output) {
    let mut f = Fixture::with_settings(vec![(
        CAMINITO,
        "workspaces-per-output".into(),
        Value::Bool(true),
    )]);
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

    // Off: every output takes the primary output's, and no window's number changes.
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
