//! The keys that run a command (104): `Print`, `<Shift>Print` and `<Alt>Print`, which run
//! Marco's three screenshot commands as Marco's `run-command-*` bindings do. A stand-in for
//! mate-screenshot, first in the `PATH` Camino gives what it spawns, writes down how it was
//! run.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use tempfile::TempDir;

use super::fixture::{Fixture, KEY_LEFTALT, KEY_LEFTSHIFT, KEY_PRINT};
use crate::settings::Value;
use crate::settings::translate::MARCO_KEYBINDING_COMMANDS;

/// A directory with a `mate-screenshot` that writes its arguments to `ran` beside it, each
/// on a line of its own, and the path of `ran`.
fn stand_in() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let program = dir.path().join("mate-screenshot");
    // Moved into place, so it's never read half-written.
    let script = "#!/bin/sh\n\
                  here=$(dirname \"$0\")\n\
                  printf '%s\\n' \"$0\" \"$@\" > \"$here/running\"\n\
                  mv \"$here/running\" \"$here/ran\"\n";
    fs::write(&program, script).unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    let ran = dir.path().join("ran");
    (dir, ran)
}

/// Puts `dir` first in the `PATH` of what Camino spawns.
fn first_in_path(f: &mut Fixture, dir: &TempDir) {
    let mut path = dir.path().as_os_str().to_owned();
    if let Some(rest) = std::env::var_os("PATH") {
        path.push(":");
        path.push(rest);
    }
    f.state.camino.environment.insert("PATH".into(), path);
}

/// Presses `keys` as a chord, and returns the arguments the stand-in was run with.
fn run_by(f: &mut Fixture, keys: &[u32]) -> Vec<String> {
    let (dir, ran) = stand_in();
    first_in_path(f, &dir);

    f.chord(keys);

    f.dispatch_until(|_| ran.exists());
    let written = fs::read_to_string(&ran).unwrap();
    let mut lines = written.lines().map(str::to_owned);
    assert_eq!(
        lines.next(),
        dir.path()
            .join("mate-screenshot")
            .to_str()
            .map(str::to_owned)
    );
    lines.collect()
}

#[test]
fn print_runs_the_screenshot_command() {
    let mut f = Fixture::new();

    assert_eq!(run_by(&mut f, &[KEY_PRINT]), [""; 0]);
}

#[test]
fn alt_print_runs_the_window_screenshot_command() {
    let mut f = Fixture::new();

    assert_eq!(run_by(&mut f, &[KEY_LEFTALT, KEY_PRINT]), ["--window"]);
}

#[test]
fn shift_print_runs_the_area_screenshot_command() {
    let mut f = Fixture::new();

    assert_eq!(run_by(&mut f, &[KEY_LEFTSHIFT, KEY_PRINT]), ["--area"]);
}

#[test]
fn the_command_run_is_the_one_in_marcos_settings() {
    let command = Value::Str("mate-screenshot --delay 3 --border-effect 'no ne'".into());
    let mut f = Fixture::with_settings(vec![(
        MARCO_KEYBINDING_COMMANDS,
        "command-screenshot".into(),
        command,
    )]);

    assert_eq!(
        run_by(&mut f, &[KEY_PRINT]),
        ["--delay", "3", "--border-effect", "no ne"]
    );
}
