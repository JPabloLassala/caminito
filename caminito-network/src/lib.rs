//! caminito-network (ADR 0041): the program that runs through a Session as NetworkManager's
//! secret agent, asking for the passwords a connection needs.
//!
//! The library holds what names no toolkit: [`agent`], the secret agent over gio's D-Bus,
//! which answers NetworkManager's requests through dialogs the binary draws. The integration
//! tests drive it against python-dbusmock's `networkmanager` template on a private bus. The
//! application and its dialogs are the binary's, the one part that uses GTK.

#![forbid(unsafe_code)]

/// The log domain of GLib's warnings, the program's and the library's.
pub const DOMAIN: &str = "caminito-network";

pub mod agent;
