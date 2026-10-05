//! The protocols Camino implements itself, each as a state and a handler trait (15).

pub mod ext_workspace;
pub mod foreign_toplevel;
pub mod gtk_shell;
pub mod output_management;
pub mod raw;
pub mod xfce_foreign_toplevel;

use smithay::reexports::wayland_server::Client;

/// Says whether a client is offered a global, as Smithay's states' filters do.
pub(crate) type GlobalFilter = Box<dyn for<'c> Fn(&'c Client) -> bool + Send + Sync>;
