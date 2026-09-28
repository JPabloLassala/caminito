//! Marco's theme engine, with no Smithay or GTK.

mod builtin;
mod button_layout;
mod color;
mod color_names;
mod draw;
mod error;
mod expr;
mod geometry;
mod image;
mod load;
mod markup;
mod model;
mod number;
mod palette;
mod parser;
mod title;

pub use button_layout::{ButtonFunction, ButtonLayout, LayoutButton};
pub use color::{ColorComponent, ColorSpec, GtkState, Rgba};
pub use draw::{ButtonStates, Frame, Strip};
pub use error::ThemeError;
pub use expr::{Constants, Expr, ExprEnv, Operator, Variable};
pub use geometry::{
    ButtonBackgrounds, ButtonSpace, CornerRadii, FrameBorders, FrameFlags, FrameGeometry,
    MAX_MIDDLE_BACKGROUNDS, Rect,
};
pub use image::surface_from_pixbuf;
pub use load::ThemeDirs;
pub use model::{
    AlphaGradient, Border, ButtonSizing, ButtonState, ButtonType, DrawOp, DrawOpList, DrawRect,
    FrameFocus, FrameLayout, FramePiece, FrameResize, FrameState, FrameStyle, FrameStyleSet,
    FrameType, GradientType, GtkArrow, GtkShadow, ImageFill, ImageSource, Theme, ThemeInfo,
};
pub use palette::{
    PALETTE_STREAM_HEADER, Palette, PaletteCache, PaletteCacheError, PaletteColors, PaletteReader,
    PaletteStreamError, ReportedPalette, StateColors, Variant, format_report, names_request,
};
pub use title::{Title, title_context};

// The versions of pango, cairo and gdk-pixbuf the API takes.
pub use gdk_pixbuf;
pub use pango;
pub use pangocairo::cairo;
