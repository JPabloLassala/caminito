//! The Keyboard monitor for Orca (13, 135): `org.freedesktop.a11y.KeyboardMonitor` at
//! `/org/freedesktop/a11y/Manager`, under the name `org.freedesktop.a11y.Manager`, with
//! Mutter's interface (`data/dbus-interfaces/org.freedesktop.a11y.xml`). at-spi2-core 2.55
//! and later looks for it on Wayland, and without it Orca hears only the keys typed into
//! accessible apps.
//!
//! Its callers watch the keys, grab all of them, or grab some: modifiers, which take the
//! keys pressed while they're down too, and keystrokes. A key a caller grabbed is told to
//! that caller and goes no further. Only the owner of Orca's name may call it, as in
//! Mutter (`meta-a11y-manager.c`), and what a caller asked for is dropped when it leaves the
//! bus. The decision about each key is [`KeyboardMonitor::key`], which the keyboard path's
//! stage 3 calls, and the `KeyEvent`s are sent from zbus's thread in the order they're
//! decided, so the key path never waits on the bus (15).
//!
//! The rules on keys are niri's (`src/dbus/freedesktop_a11y.rs`, `1f03391`), with the held
//! keys known by their codes rather than their keysyms.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use futures_channel::{mpsc, oneshot};
use smithay::input::keyboard::Keycode;
use smithay::reexports::calloop::channel::Sender;
use zbus::fdo::{self, RequestNameFlags, RequestNameReply};
use zbus::message::Header;
use zbus::names::BusName;
use zbus::object_server::SignalEmitter;

use super::{Callers, caller};
use crate::state::Caminito;

/// The name at-spi2-core looks for the monitor at.
pub(crate) const NAME: &str = "org.freedesktop.a11y.Manager";
/// Where the monitor is served.
pub(super) const PATH: &str = "/org/freedesktop/a11y/Manager";
/// The name at-spi2-core asks for on Orca's behalf (`atspi-device-a11y-manager.c:552`),
/// whose owner alone may call the monitor (13).
pub(crate) const ORCA_NAME: &str = "org.gnome.Orca.KeyboardMonitor";

/// A call, for the event loop to answer. Each one has been allowed.
pub(super) enum Request {
    /// `WatchKeyboard` or `UnwatchKeyboard`.
    Watch {
        caller: String,
        watch: bool,
        reply: oneshot::Sender<()>,
    },
    /// `GrabKeyboard` or `UngrabKeyboard`.
    Grab {
        caller: String,
        grab: bool,
        reply: oneshot::Sender<()>,
    },
    /// `SetKeyGrabs`.
    SetKeyGrabs {
        caller: String,
        modifiers: Vec<u32>,
        keystrokes: Vec<(u32, u32)>,
        reply: oneshot::Sender<()>,
    },
}

/// A key event as the monitor sees it: past AccessX and before xkbcommon has it, so its
/// keysym and modifiers are those of the state before it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Key {
    pub keycode: Keycode,
    pub pressed: bool,
    /// The event's time, in milliseconds.
    pub time: u32,
    /// The modifiers in effect, as xkb serializes them.
    pub state: u32,
    pub keysym: u32,
    /// The character the key types, or 0.
    pub unichar: u32,
}

/// What the monitor makes of a key.
#[derive(Debug, Default)]
pub(crate) struct Verdict {
    /// The callers to signal it to, by their unique names.
    pub told: Vec<String>,
    /// Whether it goes on to the bindings and the client.
    pub passes: bool,
}

/// One caller's watch and grabs.
#[derive(Debug, Default)]
struct Caller {
    watches: bool,
    grabs_the_keyboard: bool,
    /// Keysyms.
    modifiers: BTreeSet<u32>,
    /// Keysyms, each with the exact modifier mask it's grabbed with.
    keystrokes: BTreeSet<(u32, u32)>,
}

impl Caller {
    fn wants_nothing(&self) -> bool {
        !self.watches
            && !self.grabs_the_keyboard
            && self.modifiers.is_empty()
            && self.keystrokes.is_empty()
    }

    /// Whether `key` is this caller's, while the monitor holds back the keys whose keysyms
    /// are `held`.
    fn grabs(&self, key: &Key, held: &[u32]) -> bool {
        self.grabs_the_keyboard
            || self.modifiers.contains(&key.keysym)
            || self.keystrokes.contains(&(key.keysym, key.state))
            || held.iter().any(|keysym| self.modifiers.contains(keysym))
    }
}

/// A key whose press went to the monitor alone, so its release does too.
#[derive(Debug)]
struct Held {
    /// Its keysym at the press.
    keysym: u32,
    /// The callers that grabbed it, who are told of its release.
    grabbers: Vec<String>,
}

/// The monitor's callers, and the keys it holds back from the rest of the key path.
#[derive(Debug, Default)]
pub(crate) struct KeyboardMonitor {
    callers: BTreeMap<String, Caller>,
    held: HashMap<Keycode, Held>,
    /// When each grabbed modifier was last pressed, in the key events' milliseconds.
    modifier_presses: HashMap<u32, u32>,
}

impl KeyboardMonitor {
    /// Decides where `key` goes. A grabbed modifier pressed alone is the monitor's, unless
    /// it's pressed again within `repeat_delay` milliseconds, when the second press goes on
    /// as itself, so that Orca's modifier still works as its key (13).
    pub(crate) fn key(&mut self, key: &Key, repeat_delay: u32) -> Verdict {
        if !key.pressed {
            return self.release(key);
        }
        let held: Vec<u32> = self.held.values().map(|held| held.keysym).collect();
        let grabbers = self.callers_that(|caller| caller.grabs(key, &held));
        let pressed_again = self.pressed_again(key, repeat_delay);
        let told = self.told(grabbers.iter().cloned());
        // A key down already, as on two keyboards at once, is the monitor's until let go.
        let already_held = self.held.contains_key(&key.keycode);
        let passes = !already_held && (grabbers.is_empty() || pressed_again);
        if !passes && !already_held {
            let keysym = key.keysym;
            self.held.insert(key.keycode, Held { keysym, grabbers });
        }
        Verdict { told, passes }
    }

    /// Whether `key` is a grabbed modifier pressed again within `repeat_delay` of its last
    /// press, which it becomes.
    fn pressed_again(&mut self, key: &Key, repeat_delay: u32) -> bool {
        let grabbed = self
            .callers
            .values()
            .any(|caller| caller.modifiers.contains(&key.keysym));
        grabbed
            && self
                .modifier_presses
                .insert(key.keysym, key.time)
                .is_some_and(|last| key.time.wrapping_sub(last) <= repeat_delay)
    }

    /// The release of a key held back goes where its press went, to those of its grabbers
    /// still on the bus, and any other release goes on.
    fn release(&mut self, key: &Key) -> Verdict {
        match self.held.remove(&key.keycode) {
            Some(held) => {
                let grabbers = held.grabbers.into_iter();
                let grabbers = grabbers.filter(|grabber| self.callers.contains_key(grabber));
                Verdict {
                    told: self.told(grabbers.collect::<Vec<_>>()),
                    passes: false,
                }
            }
            None => Verdict {
                told: self.told(self.callers_that(|caller| caller.grabs(key, &[]))),
                passes: true,
            },
        }
    }

    /// The callers for which `test` holds.
    fn callers_that(&self, test: impl Fn(&Caller) -> bool) -> Vec<String> {
        let callers = self.callers.iter().filter(|(_, caller)| test(caller));
        callers.map(|(name, _)| name.clone()).collect()
    }

    /// The callers that watch, and `grabbers`, each once.
    fn told(&self, grabbers: impl IntoIterator<Item = String>) -> Vec<String> {
        let mut told = self.callers_that(|caller| caller.watches);
        for grabber in grabbers {
            if !told.contains(&grabber) {
                told.push(grabber);
            }
        }
        told
    }

    /// Forgets the keys held back, as when every key is let go at once.
    pub(crate) fn clear(&mut self) {
        self.held.clear();
    }

    /// Changes `caller`'s entry, and drops it once it asks for nothing.
    fn change(&mut self, caller: String, change: impl FnOnce(&mut Caller)) {
        let entry = self.callers.entry(caller.clone()).or_default();
        change(entry);
        if entry.wants_nothing() {
            self.callers.remove(&caller);
        }
        self.forget_modifiers_no_one_grabs();
    }

    /// Drops what a caller that left the bus asked for.
    pub(super) fn caller_left(&mut self, caller: &str) {
        self.callers.remove(caller);
        self.forget_modifiers_no_one_grabs();
    }

    fn forget_modifiers_no_one_grabs(&mut self) {
        let callers = &self.callers;
        self.modifier_presses.retain(|keysym, _| {
            callers
                .values()
                .any(|caller| caller.modifiers.contains(keysym))
        });
    }
}

impl Caminito {
    pub(super) fn a11y_request(&mut self, request: Request) {
        // A caller that has gone away doesn't want the answer.
        match request {
            Request::Watch {
                caller,
                watch,
                reply,
            } => {
                self.keyboard_monitor
                    .change(caller, |caller| caller.watches = watch);
                let _ = reply.send(());
            }
            Request::Grab {
                caller,
                grab,
                reply,
            } => {
                self.keyboard_monitor
                    .change(caller, |caller| caller.grabs_the_keyboard = grab);
                let _ = reply.send(());
            }
            Request::SetKeyGrabs {
                caller,
                modifiers,
                keystrokes,
                reply,
            } => {
                self.keyboard_monitor.change(caller, |caller| {
                    caller.modifiers = modifiers.into_iter().collect();
                    caller.keystrokes = keystrokes.into_iter().collect();
                });
                let _ = reply.send(());
            }
        }
    }

    /// Signals `key` to the callers `told`, in order with the keys before it, from zbus's
    /// thread.
    pub(crate) fn signal_key_event(&self, key: &Key, told: Vec<String>) {
        if let Some(dbus) = &self.dbus {
            for caller in told {
                // The sending task stops only with the connection.
                let _ = dbus.key_events.unbounded_send((caller, *key));
            }
        }
    }
}

/// Sends each `KeyEvent` that comes on `events` to its caller, in the order they come.
pub(super) async fn signal_key_events(
    connection: zbus::Connection,
    mut events: mpsc::UnboundedReceiver<(String, Key)>,
) {
    while let Ok((caller, key)) = events.recv().await {
        let signalled = async {
            let emitter = SignalEmitter::new(&connection, PATH)?
                .set_destination(BusName::try_from(caller.as_str())?);
            // A keycode is 8 to 255 in X's range, and xkb's codes go past it only for keys
            // no X client sees either.
            let keycode = u16::try_from(key.keycode.raw()).unwrap_or(0);
            KeyboardMonitorInterface::key_event(
                &emitter,
                !key.pressed,
                key.state,
                key.keysym,
                key.unichar,
                keycode,
            )
            .await
        };
        if let Err(err) = signalled.await {
            tracing::debug!("can't signal a key event to {caller}: {err}");
        }
    }
}

/// `org.freedesktop.a11y.KeyboardMonitor`.
pub(super) struct KeyboardMonitorInterface {
    requests: Sender<Request>,
    callers: Callers,
}

impl KeyboardMonitorInterface {
    pub(super) fn new(requests: Sender<Request>, callers: Callers) -> Self {
        Self { requests, callers }
    }

    /// Asks the event loop to answer `header`'s sender's call, if that's Orca, and has the
    /// caller watched for leaving the bus.
    async fn ask(
        &self,
        header: &Header<'_>,
        connection: &zbus::Connection,
        request: impl FnOnce(String, oneshot::Sender<()>) -> Request,
    ) -> fdo::Result<()> {
        let caller = caller(header);
        if !is_orca(connection, &caller).await {
            return Err(fdo::Error::AccessDenied(format!(
                "only the owner of {ORCA_NAME} may use the keyboard monitor"
            )));
        }
        let answer = super::ask(&self.requests, |reply| request(caller.clone(), reply)).await;
        self.callers.report_if_gone(connection, caller).await;
        answer
    }
}

/// Whether `caller` owns Orca's name.
async fn is_orca(connection: &zbus::Connection, caller: &str) -> bool {
    let owner = async {
        let bus = fdo::DBusProxy::new(connection).await?;
        zbus::Result::Ok(
            bus.get_name_owner(BusName::from_static_str(ORCA_NAME)?)
                .await?,
        )
    };
    owner.await.is_ok_and(|owner| owner.as_str() == caller)
}

#[zbus::interface(name = "org.freedesktop.a11y.KeyboardMonitor")]
impl KeyboardMonitorInterface {
    /// Every key is the caller's until `UngrabKeyboard` or it leaves the bus.
    async fn grab_keyboard(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> fdo::Result<()> {
        self.ask(&header, connection, |caller, reply| Request::Grab {
            caller,
            grab: true,
            reply,
        })
        .await
    }

    async fn ungrab_keyboard(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> fdo::Result<()> {
        self.ask(&header, connection, |caller, reply| Request::Grab {
            caller,
            grab: false,
            reply,
        })
        .await
    }

    /// The caller is told of every key, which goes on as before.
    async fn watch_keyboard(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> fdo::Result<()> {
        self.ask(&header, connection, |caller, reply| Request::Watch {
            caller,
            watch: true,
            reply,
        })
        .await
    }

    async fn unwatch_keyboard(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> fdo::Result<()> {
        self.ask(&header, connection, |caller, reply| Request::Watch {
            caller,
            watch: false,
            reply,
        })
        .await
    }

    /// Replaces the caller's grabs: `modifiers` are keysyms, and each keystroke is a keysym
    /// and the modifier mask it's grabbed with.
    async fn set_key_grabs(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
        modifiers: Vec<u32>,
        keystrokes: Vec<(u32, u32)>,
    ) -> fdo::Result<()> {
        self.ask(&header, connection, |caller, reply| Request::SetKeyGrabs {
            caller,
            modifiers,
            keystrokes,
            reply,
        })
        .await
    }

    /// A key the caller watches or grabbed.
    #[zbus(signal)]
    async fn key_event(
        emitter: &SignalEmitter<'_>,
        released: bool,
        state: u32,
        keysym: u32,
        unichar: u32,
        keycode: u16,
    ) -> zbus::Result<()>;
}

/// Asks the bus for the monitor's name, without replacing an owner, such as the host's
/// Mutter in a nested run. Caminito carries on without it.
pub(super) async fn own_name(connection: &zbus::Connection) {
    let reply = connection
        .request_name_with_flags(NAME, RequestNameFlags::DoNotQueue.into())
        .await;
    match reply {
        Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => {}
        Ok(RequestNameReply::Exists | RequestNameReply::InQueue) | Err(zbus::Error::NameTaken) => {
            let owner = super::screensaver::owner(connection, NAME).await;
            tracing::warn!("can't own {NAME}: {owner} has it");
        }
        Err(err) => tracing::warn!("can't own {NAME}: {err}"),
    }
}
