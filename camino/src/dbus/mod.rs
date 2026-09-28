//! Camino's D-Bus (15): one file per name or peer.
//!
//! zbus runs async on its own executor thread. Interface methods send a request with a
//! reply sender into the event loop over a `calloop::channel` and await the reply, so the
//! main thread never waits on the bus.

pub mod debug;

use smithay::reexports::calloop::channel;

use crate::state::{Camino, State};

/// Which names Camino's connection asks the bus for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Names {
    /// Its well-known names, such as `org.mate.Camino.Debug`.
    WellKnown,
    /// None: it's reached at its unique name. Tests share one bus, so they can't all own
    /// the well-known names.
    UniqueOnly,
}

/// Camino's connection to the session bus.
pub struct Dbus {
    connection: zbus::Connection,
}

impl Dbus {
    /// The connection's unique name on the bus.
    pub fn unique_name(&self) -> Option<String> {
        self.connection.unique_name().map(|name| name.to_string())
    }
}

impl Camino {
    /// Connects to the session bus and serves Camino's interfaces on it. Connecting is the
    /// one time the main thread waits on the bus.
    pub fn start_dbus(&mut self, names: Names) -> zbus::Result<()> {
        let (debug_requests, requests) = channel::channel();
        self.loop_handle
            .insert_source(requests, |event, _, state: &mut State| match event {
                channel::Event::Msg(request) => state.camino.debug_request(request),
                channel::Event::Closed => {}
            })
            .expect("the D-Bus channel joins the event loop");

        let mut builder = zbus::connection::Builder::session()?
            .serve_at(debug::PATH, debug::DebugInterface::new(debug_requests))?;
        if names == Names::WellKnown {
            builder = builder.name(debug::NAME)?;
        }
        let dbus = Dbus {
            connection: zbus::block_on(builder.build())?,
        };
        tracing::info!(
            "on the session bus as {}",
            dbus.unique_name().unwrap_or_default()
        );
        self.dbus = Some(dbus);
        Ok(())
    }
}
