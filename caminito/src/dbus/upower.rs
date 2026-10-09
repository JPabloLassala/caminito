//! UPower (09, 129), on the system bus: whether the machine runs on its battery, which picks
//! the `sleep-display` key the outputs are powered off after. Caminito reads `OnBattery` as
//! UPower starts and at each `PropertiesChanged` of its interface. Without UPower the machine
//! is on AC, and a UPS counts as AC unless UPower says battery, as 09 has it.
//!
//! UPower is never started for this, as mate-session isn't (`session_manager`):
//! mate-power-manager's client starts it, and Caminito hears it come.

use smithay::reexports::calloop::channel;
use zbus::export::ordered_stream::{self, OrderedStreamExt};
use zbus::message::Type;
use zbus::proxy::{Builder, CacheProperties, MethodFlags};
use zbus::zvariant::OwnedValue;
use zbus::{MatchRule, MessageStream};

use crate::state::{Caminito, State};

/// UPower's name, and the object and interface `OnBattery` is on.
const NAME: &str = "org.freedesktop.UPower";
const PATH: &str = "/org/freedesktop/UPower";
const INTERFACE: &str = "org.freedesktop.UPower";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

impl Caminito {
    /// Follows UPower's `OnBattery` on the system bus, or on the bus at `address`.
    /// Connecting is the one time the main thread waits on the bus. A bus that can't be
    /// reached logs one line, and the machine counts as on AC.
    pub(crate) fn start_upower(&mut self, address: Option<&str>) {
        let builder = match address {
            Some(address) => zbus::connection::Builder::address(address),
            None => zbus::connection::Builder::system(),
        };
        let connection = match builder.and_then(|builder| zbus::block_on(builder.build())) {
            Ok(connection) => connection,
            Err(err) => {
                tracing::warn!(
                    "can't connect to the system bus, so the outputs power off after \
                     sleep-display-ac: {err}"
                );
                return;
            }
        };
        let (on_battery, changes) = channel::channel();
        self.loop_handle
            .insert_source(changes, |event, _, state: &mut State| {
                if let channel::Event::Msg(on_battery) = event {
                    state.idle_on_battery(on_battery);
                }
            })
            .expect("UPower's channel joins the event loop");
        match zbus::block_on(watch(&connection, on_battery)) {
            Ok(watch) => self.upower = Some(connection.executor().spawn(watch, "UPower")),
            Err(err) => tracing::warn!("can't follow UPower's OnBattery: {err}"),
        }
    }
}

/// Tells the event loop whether the machine runs on its battery, now and whenever UPower's
/// properties or its name's owner change, until the task is dropped. It returns once the bus
/// is sending the changes, and the future it returns then passes them on.
async fn watch(
    connection: &zbus::Connection,
    on_battery: channel::Sender<bool>,
) -> zbus::Result<impl Future<Output = ()> + Send + 'static> {
    let owner = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender("org.freedesktop.DBus")?
        .interface("org.freedesktop.DBus")?
        .member("NameOwnerChanged")?
        .arg(0, NAME)?
        .build();
    let changed = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(NAME)?
        .path(PATH)?
        .interface(PROPERTIES)?
        .member("PropertiesChanged")?
        .arg(0, INTERFACE)?
        .build();
    let owners = MessageStream::for_match_rule(owner, connection, None).await?;
    let changes = MessageStream::for_match_rule(changed, connection, None).await?;
    let properties = Builder::<zbus::Proxy<'_>>::new(connection)
        .destination(NAME)?
        .path(PATH)?
        .interface(PROPERTIES)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    // The task holds the connection, which nothing else does.
    let connection = connection.clone();
    Ok(async move {
        let _connection = connection;
        // Asked after the changes are, so that none meanwhile is missed.
        let mut changes = ordered_stream::join(owners, changes);
        loop {
            if on_battery.send(is_on_battery(&properties).await).is_err() {
                break;
            }
            if changes.next().await.is_none() {
                break;
            }
        }
    })
}

/// Whether UPower says the machine runs on its battery. Without UPower, it doesn't.
async fn is_on_battery(properties: &zbus::Proxy<'_>) -> bool {
    let flags = MethodFlags::NoAutoStart.into();
    let answer = properties
        .call_with_flags::<_, _, OwnedValue>("Get", flags, &(INTERFACE, "OnBattery"))
        .await;
    match answer {
        Ok(value) => value
            .and_then(|value| bool::try_from(value).ok())
            .unwrap_or(false),
        Err(err) => {
            tracing::debug!("can't ask UPower whether the machine is on battery: {err}");
            false
        }
    }
}
