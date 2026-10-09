//! Night Light (137): `org.mate.Caminito`'s keys, the state dump, and the `NightLight`
//! property and `ToggleNightLight()`. Headless has no gamma tables (ADR 0023), so these read
//! Night Light's state; the ramps and the fades' arithmetic are `night_light`'s unit tests,
//! and the tables on the tty are checked by hand.

use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use caminito_test_client::dbus::caminito_proxy;
use zbus::zvariant::{Dict, OwnedValue};

use super::fixture::{Fixture, test_schemas};
use crate::dbus::caminito::PATH;
use crate::settings::Value;
use crate::settings::translate::CAMINITO;

fn dump(f: &Fixture) -> serde_json::Value {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    dump["night_light"].clone()
}

fn target(f: &Fixture) -> u64 {
    dump(f)["target_temperature"].as_u64().unwrap()
}

/// The first output's temperature now, from the dump.
fn applied(f: &Fixture) -> u64 {
    dump(f)["outputs"][0]["temperature"].as_u64().unwrap()
}

fn property(f: &mut Fixture, bus: &str) -> HashMap<String, OwnedValue> {
    let bus = bus.to_owned();
    f.dbus_call(move || caminito_proxy(&bus).unwrap().night_light().unwrap())
}

fn property_temperature(f: &mut Fixture, bus: &str) -> u32 {
    u32::try_from(&property(f, bus)["temperature"]).unwrap()
}

fn toggle(f: &mut Fixture, bus: &str) {
    let bus = bus.to_owned();
    f.dbus_call(move || caminito_proxy(&bus).unwrap().toggle_night_light().unwrap());
}

fn constant_schedule() -> (&'static str, String, Value) {
    (
        CAMINITO,
        "night-light-schedule".into(),
        Value::Str("constant".into()),
    )
}

fn enabled() -> (&'static str, String, Value) {
    (CAMINITO, "night-light-enabled".into(), Value::Bool(true))
}

#[test]
fn caminitos_schema_has_night_lights_seven_keys() {
    let source = gio::SettingsSchemaSource::from_directory(test_schemas(), None, false).unwrap();
    let schema = source.lookup(CAMINITO, false).unwrap();
    let key = |name: &str| {
        let key = schema.key(name);
        (
            key.value_type().as_str().to_owned(),
            key.default_value().print(false).to_string(),
            key.range().print(false).to_string(),
        )
    };
    let range = "('range', <(uint32 1000, uint32 6500)>)";
    let table = [
        ("night-light-enabled", "b", "false", "('type', <@ab []>)"),
        (
            "night-light-schedule",
            "s",
            "'sun'",
            "('enum', <['sun', 'constant']>)",
        ),
        ("night-light-noon-temperature", "u", "5500", range),
        ("night-light-night-temperature", "u", "3400", range),
        ("night-light-constant-temperature", "u", "4000", range),
        ("night-light-paused-until", "x", "0", "('type', <@ax []>)"),
        (
            "night-light-last-location",
            "m(dd)",
            "nothing",
            "('type', <@am(dd) []>)",
        ),
    ];
    for (name, value_type, default, range) in table {
        assert_eq!(
            key(name),
            (value_type.into(), default.into(), range.into()),
            "{name}"
        );
    }
}

#[test]
fn night_light_is_off_and_neutral_by_default() {
    let mut f = Fixture::new();
    f.add_output((800, 600), 1.0);
    let bus = f.start_dbus();

    let night_light = dump(&f);
    assert_eq!(night_light["enabled"], false);
    assert_eq!(night_light["schedule"], "sun");
    assert_eq!(night_light["location_source"], "none");
    assert_eq!(target(&f), 6500);
    assert_eq!(applied(&f), 6500);
    let property = property(&mut f, &bus);
    assert_eq!(u32::try_from(&property["temperature"]).unwrap(), 6500);
    assert_eq!(
        <&str>::try_from(&property["location-source"]).unwrap(),
        "none"
    );
    assert_eq!(i64::try_from(&property["next-sunrise"]).unwrap(), 0);
}

#[test]
fn the_constant_schedule_sets_the_target_in_the_dump_and_the_property() {
    let mut f = Fixture::new();
    f.add_output((800, 600), 1.0);
    let bus = f.start_dbus();

    f.set_setting(
        CAMINITO,
        "night-light-schedule",
        Value::Str("constant".into()),
    );
    f.set_setting(CAMINITO, "night-light-enabled", Value::Bool(true));
    f.dispatch_until(|f| target(f) == 4000);
    assert_eq!(property_temperature(&mut f, &bus), 4000);

    f.set_setting(
        CAMINITO,
        "night-light-constant-temperature",
        Value::Uint(3000),
    );
    f.dispatch_until(|f| target(f) == 3000);
    assert_eq!(property_temperature(&mut f, &bus), 3000);

    f.set_setting(CAMINITO, "night-light-enabled", Value::Bool(false));
    f.dispatch_until(|f| target(f) == 6500);
    assert_eq!(property_temperature(&mut f, &bus), 6500);
}

#[test]
fn the_sun_schedule_is_neutral_until_it_is_worked_out() {
    let f = Fixture::with_settings(vec![enabled()]);
    assert_eq!(dump(&f)["enabled"], true);
    assert_eq!(target(&f), 6500);
}

#[test]
fn toggle_night_light_flips_the_key() {
    let mut f = Fixture::with_settings(vec![constant_schedule()]);
    let bus = f.start_dbus();

    toggle(&mut f, &bus);
    f.dispatch_until(|f| f.state.caminito.settings.night_light.enabled);
    assert_eq!(target(&f), 4000);

    toggle(&mut f, &bus);
    f.dispatch_until(|f| !f.state.caminito.settings.night_light.enabled);
    assert_eq!(target(&f), 6500);
}

#[test]
fn at_the_start_the_outputs_have_the_target_at_once() {
    let mut f = Fixture::with_settings(vec![enabled(), constant_schedule()]);
    f.add_output((800, 600), 1.0);
    f.dispatch();
    assert_eq!(applied(&f), 4000);
}

#[test]
fn the_outputs_fade_to_the_target_over_two_seconds() {
    let mut f = Fixture::with_settings(vec![constant_schedule()]);
    f.add_output((800, 600), 1.0);
    f.set_setting(CAMINITO, "night-light-enabled", Value::Bool(true));
    f.dispatch_until(|f| target(f) == 4000);
    // Only as long as the dispatches took has passed.
    assert!(applied(&f) > 6000, "{}", applied(&f));

    f.advance_clock(Duration::from_secs(1));
    let halfway = applied(&f);
    assert!((4500..5500).contains(&halfway), "{halfway}");

    f.advance_clock(Duration::from_secs(1));
    assert_eq!(applied(&f), 4000);
}

/// Listens on another thread for `org.mate.Caminito` at `bus` to signal `NightLight`
/// `count` times, and returns a handle to the temperatures each signal said, in order.
fn listen(f: &mut Fixture, bus: &str, count: usize) -> thread::JoinHandle<Vec<u32>> {
    let bus = bus.to_owned();
    let (listening, is_listening) = mpsc::channel();
    let listener = thread::spawn(move || {
        let connection = caminito_test_client::dbus::session().unwrap();
        let properties = zbus::blocking::fdo::PropertiesProxy::builder(&connection)
            .destination(bus)
            .unwrap()
            .path(PATH)
            .unwrap()
            .build()
            .unwrap();
        let changes = properties.receive_properties_changed().unwrap();
        listening.send(()).unwrap();
        changes
            .filter_map(|change| {
                let args = change.args().unwrap();
                let night_light = args.changed_properties().get("NightLight")?;
                let night_light = <&Dict>::try_from(night_light).unwrap();
                night_light.get::<_, u32>(&"temperature").unwrap()
            })
            .take(count)
            .collect()
    });
    f.dispatch_until(|_| is_listening.try_recv().is_ok());
    listener
}

#[test]
fn the_property_is_signalled_when_the_target_changes() {
    let mut f = Fixture::with_settings(vec![constant_schedule()]);
    let bus = f.start_dbus();
    let listener = listen(&mut f, &bus, 2);

    f.set_setting(CAMINITO, "night-light-enabled", Value::Bool(true));
    f.dispatch_until(|f| target(f) == 4000);
    f.set_setting(CAMINITO, "night-light-enabled", Value::Bool(false));

    f.dispatch_until(|_| listener.is_finished());
    assert_eq!(listener.join().unwrap(), [4000, 6500]);
}
