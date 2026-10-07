//! The commands a keybinding runs (104), as Marco's `run-command-*` bindings run those of
//! `org.mate.Marco.keybinding-commands`. So far, mate-screenshot's three.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::Path;

use gio::glib;

use crate::state::Camino;

/// A command of `org.mate.Marco.keybinding-commands`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// `command-screenshot`, of the screen.
    Screenshot,
    /// `command-window-screenshot`, of the active window.
    WindowScreenshot,
    /// `command-area-screenshot`, of an area the user picks.
    AreaScreenshot,
}

impl Camino {
    /// Runs the command line the settings have for `command`. One that's empty or can't be
    /// run is logged: Marco's dialog for it comes with the bindings, in M8 (17).
    pub(crate) fn run_command(&mut self, command: Command) {
        let commands = &self.settings.commands;
        let line = match command {
            Command::Screenshot => &commands.screenshot,
            Command::WindowScreenshot => &commands.window_screenshot,
            Command::AreaScreenshot => &commands.area_screenshot,
        };
        match spawn(line, &self.environment) {
            Ok(()) => tracing::debug!("ran {line:?} for a keybinding"),
            Err(err) => tracing::warn!("can't run {line:?} for a keybinding: {err}"),
        }
    }
}

/// Spawns `line`, split as a shell splits words, with Camino's own environment under
/// `environment`, and doesn't wait for it. It's GLib's spawn, as Marco's
/// (`core/util.c`'s `meta_spawn_command_line_async_on_screen`): the program is a grandchild,
/// so Camino has no child to reap.
fn spawn(line: &str, environment: &BTreeMap<OsString, OsString>) -> Result<(), glib::Error> {
    let argv = glib::shell_parse_argv(line)?;
    let argv: Vec<&Path> = argv.iter().map(Path::new).collect();

    let mut variables: BTreeMap<OsString, OsString> = std::env::vars_os().collect();
    // A nested run has its host's, which isn't Camino's Xwayland's.
    if !environment.contains_key(OsStr::new("DISPLAY")) {
        variables.remove(OsStr::new("DISPLAY"));
    }
    variables.extend(environment.clone());
    let envp: Vec<OsString> = variables
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
