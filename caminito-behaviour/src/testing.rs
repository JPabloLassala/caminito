//! What every page's tests share: `org.mate.Caminito` as data/ has it, compiled by the test,
//! over GSettings' memory backend.

use std::path::Path;
use std::process::Command;

use gio::prelude::*;

/// `org.mate.Caminito` as data/ has it, compiled into `dir`, over a backend in memory.
pub(crate) fn settings(dir: &Path) -> gio::Settings {
    let schema = schema(dir);
    gio::Settings::new_full(&schema, Some(&gio::memory_settings_backend_new()), None)
}

/// Another `Settings` over the same keys as `settings`, as `gsettings` would have.
pub(crate) fn outside(settings: &gio::Settings) -> gio::Settings {
    let schema = settings.settings_schema().unwrap();
    gio::Settings::new_full(&schema, settings.backend().as_ref(), None)
}

/// `org.mate.Caminito` as data/ has it, compiled into `dir`.
fn schema(dir: &Path) -> gio::SettingsSchema {
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
    source.lookup(crate::SCHEMA, false).unwrap()
}
