//! The sun's position on `org.mate.Caminito` (215), for a dynamic wallpaper: the `Sun`
//! property and the state dump, with Night Light off, on a fake clock. The sums are
//! `night_light`'s unit tests.

use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use caminito_test_client::dbus::caminito_proxy;
use zbus::zvariant::{Dict, OwnedValue};

use super::fixture::Fixture;
use crate::dbus::caminito::PATH;
use crate::settings::Value;
use crate::settings::translate::CAMINITO;

/// 2026-12-21 at 03:00 UTC: midnight in Montevideo, the night before its longest day.
const MONTEVIDEO_MIDNIGHT: f64 = 1_797_822_000.0;

const HOUR: Duration = Duration::from_secs(3600);

fn stored_location() -> (&'static str, String, Value) {
    (
        CAMINITO,
        "night-light-last-location".into(),
        Value::MaybePair(Some((-34.9011, -56.1645))),
    )
}

/// The `Sun` property's elevation, azimuth and location source.
fn property(f: &mut Fixture, bus: &str) -> (f64, f64, String) {
    let bus = bus.to_owned();
    let sun: HashMap<String, OwnedValue> =
        f.dbus_call(move || caminito_proxy(&bus).unwrap().sun().unwrap());
    (
        f64::try_from(&sun["elevation"]).unwrap(),
        f64::try_from(&sun["azimuth"]).unwrap(),
        <&str>::try_from(&sun["location-source"])
            .unwrap()
            .to_owned(),
    )
}

/// The dump's elevation, azimuth and location source.
fn dumped(f: &Fixture) -> (f64, f64, String) {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    let sun = &dump["sun"];
    (
        sun["elevation"].as_f64().unwrap(),
        sun["azimuth"].as_f64().unwrap(),
        sun["location_source"].as_str().unwrap().to_owned(),
    )
}

#[track_caller]
fn assert_near(actual: f64, expected: f64, within: f64) {
    assert!(
        (actual - expected).abs() <= within,
        "{actual} isn't within {within} of {expected}"
    );
}

/// That the dump shows the property's `sun`, to the last bits serde_json's floats keep.
#[track_caller]
fn assert_dumped(f: &Fixture, (elevation, azimuth, source): (f64, f64, String)) {
    let dumped = dumped(f);
    assert_near(dumped.0, elevation, 1e-9);
    assert_near(dumped.1, azimuth, 1e-9);
    assert_eq!(dumped.2, source);
}

#[test]
fn the_sun_moves_with_the_clock_while_night_light_is_off() {
    let mut f = Fixture::with_settings(vec![stored_location()]);
    let bus = f.start_dbus();
    f.set_wall_clock(MONTEVIDEO_MIDNIGHT);

    // Night Light's key is off by default; the sun is worked out all the same.
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    assert_eq!(dump["night_light"]["enabled"], false);
    let (elevation, azimuth, source) = property(&mut f, &bus);
    assert_eq!(source, "stored");
    // Montevideo's sun at midnight is far below, under the south: in its summer it rises
    // south of east and goes round by the north. PyEphem 4.2's values, as `sun`'s tests'.
    assert_near(elevation, -30.800, 0.5);
    assert_near(azimuth, 191.368, 0.5);
    assert_dumped(&f, (elevation, azimuth, source));

    // 2026-12-21 at 08:00 UTC, the sun near civil twilight, south of east.
    f.advance_clock(5 * HOUR);
    let (elevation, azimuth, source) = property(&mut f, &bus);
    assert_eq!(source, "stored");
    assert_near(elevation, -5.643, 0.5);
    assert_near(azimuth, 123.794, 0.5);
    assert_dumped(&f, (elevation, azimuth, source));
}

#[test]
fn with_no_location_the_sun_is_the_fixed_times() {
    // No stored location, and the tests' `localtime` is missing: UTC, and no time zone.
    let mut f = Fixture::new();
    let bus = f.start_dbus();
    f.set_wall_clock(MONTEVIDEO_MIDNIGHT);

    // 03:00: below, and north-east, as though it rose at 07:00.
    let (elevation, azimuth, source) = property(&mut f, &bus);
    assert_eq!(source, "none");
    assert!(elevation < -6.0, "{elevation}");
    assert_near(azimuth, 30.0, 1e-6);

    // 13:00: its highest, due south.
    f.advance_clock(10 * HOUR);
    let (elevation, azimuth, _) = property(&mut f, &bus);
    assert_near(elevation, 45.0, 0.5);
    assert_near(azimuth, 180.0, 0.5);
}

/// Listens on another thread for `org.mate.Caminito` at `bus` to signal `Sun`, and sends
/// the elevation each signal said, in order.
fn listen(f: &mut Fixture, bus: &str) -> mpsc::Receiver<f64> {
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
                let sun = args.changed_properties().get("Sun")?;
                let sun = <&Dict>::try_from(sun).unwrap();
                sun.get::<_, f64>(&"elevation").unwrap()
            })
            .try_for_each(|elevation| signalled.send(elevation))
    });
    f.dispatch_until(|_| is_listening.try_recv().is_ok());
    signals
}

#[test]
fn the_sun_is_signalled_each_minute_it_moves() {
    let mut f = Fixture::with_settings(vec![stored_location()]);
    let bus = f.start_dbus();
    f.set_wall_clock(MONTEVIDEO_MIDNIGHT + 5.0 * HOUR.as_secs_f64());
    let signals = listen(&mut f, &bus);
    let mut heard = Vec::new();

    // Each minute's is heard before the next: the signal reads the property when it's sent.
    for minutes in 1..=2 {
        f.advance_clock(Duration::from_secs(60));
        f.dispatch_until_mut(|_| {
            heard.extend(signals.try_iter());
            heard.len() >= minutes
        });
    }
    // The sun climbs at dawn.
    assert!(heard[0] < heard[1], "{heard:?}");
}
