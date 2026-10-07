//! The GTK colours Marco themes refer to (07, ADR 0003).
//!
//! The palette helper reads them from GTK and reports them on its stdout ([`stream`]), and
//! Caminito keeps the last ones on disk ([`cache`]).

mod cache;
mod stream;

use std::collections::BTreeMap;

pub use cache::{PaletteCache, PaletteCacheError};
pub use stream::{
    PALETTE_STREAM_HEADER, PaletteReader, PaletteStreamError, ReportedPalette, format_report,
    names_request,
};

use crate::color::{GtkState, Rgba};

/// Which of the GTK theme's variants a window gets. Native windows always get the
/// default; XWayland windows with `_GTK_THEME_VARIANT=dark` get the dark one (07).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Variant {
    Default,
    Dark,
}

/// The GTK theme's colours, for both variants. Every colour a frame is drawn with comes
/// from here.
#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    pub default: PaletteColors,
    pub dark: PaletteColors,
}

impl Palette {
    /// The palette used before the helper has reported one and with nothing cached:
    /// GTK3's built-in Adwaita's `window.background`, with no named colours.
    pub fn neutral() -> Palette {
        let colors = |background: Rgba, text: Rgba, backdrop_text: Rgba| {
            let states = GtkState::ALL.map(|state| StateColors {
                background,
                text: if state == GtkState::Backdrop {
                    backdrop_text
                } else {
                    text
                },
            });
            PaletteColors::new(states, BTreeMap::new())
        };
        Palette {
            default: colors(
                Rgba::from_rgb8(0xf6, 0xf5, 0xf4),
                Rgba::from_rgb8(0x2e, 0x34, 0x36),
                Rgba::from_rgb8(0x92, 0x95, 0x95),
            ),
            dark: colors(
                Rgba::from_rgb8(0x35, 0x35, 0x35),
                Rgba::from_rgb8(0xee, 0xee, 0xec),
                Rgba::from_rgb8(0x91, 0x91, 0x90),
            ),
        }
    }

    pub fn variant(&self, variant: Variant) -> &PaletteColors {
        match variant {
            Variant::Default => &self.default,
            Variant::Dark => &self.dark,
        }
    }
}

/// One variant's colours: those of `window.background` in each state Marco maps, and the
/// theme's `@define-color` table.
#[derive(Debug, Clone, PartialEq)]
pub struct PaletteColors {
    states: [StateColors; GtkState::ALL.len()],
    named: BTreeMap<String, Rgba>,
}

/// `background-color` and `color` in one state. Marco's `bg` and `base` read the first,
/// and `fg` and `text` the second.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StateColors {
    pub background: Rgba,
    pub text: Rgba,
}

impl PaletteColors {
    /// `states` is in [`GtkState::ALL`]'s order.
    pub fn new(states: [StateColors; GtkState::ALL.len()], named: BTreeMap<String, Rgba>) -> Self {
        PaletteColors { states, named }
    }

    pub fn background(&self, state: GtkState) -> Rgba {
        self.states[state as usize].background
    }

    pub fn text(&self, state: GtkState) -> Rgba {
        self.states[state as usize].text
    }

    /// A `@define-color`, which `gtk:custom` reads.
    pub fn named(&self, name: &str) -> Option<Rgba> {
        self.named.get(name).copied()
    }
}

/// The `state` and `named` lines of one variant, which the helper's stdout and the cache
/// share.
#[derive(Debug, Default)]
struct ColorsBuilder {
    states: [Option<StateColors>; GtkState::ALL.len()],
    named: BTreeMap<String, Rgba>,
}

impl ColorsBuilder {
    /// Takes a `state` or `named` line, split after its keyword. False if it isn't one,
    /// or it repeats a state or name.
    fn line(&mut self, keyword: &str, rest: &str) -> bool {
        let mut fields = rest.split(' ');
        let Some(name) = fields.next().filter(|name| !name.is_empty()) else {
            return false;
        };
        let channels: Option<Vec<f64>> = fields.map(parse_channel).collect();
        match (keyword, channels.as_deref()) {
            ("state", Some(&[br, bg, bb, ba, tr, tg, tb, ta])) => {
                let Some(state) = GtkState::ALL.into_iter().find(|&s| state_name(s) == name) else {
                    return false;
                };
                let slot = &mut self.states[state as usize];
                if slot.is_some() {
                    return false;
                }
                *slot = Some(StateColors {
                    background: rgba(br, bg, bb, ba),
                    text: rgba(tr, tg, tb, ta),
                });
                true
            }
            ("named", Some(&[r, g, b, a])) => self
                .named
                .insert(name.to_owned(), rgba(r, g, b, a))
                .is_none(),
            _ => false,
        }
    }

    /// The colours, if every state was given.
    fn build(self) -> Option<PaletteColors> {
        let mut states = [StateColors {
            background: rgba(0.0, 0.0, 0.0, 0.0),
            text: rgba(0.0, 0.0, 0.0, 0.0),
        }; GtkState::ALL.len()];
        for (slot, given) in states.iter_mut().zip(self.states) {
            *slot = given?;
        }
        Some(PaletteColors::new(states, self.named))
    }
}

/// Writes a variant's `state` and `named` lines.
fn write_colors(out: &mut String, colors: &PaletteColors) {
    use std::fmt::Write;

    let channels = |c: Rgba| format!("{} {} {} {}", c.red, c.green, c.blue, c.alpha);
    for state in GtkState::ALL {
        let _ = writeln!(
            out,
            "state {} {} {}",
            state_name(state),
            channels(colors.background(state)),
            channels(colors.text(state)),
        );
    }
    for (name, color) in &colors.named {
        let _ = writeln!(out, "named {name} {}", channels(*color));
    }
}

/// A state's name in the formats: [`GtkState::name`] in lower case.
fn state_name(state: GtkState) -> String {
    state.name().to_ascii_lowercase()
}

/// A channel: a finite decimal from 0 to 1.
fn parse_channel(text: &str) -> Option<f64> {
    let value: f64 = text.parse().ok()?;
    (0.0..=1.0).contains(&value).then_some(value)
}

fn rgba(red: f64, green: f64, blue: f64, alpha: f64) -> Rgba {
    Rgba {
        red,
        green,
        blue,
        alpha,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grey(level: u8) -> Rgba {
        Rgba::from_rgb8(level, level, level)
    }

    #[test]
    fn colours_are_looked_up_by_variant_state_and_name() {
        let colors = |offset: u8| {
            let states = GtkState::ALL.map(|state| StateColors {
                background: grey(offset + state as u8),
                text: grey(offset + 100 + state as u8),
            });
            let named = [("wm_bg".to_owned(), grey(offset + 50))].into();
            PaletteColors::new(states, named)
        };
        let palette = Palette {
            default: colors(0),
            dark: colors(10),
        };

        let default = palette.variant(Variant::Default);
        assert_eq!(default.background(GtkState::Normal), grey(0));
        assert_eq!(default.background(GtkState::Selected), grey(3));
        assert_eq!(default.text(GtkState::Backdrop), grey(107));
        assert_eq!(default.named("wm_bg"), Some(grey(50)));
        assert_eq!(default.named("wm_title"), None);

        let dark = palette.variant(Variant::Dark);
        assert_eq!(dark.background(GtkState::Insensitive), grey(14));
        assert_eq!(dark.named("wm_bg"), Some(grey(60)));
    }
}
