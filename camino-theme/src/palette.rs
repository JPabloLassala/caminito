//! The GTK colours Marco themes refer to (07, ADR 0003).
//!
//! The palette helper reads them from GTK. Its wire format and Camino's on-disk cache of
//! them are the palette helper ticket's (28).

use std::collections::BTreeMap;

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
