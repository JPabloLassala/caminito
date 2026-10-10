//! `docs/parity.md`, the parity table (17, 147): a row for every key and action of Marco's
//! five schemas, and for every key Caminito logs as unsupported. Like the keybinding
//! manifests it's data, so it needs no fixture.
//!
//! A row is a line of a table whose first cell names its key in backticks, in the schema the
//! nearest heading above it names, or as `` `schema` `key` `` under a heading that names
//! none. A numbered family shares a row as `` `name-1` to `-12` ``, and a family of keys
//! as `` `prefix-*` ``.

use std::fs;

use super::keybinding_manifests::schema_keys;
use crate::settings::translate::UNSUPPORTED;

/// Marco's five schemas, which the table covers whole.
const MARCO_SCHEMAS: [&str; 5] = [
    "org.mate.Marco.general",
    "org.mate.Marco.workspace-names",
    "org.mate.Marco.keybinding-commands",
    "org.mate.Marco.global-keybindings",
    "org.mate.Marco.window-keybindings",
];

/// The keys Marco's master adds to the vendored 1.29.0 schema, which Arch ships (147).
const MASTER_KEYS: [(&str, &str); 5] = [
    ("org.mate.Marco.general", "dynamic-workspaces"),
    ("org.mate.Marco.general", "alt-tab-minimized-placement"),
    ("org.mate.Marco.general", "alt-tab-urgent-placement"),
    (
        "org.mate.Marco.global-keybindings",
        "run-command-area-screenshot",
    ),
    (
        "org.mate.Marco.keybinding-commands",
        "command-area-screenshot",
    ),
];

/// 17's four statuses, and *to do* until M8 closes (173).
const STATUSES: [&str; 5] = [
    "honoured",
    "never",
    "left to another component",
    "unsupported",
    "to do",
];

/// The tickets that settle M8's *to do* rows.
const M8_TICKETS: std::ops::RangeInclusive<u32> = 148..=172;

#[derive(Debug)]
struct Row {
    schema: String,
    key: String,
    status: &'static str,
}

impl Row {
    fn is(&self, schema: &str, key: &str) -> bool {
        self.schema == schema
            && match self.key.strip_suffix('*') {
                Some(prefix) => key.starts_with(prefix),
                None => self.key == key,
            }
    }
}

/// The texts between backticks.
fn quoted(text: &str) -> Vec<&str> {
    text.split('`').skip(1).step_by(2).collect()
}

/// The numbers in a text.
fn numbers(text: &str) -> Vec<u32> {
    text.split(|c: char| !c.is_ascii_digit())
        .filter_map(|n| n.parse().ok())
        .collect()
}

/// The keys a row's first cell names, under the schema of the heading above it.
fn keys(cell: &str, heading: Option<&str>) -> Vec<(String, String)> {
    let quoted = quoted(cell);
    match (heading, quoted.as_slice()) {
        (_, [schema, key]) if schema.starts_with("org.mate.") => {
            vec![(schema.to_string(), key.to_string())]
        }
        (None, _) => vec![],
        (Some(schema), [key]) => vec![(schema.into(), key.to_string())],
        (Some(schema), [first, last]) if cell.contains("` to `-") => {
            let (prefix, from) = first.rsplit_once('-').expect("a family's key is numbered");
            let from: u32 = from.parse().expect("a family's key is numbered");
            let to: u32 = last[1..].parse().expect("a family ends at a number");
            (from..=to)
                .map(|n| (schema.into(), format!("{prefix}-{n}")))
                .collect()
        }
        _ => panic!("a row names one key or one numbered family: {cell}"),
    }
}

/// Every row of the table.
fn rows() -> Vec<Row> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/parity.md");
    let text = fs::read_to_string(path).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let mut heading = None;
    let mut rows = Vec::new();
    for (n, line) in lines.iter().enumerate() {
        if line.starts_with('#') {
            heading = quoted(line)
                .into_iter()
                .find(|q| q.starts_with("org.mate."));
            continue;
        }
        let is_header = lines
            .get(n + 1)
            .is_some_and(|next| next.starts_with("| ---"));
        if !line.starts_with("| ") || line.starts_with("| ---") || is_header {
            continue;
        }
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        let (first, status) = (cells[1], cells[2]);
        for (schema, key) in keys(first, heading) {
            let status = STATUSES
                .into_iter()
                .find(|s| status.starts_with(s))
                .unwrap_or_else(|| panic!("{key}'s status isn't one of 17's: {status}"));
            if status == "to do" {
                assert!(
                    numbers(cells[2])
                        .into_iter()
                        .any(|t| M8_TICKETS.contains(&t)),
                    "{key} is to do and names none of 148 to 172"
                );
            }
            rows.push(Row {
                schema,
                key,
                status,
            });
        }
    }
    rows
}

#[test]
fn every_key_and_action_of_marcos_schemas_has_one_row() {
    let rows = rows();
    let mut problems = Vec::new();
    for schema in MARCO_SCHEMAS {
        let mut keys = schema_keys(schema);
        keys.extend(
            MASTER_KEYS
                .iter()
                .filter(|(s, _)| *s == schema)
                .map(|(_, key)| key.to_string()),
        );
        for key in &keys {
            match rows.iter().filter(|row| row.is(schema, key)).count() {
                0 => problems.push(format!("{schema} {key} has no row")),
                1 => {}
                _ => problems.push(format!("{schema} {key} has more than one row")),
            }
        }
        for row in rows.iter().filter(|row| row.schema == schema) {
            if !keys.contains(&row.key) {
                problems.push(format!("{schema} has no key {}", row.key));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn every_unsupported_key_has_an_unsupported_or_to_do_row() {
    let rows = rows();
    for (schema, key) in UNSUPPORTED {
        let row = rows
            .iter()
            .find(|row| row.is(schema, key))
            .unwrap_or_else(|| panic!("{schema} {key} is unsupported and has no row"));
        assert!(
            matches!(row.status, "unsupported" | "to do"),
            "{schema} {key} is unsupported, and its row says {}",
            row.status
        );
    }
}
