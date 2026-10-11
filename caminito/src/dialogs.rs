//! Marco's zenity dialogs (167): the error a keybinding's command shows when it can't be
//! run, and `rename-workspace`'s entry. They're Marco's (`core/util.c:535-717`,
//! `meta_show_dialog` and `meta_show_entry_dialog`, at `8522ed1`), with its arguments but
//! `--display` and `--class`, which zenity 4 refuses: the display is in the environment
//! Caminito gives what it spawns. A dialog is a client like any other, and nothing waits for it.

use std::fs::File;
use std::io::Read;
use std::os::fd::OwnedFd;
use std::process::{Command, Stdio};

use caminito_policy::MAX_WORKSPACES;
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{Interest, Mode, PostAction};

use crate::commands::{child_environment, spawn_argv};
use crate::settings::Value;
use crate::settings::translate::MARCO_WORKSPACE_NAMES;
use crate::state::Caminito;

/// The title of Marco's dialogs.
const TITLE: &str = "Marco";

impl Caminito {
    /// Shows Marco's error dialog with `text`, in zenity's markup. zenity missing is logged,
    /// and nothing shows.
    pub(crate) fn show_error_dialog(&self, text: &str) {
        let argv = ["zenity", "--error", "--title", TITLE, "--text", text];
        if let Err(err) = spawn_argv(&argv, &self.environment) {
            tracing::warn!("can't show Marco's error dialog: {err}");
        }
    }

    /// `rename-workspace` (`core/keybindings.c:3744-3789`, `handle_rename_workspace`): asks
    /// for the active workspace's new name, with its name as the entry's text, and writes the answer
    /// to its `name-N`, which the pager then shows (97). A cancel, or an empty answer,
    /// leaves the name. While the dialog shows, another isn't.
    pub(crate) fn rename_workspace(&mut self) {
        if self.workspace_rename.is_some() {
            return;
        }
        let workspace = self.policy.active_workspace();
        let Some(name) = self.policy.workspace_names().get(workspace) else {
            return;
        };
        let spawned = Command::new("zenity")
            .args(["--entry", "--title", TITLE, "--text", "New Workspace Name:"])
            .args(["--entry-text", name])
            .args(["--ok-label", "OK", "--cancel-label", "Cancel"])
            .env_clear()
            .envs(child_environment(&self.environment))
            .current_dir("/")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(err) => {
                tracing::warn!("can't show Marco's dialog for rename-workspace: zenity: {err}");
                return;
            }
        };
        let stdout = child.stdout.take().expect("zenity's stdout is piped");
        // A file, which the source can read through a shared reference.
        let stdout = File::from(OwnedFd::from(stdout));
        // zenity's answer comes on the loop, as on Marco's: nothing waits for the dialog.
        let source = Generic::new(stdout, Interest::READ, Mode::Level);
        let mut answer = Vec::new();
        let inserted = self
            .loop_handle
            .insert_source(source, move |_, stdout, state| {
                let mut buffer = [0; 512];
                let read = (&**stdout).read(&mut buffer);
                if let Ok(n @ 1..) = read {
                    answer.extend_from_slice(&buffer[..n]);
                    return Ok(PostAction::Continue);
                }
                // zenity closes its stdout as it ends, so the wait is short.
                let ended = read.and_then(|_| child.wait());
                let caminito = &mut state.caminito;
                caminito.workspace_rename = None;
                match ended {
                    Ok(status) if status.success() => {
                        caminito.workspace_renamed(workspace, &answer)
                    }
                    Ok(_) => tracing::debug!("rename-workspace's dialog was cancelled"),
                    Err(err) => tracing::warn!("can't read rename-workspace's answer: {err}"),
                }
                Ok(PostAction::Remove)
            });
        match inserted {
            Ok(token) => self.workspace_rename = Some(token),
            Err(err) => tracing::warn!("can't wait for rename-workspace's dialog: {err}"),
        }
    }

    /// Writes the name zenity answered for `workspace`, as Marco's
    /// `handle_rename_workspace_callback` (`core/keybindings.c:3647-3741`) and
    /// `meta_prefs_change_workspace_name` (`core/prefs.c:2271-2306`) do: the answer without
    /// its newline, unless it's empty or the name it has.
    fn workspace_renamed(&mut self, workspace: usize, answer: &[u8]) {
        let Ok(answer) = std::str::from_utf8(answer) else {
            tracing::debug!("rename-workspace's dialog answered a name that isn't UTF-8");
            return;
        };
        let answer = answer.strip_suffix('\n').unwrap_or(answer);
        // Marco's schema names 36 workspaces.
        if answer.is_empty() || workspace >= MAX_WORKSPACES {
            return;
        }
        let names = self.policy.workspace_names();
        if names.get(workspace).is_some_and(|name| name == answer) {
            return;
        }
        let key = format!("name-{}", workspace + 1);
        self.settings_thread
            .set(MARCO_WORKSPACE_NAMES, &key, Value::Str(answer.to_owned()));
    }
}
