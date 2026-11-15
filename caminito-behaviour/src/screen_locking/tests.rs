use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use super::*;

/// `org.mate.screensaver` as data/ has it, and the keys the page uses of `org.mate.session`
/// and `org.mate.power-manager`, which data/ doesn't have, compiled into `dir`.
fn source(dir: &Path) -> gio::SettingsSchemaSource {
    let screensaver = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../data/org.mate.screensaver.gschema.xml"
    );
    std::fs::copy(screensaver, dir.join("org.mate.screensaver.gschema.xml")).unwrap();
    let mate = r#"<schemalist>
      <schema id="org.mate.session" path="/org/mate/desktop/session/">
        <key name="idle-delay" type="i"><default>5</default></key>
      </schema>
      <schema id="org.mate.power-manager" path="/org/mate/power-manager/">
        <key name="sleep-display-ac" type="i"><default>1800</default></key>
        <key name="sleep-display-battery" type="i"><default>600</default></key>
      </schema>
    </schemalist>"#;
    std::fs::write(dir.join("mate.gschema.xml"), mate).unwrap();
    let status = std::process::Command::new("glib-compile-schemas")
        .arg(dir)
        .status()
        .unwrap();
    assert!(status.success());
    gio::SettingsSchemaSource::from_directory(dir, None, false).unwrap()
}

/// The three schemas over one backend in memory, as a session has them.
struct Keys {
    source: gio::SettingsSchemaSource,
    backend: gio::SettingsBackend,
    session: gio::Settings,
    screensaver: gio::Settings,
    power: gio::Settings,
}

impl Keys {
    fn new(dir: &Path) -> Self {
        let source = source(dir);
        let backend = gio::memory_settings_backend_new();
        let open = |id| {
            let schema = source.lookup(id, false).unwrap();
            gio::Settings::new_full(&schema, Some(&backend), None)
        };
        let (session, screensaver, power) = (
            open("org.mate.session"),
            open("org.mate.screensaver"),
            open("org.mate.power-manager"),
        );
        Keys {
            source,
            backend,
            session,
            screensaver,
            power,
        }
    }

    /// Another `Settings` of `id` over the same keys, as `gsettings` would have.
    fn outside(&self, id: &str) -> gio::Settings {
        let schema = self.source.lookup(id, false).unwrap();
        gio::Settings::new_full(&schema, Some(&self.backend), None)
    }

    fn screen_locking(&self) -> ScreenLocking {
        ScreenLocking::new(
            self.session.clone(),
            self.screensaver.clone(),
            Some(self.power.clone()),
        )
    }
}

#[test]
fn the_blank_delay_is_five_minutes_until_it_is_set() {
    let dir = tempfile::tempdir().unwrap();
    let keys = Keys::new(dir.path());
    assert_eq!(keys.screen_locking().blank_delay(), Delay::Minutes(5));
}

#[test]
fn a_blank_delay_in_minutes_is_written_and_turns_blanking_on() {
    let dir = tempfile::tempdir().unwrap();
    let keys = Keys::new(dir.path());
    keys.screensaver
        .set_boolean("idle-activation-enabled", false)
        .unwrap();
    keys.screen_locking()
        .set_blank_delay(Delay::Minutes(15))
        .unwrap();
    assert_eq!(keys.session.int("idle-delay"), 15);
    assert!(keys.screensaver.boolean("idle-activation-enabled"));
}

#[test]
fn a_blank_delay_of_never_turns_blanking_off_and_keeps_the_minutes() {
    let dir = tempfile::tempdir().unwrap();
    let keys = Keys::new(dir.path());
    keys.session.set_int("idle-delay", 10).unwrap();
    let screen_locking = keys.screen_locking();
    screen_locking.set_blank_delay(Delay::Never).unwrap();
    assert!(!keys.screensaver.boolean("idle-activation-enabled"));
    assert_eq!(keys.session.int("idle-delay"), 10);
    assert_eq!(screen_locking.blank_delay(), Delay::Never);
}

#[test]
fn the_lock_comes_with_the_blank_until_it_is_set() {
    let dir = tempfile::tempdir().unwrap();
    let keys = Keys::new(dir.path());
    assert_eq!(keys.screen_locking().lock_delay(), Delay::Minutes(0));
}

#[test]
fn a_lock_delay_in_minutes_is_written_and_turns_the_lock_on() {
    let dir = tempfile::tempdir().unwrap();
    let keys = Keys::new(dir.path());
    keys.screensaver.set_boolean("lock-enabled", false).unwrap();
    keys.screen_locking()
        .set_lock_delay(Delay::Minutes(2))
        .unwrap();
    assert_eq!(keys.screensaver.int("lock-delay"), 2);
    assert!(keys.screensaver.boolean("lock-enabled"));
}

#[test]
fn a_lock_delay_of_never_turns_the_lock_off_and_keeps_the_minutes() {
    let dir = tempfile::tempdir().unwrap();
    let keys = Keys::new(dir.path());
    keys.screensaver.set_int("lock-delay", 5).unwrap();
    let screen_locking = keys.screen_locking();
    screen_locking.set_lock_delay(Delay::Never).unwrap();
    assert!(!keys.screensaver.boolean("lock-enabled"));
    assert_eq!(keys.screensaver.int("lock-delay"), 5);
    assert_eq!(screen_locking.lock_delay(), Delay::Never);
}

#[test]
fn the_blank_delays_offered_end_with_never() {
    let minutes = [1, 2, 3, 5, 10, 15, 30, 60].map(Delay::Minutes);
    let choices = blank_choices(Delay::Minutes(5));
    assert_eq!(choices[..8], minutes);
    assert_eq!(choices[8..], [Delay::Never]);
}

#[test]
fn a_blank_delay_set_outside_the_list_is_offered_in_its_place() {
    let choices = blank_choices(Delay::Minutes(7));
    assert_eq!(
        choices,
        [1, 2, 3, 5, 7, 10, 15, 30, 60]
            .map(Delay::Minutes)
            .into_iter()
            .chain([Delay::Never])
            .collect::<Vec<_>>()
    );
    assert_eq!(blank_choices(Delay::Minutes(120))[9], Delay::Never);
    assert_eq!(blank_choices(Delay::Minutes(120))[8], Delay::Minutes(120));
}

#[test]
fn the_lock_delays_offered_start_with_the_blank_and_take_one_set_outside() {
    let offered = |current| lock_choices(current);
    assert_eq!(
        offered(Delay::Never),
        [0, 1, 2, 5, 15, 30]
            .map(Delay::Minutes)
            .into_iter()
            .chain([Delay::Never])
            .collect::<Vec<_>>()
    );
    assert_eq!(offered(Delay::Minutes(3))[3], Delay::Minutes(3));
    assert_eq!(offered(Delay::Minutes(3)).len(), 8);
}

#[test]
fn the_delays_read_as_the_page_words_them() {
    assert_eq!(Delay::Minutes(1).blank_label(), "1 minute");
    assert_eq!(Delay::Minutes(60).blank_label(), "60 minutes");
    assert_eq!(Delay::Never.blank_label(), "Never");
    assert_eq!(Delay::Minutes(0).lock_label(), "When it blanks");
    assert_eq!(Delay::Minutes(1).lock_label(), "1 minute after it blanks");
    assert_eq!(
        Delay::Minutes(15).lock_label(),
        "15 minutes after it blanks"
    );
    assert_eq!(Delay::Never.lock_label(), "Never");
}

#[test]
fn each_clock_is_the_locales_until_it_is_set() {
    let dir = tempfile::tempdir().unwrap();
    let keys = Keys::new(dir.path());
    let screen_locking = keys.screen_locking();
    for clock in Clock::ALL {
        assert_eq!(screen_locking.format(clock), "locale", "{clock:?}");
        assert_eq!(clock.preset(&screen_locking.format(clock)), Some(0));
    }
}

#[test]
fn a_clock_format_is_written_to_its_own_key() {
    let dir = tempfile::tempdir().unwrap();
    let keys = Keys::new(dir.path());
    let screen_locking = keys.screen_locking();
    screen_locking.set_format(Clock::Time, "%R").unwrap();
    screen_locking.set_format(Clock::Date, "%F").unwrap();
    assert_eq!(keys.screensaver.string("lock-dialog-time-format"), "%R");
    assert_eq!(keys.screensaver.string("lock-dialog-date-format"), "%F");
}

#[test]
fn a_format_that_is_no_preset_reads_as_custom() {
    assert_eq!(Clock::Time.preset("%R"), Some(1));
    assert_eq!(Clock::Time.preset("%I:%M %p"), Some(2));
    assert_eq!(Clock::Date.preset("%F"), Some(1));
    assert_eq!(Clock::Time.preset("%H.%M"), None);
    assert_eq!(Clock::Date.preset("%R"), None);
}

#[test]
fn a_custom_format_is_previewed_and_one_glib_refuses_has_none() {
    let moment = gio::glib::DateTime::from_local(2026, 11, 15, 21, 5, 0.0).unwrap();
    assert_eq!(
        preview("%F %R", &moment).as_deref(),
        Some("2026-11-15 21:05")
    );
    assert_eq!(preview("%Q", &moment), None);
}

#[test]
fn the_power_line_says_when_the_screens_turn_off() {
    let dir = tempfile::tempdir().unwrap();
    let keys = Keys::new(dir.path());
    let screen_locking = keys.screen_locking();
    let line = |ac, battery| {
        keys.power.set_int("sleep-display-ac", ac).unwrap();
        keys.power
            .set_int("sleep-display-battery", battery)
            .unwrap();
        screen_locking.power_off().unwrap()
    };
    assert_eq!(
        line(1800, 600),
        "The screens turn off after 30 minutes, or 10 minutes on battery."
    );
    assert_eq!(line(3600, 3600), "The screens turn off after 1 hour.");
    assert_eq!(line(0, 0), "The screens never turn off.");
    assert_eq!(
        line(0, 60),
        "The screens turn off after 1 minute on battery, and never otherwise."
    );
    assert_eq!(
        line(7200, 0),
        "The screens turn off after 2 hours, and never on battery."
    );
    assert_eq!(line(90, 90), "The screens turn off after 90 seconds.");
}

#[test]
fn without_powers_schema_there_is_no_power_line() {
    let dir = tempfile::tempdir().unwrap();
    let keys = Keys::new(dir.path());
    let screen_locking = ScreenLocking::new(keys.session.clone(), keys.screensaver.clone(), None);
    assert_eq!(screen_locking.power_off(), None);
}

#[test]
fn keys_set_from_outside_are_heard() {
    // GSettings signals a change in the context its Settings were made in.
    let context = gio::glib::MainContext::new();
    context
        .with_thread_default(|| {
            let dir = tempfile::tempdir().unwrap();
            let keys = Keys::new(dir.path());
            let screen_locking = keys.screen_locking();
            let heard = Rc::new(RefCell::new(Vec::<String>::new()));
            let hear = |what: &'static str| {
                let heard = heard.clone();
                move |value: String| heard.borrow_mut().push(format!("{what} {value}"))
            };
            screen_locking.connect_blank_delay_changed({
                let hear = hear("blank");
                move |delay| hear(delay.blank_label())
            });
            screen_locking.connect_lock_delay_changed({
                let hear = hear("lock");
                move |delay| hear(delay.lock_label())
            });
            screen_locking.connect_format_changed({
                let hear = hear("format");
                move |clock, format| hear(format!("{clock:?} {format}"))
            });
            screen_locking.connect_power_off_changed(hear("power"));

            let session = keys.outside("org.mate.session");
            let screensaver = keys.outside("org.mate.screensaver");
            let power = keys.outside("org.mate.power-manager");
            session.set_int("idle-delay", 10).unwrap();
            while context.iteration(false) {}
            screensaver
                .set_boolean("idle-activation-enabled", false)
                .unwrap();
            while context.iteration(false) {}
            screensaver.set_int("lock-delay", 2).unwrap();
            while context.iteration(false) {}
            screensaver.set_boolean("lock-enabled", false).unwrap();
            while context.iteration(false) {}
            screensaver
                .set_string("lock-dialog-date-format", "%F")
                .unwrap();
            while context.iteration(false) {}
            power.set_int("sleep-display-ac", 600).unwrap();
            while context.iteration(false) {}
            screensaver.set_string("lock-dialog-theme", "red").unwrap();
            while context.iteration(false) {}

            assert_eq!(
                *heard.borrow(),
                [
                    "blank 10 minutes",
                    "blank Never",
                    "lock 2 minutes after it blanks",
                    "lock Never",
                    "format Date %F",
                    "power The screens turn off after 10 minutes.",
                ]
            );
        })
        .unwrap();
}
