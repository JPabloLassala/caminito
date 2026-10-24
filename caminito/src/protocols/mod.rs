//! The protocols Caminito implements itself, each as a state and, where the compositor hears
//! of something from it, a handler trait (15).

pub mod appmenu;
pub mod ext_workspace;
pub mod foreign_toplevel;
pub mod gamma_control;
pub mod gtk_shell;
pub mod idle_notify;
pub mod intellihide;
pub mod output_management;
pub mod raw;
pub mod toplevel_identifier;
pub mod xfce_foreign_toplevel;

use smithay::reexports::wayland_server::Client;

/// Says whether a client is offered a global, as Smithay's states' filters do.
pub(crate) type GlobalFilter = Box<dyn for<'c> Fn(&'c Client) -> bool + Send + Sync>;
