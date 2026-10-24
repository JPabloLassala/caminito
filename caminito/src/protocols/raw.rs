//! The protocols no crate has, generated with `wayland-scanner` from the XML in `resources/`
//! (15): GTK's `gtk_shell1` and `gtk_surface1`, from `gtk-shell.xml` (67), and in [`xfce`]
//! Xfce's extension of foreign toplevel management, from
//! `xfce-foreign-toplevel-management-private-v1.xml` (75), and in [`appmenu`] KDE's
//! `org_kde_kwin_appmenu_manager`, from `appmenu.xml` (112), and in [`intellihide`] Caminito's
//! own `caminito_intellihide_manager_v1`, from `caminito-intellihide-v1.xml` (CMN-224), and in
//! [`toplevel_identifier`] `caminito_toplevel_identifier_manager_v1`, from
//! `caminito-toplevel-identifier-v1.xml` (CMN-243).

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

/// `xfce_foreign_toplevel_manager_private_v1` and the two interfaces it makes. Its requests
/// and events name objects of foreign toplevel management and of ext-workspace.
pub mod xfce {
    use smithay::reexports::wayland_protocols::ext::workspace::v1::server::*;
    use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::server::*;
    use smithay::reexports::wayland_server;
    use smithay::reexports::wayland_server::protocol::*;

    pub mod __interfaces {
        use smithay::reexports::wayland_protocols::ext::workspace::v1::server::__interfaces::*;
        use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::server::__interfaces::*;
        use smithay::reexports::wayland_server::backend as wayland_backend;
        use smithay::reexports::wayland_server::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!(
            "resources/xfce-foreign-toplevel-management-private-v1.xml"
        );
    }
    use self::__interfaces::*;

    wayland_scanner::generate_server_code!(
        "resources/xfce-foreign-toplevel-management-private-v1.xml"
    );
}

/// `org_kde_kwin_appmenu_manager` and the `org_kde_kwin_appmenu` it makes.
pub mod appmenu {
    use smithay::reexports::wayland_server;
    use smithay::reexports::wayland_server::protocol::*;

    pub mod __interfaces {
        use smithay::reexports::wayland_server::backend as wayland_backend;
        use smithay::reexports::wayland_server::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("resources/appmenu.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_server_code!("resources/appmenu.xml");
}

/// `caminito_intellihide_manager_v1` and the `caminito_intellihide_v1` it makes. Its request
/// names a layer surface of layer shell.
pub mod intellihide {
    use smithay::reexports::wayland_protocols_wlr::layer_shell::v1::server::*;
    use smithay::reexports::wayland_server;
    use smithay::reexports::wayland_server::protocol::*;

    pub mod __interfaces {
        use smithay::reexports::wayland_protocols_wlr::layer_shell::v1::server::__interfaces::*;
        use smithay::reexports::wayland_server::backend as wayland_backend;
        use smithay::reexports::wayland_server::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("resources/caminito-intellihide-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_server_code!("resources/caminito-intellihide-v1.xml");
}

/// `caminito_toplevel_identifier_manager_v1` and the `caminito_toplevel_identifier_v1` it
/// makes (CMN-243). Its request names a handle of wlr-foreign-toplevel-management.
pub mod toplevel_identifier {
    use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::server::*;
    use smithay::reexports::wayland_server;
    use smithay::reexports::wayland_server::protocol::*;

    pub mod __interfaces {
        use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::server::__interfaces::*;
        use smithay::reexports::wayland_server::backend as wayland_backend;
        use smithay::reexports::wayland_server::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("resources/caminito-toplevel-identifier-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_server_code!("resources/caminito-toplevel-identifier-v1.xml");
}
