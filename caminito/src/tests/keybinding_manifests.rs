//! The `50-caminito-*.xml` keybinding manifests (05, 101), which tell the control center's
//! Keyboard Shortcuts dialog what Caminito binds. They need no fixture: they're data, checked
//! against Marco's schema, whose keys they point at, and against the actions Caminito binds
//! (173). That schema has no key for an action Caminito never implements (05), so a manifest
//! that passes lists none. The Overview's manifest points at Caminito's own schema, which
//! has its key (241).

use std::fs;

use crate::input::bindings::bound_keys;
use crate::settings::translate::CAMINITO;

const MANIFESTS: [&str; 4] = [
    "50-caminito-desktop-key.xml",
    "50-caminito-global-key.xml",
    "50-caminito-overview-key.xml",
    "50-caminito-window-key.xml",
];

/// The value of `name="…"` in an element's text.
fn attribute<'a>(element: &'a str, name: &str) -> Option<&'a str> {
    let opening = format!(" {name}=\"");
    let start = element.find(&opening)? + opening.len();
    let len = element[start..].find('"')?;
    Some(&element[start..start + len])
}

/// The names of the keys of one schema: one of the vendored `org.mate.marco.gschema.xml`,
/// or Caminito's own.
pub(super) fn schema_keys(id: &str) -> Vec<String> {
    let path = if id == CAMINITO {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../data/org.mate.Caminito.gschema.xml"
        )
    } else {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/schemas/org.mate.marco.gschema.xml"
        )
    };
    let schemas = fs::read_to_string(path).unwrap();
    let start = schemas
        .find(&format!("<schema id=\"{id}\""))
        .unwrap_or_else(|| panic!("{path} has no schema {id}"));
    let schema = &schemas[start..];
    let schema = &schema[..schema.find("</schema>").unwrap()];
    schema
        .split("<key ")
        .skip(1)
        .map(|key| attribute(&format!(" {key}"), "name").unwrap().to_owned())
        .collect()
}

#[test]
fn each_manifest_is_caminitos_and_names_only_keys_its_schema_has() {
    for manifest in MANIFESTS {
        let path = format!(
            "{}/../data/keybindings/{manifest}",
            env!("CARGO_MANIFEST_DIR")
        );
        let text = fs::read_to_string(&path).unwrap_or_else(|err| panic!("{path}: {err}"));
        let mut elements = text.split('<').filter(|e| e.starts_with("KeyListEntr"));

        let list = elements.next().unwrap();
        assert!(list.starts_with("KeyListEntries "), "{manifest}");
        assert_eq!(attribute(list, "wm_name"), Some("Caminito"), "{manifest}");
        // Marco's catalogue translates the names, and Caminito depends on marco.
        assert_eq!(attribute(list, "package"), Some("marco"), "{manifest}");
        let schema = attribute(list, "schema").unwrap();
        assert!(
            schema.starts_with("org.mate.Marco.") || schema == CAMINITO,
            "{manifest}: {schema}"
        );
        let keys = schema_keys(schema);

        let mut entries = 0;
        for entry in elements {
            let name = attribute(entry, "name").unwrap();
            assert!(
                keys.iter().any(|key| key == name),
                "{manifest}: {schema} has no key {name}"
            );
            // An entry shown only for some value of another key names that key's schema.
            if let Some(key) = attribute(entry, "key") {
                let schema = attribute(entry, "schema").unwrap();
                assert!(
                    schema_keys(schema).iter().any(|k| k == key),
                    "{manifest}: {schema} has no key {key}"
                );
            }
            entries += 1;
        }
        assert!(entries > 0, "{manifest} lists no action");
    }
}

/// The schema and name of every entry of the three manifests.
fn entries() -> Vec<(String, String)> {
    let mut entries = Vec::new();
    for manifest in MANIFESTS {
        let path = format!(
            "{}/../data/keybindings/{manifest}",
            env!("CARGO_MANIFEST_DIR")
        );
        let text = fs::read_to_string(&path).unwrap_or_else(|err| panic!("{path}: {err}"));
        let mut elements = text.split('<').filter(|e| e.starts_with("KeyListEntr"));
        let schema = attribute(elements.next().unwrap(), "schema").unwrap();
        for entry in elements {
            let name = attribute(entry, "name").unwrap();
            entries.push((schema.to_owned(), name.to_owned()));
        }
    }
    entries
}

/// The control center's Keyboard Shortcuts shows what the manifests list, so they list each
/// action Caminito binds once, and nothing else. `run-command-area-screenshot` is master's
/// (147): the schema Arch ships hasn't its key, and the dialog aborts on a key its schema
/// hasn't, so no manifest lists it until Marco's release has it.
#[test]
fn the_manifests_list_every_action_caminito_binds_once() {
    let entries = entries();
    let mut problems = Vec::new();
    for (schema, key) in bound_keys() {
        if !schema_keys(schema).iter().any(|k| k == key) {
            assert_eq!(key, "run-command-area-screenshot");
            continue;
        }
        match entries
            .iter()
            .filter(|(s, k)| s == schema && k == key)
            .count()
        {
            0 => problems.push(format!("no manifest lists {schema} {key}")),
            1 => {}
            n => problems.push(format!("{n} entries list {schema} {key}")),
        }
    }
    let bound = bound_keys();
    for (schema, key) in &entries {
        if !bound.iter().any(|(s, k)| s == schema && k == key) {
            problems.push(format!("Caminito doesn't bind {schema} {key}"));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
