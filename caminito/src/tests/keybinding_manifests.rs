//! The `50-caminito-*.xml` keybinding manifests (05, 101), which tell the control center's
//! Keyboard Shortcuts dialog what Caminito binds. They need no fixture: they're data, checked
//! against Marco's schema, whose keys they point at. That schema has no key for an action
//! Caminito never implements (05), so a manifest that passes lists none.

use std::fs;

const MANIFESTS: [&str; 3] = [
    "50-caminito-desktop-key.xml",
    "50-caminito-global-key.xml",
    "50-caminito-window-key.xml",
];

/// The value of `name="…"` in an element's text.
fn attribute<'a>(element: &'a str, name: &str) -> Option<&'a str> {
    let opening = format!(" {name}=\"");
    let start = element.find(&opening)? + opening.len();
    let len = element[start..].find('"')?;
    Some(&element[start..start + len])
}

/// The names of the keys of one schema in the vendored `org.mate.marco.gschema.xml`.
pub(super) fn schema_keys(id: &str) -> Vec<String> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/schemas/org.mate.marco.gschema.xml"
    );
    let schemas = fs::read_to_string(path).unwrap();
    let start = schemas
        .find(&format!("<schema id=\"{id}\""))
        .unwrap_or_else(|| panic!("Marco has no schema {id}"));
    let schema = &schemas[start..];
    let schema = &schema[..schema.find("</schema>").unwrap()];
    schema
        .split("<key ")
        .skip(1)
        .map(|key| attribute(&format!(" {key}"), "name").unwrap().to_owned())
        .collect()
}

#[test]
fn each_manifest_is_caminitos_and_names_only_keys_marcos_schema_has() {
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
            schema.starts_with("org.mate.Marco."),
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
