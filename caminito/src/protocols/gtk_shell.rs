//! A minimal `gtk_shell1` (06, 67): GTK3 says only through this GNOME protocol that a window
//! is modal, and rings the bell only through it. GTK4 has xdg-dialog and xdg-system-bell for
//! those. Both say through it where a `GtkApplication` window's menus are on the bus, and
//! both draw no menubar of their own while it has the `global_menu_bar` capability (110,
//! ADR 0017).
//!
//! This module owns the protocol's objects, each surface's modal hint and each surface's
//! menus. It knows nothing of windows: the compositor reads them with [`is_modal`] and
//! [`dbus_properties`], and hears of a change of the hint and of the bell through
//! [`GtkShellHandler`]. Every other request is taken and ignored, and the one event sent is
//! `capabilities`, which GTK needs none of to start.
//!
//! It isn't offered to make GTK3 drop its client-side decorations: GTK3 asks for a frame
//! through KDE server-decoration (07).

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use smithay::reexports::wayland_server::backend::GlobalId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource, Weak,
};
use smithay::wayland::compositor::with_states;
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use crate::protocols::raw::gtk_shell1::{self, GtkShell1};
use crate::protocols::raw::gtk_surface1::{self, GtkSurface1};

/// The version of the protocol Caminito offers: the lowest at which GTK3 uses `set_modal` and
/// `system_bell`, which are both in version 1. GTK3 binds the lower of this and 5
/// (gdkdisplay-wayland.c:429-436, gtk-3-24 at 2776f156), and skips what version 1 lacks:
/// `request_focus` (gdkwindow-wayland.c:4056), `release` (3584-3588), `titlebar_gesture`
/// (5094) and the edge constraints of `configure_edges` (1764), which it then works out
/// itself.
const VERSION: u32 = 1;

/// `gtk_shell1`'s `global_menu_bar` capability. GTK takes the enum's values for bits: with
/// this one sent, GTK 3.24.52 and GTK 4.22.5 both turn `gtk-shell-shows-menubar` on, and
/// with 0 off (110).
const GLOBAL_MENU_BAR: u32 = 2;

/// What the compositor does with what GTK3 says.
pub trait GtkShellHandler: 'static {
    /// The global's state, which a new `gtk_shell1` joins.
    fn gtk_shell_state(&mut self) -> &mut GtkShellState;

    /// `surface` became modal, or stopped being modal. [`is_modal`] has the new hint.
    fn modal_changed(&mut self, surface: &WlSurface);

    /// A client rang the bell for `surface`, which may be gone by now, or for no surface.
    fn system_bell(&mut self, surface: Option<Weak<WlSurface>>);
}

/// The `gtk_shell1` global.
#[derive(Debug)]
pub struct GtkShellState {
    _global: GlobalId,
    /// Every client's `gtk_shell1`, to send a change of the capabilities to.
    shells: Vec<GtkShell1>,
    /// Whether the clients are told that something shows their menubars.
    global_menu_bar: bool,
}

/// The global's data.
#[derive(Debug)]
pub struct GtkShellGlobalData;

/// A `gtk_shell1`'s data.
#[derive(Debug)]
pub struct GtkShellData;

/// A `gtk_surface1`'s data.
#[derive(Debug)]
pub struct GtkSurfaceData {
    surface: Weak<WlSurface>,
}

/// A surface's modal hint, in its data map. It outlives the `gtk_surface1` that set it, as
/// the protocol has no word for what destroying one undoes.
struct Modal(AtomicBool);

/// What a `GtkApplication` last said of a surface with `set_dbus_properties`: where its
/// menus, `org.gtk.Menus`, and their actions, `org.gtk.Actions`, are on the bus. Each is
/// `None` if the client left it out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DbusProperties {
    /// The connection that serves them all.
    pub unique_bus_name: Option<String>,
    /// Where the app menu is, which GNOME's shell used to show beside its Activities.
    pub app_menu_path: Option<String>,
    /// Where the menubar is.
    pub menubar_path: Option<String>,
    /// Where the `win.` actions are.
    pub window_object_path: Option<String>,
    /// Where the `app.` actions are.
    pub application_object_path: Option<String>,
}

/// A surface's menus, in its data map, which outlive the `gtk_surface1` as [`Modal`] does.
struct Menus(Mutex<DbusProperties>);

impl GtkShellState {
    /// Offers `gtk_shell1` to every client.
    pub fn new<D>(display: &DisplayHandle) -> Self
    where
        D: GlobalDispatch<GtkShell1, GtkShellGlobalData> + 'static,
    {
        let global = display.create_global::<D, GtkShell1, _>(VERSION, GtkShellGlobalData);
        Self {
            _global: global,
            shells: Vec::new(),
            global_menu_bar: false,
        }
    }

    /// Tells every client whether something shows its menubars, now and as it binds. A GTK
    /// app follows a change while it runs.
    pub fn set_global_menu_bar(&mut self, shown: bool) {
        if self.global_menu_bar == shown {
            return;
        }
        self.global_menu_bar = shown;
        self.shells.retain(Resource::is_alive);
        for shell in &self.shells {
            shell.capabilities(self.capabilities());
        }
    }

    fn capabilities(&self) -> u32 {
        if self.global_menu_bar {
            GLOBAL_MENU_BAR
        } else {
            0
        }
    }
}

/// What a client last said, with `gtk_surface1.set_dbus_properties`, of where `surface`'s
/// menus are on the bus, if it said anything.
pub fn dbus_properties(surface: &WlSurface) -> Option<DbusProperties> {
    with_states(surface, |states| {
        let menus = states.data_map.get::<Menus>()?;
        Some(menus.0.lock().unwrap().clone())
    })
}

/// Whether a client last said, with `gtk_surface1.set_modal`, that `surface` is modal.
pub fn is_modal(surface: &WlSurface) -> bool {
    with_states(surface, |states| {
        states
            .data_map
            .get::<Modal>()
            .is_some_and(|modal| modal.0.load(Ordering::Relaxed))
    })
}

/// Records a surface's modal hint, and returns whether it changed.
fn set_modal(surface: &WlSurface, modal: bool) -> bool {
    with_states(surface, |states| {
        let hint = states
            .data_map
            .get_or_insert_threadsafe(|| Modal(AtomicBool::new(false)));
        hint.0.swap(modal, Ordering::Relaxed) != modal
    })
}

/// Records where a surface's menus are on the bus.
fn set_dbus_properties(surface: &WlSurface, properties: DbusProperties) {
    with_states(surface, |states| {
        let menus = states
            .data_map
            .get_or_insert_threadsafe(|| Menus(Mutex::default()));
        *menus.0.lock().unwrap() = properties;
    });
}

impl<D> GlobalDispatch2<GtkShell1, D> for GtkShellGlobalData
where
    D: GtkShellHandler + Dispatch<GtkShell1, GtkShellData>,
{
    fn bind(
        &self,
        state: &mut D,
        _display: &DisplayHandle,
        _client: &Client,
        resource: New<GtkShell1>,
        data_init: &mut DataInit<'_, D>,
    ) {
        let shell = data_init.init(resource, GtkShellData);
        let state = state.gtk_shell_state();
        // GTK takes the capabilities to be none until it's told, so without a global menu
        // there's nothing to send: Caminito has no app menu and draws no desktop icons.
        if state.global_menu_bar {
            shell.capabilities(state.capabilities());
        }
        state.shells.retain(Resource::is_alive);
        state.shells.push(shell);
    }
}

impl<D> Dispatch2<GtkShell1, D> for GtkShellData
where
    D: GtkShellHandler + Dispatch<GtkSurface1, GtkSurfaceData>,
{
    fn request(
        &self,
        state: &mut D,
        _client: &Client,
        _resource: &GtkShell1,
        request: gtk_shell1::Request,
        _display: &DisplayHandle,
        data_init: &mut DataInit<'_, D>,
    ) {
        match request {
            gtk_shell1::Request::GetGtkSurface {
                gtk_surface,
                surface,
            } => {
                let data = GtkSurfaceData {
                    surface: surface.downgrade(),
                };
                data_init.init(gtk_surface, data);
            }
            gtk_shell1::Request::SystemBell { surface } => {
                // A `gtk_surface1` has no destructor at version 1, so its surface can go
                // before it does, and a bell for it then names a surface that's gone.
                let surface = surface
                    .as_ref()
                    .and_then(|gtk_surface| gtk_surface.data::<GtkSurfaceData>())
                    .map(|data| data.surface.clone());
                state.system_bell(surface);
            }
            // Startup notification goes through xdg-activation. `notify_launch` is newer
            // than the version offered.
            gtk_shell1::Request::SetStartupId { .. } | gtk_shell1::Request::NotifyLaunch { .. } => {
            }
        }
    }
}

impl<D: GtkShellHandler> Dispatch2<GtkSurface1, D> for GtkSurfaceData {
    fn request(
        &self,
        state: &mut D,
        _client: &Client,
        _resource: &GtkSurface1,
        request: gtk_surface1::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        let modal = match request {
            gtk_surface1::Request::SetModal => true,
            gtk_surface1::Request::UnsetModal => false,
            // The compositor reads the menus when it next publishes the global menu (110), so
            // there's nothing to tell it. The application id is left: the window has an
            // `app_id` already.
            gtk_surface1::Request::SetDbusProperties {
                application_id: _,
                app_menu_path,
                menubar_path,
                window_object_path,
                application_object_path,
                unique_bus_name,
            } => {
                if let Ok(surface) = self.surface.upgrade() {
                    let properties = DbusProperties {
                        unique_bus_name,
                        app_menu_path,
                        menubar_path,
                        window_object_path,
                        application_object_path,
                    };
                    set_dbus_properties(&surface, properties);
                }
                return;
            }
            // Activation goes through xdg-activation, and a titlebar gesture is only made of
            // a client-side titlebar, whose actions GTK3 then takes itself. The last three
            // are newer than the version offered, so the Wayland library lets none through;
            // they're here because the XML has them.
            gtk_surface1::Request::Present { .. }
            | gtk_surface1::Request::RequestFocus { .. }
            | gtk_surface1::Request::TitlebarGesture { .. }
            | gtk_surface1::Request::Release => return,
        };
        let Ok(surface) = self.surface.upgrade() else {
            return;
        };
        if set_modal(&surface, modal) {
            state.modal_changed(&surface);
        }
    }
}
