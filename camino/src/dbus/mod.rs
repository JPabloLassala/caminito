//! Camino's D-Bus (15): one file per name or peer. `systemd` is the one peer so far.
//!
//! zbus runs async on its own executor thread. Interface methods send a request with a
//! reply sender into the event loop over a `calloop::channel` and await the reply, so the
//! main thread never waits on the bus.

pub mod debug;
pub mod screensaver;
pub(crate) mod systemd;

use futures_channel::oneshot;
use smithay::reexports::calloop::channel;

use crate::state::{Camino, State};

/// Which names Camino's connection asks the bus for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Names {
    /// Its well-known names: `org.mate.Camino.Debug`, and the Screensaver shim's two (62).
    WellKnown,
    /// None: it's reached at its unique name. Tests share one bus, so they can't all own
    /// the well-known names.
    UniqueOnly,
}

/// Sends the event loop a request, and waits for its reply: what makes an interface's
/// methods thin (15).
async fn ask<R, T>(
    requests: &channel::Sender<R>,
    request: impl FnOnce(oneshot::Sender<T>) -> R,
) -> zbus::fdo::Result<T> {
    let (reply, answer) = oneshot::channel();
    let gone = || zbus::fdo::Error::Failed("Camino's event loop has stopped".into());
    requests.send(request(reply)).map_err(|_| gone())?;
    answer.await.map_err(|_| gone())
}

/// Camino's connection to the session bus.
pub struct Dbus {
    /// Passes on the callers that leave the bus (62). It's before the connection, which
    /// it holds, so that it stops first.
    _caller_watch: Option<zbus::Task<()>>,
    connection: zbus::Connection,
}

impl Dbus {
    /// The connection, for the Session's calls to the bus and to systemd (61).
    pub(crate) fn connection(&self) -> &zbus::Connection {
        &self.connection
    }

    /// The connection's unique name on the bus.
    pub fn unique_name(&self) -> Option<String> {
        self.connection.unique_name().map(|name| name.to_string())
    }
}

impl Camino {
    /// Connects to the session bus and serves Camino's interfaces on it. Connecting is the
    /// one time the main thread waits on the bus, and asking for the names is part of it:
    /// it returns once the Screensaver shim's names are owned or refused (62), so
    /// [`Camino::dbus_ready`] holds from then on.
    pub fn start_dbus(&mut self, names: Names) -> zbus::Result<()> {
        let debug_requests = self.dbus_requests(Camino::debug_request);
        let screensaver_requests = self.dbus_requests(Camino::screensaver_request);

        let mut builder = zbus::connection::Builder::session()?
            .serve_at(debug::PATH, debug::DebugInterface::new(debug_requests))?;
        builder = screensaver::serve(builder, screensaver_requests.clone())?;
        if names == Names::WellKnown {
            builder = builder.name(debug::NAME)?;
        }
        let connection = zbus::block_on(builder.build())?;
        if names == Names::WellKnown {
            zbus::block_on(screensaver::own_names(&connection));
        }
        let caller_watch = zbus::block_on(screensaver::watch_callers(
            &connection,
            screensaver_requests,
        ))
        .inspect_err(|err| tracing::warn!("can't watch for callers that leave the bus: {err}"))
        .ok()
        .map(|watch| connection.executor().spawn(watch, "screensaver callers"));
        let dbus = Dbus {
            _caller_watch: caller_watch,
            connection,
        };
        tracing::info!(
            "on the session bus as {}",
            dbus.unique_name().unwrap_or_default()
        );
        self.dbus = Some(dbus);
        Ok(())
    }

    /// Whether Camino's interfaces are served and its names are owned, or were refused
    /// (62). The session's start-up waits for it before it spawns mate-session (61). It
    /// never holds if `start_dbus` failed: then there are no names to wait for.
    pub fn dbus_ready(&self) -> bool {
        self.dbus.is_some()
    }

    /// A sender for an interface's requests, which `answer` gets in the event loop.
    fn dbus_requests<R: 'static>(&self, answer: fn(&mut Camino, R)) -> channel::Sender<R> {
        let (sender, requests) = channel::channel();
        self.loop_handle
            .insert_source(requests, move |event, _, state: &mut State| match event {
                channel::Event::Msg(request) => answer(&mut state.camino, request),
                channel::Event::Closed => {}
            })
            .expect("a D-Bus channel joins the event loop");
        sender
    }
}
