//! Marco's zenity dialogs (167): the error a `run-command-*` binding shows when its
//! command is empty or can't be run, and `rename-workspace`'s entry. A stand-in for zenity,
//! first in the `PATH` Caminito gives what it spawns, writes down its arguments and gives
//! the answer the test sets, or cancels.

use std::fs;
use std::path::PathBuf;

use tempfile::TempDir;

use super::commands::write_program;
use super::fixture::{Fixture, KEY_LEFTALT, KEY_LEFTCTRL, KEY_LEFTMETA, KEY_Q, KEY_R, KEY_RIGHT};
use crate::settings::Value;
use crate::settings::translate::{
    MARCO_GLOBAL_KEYBINDINGS, MARCO_KEYBINDING_COMMANDS, MARCO_WORKSPACE_NAMES,
};

/// A stand-in for zenity in a directory of its own.
struct Zenity {
    dir: TempDir,
}

impl Zenity {
    /// Puts the stand-in first in the `PATH` of what Caminito spawns. It answers `answer`,
    /// as a user who types it and presses OK, or with `None` cancels.
    fn install(f: &mut Fixture, answer: Option<&str>) -> Self {
        let dir = tempfile::tempdir().unwrap();
        // Each argument ends in a NUL, as the error's text has lines of its own. The record
        // is moved into place once the answer is out, so it's never read half-written.
        let script = "#!/bin/sh\n\
                      here=$(dirname \"$0\")\n\
                      printf '%s\\0' \"$@\" > \"$here/running\"\n\
                      status=1\n\
                      if [ -e \"$here/answer\" ]; then cat \"$here/answer\"; status=0; fi\n\
                      mv \"$here/running\" \"$here/ran\"\n\
                      exit $status\n";
        let program = dir.path().join("zenity");
        write_program(&program, script);
        if let Some(answer) = answer {
            // zenity prints the entry's text and a newline.
            fs::write(dir.path().join("answer"), format!("{answer}\n")).unwrap();
        }

        let mut path = dir.path().as_os_str().to_owned();
        if let Some(rest) = std::env::var_os("PATH") {
            path.push(":");
            path.push(rest);
        }
        f.state.caminito.environment.insert("PATH".into(), path);
        Self { dir }
    }

    fn ran(&self) -> PathBuf {
        self.dir.path().join("ran")
    }

    /// Waits for the stand-in to have run, and returns its arguments.
    fn arguments(&self, f: &mut Fixture) -> Vec<String> {
        let ran = self.ran();
        f.dispatch_until(|_| ran.exists());
        let written = fs::read_to_string(&ran).unwrap();
        written.split_terminator('\0').map(str::to_owned).collect()
    }
}

fn set(schema: &'static str, key: &str, value: &str) -> (&'static str, String, Value) {
    (schema, key.into(), Value::Str(value.into()))
}

fn error_dialog(text: &str) -> Vec<String> {
    ["--error", "--title", "Marco", "--text", text]
        .map(str::to_owned)
        .to_vec()
}

// A command that fails.

/// A fixture with `run-command-1` on Super+Q, and `command-1` set to `command`.
fn command_1(command: &str) -> Fixture {
    Fixture::with_settings(vec![
        set(MARCO_GLOBAL_KEYBINDINGS, "run-command-1", "<Super>q"),
        set(MARCO_KEYBINDING_COMMANDS, "command-1", command),
    ])
}

#[test]
fn an_empty_command_shows_marcos_error_dialog_with_glibs_message() {
    let mut f = command_1("");
    let zenity = Zenity::install(&mut f, None);

    f.chord(&[KEY_LEFTMETA, KEY_Q]);

    assert_eq!(
        zenity.arguments(&mut f),
        error_dialog("Text was empty (or contained only whitespace)")
    );
}

#[test]
fn marcos_default_command_of_a_space_shows_the_dialog_naming_it() {
    let mut f = Fixture::with_settings(vec![set(
        MARCO_GLOBAL_KEYBINDINGS,
        "run-command-1",
        "<Super>q",
    )]);
    let zenity = Zenity::install(&mut f, None);

    f.chord(&[KEY_LEFTMETA, KEY_Q]);

    assert_eq!(
        zenity.arguments(&mut f),
        error_dialog(
            "There was an error running <tt> </tt>:\n\n\
             Text was empty (or contained only whitespace)"
        )
    );
}

#[test]
fn a_missing_program_shows_marcos_error_dialog_naming_the_command() {
    let mut f = command_1("no-such-program --open");
    let zenity = Zenity::install(&mut f, None);

    f.chord(&[KEY_LEFTMETA, KEY_Q]);

    assert_eq!(
        zenity.arguments(&mut f),
        error_dialog(
            "There was an error running <tt>no-such-program --open</tt>:\n\n\
             Failed to execute child process “no-such-program” (No such file or directory)"
        )
    );
}

#[test]
fn the_command_in_the_dialog_is_escaped_for_its_markup() {
    let mut f = command_1("no-such-program <a&b>");
    let zenity = Zenity::install(&mut f, None);

    f.chord(&[KEY_LEFTMETA, KEY_Q]);

    let text = &zenity.arguments(&mut f)[4];
    assert!(
        text.starts_with("There was an error running <tt>no-such-program &lt;a&amp;b&gt;</tt>"),
        "{text}"
    );
}

// `rename-workspace`.

/// A fixture with `rename-workspace` on Super+R, and `initial` besides.
fn with_rename_bound(mut initial: Vec<(&'static str, String, Value)>) -> Fixture {
    initial.push(set(
        MARCO_GLOBAL_KEYBINDINGS,
        "rename-workspace",
        "<Super>r",
    ));
    Fixture::with_settings(initial)
}

fn entry_dialog(entry_text: &str) -> Vec<String> {
    [
        "--entry",
        "--title",
        "Marco",
        "--text",
        "New Workspace Name:",
        "--entry-text",
        entry_text,
        "--ok-label",
        "OK",
        "--cancel-label",
        "Cancel",
    ]
    .map(str::to_owned)
    .to_vec()
}

fn name(f: &Fixture, workspace: usize) -> String {
    f.state.caminito.policy.workspace_names()[workspace].clone()
}

#[test]
fn rename_workspace_asks_for_the_active_workspaces_name_and_writes_the_answer() {
    let mut f = with_rename_bound(vec![set(MARCO_WORKSPACE_NAMES, "name-3", "Mail")]);
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    assert_eq!(f.state.caminito.policy.active_workspace(), 2);
    let zenity = Zenity::install(&mut f, Some("Post"));

    f.chord(&[KEY_LEFTMETA, KEY_R]);

    assert_eq!(zenity.arguments(&mut f), entry_dialog("Mail"));
    // The pager's names are policy's (97).
    f.dispatch_until(|f| name(f, 2) == "Post");
    assert_eq!(name(&f, 0), "Workspace 1");
}

#[test]
fn an_unnamed_workspace_is_offered_marcos_default_name() {
    let mut f = with_rename_bound(vec![]);
    let zenity = Zenity::install(&mut f, Some("Code"));

    f.chord(&[KEY_LEFTMETA, KEY_R]);

    assert_eq!(zenity.arguments(&mut f), entry_dialog("Workspace 1"));
    f.dispatch_until(|f| name(f, 0) == "Code");
}

/// Waits for the dialog to be over, then for a write of the test's own to come back from
/// the settings thread, so that any write the dialog made is in by then.
fn wait_for_the_dialog_and_the_settings(f: &mut Fixture, zenity: &Zenity) {
    let ran = zenity.ran();
    f.dispatch_until(|f| ran.exists() && f.state.caminito.workspace_rename.is_none());
    f.set_setting(
        MARCO_WORKSPACE_NAMES,
        "name-4",
        Value::Str("Barrier".into()),
    );
    f.dispatch_until(|f| name(f, 3) == "Barrier");
}

#[test]
fn cancelling_rename_workspace_leaves_the_name() {
    let mut f = with_rename_bound(vec![set(MARCO_WORKSPACE_NAMES, "name-1", "Mail")]);
    let zenity = Zenity::install(&mut f, None);

    f.chord(&[KEY_LEFTMETA, KEY_R]);

    wait_for_the_dialog_and_the_settings(&mut f, &zenity);
    assert_eq!(name(&f, 0), "Mail");
}

#[test]
fn an_empty_answer_leaves_the_name() {
    let mut f = with_rename_bound(vec![set(MARCO_WORKSPACE_NAMES, "name-1", "Mail")]);
    let zenity = Zenity::install(&mut f, Some(""));

    f.chord(&[KEY_LEFTMETA, KEY_R]);

    wait_for_the_dialog_and_the_settings(&mut f, &zenity);
    assert_eq!(name(&f, 0), "Mail");
}

#[test]
fn rename_workspace_is_unbound_by_default() {
    let mut f = Fixture::new();
    let zenity = Zenity::install(&mut f, Some("Code"));

    f.chord(&[KEY_LEFTMETA, KEY_R]);
    f.set_setting(
        MARCO_WORKSPACE_NAMES,
        "name-4",
        Value::Str("Barrier".into()),
    );
    f.dispatch_until(|f| name(f, 3) == "Barrier");

    assert!(!zenity.ran().exists());
    assert_eq!(name(&f, 0), "Workspace 1");
}
