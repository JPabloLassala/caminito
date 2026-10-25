//! caminito-notifications (ADR 0051): the notification daemon of a Session, a Helper of
//! Caminito's, which owns `org.freedesktop.Notifications` and shows each notification as a
//! Notification bubble.
//!
//! The library holds what names no toolkit: [`server`], the Desktop Notifications spec over
//! gio's D-Bus, which decides when a bubble opens, changes and closes and which sound plays,
//! through the [`server::Bubbles`] and [`server::Sounds`] the binary gives it, and `markup`,
//! which makes a body's markup safe for Pango. The integration tests drive the server on a
//! private bus with fakes of both. The bubbles' layer surfaces and libcanberra are the
//! binary's.

#![forbid(unsafe_code)]

/// The log domain of GLib's warnings, the program's and the library's.
pub const DOMAIN: &str = "caminito-notifications";

mod markup;
mod notification;
pub mod server;
