//! mate-session's inhibitors (06, 128): an application that asks mate-session to keep the
//! session from going idle, `Inhibit` with the idle flag on `org.gnome.SessionManager`,
//! holds Caminito's idle off as an idle-inhibit surface does. Caminito asks `IsInhibited`
//! as mate-session starts and whenever it says its inhibitors changed.
//!
//! mate-session's own idle presence follows `org.mate.ScreenSaver`'s `ActiveChanged`, and
//! X SYNC's idle time from XWayland, which sees only X input (06); on a change it calls
//! logind's `SetIdleHint`, and nothing else (`gsm-presence.c:183-195`,
//! `gsm-manager.c:2814-2835`, mate-session-manager 1.28.0).

use smithay::reexports::calloop::channel;
use zbus::export::ordered_stream::{self, OrderedStreamExt};
use zbus::message::Type;
use zbus::proxy::{Builder, CacheProperties, MethodFlags};
use zbus::{MatchRule, MessageStream};

/// mate-session's name, and the object and interface its inhibitors are on.
const NAME: &str = "org.gnome.SessionManager";
const PATH: &str = "/org/gnome/SessionManager";
const INTERFACE: &str = "org.gnome.SessionManager";

/// `IsInhibited`'s flag for "the session being marked as idle"
/// (`org.gnome.SessionManager.xml:146-148`).
const IDLE: u32 = 8;

/// Tells the event loop whether mate-session holds idle off, now and whenever its inhibitors
/// or its name's owner change, until the task is dropped. It returns once the bus is sending
/// the changes, and the future it returns then passes them on.
pub(super) async fn watch(
    connection: &zbus::Connection,
    inhibited: channel::Sender<bool>,
) -> zbus::Result<impl Future<Output = ()> + Send + 'static> {
    let owner = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender("org.freedesktop.DBus")?
        .interface("org.freedesktop.DBus")?
        .member("NameOwnerChanged")?
        .arg(0, NAME)?
        .build();
    // `InhibitorAdded` and `InhibitorRemoved`, and the interface's other signals, which ask
    // again for nothing.
    let signals = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(NAME)?
        .path(PATH)?
        .interface(INTERFACE)?
        .build();
    let owners = MessageStream::for_match_rule(owner, connection, None).await?;
    let signals = MessageStream::for_match_rule(signals, connection, None).await?;
    let manager = Builder::<zbus::Proxy<'_>>::new(connection)
        .destination(NAME)?
        .path(PATH)?
        .interface(INTERFACE)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    Ok(async move {
        // Asked after the changes are, so that none meanwhile is missed.
        let mut changes = ordered_stream::join(owners, signals);
        loop {
            if inhibited.send(is_inhibited(&manager).await).is_err() {
                break;
            }
            if changes.next().await.is_none() {
                break;
            }
        }
    })
}

/// Whether mate-session says an inhibitor holds idle off. Without mate-session, none does.
/// It's never started for this: Caminito asks only the one that runs.
async fn is_inhibited(manager: &zbus::Proxy<'_>) -> bool {
    let flags = MethodFlags::NoAutoStart.into();
    let answer = manager
        .call_with_flags::<_, _, bool>("IsInhibited", flags, &(IDLE,))
        .await;
    match answer {
        Ok(inhibited) => inhibited.unwrap_or(false),
        Err(err) => {
            tracing::debug!("can't ask mate-session for its inhibitors: {err}");
            false
        }
    }
}
