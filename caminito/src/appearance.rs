//! The Colour scheme and the two Appearances (ADR 0059, CMN-327): Caminito owns them in
//! `org.mate.Caminito`, writes the Appearance the Colour scheme picks into MATE's theme
//! keys, and saves a theme change made anywhere else into the Appearance in use.
//!
//! [`Appearances::reconcile`] is pure: it takes a reading of Caminito's keys and MATE's, and
//! returns the keys to write. `State` runs it on every reading and on a new palette, and
//! writes what it returns through the settings thread.

#[cfg(test)]
mod tests;

use std::io;
use std::path::{Path, PathBuf};

use caminito_theme::{GtkState, PaletteColors, Rgba};

use crate::settings::Value;
use crate::settings::translate::{
    AppearanceKeys, CAMINITO, ColourScheme, INTERFACE, MARCO_GENERAL, Themes,
};

/// One of the three themes an Appearance holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Gtk,
    Marco,
    Icons,
}

const KINDS: [Kind; 3] = [Kind::Gtk, Kind::Marco, Kind::Icons];

impl Kind {
    /// MATE's key for the theme.
    fn mate_key(self) -> (&'static str, &'static str) {
        match self {
            Kind::Gtk => (INTERFACE, "gtk-theme"),
            Kind::Marco => (MARCO_GENERAL, "theme"),
            Kind::Icons => (INTERFACE, "icon-theme"),
        }
    }

    /// Caminito's key for the theme of `scheme`'s Appearance.
    fn caminito_key(self, scheme: ColourScheme) -> String {
        let scheme = match scheme {
            ColourScheme::Light => "light",
            ColourScheme::Dark => "dark",
        };
        let kind = match self {
            Kind::Gtk => "gtk",
            Kind::Marco => "marco",
            Kind::Icons => "icon",
        };
        format!("{scheme}-{kind}-theme")
    }

    fn of(self, themes: &Themes) -> &str {
        match self {
            Kind::Gtk => &themes.gtk,
            Kind::Marco => &themes.marco,
            Kind::Icons => &themes.icons,
        }
    }

    fn of_mut(self, themes: &mut Themes) -> &mut String {
        match self {
            Kind::Gtk => &mut themes.gtk,
            Kind::Marco => &mut themes.marco,
            Kind::Icons => &mut themes.icons,
        }
    }

    /// The known pairs of light and dark themes of this kind. A theme may be in more than
    /// one row; the first row it's in gives its pair.
    fn pairs(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Kind::Gtk => &[
                ("Menta", "BlackMATE"),
                ("TraditionalOk", "BlackMATE"),
                ("Adwaita", "Adwaita-dark"),
                ("Breeze", "Breeze-Dark"),
                ("Yaru", "Yaru-dark"),
                ("Arc", "Arc-Dark"),
                ("Materia", "Materia-dark"),
                ("Mint-Y", "Mint-Y-Dark"),
            ],
            Kind::Marco => &[
                ("Menta", "BlackMATE"),
                ("TraditionalOk", "BlackMATE"),
                ("Breeze", "Breeze-Dark"),
                ("Yaru", "Yaru-dark"),
                ("Arc", "Arc-Dark"),
                ("Materia", "Materia-dark"),
                ("Mint-Y", "Mint-Y-Dark"),
            ],
            Kind::Icons => &[
                ("breeze", "breeze-dark"),
                ("Yaru", "Yaru-dark"),
                ("Papirus", "Papirus-Dark"),
                ("Papirus-Light", "Papirus-Dark"),
            ],
        }
    }

    /// The theme `theme` pairs with in `scheme`'s Appearance: its pair from the table, or
    /// itself with none. A dark Appearance that's the light one's GTK theme is drawn with
    /// `prefer-dark`.
    fn paired(self, theme: &str, scheme: ColourScheme) -> String {
        let pairs = self.pairs().iter();
        let found = match scheme {
            ColourScheme::Dark => pairs
                .clone()
                .find(|(light, _)| *light == theme)
                .map(|(_, dark)| dark),
            ColourScheme::Light => pairs
                .clone()
                .find(|(_, dark)| *dark == theme)
                .map(|(light, _)| light),
        };
        found.map_or(theme, |found| found).to_owned()
    }
}

/// A theme's Colour scheme as the table of known pairs has it: dark if it's the dark one
/// of a pair, light if it's the light one, and `None` if it's in no pair.
pub fn known_scheme(gtk_theme: &str) -> Option<ColourScheme> {
    Kind::Gtk.pairs().iter().find_map(|(light, dark)| {
        if *dark == gtk_theme {
            Some(ColourScheme::Dark)
        } else if *light == gtk_theme {
            Some(ColourScheme::Light)
        } else {
            None
        }
    })
}

/// ADR 0005's luminance test on a GTK theme's default palette: the theme is dark when its
/// window's background is darker than its text.
pub fn palette_is_dark(colors: &PaletteColors) -> bool {
    luminance(colors.background(GtkState::Normal)) < luminance(colors.text(GtkState::Normal))
}

/// WCAG 2's relative luminance, from sRGB, as msd's mirror computes it.
fn luminance(color: Rgba) -> f64 {
    let linear = |c: f64| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.red) + 0.7152 * linear(color.green) + 0.0722 * linear(color.blue)
}

/// A key to write.
#[derive(Debug, Clone, PartialEq)]
pub struct Write {
    pub schema: &'static str,
    pub key: String,
    pub value: Value,
}

/// What a reading calls for.
#[derive(Debug, Default, PartialEq)]
pub struct Outcome {
    /// The keys to write, Caminito's and MATE's.
    pub writes: Vec<Write>,
    /// GTK3's `gtk-application-prefer-dark-theme` to write into its `settings.ini`, when it
    /// changed (CMN-326).
    pub prefer_dark: Option<bool>,
}

/// A theme Caminito wrote into MATE's key, which a reading may not show yet.
#[derive(Debug, Clone)]
struct Pending {
    /// The value written.
    to: String,
    /// The values the key had before, which a reading may still show.
    stale: Vec<String>,
}

/// What Caminito remembers between readings.
#[derive(Debug, Default)]
pub struct Appearances {
    /// The Colour scheme of the last reading, so a change of it is told from the first.
    last_scheme: Option<ColourScheme>,
    /// For each [`Kind`], the theme written into MATE's key and not read back yet.
    pending: [Option<Pending>; 3],
    /// `prefer-dark` as last written into `settings.ini`.
    prefer_dark: Option<bool>,
}

impl Appearances {
    /// Works out what to write for a reading of Caminito's keys and of the themes MATE's
    /// keys have. `is_dark` tells whether a GTK theme is dark, or `None` while that isn't
    /// known yet; it's asked only on the first start, while the Colour scheme is unset.
    pub fn reconcile(
        &mut self,
        keys: &AppearanceKeys,
        applied: &Themes,
        is_dark: impl FnOnce(&str) -> Option<bool>,
    ) -> Outcome {
        let seeding = keys.colour_scheme.is_none();
        let scheme = match keys.colour_scheme {
            Some(scheme) => scheme,
            None => match is_dark(&applied.gtk) {
                Some(true) => ColourScheme::Dark,
                Some(false) => ColourScheme::Light,
                None => return Outcome::default(),
            },
        };
        let changed = self.last_scheme.is_some_and(|last| last != scheme);
        self.last_scheme = Some(scheme);
        let other = match scheme {
            ColourScheme::Light => ColourScheme::Dark,
            ColourScheme::Dark => ColourScheme::Light,
        };

        let mut light = keys.light.clone();
        let mut dark = keys.dark.clone();
        let mut writes = Vec::new();
        for (index, kind) in KINDS.into_iter().enumerate() {
            let shown = kind.of(applied);
            let (active, inactive) = match scheme {
                ColourScheme::Light => (kind.of_mut(&mut light), kind.of_mut(&mut dark)),
                ColourScheme::Dark => (kind.of_mut(&mut dark), kind.of_mut(&mut light)),
            };
            // The first start makes what's on screen the Appearance in use.
            if seeding || (active.is_empty() && inactive.is_empty()) {
                *active = shown.to_owned();
            } else if active.is_empty() {
                *active = kind.paired(inactive, scheme);
            }
            if inactive.is_empty() {
                *inactive = kind.paired(active, other);
            }

            let stale = match &self.pending[index] {
                Some(pending) if pending.to == shown => {
                    self.pending[index] = None;
                    false
                }
                Some(pending) if pending.stale.iter().any(|s| s == shown) => true,
                Some(_) => {
                    self.pending[index] = None;
                    false
                }
                None => false,
            };
            if changed {
                let previous = self.pending[index].take();
                let effective = previous
                    .as_ref()
                    .map_or(shown, |p| p.to.as_str())
                    .to_owned();
                if *active != effective {
                    let (schema, key) = kind.mate_key();
                    writes.push(Write {
                        schema,
                        key: key.into(),
                        value: Value::Str(active.clone()),
                    });
                    let mut stale = previous.map(|p| p.stale).unwrap_or_default();
                    for value in [shown, effective.as_str()] {
                        if !stale.iter().any(|s| s == value) {
                            stale.push(value.to_owned());
                        }
                    }
                    self.pending[index] = Some(Pending {
                        to: active.clone(),
                        stale,
                    });
                } else {
                    self.pending[index] = previous;
                }
            } else if !stale && *active != shown {
                // A theme picked anywhere else edits the Appearance in use.
                *active = shown.to_owned();
            }
        }

        let mut caminito = Vec::new();
        if seeding {
            let nick = match scheme {
                ColourScheme::Light => "light",
                ColourScheme::Dark => "dark",
            };
            caminito.push(Write {
                schema: CAMINITO,
                key: "colour-scheme".into(),
                value: Value::Str(nick.into()),
            });
        }
        for (scheme, themes, stored) in [
            (ColourScheme::Light, &light, &keys.light),
            (ColourScheme::Dark, &dark, &keys.dark),
        ] {
            for kind in KINDS {
                if kind.of(themes) != kind.of(stored) {
                    caminito.push(Write {
                        schema: CAMINITO,
                        key: kind.caminito_key(scheme),
                        value: Value::Str(kind.of(themes).to_owned()),
                    });
                }
            }
        }
        caminito.append(&mut writes);

        let prefer_dark = scheme == ColourScheme::Dark && dark.gtk == light.gtk;
        let prefer_dark = (self.prefer_dark != Some(prefer_dark)).then(|| {
            self.prefer_dark = Some(prefer_dark);
            prefer_dark
        });
        Outcome {
            writes: caminito,
            prefer_dark,
        }
    }
}

/// GTK3's user `settings.ini`, in `$XDG_CONFIG_HOME`, or `~/.config` without it.
pub fn gtk3_settings_path() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(config.join("gtk-3.0").join("settings.ini"))
}

const PREFER_DARK: &str = "gtk-application-prefer-dark-theme";

/// `text`, a `settings.ini`, with `gtk-application-prefer-dark-theme` set to `on` in its
/// `[Settings]` group, and every other line as it was (CMN-326).
pub fn with_prefer_dark(text: &str, on: bool) -> String {
    let line = format!("{PREFER_DARK}={on}");
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let group = |l: &str| l.trim_start().starts_with('[');
    let header = lines.iter().position(|l| l.trim() == "[Settings]");
    match header {
        Some(header) => {
            let end = lines[header + 1..]
                .iter()
                .position(|l| group(l))
                .map_or(lines.len(), |i| header + 1 + i);
            let existing = lines[header + 1..end].iter().position(|l| {
                l.split_once('=')
                    .is_some_and(|(key, _)| key.trim() == PREFER_DARK)
            });
            match existing {
                Some(i) => lines[header + 1 + i] = line,
                None => lines.insert(header + 1, line),
            }
        }
        None => {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push("[Settings]".into());
            lines.push(line);
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// Sets `gtk-application-prefer-dark-theme` in the `settings.ini` at `path`, which is
/// made if it's missing, and replaced whole so a GTK3 app starting meanwhile never reads
/// half of it.
pub fn write_prefer_dark(path: &Path, on: bool) -> io::Result<()> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err),
    };
    let new = with_prefer_dark(&text, on);
    if new == text {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temporary = path.with_extension("ini.caminito");
    std::fs::write(&temporary, new)?;
    std::fs::rename(&temporary, path)
}
