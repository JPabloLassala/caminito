//! The Session's environment on the bus (08, 14): the D-Bus activation environment, which
//! the bus itself keeps, and systemd's user manager, `org.freedesktop.systemd1`.
//!
//! A nested run's private bus has no systemd on it (14), so the manager is called only when
//! its name has an owner. Camino never runs `systemctl`.
//!
//! These run on zbus's executor and log nothing: the `session` module logs what they
//! return, from the event loop.

use std::collections::HashMap;

use zbus::fdo;
use zbus::names::BusName;

/// systemd's user manager.
pub(crate) const NAME: &str = "org.freedesktop.systemd1";
const PATH: &str = "/org/freedesktop/systemd1";
const INTERFACE: &str = "org.freedesktop.systemd1.Manager";

/// What became of an export.
pub(crate) struct Exported {
    /// The bus's `UpdateActivationEnvironment`.
    pub(crate) activation: zbus::Result<()>,
    /// The manager's `SetEnvironment`: whether it was there to call.
    pub(crate) systemd: zbus::Result<bool>,
}

/// Gives `variables` to the activation environment, and to systemd's user manager if it's
/// on the bus. It returns once both have answered.
pub(crate) async fn export(
    connection: &zbus::Connection,
    variables: &[(String, String)],
) -> Exported {
    Exported {
        activation: update_activation_environment(connection, variables).await,
        systemd: set_environment(connection, variables).await,
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

/// Has systemd's user manager forget `names`.
pub(crate) async fn unset(connection: &zbus::Connection, names: &[&str]) -> zbus::Result<()> {
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
