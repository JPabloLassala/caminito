//! The Screensaver shim's shell (12, 62): `org.mate.ScreenSaver` and
//! `org.freedesktop.ScreenSaver`, owned so that mate-screensaver's daemon finds its name
//! taken and exits. M7 gives it the lock, idle and inhibit state (17). Until then `Lock` is
//! refused, the session is never active, and the cookies are kept but not read.
//!
//! The methods are mate-screensaver 1.28.0's (`gs-listener-dbus.c:1373-1417`), without
//! `Quit` (12).

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use futures_channel::oneshot;
use smithay::reexports::calloop::channel::Sender;
use zbus::fdo::{self, RequestNameFlags, RequestNameReply};
use zbus::message::Header;
use zbus::names::BusName;

use super::{Callers, caller};
use crate::state::Caminito;

/// The names mate-screensaver's daemon and X11 applications look for.
pub(crate) const NAMES: [&str; 2] = ["org.mate.ScreenSaver", "org.freedesktop.ScreenSaver"];
/// Where `org.mate.ScreenSaver` is served.
const MATE_PATH: &str = "/org/mate/ScreenSaver";
/// `org.freedesktop.ScreenSaver` is served at both (12).
const FREEDESKTOP_PATHS: [&str; 2] = ["/org/freedesktop/ScreenSaver", "/ScreenSaver"];

/// What a cookie holds back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HoldKind {
    /// Idle, from M7.
    Inhibit,
    /// Nothing: throttling only slows screensaver themes (12).
    Throttle,
}

/// A call, for the event loop to answer.
pub(super) enum Request {
    /// `Lock`, from `caller`.
    Lock {
        caller: String,
        reply: oneshot::Sender<Result<(), Refused>>,
    },
    /// `Unlock`, from `caller`.
    Unlock {
        caller: String,
        reply: oneshot::Sender<Result<(), Refused>>,
    },
    /// `SetActive`.
    SetActive {
        active: bool,
        reply: oneshot::Sender<()>,
    },
    /// `GetActive`: whether the session is blanked or locked.
    GetActive(oneshot::Sender<bool>),
    /// `GetActiveTime`: for how many seconds it has been.
    GetActiveTime(oneshot::Sender<u32>),
    /// `Inhibit` or `Throttle`, answered with the new cookie.
    Hold {
        kind: HoldKind,
        caller: String,
        application: String,
        reason: String,
        reply: oneshot::Sender<u32>,
    },
    /// `UnInhibit` or `UnThrottle`.
    Release {
        kind: HoldKind,
        caller: String,
        cookie: u32,
        reply: oneshot::Sender<()>,
    },
    /// `GetInhibitors`: a description of each.
    GetInhibitors(oneshot::Sender<Vec<String>>),
}

/// Why the event loop refused a call.
#[derive(Debug)]
pub(super) enum Refused {
    /// The session can't lock until M7. A caller told that it locked would be wrong in
    /// the unsafe direction (62).
    NoLockYet,
    /// Only the locker or logind unlocks (12).
    UnlockIsTheLockers,
}

impl From<Refused> for fdo::Error {
    fn from(refused: Refused) -> Self {
        match refused {
            Refused::NoLockYet => Self::NotSupported("Caminito can't lock the session yet".into()),
            Refused::UnlockIsTheLockers => {
                Self::AccessDenied("only the locker or logind unlocks the session".into())
            }
        }
    }
}

/// One caller's inhibitor or throttle.
struct Hold {
    kind: HoldKind,
    /// The caller's unique name on the bus.
    caller: String,
    application: String,
    reason: String,
    /// Seconds since the Unix epoch.
    since: u64,
}

/// The Screensaver shim's state: the cookies it has handed out.
#[derive(Default)]
pub struct ScreensaverShim {
    holds: BTreeMap<u32, Hold>,
    last_cookie: u32,
}

impl ScreensaverShim {
    /// A cookie no hold has, and never 0, which callers use for "none".
    fn hold(&mut self, hold: Hold) -> u32 {
        loop {
            self.last_cookie = self.last_cookie.wrapping_add(1);
            if self.last_cookie != 0 && !self.holds.contains_key(&self.last_cookie) {
                break;
            }
        }
        self.holds.insert(self.last_cookie, hold);
        self.last_cookie
    }

    /// Takes back `cookie` if it's `caller`'s hold of `kind`. mate-screensaver takes back
    /// any caller's (`gs-listener-dbus.c:1030`, a FIXME there), but they're kept per caller (12).
    fn release(&mut self, kind: HoldKind, caller: &str, cookie: u32) {
        let held = self
            .holds
            .get(&cookie)
            .is_some_and(|hold| hold.kind == kind && hold.caller == caller);
        if held {
            self.holds.remove(&cookie);
        } else {
            tracing::debug!("{caller} has no {kind:?} cookie {cookie} to give back");
        }
    }

    /// Drops the cookies of a caller that left the bus.
    pub(super) fn caller_left(&mut self, caller: &str) {
        self.holds.retain(|_, hold| hold.caller != caller);
    }

    /// The inhibitors as mate-screensaver describes them (`gs-listener-dbus.c:840`).
    fn inhibitors(&self) -> Vec<String> {
        self.holds
            .values()
            .filter(|hold| hold.kind == HoldKind::Inhibit)
            .map(|hold| {
                format!(
                    "Application=\"{}\"; Since=\"{}\"; Reason=\"{}\";",
                    hold.application,
                    iso8601(hold.since),
                    hold.reason
                )
            })
            .collect()
    }
}

/// `seconds` since the Unix epoch as `2026-10-04T12:00:00Z`.
fn iso8601(seconds: u64) -> String {
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Howard Hinnant's `civil_from_days`, for days since 1970-01-01:
    // https://howardhinnant.github.io/date_algorithms.html#civil_from_days
    let z = days + 719_468;
    let era = z / 146_097;
    let day_of_era = z % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

impl Caminito {
    pub(super) fn screensaver_request(&mut self, request: Request) {
        // A caller that has gone away doesn't want the answer.
        match request {
            Request::Lock { caller, reply } => {
                tracing::warn!("can't lock the session for {caller}: the lock comes with M7");
                let _ = reply.send(Err(Refused::NoLockYet));
            }
            Request::Unlock { caller, reply } => {
                tracing::warn!("can't unlock the session for {caller}: only the locker does");
                let _ = reply.send(Err(Refused::UnlockIsTheLockers));
            }
            Request::SetActive { active, reply } => {
                tracing::warn!("can't set the session active to {active}: blanking comes with M7");
                let _ = reply.send(());
            }
            Request::GetActive(reply) => {
                let _ = reply.send(false);
            }
            Request::GetActiveTime(reply) => {
                let _ = reply.send(0);
            }
            Request::Hold {
                kind,
                caller,
                application,
                reason,
                reply,
            } => {
                let since = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |since| since.as_secs());
                let cookie = self.screensaver.hold(Hold {
                    kind,
                    caller,
                    application,
                    reason,
                    since,
                });
                let _ = reply.send(cookie);
            }
            Request::Release {
                kind,
                caller,
                cookie,
                reply,
            } => {
                self.screensaver.release(kind, &caller, cookie);
                let _ = reply.send(());
            }
            Request::GetInhibitors(reply) => {
                let _ = reply.send(self.screensaver.inhibitors());
            }
        }
    }
}

/// The way into the event loop that both interfaces share.
#[derive(Clone)]
struct Requests {
    requests: Sender<Request>,
    callers: Callers,
}

impl Requests {
    /// Sends the event loop a request, and waits for its reply.
    async fn ask<T>(&self, request: impl FnOnce(oneshot::Sender<T>) -> Request) -> fdo::Result<T> {
        super::ask(&self.requests, request).await
    }

    /// Hands `header`'s sender a cookie.
    async fn hold(
        &self,
        kind: HoldKind,
        header: &Header<'_>,
        connection: &zbus::Connection,
        application: String,
        reason: String,
    ) -> fdo::Result<u32> {
        let caller = caller(header);
        let hold = |reply| Request::Hold {
            kind,
            caller: caller.clone(),
            application,
            reason,
            reply,
        };
        let cookie = self.ask(hold).await?;
        self.callers.report_if_gone(connection, caller).await;
        Ok(cookie)
    }

    async fn release(&self, kind: HoldKind, header: &Header<'_>, cookie: u32) -> fdo::Result<()> {
        let caller = caller(header);
        self.ask(|reply| Request::Release {
            kind,
            caller,
            cookie,
            reply,
        })
        .await
    }
}

/// `org.mate.ScreenSaver`, with mate-screensaver's methods.
struct MateInterface {
    requests: Requests,
}

#[zbus::interface(name = "org.mate.ScreenSaver")]
impl MateInterface {
    /// Refused until M7.
    async fn lock(&self, #[zbus(header)] header: Header<'_>) -> fdo::Result<()> {
        let caller = caller(&header);
        let locked = self.requests.ask(|reply| Request::Lock { caller, reply });
        Ok(locked.await??)
    }

    /// Always refused.
    async fn unlock(&self, #[zbus(header)] header: Header<'_>) -> fdo::Result<()> {
        let caller = caller(&header);
        let unlocked = self.requests.ask(|reply| Request::Unlock { caller, reply });
        Ok(unlocked.await??)
    }

    /// Accepted, with no effect: there are no screensaver themes to cycle.
    async fn cycle(&self) {}

    /// Accepted, with no effect until M7's idle.
    async fn simulate_user_activity(&self) {}

    #[zbus(out_args("cookie"))]
    async fn inhibit(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
        application_name: String,
        reason: String,
    ) -> fdo::Result<u32> {
        self.requests
            .hold(
                HoldKind::Inhibit,
                &header,
                connection,
                application_name,
                reason,
            )
            .await
    }

    async fn un_inhibit(&self, #[zbus(header)] header: Header<'_>, cookie: u32) -> fdo::Result<()> {
        self.requests
            .release(HoldKind::Inhibit, &header, cookie)
            .await
    }

    /// A description of each inhibitor.
    #[zbus(out_args("list"))]
    async fn get_inhibitors(&self) -> fdo::Result<Vec<String>> {
        self.requests.ask(Request::GetInhibitors).await
    }

    #[zbus(out_args("cookie"))]
    async fn throttle(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
        application_name: String,
        reason: String,
    ) -> fdo::Result<u32> {
        self.requests
            .hold(
                HoldKind::Throttle,
                &header,
                connection,
                application_name,
                reason,
            )
            .await
    }

    async fn un_throttle(
        &self,
        #[zbus(header)] header: Header<'_>,
        cookie: u32,
    ) -> fdo::Result<()> {
        self.requests
            .release(HoldKind::Throttle, &header, cookie)
            .await
    }

    /// False until M7: the session neither blanks nor locks.
    #[zbus(out_args("value"))]
    async fn get_active(&self) -> fdo::Result<bool> {
        self.requests.ask(Request::GetActive).await
    }

    #[zbus(out_args("seconds"))]
    async fn get_active_time(&self) -> fdo::Result<u32> {
        self.requests.ask(Request::GetActiveTime).await
    }

    /// Logged, with no effect until M7.
    async fn set_active(&self, value: bool) -> fdo::Result<()> {
        self.requests
            .ask(|reply| Request::SetActive {
                active: value,
                reply,
            })
            .await
    }

    /// Accepted, with no effect.
    async fn show_message(&self, summary: String, body: String, icon: String) {
        // Named as mate-screensaver names them, for introspection.
        let _ = (summary, body, icon);
    }

    /// Never sent until M7, but declared, so that a caller that connects to it finds it.
    #[zbus(signal)]
    async fn active_changed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        new_value: bool,
    ) -> zbus::Result<()>;
}

/// `org.freedesktop.ScreenSaver`: the inhibitors only, the same set as
/// `org.mate.ScreenSaver`'s (12).
struct FreedesktopInterface {
    requests: Requests,
}

#[zbus::interface(name = "org.freedesktop.ScreenSaver")]
impl FreedesktopInterface {
    #[zbus(out_args("cookie"))]
    async fn inhibit(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
        application_name: String,
        reason_for_inhibit: String,
    ) -> fdo::Result<u32> {
        self.requests
            .hold(
                HoldKind::Inhibit,
                &header,
                connection,
                application_name,
                reason_for_inhibit,
            )
            .await
    }

    async fn un_inhibit(&self, #[zbus(header)] header: Header<'_>, cookie: u32) -> fdo::Result<()> {
        self.requests
            .release(HoldKind::Inhibit, &header, cookie)
            .await
    }
}

/// Adds the shim's interfaces to a connection being built.
pub(super) fn serve<'a>(
    mut builder: zbus::connection::Builder<'a>,
    requests: Sender<Request>,
    callers: Callers,
) -> zbus::Result<zbus::connection::Builder<'a>> {
    let requests = Requests { requests, callers };
    builder = builder.serve_at(
        MATE_PATH,
        MateInterface {
            requests: requests.clone(),
        },
    )?;
    for path in FREEDESKTOP_PATHS {
        builder = builder.serve_at(
            path,
            FreedesktopInterface {
                requests: requests.clone(),
            },
        )?;
    }
    Ok(builder)
}

/// Asks the bus for the shim's names, without replacing an owner and without letting one
/// replace Caminito. A name that's taken is logged with its owner, and isn't an error: a
/// nested run on a bus that already has a screensaver must still start (62).
pub(super) async fn own_names(connection: &zbus::Connection) {
    for name in NAMES {
        let reply = connection
            .request_name_with_flags(name, RequestNameFlags::DoNotQueue.into())
            .await;
        match reply {
            Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => {}
            Ok(RequestNameReply::Exists | RequestNameReply::InQueue)
            | Err(zbus::Error::NameTaken) => {
                let owner = owner(connection, name).await;
                // An error, though Caminito carries on: the session can then get an X11
                // lock that Caminito can't show (62).
                tracing::error!("can't own {name}: {owner} has it");
            }
            Err(err) => tracing::error!("can't own {name}: {err}"),
        }
    }
}

/// Who has `name`, for the log.
async fn owner(connection: &zbus::Connection, name: &'static str) -> String {
    let owner = async {
        let bus = fdo::DBusProxy::new(connection).await?;
        let name = BusName::from_static_str(name)?;
        zbus::Result::Ok(bus.get_name_owner(name).await?)
    };
    match owner.await {
        Ok(owner) => owner.to_string(),
        Err(_) => "another connection".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::iso8601;

    #[test]
    fn a_time_is_written_as_iso_8601_in_utc() {
        assert_eq!(iso8601(0), "1970-01-01T00:00:00Z");
        // `date -u -d @1700000000 +%FT%TZ`
        assert_eq!(iso8601(1_700_000_000), "2023-11-14T22:13:20Z");
        // A leap day: `date -u -d @1709210096 +%FT%TZ`
        assert_eq!(iso8601(1_709_210_096), "2024-02-29T12:34:56Z");
    }
}
