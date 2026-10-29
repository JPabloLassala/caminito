//! `org.mate.Caminito.notifications` as data/ has it, compiled by the test, over GSettings'
//! memory backend.

use std::path::Path;
use std::process::Command;

use gio::prelude::*;

const SCHEMA: &str = "org.mate.Caminito.notifications";

/// The notifications' settings as data/ has them, compiled into `dir`, over a backend in
/// memory.
pub fn compiled(dir: &Path) -> gio::Settings {
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
    gio::Settings::new_full(&schema, Some(&gio::memory_settings_backend_new()), None)
}

/// Another `Settings` over the same keys as `settings`, as another process would have.
pub fn outside(settings: &gio::Settings) -> gio::Settings {
    let schema = settings.settings_schema().unwrap();
    gio::Settings::new_full(&schema, settings.backend().as_ref(), None)
}
