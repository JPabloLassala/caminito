//! The Desktop Notifications server on a private bus (CMN-258): the test calls it as an app
//! would, over its own connection, and reads the signals the app would get. The bubbles and
//! the sounds are fakes, which record what the server asked of them, and act on a bubble as
//! the user would: hovering it, clicking it, using an action or closing it.

mod bus;
mod settings;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use caminito_notifications::server::{
    Action, Bubble, Bubbles, Events, Icon, Image, Server, Sound, Sounds,
};
use gio::glib::{self, Variant, VariantDict};
use gio::prelude::*;

use bus::Bus;

const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

/// The spec's reasons a notification was closed.
const EXPIRED: u32 = 1;
const DISMISSED: u32 = 2;
const CLOSED: u32 = 3;

/// The bubbles on screen, oldest first, and what the user can do to each.
#[derive(Default)]
struct FakeBubbles {
    shown: RefCell<Vec<(u32, Bubble)>>,
    events: RefCell<HashMap<u32, Events>>,
    /// How many times each bubble was shown, updated or closed, in order.
    calls: RefCell<Vec<String>>,
}

impl Bubbles for FakeBubbles {
    fn show(&self, id: u32, bubble: &Bubble, events: Events) {
        self.shown.borrow_mut().push((id, bubble.clone()));
        self.events.borrow_mut().insert(id, events);
        self.calls.borrow_mut().push(format!("show {id}"));
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
}

#[derive(Default)]
struct FakeSounds(RefCell<Vec<Sound>>);

impl Sounds for FakeSounds {
    fn play(&self, sound: &Sound) {
        self.0.borrow_mut().push(sound.clone());
    }
}

/// The server on the bus, and an app's connection to it.
struct Daemon<'a> {
    bus: &'a Bus,
    _server: Server,
    /// The server's settings, as `gsettings` would change them.
    settings: gio::Settings,
    _schemas: tempfile::TempDir,
    bubbles: Rc<FakeBubbles>,
    sounds: Rc<FakeSounds>,
    app: gio::DBusConnection,
    _subscription: gio::SignalSubscription,
    /// Each signal the app got, by its name, with its arguments.
    signals: Rc<RefCell<Vec<(String, Variant)>>>,
}

fn start(bus: &Bus) -> Daemon<'_> {
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
    )
    .expect("the server registers");
    bus.wait_for_owner(NAME, &connection);
    let app = bus.connect();
    let signals: Rc<RefCell<Vec<(String, Variant)>>> = Rc::default();
    let subscription = app.subscribe_to_signal(
        None,
        Some(NAME),
        None,
        Some(PATH),
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
    Daemon {
        bus,
        _server: server,
        settings,
        _schemas: schemas,
        bubbles,
        sounds,
        app,
        _subscription: subscription,
        signals,
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
        let answer: Rc<RefCell<Option<Result<Variant, glib::Error>>>> = Rc::default();
        self.app.call(
            Some(NAME),
            PATH,
            NAME,
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
        let answer = answer.take().expect("it answered");
        answer.unwrap_or_else(|err| panic!("{method} answers: {err}"))
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
fn three_bubbles_show_at_once_and_the_rest_wait_for_a_place() {
    bus::run(|bus| {
        let daemon = start(bus);
        let ids: Vec<u32> = ["1", "2", "3", "4", "5"]
            .into_iter()
            .map(|summary| daemon.notify(notify(summary)))
            .collect();
        assert_eq!(daemon.bubbles.ids(), ids[..3]);

        // One that waits is replaced where it waits, and closed without ever showing.
        daemon.notify(Notify {
            replaces: ids[3],
            ..notify("4 again")
        });
        daemon.close(ids[4]);
        daemon.wait_for_signals(1);
        assert_eq!(daemon.bubbles.ids(), ids[..3]);

        daemon.bubbles.events(ids[1]).dismiss();
        assert_eq!(daemon.bubbles.ids(), [ids[0], ids[2], ids[3]]);
        assert_eq!(daemon.bubbles.bubble(ids[3]).summary, "4 again");
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
fn a_waiting_notification_follows_the_settings_as_its_bubble_opens() {
    bus::run(|bus| {
        let daemon = start(bus);
        let ids: Vec<u32> = ["1", "2", "3", "4"]
            .into_iter()
            .map(|summary| daemon.notify(notify(summary)))
            .collect();

        daemon.settings.set_boolean("previews", false).unwrap();
        daemon.bubbles.events(ids[0]).dismiss();

        assert_eq!(daemon.bubbles.bubble(ids[1]).summary, "2");
        assert_eq!(daemon.bubbles.bubble(ids[3]).summary, "Notification");
    });
}
