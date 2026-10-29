//! caminito-notifications (ADR 0051): the notification daemon of a Session, a Helper of
//! Caminito's, which owns `org.freedesktop.Notifications` and shows each notification as a
//! Notification bubble.
//!
//! The library holds what names no toolkit: [`server`], the Desktop Notifications spec over
//! gio's D-Bus, which decides when a bubble opens, changes and closes and which sound plays,
//! through the [`server::Bubbles`] and [`server::Sounds`] the binary gives it, following
//! [`settings`], the keys of `org.mate.Caminito.notifications` and the Notifying apps' own,
//! [`apps`], the Notifying apps the settings window lists, [`private_api`], what the
//! applets call, and `markup`, which makes a body's markup safe for Pango. The server also
//! keeps the Notification history (CMN-259), in a file, which `private_api` serves, and
//! decides Do Not Disturb (CMN-263), which a file keeps too. The
//! integration tests drive the server and the private API on a private bus with fakes of what
//! the binary gives them. The bubbles' layer surfaces, the settings window and libcanberra are the
//! binary's.

#![forbid(unsafe_code)]

/// The log domain of GLib's warnings, the program's and the library's.
pub const DOMAIN: &str = "caminito-notifications";

pub mod apps;
mod do_not_disturb;
mod history;
mod markup;
mod notification;
pub mod private_api;
pub mod server;
pub mod settings;
