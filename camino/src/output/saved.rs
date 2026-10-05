//! Saved layouts (83, ADR 0014): for each set of connected monitors the user saved one for,
//! which monitors are enabled, their modes, places and scales, and the primary output. The
//! file they're kept in, saving one when `org.mate.Camino` asks, and restoring the one for
//! the monitors there are when a monitor comes or goes.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::{env, fmt, fs, io};

use serde::{Deserialize, Serialize};
use smithay::output::{Mode, Output};

use super::primary::MonitorIdentity;
use super::{OutputChange, OutputSettings, SETTABLE_COORDINATES, SETTABLE_SCALES, matching_mode};
use crate::state::{Camino, State};

/// The file's name, in Camino's directory under `$XDG_CONFIG_HOME`.
pub(crate) const FILE_NAME: &str = "layouts.json";

/// The version of the file's form that this Camino reads and writes.
const VERSION: u32 = 1;

/// Where the saved layouts are kept: `camino/layouts.json` under `$XDG_CONFIG_HOME`, or
/// under `~/.config`. `None` when the environment names neither.
pub(crate) fn default_path() -> Option<PathBuf> {
    path_from(env::var_os("XDG_CONFIG_HOME"), env::var_os("HOME"))
}

/// [`default_path`] from the variables' values. An `XDG_CONFIG_HOME` that is empty or
/// relative counts as unset, as the XDG base directory specification has it.
fn path_from(config_home: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    let config_home = config_home
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute());
    let home = || home.filter(|home| !home.is_empty());
    let config = config_home.or_else(|| Some(PathBuf::from(home()?).join(".config")))?;
    Some(config.join("camino").join(FILE_NAME))
}

/// Why a layout wasn't saved, or the saved ones weren't read.
#[derive(Debug)]
pub(crate) enum Error {
    /// The environment names no directory to keep the file in.
    NoPlace,
    /// No output is enabled: restored, such a layout would leave nothing to see it on.
    NothingEnabled,
    /// The file is there and couldn't be read.
    Read(PathBuf, io::Error),
    /// The file isn't in the form Camino writes.
    Parse(PathBuf, serde_json::Error),
    /// The file is of a version this Camino doesn't know.
    Version(PathBuf, u32),
    /// The file, or its directory, couldn't be written.
    Write(PathBuf, io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoPlace => {
                f.write_str("neither XDG_CONFIG_HOME nor HOME names a place for saved layouts")
            }
            Error::NothingEnabled => f.write_str("no output is enabled"),
            Error::Read(path, err) => write!(f, "can't read {}: {err}", path.display()),
            Error::Parse(path, err) => {
                write!(f, "{} isn't a file of saved layouts: {err}", path.display())
            }
            Error::Version(path, version) => write!(
                f,
                "{} is of version {version}, and this Camino reads version {VERSION}",
                path.display()
            ),
            Error::Write(path, err) => write!(f, "can't write {}: {err}", path.display()),
        }
    }
}

impl std::error::Error for Error {}

/// The file, as JSON.
#[derive(Debug, Serialize, Deserialize)]
struct File {
    version: u32,
    layouts: Vec<SavedLayout>,
}

/// What every version of the file has.
#[derive(Debug, Deserialize)]
struct Versioned {
    version: u32,
}

/// The layout the user saved for one set of connected monitors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SavedLayout {
    monitors: Vec<SavedMonitor>,
}

/// One monitor of a saved layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SavedMonitor {
    /// The connector it was at, which is what a monitor without an EDID serial is known
    /// by, and what tells two monitors with the same EDID apart.
    connector: String,
    /// Its EDID's make, model and serial, if it has a serial.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    edid: Option<Edid>,
    /// What it shows, or `None` if it's disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shows: Option<Shown>,
    /// Whether it's the primary output (81).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    primary: bool,
}

/// What a monitor's EDID says of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Edid {
    make: String,
    model: String,
    serial: String,
}

/// What an enabled monitor of a saved layout shows.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct Shown {
    /// Its mode, if it had one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mode: Option<SavedMode>,
    /// Its place in the global logical space.
    x: i32,
    y: i32,
    scale: f64,
}

/// A mode: its size in pixels, and its refresh rate in mHz.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct SavedMode {
    width: i32,
    height: i32,
    refresh: i32,
}

impl SavedMonitor {
    /// The monitor `output` shows, with what it's to be saved as showing.
    fn of(output: &Output, shows: Option<Shown>, primary: bool) -> Self {
        let edid = match MonitorIdentity::of(output) {
            MonitorIdentity::Edid {
                make,
                model,
                serial,
            } => Some(Edid {
                make,
                model,
                serial,
            }),
            MonitorIdentity::Connector(_) => None,
        };
        SavedMonitor {
            connector: output.name(),
            edid,
            shows,
            primary,
        }
    }

    /// What the monitor is known by (ADR 0014).
    fn identity(&self) -> MonitorIdentity {
        match self.edid.clone() {
            Some(Edid {
                make,
                model,
                serial,
            }) => MonitorIdentity::Edid {
                make,
                model,
                serial,
            },
            None => MonitorIdentity::Connector(self.connector.clone()),
        }
    }
}

/// What an output shows now.
fn shown(output: &Output) -> Shown {
    let place = output.current_location();
    Shown {
        mode: output.current_mode().map(|mode| SavedMode {
            width: mode.size.w,
            height: mode.size.h,
            refresh: mode.refresh,
        }),
        x: place.x,
        y: place.y,
        scale: output.current_scale().fractional_scale(),
    }
}

/// Each of a layout's `monitors` with the one of `connected` that it is, if the layout is
/// for exactly those monitors. Of two monitors with the same EDID, each is the one at the
/// connector it was saved at, if one is there.
fn assign<'a>(
    monitors: &'a [SavedMonitor],
    connected: &[Output],
) -> Option<Vec<(Output, &'a SavedMonitor)>> {
    if monitors.len() != connected.len() {
        return None;
    }
    let mut free: Vec<&Output> = connected.iter().collect();
    let mut found: Vec<Option<&Output>> = vec![None; monitors.len()];
    for at_its_connector in [true, false] {
        for (monitor, found) in monitors.iter().zip(&mut found) {
            if found.is_some() {
                continue;
            }
            let identity = monitor.identity();
            let index = free.iter().position(|output| {
                MonitorIdentity::of(output) == identity
                    && (!at_its_connector || output.name() == monitor.connector)
            });
            *found = index.map(|index| free.remove(index));
        }
    }
    monitors
        .iter()
        .zip(found)
        .map(|(monitor, output)| Some((output?.clone(), monitor)))
        .collect()
}

/// The saved layouts, and the file they're kept in.
#[derive(Debug, Default)]
pub(crate) struct SavedLayouts {
    /// The file, or `None` where nothing is kept: a Camino that saves no layout.
    path: Option<PathBuf>,
    layouts: Vec<SavedLayout>,
}

impl SavedLayouts {
    /// The layouts saved in the file at `path`: none if it isn't there yet, and none, with
    /// a warning, if it can't be read. The next layout saved replaces it.
    pub(crate) fn load(path: Option<PathBuf>) -> Self {
        let layouts = match path.as_deref().map(read) {
            Some(Ok(layouts)) => layouts,
            Some(Err(err)) => {
                tracing::warn!("can't restore the saved layouts: {err}");
                Vec::new()
            }
            None => Vec::new(),
        };
        SavedLayouts { path, layouts }
    }

    /// The layout saved for exactly the monitors `connected`, as each of them with what
    /// it was saved as.
    fn for_monitors(&self, connected: &[Output]) -> Option<Vec<(Output, &SavedMonitor)>> {
        self.layouts
            .iter()
            .find_map(|layout| assign(&layout.monitors, connected))
    }

    /// Keeps `layout` as the one for the monitors `connected`, in place of the one there
    /// was for them, and writes the file.
    fn keep(&mut self, layout: SavedLayout, connected: &[Output]) -> Result<(), Error> {
        let path = self.path.as_deref().ok_or(Error::NoPlace)?;
        let mut layouts = self.layouts.clone();
        layouts.retain(|other| assign(&other.monitors, connected).is_none());
        layouts.push(layout);
        write(path, &layouts).map_err(|err| Error::Write(path.to_owned(), err))?;
        self.layouts = layouts;
        Ok(())
    }
}

/// The layouts in the file at `path`, or none if there's no such file.
fn read(path: &Path) -> Result<Vec<SavedLayout>, Error> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(Error::Read(path.to_owned(), err)),
    };
    let parse = |err| Error::Parse(path.to_owned(), err);
    // First, since a later version's layouts needn't parse as this one's.
    let Versioned { version } = serde_json::from_str(&text).map_err(parse)?;
    if version != VERSION {
        return Err(Error::Version(path.to_owned(), version));
    }
    let file: File = serde_json::from_str(&text).map_err(parse)?;
    Ok(file.layouts)
}

/// Writes the file at `path` whole, beside it and then over it, so that a Camino that
/// stops halfway leaves the old one.
fn write(path: &Path, layouts: &[SavedLayout]) -> io::Result<()> {
    let file = File {
        version: VERSION,
        layouts: layouts.to_vec(),
    };
    let text = serde_json::to_string_pretty(&file).map_err(io::Error::other)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let beside = path.with_extension("json.new");
    fs::write(&beside, text + "\n")?;
    fs::rename(&beside, path)
}

/// What restoring a saved layout does.
struct Restore {
    changes: Vec<OutputChange>,
    /// The output of the monitor that was the primary output.
    primary: Option<Output>,
}

impl Camino {
    /// The connected monitors: the outputs, then the ones output management disabled.
    fn connected_monitors(&self) -> Vec<Output> {
        let disabled = self.disabled_outputs.iter();
        self.outputs.iter().chain(disabled).cloned().collect()
    }

    /// Saves the layout as it is now for the connected monitors, in place of the one saved
    /// for them before: which are enabled, their modes, places and scales, and the primary
    /// output there is, the user's choice or its stand-in.
    pub(crate) fn save_layout(&mut self) -> Result<(), Error> {
        if self.outputs.is_empty() {
            return Err(Error::NothingEnabled);
        }
        let primary = self.primary_output();
        let enabled = self
            .outputs
            .iter()
            .map(|output| SavedMonitor::of(output, Some(shown(output)), Some(output) == primary));
        let disabled = self
            .disabled_outputs
            .iter()
            .map(|output| SavedMonitor::of(output, None, false));
        let layout = SavedLayout {
            monitors: enabled.chain(disabled).collect(),
        };
        let connected = self.connected_monitors();
        self.saved_layouts.keep(layout, &connected)
    }

    /// The mode a monitor is restored in when its saved one is of no use: the one it
    /// shows, or for a disabled monitor, the one it prefers.
    fn mode_without_a_saved_one(&self, output: &Output) -> Option<Mode> {
        if self.outputs.contains(output) {
            output.current_mode()
        } else {
            output.preferred_mode()
        }
    }

    /// What the layout saved for the connected monitors changes of them, if one is saved.
    /// What of it no longer fits is left as it is: a mode the monitor no longer has, or a
    /// scale or a place of no use. A layout that enables no monitor isn't restored.
    fn plan_saved_layout(&self) -> Option<Restore> {
        let connected = self.connected_monitors();
        let saved = self.saved_layouts.for_monitors(&connected)?;
        let mut primary = None;
        let mut changes = Vec::new();
        for (output, monitor) in saved {
            let Some(shows) = monitor.shows else {
                changes.push(OutputChange { output, to: None });
                continue;
            };
            let saved_mode = shows.mode.and_then(|mode| {
                let mode = Mode {
                    size: (mode.width, mode.height).into(),
                    refresh: mode.refresh,
                };
                matching_mode(&output.modes(), mode)
            });
            let Some(mode) = saved_mode.or_else(|| self.mode_without_a_saved_one(&output)) else {
                continue;
            };
            let place_of_use =
                SETTABLE_COORDINATES.contains(&shows.x) && SETTABLE_COORDINATES.contains(&shows.y);
            let settings = OutputSettings {
                mode,
                position: place_of_use.then(|| (shows.x, shows.y).into()),
                scale: Some(shows.scale).filter(|scale| SETTABLE_SCALES.contains(scale)),
            };
            if monitor.primary {
                primary = Some(output.clone());
            }
            changes.push(OutputChange {
                output,
                to: Some(settings),
            });
        }
        let enables_one = changes.iter().any(|change| change.to.is_some());
        enables_one.then_some(Restore { changes, primary })
    }
}

impl State {
    /// Restores the layout saved for the connected monitors, if a monitor came or went
    /// since the last time and one is saved for those there are now. The backends call it
    /// once they've added and removed what a scan found, so the first frame of a Session
    /// and of a monitor plugged in has it. Without one, the outputs stay as they are: a
    /// new one at Camino's defaults (51).
    pub(crate) fn restore_saved_layout(&mut self) {
        if !self.camino.monitors_changed {
            return;
        }
        let Some(Restore { changes, primary }) = self.camino.plan_saved_layout() else {
            self.camino.monitors_changed = false;
            return;
        };
        // Not now, as on a VT that's switched away: the next scan of the monitors tries
        // again.
        if !self.backend.can_change_outputs(&changes) {
            tracing::debug!("the backend can't show the saved layout yet");
            return;
        }
        self.camino.monitors_changed = false;
        // First, since healing holds the primary output still (82).
        let choice_before = self.camino.primary_choice.clone();
        if let Some(primary) = primary {
            self.camino.restore_primary_choice(&primary);
        }
        if self.apply_output_changes(&changes) {
            return;
        }
        // A mode the monitor lists and the backend can't show: the rest of the layout is
        // still restored, in the modes the outputs show, and a disabled monitor's
        // preferred one.
        let in_other_modes: Vec<OutputChange> = changes
            .into_iter()
            .map(|mut change| {
                if let Some(settings) = &mut change.to
                    && let Some(mode) = self.camino.mode_without_a_saved_one(&change.output)
                {
                    settings.mode = mode;
                }
                change
            })
            .collect();
        if !self.apply_output_changes(&in_other_modes) {
            tracing::warn!("can't restore the saved layout: the backend can't show it");
            self.camino.primary_choice = choice_before;
            self.camino.tell_policy_the_primary_output();
        }
    }
}

#[cfg(test)]
mod tests;
