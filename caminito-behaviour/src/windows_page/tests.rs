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

/// The page's switch.
fn switch(page: &gtk::Widget) -> gtk::Switch {
    let mut child = page.first_child();
    while let Some(each) = child {
        if let Ok(switch) = each.clone().downcast::<gtk::Switch>() {
            return switch;
        }
        child = each.next_sibling();
    }
    panic!("the page has a switch")
}

#[test]
fn the_switch_writes_the_key_and_follows_it() {
    on_gtk(|| {
        let context = glib::MainContext::default();
        let (_dir, settings) = settings();
        let outside = settings.clone();
        let page = build(Windows::new(settings));
        let switch = switch(&page);

        // The key's default: off.
        assert!(!switch.is_active());

        switch.set_active(true);
        assert!(outside.boolean("fullscreen-workspaces"));

        outside.set_boolean("fullscreen-workspaces", false).unwrap();
        while context.iteration(false) {}
        assert!(!switch.is_active());
    });
}
