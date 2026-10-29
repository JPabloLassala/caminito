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
    Sound, Sounds,
};
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
    let settings = settings::compiled(schemas.path());
    let connection = bus.connect();
    let bubbles = Rc::new(FakeBubbles::default());
    let sounds = Rc::new(FakeSounds::default());
    let server = Server::new(
        &connection,
        bubbles.clone(),
        sounds.clone(),
        settings::outside(&settings),
        Config {
            history_file: home.directory.path().join("caminito").join("history"),
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
        settings,
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
    bool,
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
    reply: bool,
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

#[test]
fn closing_a_bubble_or_clicking_one_without_a_default_action_dismisses_it() {
    bus::run(|bus| {
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
fn bubbles_show_as_many_as_fit_and_one_that_doesnt_fit_gets_none_later_either() {
    bus::run(|bus| {
        let daemon = start(bus);
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
fn a_critical_notification_takes_the_place_of_the_oldest_ordinary_bubble_or_gets_none() {
    bus::run(|bus| {
        let daemon = start(bus);
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
            actions: vec!["default", "Open", "reply", "Reply"],
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
                actions: vec![("reply".to_owned(), "Reply".to_owned())],
                has_default: true,
                resident: false,
                reply: true,
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

        // There's no default action here, and no such action.
        assert!(
            daemon
                .try_api("InvokeAction", (id, "default", "").to_variant())
                .is_err()
        );
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
fn the_api_replies_to_an_entry_with_a_reply_field_and_refuses_one_without() {
    bus::run(|bus| {
        let daemon = start(bus);
        let with = daemon.notify(Notify {
            timeout: 0,
            ..notify("Chat").hint("x-kde-reply-placeholder-text", "Reply")
        });
        let without = daemon.notify(Notify {
            timeout: 0,
            ..notify("Plain")
        });
        let resident = daemon.notify(Notify {
            timeout: 0,
            ..notify("Resident")
                .hint("x-kde-reply-placeholder-text", "Reply")
                .hint("resident", true)
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
