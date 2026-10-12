//! `data/mate-keybinding.desktop` (183), which names the control center's Keyboard Shortcuts
//! dialog *System Shortcuts* in a Session. It needs no fixture: it's data, checked against
//! the names the spec gives and against what the stock entry launches.

use std::{collections::BTreeMap, fs};

/// The keys of the entry's one group, with their values.
fn entry() -> BTreeMap<String, String> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../data/mate-keybinding.desktop"
    );
    let text = fs::read_to_string(path).unwrap_or_else(|err| panic!("{path}: {err}"));
    let mut lines = text
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'));
    assert_eq!(lines.next(), Some("[Desktop Entry]"));
    lines
        .map(|line| {
            let (key, value) = line.split_once('=').unwrap();
            (key.to_owned(), value.to_owned())
        })
        .collect()
}

#[test]
fn the_launcher_is_system_shortcuts_and_spanish_has_its_own_name() {
    let entry = entry();

    assert_eq!(entry["Name"], "System Shortcuts");
    assert_eq!(entry["Name[es]"], "Atajos del sistema");
    // Every other locale shows the English.
    let names: Vec<_> = entry.keys().filter(|k| k.starts_with("Name[")).collect();
    assert_eq!(names, ["Name[es]"]);
}

/// It shadows the stock entry, so it has to start the same dialog from the same place in the
/// control center, and only in MATE.
#[test]
fn it_launches_the_stock_dialog_where_the_stock_entry_did() {
    let entry = entry();

    assert_eq!(entry["Type"], "Application");
    assert_eq!(entry["Exec"], "mate-keybinding-properties");
    assert_eq!(entry["Icon"], "preferences-desktop-keyboard-shortcuts");
    assert_eq!(entry["Categories"], "GTK;Settings;HardwareSettings;");
    assert_eq!(entry["OnlyShowIn"], "MATE;");
}
