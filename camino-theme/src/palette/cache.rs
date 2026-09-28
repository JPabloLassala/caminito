//! Camino's on-disk cache of the palettes the helper reported (07, 28), so frames drawn
//! before the helper replies already have the right colours.
//!
//! Each GTK theme and variant is a file, `<dir>/<GTK theme>/default` or `.../dark`:
//!
//! ```text
//! camino-palette-cache 1
//! state <state> <background r g b a> <text r g b a>   (all eight states)
//! named <name> <r g b a>
//! end
//! ```
//!
//! The lines are those of the helper's stdout ([`super::stream`]).

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::{ColorsBuilder, PaletteColors, write_colors};
use crate::Variant;

/// The version header this cache reads and writes.
const HEADER: &str = "camino-palette-cache 1";

/// The last palette reported for each GTK theme and variant, one file each.
#[derive(Debug, Clone)]
pub struct PaletteCache {
    dir: PathBuf,
}

impl PaletteCache {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        PaletteCache { dir: dir.into() }
    }

    /// `$XDG_CACHE_HOME/camino/palette`, or `~/.cache/camino/palette`. None without
    /// either variable.
    pub fn from_env() -> Option<Self> {
        let absolute = |name| {
            std::env::var_os(name)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
        };
        let cache_home =
            absolute("XDG_CACHE_HOME").or_else(|| Some(absolute("HOME")?.join(".cache")))?;
        Some(PaletteCache::new(cache_home.join("camino/palette")))
    }

    /// The file a GTK theme's variant is kept in. None for a name that isn't a single
    /// path component, which is never cached.
    pub fn path(&self, gtk_theme: &str, variant: Variant) -> Option<PathBuf> {
        let usable = !gtk_theme.is_empty()
            && gtk_theme != "."
            && gtk_theme != ".."
            && !gtk_theme.contains(['/', '\0']);
        usable.then(|| self.dir.join(gtk_theme).join(variant_name(variant)))
    }

    /// Replaces the variant's file, atomically.
    pub fn store(
        &self,
        gtk_theme: &str,
        variant: Variant,
        colors: &PaletteColors,
    ) -> io::Result<()> {
        let Some(path) = self.path(gtk_theme, variant) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{gtk_theme:?} isn't a theme name that can be cached"),
            ));
        };
        let dir = path.parent().expect("the path has the theme's directory");
        fs::create_dir_all(dir)?;
        let temporary = dir.join(format!(".{}.{}", variant_name(variant), std::process::id()));
        let mut file = fs::File::create(&temporary)?;
        file.write_all(format_cache(colors).as_bytes())?;
        file.sync_all()?;
        fs::rename(&temporary, &path)
    }

    /// The variant's cached colours, None if there are none.
    pub fn load(
        &self,
        gtk_theme: &str,
        variant: Variant,
    ) -> Result<Option<PaletteColors>, PaletteCacheError> {
        let Some(path) = self.path(gtk_theme, variant) else {
            return Ok(None);
        };
        read(&path)
    }
}

fn read(path: &Path) -> Result<Option<PaletteColors>, PaletteCacheError> {
    match fs::read_to_string(path) {
        Ok(text) => parse_cache(&text).map(Some),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(PaletteCacheError::Io(err.to_string())),
    }
}

fn variant_name(variant: Variant) -> &'static str {
    match variant {
        Variant::Default => "default",
        Variant::Dark => "dark",
    }
}

fn format_cache(colors: &PaletteColors) -> String {
    let mut out = format!("{HEADER}\n");
    write_colors(&mut out, colors);
    out.push_str("end\n");
    out
}

fn parse_cache(text: &str) -> Result<PaletteColors, PaletteCacheError> {
    let mut lines = text.split_inclusive('\n');
    let header = lines.next().unwrap_or_default();
    if header.strip_suffix('\n') != Some(HEADER) {
        return Err(PaletteCacheError::UnknownVersion {
            header: header.trim_end_matches('\n').to_owned(),
        });
    }
    let mut colors = ColorsBuilder::default();
    for line in lines {
        let Some(line) = line.strip_suffix('\n') else {
            return Err(PaletteCacheError::Truncated);
        };
        if line == "end" {
            return colors.build().ok_or(PaletteCacheError::MissingStates);
        }
        let (keyword, rest) = line.split_once(' ').unwrap_or((line, ""));
        if !colors.line(keyword, rest) {
            return Err(PaletteCacheError::Malformed {
                line: line.to_owned(),
            });
        }
    }
    Err(PaletteCacheError::Truncated)
}

/// Why a cache file wasn't read.
#[derive(Debug, Clone, PartialEq)]
pub enum PaletteCacheError {
    /// The first line isn't this version's header.
    UnknownVersion {
        header: String,
    },
    Malformed {
        line: String,
    },
    MissingStates,
    /// The file ends before its `end`.
    Truncated,
    Io(String),
}

impl std::fmt::Display for PaletteCacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaletteCacheError::UnknownVersion { header } => {
                write!(f, "unknown palette cache header {header:?}")
            }
            PaletteCacheError::Malformed { line } => write!(f, "malformed line {line:?}"),
            PaletteCacheError::MissingStates => f.write_str("missing states"),
            PaletteCacheError::Truncated => f.write_str("cut off before its end"),
            PaletteCacheError::Io(err) => f.write_str(err),
        }
    }
}

impl std::error::Error for PaletteCacheError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{GtkState, Rgba};
    use crate::palette::StateColors;

    fn colors(level: u8) -> PaletteColors {
        let states = GtkState::ALL.map(|state| StateColors {
            background: Rgba::from_rgb8(level, level, state as u8),
            text: Rgba {
                red: 1.0 / 3.0,
                green: 0.0,
                blue: 1.0,
                alpha: 0.18,
            },
        });
        let named = [("wm_bg".to_owned(), Rgba::from_rgb8(1, 2, level))].into();
        PaletteColors::new(states, named)
    }

    #[test]
    fn stored_colours_load_back_for_their_theme_and_variant() {
        let dir = tempfile::tempdir().unwrap();
        let cache = PaletteCache::new(dir.path().join("camino/palette"));
        cache
            .store("Adwaita", Variant::Default, &colors(1))
            .unwrap();
        cache.store("Adwaita", Variant::Dark, &colors(2)).unwrap();
        cache.store("Menta", Variant::Default, &colors(3)).unwrap();

        assert_eq!(cache.load("Adwaita", Variant::Default), Ok(Some(colors(1))));
        assert_eq!(cache.load("Adwaita", Variant::Dark), Ok(Some(colors(2))));
        assert_eq!(cache.load("Menta", Variant::Default), Ok(Some(colors(3))));
        assert_eq!(cache.load("Menta", Variant::Dark), Ok(None));
    }

    fn cache_with(text: &str) -> (tempfile::TempDir, PaletteCache) {
        let dir = tempfile::tempdir().unwrap();
        let cache = PaletteCache::new(dir.path());
        cache
            .store("Adwaita", Variant::Default, &colors(1))
            .unwrap();
        let path = cache.path("Adwaita", Variant::Default).unwrap();
        fs::write(path, text).unwrap();
        (dir, cache)
    }

    #[test]
    fn a_file_cut_off_before_its_end_is_truncated() {
        let whole = format_cache(&colors(1));
        let (_dir, cache) = cache_with(whole.strip_suffix("end\n").unwrap());
        assert_eq!(
            cache.load("Adwaita", Variant::Default),
            Err(PaletteCacheError::Truncated)
        );
    }

    #[test]
    fn a_file_cut_off_in_a_line_is_truncated() {
        let whole = format_cache(&colors(1));
        let cut = &whole[..whole.find("state focused").unwrap() + 20];
        let (_dir, cache) = cache_with(cut);
        assert_eq!(
            cache.load("Adwaita", Variant::Default),
            Err(PaletteCacheError::Truncated)
        );
    }

    #[test]
    fn a_file_of_another_version_is_refused() {
        let whole = format_cache(&colors(1));
        let (_dir, cache) =
            cache_with(&whole.replace("camino-palette-cache 1", "camino-palette-cache 2"));
        assert_eq!(
            cache.load("Adwaita", Variant::Default),
            Err(PaletteCacheError::UnknownVersion {
                header: "camino-palette-cache 2".to_owned()
            })
        );
    }

    #[test]
    fn a_file_missing_a_state_is_refused() {
        let whole = format_cache(&colors(1));
        let start = whole.find("state active").unwrap();
        let end = start + whole[start..].find('\n').unwrap() + 1;
        let (_dir, cache) = cache_with(&format!("{}{}", &whole[..start], &whole[end..]));
        assert_eq!(
            cache.load("Adwaita", Variant::Default),
            Err(PaletteCacheError::MissingStates)
        );
    }

    #[test]
    fn a_theme_name_that_isnt_one_path_component_is_never_cached() {
        let dir = tempfile::tempdir().unwrap();
        let cache = PaletteCache::new(dir.path().join("palette"));
        for name in ["", ".", "..", "../Adwaita", "a/b"] {
            assert!(
                cache.store(name, Variant::Default, &colors(1)).is_err(),
                "{name:?}"
            );
            assert_eq!(cache.load(name, Variant::Default), Ok(None), "{name:?}");
        }
        assert!(!dir.path().join("Adwaita").exists());
    }
}
