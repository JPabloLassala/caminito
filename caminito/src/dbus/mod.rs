//! Caminito's D-Bus (15): one file per name or peer. `systemd`, `panel` and `global_menu`'s
//! applet are the peers so far.
//!
//! zbus runs async on its own executor thread. Interface methods send a request with a
//! reply sender into the event loop over a `calloop::channel` and await the reply, so the
//! main thread never waits on the bus.

pub mod caminito;
pub mod debug;
pub(crate) mod global_menu;
pub(crate) mod panel;
pub mod screensaver;
pub(crate) mod systemd;

use futures_channel::oneshot;
use smithay::reexports::calloop::channel;
use zbus::export::ordered_stream::OrderedStreamExt;
use zbus::message::Header;
use zbus::names::BusName;

use crate::state::{Caminito, State};

/// Which names Caminito's connection asks the bus for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Names {
    /// Its well-known names: `org.mate.Caminito`, which carried patches look for an owner of
    /// (ADR 0011), `org.mate.Caminito.Debug`, and the Screensaver shim's two (62).
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
    let gone = || zbus::fdo::Error::Failed("Caminito's event loop has stopped".into());
    requests.send(request(reply)).map_err(|_| gone())?;
    answer.await.map_err(|_| gone())
}

/// The unique name of the connection a call came from. The bus fills it in for every
/// message it passes on.
fn caller(header: &Header<'_>) -> String {
    header.sender().map(ToString::to_string).unwrap_or_default()
}

/// Whether `name` has an owner. An answer that can't be had counts as yes: the caller
/// watch still reports it when it leaves.
async fn on_the_bus(connection: &zbus::Connection, name: &str) -> bool {
    let has_owner = async {
        let bus = zbus::fdo::DBusProxy::new(connection).await?;
        zbus::Result::Ok(bus.name_has_owner(BusName::try_from(name)?).await?)
    };
    has_owner.await.unwrap_or(true)
}

/// Where the event loop hears of the callers that left the bus, by their unique names, so
/// that what they held goes with them: the shim's cookies (62) and the accelerator grabs
/// (98).
#[derive(Clone)]
struct Callers(channel::Sender<String>);

impl Callers {
    /// Reports `caller` if it has left the bus, after a call that gave it something to
    /// hold. The caller watch is another task, so a caller that left at once can have
    /// been reported before the loop got its call, and what it holds would stay for good.
    /// A caller that's still on the bus now is reported after it.
    async fn report_if_gone(&self, connection: &zbus::Connection, caller: String) {
        if !on_the_bus(connection, &caller).await {
            let _ = self.0.send(caller);
        }
    }

    /// Reports each connection that leaves the bus, until the task is dropped. It
    /// returns once the bus is sending the changes, and the future it returns then passes
    /// them on.
    async fn watch(
        self,
        connection: &zbus::Connection,
    ) -> zbus::Result<impl Future<Output = ()> + Send + 'static> {
        let bus = zbus::fdo::DBusProxy::new(connection).await?;
        // An empty new owner: the name has gone.
        let mut changes = bus.receive_name_owner_changed_with_args(&[(2, "")]).await?;
        Ok(async move {
            // The proxy stays with the stream, which borrows nothing from it but needs its
            // match rule kept.
            let _bus = bus;
            while let Some(change) = changes.next().await {
                let Ok(args) = change.args() else { continue };
                if let BusName::Unique(name) = args.name()
                    && self.0.send(name.to_string()).is_err()
                {
                    break;
                }
            }
        })
    }
}

/// Caminito's connection to the session bus.
pub struct Dbus {
    /// Passes on the callers that leave the bus (62, 98). It's before the connection, which
    /// it holds, so that it stops first.
    _caller_watch: Option<zbus::Task<()>>,
    /// Passes on whether the Global menu applet is on the bus (110).
    _applet_watch: Option<zbus::Task<()>>,
    /// The registrar's name, which follows the applet (112).
    registrar: global_menu::Registrar,
    connection: zbus::Connection,
    /// The bus's address, if it isn't the session bus's: a test's own bus.
    address: Option<String>,
}

impl Dbus {
    /// The connection, for the Session's calls to the bus and to systemd (61).
    pub(crate) fn connection(&self) -> &zbus::Connection {
        &self.connection
    }

    /// A builder for another connection to the same bus, which finds the bus that has
    /// replaced the one `connection` was to (74).
    pub(crate) fn builder(&self) -> zbus::Result<zbus::connection::Builder<'static>> {
        builder(self.address.as_deref())
    }

    /// The connection's unique name on the bus.
    pub fn unique_name(&self) -> Option<String> {
        self.connection.unique_name().map(|name| name.to_string())
    }
}

/// A builder for a connection to the bus at `address`, or to the session bus.
fn builder(address: Option<&str>) -> zbus::Result<zbus::connection::Builder<'static>> {
    match address {
        Some(address) => zbus::connection::Builder::address(address),
        None => zbus::connection::Builder::session(),
    }
}

impl Caminito {
    /// Connects to the session bus and serves Caminito's interfaces on it. Connecting is the
    /// one time the main thread waits on the bus, and asking for the names is part of it:
    /// it returns once the Screensaver shim's names are owned or refused (62), so
    /// [`Caminito::dbus_ready`] holds from then on.
    pub fn start_dbus(&mut self, names: Names) -> zbus::Result<()> {
        self.start_dbus_at(names, None)
    }

    /// [`Caminito::start_dbus`] on the bus at `address`, or on the session bus.
    pub(crate) fn start_dbus_at(
        &mut self,
        names: Names,
        address: Option<&str>,
    ) -> zbus::Result<()> {
        let caminito_requests = self.dbus_requests(Caminito::caminito_request);
        let debug_requests = self.dbus_requests(Caminito::debug_request);
        let screensaver_requests = self.dbus_requests(Caminito::screensaver_request);
        let callers = Callers(self.dbus_requests(Caminito::caller_left));

        let caminito = caminito::CaminitoInterface::new(caminito_requests, callers.clone());
        let mut builder = builder(address)?
            .serve_at(caminito::PATH, caminito)?
            .serve_at(debug::PATH, debug::DebugInterface::new(debug_requests))?;
        builder = screensaver::serve(builder, screensaver_requests, callers.clone())?;
        if names == Names::WellKnown {
            builder = builder.name(caminito::NAME)?.name(debug::NAME)?;
        }
        let connection = zbus::block_on(builder.build())?;
        if names == Names::WellKnown {
            zbus::block_on(screensaver::own_names(&connection));
        }
        let caller_watch = zbus::block_on(callers.watch(&connection))
            .inspect_err(|err| tracing::warn!("can't watch for callers that leave the bus: {err}"))
            .ok()
            .map(|watch| connection.executor().spawn(watch, "callers that leave"));
        let applet = self.dbus_requests(|caminito: &mut Caminito, present| {
            let now = caminito.now();
            caminito.global_menu_applet_changed(present, now);
        });
        let applet_watch = zbus::block_on(global_menu::watch_applet(&connection, applet))
            .inspect_err(|err| tracing::warn!("can't watch for the global menu applet: {err}"))
            .ok()
            .map(|watch| connection.executor().spawn(watch, "global menu applet"));
        let registrar_released = self.dbus_requests(Caminito::registrar_released);
        let dbus = Dbus {
            _caller_watch: caller_watch,
            _applet_watch: applet_watch,
            registrar: global_menu::Registrar::new(registrar_released),
            connection,
            address: address.map(str::to_owned),
        };
        tracing::info!("on the bus as {}", dbus.unique_name().unwrap_or_default());
        self.dbus = Some(dbus);
        self.properties_read_from_now_on();
        Ok(())
    }

    /// Whether Caminito's interfaces are served and its names are owned, or were refused
    /// (62). The session's start-up waits for it before it spawns mate-session (61). It
    /// never holds if `start_dbus` failed: then there are no names to wait for.
    pub fn dbus_ready(&self) -> bool {
        self.dbus.is_some()
    }

    /// Asks the bus for the registrar's name, or gives it up, if that's not what was last
    /// done.
    pub(crate) fn refresh_registrar(&mut self, owned: bool) {
        if let Some(dbus) = &mut self.dbus {
            dbus.registrar.set_owned(&dbus.connection, owned);
        }
    }

    /// Whether giving the registrar's name up at `change` was the last thing asked of the
    /// bus.
    pub(crate) fn registrar_is_given_up(&self, change: u64) -> bool {
        let dbus = self.dbus.as_ref();
        dbus.is_none_or(|dbus| dbus.registrar.is_given_up(change))
    }

    /// A caller left the bus, and what it held goes with it.
    fn caller_left(&mut self, caller: String) {
        self.screensaver.caller_left(&caller);
        self.accelerator_grabs.owner_left(&caller);
    }

    /// A sender for an interface's requests, which `answer` gets in the event loop.
    fn dbus_requests<R: 'static>(&self, answer: fn(&mut Caminito, R)) -> channel::Sender<R> {
        let (sender, requests) = channel::channel();
        self.loop_handle
            .insert_source(requests, move |event, _, state: &mut State| match event {
                channel::Event::Msg(request) => answer(&mut state.caminito, request),
                channel::Event::Closed => {}
            })
            .expect("a D-Bus channel joins the event loop");
        sender
    }
}
