//! The Desktop Notifications server on a private bus (CMN-258): the test calls it as an app
//! would, over its own connection, and reads the signals the app would get. The bubbles and
//! the sounds are fakes, which record what the server asked of them, and act on a bubble as
//! the user would: hovering it, clicking it, using an action or closing it.

mod bus;
mod settings;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use caminito_notifications::private_api::PrivateApi;
use caminito_notifications::server::{
    Action, Bubble, Bubbles, Clock, Config, Events, Header, HeaderEvents, Icon, Image, Server,
    Sound, Sounds, TimeLeft,
};
use caminito_notifications::settings::Settings;
use gio::glib::{self, Variant, VariantDict};
use gio::prelude::*;

use bus::Bus;

const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
const API_NAME: &str = "org.mate.Caminito.Notifications";
const API_PATH: &str = "/org/mate/Caminito/Notifications";

/// How long a test waits to see that no signal comes. A signal crosses the private bus in a
/// millisecond or two, so this is many times what one needs.
const QUIET: Duration = Duration::from_millis(100);

/// Caminito's name and interface, which the daemon asks to bring an app forward, and whose
/// properties turn Do Not Disturb on.
const CAMINITO: &str = "org.mate.Caminito";
const CAMINITO_PATH: &str = "/org/mate/Caminito";
const CAMINITO_INTERFACE: &str = r#"
<node>
  <interface name="org.mate.Caminito">
    <method name="ActivateApplication">
      <arg type="s" name="app" direction="in"/>
    </method>
    <property name="PointerOutputFullscreen" type="b" access="read"/>
    <property name="Screencasting" type="b" access="read"/>
  </interface>
</node>
"#;

/// The seconds in a week, which the history keeps an entry for.
const WEEK: u64 = 7 * 24 * 60 * 60;

/// The spec's reasons a notification was closed.
const EXPIRED: u32 = 1;
const DISMISSED: u32 = 2;
const CLOSED: u32 = 3;

/// The bubbles on screen, oldest first, and what the user can do to each.
#[derive(Default)]
struct FakeBubbles {
    shown: RefCell<Vec<(u32, Bubble)>>,
    events: RefCell<HashMap<u32, Events>>,
    /// How many bubbles fit on the output, or `None` for any number.
    room: Cell<Option<usize>>,
    /// The header, as the server last gave it, and what the user can do to it.
    header: Cell<Header>,
    header_events: RefCell<Option<HeaderEvents>>,
    /// How many times each bubble was shown, updated or closed, in order.
    calls: RefCell<Vec<String>>,
    /// What each bubble was last told of its timeout.
    time_left: RefCell<HashMap<u32, Option<TimeLeft>>>,
}

impl Bubbles for FakeBubbles {
    fn show(&self, id: u32, bubble: &Bubble, events: Events) -> bool {
        if self
            .room
            .get()
            .is_some_and(|room| self.shown.borrow().len() >= room)
        {
            return false;
        }
        self.shown.borrow_mut().push((id, bubble.clone()));
        self.events.borrow_mut().insert(id, events);
        self.calls.borrow_mut().push(format!("show {id}"));
        true
    }

    fn header(&self, header: Header, events: HeaderEvents) {
        self.header.set(header);
        *self.header_events.borrow_mut() = Some(events);
    }

    fn update(&self, id: u32, bubble: &Bubble) {
        for shown in self.shown.borrow_mut().iter_mut() {
            if shown.0 == id {
                shown.1 = bubble.clone();
            }
        }
        self.calls.borrow_mut().push(format!("update {id}"));
    }

    fn close(&self, id: u32) {
        self.shown.borrow_mut().retain(|shown| shown.0 != id);
        self.events.borrow_mut().remove(&id);
        self.calls.borrow_mut().push(format!("close {id}"));
    }

    fn time_left(&self, id: u32, time_left: Option<TimeLeft>) {
        self.time_left.borrow_mut().insert(id, time_left);
    }
}

impl FakeBubbles {
    fn ids(&self) -> Vec<u32> {
        self.shown.borrow().iter().map(|shown| shown.0).collect()
    }

    fn bubble(&self, id: u32) -> Bubble {
        let shown = self.shown.borrow();
        let found = shown.iter().find(|shown| shown.0 == id);
        found.unwrap_or_else(|| panic!("{id} is shown")).1.clone()
    }

    /// What the bubble `id` was last told of its timeout.
    fn told_time_left(&self, id: u32) -> Option<TimeLeft> {
        let told = self.time_left.borrow();
        *told
            .get(&id)
            .unwrap_or_else(|| panic!("{id} was told its time left"))
    }

    /// What the user does to the bubble `id`.
    fn events(&self, id: u32) -> Events {
        self.events.borrow()[&id].clone()
    }

    /// What the user does to the header.
    fn header_events(&self) -> HeaderEvents {
        self.header_events
            .borrow()
            .clone()
            .expect("the server gave the header")
    }
}

#[derive(Default)]
struct FakeSounds(RefCell<Vec<Sound>>);

impl Sounds for FakeSounds {
    fn play(&self, sound: &Sound) {
        self.0.borrow_mut().push(sound.clone());
    }
}

/// A clock the test sets.
struct FakeClock(Cell<u64>);

impl Clock for FakeClock {
    fn now(&self) -> u64 {
        self.0.get()
    }

    /// UTC, so that the test's mornings are the same wherever it runs.
    fn zone(&self) -> glib::TimeZone {
        glib::TimeZone::utc()
    }
}

/// Caminito on the bus, as much of it as the daemon calls: the apps it was asked to bring
/// forward, in order, and its properties, by name, which are false until they're set.
struct FakeCaminito {
    activated: Rc<RefCell<Vec<String>>>,
    properties: Rc<RefCell<HashMap<String, bool>>>,
    connection: gio::DBusConnection,
    registration: Option<gio::RegistrationId>,
}

impl FakeCaminito {
    fn start(bus: &Bus) -> FakeCaminito {
        let connection = bus.connect();
        let node = gio::DBusNodeInfo::for_xml(CAMINITO_INTERFACE).unwrap();
        let interface = node.lookup_interface(CAMINITO).unwrap();
        let activated: Rc<RefCell<Vec<String>>> = Rc::default();
        let properties: Rc<RefCell<HashMap<String, bool>>> = Rc::default();
        let registration = connection
            .register_object(CAMINITO_PATH, &interface)
            .method_call({
                let activated = activated.clone();
                move |_, _, _, _, _, parameters, invocation| {
                    let app = parameters.child_value(0).str().unwrap_or("").to_owned();
                    activated.borrow_mut().push(app);
                    invocation.return_value(None);
                }
            })
            .property({
                let properties = properties.clone();
                move |_, _, _, _, name| {
                    let value = properties.borrow().get(name).copied();
                    value.unwrap_or(false).to_variant()
                }
            })
            .build()
            .expect("the fake registers");
        let reply = connection.call_sync(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "RequestName",
            Some(&(CAMINITO, 0u32).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            -1,
            gio::Cancellable::NONE,
        );
        reply.expect("the fake owns Caminito's name");
        FakeCaminito {
            activated,
            properties,
            connection,
            registration: Some(registration),
        }
    }

    fn activated(&self) -> Vec<String> {
        self.activated.borrow().clone()
    }

    /// Sets the property `name` to `value`, and signals it.
    fn set(&self, name: &str, value: bool) {
        self.properties.borrow_mut().insert(name.to_owned(), value);
        let changed = VariantDict::new(None);
        changed.insert_value(name, &value.to_variant());
        let invalidated: Vec<String> = Vec::new();
        // Built of its parts, as a tuple's would box the dictionary in a variant.
        let arguments = Variant::tuple_from_iter([
            CAMINITO.to_variant(),
            changed.end(),
            invalidated.to_variant(),
        ]);
        self.connection
            .emit_signal(
                None,
                CAMINITO_PATH,
                "org.freedesktop.DBus.Properties",
                "PropertiesChanged",
                Some(&arguments),
            )
            .expect("the fake signals its property");
    }
}

impl Drop for FakeCaminito {
    fn drop(&mut self) {
        if let Some(registration) = self.registration.take() {
            let _ = self.connection.unregister_object(registration);
        }
    }
}

/// What outlives a daemon: the directory of its history, and the time.
struct Home {
    directory: tempfile::TempDir,
    clock: Rc<FakeClock>,
}

impl Home {
    fn new() -> Rc<Home> {
        Rc::new(Home {
            directory: tempfile::tempdir().expect("a temporary directory"),
            clock: Rc::new(FakeClock(Cell::new(1_000_000))),
        })
    }
}

/// The server on the bus, and an app's connection to it.
struct Daemon<'a> {
    bus: &'a Bus,
    home: Rc<Home>,
    _server: Server,
    _private_api: PrivateApi,
    /// The server's settings, as `gsettings` would change them.
    settings: gio::Settings,
    /// The same with the apps' own, as the settings window changes them.
    apps: Settings,
    _schemas: tempfile::TempDir,
    /// The apps whose pages the private API was asked to open the settings window at.
    opened_settings: Rc<RefCell<Vec<Option<String>>>>,
    bubbles: Rc<FakeBubbles>,
    sounds: Rc<FakeSounds>,
    app: gio::DBusConnection,
    /// The server's unique name, which its private API is called at.
    service: String,
    _subscriptions: Vec<gio::SignalSubscription>,
    /// Each signal the app got, by its name, with its arguments.
    signals: Rc<RefCell<Vec<(String, Variant)>>>,
    /// The same for the private API's.
    api_signals: Rc<RefCell<Vec<(String, Variant)>>>,
}

fn start(bus: &Bus) -> Daemon<'_> {
    start_in(bus, Home::new())
}

/// A daemon that keeps its history in `home`, as one after a restart does.
fn start_in(bus: &Bus, home: Rc<Home>) -> Daemon<'_> {
    let schemas = tempfile::tempdir().unwrap();
    let apps = settings::compiled(schemas.path());
    let connection = bus.connect();
    let bubbles = Rc::new(FakeBubbles::default());
    let sounds = Rc::new(FakeSounds::default());
    let server = Server::new(
        &connection,
        bubbles.clone(),
        sounds.clone(),
        settings::outside(&apps),
        Config {
            history_file: home.directory.path().join("caminito").join("history"),
            do_not_disturb_file: home
                .directory
                .path()
                .join("caminito")
                .join("do-not-disturb"),
            clock: home.clock.clone(),
        },
    )
    .expect("the server registers");
    let opened_settings: Rc<RefCell<Vec<Option<String>>>> = Rc::default();
    let private_api = PrivateApi::new(&connection, server.history_calls(), {
        let opened = opened_settings.clone();
        move |app| opened.borrow_mut().push(app.map(str::to_owned))
    })
    .expect("the private API registers");
    bus.wait_for_owner(NAME, &connection);
    let app = bus.connect();
    let service = connection
        .unique_name()
        .expect("a bus connection has a unique name")
        .to_string();
    let subscribe = |name: &str, path: &str| {
        let signals: Rc<RefCell<Vec<(String, Variant)>>> = Rc::default();
        let subscription = app.subscribe_to_signal(
            Some(&service),
            Some(name),
            None,
            Some(path),
            None,
            gio::DBusSignalFlags::NONE,
            {
                let signals = signals.clone();
                move |signal| {
                    signals
                        .borrow_mut()
                        .push((signal.signal_name.to_owned(), signal.parameters.clone()));
                }
            },
        );
        (signals, subscription)
    };
    let (signals, first) = subscribe(NAME, PATH);
    let (api_signals, second) = subscribe(API_NAME, API_PATH);
    Daemon {
        bus,
        home,
        _server: server,
        _private_api: private_api,
        settings: apps.global().clone(),
        apps,
        _schemas: schemas,
        opened_settings,
        bubbles,
        sounds,
        app,
        service,
        _subscriptions: vec![first, second],
        signals,
        api_signals,
    }
}

/// An entry of the history as `List` gives it.
type WireEntry = (
    u32,
    String,
    String,
    String,
    String,
    String,
    Variant,
    Vec<(String, String)>,
    bool,
    bool,
    String,
    u64,
);

#[derive(Debug, PartialEq)]
struct Listed {
    id: u32,
    app: String,
    app_name: String,
    app_icon: String,
    summary: String,
    body: String,
    image: String,
    actions: Vec<(String, String)>,
    has_default: bool,
    resident: bool,
    /// The reply field's placeholder, or empty for none.
    reply: String,
    time: u64,
}

impl From<WireEntry> for Listed {
    fn from(entry: WireEntry) -> Listed {
        Listed {
            id: entry.0,
            app: entry.1,
            app_name: entry.2,
            app_icon: entry.3,
            summary: entry.4,
            body: entry.5,
            // Reading a tuple of Rust types unboxes the variant already.
            image: entry.6.str().unwrap_or_default().to_owned(),
            actions: entry.7,
            has_default: entry.8,
            resident: entry.9,
            reply: entry.10,
            time: entry.11,
        }
    }
}

/// A notification as an app sends one, with nothing but its summary.
struct Notify {
    app_name: &'static str,
    replaces: u32,
    icon: &'static str,
    summary: &'static str,
    body: &'static str,
    actions: Vec<&'static str>,
    hints: VariantDict,
    timeout: i32,
}

fn notify(summary: &'static str) -> Notify {
    Notify {
        app_name: "Test",
        replaces: 0,
        icon: "",
        summary,
        body: "",
        actions: Vec::new(),
        hints: VariantDict::new(None),
        timeout: -1,
    }
}

impl Notify {
    fn hint(self, key: &str, value: impl ToVariant) -> Self {
        self.hints.insert_value(key, &value.to_variant());
        self
    }
}

impl Daemon<'_> {
    /// Sends `notification` and returns its id.
    fn notify(&self, notification: Notify) -> u32 {
        // The hints are an a{sv} of their own, which a tuple's `to_variant` would box.
        let parameters = Variant::tuple_from_iter([
            notification.app_name.to_variant(),
            notification.replaces.to_variant(),
            notification.icon.to_variant(),
            notification.summary.to_variant(),
            notification.body.to_variant(),
            notification.actions.to_variant(),
            notification.hints.end(),
            notification.timeout.to_variant(),
        ]);
        self.call("Notify", parameters)
            .get::<(u32,)>()
            .expect("Notify answers an id")
            .0
    }

    fn close(&self, id: u32) {
        self.call("CloseNotification", (id,).to_variant());
    }

    /// Calls the server's `method`, without blocking the main context, which runs the server
    /// too.
    fn call(&self, method: &str, parameters: Variant) -> Variant {
        self.send(NAME, PATH, NAME, method, parameters)
            .unwrap_or_else(|err| panic!("{method} answers: {err}"))
    }

    /// Calls the private API's `method`.
    fn api(&self, method: &str, parameters: Variant) -> Variant {
        self.try_api(method, parameters)
            .unwrap_or_else(|err| panic!("{method} answers: {err}"))
    }

    /// Calls the private API's `method`, which may refuse.
    fn try_api(&self, method: &str, parameters: Variant) -> Result<Variant, glib::Error> {
        self.send(&self.service, API_PATH, API_NAME, method, parameters)
    }

    fn send(
        &self,
        destination: &str,
        path: &str,
        interface: &str,
        method: &str,
        parameters: Variant,
    ) -> Result<Variant, glib::Error> {
        self.send_on(&self.app, destination, path, interface, method, parameters)
    }

    /// Calls `method` over `connection`, which may be another client's than the app's.
    fn send_on(
        &self,
        connection: &gio::DBusConnection,
        destination: &str,
        path: &str,
        interface: &str,
        method: &str,
        parameters: Variant,
    ) -> Result<Variant, glib::Error> {
        let answer: Rc<RefCell<Option<Result<Variant, glib::Error>>>> = Rc::default();
        connection.call(
            Some(destination),
            path,
            interface,
            method,
            Some(&parameters),
            None,
            gio::DBusCallFlags::NONE,
            -1,
            gio::Cancellable::NONE,
            {
                let answer = answer.clone();
                move |result| *answer.borrow_mut() = Some(result)
            },
        );
        self.bus
            .wait_until(&format!("{method}'s answer"), || answer.borrow().is_some());
        answer.take().expect("it answered")
    }

    /// The history, newest first.
    fn history(&self) -> Vec<Listed> {
        let (entries,) = self
            .api("List", ().to_variant())
            .get::<(Vec<WireEntry>,)>()
            .expect("List answers entries");
        entries.into_iter().map(Listed::from).collect()
    }

    /// The summaries of the history, newest first.
    fn summaries(&self) -> Vec<String> {
        self.history()
            .into_iter()
            .map(|entry| entry.summary)
            .collect()
    }

    /// Each Notifying app's count.
    fn counts(&self) -> Vec<(String, u32)> {
        let (counts,) = self
            .api("Counts", ().to_variant())
            .get::<(HashMap<String, u32>,)>()
            .expect("Counts answers a dictionary");
        let mut counts: Vec<_> = counts.into_iter().collect();
        counts.sort();
        counts
    }

    /// Do Not Disturb as the private API gives it: whether it's on, its end, its reason.
    fn do_not_disturb(&self) -> (bool, u64, String) {
        self.api("DoNotDisturb", ().to_variant())
            .get::<(bool, u64, String)>()
            .expect("DoNotDisturb answers its state")
    }

    /// Turns Do Not Disturb on `until` one of the durations, or `off`.
    fn set_do_not_disturb(&self, until: &str) {
        self.api("SetDoNotDisturb", (until,).to_variant());
    }

    /// Each private API signal so far, as its name and its arguments' text.
    fn api_signals(&self) -> Vec<(String, String)> {
        self.api_signals
            .borrow()
            .iter()
            .map(|(name, parameters)| (name.clone(), parameters.print(false).to_string()))
            .collect()
    }

    fn wait_for_api_signals(&self, count: usize) {
        self.bus.wait_until(&format!("{count} API signals"), || {
            self.api_signals.borrow().len() >= count
        });
    }

    /// Forgets the signals so far. A call's answer comes after the signals the server emitted
    /// before it, so those on their way have arrived once it has.
    fn forget_signals(&self) {
        self.counts();
        self.signals.borrow_mut().clear();
        self.api_signals.borrow_mut().clear();
    }

    /// Each signal so far, as its name and its arguments' text.
    fn signals(&self) -> Vec<(String, String)> {
        self.signals
            .borrow()
            .iter()
            .map(|(name, parameters)| (name.clone(), parameters.print(false).to_string()))
            .collect()
    }

    fn wait_for_signals(&self, count: usize) {
        self.bus.wait_until(&format!("{count} signals"), || {
            self.signals.borrow().len() >= count
        });
    }
}

fn signal(name: &str, arguments: &str) -> (String, String) {
    (name.to_owned(), arguments.to_owned())
}

#[test]
fn the_server_names_itself_and_its_capabilities() {
    bus::run(|bus| {
        let daemon = start(bus);

        let information = daemon.call("GetServerInformation", ().to_variant());
        assert_eq!(
            information
                .get::<(String, String, String, String)>()
                .unwrap(),
            (
                "Caminito".to_owned(),
                "MATE".to_owned(),
                env!("CARGO_PKG_VERSION").to_owned(),
                "1.3".to_owned()
            )
        );
        let capabilities = daemon.call("GetCapabilities", ().to_variant());
        assert_eq!(
            capabilities.get::<(Vec<String>,)>().unwrap().0,
            [
                "actions",
                "body",
                "body-hyperlinks",
                "body-markup",
                "icon-static",
                "inline-reply",
                "persistence",
                "sound"
            ]
        );
    });
}

#[test]
fn the_name_is_taken_from_a_daemon_that_allows_its_replacement() {
    bus::run(|bus| {
        // As mate-notification-daemon owns it once D-Bus activation started it.
        let other = bus.connect();
        let _other = gio::bus_own_name_on_connection(
            &other,
            NAME,
            gio::BusNameOwnerFlags::ALLOW_REPLACEMENT,
            |_, _| {},
            |_, _| {},
        );
        bus.wait_for_owner(NAME, &other);

        // `start` waits until the server's connection owns the name.
        let _daemon = start(bus);
    });
}

#[test]
fn a_notification_gets_a_bubble_with_its_app_text_and_actions() {
    bus::run(|bus| {
        let daemon = start(bus);

        let id = daemon.notify(Notify {
            icon: "mail-unread",
            body: "From <b>Ann</b> & Bob",
            actions: vec!["default", "Open", "reply", "Reply", "later", "Later"],
            ..notify("New mail")
        });

        assert_ne!(id, 0);
        assert_eq!(daemon.bubbles.ids(), [id]);
        assert_eq!(
            daemon.bubbles.bubble(id),
            Bubble {
                app_name: "Test".to_owned(),
                app_icon: Some(Icon::Name("mail-unread".to_owned())),
                summary: "New mail".to_owned(),
                body: "From <b>Ann</b> &amp; Bob".to_owned(),
                image: None,
                actions: vec![
                    Action {
                        key: "reply".to_owned(),
                        label: "Reply".to_owned()
                    },
                    Action {
                        key: "later".to_owned(),
                        label: "Later".to_owned()
                    },
                ],
                reply: None,
                progress: None,
                earlier: Vec::new(),
            }
        );
        let second = daemon.notify(notify("Another"));
        assert_ne!(second, id);
    });
}

#[test]
fn an_image_path_hint_is_the_bubbles_image() {
    bus::run(|bus| {
        let daemon = start(bus);

        let id = daemon.notify(notify("Photo").hint("image-path", "file:///tmp/photo.png"));

        assert_eq!(
            daemon.bubbles.bubble(id).image,
            Some(Image::Icon(Icon::File("/tmp/photo.png".into())))
        );
    });
}

#[test]
fn a_replacement_changes_the_bubble_in_place_and_keeps_its_id() {
    bus::run(|bus| {
        let daemon = start(bus);
        let first = daemon.notify(notify("One"));
        let other = daemon.notify(notify("Other"));

        let replaced = daemon.notify(Notify {
            replaces: first,
            ..notify("Two")
        });

        assert_eq!(replaced, first);
        assert_eq!(daemon.bubbles.ids(), [first, other]);
        assert_eq!(daemon.bubbles.bubble(first).summary, "Two");
        assert_eq!(
            *daemon.bubbles.calls.borrow(),
            [
                format!("show {first}"),
                format!("show {other}"),
                format!("update {first}")
            ]
        );
        // One the server no longer has is a new notification.
        let new = daemon.notify(Notify {
            replaces: 9999,
            ..notify("Three")
        });
        assert!(new != 9999 && new != first && new != other);
    });
}

#[test]
fn the_value_hint_is_a_progress_bar_that_a_replacement_moves_in_place() {
    bus::run(|bus| {
        let daemon = start(bus);
        let id = daemon.notify(notify("Copying").hint("value", 40i32));
        assert_eq!(daemon.bubbles.bubble(id).progress, Some(40));

        let replaced = daemon.notify(Notify {
            replaces: id,
            ..notify("Copying").hint("value", 75i32)
        });

        assert_eq!(replaced, id);
        assert_eq!(daemon.bubbles.bubble(id).progress, Some(75));
        assert_eq!(
            *daemon.bubbles.calls.borrow(),
            [format!("show {id}"), format!("update {id}")]
        );
        daemon.close(id);
        // Out of range is kept to it, and no hint is no bar.
        let over = daemon.notify(notify("Over").hint("value", 140i32));
        let under = daemon.notify(notify("Under").hint("value", -5i32));
        assert_eq!(daemon.bubbles.bubble(over).progress, Some(100));
        assert_eq!(daemon.bubbles.bubble(under).progress, Some(0));
        assert_eq!(
            daemon
                .bubbles
                .bubble(daemon.notify(notify("Plain")))
                .progress,
            None
        );
    });
}

#[test]
fn an_app_closing_its_notification_takes_the_bubble_away() {
    bus::run(|bus| {
        let daemon = start(bus);
        let id = daemon.notify(notify("Closed"));

        daemon.close(id);
        daemon.wait_for_signals(1);

        assert!(daemon.bubbles.ids().is_empty());
        assert_eq!(
            daemon.signals(),
            [signal("NotificationClosed", &format!("({id}, {CLOSED})"))]
        );
        // A second close of it, or of one there never was, does nothing.
        daemon.close(id);
        daemon.close(4242);
        bus.run_for(Duration::from_millis(100));
        assert_eq!(daemon.signals().len(), 1);
    });
}

#[test]
fn a_bubble_expires_after_the_apps_timeout_or_else_the_timeout_setting() {
    bus::run(|bus| {
        let daemon = start(bus);
        // Changed while the daemon runs, as the settings window changes it.
        daemon.settings.set_uint("timeout", 1).unwrap();
        let own = daemon.notify(Notify {
            timeout: 50,
            ..notify("Own")
        });
        let default = daemon.notify(notify("Default"));

        daemon.wait_for_signals(1);
        assert_eq!(
            daemon.signals(),
            [signal("NotificationClosed", &format!("({own}, {EXPIRED})"))]
        );
        assert_eq!(daemon.bubbles.ids(), [default]);
        daemon.wait_for_signals(2);
        assert_eq!(
            daemon.signals()[1],
            signal("NotificationClosed", &format!("({default}, {EXPIRED})"))
        );
        assert!(daemon.bubbles.ids().is_empty());
    });
}

#[test]
fn a_critical_notification_or_one_with_no_timeout_never_expires() {
    bus::run(|bus| {
        let daemon = start(bus);
        let critical = daemon.notify(Notify {
            timeout: 50,
            ..notify("Battery low").hint("urgency", 2u8)
        });
        let forever = daemon.notify(Notify {
            timeout: 0,
            ..notify("Forever")
        });

        // Six times the timeouts it would have had.
        bus.run_for(Duration::from_millis(300));

        assert_eq!(daemon.bubbles.ids(), [critical, forever]);
        assert!(daemon.signals().is_empty());
    });
}

#[test]
fn hovering_a_bubble_pauses_its_timeout() {
    bus::run(|bus| {
        let daemon = start(bus);
        let id = daemon.notify(Notify {
            timeout: 150,
            ..notify("Hovered")
        });
        daemon.bubbles.events(id).hovered(true);

        // Three times its timeout.
        bus.run_for(Duration::from_millis(450));
        assert_eq!(daemon.bubbles.ids(), [id]);

        daemon.bubbles.events(id).hovered(false);
        daemon.wait_for_signals(1);
        assert_eq!(
            daemon.signals(),
            [signal("NotificationClosed", &format!("({id}, {EXPIRED})"))]
        );
    });
}

// The line of a bubble's time left (CMN-286): what the bubble is told of its timeout.

/// Asserts that `told` is a timeout of `timeout` ms, `running` or not, with between `least` and
/// `timeout` ms of it left.
fn assert_time_left(told: Option<TimeLeft>, timeout: u64, least: u64, running: bool) {
    let told = told.expect("the bubble has a timeout");
    assert_eq!(told.timeout, Duration::from_millis(timeout));
    assert_eq!(told.running, running);
    assert!(
        (Duration::from_millis(least)..=Duration::from_millis(timeout)).contains(&told.left),
        "{:?} left of {timeout} ms, expected at least {least} ms",
        told.left
    );
}

#[test]
fn an_opening_bubble_is_told_its_whole_timeout_running() {
    bus::run(|bus| {
        let daemon = start(bus);
        let id = daemon.notify(Notify {
            timeout: 1000,
            ..notify("Timed")
        });
        assert_time_left(daemon.bubbles.told_time_left(id), 1000, 900, true);
    });
}

#[test]
fn a_hovered_bubble_is_told_its_time_left_waits_and_then_runs_on_from_there() {
    bus::run(|bus| {
        let daemon = start(bus);
        let id = daemon.notify(Notify {
            timeout: 1000,
            ..notify("Hovered")
        });
        // A fifth of its timeout, so that what's left is less than all of it.
        bus.run_for(Duration::from_millis(200));

        daemon.bubbles.events(id).hovered(true);
        let paused = daemon.bubbles.told_time_left(id);
        assert_time_left(paused, 1000, 500, false);
        let left = paused.unwrap().left;
        assert!(
            left <= Duration::from_millis(800),
            "{left:?} left after 200 ms"
        );

        // The time it waits doesn't count, so that it goes on with what's left after as long
        // as it ran before, and more.
        bus.run_for(Duration::from_millis(300));
        daemon.bubbles.events(id).hovered(false);
        let running = daemon.bubbles.told_time_left(id).unwrap();
        assert!(running.running);
        assert!(running.left <= left && running.left + Duration::from_millis(50) >= left);
    });
}

#[test]
fn a_bubble_is_told_its_whole_timeout_again_when_it_restarts() {
    bus::run(|bus| {
        let daemon = start(bus);
        let timed = |summary| Notify {
            timeout: 1000,
            ..notify(summary)
        };
        let replaced = daemon.notify(Notify {
            app_name: "Other",
            ..timed("Replaced")
        });
        daemon.notify(timed("1"));
        daemon.notify(timed("2"));
        let grouped = daemon.notify(timed("3"));
        // Long enough into their timeouts that a restart has more left than they had.
        bus.run_for(Duration::from_millis(400));
        // So that what they're told next is from their restart.
        daemon.bubbles.time_left.borrow_mut().clear();

        daemon.notify(Notify {
            app_name: "Other",
            replaces: replaced,
            ..timed("Replacement")
        });
        daemon.notify(timed("4"));
        assert_time_left(daemon.bubbles.told_time_left(replaced), 1000, 900, true);
        assert_time_left(daemon.bubbles.told_time_left(grouped), 1000, 900, true);
    });
}

#[test]
fn a_bubble_that_never_times_out_is_told_it_has_no_time_left() {
    bus::run(|bus| {
        let daemon = start(bus);
        let critical = daemon.notify(Notify {
            timeout: 1000,
            ..notify("Battery low").hint("urgency", 2u8)
        });
        let forever = daemon.notify(Notify {
            app_name: "Forever",
            timeout: 0,
            ..notify("Forever")
        });
        let replaced = daemon.notify(Notify {
            app_name: "Replaced",
            timeout: 1000,
            ..notify("Timed")
        });
        daemon.notify(Notify {
            app_name: "Replaced",
            replaces: replaced,
            timeout: 0,
            ..notify("Forever now")
        });

        assert_eq!(daemon.bubbles.told_time_left(critical), None);
        assert_eq!(daemon.bubbles.told_time_left(forever), None);
        assert_eq!(daemon.bubbles.told_time_left(replaced), None);
    });
}

#[test]
fn closing_a_bubble_or_clicking_one_without_a_default_action_dismisses_it() {
    bus::run(|bus| {
        // Outside a Session there's no Caminito to bring the app forward.
        let daemon = start(bus);
        let closed = daemon.notify(notify("Closed"));
        let clicked = daemon.notify(Notify {
            actions: vec!["open", "Open"],
            ..notify("Clicked")
        });

        daemon.bubbles.events(closed).dismiss();
        daemon
            .bubbles
            .events(clicked)
            .click(Some("token".to_owned()));
        daemon.wait_for_signals(2);

        assert!(daemon.bubbles.ids().is_empty());
        assert_eq!(
            daemon.signals(),
            [
                signal("NotificationClosed", &format!("({closed}, {DISMISSED})")),
                signal("NotificationClosed", &format!("({clicked}, {DISMISSED})")),
            ]
        );
    });
}

#[test]
fn a_click_without_a_default_action_asks_caminito_to_bring_the_app_forward() {
    bus::run(|bus| {
        let caminito = FakeCaminito::start(bus);
        let daemon = start(bus);
        let by_entry = daemon.notify(notify("Mail").hint("desktop-entry", "org.gnome.Evolution"));
        let by_name = daemon.notify(Notify {
            app_name: "Pidgin",
            ..notify("Chat")
        });

        daemon
            .bubbles
            .events(by_entry)
            .click(Some("token".to_owned()));
        daemon.bubbles.events(by_name).click(None);
        bus.wait_until("two activations", || caminito.activated().len() == 2);
        daemon.wait_for_signals(2);

        assert_eq!(caminito.activated(), ["org.gnome.Evolution", "Pidgin"]);
        // The app hears of no action it didn't offer, only that the notification went.
        assert_eq!(
            daemon.signals(),
            [
                signal("NotificationClosed", &format!("({by_entry}, {DISMISSED})")),
                signal("NotificationClosed", &format!("({by_name}, {DISMISSED})")),
            ]
        );
    });
}

#[test]
fn a_click_with_a_default_action_does_not_ask_caminito() {
    bus::run(|bus| {
        let caminito = FakeCaminito::start(bus);
        let daemon = start(bus);
        let id = daemon.notify(Notify {
            actions: vec!["default", "Open"],
            ..notify("Mail")
        });

        daemon
            .bubbles
            .events(id)
            .click(Some("xdg-token".to_owned()));
        daemon.wait_for_signals(3);
        bus.run_for(QUIET);

        // The token first, so that the app has it as it acts and Caminito lets it in.
        assert_eq!(
            daemon.signals(),
            [
                signal("ActivationToken", &format!("({id}, 'xdg-token')")),
                signal("ActionInvoked", &format!("({id}, 'default')")),
                signal("NotificationClosed", &format!("({id}, {DISMISSED})")),
            ]
        );
        assert!(caminito.activated().is_empty());
    });
}

#[test]
fn an_action_gives_the_app_its_token_and_its_key_and_closes_the_notification() {
    bus::run(|bus| {
        let daemon = start(bus);
        let id = daemon.notify(Notify {
            actions: vec!["default", "Open", "reply", "Reply"],
            ..notify("Message")
        });

        daemon
            .bubbles
            .events(id)
            .invoke("reply", Some("xdg-token".to_owned()));
        daemon.wait_for_signals(3);

        assert_eq!(
            daemon.signals(),
            [
                signal("ActivationToken", &format!("({id}, 'xdg-token')")),
                signal("ActionInvoked", &format!("({id}, 'reply')")),
                signal("NotificationClosed", &format!("({id}, {DISMISSED})")),
            ]
        );
        assert!(daemon.bubbles.ids().is_empty());
    });
}

#[test]
fn a_click_invokes_the_default_action_and_a_resident_notification_stays() {
    bus::run(|bus| {
        let daemon = start(bus);
        let id = daemon.notify(Notify {
            actions: vec!["default", "Open"],
            ..notify("Resident").hint("resident", true)
        });

        // Without a token, as when GDK couldn't get one.
        daemon.bubbles.events(id).click(None);
        daemon.wait_for_signals(1);
        bus.run_for(Duration::from_millis(100));

        assert_eq!(
            daemon.signals(),
            [signal("ActionInvoked", &format!("({id}, 'default')"))]
        );
        assert_eq!(daemon.bubbles.ids(), [id]);
    });
}

#[test]
fn an_inline_reply_action_is_a_reply_field_with_the_apps_placeholder() {
    bus::run(|bus| {
        let daemon = start(bus);

        let with_placeholder = daemon.notify(Notify {
            actions: vec!["default", "Open", "inline-reply", "Send", "mute", "Mute"],
            ..notify("Ana").hint("x-kde-reply-placeholder-text", "Reply to Ana…")
        });
        let without_placeholder = daemon.notify(Notify {
            actions: vec!["inline-reply", "Send"],
            ..notify("Bob")
        });

        let bubble = daemon.bubbles.bubble(with_placeholder);
        assert_eq!(bubble.reply.as_deref(), Some("Reply to Ana…"));
        // The field isn't a button.
        assert_eq!(
            bubble.actions,
            [Action {
                key: "mute".to_owned(),
                label: "Mute".to_owned()
            }]
        );
        // Without the hint, the action's label is the placeholder.
        assert_eq!(
            daemon.bubbles.bubble(without_placeholder).reply.as_deref(),
            Some("Send")
        );
        assert_eq!(
            daemon.bubbles.bubble(daemon.notify(notify("Plain"))).reply,
            None
        );
    });
}

#[test]
fn a_reply_from_the_bubble_reaches_the_app_and_closes_the_notification() {
    bus::run(|bus| {
        let daemon = start(bus);
        let id = daemon.notify(Notify {
            actions: vec!["inline-reply", "Reply"],
            ..notify("Ana")
        });

        daemon.bubbles.events(id).reply("On my way");
        daemon.wait_for_signals(2);

        assert_eq!(
            daemon.signals(),
            [
                signal("NotificationReplied", &format!("({id}, 'On my way')")),
                signal("NotificationClosed", &format!("({id}, {DISMISSED})")),
            ]
        );
        assert!(daemon.bubbles.ids().is_empty());
        assert!(daemon.summaries().is_empty());
    });
}

#[test]
fn bubbles_show_as_many_as_fit_and_one_that_doesnt_fit_gets_none_later_either() {
    bus::run(|bus| {
        let daemon = start(bus);
        // Each in a bubble of its own.
        daemon.settings.set_boolean("group", false).unwrap();
        daemon.bubbles.room.set(Some(3));
        let ids: Vec<u32> = ["1", "2", "3", "4", "5"]
            .into_iter()
            .map(|summary| daemon.notify(notify(summary)))
            .collect();
        assert_eq!(daemon.bubbles.ids(), ids[..3]);
        assert_eq!(
            daemon.bubbles.header.get(),
            Header {
                more: 2,
                critical: false
            }
        );
        // They're in the history, as any other.
        assert_eq!(daemon.summaries(), ["5", "4", "3", "2", "1"]);

        // A bubble that closes doesn't make room for them.
        daemon.bubbles.events(ids[1]).dismiss();
        assert_eq!(daemon.bubbles.ids(), [ids[0], ids[2]]);
        assert_eq!(daemon.bubbles.header.get().more, 2);

        // The next one fits where that one was.
        let next = daemon.notify(notify("6"));
        assert_eq!(daemon.bubbles.ids(), [ids[0], ids[2], next]);
    });
}

#[test]
fn the_count_loses_what_leaves_the_history_and_empties_when_the_last_bubble_closes() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.bubbles.room.set(Some(1));
        let shown = daemon.notify(notify("Shown"));
        let withdrawn = daemon.notify(notify("Withdrawn"));
        daemon.notify(notify("Counted"));
        assert_eq!(daemon.bubbles.header.get().more, 2);

        daemon.close(withdrawn);
        assert_eq!(daemon.bubbles.header.get().more, 1);

        daemon.bubbles.events(shown).dismiss();
        assert!(daemon.bubbles.ids().is_empty());
        assert_eq!(daemon.bubbles.header.get(), Header::default());
        assert_eq!(daemon.summaries(), ["Counted"]);
    });
}

#[test]
fn clear_all_in_the_header_takes_the_bubbles_out_of_the_history_and_leaves_the_counted() {
    bus::run(|bus| {
        let daemon = start(bus);
        // Each in a bubble of its own.
        daemon.settings.set_boolean("group", false).unwrap();
        daemon.bubbles.room.set(Some(2));
        let first = daemon.notify(notify("First"));
        let second = daemon.notify(notify("Second"));
        daemon.notify(notify("Counted"));
        daemon.forget_signals();

        daemon.bubbles.header_events().clear_all();

        assert!(daemon.bubbles.ids().is_empty());
        assert_eq!(daemon.bubbles.header.get(), Header::default());
        assert_eq!(daemon.summaries(), ["Counted"]);
        daemon.wait_for_signals(2);
        assert_eq!(
            daemon.signals(),
            [
                signal("NotificationClosed", &format!("({first}, {DISMISSED})")),
                signal("NotificationClosed", &format!("({second}, {DISMISSED})")),
            ]
        );
    });
}

#[test]
fn clear_all_in_the_header_works_while_every_notification_has_a_bubble() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.notify(notify("Only"));

        daemon.bubbles.header_events().clear_all();

        assert!(daemon.bubbles.ids().is_empty());
        assert!(daemon.summaries().is_empty());
    });
}

#[test]
fn a_replacement_of_a_counted_notification_is_counted_once() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.bubbles.room.set(Some(1));
        daemon.notify(notify("Shown"));
        let counted = daemon.notify(notify("Counted"));

        daemon.notify(Notify {
            replaces: counted,
            ..notify("Counted again")
        });

        assert_eq!(daemon.bubbles.header.get().more, 1);
    });
}

#[test]
fn a_critical_notification_takes_the_place_of_the_oldest_ordinary_bubble_or_gets_none() {
    bus::run(|bus| {
        let daemon = start(bus);
        // Each in a bubble of its own.
        daemon.settings.set_boolean("group", false).unwrap();
        daemon.bubbles.room.set(Some(2));
        let critical = |summary| notify(summary).hint("urgency", 2u8);
        daemon.notify(notify("Ordinary"));
        let first = daemon.notify(critical("First"));
        daemon.forget_signals();

        let second = daemon.notify(critical("Second"));
        assert_eq!(daemon.bubbles.ids(), [first, second]);
        assert_eq!(
            daemon.bubbles.header.get(),
            Header {
                more: 1,
                critical: false
            }
        );
        // Its notification stays the app's, in the history.
        bus.run_for(QUIET);
        assert!(daemon.signals().is_empty());
        assert!(daemon.summaries().contains(&"Ordinary".to_owned()));

        let third = daemon.notify(critical("Third"));
        assert_eq!(daemon.bubbles.ids(), [first, second]);
        assert_eq!(
            daemon.bubbles.header.get(),
            Header {
                more: 2,
                critical: true
            }
        );
        assert!(daemon.history().iter().any(|entry| entry.id == third));
    });
}

#[test]
fn a_new_notification_plays_its_sound_or_the_themes_and_none_when_suppressed() {
    bus::run(|bus| {
        let daemon = start(bus);

        let plain = daemon.notify(notify("Plain"));
        daemon.notify(notify("Named").hint("sound-name", "bell"));
        daemon.notify(notify("File").hint("sound-file", "/tmp/ding.oga"));
        daemon.notify(
            notify("Quiet")
                .hint("sound-name", "bell")
                .hint("suppress-sound", true),
        );
        // A replacement isn't a new notification.
        daemon.notify(Notify {
            replaces: plain,
            ..notify("Plain again")
        });

        assert_eq!(
            *daemon.sounds.0.borrow(),
            [
                Sound::Name("message-new-instant".to_owned()),
                Sound::Name("bell".to_owned()),
                Sound::File("/tmp/ding.oga".into()),
            ]
        );
    });
}

#[test]
fn an_image_data_hints_pixels_are_the_bubbles_image_and_a_broken_one_is_none() {
    bus::run(|bus| {
        let daemon = start(bus);
        // Two by one, RGBA, with a row of padding.
        let data = vec![255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0];
        let image = (2i32, 1i32, 10i32, true, 8i32, 4i32, data.clone());

        let id = daemon.notify(notify("Pixels").hint("image-data", image));
        let broken = daemon.notify(notify("Broken").hint(
            "image-data",
            (2i32, 2i32, 10i32, true, 8i32, 4i32, data.clone()),
        ));

        let Some(Image::Pixels(pixels)) = daemon.bubbles.bubble(id).image else {
            panic!("the bubble has the pixels");
        };
        assert_eq!(
            (
                pixels.width,
                pixels.height,
                pixels.rowstride,
                pixels.has_alpha
            ),
            (2, 1, 10, true)
        );
        assert_eq!(&pixels.data[..], &data[..]);
        // Its second row isn't there.
        assert_eq!(daemon.bubbles.bubble(broken).image, None);
    });
}

/// The summaries of the bubbles on screen, oldest first.
fn shown_summaries(daemon: &Daemon<'_>) -> Vec<String> {
    daemon
        .bubbles
        .shown
        .borrow()
        .iter()
        .map(|shown| shown.1.summary.clone())
        .collect()
}

#[test]
fn a_notification_enters_the_history_with_what_a_bubble_shows_unless_it_is_transient() {
    bus::run(|bus| {
        let daemon = start(bus);

        let id = daemon.notify(Notify {
            icon: "mail-unread",
            body: "From <b>Ann</b> & Bob",
            actions: vec![
                "default",
                "Open",
                "inline-reply",
                "Reply",
                "archive",
                "Archive",
            ],
            ..notify("New mail").hint("x-kde-reply-placeholder-text", "Reply to Ann")
        });
        daemon.notify(notify("Gone").hint("transient", true));

        assert_eq!(
            daemon.history(),
            [Listed {
                id,
                app: "Test".to_owned(),
                app_name: "Test".to_owned(),
                app_icon: "mail-unread".to_owned(),
                summary: "New mail".to_owned(),
                body: "From <b>Ann</b> &amp; Bob".to_owned(),
                image: String::new(),
                actions: vec![("archive".to_owned(), "Archive".to_owned())],
                has_default: true,
                resident: false,
                reply: "Reply to Ann".to_owned(),
                time: 1_000_000,
            }]
        );
        // Its bubble was shown all the same.
        assert_eq!(shown_summaries(&daemon), ["New mail", "Gone"]);
    });
}

#[test]
fn the_image_of_an_entry_is_its_path_or_its_pixels() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.notify(notify("Path").hint("image-path", "file:///tmp/photo.png"));
        daemon.notify(notify("Pixels").hint(
            "image-data",
            (1i32, 1i32, 4i32, true, 8i32, 4i32, vec![1u8, 2, 3, 4]),
        ));

        let (entries,) = daemon
            .api("List", ().to_variant())
            .get::<(Vec<WireEntry>,)>()
            .unwrap();

        let pixels = &entries[0].6;
        assert_eq!(
            pixels.get::<(i32, i32, i32, bool, i32, i32, Vec<u8>)>(),
            Some((1, 1, 4, true, 8, 4, vec![1, 2, 3, 4]))
        );
        assert_eq!(
            Listed::from(entries.into_iter().nth(1).unwrap()).image,
            "/tmp/photo.png"
        );
    });
}

#[test]
fn an_expired_bubble_leaves_its_notification_in_the_history() {
    bus::run(|bus| {
        let daemon = start(bus);
        let id = daemon.notify(Notify {
            timeout: 50,
            ..notify("Missed")
        });

        daemon.wait_for_signals(1);

        assert_eq!(
            daemon.signals(),
            [signal("NotificationClosed", &format!("({id}, {EXPIRED})"))]
        );
        assert!(daemon.bubbles.ids().is_empty());
        assert_eq!(daemon.summaries(), ["Missed"]);
    });
}

#[test]
fn the_history_survives_a_restart_of_the_daemon_and_its_ids_stay_taken() {
    bus::run(|bus| {
        let daemon = start(bus);
        let first = daemon.notify(Notify {
            timeout: 0,
            ..notify("One").hint("image-path", "/tmp/one.png")
        });
        let second = daemon.notify(Notify {
            actions: vec!["default", "Open"],
            ..notify("Two").hint("desktop-entry", "mail")
        });
        let before = daemon.history();
        let home = daemon.home.clone();
        drop(daemon);

        let daemon = start_in(bus, home);

        assert_eq!(daemon.history(), before);
        assert_eq!(
            daemon.counts(),
            [("Test".to_owned(), 1), ("mail".to_owned(), 1)]
        );
        let third = daemon.notify(notify("Three"));
        assert!(![first, second].contains(&third));
        assert_eq!(daemon.summaries(), ["Three", "Two", "One"]);
    });
}

#[test]
fn an_entry_a_week_old_goes_when_the_history_is_next_asked() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.notify(notify("Old"));
        daemon.home.clock.0.set(1_000_000 + WEEK - 1);
        daemon.notify(notify("Young"));
        assert_eq!(daemon.summaries(), ["Young", "Old"]);
        daemon.forget_signals();

        daemon.home.clock.0.set(1_000_000 + WEEK);

        assert_eq!(daemon.summaries(), ["Young"]);
        daemon.wait_for_api_signals(2);
        assert_eq!(
            daemon.api_signals(),
            [
                signal("HistoryChanged", "()"),
                signal("CountsChanged", "({'Test': 1},)"),
            ]
        );
        // A restart a week on forgets what the file still holds.
        let home = daemon.home.clone();
        drop(daemon);
        home.clock.0.set(1_000_000 + 2 * WEEK);
        assert!(start_in(bus, home).history().is_empty());
    });
}

#[test]
fn an_entry_goes_when_its_week_ends_though_nobody_asks() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.notify(notify("Old"));
        daemon.home.clock.0.set(1_000_000 + WEEK - 1);
        // It waits for the end of the oldest entry's week, which is a second away.
        daemon.notify(notify("Young"));
        daemon.forget_signals();

        daemon.home.clock.0.set(1_000_000 + WEEK);
        daemon.wait_for_api_signals(1);

        assert_eq!(daemon.api_signals()[0], signal("HistoryChanged", "()"));
        assert_eq!(daemon.summaries(), ["Young"]);
    });
}

#[test]
fn dismissing_clicking_or_using_an_action_takes_a_notification_out_of_the_history() {
    bus::run(|bus| {
        let daemon = start(bus);
        // Each in a bubble of its own.
        daemon.settings.set_boolean("group", false).unwrap();
        let dismissed = daemon.notify(notify("Dismissed"));
        let clicked = daemon.notify(Notify {
            actions: vec!["default", "Open"],
            ..notify("Clicked")
        });
        let acted = daemon.notify(Notify {
            actions: vec!["act", "Act"],
            ..notify("Acted")
        });
        daemon.bubbles.events(dismissed).dismiss();
        daemon.bubbles.events(clicked).click(None);
        daemon.bubbles.events(acted).invoke("act", None);
        assert!(daemon.history().is_empty());

        // A resident notification stays through both.
        let click = daemon.notify(Notify {
            actions: vec!["default", "Open"],
            ..notify("Resident click").hint("resident", true)
        });
        let action = daemon.notify(Notify {
            actions: vec!["act", "Act"],
            ..notify("Resident action").hint("resident", true)
        });
        let kept = daemon.notify(notify("Untouched"));
        daemon.bubbles.events(click).click(None);
        daemon.bubbles.events(action).invoke("act", None);

        assert_eq!(
            daemon.summaries(),
            ["Untouched", "Resident action", "Resident click"]
        );
        assert!(
            daemon
                .history()
                .iter()
                .any(|entry| entry.id == kept && !entry.resident)
        );
    });
}

#[test]
fn an_app_withdrawing_a_notification_takes_it_out_of_the_history_even_after_its_bubble_expired() {
    bus::run(|bus| {
        let daemon = start(bus);
        let live = daemon.notify(notify("Live"));
        let missed = daemon.notify(Notify {
            timeout: 50,
            ..notify("Missed")
        });
        daemon.wait_for_signals(1);

        daemon.close(live);
        daemon.close(missed);
        daemon.wait_for_signals(3);

        assert!(daemon.history().is_empty());
        assert_eq!(
            daemon.signals()[1..],
            [
                signal("NotificationClosed", &format!("({live}, {CLOSED})")),
                signal("NotificationClosed", &format!("({missed}, {CLOSED})")),
            ]
        );
    });
}

#[test]
fn a_replacement_of_an_entry_whose_bubble_expired_keeps_its_id_and_shows_a_bubble_again() {
    bus::run(|bus| {
        let daemon = start(bus);
        let id = daemon.notify(Notify {
            timeout: 50,
            ..notify("One")
        });
        daemon.wait_for_signals(1);

        let replaced = daemon.notify(Notify {
            replaces: id,
            ..notify("Two")
        });

        assert_eq!(replaced, id);
        assert_eq!(daemon.summaries(), ["Two"]);
        assert_eq!(daemon.bubbles.ids(), [id]);
        // Not a new notification, so it has no sound of its own.
        assert_eq!(daemon.sounds.0.borrow().len(), 1);
    });
}

#[test]
fn a_replacement_changes_the_entry_in_place_and_a_transient_one_leaves_the_history() {
    bus::run(|bus| {
        let daemon = start(bus);
        let id = daemon.notify(Notify {
            timeout: 0,
            ..notify("One")
        });
        daemon.notify(notify("Other"));

        daemon.notify(Notify {
            replaces: id,
            timeout: 0,
            ..notify("Two")
        });
        assert_eq!(daemon.summaries(), ["Two", "Other"]);

        daemon.notify(Notify {
            replaces: id,
            timeout: 0,
            ..notify("Three").hint("transient", true)
        });
        assert_eq!(daemon.summaries(), ["Other"]);
    });
}

#[test]
fn notifying_apps_are_keyed_by_their_desktop_entry_or_else_their_name_and_counted() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.notify(notify("Named"));
        daemon.notify(Notify {
            app_name: "Mail",
            ..notify("Entry").hint("desktop-entry", "org.example.Mail")
        });
        daemon.notify(Notify {
            app_name: "Mail Beta",
            ..notify("Same entry").hint("desktop-entry", "org.example.Mail")
        });
        daemon.notify(notify("Transient").hint("transient", true));

        assert_eq!(
            daemon.counts(),
            [("Test".to_owned(), 1), ("org.example.Mail".to_owned(), 2)]
        );
        assert_eq!(
            daemon
                .history()
                .iter()
                .map(|entry| entry.app.as_str())
                .collect::<Vec<_>>(),
            ["org.example.Mail", "org.example.Mail", "Test"]
        );
    });
}

#[test]
fn a_new_notification_and_a_replacement_fire_the_change_signals_that_changed() {
    bus::run(|bus| {
        let daemon = start(bus);

        let id = daemon.notify(Notify {
            timeout: 0,
            ..notify("One")
        });
        daemon.wait_for_api_signals(2);
        daemon.forget_signals();
        daemon.notify(Notify {
            replaces: id,
            timeout: 0,
            ..notify("Two")
        });
        daemon.wait_for_api_signals(1);
        daemon.bus.run_for(QUIET);

        // The count of the app is the same, so the counts don't change.
        assert_eq!(daemon.api_signals(), [signal("HistoryChanged", "()")]);
        daemon.forget_signals();
        daemon.notify(notify("Transient").hint("transient", true));
        daemon.bus.run_for(QUIET);
        assert!(daemon.api_signals().is_empty());
    });
}

#[test]
fn the_api_dismisses_one_entry_and_the_app_hears_it_was_dismissed() {
    bus::run(|bus| {
        let daemon = start(bus);
        let first = daemon.notify(Notify {
            timeout: 0,
            ..notify("First")
        });
        daemon.notify(Notify {
            timeout: 0,
            ..notify("Second")
        });
        daemon.forget_signals();

        daemon.api("Dismiss", (first,).to_variant());
        daemon.wait_for_signals(1);
        daemon.wait_for_api_signals(2);

        assert_eq!(daemon.summaries(), ["Second"]);
        assert_eq!(shown_summaries(&daemon), ["Second"]);
        assert_eq!(
            daemon.signals(),
            [signal(
                "NotificationClosed",
                &format!("({first}, {DISMISSED})")
            )]
        );
        assert_eq!(
            daemon.api_signals(),
            [
                signal("HistoryChanged", "()"),
                signal("CountsChanged", "({'Test': 1},)"),
            ]
        );
        // One that isn't there is refused, and changes nothing.
        assert!(daemon.try_api("Dismiss", (first,).to_variant()).is_err());
        assert_eq!(daemon.summaries(), ["Second"]);
    });
}

#[test]
fn the_api_clears_an_apps_entries_and_then_all_of_them_with_one_change_each() {
    bus::run(|bus| {
        let daemon = start(bus);
        let notify_for = |app_name, summary| {
            daemon.notify(Notify {
                app_name,
                timeout: 0,
                ..notify(summary)
            })
        };
        let a1 = notify_for("A", "A one");
        let b1 = notify_for("B", "B one");
        let a2 = notify_for("A", "A two");
        daemon.forget_signals();

        daemon.api("ClearApp", ("A",).to_variant());
        daemon.wait_for_signals(2);
        daemon.wait_for_api_signals(2);

        assert_eq!(daemon.summaries(), ["B one"]);
        assert_eq!(daemon.counts(), [("B".to_owned(), 1)]);
        let mut closed = daemon.signals();
        closed.sort();
        assert_eq!(
            closed,
            [
                signal("NotificationClosed", &format!("({a1}, {DISMISSED})")),
                signal("NotificationClosed", &format!("({a2}, {DISMISSED})")),
            ]
        );
        assert_eq!(
            daemon.api_signals(),
            [
                signal("HistoryChanged", "()"),
                signal("CountsChanged", "({'B': 1},)"),
            ]
        );
        daemon.forget_signals();

        daemon.api("ClearAll", ().to_variant());
        daemon.wait_for_signals(1);
        daemon.wait_for_api_signals(2);

        assert!(daemon.history().is_empty());
        assert!(daemon.counts().is_empty());
        assert!(daemon.bubbles.ids().is_empty());
        assert_eq!(
            daemon.signals(),
            [signal(
                "NotificationClosed",
                &format!("({b1}, {DISMISSED})")
            )]
        );
        assert_eq!(
            daemon.api_signals(),
            [
                signal("HistoryChanged", "()"),
                signal("CountsChanged", "({},)"),
            ]
        );
        // Nothing to clear is no change.
        daemon.forget_signals();
        daemon.api("ClearAll", ().to_variant());
        daemon.api("ClearApp", ("A",).to_variant());
        daemon.bus.run_for(QUIET);
        assert!(daemon.api_signals().is_empty());
    });
}

#[test]
fn the_api_invokes_an_action_with_the_token_and_the_notification_leaves_unless_resident() {
    bus::run(|bus| {
        let daemon = start(bus);
        let id = daemon.notify(Notify {
            timeout: 50,
            actions: vec!["default", "Open", "reply", "Reply"],
            ..notify("Message")
        });
        let resident = daemon.notify(Notify {
            timeout: 0,
            actions: vec!["default", "Open"],
            ..notify("Resident").hint("resident", true)
        });
        // The first one's bubble is gone: the entry is all that's left.
        daemon.wait_for_signals(1);
        daemon.forget_signals();

        daemon.api("InvokeAction", (id, "reply", "xdg-token").to_variant());
        daemon.wait_for_signals(3);

        assert_eq!(
            daemon.signals(),
            [
                signal("ActivationToken", &format!("({id}, 'xdg-token')")),
                signal("ActionInvoked", &format!("({id}, 'reply')")),
                signal("NotificationClosed", &format!("({id}, {DISMISSED})")),
            ]
        );
        daemon.forget_signals();

        // Without a token, and for the default action a click invokes.
        daemon.api("InvokeAction", (resident, "default", "").to_variant());
        daemon.wait_for_signals(1);
        daemon.bus.run_for(QUIET);

        assert_eq!(
            daemon.signals(),
            [signal("ActionInvoked", &format!("({resident}, 'default')"))]
        );
        assert_eq!(daemon.summaries(), ["Resident"]);
    });
}

#[test]
fn the_api_refuses_an_action_the_entry_doesnt_have_and_an_entry_that_isnt_there() {
    bus::run(|bus| {
        let daemon = start(bus);
        let id = daemon.notify(Notify {
            timeout: 0,
            actions: vec!["open", "Open"],
            ..notify("Message")
        });

        // There's no such action.
        assert!(
            daemon
                .try_api("InvokeAction", (id, "other", "").to_variant())
                .is_err()
        );
        assert!(
            daemon
                .try_api("InvokeAction", (id + 1, "open", "").to_variant())
                .is_err()
        );
        daemon.bus.run_for(QUIET);

        assert_eq!(daemon.summaries(), ["Message"]);
        assert!(daemon.signals().is_empty());
    });
}

#[test]
fn the_api_clicking_an_entry_without_a_default_action_brings_its_app_forward() {
    bus::run(|bus| {
        let caminito = FakeCaminito::start(bus);
        let daemon = start(bus);
        let id = daemon.notify(Notify {
            timeout: 0,
            actions: vec!["open", "Open"],
            ..notify("Mail").hint("desktop-entry", "org.gnome.Evolution.desktop")
        });
        daemon.forget_signals();

        daemon.api("InvokeAction", (id, "default", "").to_variant());
        bus.wait_until("the activation", || !caminito.activated().is_empty());
        daemon.wait_for_signals(1);

        assert_eq!(caminito.activated(), ["org.gnome.Evolution"]);
        assert_eq!(
            daemon.signals(),
            [signal(
                "NotificationClosed",
                &format!("({id}, {DISMISSED})")
            )]
        );
        assert!(daemon.summaries().is_empty());
    });
}

#[test]
fn the_api_replies_to_an_entry_with_a_reply_field_and_refuses_one_without() {
    bus::run(|bus| {
        let daemon = start(bus);
        let with = daemon.notify(Notify {
            timeout: 0,
            actions: vec!["inline-reply", "Reply"],
            ..notify("Chat")
        });
        // A placeholder without the action is no reply field.
        let without = daemon.notify(Notify {
            timeout: 0,
            ..notify("Plain").hint("x-kde-reply-placeholder-text", "Reply")
        });
        let resident = daemon.notify(Notify {
            timeout: 0,
            actions: vec!["inline-reply", "Reply"],
            ..notify("Resident").hint("resident", true)
        });
        daemon.forget_signals();

        daemon.api("Reply", (with, "On my way").to_variant());
        daemon.wait_for_signals(2);

        assert_eq!(
            daemon.signals(),
            [
                signal("NotificationReplied", &format!("({with}, 'On my way')")),
                signal("NotificationClosed", &format!("({with}, {DISMISSED})")),
            ]
        );
        assert!(
            daemon
                .try_api("Reply", (without, "Hi").to_variant())
                .is_err()
        );
        assert!(
            daemon
                .try_api("Reply", (9999u32, "Hi").to_variant())
                .is_err()
        );

        // A resident one stays.
        daemon.api("Reply", (resident, "Hi").to_variant());
        daemon.wait_for_signals(3);
        daemon.bus.run_for(QUIET);
        assert_eq!(daemon.signals().len(), 3);
        assert_eq!(daemon.summaries(), ["Resident", "Plain"]);
    });
}

#[test]
fn with_previews_off_a_bubble_shows_only_its_app_and_notification() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.settings.set_boolean("previews", false).unwrap();

        let id = daemon.notify(Notify {
            icon: "mail-unread",
            body: "From Ann",
            actions: vec!["reply", "Reply"],
            ..notify("New mail").hint("image-path", "/tmp/photo.png")
        });

        assert_eq!(
            daemon.bubbles.bubble(id),
            Bubble {
                app_name: "Test".to_owned(),
                app_icon: Some(Icon::Name("mail-unread".to_owned())),
                summary: "Notification".to_owned(),
                body: String::new(),
                image: None,
                actions: vec![Action {
                    key: "reply".to_owned(),
                    label: "Reply".to_owned()
                }],
                reply: None,
                progress: None,
                earlier: Vec::new(),
            }
        );
    });
}

#[test]
fn with_the_app_icon_off_a_bubble_has_no_app_icon_but_keeps_its_image() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.settings.set_boolean("app-icon", false).unwrap();

        let off = daemon.notify(Notify {
            icon: "mail-unread",
            ..notify("Off").hint("image-path", "/tmp/photo.png")
        });
        daemon.settings.set_boolean("app-icon", true).unwrap();
        let on = daemon.notify(Notify {
            icon: "mail-unread",
            ..notify("On")
        });

        let off = daemon.bubbles.bubble(off);
        assert_eq!(off.app_icon, None);
        assert_eq!(
            off.image,
            Some(Image::Icon(Icon::File("/tmp/photo.png".into())))
        );
        assert_eq!(
            daemon.bubbles.bubble(on).app_icon,
            Some(Icon::Name("mail-unread".to_owned()))
        );
    });
}

#[test]
fn with_sound_off_no_notification_plays_one() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.settings.set_boolean("sound", false).unwrap();

        let id = daemon.notify(notify("Quiet").hint("sound-name", "bell"));
        daemon.settings.set_boolean("sound", true).unwrap();
        daemon.notify(notify("Loud").hint("sound-name", "bell"));

        assert_eq!(daemon.bubbles.ids().len(), 2);
        assert_eq!(daemon.bubbles.bubble(id).summary, "Quiet");
        assert_eq!(*daemon.sounds.0.borrow(), [Sound::Name("bell".to_owned())]);
    });
}

#[test]
fn while_the_center_is_open_there_are_no_bubbles_and_a_new_notification_only_sounds() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.bubbles.room.set(Some(1));
        daemon.notify(notify("Shown"));
        daemon.notify(notify("Counted"));
        daemon.forget_signals();

        daemon.api("SetCenterOpen", (true,).to_variant());
        assert!(daemon.bubbles.ids().is_empty());
        assert_eq!(daemon.bubbles.header.get(), Header::default());
        // Its notification waits in the center, still the app's.
        bus.run_for(QUIET);
        assert!(daemon.signals().is_empty());

        daemon.notify(notify("In the center"));
        assert!(daemon.bubbles.ids().is_empty());
        assert_eq!(daemon.bubbles.header.get(), Header::default());
        assert_eq!(daemon.sounds.0.borrow().len(), 3);
        assert_eq!(daemon.summaries(), ["In the center", "Counted", "Shown"]);

        daemon.api("SetCenterOpen", (false,).to_variant());
        let after = daemon.notify(notify("After"));
        assert_eq!(daemon.bubbles.ids(), [after]);
    });
}

#[test]
fn the_center_counts_as_closed_once_the_client_that_opened_it_leaves_the_bus() {
    bus::run(|bus| {
        let daemon = start(bus);
        let applet = bus.connect();
        daemon
            .send_on(
                &applet,
                &daemon.service,
                API_PATH,
                API_NAME,
                "SetCenterOpen",
                (true,).to_variant(),
            )
            .expect("SetCenterOpen answers");
        daemon.notify(notify("In the center"));
        assert!(daemon.bubbles.ids().is_empty());

        applet.close_sync(gio::Cancellable::NONE).unwrap();
        // The bus tells the server it left, which a call after it waits for.
        bus.run_for(QUIET);
        let after = daemon.notify(notify("After"));
        assert_eq!(daemon.bubbles.ids(), [after]);
    });
}

#[test]
fn open_center_and_the_headers_count_ask_the_applet_for_the_center() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.bubbles.room.set(Some(1));
        daemon.notify(notify("Shown"));
        daemon.notify(notify("Counted"));
        daemon.forget_signals();

        daemon.api("OpenCenter", ().to_variant());
        daemon.bubbles.header_events().open_center();

        daemon.wait_for_api_signals(2);
        assert_eq!(
            daemon.api_signals(),
            [
                signal("CenterRequested", "()"),
                signal("CenterRequested", "()")
            ]
        );
    });
}

#[test]
fn open_settings_opens_the_window_at_the_general_page_or_an_apps() {
    bus::run(|bus| {
        let daemon = start(bus);

        daemon.api("OpenSettings", ("",).to_variant());
        daemon.api("OpenSettings", ("firefox",).to_variant());

        assert_eq!(
            *daemon.opened_settings.borrow(),
            [None, Some("firefox".to_owned())]
        );
    });
}

#[test]
fn history_only_gives_a_notification_its_entry_but_no_bubble_and_no_sound() {
    bus::run(|bus| {
        let daemon = start(bus);
        let own = daemon.apps.app("Test");
        own.set_string("delivery", "history").unwrap();

        let id = daemon.notify(notify("Quiet").hint("sound-name", "bell"));

        assert!(daemon.bubbles.ids().is_empty());
        assert!(daemon.sounds.0.borrow().is_empty());
        assert_eq!(daemon.summaries(), ["Quiet"]);
        assert_eq!(daemon.history()[0].id, id);
    });
}

#[test]
fn off_drops_a_notification_which_never_enters_the_history() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon
            .apps
            .app("Test")
            .set_string("delivery", "off")
            .unwrap();

        let id = daemon.notify(notify("Dropped").hint("sound-name", "bell"));
        daemon.close(id);

        assert_ne!(id, 0);
        assert!(daemon.bubbles.ids().is_empty());
        assert!(daemon.sounds.0.borrow().is_empty());
        assert!(daemon.summaries().is_empty());
        assert!(daemon.signals().is_empty());
    });
}

#[test]
fn an_app_without_its_own_settings_follows_the_default() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.settings.set_string("delivery", "history").unwrap();

        daemon.notify(notify("Default"));
        daemon
            .apps
            .app("Own")
            .set_string("delivery", "bubble")
            .unwrap();
        let own = daemon.notify(Notify {
            app_name: "Own",
            ..notify("Own")
        });

        assert_eq!(daemon.bubbles.ids(), [own]);
        assert_eq!(daemon.summaries(), ["Own", "Default"]);
    });
}

#[test]
fn an_apps_own_sound_previews_and_app_icon_override_the_default() {
    bus::run(|bus| {
        let daemon = start(bus);
        let own = daemon.apps.app("Own");
        own.set_boolean("sound", false).unwrap();
        own.set_boolean("previews", false).unwrap();
        own.set_boolean("app-icon", false).unwrap();
        daemon.settings.set_boolean("sound", true).unwrap();

        let mine = daemon.notify(Notify {
            app_name: "Own",
            icon: "mail-unread",
            ..notify("Secret").hint("sound-name", "own")
        });
        let other = daemon.notify(Notify {
            icon: "mail-unread",
            ..notify("Open").hint("sound-name", "other")
        });

        let mine = daemon.bubbles.bubble(mine);
        assert_eq!(
            (mine.summary.as_str(), mine.app_icon),
            ("Notification", None)
        );
        let other = daemon.bubbles.bubble(other);
        assert_eq!(other.summary, "Open");
        assert_eq!(other.app_icon, Some(Icon::Name("mail-unread".to_owned())));
        assert_eq!(*daemon.sounds.0.borrow(), [Sound::Name("other".to_owned())]);
    });
}

#[test]
fn an_app_joins_the_known_apps_with_its_first_notification() {
    bus::run(|bus| {
        let daemon = start(bus);

        daemon.notify(notify("First"));
        daemon.notify(Notify {
            app_name: "Mail",
            ..notify("Entry").hint("desktop-entry", "org.example.Mail.desktop")
        });
        daemon.notify(notify("Second"));

        assert_eq!(daemon.apps.known_apps(), ["Test", "org.example.Mail"]);
    });
}

/// The summaries of the notifications before the newest in the bubble `id`, newest first.
fn earlier(daemon: &Daemon<'_>, id: u32) -> Vec<String> {
    let bubble = daemon.bubbles.bubble(id);
    bubble
        .earlier
        .into_iter()
        .map(|earlier| earlier.summary)
        .collect()
}

#[test]
fn an_apps_third_notification_merges_its_first_two_bubbles_into_one_which_later_ones_join() {
    bus::run(|bus| {
        let daemon = start(bus);
        let first = daemon.notify(notify("1"));
        let second = daemon.notify(notify("2"));
        let other = daemon.notify(Notify {
            app_name: "Other",
            ..notify("Other")
        });
        assert_eq!(daemon.bubbles.ids(), [first, second, other]);

        let third = daemon.notify(Notify {
            actions: vec!["reply", "Reply"],
            ..notify("3")
        });
        assert_eq!(daemon.bubbles.ids(), [other, third]);
        let grouped = daemon.bubbles.bubble(third);
        assert_eq!(grouped.summary, "3");
        assert_eq!(grouped.actions[0].key, "reply");
        assert_eq!(earlier(&daemon, third), ["2", "1"]);

        let fourth = daemon.notify(notify("4"));
        assert_eq!(daemon.bubbles.ids(), [other, third]);
        assert_eq!(daemon.bubbles.bubble(third).summary, "4");
        assert_eq!(earlier(&daemon, third), ["3", "2", "1"]);

        // Its actions are the newest's, and the others stay in the bubble.
        daemon.bubbles.events(third).invoke("reply", None);
        daemon.wait_for_signals(2);
        assert_eq!(
            daemon.signals(),
            [
                signal("ActionInvoked", &format!("({fourth}, 'reply')")),
                signal("NotificationClosed", &format!("({fourth}, {DISMISSED})")),
            ]
        );
        assert_eq!(daemon.bubbles.bubble(third).summary, "3");

        // Closing it dismisses all of them.
        daemon.forget_signals();
        daemon.bubbles.events(third).dismiss();
        daemon.wait_for_signals(3);
        assert_eq!(daemon.bubbles.ids(), [other]);
        assert_eq!(daemon.summaries(), ["Other"]);
    });
}

#[test]
fn a_notification_joining_a_grouped_bubble_starts_its_timeout_again() {
    bus::run(|bus| {
        let daemon = start(bus);
        let timed = |summary| Notify {
            timeout: 300,
            ..notify(summary)
        };
        daemon.notify(timed("1"));
        daemon.notify(timed("2"));
        let grouped = daemon.notify(timed("3"));

        bus.run_for(Duration::from_millis(200));
        daemon.notify(timed("4"));
        // Past the first three's timeout, but not the fourth's.
        bus.run_for(Duration::from_millis(200));
        assert_eq!(daemon.bubbles.ids(), [grouped]);
        assert!(daemon.signals().is_empty());

        daemon.wait_for_signals(4);
        assert!(daemon.bubbles.ids().is_empty());
        assert_eq!(daemon.summaries(), ["4", "3", "2", "1"]);
    });
}

#[test]
fn critical_notifications_group_only_with_their_apps_critical_ones_and_never_expire() {
    bus::run(|bus| {
        let daemon = start(bus);
        let critical = |summary| Notify {
            timeout: 50,
            ..notify(summary).hint("urgency", 2u8)
        };
        let ordinary = daemon.notify(Notify {
            timeout: 0,
            ..notify("Ordinary")
        });
        daemon.notify(critical("Low"));
        daemon.notify(critical("Lower"));
        let grouped = daemon.notify(critical("Lowest"));

        assert_eq!(daemon.bubbles.ids(), [ordinary, grouped]);
        assert_eq!(earlier(&daemon, grouped), ["Lower", "Low"]);
        assert!(earlier(&daemon, ordinary).is_empty());

        // Six times the timeouts it would have had.
        bus.run_for(Duration::from_millis(300));
        assert_eq!(daemon.bubbles.ids(), [ordinary, grouped]);
    });
}

#[test]
fn with_grouping_off_each_notification_has_a_bubble_of_its_own() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.apps.app("Test").set_boolean("group", false).unwrap();
        let ids: Vec<u32> = ["1", "2", "3"]
            .into_iter()
            .map(|summary| daemon.notify(notify(summary)))
            .collect();

        assert_eq!(daemon.bubbles.ids(), ids);
        assert!(ids.iter().all(|&id| earlier(&daemon, id).is_empty()));
    });
}

impl Daemon<'_> {
    /// The Notifying apps the private API says have grouping off.
    fn ungrouped(&self) -> Vec<String> {
        self.api("Ungrouped", ().to_variant())
            .get::<(Vec<String>,)>()
            .expect("Ungrouped answers apps")
            .0
    }

    /// Waits for the private API's signal `name`.
    fn wait_for_api_signal(&self, name: &str) -> String {
        self.bus.wait_until(name, || {
            self.api_signals
                .borrow()
                .iter()
                .any(|signal| signal.0 == name)
        });
        let signals = self.api_signals();
        signals
            .into_iter()
            .rfind(|signal| signal.0 == name)
            .expect("it came")
            .1
    }
}

#[test]
fn the_api_names_the_apps_with_grouping_off_and_signals_a_change() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.notify(notify("Grouped"));
        daemon.notify(Notify {
            app_name: "Mail",
            ..notify("Mail")
        });
        assert!(daemon.ungrouped().is_empty());

        daemon.forget_signals();
        daemon.apps.app("Mail").set_boolean("group", false).unwrap();
        assert_eq!(
            daemon.wait_for_api_signal("UngroupedChanged"),
            "(['Mail'],)"
        );
        assert_eq!(daemon.ungrouped(), ["Mail"]);

        // An app without its own settings follows the default.
        daemon.forget_signals();
        daemon.settings.set_boolean("group", false).unwrap();
        assert_eq!(
            daemon.wait_for_api_signal("UngroupedChanged"),
            "(['Mail', 'Test'],)"
        );
    });
}

#[test]
fn a_grouped_bubble_merged_under_the_pointer_waits_for_it_to_leave() {
    bus::run(|bus| {
        let daemon = start(bus);
        let timed = |summary| Notify {
            timeout: 100,
            ..notify(summary)
        };
        daemon.notify(timed("1"));
        let hovered = daemon.notify(timed("2"));
        daemon.bubbles.events(hovered).hovered(true);
        let grouped = daemon.notify(timed("3"));

        // Three times its timeout.
        bus.run_for(Duration::from_millis(300));
        assert_eq!(daemon.bubbles.ids(), [grouped]);

        daemon.bubbles.events(grouped).hovered(false);
        daemon.wait_for_signals(3);
        assert!(daemon.bubbles.ids().is_empty());
    });
}

// Do Not Disturb (CMN-263), set over the private API, with the fake clock. The test's time,
// 1 000 000 s, is 13:46:40 on 12 January 1970 in UTC, the fake clock's zone.

/// The fake clock's start.
const NOW: u64 = 1_000_000;
const HOUR: u64 = 60 * 60;

#[test]
fn do_not_disturb_for_an_hour_is_on_until_the_hour_ends() {
    bus::run(|bus| {
        let daemon = start(bus);
        assert_eq!(daemon.do_not_disturb(), (false, 0, String::new()));

        daemon.set_do_not_disturb("one-hour");

        assert_eq!(
            daemon.do_not_disturb(),
            (true, NOW + HOUR, "manual".to_owned())
        );
        assert_eq!(
            daemon.api_signals(),
            [signal("DoNotDisturbChanged", "(true, 1003600, 'manual')")]
        );
        daemon.forget_signals();
        daemon.home.clock.0.set(NOW + HOUR - 1);
        assert!(daemon.do_not_disturb().0);
        daemon.home.clock.0.set(NOW + HOUR);
        assert_eq!(daemon.do_not_disturb(), (false, 0, String::new()));
        assert_eq!(
            daemon.api_signals(),
            [signal("DoNotDisturbChanged", "(false, 0, '')")]
        );
    });
}

#[test]
fn while_do_not_disturb_is_on_a_notification_has_no_bubble_and_no_sound_but_its_entry() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.set_do_not_disturb("one-hour");

        daemon.notify(notify("Quiet"));

        assert!(daemon.bubbles.ids().is_empty());
        assert!(daemon.sounds.0.borrow().is_empty());
        assert_eq!(daemon.summaries(), ["Quiet"]);
    });
}

#[test]
fn nothing_is_shown_when_do_not_disturb_ends() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.set_do_not_disturb("one-hour");
        daemon.notify(notify("Quiet"));

        daemon.home.clock.0.set(NOW + HOUR);
        assert!(!daemon.do_not_disturb().0);
        daemon.bus.run_for(QUIET);

        assert!(daemon.bubbles.ids().is_empty());
        assert!(daemon.sounds.0.borrow().is_empty());
        // The next one has its bubble again.
        let next = daemon.notify(notify("Loud"));
        assert_eq!(daemon.bubbles.ids(), [next]);
    });
}

#[test]
fn a_critical_notification_breaks_through_unless_the_setting_says_otherwise() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.set_do_not_disturb("until-turned-off");

        let critical = daemon.notify(notify("Battery low").hint("urgency", 2u8));
        assert_eq!(daemon.bubbles.ids(), [critical]);
        assert_eq!(daemon.sounds.0.borrow().len(), 1);

        daemon
            .settings
            .set_boolean("critical-breaks-through", false)
            .unwrap();
        daemon.notify(notify("Battery dying").hint("urgency", 2u8));
        assert_eq!(daemon.bubbles.ids(), [critical]);
        assert_eq!(daemon.sounds.0.borrow().len(), 1);
        assert_eq!(daemon.summaries(), ["Battery dying", "Battery low"]);
    });
}

#[test]
fn an_app_that_breaks_through_gets_its_bubble_and_its_sound() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon
            .apps
            .app("Phone")
            .set_boolean("breaks-through", true)
            .unwrap();
        daemon.set_do_not_disturb("until-turned-off");

        daemon.notify(notify("Mail"));
        let call = daemon.notify(Notify {
            app_name: "Phone",
            ..notify("Ana is calling")
        });

        assert_eq!(daemon.bubbles.ids(), [call]);
        assert_eq!(daemon.sounds.0.borrow().len(), 1);
    });
}

#[test]
fn do_not_disturb_ends_after_four_hours_tomorrow_morning_or_never() {
    bus::run(|bus| {
        let daemon = start(bus);
        let set = |until| {
            daemon.set_do_not_disturb(until);
            daemon.do_not_disturb()
        };
        assert_eq!(
            set("four-hours"),
            (true, NOW + 4 * HOUR, "manual".to_owned())
        );
        // 08:00 on 13 January 1970.
        assert_eq!(
            set("tomorrow-morning"),
            (true, 1_065_600, "manual".to_owned())
        );
        assert_eq!(set("until-turned-off"), (true, 0, "manual".to_owned()));
        daemon.home.clock.0.set(NOW + 100 * HOUR);
        assert!(daemon.do_not_disturb().0);
        assert_eq!(set("off"), (false, 0, String::new()));

        let refused = daemon.try_api("SetDoNotDisturb", ("a-while",).to_variant());
        assert!(refused.is_err());
        assert!(!daemon.do_not_disturb().0);
    });
}

#[test]
fn toggling_turns_do_not_disturb_off_or_on_until_turned_off() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.set_do_not_disturb("one-hour");

        daemon.api("ToggleDoNotDisturb", ().to_variant());
        assert_eq!(daemon.do_not_disturb(), (false, 0, String::new()));
        daemon.api("ToggleDoNotDisturb", ().to_variant());
        assert_eq!(daemon.do_not_disturb(), (true, 0, "manual".to_owned()));
    });
}

#[test]
fn do_not_disturb_survives_a_restart_of_the_daemon() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.set_do_not_disturb("one-hour");
        let home = daemon.home.clone();
        drop(daemon);

        let daemon = start_in(bus, home.clone());
        assert_eq!(
            daemon.do_not_disturb(),
            (true, NOW + HOUR, "manual".to_owned())
        );
        daemon.notify(notify("Quiet"));
        assert!(daemon.bubbles.ids().is_empty());
        drop(daemon);

        // One that ended while no daemon ran is off, and one until turned off stays on.
        home.clock.0.set(NOW + HOUR);
        let daemon = start_in(bus, home.clone());
        assert_eq!(daemon.do_not_disturb(), (false, 0, String::new()));
        daemon.set_do_not_disturb("until-turned-off");
        drop(daemon);
        home.clock.0.set(NOW + 100 * HOUR);
        let daemon = start_in(bus, home);
        assert_eq!(daemon.do_not_disturb(), (true, 0, "manual".to_owned()));
    });
}

#[test]
fn do_not_disturb_ends_when_its_time_comes_though_nobody_asks() {
    bus::run(|bus| {
        let home = Home::new();
        let daemon = start_in(bus, home.clone());
        daemon.set_do_not_disturb("one-hour");
        drop(daemon);
        // Started a second before the end, it waits a second.
        home.clock.0.set(NOW + HOUR - 1);
        let daemon = start_in(bus, home);
        daemon.forget_signals();

        daemon.home.clock.0.set(NOW + HOUR);
        daemon.wait_for_api_signals(1);

        assert_eq!(
            daemon.api_signals(),
            [signal("DoNotDisturbChanged", "(false, 0, '')")]
        );
    });
}

/// `day` January 1970 at `hour`:`minute`, in UTC, the fake clock's zone.
fn january(day: u64, hour: u64, minute: u64) -> u64 {
    (day - 1) * 24 * HOUR + hour * HOUR + minute * 60
}

impl Daemon<'_> {
    /// Gives Do Not Disturb its daily schedule, 22:00 to 07:00.
    fn schedule_nightly(&self) {
        self.settings
            .set_uint("do-not-disturb-from", 22 * 60)
            .unwrap();
        self.settings.set_uint("do-not-disturb-to", 7 * 60).unwrap();
        self.settings
            .set_boolean("do-not-disturb-schedule", true)
            .unwrap();
    }

    /// Waits for Do Not Disturb as the private API gives it to be `state`.
    fn wait_for_do_not_disturb(&self, state: (bool, u64, &str)) {
        let state = (state.0, state.1, state.2.to_owned());
        self.bus
            .wait_until(&format!("Do Not Disturb to be {state:?}"), || {
                self.do_not_disturb() == state
            });
    }
}

#[test]
fn the_schedule_turns_do_not_disturb_on_and_off_across_midnight() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.schedule_nightly();
        let clock = &daemon.home.clock.0;
        let morning = january(13, 7, 0);

        clock.set(january(12, 21, 59));
        assert_eq!(daemon.do_not_disturb(), (false, 0, String::new()));
        clock.set(january(12, 22, 0));
        assert_eq!(
            daemon.do_not_disturb(),
            (true, morning, "schedule".to_owned())
        );
        clock.set(january(13, 6, 59));
        assert!(daemon.do_not_disturb().0);
        daemon.notify(notify("Quiet"));
        assert!(daemon.bubbles.ids().is_empty());
        clock.set(morning);
        assert_eq!(daemon.do_not_disturb(), (false, 0, String::new()));
        let changes: Vec<_> = daemon
            .api_signals()
            .into_iter()
            .filter(|(name, _)| name == "DoNotDisturbChanged")
            .collect();
        assert_eq!(
            changes,
            [
                signal("DoNotDisturbChanged", "(true, 1062000, 'schedule')"),
                signal("DoNotDisturbChanged", "(false, 0, '')"),
            ]
        );
    });
}

#[test]
fn turning_the_schedule_on_during_it_is_signalled_without_a_call() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.home.clock.0.set(january(12, 23, 0));

        daemon.schedule_nightly();

        daemon.wait_for_api_signals(1);
        assert_eq!(
            daemon.api_signals(),
            [signal("DoNotDisturbChanged", "(true, 1062000, 'schedule')")]
        );
    });
}

#[test]
fn turning_it_off_during_the_schedule_holds_it_off_until_the_schedules_next_start() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.schedule_nightly();
        let clock = &daemon.home.clock.0;
        clock.set(january(12, 23, 0));
        assert!(daemon.do_not_disturb().0);

        daemon.set_do_not_disturb("off");

        assert_eq!(daemon.do_not_disturb(), (false, 0, String::new()));
        clock.set(january(13, 3, 0));
        assert!(!daemon.do_not_disturb().0);
        clock.set(january(13, 21, 59));
        assert!(!daemon.do_not_disturb().0);
        clock.set(january(13, 22, 0));
        assert_eq!(
            daemon.do_not_disturb(),
            (true, january(14, 7, 0), "schedule".to_owned())
        );
    });
}

#[test]
fn the_user_turning_it_on_during_the_schedule_is_the_reason_until_their_end() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon.schedule_nightly();
        let clock = &daemon.home.clock.0;
        clock.set(january(12, 23, 0));

        daemon.set_do_not_disturb("one-hour");

        assert_eq!(
            daemon.do_not_disturb(),
            (true, january(13, 0, 0), "manual".to_owned())
        );
        clock.set(january(13, 0, 0));
        assert_eq!(
            daemon.do_not_disturb(),
            (true, january(13, 7, 0), "schedule".to_owned())
        );
    });
}

#[test]
fn a_fullscreen_window_turns_do_not_disturb_on_while_its_setting_is_on() {
    bus::run(|bus| {
        let daemon = start(bus);
        let caminito = FakeCaminito::start(bus);

        caminito.set("PointerOutputFullscreen", true);
        daemon.wait_for_do_not_disturb((true, 0, "fullscreen"));
        daemon.notify(notify("Quiet"));
        assert!(daemon.bubbles.ids().is_empty());

        daemon
            .settings
            .set_boolean("do-not-disturb-while-fullscreen", false)
            .unwrap();
        daemon.wait_for_do_not_disturb((false, 0, ""));
        daemon
            .settings
            .set_boolean("do-not-disturb-while-fullscreen", true)
            .unwrap();
        daemon.wait_for_do_not_disturb((true, 0, "fullscreen"));
        caminito.set("PointerOutputFullscreen", false);
        daemon.wait_for_do_not_disturb((false, 0, ""));
    });
}

#[test]
fn a_screencast_turns_do_not_disturb_on_while_its_setting_is_on() {
    bus::run(|bus| {
        let daemon = start(bus);
        daemon
            .settings
            .set_boolean("do-not-disturb-while-screencasting", false)
            .unwrap();
        let caminito = FakeCaminito::start(bus);

        caminito.set("Screencasting", true);
        daemon.bus.run_for(QUIET);
        assert!(!daemon.do_not_disturb().0);

        daemon
            .settings
            .set_boolean("do-not-disturb-while-screencasting", true)
            .unwrap();
        daemon.wait_for_do_not_disturb((true, 0, "screencast"));
        // Caminito leaving the bus ends it.
        drop(caminito);
        daemon.wait_for_do_not_disturb((false, 0, ""));
    });
}

#[test]
fn a_caminito_already_fullscreen_as_the_daemon_starts_turns_it_on() {
    bus::run(|bus| {
        let caminito = FakeCaminito::start(bus);
        caminito.set("PointerOutputFullscreen", true);

        let daemon = start(bus);

        daemon.wait_for_do_not_disturb((true, 0, "fullscreen"));
    });
}

#[test]
fn turning_it_off_during_a_fullscreen_window_holds_it_off_until_the_window_leaves() {
    bus::run(|bus| {
        let daemon = start(bus);
        let caminito = FakeCaminito::start(bus);
        caminito.set("PointerOutputFullscreen", true);
        daemon.wait_for_do_not_disturb((true, 0, "fullscreen"));

        daemon.api("ToggleDoNotDisturb", ().to_variant());

        assert!(!daemon.do_not_disturb().0);
        caminito.set("Screencasting", true);
        daemon.wait_for_do_not_disturb((true, 0, "screencast"));
        caminito.set("Screencasting", false);
        caminito.set("PointerOutputFullscreen", false);
        daemon.wait_for_do_not_disturb((false, 0, ""));
        caminito.set("PointerOutputFullscreen", true);
        daemon.wait_for_do_not_disturb((true, 0, "fullscreen"));
    });
}
