//! Night Light (137, 138): `org.mate.Caminito`'s keys, the state dump, and the `NightLight`
//! property and `ToggleNightLight()`, and the sun schedule through a day on a fake clock.
//! Headless has no gamma tables (ADR 0023), so these read Night Light's state; the ramps,
//! the fades' arithmetic, the sun, the curve and the time zone are `night_light`'s unit
//! tests, and the tables on the tty are checked by hand.

use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use caminito_test_client::dbus::caminito_proxy;
use zbus::zvariant::{Dict, OwnedValue};

use super::fixture::{Fixture, test_schemas};
use crate::dbus::caminito::PATH;
use crate::night_light::ZoneFiles;
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
    // The fixed times' 07:00, with no location.
    assert!(i64::try_from(&property["next-sunrise"]).unwrap() > 0);
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
fn toggle_night_light_flips_the_key() {
    let mut f = Fixture::with_settings(vec![constant_schedule()]);
    let bus = f.start_dbus();

    toggle(&mut f, &bus);
    f.dispatch_until(|f| dump(f)["enabled"] == true);
    assert_eq!(target(&f), 4000);

    toggle(&mut f, &bus);
    f.dispatch_until(|f| dump(f)["enabled"] == false);
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

/// Listens on another thread for `org.mate.Caminito` at `bus` to signal `NightLight`, and
/// sends the temperature each signal said, in order.
fn listen(f: &mut Fixture, bus: &str) -> mpsc::Receiver<u32> {
    let bus = bus.to_owned();
    let (listening, is_listening) = mpsc::channel();
    let (signalled, signals) = mpsc::channel();
    thread::spawn(move || {
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
            .try_for_each(|temperature| signalled.send(temperature))
    });
    f.dispatch_until(|_| is_listening.try_recv().is_ok());
    signals
}

#[test]
fn the_property_is_signalled_when_the_target_changes() {
    let mut f = Fixture::with_settings(vec![constant_schedule()]);
    let bus = f.start_dbus();
    let signals = listen(&mut f, &bus);
    let mut heard = Vec::new();

    // Each change is heard before the next is made: the signal reads the property when
    // it's sent, so two quick changes could both say the second.
    f.set_setting(CAMINITO, "night-light-enabled", Value::Bool(true));
    f.dispatch_until_mut(|_| {
        heard.extend(signals.try_iter());
        !heard.is_empty()
    });
    f.set_setting(CAMINITO, "night-light-enabled", Value::Bool(false));
    f.dispatch_until_mut(|_| {
        heard.extend(signals.try_iter());
        heard.len() >= 2
    });
    assert_eq!(heard, [4000, 6500]);
}

/// 2026-12-21 at 03:00 UTC: midnight in Montevideo, the night before its longest day.
const MONTEVIDEO_MIDNIGHT: f64 = 1_797_822_000.0;

/// Montevideo's solar noon that day, 15:42:48 UTC.
const MONTEVIDEO_NOON: f64 = 1_797_867_768.0;

const HOUR: Duration = Duration::from_secs(3600);

fn stored_location() -> (&'static str, String, Value) {
    (
        CAMINITO,
        "night-light-last-location".into(),
        Value::MaybePair(Some((-34.9011, -56.1645))),
    )
}

fn source(f: &Fixture) -> String {
    dump(f)["location_source"].as_str().unwrap().to_owned()
}

#[test]
fn the_sun_schedule_follows_the_sun_through_a_day() {
    let mut f = Fixture::with_settings(vec![enabled(), stored_location()]);
    f.add_output((800, 600), 1.0);
    f.set_wall_clock(MONTEVIDEO_MIDNIGHT);
    // The jump to the fake clock fades; an hour on, it's long over.
    f.advance_clock(HOUR);

    assert_eq!(source(&f), "stored");
    assert_eq!(
        dump(&f)["location"],
        serde_json::json!([-34.9011, -56.1645])
    );
    let highest = dump(&f)["sun_highest"].as_f64().unwrap();
    assert!((78.0..79.0).contains(&highest), "{highest}");

    let mut day = Vec::new();
    for _ in 0..24 {
        assert_eq!(applied(&f), target(&f), "the minute's steps aren't faded");
        day.push((dump(&f)["sun_elevation"].as_f64().unwrap(), target(&f)));
        f.advance_clock(HOUR);
    }
    // Night until the sun passes −6° at about 07:58 UTC, warming up to about 15:42 and
    // cooling from there to night again.
    assert!(
        day[..4].iter().all(|(_, kelvin)| *kelvin == 3400),
        "{day:?}"
    );
    assert!(
        day[20..].iter().all(|(_, kelvin)| *kelvin == 3400),
        "{day:?}"
    );
    let noon = day
        .iter()
        .position(|(_, kelvin)| *kelvin > 5400)
        .expect("{day:?}");
    assert!(
        day[..=noon].is_sorted_by_key(|(_, kelvin)| *kelvin),
        "{day:?}"
    );
    assert!(
        day[noon + 1..].is_sorted_by_key(|(_, kelvin)| u64::MAX - kelvin),
        "{day:?}"
    );
    for (elevation, kelvin) in &day {
        assert!(*elevation >= -6.0 || *kelvin == 3400, "{day:?}");
        assert!(*elevation < -5.0 || *kelvin > 3400, "{day:?}");
    }

    f.set_wall_clock(MONTEVIDEO_NOON);
    assert_eq!(target(&f), 5500);
}

#[test]
fn a_jump_of_the_clock_fades() {
    let mut f = Fixture::with_settings(vec![enabled(), stored_location()]);
    f.add_output((800, 600), 1.0);
    f.set_wall_clock(MONTEVIDEO_MIDNIGHT);
    f.advance_clock(Duration::from_secs(2));
    assert_eq!(applied(&f), 3400);

    f.set_wall_clock(MONTEVIDEO_NOON);
    assert_eq!(target(&f), 5500);
    assert!(applied(&f) < 4000, "{}", applied(&f));
    f.advance_clock(Duration::from_secs(2));
    assert_eq!(applied(&f), 5500);
}

/// Makes `dir`'s `localtime` link to `target`, and has Caminito read it.
fn set_localtime(f: &mut Fixture, dir: &std::path::Path, target: &str) {
    let localtime = dir.join("localtime");
    let _ = std::fs::remove_file(&localtime);
    std::os::unix::fs::symlink(target, &localtime).unwrap();
    f.state.caminito.night_light.set_zone_files(ZoneFiles {
        localtime,
        ..ZoneFiles::default()
    });
}

#[test]
fn the_location_falls_back_as_its_sources_are_taken_away() {
    let mut f = Fixture::with_settings(vec![enabled(), stored_location()]);
    let bus = f.start_dbus();
    let dir = tempfile::tempdir().unwrap();
    set_localtime(&mut f, dir.path(), "/usr/share/zoneinfo/Europe/Amsterdam");
    f.set_wall_clock(MONTEVIDEO_MIDNIGHT);
    assert_eq!(source(&f), "stored");

    // With no stored location, Amsterdam's coordinates in the test `zone.tab`: at 03:00 UTC
    // its sun is down too.
    f.set_setting(
        CAMINITO,
        "night-light-last-location",
        Value::MaybePair(None),
    );
    f.dispatch_until(|f| source(f) == "time-zone");
    let location = &dump(&f)["location"];
    assert!(
        (location[0].as_f64().unwrap() - 52.3667).abs() < 1e-3,
        "{location}"
    );
    assert!(
        (location[1].as_f64().unwrap() - 4.9).abs() < 1e-3,
        "{location}"
    );
    assert_eq!(target(&f), 3400);
    let property = property(&mut f, &bus);
    assert_eq!(
        <&str>::try_from(&property["location-source"]).unwrap(),
        "time-zone"
    );
    // Amsterdam's sun climbs past −6° at about 07:08 UTC.
    let rise = i64::try_from(&property["next-sunrise"]).unwrap();
    let after = rise - MONTEVIDEO_MIDNIGHT as i64;
    assert!((4 * 3600..5 * 3600).contains(&after), "{after}");

    // Under `Etc/UTC` there are none: the fixed times, night at 03:00 and noon's at 13:00.
    set_localtime(&mut f, dir.path(), "/usr/share/zoneinfo/Etc/UTC");
    f.advance_clock(Duration::from_secs(60));
    assert_eq!(source(&f), "none");
    assert_eq!(dump(&f)["location"], serde_json::Value::Null);
    assert_eq!(dump(&f)["sun_elevation"], serde_json::Value::Null);
    assert_eq!(target(&f), 3400);
    // 07:00 UTC.
    let next_sunrise = dump(&f)["next_sunrise"].as_i64().unwrap();
    assert_eq!(next_sunrise, MONTEVIDEO_MIDNIGHT as i64 + 4 * 3600);
    f.advance_clock(10 * HOUR);
    assert_eq!(target(&f), 5500);
}
