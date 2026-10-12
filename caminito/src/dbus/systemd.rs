//! The Session's environment on the bus (08, 14): the D-Bus activation environment, which
//! the bus itself keeps, and systemd's user manager, `org.freedesktop.systemd1`.
//!
//! A nested run's private bus has no systemd on it (14), so the manager is called only when
//! its name has an owner. Caminito never runs `systemctl`.
//!
//! The portal is restarted once systemd has the environment (232). mate-session restarts the
//! user's bus as the last Session ends (74), and systemd starts xdg-desktop-portal again on
//! the new bus with the variables already forgotten. In a user manager that outlives the
//! login, that portal is still there at the next one, and what it launches for a client, a
//! browser for a link, has no display.
//!
//! These run off the event loop and log nothing: the `session` module logs what they
//! return, from the event loop.

use std::collections::HashMap;

use zbus::fdo;
use zbus::names::BusName;

/// systemd's user manager.
pub(crate) const NAME: &str = "org.freedesktop.systemd1";
const PATH: &str = "/org/freedesktop/systemd1";
const INTERFACE: &str = "org.freedesktop.systemd1.Manager";

/// xdg-desktop-portal's unit. Its backends aren't restarted: the ones a MATE login has
/// (`mate-portals.conf`) exit without a display, so none is left from before.
const PORTAL: &str = "xdg-desktop-portal.service";
/// The manager's error for a unit it has no file for.
const NO_SUCH_UNIT: &str = "org.freedesktop.systemd1.NoSuchUnit";

/// What became of an export.
pub(crate) struct Exported {
    /// The bus's `UpdateActivationEnvironment`.
    pub(crate) activation: zbus::Result<()>,
    /// The manager's `SetEnvironment`: whether it was there to call.
    pub(crate) systemd: zbus::Result<bool>,
    /// The manager's `TryRestartUnit` of the portal, which is asked only once the manager
    /// has the environment.
    pub(crate) portal: zbus::Result<()>,
}

/// Gives `variables` to the activation environment, and to systemd's user manager if it's
/// on the bus, which then restarts the portal. It returns once each has answered.
pub(crate) async fn export(
    connection: &zbus::Connection,
    variables: &[(String, String)],
) -> Exported {
    let activation = update_activation_environment(connection, variables).await;
    let systemd = set_environment(connection, variables).await;
    let portal = match systemd {
        Ok(true) => restart_portal(connection).await,
        _ => Ok(()),
    };
    Exported {
        activation,
        systemd,
        portal,
    }
}

async fn update_activation_environment(
    connection: &zbus::Connection,
    variables: &[(String, String)],
) -> zbus::Result<()> {
    let bus = fdo::DBusProxy::new(connection).await?;
    let variables: HashMap<&str, &str> = variables
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    Ok(bus.update_activation_environment(variables).await?)
}

async fn set_environment(
    connection: &zbus::Connection,
    variables: &[(String, String)],
) -> zbus::Result<bool> {
    let bus = fdo::DBusProxy::new(connection).await?;
    if !bus.name_has_owner(BusName::from_static_str(NAME)?).await? {
        return Ok(false);
    }
    let assignments: Vec<String> = variables
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect();
    connection
        .call_method(
            Some(NAME),
            PATH,
            Some(INTERFACE),
            "SetEnvironment",
            &(assignments,),
        )
        .await?;
    Ok(true)
}

/// Restarts the portal if it runs, for it to start again with the environment the manager
/// now has. One that doesn't run is started by the bus when a client first asks for it, and
/// a machine without xdg-desktop-portal has no unit to restart.
async fn restart_portal(connection: &zbus::Connection) -> zbus::Result<()> {
    let restarted = connection
        .call_method(
            Some(NAME),
            PATH,
            Some(INTERFACE),
            "TryRestartUnit",
            &(PORTAL, "replace"),
        )
        .await;
    match restarted {
        Err(zbus::Error::MethodError(name, ..)) if name == NO_SUCH_UNIT => Ok(()),
        restarted => restarted.map(drop),
    }
}

/// Has systemd's user manager forget `names`, over a connection of its own, made from
/// `builder`. Caminito's may be to a bus that has stopped and been replaced, and then nothing
/// runs what's spawned on its executor (74).
pub(crate) async fn unset(
    builder: zbus::connection::Builder<'static>,
    names: &[&str],
) -> zbus::Result<()> {
    let connection = builder.build().await?;
    connection
        .call_method(
            Some(NAME),
            PATH,
            Some(INTERFACE),
            "UnsetEnvironment",
            &(names,),
        )
        .await?;
    Ok(())
}
