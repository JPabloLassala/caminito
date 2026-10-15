//! Window icons (07, 30): icons found by the freedesktop icon theme spec, and the icons
//! of desktop entries. Frames and the Window switcher (108) use them now;
//! wlr-foreign-toplevel later.

use std::collections::{HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};

use caminito_theme::ThemeDirs;
use caminito_theme::cairo::ImageSurface;
use caminito_theme::gdk_pixbuf::Pixbuf;

/// The icon a window gets when nothing better is found.
pub const FALLBACK_ICON: &str = "application-x-executable";

/// The theme every theme falls back to, looked in last.
const HICOLOR: &str = "hicolor";

/// Icon file extensions, in the spec's order of preference.
const EXTENSIONS: [&str; 3] = ["png", "svg", "xpm"];

/// Finds icons by the freedesktop icon theme spec, and desktop entries' icons, in the
/// XDG data dirs.
#[derive(Debug)]
pub struct IconLookup {
    data_dirs: Vec<PathBuf>,
    /// The spec's base directories: `$HOME/.icons`, each `<data_dir>/icons`, then each
    /// `<data_dir>/pixmaps`.
    base_dirs: Vec<PathBuf>,
    theme: String,
    /// Each theme looked for, and `None` for one that isn't installed.
    themes: HashMap<String, Option<IconTheme>>,
    /// The themes an icon is looked for in, in order, for the theme in use.
    chain: Option<Vec<String>>,
    desktop_icons: HashMap<String, Option<String>>,
    /// Each icon file loaded, by its path and its size in device pixels, and `None` for
    /// one that couldn't be: an icon's decoded once, not at each label or frame drawn.
    loaded: HashMap<(PathBuf, i32), Option<ImageSurface>>,
}

impl IconLookup {
    /// `data_dirs` in XDG order, data home first. Icon dirs are `$HOME/.icons` (if `home`
    /// is given), then each `<data_dir>/icons`, and the spec's last resort is each
    /// `<data_dir>/pixmaps`.
    pub fn new(data_dirs: Vec<PathBuf>, home: Option<PathBuf>, theme: &str) -> Self {
        let mut base_dirs: Vec<PathBuf> =
            home.map(|home| home.join(".icons")).into_iter().collect();
        base_dirs.extend(data_dirs.iter().map(|dir| dir.join("icons")));
        base_dirs.extend(data_dirs.iter().map(|dir| dir.join("pixmaps")));
        IconLookup {
            data_dirs,
            base_dirs,
            theme: theme.to_owned(),
            themes: HashMap::new(),
            chain: None,
            desktop_icons: HashMap::new(),
            loaded: HashMap::new(),
        }
    }

    /// The XDG data dirs and `$HOME` of the environment.
    pub fn from_env(theme: &str) -> Self {
        let home = env::var_os("HOME")
            .filter(|home| !home.is_empty())
            .map(PathBuf::from);
        IconLookup::new(ThemeDirs::from_env().dirs().to_vec(), home, theme)
    }

    pub fn theme(&self) -> &str {
        &self.theme
    }

    /// Changes the theme. The themes are read again, so one installed since is found.
    pub fn set_theme(&mut self, theme: &str) {
        self.theme = theme.to_owned();
        self.themes.clear();
        self.chain = None;
        self.loaded.clear();
    }

    /// The icon file `path` at `size` by `size` device pixels, as [`load_icon`] loads it,
    /// from what an earlier load decoded if it's loaded already.
    pub fn load(&mut self, path: &Path, size: i32) -> Option<ImageSurface> {
        let key = (path.to_owned(), size);
        let loaded = self
            .loaded
            .entry(key)
            .or_insert_with(|| load_icon(path, size));
        loaded.clone()
    }

    /// The file for icon `name` at `size` logical px times `scale`, by the spec's
    /// `FindIcon`: in the theme, the themes it inherits depth first, `hicolor`, then
    /// unthemed icons in the base dirs. An absolute path that exists is its own file.
    pub fn find_icon(&mut self, name: &str, size: i32, scale: i32) -> Option<PathBuf> {
        let path = Path::new(name);
        if path.is_absolute() {
            return path.is_file().then(|| path.to_owned());
        }
        // Desktop entries shouldn't name the extension, but some do, and GTK allows it.
        let name = EXTENSIONS
            .iter()
            .find_map(|ext| name.strip_suffix(&format!(".{ext}")[..]))
            .unwrap_or(name);
        if name.is_empty() || name.contains('/') {
            return None;
        }
        let chain = self.chain();
        chain
            .iter()
            .filter_map(|theme| self.themes.get(theme)?.as_ref())
            .find_map(|theme| theme.lookup(name, size, scale))
            .or_else(|| self.lookup_unthemed(name))
    }

    /// The `Icon` of the desktop entry for `app_id`: `applications/<app_id>.desktop` (or
    /// the lowercase app_id) in data-dir order, with the spec's `-` to `/` subdirectories,
    /// else the first entry whose `StartupWMClass` is app_id, ignoring case.
    pub fn desktop_icon(&mut self, app_id: &str) -> Option<String> {
        if let Some(icon) = self.desktop_icons.get(app_id) {
            return icon.clone();
        }
        let icon = self.find_desktop_icon(app_id);
        self.desktop_icons.insert(app_id.to_owned(), icon.clone());
        icon
    }

    /// A window's icon file when it has no xdg-toplevel-icon: its desktop entry's icon,
    /// else `application-x-executable`.
    pub fn window_icon(&mut self, app_id: Option<&str>, size: i32, scale: i32) -> Option<PathBuf> {
        app_id
            .filter(|app_id| !app_id.is_empty())
            .and_then(|app_id| self.desktop_icon(app_id))
            .and_then(|icon| self.find_icon(&icon, size, scale))
            .or_else(|| self.find_icon(FALLBACK_ICON, size, scale))
    }

    /// The theme in use, what it inherits depth first without repeats, then `hicolor`.
    /// Themes that aren't installed are left out.
    fn chain(&mut self) -> Vec<String> {
        if let Some(chain) = &self.chain {
            return chain.clone();
        }
        let mut chain = Vec::new();
        let mut seen = HashSet::new();
        let theme = self.theme.clone();
        self.walk(&theme, &mut chain, &mut seen);
        self.walk(HICOLOR, &mut chain, &mut seen);
        self.chain = Some(chain.clone());
        chain
    }

    fn walk(&mut self, name: &str, chain: &mut Vec<String>, seen: &mut HashSet<String>) {
        if !seen.insert(name.to_owned()) {
            return;
        }
        let Some(theme) = self.icon_theme(name) else {
            return;
        };
        let parents = theme.inherits.clone();
        chain.push(name.to_owned());
        for parent in parents {
            self.walk(&parent, chain, seen);
        }
    }

    fn icon_theme(&mut self, name: &str) -> Option<&IconTheme> {
        if !self.themes.contains_key(name) {
            let theme = IconTheme::load(name, &self.base_dirs);
            self.themes.insert(name.to_owned(), theme);
        }
        self.themes.get(name)?.as_ref()
    }

    /// The spec's `LookupFallbackIcon`.
    fn lookup_unthemed(&self, name: &str) -> Option<PathBuf> {
        self.base_dirs.iter().find_map(|dir| {
            EXTENSIONS
                .iter()
                .map(|ext| dir.join(format!("{name}.{ext}")))
                .find(|path| path.is_file())
        })
    }

    fn find_desktop_icon(&self, app_id: &str) -> Option<String> {
        let lowercase = app_id.to_lowercase();
        let ids = std::iter::once(app_id).chain((lowercase != app_id).then_some(&lowercase[..]));
        for id in ids {
            let found = self
                .applications_dirs()
                .find_map(|dir| desktop_file_by_id(&dir, id));
            if let Some(path) = found {
                return desktop_entry_icon(&path);
            }
        }
        self.desktop_icon_by_wm_class(app_id)
    }

    fn desktop_icon_by_wm_class(&self, app_id: &str) -> Option<String> {
        let app_id = app_id.to_lowercase();
        // An entry is shadowed by one with the same ID in an earlier dir.
        let mut seen = HashSet::new();
        for dir in self.applications_dirs() {
            let mut entries = Vec::new();
            desktop_files(&dir, "", &mut entries);
            for (id, path) in entries {
                if !seen.insert(id) {
                    continue;
                }
                let Some(entry) = read_desktop_entry(&path) else {
                    continue;
                };
                let matches = entry
                    .get("StartupWMClass")
                    .is_some_and(|class| class.to_lowercase() == app_id);
                if matches {
                    return entry.get("Icon").cloned();
                }
            }
        }
        None
    }

    fn applications_dirs(&self) -> impl Iterator<Item = PathBuf> + '_ {
        self.data_dirs.iter().map(|dir| dir.join("applications"))
    }
}

/// Loads an icon file at `size` by `size` device pixels, keeping its aspect, into a
/// surface. Logs a warning and gives `None` if it can't.
pub fn load_icon(path: &Path, size: i32) -> Option<ImageSurface> {
    let surface = Pixbuf::from_file_at_size(path, size, size)
        .map_err(|err| err.to_string())
        .and_then(|pixbuf| {
            caminito_theme::surface_from_pixbuf(&pixbuf).map_err(|err| err.to_string())
        });
    match surface {
        Ok(surface) => Some(surface),
        Err(err) => {
            tracing::warn!("can't load the icon {}: {err}", path.display());
            None
        }
    }
}

/// An installed icon theme, from its `index.theme`.
#[derive(Debug)]
struct IconTheme {
    /// `<base_dir>/<theme>` for each base dir that has it.
    roots: Vec<PathBuf>,
    directories: Vec<Directory>,
    inherits: Vec<String>,
}

impl IconTheme {
    /// The theme whose `index.theme` is first in the base dirs, or `None` if none has
    /// one that reads.
    fn load(name: &str, base_dirs: &[PathBuf]) -> Option<IconTheme> {
        let roots: Vec<PathBuf> = base_dirs
            .iter()
            .map(|dir| dir.join(name))
            .filter(|root| root.is_dir())
            .collect();
        let index = roots.iter().find_map(|root| {
            let text = std::fs::read_to_string(root.join("index.theme")).ok()?;
            Some(parse_ini(&text))
        })?;
        let theme = index.get("Icon Theme")?;
        let list = |key: &str| -> Vec<String> {
            theme
                .get(key)
                .map(|value| {
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|item| !item.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default()
        };
        let mut names = list("Directories");
        names.extend(list("ScaledDirectories"));
        let mut seen = HashSet::new();
        let directories = names
            .into_iter()
            .filter(|name| seen.insert(name.clone()))
            .filter_map(|name| {
                let group = index.get(&name)?;
                Directory::parse(name, group)
            })
            .collect();
        Some(IconTheme {
            roots,
            directories,
            inherits: list("Inherits"),
        })
    }

    /// The spec's `LookupIcon`: the first file in a directory that matches the size,
    /// else the file in the directory closest to it.
    fn lookup(&self, name: &str, size: i32, scale: i32) -> Option<PathBuf> {
        let mut closest: Option<(i32, PathBuf)> = None;
        for directory in &self.directories {
            let matches = directory.matches_size(size, scale);
            for root in &self.roots {
                for ext in EXTENSIONS {
                    let path = root.join(&directory.name).join(format!("{name}.{ext}"));
                    if !path.is_file() {
                        continue;
                    }
                    if matches {
                        return Some(path);
                    }
                    let distance = directory.size_distance(size, scale);
                    if closest.as_ref().is_none_or(|(best, _)| distance < *best) {
                        closest = Some((distance, path));
                    }
                }
            }
        }
        closest.map(|(_, path)| path)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirectoryType {
    Fixed,
    Scalable,
    Threshold,
}

/// One of a theme's icon directories.
#[derive(Debug)]
struct Directory {
    name: String,
    size: i32,
    scale: i32,
    kind: DirectoryType,
    min_size: i32,
    max_size: i32,
    threshold: i32,
}

impl Directory {
    /// With the spec's defaults. `None` without a valid `Size`.
    fn parse(name: String, group: &HashMap<String, String>) -> Option<Directory> {
        let int = |key: &str| group.get(key).and_then(|value| value.parse::<i32>().ok());
        let size = int("Size")?;
        let kind = match group.get("Type").map(String::as_str) {
            Some("Fixed") => DirectoryType::Fixed,
            Some("Scalable") => DirectoryType::Scalable,
            _ => DirectoryType::Threshold,
        };
        Some(Directory {
            name,
            size,
            scale: int("Scale").unwrap_or(1),
            kind,
            min_size: int("MinSize").unwrap_or(size),
            max_size: int("MaxSize").unwrap_or(size),
            threshold: int("Threshold").unwrap_or(2),
        })
    }

    /// The spec's `DirectoryMatchesSize`.
    fn matches_size(&self, size: i32, scale: i32) -> bool {
        if self.scale != scale {
            return false;
        }
        match self.kind {
            DirectoryType::Fixed => self.size == size,
            DirectoryType::Scalable => (self.min_size..=self.max_size).contains(&size),
            DirectoryType::Threshold => {
                (self.size - self.threshold..=self.size + self.threshold).contains(&size)
            }
        }
    }

    /// The spec's `DirectorySizeDistance`, in device pixels. For `Threshold` it measures
    /// from the threshold's ends, as GTK does, where the spec's text uses `MinSize` and
    /// `MaxSize`.
    fn size_distance(&self, size: i32, scale: i32) -> i32 {
        let wanted = size * scale;
        let (min, max) = match self.kind {
            DirectoryType::Fixed => (self.size, self.size),
            DirectoryType::Scalable => (self.min_size, self.max_size),
            DirectoryType::Threshold => (self.size - self.threshold, self.size + self.threshold),
        };
        let (min, max) = (min * self.scale, max * self.scale);
        if wanted < min {
            min - wanted
        } else if wanted > max {
            wanted - max
        } else {
            0
        }
    }
}

/// The desktop file with desktop-file ID `<id>.desktop` under `dir`, where each `-` may
/// be a subdirectory's `/`.
fn desktop_file_by_id(dir: &Path, id: &str) -> Option<PathBuf> {
    let path = dir.join(format!("{id}.desktop"));
    if path.is_file() {
        return Some(path);
    }
    id.match_indices('-').find_map(|(i, _)| {
        let subdir = dir.join(&id[..i]);
        if subdir.is_dir() {
            desktop_file_by_id(&subdir, &id[i + 1..])
        } else {
            None
        }
    })
}

/// Every desktop file under `dir` with its desktop-file ID, sorted within each directory
/// so the first match doesn't depend on the file system.
fn desktop_files(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = read.filter_map(|entry| Some(entry.ok()?.path())).collect();
    entries.sort();
    for path in entries {
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if path.is_dir() {
            desktop_files(&path, &format!("{prefix}{file_name}-"), out);
        } else if let Some(stem) = file_name.strip_suffix(".desktop") {
            out.push((format!("{prefix}{stem}"), path));
        }
    }
}

fn desktop_entry_icon(path: &Path) -> Option<String> {
    read_desktop_entry(path)?.remove("Icon")
}

/// The `[Desktop Entry]` group of a desktop file.
fn read_desktop_entry(path: &Path) -> Option<HashMap<String, String>> {
    let text = std::fs::read_to_string(path).ok()?;
    parse_ini(&text).remove("Desktop Entry")
}

/// The groups of a desktop-entry-style file: `[group]` headers, `key=value` lines and `#`
/// comments. Localised `key[locale]` keys are left out, and the first of a repeated key
/// or group wins.
fn parse_ini(text: &str) -> HashMap<String, HashMap<String, String>> {
    let mut groups: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut group: Option<String> = None;
    let mut skipping = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            skipping = groups.contains_key(name);
            groups.entry(name.to_owned()).or_default();
            group = Some(name.to_owned());
            continue;
        }
        let (Some(group), false) = (&group, skipping) else {
            continue;
        };
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.contains('[') {
            continue;
        }
        groups
            .get_mut(group)
            .expect("the group was added with its header")
            .entry(key.to_owned())
            .or_insert_with(|| value.trim().to_owned());
    }
    groups
}

#[cfg(test)]
mod tests;
