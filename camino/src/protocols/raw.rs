//! The protocols no crate has, generated with `wayland-scanner` from the XML in `resources/`
//! (15): GTK's `gtk_shell1` and `gtk_surface1`, from `gtk-shell.xml` (67).

#![allow(
    dead_code,
    non_camel_case_types,
    non_upper_case_globals,
    unused_imports,
    missing_docs,
    clippy::all,
    reason = "generated code"
)]

use smithay::reexports::wayland_server;
use smithay::reexports::wayland_server::protocol::*;

pub mod __interfaces {
    use smithay::reexports::wayland_server::backend as wayland_backend;
    use smithay::reexports::wayland_server::protocol::__interfaces::*;
    wayland_scanner::generate_interfaces!("resources/gtk-shell.xml");
}
use self::__interfaces::*;

wayland_scanner::generate_server_code!("resources/gtk-shell.xml");
