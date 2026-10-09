//! logind (12, 130): Caminito's session, on the system bus. The session's `Lock` signal is a
//! lock request like the shim's, and its `Unlock` ends the lock and stops the locker, which is
//! the text-console recovery from the give-up state (126). The session's locked hint follows
//! each lock and unlock, so `loginctl` and the display manager see it.
//!
//! Caminito holds a `sleep` delay inhibitor, as mate-screensaver does
//! (`gs-listener-dbus.c:1697-1700`). On `PrepareForSleep(true)` it lets it go once a lock that's
//! pending is on screen, the locker's lock surface drawn on every output, or after 2 s, so the
//! machine never sleeps with the desktop on screen and a lock on its way. It takes it again on
//! resume. Whether to lock on suspend is mate-power-manager's key: it calls the shim's `Lock`.
//!
//! A nested run has no logind session of its own (14), so it's never asked for one: the
//! session it would find is the desktop's it's nested in.
//!
//! A task on the connection's executor finds the session, follows its signals and makes the
//! calls. It logs nothing: the event loop hears what became of them over a channel, and logs.

use std::os::fd::OwnedFd;
use std::time::Duration;

use futures_channel::mpsc;
use smithay::reexports::calloop::channel;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::{LoopHandle, RegistrationToken};
use zbus::export::ordered_stream::OrderedStreamExt;
use zbus::zvariant::OwnedObjectPath;

use crate::state::{Caminito, State};

const NAME: &str = "org.freedesktop.login1";
const MANAGER_PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";
const SESSION: &str = "org.freedesktop.login1.Session";

/// How long the sleep delay is held for a lock that isn't on screen (12).
const SLEEP_DELAY: Duration = Duration::from_secs(2);

/// What the task tells the event loop.
enum Event {
    /// The session's `Lock`.
    Lock,
    /// The session's `Unlock`.
    Unlock,
    /// `PrepareForSleep`: whether the machine is about to sleep, or has resumed.
    PrepareForSleep(bool),
    /// A `sleep` delay inhibitor, which is held while it's open.
    SleepDelay(OwnedFd),
    /// What couldn't be done, as the log has it after `can't `, and why.
    Failed(&'static str, zbus::Error),
}

/// Caminito's session in logind, and its hold on sleep.
pub(crate) struct Logind {
    /// The locked hints for the task to set, in order.
    hints: mpsc::UnboundedSender<bool>,
    /// The task, which ends when it's dropped.
    _task: zbus::Task<()>,
    /// The locked hint as last set: a session starts unlocked.
    hint: bool,
    /// The `sleep` delay inhibitor, while it's held.
    sleep_delay: Option<OwnedFd>,
    /// While the machine is about to sleep, the timer that lets the delay go at the latest.
    sleeping: Option<RegistrationToken>,
}

impl Caminito {
    /// Finds Caminito's session in logind on the system bus, or on the bus at `address`, and
    /// follows it. Connecting is the one time the main thread waits on the bus; the rest is the
    /// task's. A bus that can't be reached, or no session, logs one line, and the session
    /// locks as it does without logind.
    pub(crate) fn start_logind(&mut self, address: Option<&str>) {
        let builder = match address {
            Some(address) => zbus::connection::Builder::address(address),
            None => zbus::connection::Builder::system(),
        };
        let connection = match builder.and_then(|builder| zbus::block_on(builder.build())) {
            Ok(connection) => connection,
            Err(err) => {
                tracing::warn!(
                    "can't connect to the system bus, so caminito locks without logind: {err}"
                );
                return;
            }
        };
        let (events, received) = channel::channel();
        self.loop_handle
            .insert_source(received, |event, _, state: &mut State| {
                if let channel::Event::Msg(event) = event {
                    state.logind_event(event);
                }
            })
            .expect("logind's channel joins the event loop");
        let (hints, to_set) = mpsc::unbounded();
        let task = run(
            connection.clone(),
            self.locker.session_id.clone(),
            events,
            to_set,
        );
        self.logind = Some(Logind {
            hints,
            _task: connection.executor().spawn(task, "logind"),
            hint: false,
            sleep_delay: None,
            sleeping: None,
        });
    }

    /// The refresh pass's part of logind: the locked hint follows the lock, and the sleep
    /// delay is let go once the machine may sleep.
    pub(crate) fn refresh_logind(&mut self) {
        let locked = self.lock.is_locked();
        let Some(logind) = &mut self.logind else {
            return;
        };
        if logind.hint != locked {
            logind.hint = locked;
            // The task has ended if this fails, and then there's no session to tell.
            let _ = logind.hints.unbounded_send(locked);
        }
        if logind.sleeping.is_some() && self.may_sleep() {
            self.release_sleep_delay();
        }
    }

    /// Whether nothing of the session can be on screen as the machine sleeps: it's
    /// unlocked, or the locker shows the lock on every output. An unlocked session may sleep
    /// as it is: locking on suspend is mate-power-manager's (12).
    fn may_sleep(&self) -> bool {
        !self.lock.is_locked() || self.lock.is_shown_on(&self.outputs)
    }

    /// The machine is about to sleep: the delay is let go once it may, or after
    /// [`SLEEP_DELAY`].
    fn prepare_for_sleep(&mut self) {
        let handle: &LoopHandle<'static, State> = &self.loop_handle;
        let Some(logind) = &mut self.logind else {
            return;
        };
        if logind.sleep_delay.is_none() || logind.sleeping.is_some() {
            return;
        }
        let timer = Timer::from_duration(SLEEP_DELAY);
        let token = handle
            .insert_source(timer, |_, _, state: &mut State| {
                let caminito = &mut state.caminito;
                if let Some(logind) = &mut caminito.logind {
                    // The timer is dropped as it returns.
                    logind.sleeping = None;
                    tracing::warn!(
                        "the lock isn't on screen {} s after logind asked to sleep, so the \
                         machine sleeps without it",
                        SLEEP_DELAY.as_secs()
                    );
                    logind.sleep_delay = None;
                }
                TimeoutAction::Drop
            })
            .expect("the sleep delay's timer joins the event loop");
        logind.sleeping = Some(token);
        if self.may_sleep() {
            self.release_sleep_delay();
        }
    }

    /// Lets the sleep delay go, so the machine sleeps.
    fn release_sleep_delay(&mut self) {
        let Some(logind) = &mut self.logind else {
            return;
        };
        if let Some(token) = logind.sleeping.take() {
            self.loop_handle.remove(token);
        }
        logind.sleep_delay = None;
    }

    /// The machine has resumed. The task takes the delay again.
    fn resumed(&mut self) {
        if let Some(logind) = &mut self.logind
            && let Some(token) = logind.sleeping.take()
        {
            self.loop_handle.remove(token);
        }
    }
}

impl State {
    fn logind_event(&mut self, event: Event) {
        match event {
            Event::Lock => {
                self.request_lock();
            }
            Event::Unlock => self.caminito.end_lock(),
            Event::PrepareForSleep(true) => self.caminito.prepare_for_sleep(),
            Event::PrepareForSleep(false) => self.caminito.resumed(),
            Event::SleepDelay(fd) => {
                // zbus receives it without close-on-exec. A locker spawned with it would hold
                // the delay as long as it runs.
                if let Err(err) = rustix::io::fcntl_setfd(&fd, rustix::io::FdFlags::CLOEXEC) {
                    tracing::warn!("can't keep logind's sleep delay from the helpers: {err}");
                }
                if let Some(logind) = &mut self.caminito.logind {
                    logind.sleep_delay = Some(fd);
                }
            }
            Event::Failed(what, err) => tracing::warn!("can't {what}: {err}"),
        }
    }
}

/// Finds the session, follows its signals and sets its locked hints, until it's dropped.
async fn run(
    connection: zbus::Connection,
    session_id: Option<String>,
    events: channel::Sender<Event>,
    mut hints: mpsc::UnboundedReceiver<bool>,
) {
    let failed = |what, err| {
        let _ = events.send(Event::Failed(what, err));
    };
    let session = match find_session(&connection, session_id.as_deref()).await {
        Ok(session) => session,
        Err(err) => {
            return failed(
                "find caminito's session in logind, so it locks without logind's lock, unlock \
                 and sleep delay",
                err,
            );
        }
    };
    let proxies = async {
        let manager = zbus::Proxy::new(&connection, NAME, MANAGER_PATH, MANAGER).await?;
        let session = zbus::Proxy::new(&connection, NAME, session, SESSION).await?;
        zbus::Result::Ok((manager, session))
    };
    let (manager, session) = match proxies.await {
        Ok(proxies) => proxies,
        Err(err) => return failed("follow caminito's session in logind", err),
    };
    let signals = async {
        let locks = session.receive_signal("Lock").await?;
        let unlocks = session.receive_signal("Unlock").await?;
        let sleeps = manager.receive_signal("PrepareForSleep").await?;
        zbus::Result::Ok((locks, unlocks, sleeps))
    };
    let (mut locks, mut unlocks, mut sleeps) = match signals.await {
        Ok(signals) => signals,
        Err(err) => return failed("follow caminito's session in logind", err),
    };
    // After the signals are asked for, so that a sleep meanwhile isn't missed.
    take_sleep_delay(&manager, &events).await;

    let executor = connection.executor();
    let forward = |event: fn() -> Event| {
        let events = events.clone();
        move || {
            let _ = events.send(event());
        }
    };
    let lock = forward(|| Event::Lock);
    let _locks = executor.spawn(
        async move {
            while locks.next().await.is_some() {
                lock();
            }
        },
        "logind's lock",
    );
    let unlock = forward(|| Event::Unlock);
    let _unlocks = executor.spawn(
        async move {
            while unlocks.next().await.is_some() {
                unlock();
            }
        },
        "logind's unlock",
    );
    let sleep_events = events.clone();
    let _sleeps = executor.spawn(
        async move {
            while let Some(signal) = sleeps.next().await {
                let Ok(start) = signal.body().deserialize::<bool>() else {
                    continue;
                };
                let _ = sleep_events.send(Event::PrepareForSleep(start));
                if !start {
                    take_sleep_delay(&manager, &sleep_events).await;
                }
            }
        },
        "logind's sleep",
    );
    // The signals' tasks are dropped with this one.
    while let Ok(locked) = hints.recv().await {
        if let Err(err) = session.call_method("SetLockedHint", &(locked,)).await {
            failed("set the session's locked hint in logind", err);
        }
    }
}

/// The session's object: `XDG_SESSION_ID`'s, or else the one Caminito's process is in.
async fn find_session(
    connection: &zbus::Connection,
    id: Option<&str>,
) -> zbus::Result<OwnedObjectPath> {
    let manager = zbus::Proxy::new(connection, NAME, MANAGER_PATH, MANAGER).await?;
    match id {
        Some(id) => manager.call("GetSession", &(id,)).await,
        None => {
            manager
                .call("GetSessionByPID", &(std::process::id(),))
                .await
        }
    }
}

/// Takes a `sleep` delay inhibitor, and gives it to the event loop.
async fn take_sleep_delay(manager: &zbus::Proxy<'_>, events: &channel::Sender<Event>) {
    let why = "Caminito shows the lock before the machine sleeps";
    let inhibitor = manager
        .call::<_, _, zbus::zvariant::OwnedFd>("Inhibit", &("sleep", "Caminito", why, "delay"))
        .await;
    let event = match inhibitor {
        Ok(fd) => Event::SleepDelay(fd.into()),
        Err(err) => Event::Failed("take logind's sleep delay", err),
    };
    let _ = events.send(event);
}
