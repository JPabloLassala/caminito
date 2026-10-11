//! The commands a keybinding runs (104, 149), as Marco's `run-command-*` bindings run those
//! of `org.mate.Marco.keybinding-commands`, and `run-command-terminal` the terminal of
//! `org.mate.applications-terminal`.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::Path;

use gio::glib;

use crate::settings::translate::{MARCO_KEYBINDING_COMMANDS, TERMINAL};
use crate::state::Caminito;

/// A command of `org.mate.Marco.keybinding-commands`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Command {
    /// `command-screenshot`, of the screen.
    Screenshot,
    /// `command-window-screenshot`, of the active window.
    WindowScreenshot,
    /// `command-area-screenshot`, of an area the user picks.
    AreaScreenshot,
    /// `command-1` to `-12`, by their number.
    Numbered(usize),
    /// `org.mate.applications-terminal exec`, which `run-command-terminal` runs.
    Terminal,
}

impl Caminito {
    /// Runs the command line the settings have for `command`. One that can't be run, as
    /// an empty one can't, is logged and shows Marco's error dialog (167).
    pub(crate) fn run_command(&mut self, command: Command) {
        let commands = &self.settings.commands;
        let line = match command {
            Command::Screenshot => &commands.screenshot,
            Command::WindowScreenshot => &commands.window_screenshot,
            Command::AreaScreenshot => &commands.area_screenshot,
            Command::Numbered(n) => &commands.numbered[n - 1],
            Command::Terminal => &commands.terminal,
        };
        match spawn(line, &self.environment) {
            Ok(()) => tracing::debug!("ran {line:?} for a keybinding"),
            Err(err) => {
                tracing::warn!(
                    "can't run {} {line:?} for a keybinding: {err}",
                    command.key()
                );
                let text = error_text(line, err.message());
                self.show_error_dialog(&text);
            }
        }
    }
}

/// The text of Marco's dialog for a command that can't be run (`error_on_command`,
/// `core/keybindings.c:2397-2445`): GLib's `message`, after the command unless it's empty.
/// Marco's schema has a space for each numbered command, which GLib can't run either. Both
/// are escaped for zenity's markup, which Marco's aren't.
fn error_text(line: &str, message: &str) -> String {
    let message = glib::markup_escape_text(message);
    if line.is_empty() {
        return message.into();
    }
    let line = glib::markup_escape_text(line);
    format!("There was an error running <tt>{line}</tt>:\n\n{message}")
}

impl Command {
    /// The key that holds the command line, as `schema key`.
    fn key(self) -> String {
        let key = match self {
            Command::Screenshot => "command-screenshot".to_owned(),
            Command::WindowScreenshot => "command-window-screenshot".to_owned(),
            Command::AreaScreenshot => "command-area-screenshot".to_owned(),
            Command::Numbered(n) => format!("command-{n}"),
            Command::Terminal => return format!("{TERMINAL} exec"),
        };
        format!("{MARCO_KEYBINDING_COMMANDS} {key}")
    }
}

/// Spawns `line`, split as a shell splits words, with Caminito's own environment under
/// `environment`, and doesn't wait for it. It's GLib's spawn, as Marco's
/// (`core/keybindings.c:2449-2478`, `meta_spawn_command_line_async_on_screen`, at `8522ed1`):
/// the program is a grandchild, so Caminito has no child to reap.
fn spawn(line: &str, environment: &BTreeMap<OsString, OsString>) -> Result<(), glib::Error> {
    let argv = glib::shell_parse_argv(line)?;
    spawn_argv(&argv, environment)
}

/// Spawns `argv` as [`spawn`] does a command line.
pub(crate) fn spawn_argv(
    argv: &[impl AsRef<OsStr>],
    environment: &BTreeMap<OsString, OsString>,
) -> Result<(), glib::Error> {
    let argv: Vec<&Path> = argv.iter().map(Path::new).collect();
    let envp: Vec<OsString> = child_environment(environment)
        .into_iter()
        .map(|(mut name, value)| {
            name.push("=");
            name.push(value);
            name
        })
        .collect();
    let envp: Vec<&Path> = envp.iter().map(Path::new).collect();

    // The program is looked for in the `PATH` it's given.
    let flags = glib::SpawnFlags::SEARCH_PATH | glib::SpawnFlags::SEARCH_PATH_FROM_ENVP;
    glib::spawn_async(None::<&Path>, &argv, &envp, flags, None)?;
    Ok(())
}

/// The environment of what Caminito spawns: its own, with `environment` over it.
pub(crate) fn child_environment(
    environment: &BTreeMap<OsString, OsString>,
) -> BTreeMap<OsString, OsString> {
    let mut variables: BTreeMap<OsString, OsString> = std::env::vars_os().collect();
    // A nested run has its host's, which isn't Caminito's Xwayland's.
    if !environment.contains_key(OsStr::new("DISPLAY")) {
        variables.remove(OsStr::new("DISPLAY"));
    }
    variables.extend(environment.clone());
    variables
}
