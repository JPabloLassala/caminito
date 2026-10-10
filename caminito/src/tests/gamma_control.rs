//! wlr-gamma-control (141, ADR 0023): a gamma client holds an output's table, and Night Light
//! steps back from that output alone until the client lets go. Only the tty offers it; here
//! the headless backend stands in for the tty's CRTCs when a test gives its monitors gamma
//! tables (`Fixture::with_gamma`), keeps the client's table, and the dump shows Night
//! Light's temperatures. Reading the table from its file is `protocols::gamma_control`'s
//! unit tests'.

use std::time::Duration;

use caminito_test_client::gamma_control::GammaControlId;
use smithay::output::Output;

use super::fixture::{ClientId, Fixture};
use crate::settings::Value;
use crate::settings::translate::CAMINITO;

const MANAGER: &str = "zwlr_gamma_control_manager_v1";

/// The size of each ramp the monitors have.
const SIZE: u32 = 4;

/// A gamma client's table: a dim red, green and blue.
const TABLE: [u16; 12] = [0, 100, 200, 300, 0, 110, 220, 330, 0, 120, 240, 360];

fn sees_manager(f: &Fixture, client: ClientId) -> bool {
    let globals = &f.client_state(client).globals;
    globals.iter().any(|global| global.interface == MANAGER)
}

/// A client that has bound every output.
fn gamma_client(f: &mut Fixture) -> ClientId {
    let client = f.add_client();
    f.roundtrip(client);
    f.client(client).bind_outputs();
    f.roundtrip(client);
    client
}

fn get_control(f: &mut Fixture, client: ClientId, output: usize) -> GammaControlId {
    let control = f.client(client).get_gamma_control(output);
    f.roundtrip(client);
    control
}

fn failed(f: &mut Fixture, client: ClientId, control: GammaControlId) -> bool {
    f.client(client).gamma_control(control).failed
}

fn night_light(f: &Fixture) -> serde_json::Value {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    dump["night_light"].clone()
}

/// The `index`th output's temperature in the dump, or `None` while a gamma client's table
/// is on it.
fn applied(f: &Fixture, index: usize) -> Option<u64> {
    night_light(f)["outputs"][index]["temperature"].as_u64()
}

fn held(f: &Fixture, index: usize) -> bool {
    night_light(f)["outputs"][index]["gamma_client"] == true
}

/// Gamma tables on `outputs` outputs, and Night Light on, at 4000 K, on each.
fn warm_outputs(outputs: usize) -> (Fixture, Vec<Output>) {
    let mut f = Fixture::with_gamma(SIZE);
    let outputs = (0..outputs)
        .map(|_| f.add_output((800, 600), 1.0))
        .collect();
    f.set_setting(
        CAMINITO,
        "night-light-schedule",
        Value::Str("constant".into()),
    );
    f.set_setting(CAMINITO, "night-light-enabled", Value::Bool(true));
    f.dispatch_until(|f| night_light(f)["target_temperature"] == 4000);
    f.advance_clock(Duration::from_secs(2));
    (f, outputs)
}

/// A gamma client holding the first of two warm outputs, with its table on it.
fn first_of_two_held() -> (Fixture, Vec<Output>, ClientId, GammaControlId) {
    let (mut f, outputs) = warm_outputs(2);
    let client = gamma_client(&mut f);
    let control = get_control(&mut f, client, 0);
    f.client(client).set_gamma(control, &TABLE);
    f.roundtrip(client);
    assert_eq!(applied(&f, 0), None);
    (f, outputs, client, control)
}

/// Night Light fades back in on the first output, from neutral, over two seconds.
fn assert_fades_back_in(f: &mut Fixture, outputs: &[Output]) {
    assert!(!held(f, 0));
    assert_eq!(f.client_gamma(&outputs[0]), None);
    // Only as long as the dispatches took has passed.
    let start = applied(f, 0).unwrap();
    assert!(start > 6400, "{start}");
    f.advance_clock(Duration::from_secs(1));
    let halfway = applied(f, 0).unwrap();
    assert!((4500..5500).contains(&halfway), "{halfway}");
    f.advance_clock(Duration::from_secs(1));
    assert_eq!(applied(f, 0), Some(4000));
}

#[test]
fn the_global_isnt_offered_on_the_headless_backend() {
    let mut f = Fixture::new();
    f.add_output((800, 600), 1.0);
    let client = gamma_client(&mut f);
    assert!(!sees_manager(&f, client));
}

#[test]
fn the_global_is_privileged() {
    let mut f = Fixture::with_gamma(SIZE);
    let ordinary = gamma_client(&mut f);
    let sandboxed = f.connect_sandboxed(ordinary);

    assert!(sees_manager(&f, ordinary));
    assert!(!sees_manager(&f, sandboxed));
}

#[test]
fn a_control_is_told_its_outputs_gamma_size() {
    let (mut f, _) = warm_outputs(1);
    let client = gamma_client(&mut f);

    let control = get_control(&mut f, client, 0);

    let control = f.client(client).gamma_control(control);
    assert_eq!(control.size, Some(SIZE));
    assert!(!control.failed);
    assert!(held(&f, 0));
}

#[test]
fn a_control_fails_at_once_on_an_output_with_no_gamma() {
    let (mut f, _) = warm_outputs(1);
    f.take_gamma_away();
    f.add_output((800, 600), 1.0);
    let client = gamma_client(&mut f);

    let control = get_control(&mut f, client, 1);

    assert!(failed(&mut f, client, control));
    assert_eq!(f.client(client).gamma_control(control).size, None);
    assert!(!held(&f, 1));
}

#[test]
fn a_second_control_on_a_held_output_fails() {
    let (mut f, outputs, first, _) = first_of_two_held();
    let second = gamma_client(&mut f);

    let control = get_control(&mut f, second, 0);

    assert!(failed(&mut f, second, control));
    assert_eq!(f.client_gamma(&outputs[0]), Some(TABLE.to_vec()));
    // The other output is free.
    let other = get_control(&mut f, second, 1);
    assert!(!failed(&mut f, second, other));
    f.roundtrip(first);
}

#[test]
fn the_clients_table_is_set_at_once_and_night_light_steps_back_from_that_output_alone() {
    let (mut f, outputs, _, _) = first_of_two_held();

    assert_eq!(f.client_gamma(&outputs[0]), Some(TABLE.to_vec()));
    assert!(held(&f, 0));
    assert_eq!(applied(&f, 0), None);
    assert!(!held(&f, 1));
    assert_eq!(applied(&f, 1), Some(4000));

    // A change of temperature reaches the other output only.
    f.set_setting(
        CAMINITO,
        "night-light-constant-temperature",
        Value::Uint(3000),
    );
    f.dispatch_until(|f| night_light(f)["target_temperature"] == 3000);
    f.advance_clock(Duration::from_secs(2));
    assert_eq!(applied(&f, 0), None);
    assert_eq!(applied(&f, 1), Some(3000));
    assert_eq!(f.client_gamma(&outputs[0]), Some(TABLE.to_vec()));
}

#[test]
fn night_light_fades_back_in_when_the_control_is_destroyed() {
    let (mut f, outputs, client, control) = first_of_two_held();

    f.client(client).destroy_gamma_control(control);
    f.roundtrip(client);

    assert_fades_back_in(&mut f, &outputs);
}

#[test]
fn night_light_fades_back_in_when_the_gamma_client_is_killed() {
    let (mut f, outputs, client, _) = first_of_two_held();

    f.disconnect(client);

    assert_fades_back_in(&mut f, &outputs);
}

#[test]
fn a_short_table_fails_the_control_and_night_light_fades_back_in() {
    let (mut f, outputs, client, control) = first_of_two_held();

    f.client(client).set_gamma(control, &TABLE[..11]);
    f.roundtrip(client);

    assert!(failed(&mut f, client, control));
    assert_fades_back_in(&mut f, &outputs);
}

#[test]
fn a_control_that_never_set_a_table_leaves_night_light_as_it_is() {
    let (mut f, _) = warm_outputs(1);
    let client = gamma_client(&mut f);
    let control = get_control(&mut f, client, 0);
    assert_eq!(applied(&f, 0), Some(4000));

    f.client(client).destroy_gamma_control(control);
    f.roundtrip(client);

    assert!(!held(&f, 0));
    assert_eq!(applied(&f, 0), Some(4000));
}

#[test]
fn an_unplugged_output_fails_its_control() {
    let (mut f, outputs, client, control) = first_of_two_held();

    f.unplug(&outputs[0]);
    f.roundtrip(client);

    assert!(failed(&mut f, client, control));
    // The output that's left is still Night Light's, and is free.
    assert_eq!(applied(&f, 0), Some(4000));
    assert!(!held(&f, 0));
    f.client(client).destroy_gamma_control(control);
    f.roundtrip(client);
}
