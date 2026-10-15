//! The Night Light tab's state, through what the window does to it and reads of it.

use super::*;

/// 2026-10-15 at 22:00 UTC.
const NOW: i64 = 1_792_101_600;

fn tab() -> Tab {
    Tab::new(Settings::default(), None)
}

fn on() -> Tab {
    let mut tab = tab();
    tab.refresh(Settings {
        enabled: true,
        ..Settings::default()
    });
    tab
}

/// Hours and minutes in UTC, for `Pause::text`.
fn utc(time: i64) -> (u32, u32) {
    let minutes = time.rem_euclid(86_400) / 60;
    ((minutes / 60) as u32, (minutes % 60) as u32)
}

#[test]
fn the_settings_default_to_the_schemas_defaults() {
    assert_eq!(
        Settings::default(),
        Settings {
            enabled: false,
            schedule: Schedule::Sun,
            noon: 5500,
            night: 3400,
            constant: 4000,
            paused_until: 0,
        }
    );
}

#[test]
fn the_sun_schedule_shows_the_noon_and_night_sliders() {
    assert_eq!(
        tab().sliders(),
        vec![(Temperature::Noon, 5500), (Temperature::Night, 3400)]
    );
}

#[test]
fn the_constant_schedule_shows_the_constant_slider_alone() {
    let mut tab = tab();
    tab.set_schedule(Schedule::Constant);
    assert_eq!(tab.sliders(), vec![(Temperature::Constant, 4000)]);
}

#[test]
fn the_sliders_follow_a_schedule_changed_from_elsewhere() {
    let mut tab = tab();
    let changed = tab.refresh(Settings {
        schedule: Schedule::Constant,
        constant: 2700,
        ..Settings::default()
    });
    assert!(changed);
    assert_eq!(tab.sliders(), vec![(Temperature::Constant, 2700)]);
    assert!(!tab.refresh(tab.settings().clone()));
}

#[test]
fn each_change_is_the_write_of_its_key() {
    let mut tab = tab();
    assert_eq!(tab.set_enabled(true), Some(Write::Enabled(true)));
    assert_eq!(Write::Enabled(true).key(), "night-light-enabled");
    assert_eq!(
        tab.set_schedule(Schedule::Constant),
        Some(Write::Schedule(Schedule::Constant))
    );
    assert_eq!(
        Write::Schedule(Schedule::Constant).key(),
        "night-light-schedule"
    );
    assert_eq!(
        tab.set_temperature(Temperature::Constant, 3000),
        Some(Write::Temperature(Temperature::Constant, 3000))
    );
    let keys = [
        (Temperature::Noon, "night-light-noon-temperature"),
        (Temperature::Night, "night-light-night-temperature"),
        (Temperature::Constant, "night-light-constant-temperature"),
    ];
    for (which, key) in keys {
        assert_eq!(Write::Temperature(which, 3000).key(), key);
    }
    assert!(tab.settings().enabled);
    assert_eq!(tab.settings().schedule, Schedule::Constant);
    assert_eq!(tab.settings().constant, 3000);
}

#[test]
fn a_change_to_what_the_key_has_writes_nothing() {
    let mut tab = tab();
    assert_eq!(tab.set_enabled(false), None);
    assert_eq!(tab.set_schedule(Schedule::Sun), None);
    assert_eq!(tab.set_temperature(Temperature::Noon, 5500), None);
}

#[test]
fn a_temperature_is_kept_to_the_keys_range_in_the_sliders_steps() {
    let mut tab = tab();
    // 3400, which the key has.
    assert_eq!(tab.set_temperature(Temperature::Night, 3449), None);
    assert_eq!(
        tab.set_temperature(Temperature::Night, 3450),
        Some(Write::Temperature(Temperature::Night, 3500))
    );
    assert_eq!(
        tab.set_temperature(Temperature::Night, 200),
        Some(Write::Temperature(Temperature::Night, MIN_TEMPERATURE))
    );
    assert_eq!(
        tab.set_temperature(Temperature::Night, 9000),
        Some(Write::Temperature(Temperature::Night, MAX_TEMPERATURE))
    );
}

#[test]
fn a_temperature_reads_in_kelvin() {
    assert_eq!(kelvin(3400), "3400 K");
}

#[test]
fn with_no_pause_the_button_pauses_until_sunrise() {
    let pause = on().pause(NOW);
    assert_eq!(pause, Pause::Pause);
    assert_eq!(pause.label(), "Pause until sunrise");
    assert_eq!(pause.text(utc), None);
}

#[test]
fn while_paused_the_button_resumes_and_says_until_when() {
    let mut tab = on();
    tab.refresh(Settings {
        enabled: true,
        paused_until: NOW + 8 * 3600 + 42 * 60,
        ..Settings::default()
    });
    let pause = tab.pause(NOW);
    assert_eq!(
        pause,
        Pause::Resume {
            until: NOW + 8 * 3600 + 42 * 60
        }
    );
    assert_eq!(pause.label(), "Resume");
    assert_eq!(pause.text(utc).as_deref(), Some("Paused until 06:42"));
}

#[test]
fn a_pause_whose_time_has_passed_is_over() {
    let mut tab = on();
    tab.refresh(Settings {
        enabled: true,
        paused_until: NOW - 60,
        ..Settings::default()
    });
    assert_eq!(tab.pause(NOW), Pause::Pause);
}

#[test]
fn night_light_off_can_be_neither_paused_nor_resumed() {
    assert!(!tab().can_pause());
    assert!(on().can_pause());
}

#[test]
fn the_note_says_the_times_are_fixed_when_the_source_is_none() {
    let mut tab = tab();
    assert_eq!(tab.note(), None);
    assert!(tab.set_location_source(Some("none".to_owned())));
    assert_eq!(tab.note(), Some("Location unknown, using fixed times"));
    assert!(!tab.set_location_source(Some("none".to_owned())));
    for source in ["geoclue", "stored", "time-zone"] {
        tab.set_location_source(Some(source.to_owned()));
        assert_eq!(tab.note(), None, "{source}");
    }
    tab.set_location_source(None);
    assert_eq!(tab.note(), None);
}

#[test]
fn a_schedule_reads_from_and_writes_to_its_keys_nick() {
    for schedule in [Schedule::Sun, Schedule::Constant] {
        assert_eq!(Schedule::from_nick(schedule.nick()), Some(schedule));
    }
    assert_eq!(Schedule::Sun.nick(), "sun");
    assert_eq!(Schedule::Constant.nick(), "constant");
    assert_eq!(Schedule::from_nick("custom"), None);
}
