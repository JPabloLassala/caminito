//! The Desktop Notifications spec's server (1.3), at `/org/freedesktop/Notifications`, owning
//! `org.freedesktop.Notifications` (ADR 0051). It decides which bubbles are on screen, as many
//! as fit (CMN-285), how long each stays, and which sound plays, and tells the apps what became
//! of their notifications. A notification that doesn't fit gets no bubble, now or later, and
//! counts in the bubbles' [`Header`] until the last of them closes. The binary draws the
//! bubbles and plays the sounds, behind [`Bubbles`] and [`Sounds`]. It follows the
//! notification settings (CMN-261) as it goes, the app's own or else the default (CMN-262):
//! the Delivery and the sound as a notification arrives, what a bubble shows as it opens, and
//! the default timeout. An app joins the known apps with its first notification.
//!
//! With its app's grouping on (CMN-265), an app's third notification on screen merges its
//! first two bubbles into one grouped bubble, which its later ones join, critical ones only
//! with critical ones.
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
use std::collections::HashMap;
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
mod ungrouped;

/// The name the server owns.
pub const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

/// Caminito's name and interface, and its object, which brings an app forward.
const CAMINITO: &str = "org.mate.Caminito";
const CAMINITO_PATH: &str = "/org/mate/Caminito";

/// What the server tells of itself: its name, its vendor, its version and the spec's.
const SERVER_NAME: &str = "Caminito";
const VENDOR: &str = "MATE";
const SPEC_VERSION: &str = "1.3";

/// The spec's optional parts the server has (`GetCapabilities`), with KDE's inline reply
/// (CMN-266).
const CAPABILITIES: [&str; 8] = [
    "actions",
    "body",
    "body-hyperlinks",
    "body-markup",
    "icon-static",
    "inline-reply",
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
    /// The reply field's placeholder, if it has KDE's inline reply.
    pub reply: Option<String>,
    /// The progress bar's percentage, from the `value` hint. An app moves it by replacing the
    /// notification. The Notification center has no bar, so the history doesn't keep it.
    pub progress: Option<u8>,
    /// In a grouped bubble, the notifications before this one, newest first. Empty in a
    /// bubble of one notification.
    pub earlier: Vec<Earlier>,
}

/// What a grouped bubble shows of a notification before its newest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Earlier {
    /// The summary, as plain text.
    pub summary: String,
    /// The body as Pango markup, as [`Bubble::body`] has it.
    pub body: String,
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

/// What the header above the bubbles shows while there's a bubble: how many notifications got
/// none since the first of them opened, and whether a critical one is among those.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Header {
    /// The notifications in "*N* more notifications".
    pub more: usize,
    /// Whether one of them is critical.
    pub critical: bool,
}

/// What draws the bubbles. The server decides when each opens, changes and closes, and the
/// bubbles tell it what the user does through the [`Events`] each is shown with. A bubble's
/// `id` is its notification's, and a grouped bubble's is that of the notification that made
/// it, which it keeps while that one is gone and others stay.
pub trait Bubbles {
    /// Shows the bubble `id`, newest nearest the corner, if it fits between the panel and the
    /// output's far edge with those on screen and their header, and returns whether it did.
    /// The first one always fits.
    fn show(&self, id: u32, bubble: &Bubble, events: Events) -> bool;
    /// Shows `header` above the bubbles, and the user's clicks on it go to `events`.
    fn header(&self, header: Header, events: HeaderEvents);
    /// Shows `bubble` in place of what the bubble `id` showed, where it is.
    fn update(&self, id: u32, bubble: &Bubble);
    /// Takes the bubble `id` away.
    fn close(&self, id: u32);
}

/// What plays the sounds.
pub trait Sounds {
    /// Plays `sound`, without waiting for it.
    fn play(&self, sound: &Sound);
}

/// What the user does to one bubble, for the server to act on. A click, the reply field and
/// the actions are its newest notification's, and closing it closes all of its notifications.
/// Once the bubble is gone, they do nothing.
#[derive(Clone)]
pub struct Events {
    state: Weak<RefCell<State>>,
    /// The bubble's key in `shown`.
    key: u32,
}

impl Events {
    /// The pointer came onto the bubble, or left it. The bubble's timeout waits while it's
    /// there.
    pub fn hovered(&self, hovered: bool) {
        if let Some(state) = self.state.upgrade() {
            hover(&state, self.key, hovered);
        }
    }

    /// The user clicked the bubble: its default action, with `token`, an xdg-activation token
    /// for the app to bring its window forward with, or else Caminito brings the app forward
    /// (CMN-266).
    pub fn click(&self, token: Option<String>) {
        if let Some((state, newest)) = self.newest() {
            click(&state, newest, token);
        }
    }

    /// The user sent `text` from the bubble's reply field.
    pub fn reply(&self, text: &str) {
        if let Some((state, newest)) = self.newest() {
            reply(&state, newest, text);
        }
    }

    /// The user used the action `key`, with an xdg-activation token for the app.
    pub fn invoke(&self, key: &str, token: Option<String>) {
        if let Some((state, newest)) = self.newest() {
            invoke(&state, newest, key, token);
        }
    }

    /// The user closed the bubble.
    pub fn dismiss(&self) {
        if let Some(state) = self.state.upgrade() {
            close_bubble(&state, self.key, Reason::Dismissed);
        }
    }

    /// The bubble's newest notification, while the bubble is there.
    fn newest(&self) -> Option<(Rc<RefCell<State>>, u32)> {
        let state = self.state.upgrade()?;
        let guard = state.borrow();
        let newest = *guard.shown[guard.bubble(self.key)?].ids.last()?;
        drop(guard);
        Some((state, newest))
    }
}

/// What the user does to the header above the bubbles.
#[derive(Clone)]
pub struct HeaderEvents {
    state: Weak<RefCell<State>>,
}

impl HeaderEvents {
    /// "Clear all": the notifications of the bubbles on screen leave the history, as the
    /// private API's `Dismiss` takes them out. Those that got no bubble stay.
    pub fn clear_all(&self) {
        if let Some(state) = self.state.upgrade() {
            let shown = state.borrow().shown_ids();
            close_all(&state, &shown, Reason::Dismissed);
        }
    }

    /// "*N* more notifications": asks the Notifications applet for the Notification center.
    pub fn open_center(&self) {
        if let Some(state) = self.state.upgrade() {
            open_center(&state);
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
    /// The notifications that have a bubble.
    notifications: HashMap<u32, Notification>,
    /// The bubbles on screen, oldest first.
    shown: Vec<Shown>,
    /// The notifications that got no bubble since the first on screen opened, and whether
    /// each is critical, which the header counts.
    more: Vec<(u32, bool)>,
    /// The header the bubbles were last given, if any.
    header: Option<Header>,
    /// While the Notification center is open, what ends the watch on the client that said
    /// so, which closes it if the client leaves the bus. gio doesn't export the watch's type.
    center: Option<Box<dyn FnOnce()>>,
    /// The Notifying apps with grouping off, for the Notification center.
    ungrouped: ungrouped::Ungrouped,
    /// What the timers call back, which the state can't hold of itself.
    this: Weak<RefCell<State>>,
}

/// A bubble on screen: one notification's, or a grouped bubble of one app's.
struct Shown {
    /// What [`Bubbles`] knows it by.
    key: u32,
    /// Its notifications, oldest first, which are never none.
    ids: Vec<u32>,
    /// The app of its notifications, and whether they're critical.
    app: String,
    critical: bool,
    /// Whether it's a grouped bubble, which its app's next notifications join.
    grouped: bool,
    /// What's left of its timeout, while its timer doesn't run.
    remaining: Option<Duration>,
    /// Its timer, while it runs, and when it ends.
    timer: Option<(glib::JoinHandle<()>, Instant)>,
    hovered: bool,
}

/// Where a notification's bubble goes.
enum Place {
    /// Into the grouped bubble at this index of `shown`.
    Join(usize),
    /// Into a grouped bubble that takes the place of these bubbles of its app.
    Merge(Vec<usize>),
    /// Into a bubble of its own.
    Alone,
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
        if let Some(unwatch) = state.center.take() {
            unwatch();
        }
        if let Some(end) = state.do_not_disturb_end.take() {
            end.abort();
        }
        ungrouped::unfollow(&mut state);
        for shown in &mut state.shown {
            shown.stop_timer();
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
                more: Vec::new(),
                header: None,
                center: None,
                ungrouped: ungrouped::Ungrouped::default(),
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
        ungrouped::follow(&state);
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
/// its app's sound is off, and gets a bubble if it fits, or else counts in the header. While
/// the Notification center is open it gets none and shows only there.
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
        guard.notifications.insert(id, notification);
        let Some(index) = guard.shown.iter().position(|shown| shown.ids.contains(&id)) else {
            return id;
        };
        // A replacement starts its bubble's time again.
        guard.restart_timer(index, timeout);
        let (key, bubble) = (guard.shown[index].key, guard.render(index));
        let bubbles = guard.bubbles.clone();
        drop(guard);
        bubbles.update(key, &bubble);
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
    let critical = notification.critical;
    // A replacement of a counted one is counted again only if it doesn't fit either.
    guard.more.retain(|&(more, _)| more != id);
    guard.notifications.insert(id, notification);
    let sounds = guard.sounds.clone();
    let center = guard.center.is_some();
    let place = guard.place_of(id);
    drop(guard);
    if center {
        without_bubble(state, id, false);
    } else {
        match place {
            Place::Alone => {
                if !show_making_room(state, id, critical) {
                    without_bubble(state, id, true);
                }
            }
            Place::Join(index) => join(state, id, index),
            Place::Merge(indices) => merge(state, id, indices),
        }
    }
    sync_header(state);
    if let Some(sound) = sound {
        sounds.play(&sound);
    }
    id
}

/// Opens the bubble of `id` alone, if it fits, starts its time, and returns whether it
/// opened.
fn show(state: &Rc<RefCell<State>>, id: u32) -> bool {
    let guard = state.borrow();
    let Some(notification) = guard.notifications.get(&id) else {
        return false;
    };
    let shown = Shown::new(
        id,
        vec![id],
        notification.app.clone(),
        notification.critical,
        false,
    );
    let timeout = notification.timeout;
    drop(guard);
    open(state, shown, timeout)
}

/// Opens the bubble `shown`, if it fits, and starts its time from `timeout`.
fn open(state: &Rc<RefCell<State>>, shown: Shown, timeout: Option<Duration>) -> bool {
    let mut guard = state.borrow_mut();
    let key = shown.key;
    guard.shown.push(shown);
    let index = guard.shown.len() - 1;
    let bubble = guard.render(index);
    let bubbles = guard.bubbles.clone();
    drop(guard);
    let events = Events {
        state: Rc::downgrade(state),
        key,
    };
    let opened = bubbles.show(key, &bubble, events);
    let mut guard = state.borrow_mut();
    if opened {
        guard.restart_timer(index, timeout);
    } else {
        guard.shown.pop();
    }
    opened
}

/// Puts notification `id` into the grouped bubble at `index` of `shown`, and starts the
/// bubble's time again.
fn join(state: &Rc<RefCell<State>>, id: u32, index: usize) {
    let mut guard = state.borrow_mut();
    let timeout = guard
        .notifications
        .get(&id)
        .and_then(|notification| notification.timeout);
    guard.shown[index].ids.push(id);
    guard.restart_timer(index, timeout);
    let (key, bubble) = (guard.shown[index].key, guard.render(index));
    let bubbles = guard.bubbles.clone();
    drop(guard);
    bubbles.update(key, &bubble);
}

/// Merges the bubbles at `indices` of `shown` and notification `id` into one grouped bubble
/// in their place. If it doesn't fit, their notifications are left to the history, which the
/// header counts.
fn merge(state: &Rc<RefCell<State>>, id: u32, indices: Vec<usize>) {
    let mut guard = state.borrow_mut();
    let Some(notification) = guard.notifications.get(&id) else {
        return;
    };
    let (app, critical, timeout) = (
        notification.app.clone(),
        notification.critical,
        notification.timeout,
    );
    let mut ids = Vec::new();
    let mut merged = Vec::new();
    let mut hovered = false;
    // From the newest, so that each index is still right.
    for index in indices.into_iter().rev() {
        let mut shown = guard.shown.remove(index);
        shown.stop_timer();
        merged.push(shown.key);
        hovered |= shown.hovered;
        ids.splice(0..0, shown.ids);
    }
    ids.push(id);
    let mut grouped = Shown::new(id, ids.clone(), app, critical, true);
    // The pointer on a bubble merged is on the grouped bubble in its place.
    grouped.hovered = hovered;
    let bubbles = guard.bubbles.clone();
    drop(guard);
    for key in merged {
        bubbles.close(key);
    }
    if !open(state, grouped, timeout) {
        for id in ids {
            without_bubble(state, id, true);
        }
    }
}

/// Opens the bubble of `id` as [`show`] does. One that doesn't fit, if it's critical, takes
/// the place of the oldest ordinary bubbles until it fits, whose notifications the header
/// counts.
fn show_making_room(state: &Rc<RefCell<State>>, id: u32, critical: bool) -> bool {
    loop {
        if show(state, id) {
            return true;
        }
        if !critical {
            return false;
        }
        let guard = state.borrow();
        let oldest = guard
            .shown
            .iter()
            .find(|shown| !shown.critical)
            .map(|shown| shown.key);
        let Some(oldest) = oldest else {
            return false;
        };
        drop(guard);
        for id in put_away(state, &[oldest]) {
            without_bubble(state, id, true);
        }
    }
}

/// Takes the bubbles `keys` away without closing their notifications, and returns those.
fn put_away(state: &Rc<RefCell<State>>, keys: &[u32]) -> Vec<u32> {
    let mut guard = state.borrow_mut();
    let mut ids = Vec::new();
    for key in keys {
        if let Some(index) = guard.bubble(*key) {
            let mut shown = guard.shown.remove(index);
            shown.stop_timer();
            ids.extend(shown.ids);
        }
    }
    let bubbles = guard.bubbles.clone();
    drop(guard);
    for &key in keys {
        bubbles.close(key);
    }
    ids
}

/// Leaves the notification `id`, whose bubble didn't open or was put away, to the history,
/// where the header counts it if `counted`. A transient one isn't there, so it's closed as
/// expired.
fn without_bubble(state: &Rc<RefCell<State>>, id: u32, counted: bool) {
    let mut guard = state.borrow_mut();
    let Some(notification) = guard.notifications.remove(&id) else {
        return;
    };
    if notification.transient {
        guard.notifications.insert(id, notification);
        drop(guard);
        close(state, id, Reason::Expired);
    } else if counted {
        guard.more.push((id, notification.critical));
    }
}

/// Asks the Notifications applet for the Notification center.
fn open_center(state: &Rc<RefCell<State>>) {
    let connection = state.borrow().connection.clone();
    emit_on(
        &connection,
        PRIVATE_PATH,
        PRIVATE_INTERFACE,
        "CenterRequested",
        (),
    );
}

/// The Notification center opened, as the client `sender` says, or closed. The bubbles on
/// screen are put away as it opens, and none open while it's open.
fn set_center_open(state: &Rc<RefCell<State>>, open: bool, sender: Option<&str>) {
    let mut guard = state.borrow_mut();
    if let Some(unwatch) = guard.center.take() {
        unwatch();
    }
    let Some(sender) = sender.filter(|_| open) else {
        return;
    };
    let watch = gio::bus_watch_name_on_connection(
        &guard.connection,
        sender,
        gio::BusNameWatcherFlags::NONE,
        |_, _, _| {},
        {
            let state = Rc::downgrade(state);
            move |_, _| {
                if let Some(state) = state.upgrade() {
                    set_center_open(&state, false, None);
                }
            }
        },
    );
    guard.center = Some(Box::new(move || gio::bus_unwatch_name(watch)));
    guard.more.clear();
    let keys: Vec<u32> = guard.shown.iter().map(|shown| shown.key).collect();
    drop(guard);
    for id in put_away(state, &keys) {
        without_bubble(state, id, false);
    }
    sync_header(state);
}

/// Gives the bubbles the header, the first time and whenever it changes.
fn sync_header(state: &Rc<RefCell<State>>) {
    let mut guard = state.borrow_mut();
    let header = Header {
        more: guard.more.len(),
        critical: guard.more.iter().any(|&(_, critical)| critical),
    };
    if guard.header == Some(header) {
        return;
    }
    guard.header = Some(header);
    let bubbles = guard.bubbles.clone();
    drop(guard);
    let events = HeaderEvents {
        state: Rc::downgrade(state),
    };
    bubbles.header(header, events);
}

/// Closes the notification `id` for `reason`: see [`close_all`].
fn close(state: &Rc<RefCell<State>>, id: u32, reason: Reason) {
    close_all(state, &[id], reason);
}

/// Closes all the notifications of the bubble `key` for `reason`, if it's there.
fn close_bubble(state: &Rc<RefCell<State>>, key: u32, reason: Reason) {
    let ids = {
        let guard = state.borrow();
        guard
            .bubble(key)
            .map(|index| guard.shown[index].ids.clone())
    };
    if let Some(ids) = ids {
        close_all(state, &ids, reason);
    }
}

/// Closes the notifications `ids` that the server has, for `reason`: they leave their bubbles,
/// a bubble left with none goes, and their apps hear why. A bubble that times out leaves its
/// notifications in the history, and any other way of closing takes them out too. The
/// header's count empties with the last bubble.
fn close_all(state: &Rc<RefCell<State>>, ids: &[u32], reason: Reason) {
    let mut guard = state.borrow_mut();
    let in_history = if matches!(reason, Reason::Expired) {
        Vec::new()
    } else {
        guard.change_history(|history| history.remove_all(ids))
    };
    let mut gone = Vec::new();
    for &id in ids {
        let live = guard.notifications.remove(&id).is_some();
        if live || in_history.contains(&id) {
            gone.push(id);
        }
    }
    guard.more.retain(|&(more, _)| !gone.contains(&more));
    let mut closed = Vec::new();
    let mut changed = Vec::new();
    guard.shown.retain_mut(|shown| {
        let before = shown.ids.len();
        shown.ids.retain(|id| !gone.contains(id));
        if shown.ids.is_empty() {
            shown.stop_timer();
            closed.push(shown.key);
            return false;
        }
        if shown.ids.len() != before {
            changed.push(shown.key);
        }
        true
    });
    if guard.shown.is_empty() {
        guard.more.clear();
    }
    let updates: Vec<(u32, Bubble)> = changed
        .into_iter()
        .filter_map(|key| Some((key, guard.render(guard.bubble(key)?))))
        .collect();
    let bubbles = guard.bubbles.clone();
    let connection = guard.connection.clone();
    drop(guard);

    for key in closed {
        bubbles.close(key);
    }
    for (key, bubble) in updates {
        bubbles.update(key, &bubble);
    }
    sync_header(state);
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

/// A click on the notification `id`, in its bubble or in the Notification center: its default
/// action, with an xdg-activation token for the app to bring its window forward with, or else
/// Caminito brings the app forward, as a click on its Task item does. Either way it closes
/// unless it's resident.
fn click(state: &Rc<RefCell<State>>, id: u32, token: Option<String>) {
    let guard = state.borrow();
    let clicked = match guard.notifications.get(&id) {
        Some(notification) => Some((notification.has_default, notification.app.clone())),
        None => guard
            .history
            .get(id)
            .map(|entry| (entry.has_default, entry.app.clone())),
    };
    drop(guard);
    match clicked {
        Some((true, _)) => invoke(state, id, DEFAULT_ACTION, token),
        Some((false, app)) => act(state, id, |connection| activate_app(connection, &app)),
        None => {}
    }
}

/// Asks Caminito to bring forward the app whose key is `app`, its desktop entry or else its
/// name, with `org.mate.Caminito`'s `ActivateApplication` (ADR 0054). Without Caminito, as
/// outside a Session, nothing comes forward.
fn activate_app(connection: &gio::DBusConnection, app: &str) {
    connection.call(
        Some(CAMINITO),
        CAMINITO_PATH,
        CAMINITO,
        "ActivateApplication",
        Some(&(app,).to_variant()),
        None,
        gio::DBusCallFlags::NO_AUTO_START,
        -1,
        gio::Cancellable::NONE,
        |result| {
            if let Err(err) = result {
                glib::g_debug!(DOMAIN, "can't ask Caminito to bring the app forward: {err}");
            }
        },
    );
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
        .map(|notification| notification.resident)
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

/// Pauses the time of the bubble `key` while the pointer is on it, and goes on with what's
/// left once it leaves.
fn hover(state: &Rc<RefCell<State>>, key: u32, hovered: bool) {
    let mut guard = state.borrow_mut();
    let Some(index) = guard.bubble(key) else {
        return;
    };
    let shown = &mut guard.shown[index];
    if shown.hovered == hovered {
        return;
    }
    shown.hovered = hovered;
    if hovered {
        if let Some(end) = shown.stop_timer() {
            shown.remaining = Some(end.saturating_duration_since(Instant::now()));
        }
    } else {
        guard.start_timer(index);
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

impl Shown {
    fn new(key: u32, ids: Vec<u32>, app: String, critical: bool, grouped: bool) -> Shown {
        Shown {
            key,
            ids,
            app,
            critical,
            grouped,
            remaining: None,
            timer: None,
            hovered: false,
        }
    }

    /// Stops its timer, if it runs, and returns when it would have ended.
    fn stop_timer(&mut self) -> Option<Instant> {
        let (timer, end) = self.timer.take()?;
        timer.abort();
        Some(end)
    }
}

impl State {
    /// The index in `shown` of the bubble `key`.
    fn bubble(&self, key: u32) -> Option<usize> {
        self.shown.iter().position(|shown| shown.key == key)
    }

    /// The notifications of the bubbles on screen.
    fn shown_ids(&self) -> Vec<u32> {
        self.shown
            .iter()
            .flat_map(|shown| shown.ids.iter().copied())
            .collect()
    }

    /// Where the bubble of notification `id` goes. With its app's grouping on, it joins the
    /// app's grouped bubble of its urgency, or makes one of the app's bubbles of its urgency
    /// once there are two; otherwise it has one of its own.
    fn place_of(&self, id: u32) -> Place {
        let Some(notification) = self.notifications.get(&id) else {
            return Place::Alone;
        };
        if !self.settings.effective(&notification.app).group {
            return Place::Alone;
        }
        let same = |shown: &Shown| {
            shown.app == notification.app && shown.critical == notification.critical
        };
        if let Some(index) = self
            .shown
            .iter()
            .position(|shown| shown.grouped && same(shown))
        {
            return Place::Join(index);
        }
        let same_bubbles: Vec<usize> = (0..self.shown.len())
            .filter(|&index| same(&self.shown[index]))
            .collect();
        if same_bubbles.len() >= 2 {
            Place::Merge(same_bubbles)
        } else {
            Place::Alone
        }
    }

    /// What the bubble at `index` of `shown` shows: its newest notification, and in a grouped
    /// bubble the others before it.
    fn render(&self, index: usize) -> Bubble {
        let shown = &self.shown[index];
        let mut ids = shown.ids.iter().rev().map(|id| &self.notifications[id]);
        let newest = ids.next().expect("a bubble has a notification");
        let mut bubble = self.shown_bubble(newest);
        bubble.earlier = ids
            .map(|notification| {
                let earlier = self.shown_bubble(notification);
                Earlier {
                    summary: earlier.summary,
                    body: earlier.body,
                }
            })
            .collect();
        bubble
    }

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
            self.queue_ungrouped();
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
    /// of it, and its end is waited for again.
    fn set_do_not_disturb(&mut self, to: DoNotDisturb) {
        if to == self.do_not_disturb {
            return;
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
                && self.bubble(self.last_id).is_none()
                && self.history.get(self.last_id).is_none()
            {
                return self.last_id;
            }
        }
    }

    /// Starts the time of the bubble at `index` of `shown` again, from `timeout`.
    fn restart_timer(&mut self, index: usize, timeout: Option<Duration>) {
        let shown = &mut self.shown[index];
        shown.stop_timer();
        shown.remaining = timeout;
        self.start_timer(index);
    }

    /// Runs what's left of the timeout of the bubble at `index` of `shown`, unless the pointer
    /// is on it or it has none.
    fn start_timer(&mut self, index: usize) {
        let this = self.this.clone();
        let shown = &mut self.shown[index];
        let Some(remaining) = shown
            .remaining
            .filter(|_| !shown.hovered && shown.timer.is_none())
        else {
            return;
        };
        let key = shown.key;
        // On the thread's default context, as the bus's calls are, which a test runs a
        // context of its own for.
        let timer = glib::MainContext::ref_thread_default().spawn_local(async move {
            glib::timeout_future(remaining).await;
            if let Some(state) = this.upgrade() {
                close_bubble(&state, key, Reason::Expired);
            }
        });
        shown.timer = Some((timer, Instant::now() + remaining));
        shown.remaining = None;
    }
}
