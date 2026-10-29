//! The System shortcuts the Key remaps page marks (CMN-189): Caminito matches them before
//! any remap, so a rule whose combination is one never applies, and a target that is one
//! runs it (ADR 0038). They're read as Caminito reads its bindings (148): Marco's
//! `global-keybindings` and `window-keybindings`, and `org.mate.Caminito`'s Overview keys
//! (241, CMN-185) and Notifications keys (CMN-263). The accelerators other programs grab,
//! as mate-panel's Alt+F1, aren't in any key the page can read, and aren't marked.

use gio::prelude::*;

use crate::key_remaps::Combination;

#[cfg(test)]
mod tests;

/// Marco's schemas of bindings, every key of which is a System shortcut.
const MARCO: [&str; 2] = [
    "org.mate.Marco.global-keybindings",
    "org.mate.Marco.window-keybindings",
];

/// `org.mate.Caminito`'s keys that are System shortcuts.
const CAMINITO_KEYS: &[&str] = &[
    "overview",
    "application-overview",
    "notification-center",
    "toggle-do-not-disturb",
];

/// Each System shortcut's combination, with what it does.
#[derive(Debug, Default)]
pub struct SystemShortcuts {
    shortcuts: Vec<(Combination, String)>,
}

impl SystemShortcuts {
    /// The System shortcuts of each label and its accelerator, as Marco writes one. One that
    /// isn't a combination, as `disabled`, is none.
    pub fn from_accelerators(shortcuts: impl IntoIterator<Item = (String, String)>) -> Self {
        let shortcuts = shortcuts
            .into_iter()
            .filter_map(|(label, text)| Some((accelerator(&text)?, label)))
            .collect();
        SystemShortcuts { shortcuts }
    }

    /// Those of the installed schemas. A schema that isn't installed has none.
    pub fn installed() -> Self {
        let mut shortcuts = SystemShortcuts::default();
        let Some(source) = gio::SettingsSchemaSource::default() else {
            return shortcuts;
        };
        for id in MARCO {
            if let Some(schema) = source.lookup(id, true) {
                let keys: Vec<String> = schema
                    .list_keys()
                    .iter()
                    .map(|key| key.to_string())
                    .collect();
                let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
                shortcuts.add(&gio::Settings::new(id), &keys);
            }
        }
        if let Some(settings) = crate::installed() {
            shortcuts.add(&settings, CAMINITO_KEYS);
        }
        shortcuts
    }

    /// Adds those of `keys` of `settings`, each labelled by its key's summary.
    fn add(&mut self, settings: &gio::Settings, keys: &[&str]) {
        let schema = settings
            .settings_schema()
            .expect("a Settings made from a schema has one");
        let read = keys.iter().filter_map(|key| {
            let schema_key = schema.key(key);
            if schema_key.value_type() != *gio::glib::VariantTy::STRING {
                return None;
            }
            let label = schema_key
                .summary()
                .map_or_else(|| key.to_string(), Into::into);
            Some((label, settings.string(key).into()))
        });
        self.shortcuts
            .extend(SystemShortcuts::from_accelerators(read).shortcuts);
    }

    /// What `combination` does, if it's a System shortcut.
    pub fn named(&self, combination: &Combination) -> Option<&str> {
        let found = self.shortcuts.iter().find(|(each, _)| each == combination);
        found.map(|(_, label)| label.as_str())
    }
}

/// The combination of an accelerator as Marco writes one, `<Control><Alt>Left`, if it's one
/// Caminito binds: a key with modifiers among Ctrl, Alt, Shift and Super.
fn accelerator(text: &str) -> Option<Combination> {
    let mut rest = text.trim();
    let mut combination = Combination::new(false, false, false, false, "");
    while let Some(modifier) = rest.strip_prefix('<') {
        let (name, after) = modifier.split_once('>')?;
        match name.to_ascii_lowercase().as_str() {
            "control" | "ctrl" | "primary" => combination.ctrl = true,
            "alt" | "mod1" => combination.alt = true,
            "shift" => combination.shift = true,
            "super" | "mod4" => combination.super_ = true,
            _ => return None,
        }
        rest = after;
    }
    if rest.is_empty() || rest == "disabled" {
        return None;
    }
    Some(Combination {
        key: Combination::new(false, false, false, false, rest).key,
        ..combination
    })
}
