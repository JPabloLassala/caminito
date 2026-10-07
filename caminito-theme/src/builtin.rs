//! The theme compiled into Caminito, for when the configured one fails to load (07, 16).

use std::path::Path;

use crate::model::Theme;

const SOURCE: &str = include_str!("builtin/metacity-theme-3.xml");

impl Theme {
    /// Caminito's own theme, which needs no files and only the GTK palette. It's parsed
    /// like any other theme.
    pub fn builtin() -> Theme {
        Theme::parse(
            SOURCE.as_bytes(),
            "Caminito",
            Path::new("<built-in>/metacity-1/metacity-theme-3.xml"),
            3,
        )
        .expect("the built-in theme is valid")
    }
}
