//! The page's widgets over `org.mate.screensaver` as data/ has it and the keys it uses of
//! `org.mate.session` and `org.mate.power-manager`, in memory. They need a display, so a test
//! skips where GTK can't start.

use super::*;
use crate::gtk_testing::on_gtk;

/// The page's three schemas, compiled into a directory of the test's own, over one backend
/// in memory, as `(dir, session, screensaver, power)`.
fn keys() -> (
    tempfile::TempDir,
    gio::Settings,
    gio::Settings,
    gio::Settings,
) {
    let dir = tempfile::tempdir().unwrap();
    let screensaver = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../data/org.mate.screensaver.gschema.xml"
    );
    std::fs::copy(
        screensaver,
        dir.path().join("org.mate.screensaver.gschema.xml"),
    )
    .unwrap();
    let mate = r#"<schemalist>
      <schema id="org.mate.session" path="/org/mate/desktop/session/">
        <key name="idle-delay" type="i"><default>5</default></key>
      </schema>
      <schema id="org.mate.power-manager" path="/org/mate/power-manager/">
        <key name="sleep-display-ac" type="i"><default>1800</default></key>
        <key name="sleep-display-battery" type="i"><default>600</default></key>
      </schema>
    </schemalist>"#;
    std::fs::write(dir.path().join("mate.gschema.xml"), mate).unwrap();
    let status = std::process::Command::new("glib-compile-schemas")
        .arg(dir.path())
        .status()
        .unwrap();
    assert!(status.success());
    let source = gio::SettingsSchemaSource::from_directory(dir.path(), None, false).unwrap();
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
    (dir, session, screensaver, power)
}

/// Every widget under `widget`, itself included.
fn descendants(widget: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut all = vec![widget.clone()];
    let mut child = widget.first_child();
    while let Some(each) = child {
        all.extend(descendants(&each));
        child = each.next_sibling();
    }
    all
}

fn all<T: IsA<gtk::Widget>>(widgets: &[gtk::Widget]) -> Vec<T> {
    widgets
        .iter()
        .filter_map(|widget| widget.clone().downcast::<T>().ok())
        .collect()
}

/// The label of `dropdown`'s selected entry.
fn shown(dropdown: &gtk::DropDown) -> String {
    dropdown
        .selected_item()
        .and_downcast::<gtk::StringObject>()
        .expect("the drop-down has a selection")
        .string()
        .into()
}

/// Selects the entry labelled `text` in `dropdown`.
fn choose(dropdown: &gtk::DropDown, text: &str) {
    let model = dropdown.model().unwrap();
    let place = (0..model.n_items())
        .find(|place| {
            model
                .item(*place)
                .and_downcast::<gtk::StringObject>()
                .is_some_and(|item| item.string() == text)
        })
        .expect("the drop-down offers the entry");
    dropdown.set_selected(place);
}

fn settle(context: &glib::MainContext) {
    while context.iteration(false) {}
}

#[test]
fn the_delays_write_their_keys_and_follow_them() {
    on_gtk(|| {
        let context = glib::MainContext::default();
        let (_dir, session, screensaver, power) = keys();
        let page = build(ScreenLocking::new(
            session.clone(),
            screensaver.clone(),
            Some(power),
        ));
        let widgets = descendants(&page);
        let dropdowns: Vec<gtk::DropDown> = all(&widgets);
        let (blank, lock) = (&dropdowns[0], &dropdowns[1]);
        let never = all::<gtk::Label>(&widgets)
            .into_iter()
            .find(|label| label.label().contains("never locks"))
            .unwrap();

        assert_eq!(shown(blank), "5 minutes");
        assert_eq!(shown(lock), "When it blanks");
        assert!(!never.is_visible());

        choose(blank, "15 minutes");
        choose(lock, "2 minutes after it blanks");
        assert_eq!(session.int("idle-delay"), 15);
        assert_eq!(screensaver.int("lock-delay"), 2);

        // A value set outside the list shows as itself, and stays until another is picked.
        session.set_int("idle-delay", 7).unwrap();
        settle(&context);
        assert_eq!(shown(blank), "7 minutes");
        assert_eq!(session.int("idle-delay"), 7);

        choose(blank, "Never");
        settle(&context);
        assert!(!screensaver.boolean("idle-activation-enabled"));
        assert_eq!(session.int("idle-delay"), 7);
        assert!(!lock.is_sensitive());
        assert!(never.is_visible());
        assert_eq!(shown(lock), "2 minutes after it blanks");

        screensaver
            .set_boolean("idle-activation-enabled", true)
            .unwrap();
        settle(&context);
        assert_eq!(shown(blank), "7 minutes");
        assert!(lock.is_sensitive());
        assert!(!never.is_visible());
    });
}

#[test]
fn a_clock_is_set_by_preset_or_typed_and_follows_its_key() {
    on_gtk(|| {
        let context = glib::MainContext::default();
        let (_dir, session, screensaver, power) = keys();
        let page = build(ScreenLocking::new(
            session,
            screensaver.clone(),
            Some(power),
        ));
        let widgets = descendants(&page);
        let dropdowns: Vec<gtk::DropDown> = all(&widgets);
        let (time, date) = (&dropdowns[2], &dropdowns[3]);
        let entries: Vec<gtk::Entry> = all(&widgets);
        let (time_entry, date_entry) = (&entries[0], &entries[1]);
        let field = |entry: &gtk::Entry| entry.parent().unwrap();

        assert_eq!(shown(time), "Locale");
        assert!(!field(time_entry).is_visible());

        choose(time, "24-hour");
        assert_eq!(screensaver.string("lock-dialog-time-format"), "%R");

        choose(date, "Custom");
        assert!(field(date_entry).is_visible());
        date_entry.set_text("%d/%m/%Y");
        assert_eq!(screensaver.string("lock-dialog-date-format"), "%d/%m/%Y");
        // A format typed that is a preset's stays in Custom while it's typed in.
        date_entry.set_text("%F");
        settle(&context);
        assert_eq!(screensaver.string("lock-dialog-date-format"), "%F");
        assert_eq!(shown(date), "Custom");
        // One GLib refuses isn't written.
        date_entry.set_text("%F %Q");
        assert_eq!(screensaver.string("lock-dialog-date-format"), "%F");

        screensaver
            .set_string("lock-dialog-time-format", "%H.%M")
            .unwrap();
        settle(&context);
        assert_eq!(shown(time), "Custom");
        assert_eq!(time_entry.text(), "%H.%M");
        assert!(field(time_entry).is_visible());

        screensaver
            .set_string("lock-dialog-time-format", "locale")
            .unwrap();
        settle(&context);
        assert_eq!(shown(time), "Locale");
        assert!(!field(time_entry).is_visible());
    });
}

#[test]
fn the_power_line_follows_its_keys_and_goes_without_its_schema() {
    on_gtk(|| {
        let context = glib::MainContext::default();
        let (_dir, session, screensaver, power) = keys();
        let line = |page: &gtk::Widget| {
            all::<gtk::Label>(&descendants(page))
                .into_iter()
                .find(|label| label.label().starts_with("The screens"))
        };

        let page = build(ScreenLocking::new(
            session.clone(),
            screensaver.clone(),
            Some(power.clone()),
        ));
        let label = line(&page).unwrap();
        assert_eq!(
            label.label(),
            "The screens turn off after 30 minutes, or 10 minutes on battery."
        );
        power.set_int("sleep-display-battery", 1800).unwrap();
        settle(&context);
        assert_eq!(label.label(), "The screens turn off after 30 minutes.");

        let page = build(ScreenLocking::new(session, screensaver, None));
        assert!(line(&page).is_none());
    });
}
