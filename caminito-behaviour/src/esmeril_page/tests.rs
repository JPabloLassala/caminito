//! The page's widgets over `org.mate.Caminito` as data/ has it, in memory. They need a
//! display, so a test skips where GTK can't start (`gtk_testing`).

use super::*;
use crate::gtk_testing::on_gtk;

/// `org.mate.Caminito` as data/ has it, compiled into a directory of the test's own, over a
/// backend in memory.
fn settings() -> (tempfile::TempDir, gio::Settings) {
    let dir = tempfile::tempdir().unwrap();
    let xml = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../data/org.mate.Caminito.gschema.xml"
    );
    std::fs::copy(xml, dir.path().join("org.mate.Caminito.gschema.xml")).unwrap();
    let status = std::process::Command::new("glib-compile-schemas")
        .arg(dir.path())
        .status()
        .unwrap();
    assert!(status.success());
    let source = gio::SettingsSchemaSource::from_directory(dir.path(), None, false).unwrap();
    let schema = source.lookup(caminito_behaviour::SCHEMA, false).unwrap();
    let settings =
        gio::Settings::new_full(&schema, Some(&gio::memory_settings_backend_new()), None);
    (dir, settings)
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

fn find<T: IsA<gtk::Widget>>(widgets: &[gtk::Widget]) -> T {
    widgets
        .iter()
        .find_map(|widget| widget.clone().downcast::<T>().ok())
        .expect("the page has the widget")
}

fn check_buttons(widgets: &[gtk::Widget]) -> Vec<gtk::CheckButton> {
    widgets
        .iter()
        .filter_map(|widget| widget.clone().downcast::<gtk::CheckButton>().ok())
        .collect()
}

#[test]
fn the_controls_write_their_keys_and_follow_them() {
    on_gtk(|| {
        let context = glib::MainContext::default();
        let (_dir, settings) = settings();
        let outside = settings.clone();
        let page = build(Esmeril::new(settings, None));
        let widgets = descendants(&page);
        let switch: gtk::Switch = find(&widgets);
        let strength: gtk::Scale = find(&widgets);

        // The keys' defaults: on, at 15, with the listed application checked.
        assert!(switch.is_active());
        assert!(strength.is_sensitive());
        assert_eq!(strength.value(), 15.0);
        let apps = check_buttons(&widgets);
        assert_eq!(
            apps.iter()
                .filter(|check| check.is_active())
                .map(|check| check.label().unwrap())
                .collect::<Vec<_>>(),
            ["mate-terminal"]
        );

        // The controls write their keys, the slider greyed out while the switch is off.
        strength.set_value(6.0);
        assert_eq!(outside.uint("esmeril-strength"), 6);
        switch.set_active(false);
        assert!(!outside.boolean("esmeril"));
        assert!(!strength.is_sensitive());
        switch.set_active(true);
        assert!(strength.is_sensitive());
        apps[0].set_active(false);
        assert!(outside.strv("esmeril-applications").is_empty());

        // The controls follow the keys when they change elsewhere.
        outside.set_boolean("esmeril", false).unwrap();
        outside.set_uint("esmeril-strength", 2).unwrap();
        outside.set_strv("esmeril-applications", ["Code"]).unwrap();
        while context.iteration(false) {}
        assert!(!switch.is_active());
        assert!(!strength.is_sensitive());
        assert_eq!(strength.value(), 2.0);
        // The app id that was listed stays, unchecked, beside the new one.
        let apps = check_buttons(&descendants(&page));
        let state: Vec<_> = apps
            .iter()
            .map(|check| (check.label().unwrap().to_string(), check.is_active()))
            .collect();
        assert_eq!(
            state,
            [
                ("Code".to_owned(), true),
                ("mate-terminal".to_owned(), false)
            ]
        );
    });
}

#[test]
fn an_application_unchecked_stays_in_the_list_to_be_checked_again() {
    on_gtk(|| {
        let (_dir, settings) = settings();
        let outside = settings.clone();
        let page = build(Esmeril::new(settings, None));
        let apps = check_buttons(&descendants(&page));
        assert_eq!(apps.len(), 1);

        apps[0].set_active(false);
        let after = check_buttons(&descendants(&page));
        assert_eq!(after, apps, "a toggle keeps the same buttons");
        assert!(!after[0].is_active());

        after[0].set_active(true);
        assert_eq!(outside.strv("esmeril-applications")[..], ["mate-terminal"]);
    });
}

#[test]
fn the_page_opens_on_the_keys_as_they_are() {
    on_gtk(|| {
        let (_dir, settings) = settings();
        settings.set_boolean("esmeril", false).unwrap();
        settings.set_uint("esmeril-strength", 7).unwrap();
        settings.set_strv("esmeril-applications", ["Code"]).unwrap();
        let page = build(Esmeril::new(settings, None));
        let widgets = descendants(&page);
        let switch: gtk::Switch = find(&widgets);
        let strength: gtk::Scale = find(&widgets);
        assert!(!switch.is_active());
        assert!(!strength.is_sensitive());
        assert_eq!(strength.value(), 7.0);
        let apps = check_buttons(&widgets);
        assert_eq!(apps.len(), 1);
        assert!(apps[0].is_active());
    });
}
