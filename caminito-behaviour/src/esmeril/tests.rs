use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::testing::{outside, settings};

/// `org.mate.Marco.general` with only `reduced-resources`, compiled into `dir`, over a
/// backend in memory.
fn marco(dir: &std::path::Path) -> gio::Settings {
    let xml = r#"<schemalist>
      <schema id="org.mate.Marco.general" path="/org/mate/marco/general/">
        <key name="reduced-resources" type="b"><default>false</default></key>
      </schema>
    </schemalist>"#;
    std::fs::write(dir.join("mate.gschema.xml"), xml).unwrap();
    let status = std::process::Command::new("glib-compile-schemas")
        .arg(dir)
        .status()
        .unwrap();
    assert!(status.success());
    let source = gio::SettingsSchemaSource::from_directory(dir, None, false).unwrap();
    let schema = source.lookup("org.mate.Marco.general", false).unwrap();
    gio::Settings::new_full(&schema, Some(&gio::memory_settings_backend_new()), None)
}

#[test]
fn esmeril_is_on_at_the_strength_15_for_mate_terminal_until_it_is_set() {
    let dir = tempfile::tempdir().unwrap();
    let esmeril = Esmeril::new(settings(dir.path()), None);
    assert!(esmeril.is_on());
    assert_eq!(esmeril.strength(), 15);
    assert_eq!(esmeril.applications(), ["mate-terminal"]);
}

#[test]
fn each_control_writes_its_own_key() {
    let dir = tempfile::tempdir().unwrap();
    let settings = settings(dir.path());
    let esmeril = Esmeril::new(settings.clone(), None);
    let outside = outside(&settings);

    esmeril.set_on(false).unwrap();
    assert!(!outside.boolean("esmeril"));
    esmeril.set_strength(4).unwrap();
    assert_eq!(outside.uint("esmeril-strength"), 4);
    esmeril
        .set_applications(&["firefox".to_owned(), "Code".to_owned()])
        .unwrap();
    assert_eq!(
        outside.strv("esmeril-applications")[..],
        ["firefox", "Code"]
    );
    assert_eq!(esmeril.applications(), ["firefox", "Code"]);
}

#[test]
fn a_strength_outside_1_to_15_is_refused_and_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let esmeril = Esmeril::new(settings(dir.path()), None);
    assert!(esmeril.set_strength(0).is_err());
    assert!(esmeril.set_strength(16).is_err());
    assert_eq!(esmeril.strength(), 15);
}

#[test]
fn an_application_is_added_once_and_removed_by_its_app_id() {
    let dir = tempfile::tempdir().unwrap();
    let esmeril = Esmeril::new(settings(dir.path()), None);
    esmeril.set_listed("firefox", true).unwrap();
    esmeril.set_listed("firefox", true).unwrap();
    assert_eq!(esmeril.applications(), ["mate-terminal", "firefox"]);
    esmeril.set_listed("mate-terminal", false).unwrap();
    assert_eq!(esmeril.applications(), ["firefox"]);
    assert!(esmeril.is_listed("firefox"));
    assert!(!esmeril.is_listed("mate-terminal"));
}

#[test]
fn keys_set_from_outside_are_heard_and_other_keys_are_not() {
    // GSettings signals a change in the context its Settings were made in.
    let context = gio::glib::MainContext::new();
    context
        .with_thread_default(|| {
            let dir = tempfile::tempdir().unwrap();
            let settings = settings(dir.path());
            let esmeril = Esmeril::new(settings.clone(), None);
            let heard = Rc::new(RefCell::new(Vec::new()));
            esmeril.connect_changed({
                let heard = heard.clone();
                move |key| heard.borrow_mut().push(key)
            });

            let outside = outside(&settings);
            outside.set_int("workspace-layout-rows", 2).unwrap();
            outside.set_boolean("esmeril", false).unwrap();
            outside.set_uint("esmeril-strength", 3).unwrap();
            outside.set_strv("esmeril-applications", ["Code"]).unwrap();
            while context.iteration(false) {}

            assert_eq!(*heard.borrow(), [Key::On, Key::Strength, Key::Applications]);
        })
        .unwrap();
}

#[test]
fn reduced_resources_turns_esmeril_off_and_is_followed() {
    let context = gio::glib::MainContext::new();
    context
        .with_thread_default(|| {
            let dir = tempfile::tempdir().unwrap();
            let marco = marco(dir.path());
            let dir = tempfile::tempdir().unwrap();
            let esmeril = Esmeril::new(settings(dir.path()), Some(marco.clone()));
            assert!(!esmeril.reduced_resources());
            let heard = Rc::new(RefCell::new(Vec::new()));
            esmeril.connect_reduced_resources_changed({
                let heard = heard.clone();
                move |on| heard.borrow_mut().push(on)
            });

            marco.set_boolean("reduced-resources", true).unwrap();
            marco.set_boolean("reduced-resources", false).unwrap();
            while context.iteration(false) {}

            assert_eq!(*heard.borrow(), [true, false]);
        })
        .unwrap();
}

#[test]
fn without_marcos_schema_reduced_resources_are_off() {
    let dir = tempfile::tempdir().unwrap();
    let esmeril = Esmeril::new(settings(dir.path()), None);
    assert!(!esmeril.reduced_resources());
}
