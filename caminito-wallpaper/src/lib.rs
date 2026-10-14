//! caminito-wallpaper (ADR 0031): the drawer, `caminito-wallpaperd`, which draws each
//! output's wallpaper as a Layer surface in the background layer (210), and the dialog,
//! `caminito-wallpaper`, which sets it (212).
//!
//! The library holds what names no toolkit: a monitor's identity, where a picture goes for
//! each positioning, which settings an output gets from the store, what a dynamic wallpaper
//! shows at a moment (216), which of a slideshow's pictures an output shows (213), the
//! dialog's page and its grids' groups, which the unit tests drive. [`drawer`] is the
//! Wayland client, on GLib's loop and GSettings, and links no GTK; [`monitors`] is the
//! dialog's client of output management. The dialog's window is its binary's, the one
//! part that uses GTK.

#![forbid(unsafe_code)]

mod bridge;
pub mod drawer;
pub mod dynamic;
pub mod grid;
mod gsettings;
mod identity;
pub mod monitors;
pub mod page;
mod positioning;
mod settings;
pub mod slideshow;
pub mod sun;

pub use gsettings::GioStore;
pub use positioning::{Positioning, place};
pub use settings::{Colour, Kind, Memory, OutputSettings, Set, Store};

/// The log domain of everything the drawer logs.
const DOMAIN: &str = "caminito-wallpaperd";
