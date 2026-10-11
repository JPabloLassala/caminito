//! The keys that run a command (104, 149): `Print`, `<Shift>Print` and `<Alt>Print`, which
//! run Marco's three screenshot commands as Marco's `run-command-*` bindings do, the numbered
//! `run-command-*` and `run-command-terminal`. A stand-in for the program, first in the
//! `PATH` Caminito gives what it spawns, writes down how it was run.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::fixture::{
    Fixture, KEY_LEFTALT, KEY_LEFTMETA, KEY_LEFTSHIFT, KEY_PRINT, KEY_Q, KEY_T, KEY_W, Logs,
};
use crate::settings::Value;
use crate::settings::translate::{MARCO_GLOBAL_KEYBINDINGS, MARCO_KEYBINDING_COMMANDS, TERMINAL};

/// A directory with a `program` that writes its arguments to `ran` beside it, each on a
/// line of its own, and the path of `ran`.
fn stand_in(program: &str) -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let program = dir.path().join(program);
    // Moved into place, so it's never read half-written.
    let script = "#!/bin/sh\n\
                  here=$(dirname \"$0\")\n\
                  printf '%s\\n' \"$0\" \"$@\" > \"$here/running\"\n\
                  mv \"$here/running\" \"$here/ran\"\n";
    write_program(&program, script);
    let ran = dir.path().join("ran");
    (dir, ran)
}

/// Writes `script` to `program`, which can then be run. `install` writes it: were it this
/// process, another test's thread could fork with the file open for writing, and running
/// it would fail meanwhile as a text file that's busy.
pub(super) fn write_program(program: &Path, script: &str) {
    let source = program.with_extension("script");
    fs::write(&source, script).unwrap();
    let status = std::process::Command::new("install")
        .args(["-m", "755"])
        .args([&source, program])
        .status()
        .unwrap();
    assert!(status.success(), "{status}");
}

/// Puts `dir` first in the `PATH` of what Caminito spawns.
fn first_in_path(f: &mut Fixture, dir: &TempDir) {
    let mut path = dir.path().as_os_str().to_owned();
    if let Some(rest) = std::env::var_os("PATH") {
        path.push(":");
        path.push(rest);
    }
    f.state.caminito.environment.insert("PATH".into(), path);
}

/// Presses `keys` as a chord, and returns the arguments the stand-in was run with.
fn run_by(f: &mut Fixture, keys: &[u32]) -> Vec<String> {
    run_program_by(f, "mate-screenshot", keys)
}

/// Presses `keys` as a chord, and returns the arguments a stand-in for `program` was run
/// with.
fn run_program_by(f: &mut Fixture, program: &str, keys: &[u32]) -> Vec<String> {
    let (dir, ran) = stand_in(program);
    first_in_path(f, &dir);

    f.chord(keys);

    f.dispatch_until(|_| ran.exists());
    let written = fs::read_to_string(&ran).unwrap();
    let mut lines = written.lines().map(str::to_owned);
    assert_eq!(
        lines.next(),
        dir.path().join(program).to_str().map(str::to_owned)
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

// The numbered commands and the terminal (149), whose bindings Marco leaves disabled.

fn set(schema: &'static str, key: &str, value: &str) -> (&'static str, String, Value) {
    (schema, key.into(), Value::Str(value.into()))
}

#[test]
fn a_numbered_run_command_runs_its_command_once_bound() {
    let mut f = Fixture::with_settings(vec![
        set(MARCO_GLOBAL_KEYBINDINGS, "run-command-12", "<Super>q"),
        set(
            MARCO_KEYBINDING_COMMANDS,
            "command-12",
            "my-tool --open 'a file'",
        ),
    ]);

    assert_eq!(
        run_program_by(&mut f, "my-tool", &[KEY_LEFTMETA, KEY_Q]),
        ["--open", "a file"]
    );
}

#[test]
fn a_run_command_whose_command_is_empty_runs_nothing_and_is_logged() {
    let logs = Logs::capture();
    // `command-1` is Marco's default, a space; `command-2` the empty string.
    let mut f = Fixture::with_settings(vec![
        set(MARCO_GLOBAL_KEYBINDINGS, "run-command-1", "<Super>q"),
        set(MARCO_GLOBAL_KEYBINDINGS, "run-command-2", "<Super>w"),
        set(MARCO_KEYBINDING_COMMANDS, "command-2", ""),
    ]);
    // Marco's error dialog shows too (167): `tests::dialogs`' concern, so its stand-in here
    // does nothing.
    let dir = tempfile::tempdir().unwrap();
    let zenity = dir.path().join("zenity");
    write_program(&zenity, "#!/bin/sh\n");
    first_in_path(&mut f, &dir);
    // `tests::dialogs` logs the same line on threads that capture nothing, and tracing
    // caches the line as off if one of them is the first to log it. A first press has it
    // known, and then the cache is rebuilt on this thread.
    f.chord(&[KEY_LEFTMETA, KEY_Q]);
    tracing::callsite::rebuild_interest_cache();

    f.chord(&[KEY_LEFTMETA, KEY_Q]);
    f.chord(&[KEY_LEFTMETA, KEY_W]);

    let logs = logs.text();
    for (n, line) in [(1, "\" \""), (2, "\"\"")] {
        let line = format!(
            "can't run org.mate.Marco.keybinding-commands command-{n} {line} for a keybinding: \
             Text was empty (or contained only whitespace)"
        );
        assert!(logs.contains(&line), "{logs}");
    }
}

#[test]
fn run_command_terminal_runs_the_terminal_keys_command() {
    let mut f = Fixture::with_settings(vec![
        set(MARCO_GLOBAL_KEYBINDINGS, "run-command-terminal", "<Super>t"),
        set(TERMINAL, "exec", "my-terminal --wide"),
    ]);

    assert_eq!(
        run_program_by(&mut f, "my-terminal", &[KEY_LEFTMETA, KEY_T]),
        ["--wide"]
    );
}

#[test]
fn run_command_terminal_runs_mate_terminal_by_default() {
    let mut f = Fixture::with_settings(vec![set(
        MARCO_GLOBAL_KEYBINDINGS,
        "run-command-terminal",
        "<Super>t",
    )]);

    assert_eq!(
        run_program_by(&mut f, "mate-terminal", &[KEY_LEFTMETA, KEY_T]),
        [""; 0]
    );
}
