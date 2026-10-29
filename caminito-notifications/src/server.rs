//! The Desktop Notifications spec's server (1.3), at `/org/freedesktop/Notifications`, owning
//! `org.freedesktop.Notifications` (ADR 0051). It decides which bubbles are on screen, at most
//! [`MAX_SHOWN`] with the rest queued, how long each stays, and which sound plays, and tells
//! the apps what became of their notifications. The binary draws the bubbles and plays the
//! sounds, behind [`Bubbles`] and [`Sounds`]. It follows the notification settings (CMN-261)
//! as it goes, the app's own or else the default (CMN-262): the Delivery and the sound as a
//! notification arrives, what a bubble shows as it opens, and the default timeout. An app
//! joins the known apps with its first notification.
//!
//! Every notification that isn't transient also enters the Notification history (CMN-259),
//! which outlasts its bubble and the daemon, and `api` serves it to the applets. While Do Not
//! Disturb is on (CMN-263), a notification gets its entry but no bubble and no sound, unless
//! it breaks through: its app's own setting says so, or it's critical and the setting for
//! those does. Nothing is shown when it ends. The time the history ages by and Do Not Disturb
//! ends at comes from a [`Clock`].
//!
//! The name is asked for with `REPLACE`: a daemon that D-Bus activation started while this
//! one wasn't running, as mate-notification-daemon, gives it back at once if it allows its
//! replacement (CMN-258). This one doesn't allow its own, so no other daemon takes the name
//! from it. Refused, it waits in the bus's queue for the name.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use gio::glib::{self, Variant, VariantDict};
use gio::prelude::*;

use crate::DOMAIN;
use crate::do_not_disturb::DoNotDisturb;
use crate::history::{History, HistoryEntry, MAX_AGE};
use crate::notification::{Arguments, DEFAULT_ACTION, Notification};
use crate::private_api::{INTERFACE_NAME as PRIVATE_INTERFACE, PATH as PRIVATE_PATH};
use crate::settings::{Delivery, NO_PREVIEW, Settings};

mod api;

/// The name the server owns.
pub const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

/// How many bubbles are on screen at once. The rest wait, oldest first.
pub const MAX_SHOWN: usize = 3;

/// What the server tells of itself: its name, its vendor, its version and the spec's.
const SERVER_NAME: &str = "Caminito";
const VENDOR: &str = "MATE";
const SPEC_VERSION: &str = "1.3";

/// The spec's optional parts the server has (`GetCapabilities`). The inline reply comes with
/// its ticket (CMN-266).
const CAPABILITIES: [&str; 7] = [
    "actions",
    "body",
    "body-hyperlinks",
    "body-markup",
    "icon-static",
    "persistence",
    "sound",
];

const INTERFACE: &str = r#"
<node>
  <interface name="org.freedesktop.Notifications">
    <method name="GetCapabilities">
      <arg type="as" name="capabilities" direction="out"/>
    </method>
    <method name="Notify">
      <arg type="s" name="app_name" direction="in"/>
      <arg type="u" name="replaces_id" direction="in"/>
      <arg type="s" name="app_icon" direction="in"/>
      <arg type="s" name="summary" direction="in"/>
      <arg type="s" name="body" direction="in"/>
      <arg type="as" name="actions" direction="in"/>
      <arg type="a{sv}" name="hints" direction="in"/>
      <arg type="i" name="expire_timeout" direction="in"/>
      <arg type="u" name="id" direction="out"/>
    </method>
    <method name="CloseNotification">
      <arg type="u" name="id" direction="in"/>
    </method>
    <method name="GetServerInformation">
      <arg type="s" name="name" direction="out"/>
      <arg type="s" name="vendor" direction="out"/>
      <arg type="s" name="version" direction="out"/>
      <arg type="s" name="spec_version" direction="out"/>
    </method>
    <signal name="NotificationClosed">
      <arg type="u" name="id"/>
      <arg type="u" name="reason"/>
    </signal>
    <signal name="ActionInvoked">
      <arg type="u" name="id"/>
      <arg type="s" name="action_key"/>
    </signal>
    <signal name="ActivationToken">
      <arg type="u" name="id"/>
      <arg type="s" name="activation_token"/>
    </signal>
    <signal name="NotificationReplied">
      <arg type="u" name="id"/>
      <arg type="s" name="text"/>
    </signal>
  </interface>
</node>
"#;

/// Why a notification was closed, as `NotificationClosed` gives it.
#[derive(Debug, Clone, Copy)]
enum Reason {
    Expired = 1,
    Dismissed = 2,
    Closed = 3,
}

/// What a bubble shows.
#[derive(Debug, Clone, PartialEq)]
pub struct Bubble {
    /// The app's name, from `app_name` or else its desktop entry.
    pub app_name: String,
    /// The app's icon, from `app_icon` or else its desktop entry.
    pub app_icon: Option<Icon>,
    /// The summary, as plain text.
    pub summary: String,
    /// The body as Pango markup, with what the spec allows and nothing else.
    pub body: String,
    /// The image beside the text.
    pub image: Option<Image>,
    /// The actions that have buttons, in the app's order. The default action is a click.
    pub actions: Vec<Action>,
}

/// An icon, by its icon theme's name or as a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Icon {
    /// An icon theme's icon.
    Name(String),
    /// An image file.
    File(PathBuf),
}

/// A bubble's image.
#[derive(Debug, Clone, PartialEq)]
pub enum Image {
    /// From `image-path`.
    Icon(Icon),
    /// From `image-data`.
    Pixels(Pixels),
}

/// The pixels of an `image-data` hint, as GdkPixbuf has them.
#[derive(Debug, Clone, PartialEq)]
pub struct Pixels {
    /// The width, in pixels.
    pub width: i32,
    /// The height, in pixels.
    pub height: i32,
    /// The bytes from one row's start to the next's.
    pub rowstride: i32,
    /// Whether each pixel has an alpha byte after its red, green and blue.
    pub has_alpha: bool,
    /// The bits of each sample, which are 8.
    pub bits_per_sample: i32,
    /// The samples of each pixel: 3, or 4 with alpha.
    pub channels: i32,
    /// The rows, top first.
    pub data: glib::Bytes,
}

impl Pixels {
    /// Whether they're 8-bit RGB or RGBA, as the spec has them, with all their rows there.
    pub(crate) fn is_drawable(&self) -> bool {
        let channels = if self.has_alpha { 4 } else { 3 };
        self.bits_per_sample == 8
            && self.channels == channels
            && self.width > 0
            && self.height > 0
            && self.rowstride >= self.width * channels
            && usize::try_from(self.rowstride * (self.height - 1) + self.width * channels)
                .is_ok_and(|length| self.data.len() >= length)
    }
}

/// An action with a button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    /// What `ActionInvoked` gives the app.
    pub key: String,
    /// The button's text.
    pub label: String,
}

/// A sound that plays as a notification arrives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sound {
    /// A sound theme's sound, by its name in the sound naming spec.
    Name(String),
    /// A sound file.
    File(PathBuf),
}

/// What draws the bubbles. The server decides when each opens, changes and closes, and the
/// bubbles tell it what the user does through the [`Events`] each is shown with.
pub trait Bubbles {
    /// Shows the bubble of notification `id`, newest nearest the corner.
    fn show(&self, id: u32, bubble: &Bubble, events: Events);
    /// Shows `bubble` in place of what the bubble of `id` showed, where it is.
    fn update(&self, id: u32, bubble: &Bubble);
    /// Takes the bubble of `id` away.
    fn close(&self, id: u32);
}

/// What plays the sounds.
pub trait Sounds {
    /// Plays `sound`, without waiting for it.
    fn play(&self, sound: &Sound);
}

/// What the user does to one bubble, for the server to act on. Once the notification is
/// closed, they do nothing.
#[derive(Clone)]
pub struct Events {
    state: Weak<RefCell<State>>,
    id: u32,
}

impl Events {
    /// The pointer came onto the bubble, or left it. The bubble's timeout waits while it's
    /// there.
    pub fn hovered(&self, hovered: bool) {
        if let Some(state) = self.state.upgrade() {
            hover(&state, self.id, hovered);
        }
    }

    /// The user clicked the bubble: its default action, or else a dismissal. `token` is an
    /// xdg-activation token, for the app to bring its window forward with. Without a default
    /// action the click is to bring the app forward through Caminito (CMN-266); until then
    /// it only dismisses the bubble.
    pub fn click(&self, token: Option<String>) {
        let Some(state) = self.state.upgrade() else {
            return;
        };
        let has_default = state
            .borrow()
            .notifications
            .get(&self.id)
            .is_some_and(|entry| entry.notification.has_default);
        if has_default {
            invoke(&state, self.id, DEFAULT_ACTION, token);
        } else {
            close(&state, self.id, Reason::Dismissed);
        }
    }

    /// The user used the action `key`, with an xdg-activation token for the app.
    pub fn invoke(&self, key: &str, token: Option<String>) {
        if let Some(state) = self.state.upgrade() {
            invoke(&state, self.id, key, token);
        }
    }

    /// The user closed the bubble.
    pub fn dismiss(&self) {
        if let Some(state) = self.state.upgrade() {
            close(&state, self.id, Reason::Dismissed);
        }
    }
}

/// What tells the time the history ages by and Do Not Disturb ends at.
pub trait Clock {
    /// The time now, in seconds since the Unix epoch.
    fn now(&self) -> u64;

    /// The zone "tomorrow morning" is in.
    fn zone(&self) -> glib::TimeZone {
        glib::TimeZone::local()
    }
}

/// The system's clock.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_secs())
    }
}

/// What the Notification history and Do Not Disturb are kept with.
pub struct Config {
    /// The file the Notification history is kept in, under `$XDG_STATE_HOME/caminito/`.
    pub history_file: PathBuf,
    /// The file Do Not Disturb is kept in, beside the history's.
    pub do_not_disturb_file: PathBuf,
    /// The time the history's limits and Do Not Disturb's end are kept by.
    pub clock: Rc<dyn Clock>,
}

/// The history's and Do Not Disturb's calls of the private API, which [`crate::private_api`]
/// routes to the server.
#[derive(Clone)]
pub struct HistoryCalls(Weak<RefCell<State>>);

impl HistoryCalls {
    /// Answers `invocation`, a call of `method` of the private API that isn't the settings' or
    /// the Notification center's.
    pub fn handle(
        &self,
        method: &str,
        parameters: &Variant,
        invocation: gio::DBusMethodInvocation,
    ) {
        api::handle(&self.0, method, parameters, invocation);
    }
}

/// A failure to start the server.
#[derive(Debug)]
pub enum Error {
    /// The object can't be exported on the bus.
    Export(glib::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Export(err) => write!(f, "can't export the notification server: {err}"),
        }
    }
}

impl std::error::Error for Error {}

/// The server, on the bus until it's dropped.
pub struct Server {
    connection: gio::DBusConnection,
    registration: Option<gio::RegistrationId>,
    owner: Option<gio::OwnerId>,
    state: Rc<RefCell<State>>,
}

struct State {
    connection: gio::DBusConnection,
    bubbles: Rc<dyn Bubbles>,
    sounds: Rc<dyn Sounds>,
    settings: Settings,
    clock: Rc<dyn Clock>,
    history: History,
    /// Waits for the end of the oldest entry's week.
    expiry: Option<glib::JoinHandle<()>>,
    do_not_disturb: DoNotDisturb,
    do_not_disturb_file: PathBuf,
    /// Waits for Do Not Disturb's end.
    do_not_disturb_end: Option<glib::JoinHandle<()>>,
    last_id: u32,
    /// The notifications that have a bubble, shown or queued.
    notifications: HashMap<u32, Entry>,
    /// The bubbles on screen, oldest first.
    shown: Vec<u32>,
    /// The notifications waiting for a bubble, oldest first.
    queued: VecDeque<u32>,
    /// What the timers call back, which the state can't hold of itself.
    this: Weak<RefCell<State>>,
}

struct Entry {
    notification: Notification,
    /// What's left of its timeout, while its timer doesn't run.
    remaining: Option<Duration>,
    /// Its timer, while it runs, and when it ends.
    timer: Option<(glib::JoinHandle<()>, Instant)>,
    hovered: bool,
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take() {
            gio::bus_unown_name(owner);
        }
        if let Some(registration) = self.registration.take() {
            let _ = self.connection.unregister_object(registration);
        }
        let mut state = self.state.borrow_mut();
        if let Some(expiry) = state.expiry.take() {
            expiry.abort();
        }
        if let Some(end) = state.do_not_disturb_end.take() {
            end.abort();
        }
        for entry in state.notifications.values_mut() {
            entry.stop_timer();
        }
    }
}

impl Server {
    /// The history's and Do Not Disturb's part of the private API, for
    /// [`crate::private_api::PrivateApi`].
    pub fn history_calls(&self) -> HistoryCalls {
        HistoryCalls(Rc::downgrade(&self.state))
    }

    /// Exports the server on `connection` and asks for the notification name. It follows
    /// `settings`, and keeps the history and Do Not Disturb where `config` says.
    pub fn new(
        connection: &gio::DBusConnection,
        bubbles: Rc<dyn Bubbles>,
        sounds: Rc<dyn Sounds>,
        settings: Settings,
        config: Config,
    ) -> Result<Server, Error> {
        let now = config.clock.now();
        let history = History::load(config.history_file, now);
        let do_not_disturb = DoNotDisturb::load(&config.do_not_disturb_file, now);
        let state = Rc::new_cyclic(|this| {
            RefCell::new(State {
                connection: connection.clone(),
                bubbles,
                sounds,
                settings,
                clock: config.clock,
                // The ids the history holds stay taken, since an app may still hold one.
                last_id: history.max_id(),
                history,
                expiry: None,
                do_not_disturb,
                do_not_disturb_file: config.do_not_disturb_file,
                do_not_disturb_end: None,
                notifications: HashMap::new(),
                shown: Vec::new(),
                queued: VecDeque::new(),
                this: this.clone(),
            })
        });

        let node = gio::DBusNodeInfo::for_xml(INTERFACE).expect("the server's interface is valid");
        let interface = node
            .lookup_interface(NAME)
            .expect("the node has the server's interface");
        let registration = connection
            .register_object(PATH, &interface)
            .method_call({
                let state = Rc::downgrade(&state);
                move |_, _, _, _, method, parameters, invocation| {
                    handle(&state, method, &parameters, invocation);
                }
            })
            .build()
            .map_err(Error::Export)?;
        state.borrow_mut().schedule_expiry();
        state.borrow_mut().schedule_do_not_disturb_end();

        let owner = gio::bus_own_name_on_connection(
            connection,
            NAME,
            gio::BusNameOwnerFlags::REPLACE,
            |_, _| {},
            |_, name| {
                glib::g_warning!(
                    DOMAIN,
                    "another daemon has {name}, so this one waits for it"
                );
            },
        );

        Ok(Server {
            connection: connection.clone(),
            registration: Some(registration),
            owner: Some(owner),
            state,
        })
    }
}

/// Answers an app's call. gio has checked its arguments against [`INTERFACE`].
fn handle(
    state: &Weak<RefCell<State>>,
    method: &str,
    parameters: &Variant,
    invocation: gio::DBusMethodInvocation,
) {
    let Some(state) = state.upgrade() else {
        invocation.return_dbus_error("org.freedesktop.DBus.Error.Failed", "the server stopped");
        return;
    };
    match method {
        "GetCapabilities" => {
            invocation.return_value(Some(&(CAPABILITIES.to_vec(),).to_variant()));
        }
        "GetServerInformation" => invocation.return_value(Some(
            &(SERVER_NAME, VENDOR, env!("CARGO_PKG_VERSION"), SPEC_VERSION).to_variant(),
        )),
        "Notify" => {
            let string = |index| parameters.child_value(index).str().unwrap_or("").to_owned();
            let replaces = parameters.child_value(1).get::<u32>().unwrap_or(0);
            let (app_name, app_icon, summary, body) = (string(0), string(2), string(3), string(4));
            let arguments = Arguments {
                app_name: &app_name,
                app_icon: &app_icon,
                summary: &summary,
                body: &body,
                actions: parameters.child_value(5).get().unwrap_or_default(),
                hints: VariantDict::new(Some(&parameters.child_value(6))),
                expire_timeout: parameters.child_value(7).get().unwrap_or(-1),
            };
            let id = notify(&state, replaces, arguments);
            invocation.return_value(Some(&(id,).to_variant()));
        }
        "CloseNotification" => {
            let id = parameters.child_value(0).get::<u32>().unwrap_or(0);
            close(&state, id, Reason::Closed);
            invocation.return_value(None);
        }
        _ => invocation.return_dbus_error(
            "org.freedesktop.DBus.Error.UnknownMethod",
            &format!("there's no method {method}"),
        ),
    }
}

/// Takes a notification, in place of `replaces` if that's one the server has, and returns
/// its id. Its app's Delivery decides what it gets: with Off, nothing but an id that names no
/// notification, and with History only, its entry. Otherwise a new one plays its sound, unless
/// its app's sound is off, and gets a bubble if there's room, or waits for one.
fn notify(state: &Rc<RefCell<State>>, replaces: u32, arguments: Arguments<'_>) -> u32 {
    let mut guard = state.borrow_mut();
    let now = guard.clock.now();
    let notification = Notification::new(arguments, guard.settings.timeout());
    let timeout = notification.timeout;
    guard.settings.remember(&notification.app);
    let app = guard.settings.effective(&notification.app);
    if app.delivery == Delivery::Off {
        return guard.next_id();
    }

    let live = guard.notifications.contains_key(&replaces);
    // One whose bubble is gone is replaced too, when its app still holds its id.
    let replaced = live || guard.history.get(replaces).is_some();
    let id = if replaced { replaces } else { guard.next_id() };
    guard.remember(id, &notification, now);

    // A bubble on screen is replaced whatever the Delivery is now.
    if live {
        let bubble = guard.shown_bubble(&notification);
        let entry = guard
            .notifications
            .get_mut(&id)
            .expect("the server has the notification it replaces");
        entry.notification = notification;
        // A replacement starts its time again.
        entry.stop_timer();
        entry.remaining = timeout;
        let shown = guard.shown.contains(&id);
        if shown {
            guard.start_timer(id);
        }
        let bubbles = guard.bubbles.clone();
        drop(guard);
        if shown {
            bubbles.update(id, &bubble);
        }
        return id;
    }

    if app.delivery == Delivery::History {
        return id;
    }
    guard.settle_do_not_disturb();
    if guard.do_not_disturb.is_on() && !guard.breaks_through(&notification) {
        return id;
    }
    // A replacement isn't a new notification, so it has no sound.
    let sound = notification
        .sound
        .clone()
        .filter(|_| !replaced && app.sound);
    guard.notifications.insert(
        id,
        Entry {
            notification,
            remaining: timeout,
            timer: None,
            hovered: false,
        },
    );
    let sounds = guard.sounds.clone();
    if guard.shown.len() < MAX_SHOWN {
        drop(guard);
        show(state, id);
    } else {
        guard.queued.push_back(id);
        drop(guard);
    }
    if let Some(sound) = sound {
        sounds.play(&sound);
    }
    id
}

/// Opens the bubble of `id` and starts its time.
fn show(state: &Rc<RefCell<State>>, id: u32) {
    let mut guard = state.borrow_mut();
    let Some(entry) = guard.notifications.get(&id) else {
        return;
    };
    let bubble = guard.shown_bubble(&entry.notification);
    guard.shown.push(id);
    guard.start_timer(id);
    let bubbles = guard.bubbles.clone();
    drop(guard);
    let events = Events {
        state: Rc::downgrade(state),
        id,
    };
    bubbles.show(id, &bubble, events);
}

/// Closes the notification `id` for `reason`: see [`close_all`].
fn close(state: &Rc<RefCell<State>>, id: u32, reason: Reason) {
    close_all(state, &[id], reason);
}

/// Closes the notifications `ids` that the server has, for `reason`: their bubbles go and the
/// oldest that wait take their places, and their apps hear why. A bubble that times out leaves
/// its notification in the history, and any other way of closing takes it out too.
fn close_all(state: &Rc<RefCell<State>>, ids: &[u32], reason: Reason) {
    let mut guard = state.borrow_mut();
    let in_history = if matches!(reason, Reason::Expired) {
        Vec::new()
    } else {
        guard.change_history(|history| history.remove_all(ids))
    };
    let mut gone = Vec::new();
    for &id in ids {
        let live = guard.notifications.remove(&id);
        if live.is_some() || in_history.contains(&id) {
            gone.push(id);
        }
        if let Some(mut entry) = live {
            entry.stop_timer();
        }
    }
    let mut was_shown = Vec::new();
    for &id in &gone {
        guard.queued.retain(|queued| *queued != id);
        if guard.shown.contains(&id) {
            guard.shown.retain(|shown| *shown != id);
            was_shown.push(id);
        }
    }
    let mut next = Vec::new();
    while guard.shown.len() + next.len() < MAX_SHOWN {
        let Some(id) = guard.queued.pop_front() else {
            break;
        };
        next.push(id);
    }
    let bubbles = guard.bubbles.clone();
    let connection = guard.connection.clone();
    drop(guard);

    for id in was_shown {
        bubbles.close(id);
    }
    for id in next {
        show(state, id);
    }
    for id in gone {
        emit(&connection, "NotificationClosed", (id, reason as u32));
    }
}

/// Tells the app the user used the action `key` of `id`, with an xdg-activation token if there
/// is one, and closes the notification unless it's resident.
fn invoke(state: &Rc<RefCell<State>>, id: u32, key: &str, token: Option<String>) {
    act(state, id, |connection| {
        // Before the action, as the spec has it, so the app has it as it acts.
        if let Some(token) = token {
            emit(connection, "ActivationToken", (id, token));
        }
        emit(connection, "ActionInvoked", (id, key));
    });
}

/// Tells the app the user's `text` in reply to `id`, and closes the notification unless it's
/// resident.
fn reply(state: &Rc<RefCell<State>>, id: u32, text: &str) {
    act(state, id, |connection| {
        emit(connection, "NotificationReplied", (id, text));
    });
}

/// What the user does to a notification that the app hears of: `tell` it, then close the
/// notification unless it's resident. It may have only its history entry left.
fn act(state: &Rc<RefCell<State>>, id: u32, tell: impl FnOnce(&gio::DBusConnection)) {
    let guard = state.borrow();
    let resident = guard
        .notifications
        .get(&id)
        .map(|entry| entry.notification.resident)
        .or_else(|| guard.history.get(id).map(|entry| entry.resident));
    let Some(resident) = resident else {
        return;
    };
    let connection = guard.connection.clone();
    drop(guard);
    tell(&connection);
    if !resident {
        close(state, id, Reason::Dismissed);
    }
}

/// Pauses the bubble's time while the pointer is on it, and goes on with what's left once
/// it leaves.
fn hover(state: &Rc<RefCell<State>>, id: u32, hovered: bool) {
    let mut guard = state.borrow_mut();
    let Some(entry) = guard.notifications.get_mut(&id) else {
        return;
    };
    if entry.hovered == hovered {
        return;
    }
    entry.hovered = hovered;
    if hovered {
        if let Some(end) = entry.stop_timer() {
            entry.remaining = Some(end.saturating_duration_since(Instant::now()));
        }
    } else {
        guard.start_timer(id);
    }
}

fn emit(connection: &gio::DBusConnection, signal: &str, arguments: impl ToVariant) {
    emit_on(connection, PATH, NAME, signal, arguments);
}

fn emit_on(
    connection: &gio::DBusConnection,
    path: &str,
    interface: &str,
    signal: &str,
    arguments: impl ToVariant,
) {
    if let Err(err) =
        connection.emit_signal(None, path, interface, signal, Some(&arguments.to_variant()))
    {
        glib::g_warning!(DOMAIN, "can't emit {signal}: {err}");
    }
}

impl Entry {
    /// Stops its timer, if it runs, and returns when it would have ended.
    fn stop_timer(&mut self) -> Option<Instant> {
        let (timer, end) = self.timer.take()?;
        timer.abort();
        Some(end)
    }
}

impl State {
    /// What the bubble of `notification` shows: without its summary, body and image while
    /// its app's Previews are off, and without its app's icon while that's off.
    fn shown_bubble(&self, notification: &Notification) -> Bubble {
        let app = self.settings.effective(&notification.app);
        let mut shown = notification.bubble.clone();
        if !app.previews {
            NO_PREVIEW.clone_into(&mut shown.summary);
            shown.body.clear();
            shown.image = None;
        }
        if !app.app_icon {
            shown.app_icon = None;
        }
        shown
    }

    /// Puts `notification` in the history as `id`, in place of the entry it has, unless it's
    /// transient, which takes it out.
    fn remember(&mut self, id: u32, notification: &Notification, now: u64) {
        self.change_history(|history| {
            if notification.transient {
                history.remove(id);
            } else {
                history.insert(HistoryEntry::new(id, notification, now), now);
            }
        });
    }

    /// Runs `change` on the history. If it changed, the file is written, the clients hear of
    /// it, and the end of the oldest entry's week is waited for again.
    fn change_history<R>(&mut self, change: impl FnOnce(&mut History) -> R) -> R {
        let generation = self.history.generation();
        let counts = self.history.counts();
        let result = change(&mut self.history);
        if self.history.generation() != generation {
            self.history.save();
            emit_on(
                &self.connection,
                PRIVATE_PATH,
                PRIVATE_INTERFACE,
                "HistoryChanged",
                (),
            );
            let counts_now = self.history.counts();
            if counts_now != counts {
                emit_on(
                    &self.connection,
                    PRIVATE_PATH,
                    PRIVATE_INTERFACE,
                    "CountsChanged",
                    (counts_now,),
                );
            }
            self.schedule_expiry();
        }
        result
    }

    /// Drops the entries a week has passed over.
    fn prune_history(&mut self) {
        let now = self.clock.now();
        self.change_history(|history| history.prune(now));
    }

    /// Waits for the end of the oldest entry's week, at least a second away, and then prunes.
    /// It checks the clock again then, so that a clock that jumped is followed.
    fn schedule_expiry(&mut self) {
        if let Some(expiry) = self.expiry.take() {
            expiry.abort();
        }
        let Some(ends) = self.history.expires_at() else {
            return;
        };
        let wait = Duration::from_secs(ends.saturating_sub(self.clock.now()).clamp(1, MAX_AGE));
        let this = self.this.clone();
        self.expiry = Some(
            glib::MainContext::ref_thread_default().spawn_local(async move {
                glib::timeout_future(wait).await;
                if let Some(state) = this.upgrade() {
                    let mut guard = state.borrow_mut();
                    guard.prune_history();
                    // Nothing may have gone yet, if the clock is behind.
                    guard.schedule_expiry();
                }
            }),
        );
    }

    /// Turns Do Not Disturb off if its end has come, whenever its timer would have run.
    fn settle_do_not_disturb(&mut self) {
        if self.do_not_disturb.ended(self.clock.now()) {
            self.set_do_not_disturb(DoNotDisturb::Off);
        }
    }

    /// Whether `notification` gets its bubble and its sound while Do Not Disturb is on: its
    /// app's own setting says so, or it's critical and the setting for those does.
    fn breaks_through(&self, notification: &Notification) -> bool {
        self.settings.effective(&notification.app).breaks_through
            || (notification.critical && self.settings.critical_breaks_through())
    }

    /// Sets Do Not Disturb to `to`. If that's a change, the file is written, the clients hear
    /// of it, and its end is waited for again. Coming on, it takes the waiting notifications
    /// that don't break through out of the queue, as if they had come while it was on: they
    /// keep their entries.
    fn set_do_not_disturb(&mut self, to: DoNotDisturb) {
        if to == self.do_not_disturb {
            return;
        }
        if to.is_on() {
            let quieted: Vec<u32> = self
                .queued
                .iter()
                .copied()
                .filter(|id| {
                    self.notifications
                        .get(id)
                        .is_some_and(|entry| !self.breaks_through(&entry.notification))
                })
                .collect();
            self.queued.retain(|id| !quieted.contains(id));
            for id in quieted {
                self.notifications.remove(&id);
            }
        }
        self.do_not_disturb = to;
        to.save(&self.do_not_disturb_file);
        emit_on(
            &self.connection,
            PRIVATE_PATH,
            PRIVATE_INTERFACE,
            "DoNotDisturbChanged",
            to.to_wire(),
        );
        self.schedule_do_not_disturb_end();
    }

    /// Waits for Do Not Disturb's end, at least a second and at most a day away, and then
    /// checks the clock again, so that a clock that jumped is followed.
    fn schedule_do_not_disturb_end(&mut self) {
        if let Some(end) = self.do_not_disturb_end.take() {
            end.abort();
        }
        let DoNotDisturb::On { end: Some(end) } = self.do_not_disturb else {
            return;
        };
        let wait = end.saturating_sub(self.clock.now()).clamp(1, 24 * 60 * 60);
        let this = self.this.clone();
        self.do_not_disturb_end = Some(glib::MainContext::ref_thread_default().spawn_local(
            async move {
                glib::timeout_future(Duration::from_secs(wait)).await;
                if let Some(state) = this.upgrade() {
                    let mut guard = state.borrow_mut();
                    // Its own handle, which a change would abort as it runs.
                    guard.do_not_disturb_end = None;
                    guard.settle_do_not_disturb();
                    if guard.do_not_disturb_end.is_none() {
                        guard.schedule_do_not_disturb_end();
                    }
                }
            },
        ));
    }

    /// The next id, which is never 0: that's `replaces_id`'s "none".
    fn next_id(&mut self) -> u32 {
        loop {
            self.last_id = self.last_id.wrapping_add(1);
            if self.last_id != 0
                && !self.notifications.contains_key(&self.last_id)
                && self.history.get(self.last_id).is_none()
            {
                return self.last_id;
            }
        }
    }

    /// Runs what's left of the timeout of `id`, unless the pointer is on its bubble or it
    /// has none.
    fn start_timer(&mut self, id: u32) {
        let this = self.this.clone();
        let Some(entry) = self.notifications.get_mut(&id) else {
            return;
        };
        let Some(remaining) = entry
            .remaining
            .filter(|_| !entry.hovered && entry.timer.is_none())
        else {
            return;
        };
        // On the thread's default context, as the bus's calls are, which a test runs a
        // context of its own for.
        let timer = glib::MainContext::ref_thread_default().spawn_local(async move {
            glib::timeout_future(remaining).await;
            if let Some(state) = this.upgrade() {
                close(&state, id, Reason::Expired);
            }
        });
        entry.timer = Some((timer, Instant::now() + remaining));
        entry.remaining = None;
    }
}
