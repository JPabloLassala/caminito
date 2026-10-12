//! caminito-wallpaper (ADR 0031): the drawer, `caminito-wallpaperd`, which draws each
//! output's wallpaper as a Layer surface in the background layer (210).
//!
//! The library holds what names no toolkit: a monitor's identity, where a picture goes for
//! each positioning, and which settings an output gets from the store, which the unit tests
//! drive. [`drawer`] is the Wayland client, on GLib's loop and GSettings, and links no GTK.

#![forbid(unsafe_code)]

pub mod drawer;
mod gsettings;
mod identity;
mod positioning;
mod settings;

/// The log domain of everything the drawer logs.
const DOMAIN: &str = "caminito-wallpaperd";
