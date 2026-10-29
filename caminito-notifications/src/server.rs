//! The Desktop Notifications spec's server (1.3), at `/org/freedesktop/Notifications`, owning
//! `org.freedesktop.Notifications` (ADR 0051). It decides which bubbles are on screen, at most
//! [`MAX_SHOWN`] with the rest queued, how long each stays, and which sound plays, and tells
//! the apps what became of their notifications. The binary draws the bubbles and plays the
//! sounds, behind [`Bubbles`] and [`Sounds`]. It follows the notification settings (CMN-261)
//! as it goes: what a bubble shows as it opens, the sound as a notification arrives, and the
//! default timeout.
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
use crate::notification::{Arguments, DEFAULT_ACTION, Notification};
use crate::settings;

/// The name the server owns.
pub const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

/// How many bubbles are on screen at once. The rest wait, oldest first.
pub const MAX_SHOWN: usize = 3;

/// What the server tells of itself: its name, its vendor, its version and the spec's.
const SERVER_NAME: &str = "Caminito";
const VENDOR: &str = "MATE";
const SPEC_VERSION: &str = "1.3";

/// The spec's optional parts the server has (`GetCapabilities`). The Notification history's
/// `persistence` and the inline reply come with their tickets (CMN-259, CMN-266).
const CAPABILITIES: [&str; 6] = [
    "actions",
    "body",
    "body-hyperlinks",
    "body-markup",
    "icon-static",
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
    settings: gio::Settings,
    last_id: u32,
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
        for entry in state.notifications.values_mut() {
            entry.stop_timer();
        }
    }
}

impl Server {
    /// Exports the server on `connection` and asks for the notification name. It follows
    /// `settings`, which are `org.mate.Caminito.notifications`'s.
    pub fn new(
        connection: &gio::DBusConnection,
        bubbles: Rc<dyn Bubbles>,
        sounds: Rc<dyn Sounds>,
        settings: gio::Settings,
    ) -> Result<Server, Error> {
        let state = Rc::new_cyclic(|this| {
            RefCell::new(State {
                connection: connection.clone(),
                bubbles,
                sounds,
                settings,
                last_id: 0,
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
/// its id. A new one plays its sound, unless sounds are off, and gets a bubble if there's
/// room, or waits for one.
fn notify(state: &Rc<RefCell<State>>, replaces: u32, arguments: Arguments<'_>) -> u32 {
    let mut guard = state.borrow_mut();
    let notification = Notification::new(arguments, settings::timeout(&guard.settings));
    let timeout = notification.timeout;

    if guard.notifications.contains_key(&replaces) {
        let bubble = guard.shown_bubble(&notification.bubble);
        let entry = guard
            .notifications
            .get_mut(&replaces)
            .expect("the server has the notification it replaces");
        entry.notification = notification;
        // A replacement starts its time again.
        entry.stop_timer();
        entry.remaining = timeout;
        let shown = guard.shown.contains(&replaces);
        if shown {
            guard.start_timer(replaces);
        }
        let bubbles = guard.bubbles.clone();
        drop(guard);
        if shown {
            bubbles.update(replaces, &bubble);
        }
        return replaces;
    }

    let id = guard.next_id();
    let sound = notification.sound.clone();
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
    let sound = sound.filter(|_| guard.settings.boolean(settings::SOUND));
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
    let bubble = guard.shown_bubble(&entry.notification.bubble);
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

/// Closes the notification `id`, if the server has it, for `reason`: its bubble goes and the
/// oldest that waits takes its place, and its app hears why.
fn close(state: &Rc<RefCell<State>>, id: u32, reason: Reason) {
    let mut guard = state.borrow_mut();
    let Some(mut entry) = guard.notifications.remove(&id) else {
        return;
    };
    entry.stop_timer();
    guard.queued.retain(|queued| *queued != id);
    let was_shown = guard.shown.contains(&id);
    guard.shown.retain(|shown| *shown != id);
    let next = if was_shown {
        guard.queued.pop_front()
    } else {
        None
    };
    let bubbles = guard.bubbles.clone();
    let connection = guard.connection.clone();
    drop(guard);

    if was_shown {
        bubbles.close(id);
    }
    if let Some(next) = next {
        show(state, next);
    }
    emit(&connection, "NotificationClosed", (id, reason as u32));
}

/// Tells the app its action `key` was used, and closes the notification unless it's
/// resident.
fn invoke(state: &Rc<RefCell<State>>, id: u32, key: &str, token: Option<String>) {
    let guard = state.borrow();
    let Some(entry) = guard.notifications.get(&id) else {
        return;
    };
    let resident = entry.notification.resident;
    let connection = guard.connection.clone();
    drop(guard);
    // Before the action, as the spec has it, so the app has it as it acts.
    if let Some(token) = token {
        emit(&connection, "ActivationToken", (id, token));
    }
    emit(&connection, "ActionInvoked", (id, key));
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
    if let Err(err) =
        connection.emit_signal(None, PATH, NAME, signal, Some(&arguments.to_variant()))
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
    /// What the bubble of a notification that has `bubble` shows: without its summary, body
    /// and image while Previews are off, and without its app's icon while that's off.
    fn shown_bubble(&self, bubble: &Bubble) -> Bubble {
        let mut shown = bubble.clone();
        if !self.settings.boolean(settings::PREVIEWS) {
            settings::NO_PREVIEW.clone_into(&mut shown.summary);
            shown.body.clear();
            shown.image = None;
        }
        if !self.settings.boolean(settings::APP_ICON) {
            shown.app_icon = None;
        }
        shown
    }

    /// The next id, which is never 0: that's `replaces_id`'s "none".
    fn next_id(&mut self) -> u32 {
        loop {
            self.last_id = self.last_id.wrapping_add(1);
            if self.last_id != 0 && !self.notifications.contains_key(&self.last_id) {
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
