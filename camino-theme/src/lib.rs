//! Marco's theme engine, with no Smithay or GTK.

mod builtin;
mod color;
mod color_names;
mod error;
mod expr;
mod load;
mod markup;
mod model;
mod number;
mod palette;
mod parser;

pub use color::{ColorComponent, ColorSpec, GtkState, Rgba};
pub use error::ThemeError;
pub use expr::{Constants, Expr, Operator, Variable};
pub use load::ThemeDirs;
pub use model::{
    AlphaGradient, Border, ButtonSizing, ButtonState, ButtonType, DrawOp, DrawOpList, DrawRect,
    FrameFocus, FrameGeometry, FramePiece, FrameResize, FrameState, FrameStyle, FrameStyleSet,
    FrameType, GradientType, GtkArrow, GtkShadow, ImageFill, ImageSource, Theme, ThemeInfo,
};
pub use palette::{Palette, PaletteColors, StateColors, Variant};
