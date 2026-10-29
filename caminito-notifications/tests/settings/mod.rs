//! `org.mate.Caminito.notifications` and the apps' own `org.mate.Caminito.notifications.app`
//! as data/ has them, compiled by the test, over GSettings' memory backend.

use std::path::Path;
use std::process::Command;

use caminito_notifications::settings::{APP_SCHEMA, SCHEMA, Settings};
use gio::prelude::*;

/// The notifications' settings as data/ has them, compiled into `dir`, over a backend in
/// memory.
pub fn compiled(dir: &Path) -> Settings {
    let xml = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../data/org.mate.Caminito.gschema.xml"
    );
    std::fs::copy(xml, dir.join("org.mate.Caminito.gschema.xml")).unwrap();
    let status = Command::new("glib-compile-schemas")
        .arg(dir)
        .status()
        .unwrap();
    assert!(status.success());
    let source = gio::SettingsSchemaSource::from_directory(dir, None, false).unwrap();
    let schema = source.lookup(SCHEMA, false).unwrap();
    let global = gio::Settings::new_full(&schema, Some(&gio::memory_settings_backend_new()), None);
    Settings::new(global, source.lookup(APP_SCHEMA, false).unwrap())
}

/// Another `Settings` over the same keys as `settings`, as another process would have.
pub fn outside(settings: &Settings) -> Settings {
    let global = settings.global();
    let schema = global.settings_schema().unwrap();
    let outside = gio::Settings::new_full(&schema, global.backend().as_ref(), None);
    let app_schema = settings.app("").settings_schema().unwrap();
    Settings::new(outside, app_schema)
}
