//! caminito-behaviour's library (CMN-184): what Workspace Behaviour's pages read and write,
//! with no toolkit in it. The window is the binary's.

#![forbid(unsafe_code)]

pub mod hot_corners;
pub mod key_remaps;
pub mod open_windows;
pub mod remap_templates;
pub mod system_shortcuts;

#[cfg(test)]
mod testing;

/// The schema the pages' keys are in.
pub const SCHEMA: &str = "org.mate.Caminito";

/// The installed `org.mate.Caminito`, if it's installed and compiled.
fn installed() -> Option<gio::Settings> {
    let source = gio::SettingsSchemaSource::default()?;
    source.lookup(SCHEMA, true)?;
    Some(gio::Settings::new(SCHEMA))
}
