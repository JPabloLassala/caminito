//! caminito-bluetooth (255, ADR 0035): the program that runs through the whole session as
//! the pairing agent (256) and opens the Bluetooth settings window (257, 258).
//!
//! The library holds what names no toolkit: [`bluez`], the model of BlueZ's adapters and
//! devices over gio's D-Bus, [`rfkill`], the soft block that off means, and [`switch`],
//! which turns all of Bluetooth on and off with them. The integration tests drive them
//! against python-dbusmock's `bluez5` template on a private bus. The application and its
//! window are the binary's, the one part that uses GTK.

#![forbid(unsafe_code)]

pub mod bluez;
pub mod rfkill;
pub mod switch;
