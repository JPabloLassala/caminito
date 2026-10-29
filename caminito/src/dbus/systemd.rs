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
//! The export then waits for the portal that takes the old one's place to answer (CMN-284).
//! A GTK4 program reads the font and its DPI from the portal's Settings as it starts, and
//! one that finds no portal there takes them from `~/.config/gtk-4.0/settings.ini` for good,
//! a file another desktop may have written. The notification daemon starts next, and
//! mate-session's programs after it, so each would.
//!
//! These run off the event loop and log nothing: the `session` module logs what they
//! return, from the event loop.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use zbus::export::ordered_stream::OrderedStreamExt;
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

/// The portal's name on the bus, and its Settings, where GTK4 reads the font from.
const PORTAL_NAME: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const SETTINGS: &str = "org.freedesktop.portal.Settings";
/// The prefix of the bus's own errors, as for a service it couldn't start.
const BUS_ERRORS: &str = "org.freedesktop.DBus.Error.";
/// The setting the restarted portal is asked for: the font GTK4 takes.
const FONT: (&str, &str) = ("org.gnome.desktop.interface", "font-name");

/// How long the restarted portal has to answer before the Session starts without it.
pub(crate) const PORTAL_WAIT: Duration = Duration::from_secs(5);

/// What became of an export.
pub(crate) struct Exported {
    /// The bus's `UpdateActivationEnvironment`.
    pub(crate) activation: zbus::Result<()>,
    /// The manager's `SetEnvironment`: whether it was there to call.
    pub(crate) systemd: zbus::Result<bool>,
    /// The manager's `TryRestartUnit` of the portal, which is asked only once the manager
    /// has the environment, and the restarted portal's answer.
    pub(crate) portal: Result<(), PortalError>,
}

/// Why the portal may not have the Session's environment, or not be up yet.
#[derive(Debug)]
pub(crate) enum PortalError {
    /// The bus wouldn't say who has the portal's name, or tell of its changes.
    Watch(zbus::Error),
    /// The manager failed the restart.
    Restart(zbus::Error),
    /// The bus answered for the restarted portal, which it couldn't start.
    Unreachable(zbus::Error),
    /// The restarted portal didn't answer within the wait.
    Unanswered(Duration),
}

impl fmt::Display for PortalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PortalError::Watch(err) => write!(f, "can't watch the portal's name: {err}"),
            PortalError::Restart(err) => {
                write!(
                    f,
                    "can't restart the portal, which may have no display: {err}"
                )
            }
            PortalError::Unreachable(err) => write!(
                f,
                "can't reach the restarted portal, so what starts now may miss the Session's \
                 font settings: {err}"
            ),
            PortalError::Unanswered(wait) => write!(
                f,
                "can't hear from the restarted portal within {wait:?}, so what starts now may \
                 miss the Session's font settings"
            ),
        }
    }
}

impl std::error::Error for PortalError {}

/// Gives `variables` to the activation environment, and to systemd's user manager if it's
/// on the bus, which then restarts the portal. It returns once each has answered, and the
/// restarted portal too, or `portal_wait` after the restart.
pub(crate) async fn export(
    connection: &zbus::Connection,
    variables: &[(String, String)],
    portal_wait: Duration,
) -> Exported {
    let activation = update_activation_environment(connection, variables).await;
    let systemd = set_environment(connection, variables).await;
    let portal = match systemd {
        Ok(true) => restart_portal(connection, portal_wait).await,
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
/// now has, and returns once the one that takes its place answers, or after `wait`. One
/// that doesn't run is started by the bus when a client first asks for it, and a machine
/// without xdg-desktop-portal has no unit to restart.
async fn restart_portal(connection: &zbus::Connection, wait: Duration) -> Result<(), PortalError> {
    let watch = async {
        let bus = fdo::DBusProxy::new(connection).await?;
        // Before the restart, so that the old portal's leaving can't be missed.
        let changes = bus
            .receive_name_owner_changed_with_args(&[(0, PORTAL_NAME)])
            .await?;
        let running = bus
            .get_name_owner(BusName::from_static_str(PORTAL_NAME)?)
            .await
            .ok();
        zbus::Result::Ok((bus, changes, running))
    };
    // The proxy stays with the stream, which needs its match rule kept.
    let (_bus, mut changes, running) = watch.await.map_err(PortalError::Watch)?;
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
        Err(zbus::Error::MethodError(name, ..)) if name == NO_SUCH_UNIT => return Ok(()),
        restarted => restarted.map(drop).map_err(PortalError::Restart)?,
    }
    let Some(running) = running else {
        return Ok(());
    };
    let answered = async {
        while let Some(change) = changes.next().await {
            let left = change.args().is_ok_and(|args| {
                args.old_owner()
                    .as_ref()
                    .is_some_and(|old| old.as_str() == running.as_str())
            });
            if left {
                break;
            }
        }
        // The bus holds the call until the new portal has the name. Any answer of the
        // portal's, even an error for a key it doesn't have, is its Settings at work; one
        // of the bus's is a portal it couldn't start.
        let read = connection
            .call_method(
                Some(PORTAL_NAME),
                PORTAL_PATH,
                Some(SETTINGS),
                "ReadOne",
                &FONT,
            )
            .await;
        match read {
            Err(zbus::Error::MethodError(name, detail, reply))
                if name.as_str().starts_with(BUS_ERRORS) =>
            {
                let err = zbus::Error::MethodError(name, detail, reply);
                Err(PortalError::Unreachable(err))
            }
            _ => Ok(()),
        }
    };
    let timeout = async {
        async_io::Timer::after(wait).await;
        Err(PortalError::Unanswered(wait))
    };
    futures_lite::future::or(answered, timeout).await
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
