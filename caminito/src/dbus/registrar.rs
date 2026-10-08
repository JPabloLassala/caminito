//! `com.canonical.AppMenu.Registrar` (113, ADR 0017): where an X11 client says which
//! dbusmenu is a window's, by the X window's id. Electron exports its menu no other way, so
//! it's the only way VS Code has a global menu.
//!
//! This module owns the interface and the registrations. A registration is its caller's,
//! and goes with it, with its X window, or when the caller takes it back. Which window's
//! menu is published is `global_menu`'s.

use std::collections::HashMap;

use futures_channel::oneshot;
use smithay::reexports::calloop::channel::Sender;
use zbus::message::Header;
use zbus::zvariant::OwnedObjectPath;

use super::{Callers, caller};
use crate::state::Caminito;

/// The interface's path, which every registrar has had since Unity's.
pub(crate) const PATH: &str = "/com/canonical/AppMenu/Registrar";

/// A request for the event loop, each answered once.
pub(crate) enum Request {
    /// `RegisterWindow`, from `caller`.
    Register {
        caller: String,
        window: u32,
        path: OwnedObjectPath,
        reply: oneshot::Sender<()>,
    },
    /// `UnregisterWindow`, from `caller`.
    Unregister {
        caller: String,
        window: u32,
        reply: oneshot::Sender<()>,
    },
    /// `GetMenuForWindow`.
    MenuForWindow(u32, oneshot::Sender<Option<Registration>>),
}

/// Where a window's dbusmenu is on the bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Registration {
    /// The unique name of the connection that registered it, which serves it.
    pub(crate) owner: String,
    /// The `com.canonical.dbusmenu` object.
    pub(crate) path: OwnedObjectPath,
}

/// The menus the X11 clients registered, by the id each gave.
///
/// Nothing checks that an id is an X window's: the call can come before Xwayland tells
/// Caminito of the window. An id that's no XWayland window's is never looked up, so its
/// registration is kept and never shown. An Electron app on native Wayland makes one, of
/// a small number that counts its windows, which no X client's window has: the X server
/// gives a client its ids above its resource base.
#[derive(Debug, Default)]
pub(crate) struct Registrations {
    menus: HashMap<u32, Registration>,
}

impl Registrations {
    /// The menu registered for the X window `window`, if any.
    pub(crate) fn menu(&self, window: u32) -> Option<&Registration> {
        self.menus.get(&window)
    }

    /// Registers `owner`'s menu at `path` for `window`, in place of any it had.
    fn register(&mut self, owner: String, window: u32, path: OwnedObjectPath) {
        self.menus.insert(window, Registration { owner, path });
    }

    /// Takes back `window`'s menu, if it's `owner`'s.
    fn unregister(&mut self, owner: &str, window: u32) {
        if self.menu(window).is_some_and(|menu| menu.owner == owner) {
            self.menus.remove(&window);
        }
    }

    /// The X window `window` is destroyed, and the X server may give its id to another.
    pub(crate) fn window_destroyed(&mut self, window: u32) {
        self.menus.remove(&window);
    }

    /// Xwayland is gone with every X window, and the next one gives the ids out again.
    pub(crate) fn xwayland_gone(&mut self) {
        self.menus.clear();
    }

    /// Drops the menus of a caller that left the bus.
    pub(super) fn owner_left(&mut self, owner: &str) {
        self.menus.retain(|_, menu| menu.owner != owner);
    }
}

/// The interface, served on Caminito's connection.
pub(super) struct RegistrarInterface {
    requests: Sender<Request>,
    callers: Callers,
}

impl RegistrarInterface {
    pub(super) fn new(requests: Sender<Request>, callers: Callers) -> Self {
        Self { requests, callers }
    }
}

#[zbus::interface(name = "com.canonical.AppMenu.Registrar")]
impl RegistrarInterface {
    /// Says that the caller serves the dbusmenu of the X window `window_id` at
    /// `menu_object_path`. It lasts until the caller takes it back or leaves the bus, or
    /// the window is destroyed. A window has one menu: the last registered.
    async fn register_window(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
        window_id: u32,
        menu_object_path: OwnedObjectPath,
    ) -> zbus::fdo::Result<()> {
        let caller = caller(&header);
        let register = |reply| Request::Register {
            caller: caller.clone(),
            window: window_id,
            path: menu_object_path,
            reply,
        };
        super::ask(&self.requests, register).await?;
        self.callers.report_if_gone(connection, caller).await;
        Ok(())
    }

    /// Takes back the menu the caller registered for the X window `window_id`. Another
    /// caller's is left.
    async fn unregister_window(
        &self,
        #[zbus(header)] header: Header<'_>,
        window_id: u32,
    ) -> zbus::fdo::Result<()> {
        let caller = caller(&header);
        let unregister = |reply| Request::Unregister {
            caller,
            window: window_id,
            reply,
        };
        super::ask(&self.requests, unregister).await
    }

    /// The bus name and the object path of the menu registered for the X window
    /// `window_id`, or the empty string and `/` if it has none: an object path can't be
    /// empty.
    ///
    /// It answers for any id, which `GlobalMenu` never does (ADR 0002): the interface is
    /// the registrar's, as the apps and the importers know it, and what it tells is what
    /// its callers told it.
    async fn get_menu_for_window(
        &self,
        window_id: u32,
    ) -> zbus::fdo::Result<(String, OwnedObjectPath)> {
        let menu = |reply| Request::MenuForWindow(window_id, reply);
        let menu = super::ask(&self.requests, menu).await?;
        Ok(menu.map_or_else(
            || (String::new(), OwnedObjectPath::default()),
            |menu| (menu.owner, menu.path),
        ))
    }
}

impl Caminito {
    /// Answers the registrar's interface, in the event loop.
    pub(super) fn registrar_request(&mut self, request: Request) {
        let registrations = &mut self.app_menu_registrations;
        match request {
            Request::Register {
                caller,
                window,
                path,
                reply,
            } => {
                registrations.register(caller, window, path);
                let _ = reply.send(());
            }
            Request::Unregister {
                caller,
                window,
                reply,
            } => {
                registrations.unregister(&caller, window);
                let _ = reply.send(());
            }
            Request::MenuForWindow(window, reply) => {
                let _ = reply.send(registrations.menu(window).cloned());
            }
        }
    }
}
