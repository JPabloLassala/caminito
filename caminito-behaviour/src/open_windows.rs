//! The open windows' app ids (CMN-189), for the Key remaps page's list: read once, as the
//! page opens, from Caminito's ext-foreign-toplevel-list, on a Wayland connection of the
//! library's own beside GTK's.

use std::collections::HashMap;

use wayland_client::backend::ObjectId;
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::{
    Connection, Dispatch, DispatchError, Proxy, QueueHandle, event_created_child,
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::{
    self, ExtForeignToplevelHandleV1,
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::{
    self, ExtForeignToplevelListV1,
};

/// Why the open windows can't be read.
#[derive(Debug)]
pub enum Error {
    /// There's no compositor to connect to.
    Connect(wayland_client::ConnectError),
    /// The compositor lists no windows to this client.
    NoList,
    /// The compositor sent what the protocol doesn't allow, or the connection broke.
    Dispatch(DispatchError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Connect(err) => write!(f, "can't connect to the compositor: {err}"),
            Error::NoList => write!(f, "the compositor lists no windows"),
            Error::Dispatch(err) => write!(f, "the compositor sent a bad event: {err}"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Default)]
struct State {
    listed: bool,
    /// Each window's app id, by its handle.
    app_ids: HashMap<ObjectId, String>,
}

/// The app ids of the windows open now, each once, in order. A window with none isn't
/// counted.
pub fn app_ids() -> Result<Vec<String>, Error> {
    let connection = Connection::connect_to_env().map_err(Error::Connect)?;
    let mut queue = connection.new_event_queue();
    connection.display().get_registry(&queue.handle(), ());
    let mut state = State::default();
    // The registry's globals, then the list's windows and what each says of itself.
    for _ in 0..2 {
        queue.roundtrip(&mut state).map_err(Error::Dispatch)?;
    }
    if !state.listed {
        return Err(Error::NoList);
    }
    let mut app_ids: Vec<String> = state.app_ids.into_values().collect();
    app_ids.sort();
    app_ids.dedup();
    Ok(app_ids)
}

impl Dispatch<WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name, interface, ..
        } = event
            && interface == ExtForeignToplevelListV1::interface().name
        {
            registry.bind::<ExtForeignToplevelListV1, _, _>(name, 1, qh, ());
            state.listed = true;
        }
    }
}

impl Dispatch<ExtForeignToplevelListV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ExtForeignToplevelListV1,
        _: ext_foreign_toplevel_list_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }

    event_created_child!(State, ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        handle: &ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_foreign_toplevel_handle_v1::Event::AppId { app_id } if !app_id.is_empty() => {
                state.app_ids.insert(handle.id(), app_id);
            }
            ext_foreign_toplevel_handle_v1::Event::Closed => {
                state.app_ids.remove(&handle.id());
            }
            _ => {}
        }
    }
}
