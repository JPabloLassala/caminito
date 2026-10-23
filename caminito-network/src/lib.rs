//! caminito-network (ADR 0041): the program that runs through a Session as NetworkManager's
//! secret agent, asking for the passwords a connection needs, and tells the user when a
//! connection fails, drops, or comes up by itself.
//!
//! The library holds what names no toolkit: [`agent`], the secret agent over gio's D-Bus,
//! which answers NetworkManager's requests through dialogs the binary draws, `vpn`, a VPN
//! plugin's auth-dialog, which the agent runs for a VPN's secrets, `keyring`, the agent-owned
//! secrets it keeps in the Secret Service, and [`notifications`], which follows
//! NetworkManager's active connections and notifies through the session's notification server
//! (ADR 0044). The integration tests drive them against python-dbusmock's `networkmanager`
//! and `notification_daemon` templates on a private bus. The application and its dialogs are
//! the binary's, the one part that uses GTK.

#![forbid(unsafe_code)]

/// The log domain of GLib's warnings, the program's and the library's.
pub const DOMAIN: &str = "caminito-network";

pub mod agent;
mod keyring;
pub mod notifications;
mod vpn;
