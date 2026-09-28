//! camino-theme stays free of Smithay, GTK, Wayland and D-Bus (15, ADR 0010), so the
//! theme engine can't reach into the compositor or a toolkit.

use std::process::Command;

/// Whether camino-theme must not depend on a crate, even indirectly. glib and gio are
/// allowed: pango and cairo bring them (15). So is gdk-pixbuf, for images (07).
fn forbidden(name: &str) -> bool {
    ["smithay", "gtk", "gdk4", "wayland", "zbus", "dbus"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
        || name == "gdk"
        || name == "gdk-sys"
}

#[test]
fn no_forbidden_dependencies() {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "--offline",
            "--package",
            "camino-theme",
            "--edges",
            "normal,build",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo runs");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8(output.stdout).unwrap();
    let forbidden: Vec<&str> = tree
        .lines()
        .filter(|line| forbidden(line.split_whitespace().next().unwrap_or_default()))
        .collect();
    assert!(tree.starts_with("camino-theme "), "{tree}");
    assert!(
        forbidden.is_empty(),
        "forbidden dependencies: {forbidden:?}"
    );
}
