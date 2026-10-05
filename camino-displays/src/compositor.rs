//! Camino, as the window sees it: what it shows over output management, and the primary
//! output and the saving of layouts on `org.mate.Camino` (81, 83). Both come to the
//! window on GLib's main loop, and nothing here blocks it but the calls that say so.
//!
//! The demo stands in for all of it with made-up monitors.

use std::cell::RefCell;
use std::rc::Rc;

use camino_displays::outputs::{self, Label, Outcome, Outputs};
use camino_displays::page::{Mode, Monitor, Scale, Shows, Wanted};
use gtk::gio::prelude::*;
use gtk::{gio, glib};

/// Camino's name on the session bus, its object and its interface (ADR 0002).
const NAME: &str = "org.mate.Camino";
const PATH: &str = "/org/mate/Camino";
const PRIMARY_OUTPUT: &str = "PrimaryOutput";

/// How long a blocking call to Camino may take, in milliseconds. Camino answers from its
/// event loop, within a frame.
const NOW_TIMEOUT: i32 = 1000;

/// Why Camino didn't do what the window asked.
#[derive(Debug)]
pub(crate) enum Error {
    /// The connection to the compositor couldn't carry it.
    Outputs(outputs::Error),
    /// The connection to the compositor is gone.
    Gone,
    /// `org.mate.Camino` isn't on the session bus.
    NoCamino,
    /// Camino refused, or the bus failed, in the sender's words.
    Refused(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Outputs(err) => err.fmt(f),
            Error::Gone => write!(f, "the connection to the compositor is gone"),
            Error::NoCamino => write!(f, "Camino isn't on the session bus"),
            Error::Refused(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for Error {}

/// What the window hears of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Event {
    /// What Camino shows changed, or which output is the primary one.
    Changed,
    /// The answer to the configuration `apply` sent.
    Applied(Outcome),
    /// The connection to the compositor is gone, and why.
    Lost(String),
}

/// Camino, or the demo's stand-in.
#[derive(Clone)]
pub(crate) struct Compositor {
    inner: Rc<Inner>,
}

/// What hears of the events, on the main loop.
type Listener = Rc<dyn Fn(Event)>;

struct Inner {
    source: RefCell<Source>,
    /// `org.mate.Camino`. Its properties are cached, and kept up to date by its signal.
    camino: Option<gio::DBusProxy>,
    listener: RefCell<Option<Listener>>,
}

enum Source {
    Camino(Box<Outputs>),
    Demo {
        monitors: Vec<Monitor>,
        primary: String,
    },
    Lost,
}

impl Compositor {
    /// Connects to the compositor and to the session bus. Without `org.mate.Camino`, as
    /// under another compositor with output management, there's no primary display and
    /// nothing is saved.
    pub(crate) fn connect() -> Result<Self, outputs::Error> {
        let outputs = Outputs::connect()?;
        let camino = gio::DBusProxy::for_bus_sync(
            gio::BusType::Session,
            gio::DBusProxyFlags::DO_NOT_AUTO_START,
            None,
            NAME,
            PATH,
            NAME,
            gio::Cancellable::NONE,
        )
        .map_err(|err| glib::g_warning!("camino-displays", "can't reach the session bus: {err}"))
        .ok();
        Ok(Self::new(Source::Camino(Box::new(outputs)), camino))
    }

    /// The prototype's three monitors of mixed scales, which nothing outside the window
    /// shows.
    pub(crate) fn demo() -> Self {
        let modes = |sizes: &[(i32, i32, &[i32])]| {
            let mut modes = Vec::new();
            for &(width, height, rates) in sizes {
                for &refresh in rates {
                    modes.push(Mode {
                        width,
                        height,
                        refresh,
                        preferred: modes.is_empty(),
                    });
                }
            }
            modes
        };
        let monitor = |name: &str, model: &str, x: i32, scale: f64, modes: Vec<Mode>| Monitor {
            name: name.to_owned(),
            make: None,
            model: Some(model.to_owned()),
            modes,
            shows: Some(Shows {
                mode: 0,
                x,
                y: 0,
                scale: Scale::from_factor(scale),
            }),
        };
        let laptop = modes(&[
            (1920, 1200, &[60_000]),
            (1680, 1050, &[60_000]),
            (1280, 800, &[60_000]),
        ]);
        let dell = modes(&[
            (3840, 2160, &[60_000, 30_000]),
            (2560, 1440, &[60_000]),
            (1920, 1080, &[60_000, 59_940, 50_000]),
        ]);
        let lg = modes(&[
            (1920, 1080, &[144_000, 120_000, 60_000, 59_940]),
            (1280, 720, &[60_000]),
        ]);
        let monitors = vec![
            monitor("eDP-1", "Panel", 0, 1.35, laptop),
            monitor("DP-1", "Dell U2723QE", 1422, 1.5, dell),
            monitor("HDMI-A-1", "LG 24GN600", 3982, 1.0, lg),
        ];
        let primary = "eDP-1".to_owned();
        Self::new(Source::Demo { monitors, primary }, None)
    }

    fn new(source: Source, camino: Option<gio::DBusProxy>) -> Self {
        let inner = Inner {
            source: RefCell::new(source),
            camino,
            listener: RefCell::new(None),
        };
        Compositor {
            inner: Rc::new(inner),
        }
    }

    /// Has `listener` hear of every event from now on, on the main loop.
    pub(crate) fn listen(&self, listener: impl Fn(Event) + 'static) {
        *self.inner.listener.borrow_mut() = Some(Rc::new(listener));

        // GLib watches a copy of the connection's descriptor, as a socket of its own.
        let watched = match &*self.inner.source.borrow() {
            Source::Camino(outputs) => {
                let copy = outputs.fd().try_clone_to_owned();
                let socket = |fd| gio::Socket::from_fd(fd).map_err(|err| err.to_string());
                Some(copy.map_err(|err| err.to_string()).and_then(socket))
            }
            _ => None,
        };
        match watched {
            Some(Ok(socket)) => {
                let compositor = self.clone();
                let source = gio::prelude::SocketExtManual::create_source(
                    &socket,
                    glib::IOCondition::IN | glib::IOCondition::HUP,
                    gio::Cancellable::NONE,
                    Some("camino-displays outputs"),
                    glib::Priority::DEFAULT,
                    move |_, _| compositor.read(),
                );
                source.attach(None);
            }
            Some(Err(err)) => self.lose(format!("can't watch the compositor's connection: {err}")),
            None => {}
        }
        if let Some(camino) = &self.inner.camino {
            let compositor = self.clone();
            camino.connect_local("g-properties-changed", false, move |_| {
                compositor.emit(Event::Changed);
                None
            });
            // Camino starting or going: its properties come and go with it.
            let compositor = self.clone();
            camino.connect_notify_local(Some("g-name-owner"), move |_, _| {
                compositor.emit(Event::Changed);
            });
        }
    }

    fn emit(&self, event: Event) {
        let listener = self.inner.listener.borrow().clone();
        if let Some(listener) = listener {
            listener(event);
        }
    }

    /// Tells the listener at the main loop's next turn, as a real answer would come.
    fn emit_later(&self, events: Vec<Event>) {
        let compositor = self.clone();
        glib::idle_add_local_once(move || events.into_iter().for_each(|e| compositor.emit(e)));
    }

    fn lose(&self, why: String) {
        *self.inner.source.borrow_mut() = Source::Lost;
        self.emit(Event::Lost(why));
    }

    /// The connection is readable.
    fn read(&self) -> glib::ControlFlow {
        let read = match &mut *self.inner.source.borrow_mut() {
            Source::Camino(outputs) => outputs.dispatch(),
            _ => return glib::ControlFlow::Break,
        };
        if self.hear(read) {
            glib::ControlFlow::Continue
        } else {
            glib::ControlFlow::Break
        }
    }

    /// Tells the listener what the connection read, or that it's gone. Returns whether
    /// it's still there.
    fn hear(&self, read: Result<Vec<outputs::Event>, outputs::Error>) -> bool {
        match read {
            Ok(events) => {
                for event in events {
                    self.emit(match event {
                        outputs::Event::Changed => Event::Changed,
                        outputs::Event::Applied(outcome) => Event::Applied(outcome),
                    });
                }
                true
            }
            Err(err) => {
                self.lose(err.to_string());
                false
            }
        }
    }

    /// What Camino shows of each connected monitor.
    pub(crate) fn monitors(&self) -> Vec<Monitor> {
        match &*self.inner.source.borrow() {
            Source::Camino(outputs) => outputs.monitors(),
            Source::Demo { monitors, .. } => monitors.clone(),
            Source::Lost => Vec::new(),
        }
    }

    /// The primary output's name, if Camino is there to have one.
    pub(crate) fn primary(&self) -> Option<String> {
        if let Source::Demo { primary, .. } = &*self.inner.source.borrow() {
            return Some(primary.clone());
        }
        let property = self
            .inner
            .camino
            .as_ref()?
            .cached_property(PRIMARY_OUTPUT)?;
        property.get::<String>().filter(|name| !name.is_empty())
    }

    /// Sends a configuration. The answer comes as `Event::Applied`.
    pub(crate) fn apply(&self, wanted: &[Wanted]) -> Result<(), Error> {
        match &mut *self.inner.source.borrow_mut() {
            Source::Camino(outputs) => outputs.apply(wanted).map_err(Error::Outputs)?,
            Source::Demo { monitors, .. } => {
                for monitor in monitors.iter_mut() {
                    let wanted = wanted.iter().find(|wanted| wanted.name == monitor.name);
                    let had = monitor.shows.map(|shows| shows.scale);
                    monitor.shows = wanted.and_then(|wanted| wanted.shows).map(|shows| Shows {
                        mode: monitor
                            .modes
                            .iter()
                            .position(|mode| *mode == shows.mode)
                            .unwrap_or(0),
                        x: shows.x,
                        y: shows.y,
                        scale: shows.scale.or(had).unwrap_or(Scale::ONE),
                    });
                }
            }
            Source::Lost => return Err(Error::Gone),
        }
        if matches!(&*self.inner.source.borrow(), Source::Demo { .. }) {
            self.emit_later(vec![Event::Applied(Outcome::Succeeded), Event::Changed]);
        }
        Ok(())
    }

    /// What writes `PrimaryOutput`: the standard `Properties.Set`.
    fn primary_call(name: &str) -> glib::Variant {
        (NAME, PRIMARY_OUTPUT, name.to_variant()).to_variant()
    }

    /// Chooses the primary output, by the name of an enabled output, and tells `then`
    /// how it went. The change itself comes as `Event::Changed`.
    pub(crate) fn choose_primary(
        &self,
        name: &str,
        then: impl FnOnce(Result<(), Error>) + 'static,
    ) {
        if let Source::Demo { primary, .. } = &mut *self.inner.source.borrow_mut() {
            *primary = name.to_owned();
            let compositor = self.clone();
            glib::idle_add_local_once(move || {
                compositor.emit(Event::Changed);
                then(Ok(()));
            });
            return;
        }
        let Some(camino) = &self.inner.camino else {
            return then(Err(Error::NoCamino));
        };
        let call = camino.connection().call_future(
            Some(NAME),
            PATH,
            "org.freedesktop.DBus.Properties",
            "Set",
            Some(&Self::primary_call(name)),
            None,
            gio::DBusCallFlags::NONE,
            -1,
        );
        glib::spawn_future_local(async move {
            then(call.await.map(drop).map_err(|err| refused(&err)));
        });
    }

    /// Chooses the primary output and waits for Camino to have done it, for when the
    /// window is closing and there's no later.
    pub(crate) fn choose_primary_now(&self, name: &str) {
        let Some(camino) = &self.inner.camino else {
            return;
        };
        let called = camino.connection().call_sync(
            Some(NAME),
            PATH,
            "org.freedesktop.DBus.Properties",
            "Set",
            Some(&Self::primary_call(name)),
            None,
            gio::DBusCallFlags::NONE,
            NOW_TIMEOUT,
            gio::Cancellable::NONE,
        );
        if let Err(err) = called {
            glib::g_warning!(
                "camino-displays",
                "can't put the primary display back: {err}"
            );
        }
    }

    /// Asks Camino to save the layout it shows, for the monitors connected (83), and
    /// tells `then` how it went.
    pub(crate) fn save_layout(&self, then: impl FnOnce(Result<(), Error>) + 'static) {
        if matches!(&*self.inner.source.borrow(), Source::Demo { .. }) {
            return then(Ok(()));
        }
        let Some(camino) = &self.inner.camino else {
            return then(Err(Error::NoCamino));
        };
        let call = camino.call_future("SaveLayout", None, gio::DBusCallFlags::NONE, -1);
        glib::spawn_future_local(async move {
            then(call.await.map(drop).map_err(|err| refused(&err)));
        });
    }

    /// Waits until the compositor has handled what was sent, and tells the listener what
    /// it said meanwhile. For when the window is closing.
    pub(crate) fn settle(&self) {
        let said = match &mut *self.inner.source.borrow_mut() {
            Source::Camino(outputs) => outputs.roundtrip(),
            _ => return,
        };
        self.hear(said);
    }

    /// Whether the labels of Identify can be shown.
    pub(crate) fn can_identify(&self) -> bool {
        match &*self.inner.source.borrow() {
            Source::Camino(outputs) => outputs.can_identify(),
            _ => false,
        }
    }

    /// Shows `labels` on their outputs until `end_identify`.
    pub(crate) fn identify(&self, labels: &[Label]) {
        self.with_outputs(|outputs| outputs.identify(labels));
    }

    /// Takes the labels away.
    pub(crate) fn end_identify(&self) {
        self.with_outputs(Outputs::end_identify);
    }

    fn with_outputs(&self, ask: impl FnOnce(&mut Outputs) -> Result<(), outputs::Error>) {
        let asked = match &mut *self.inner.source.borrow_mut() {
            Source::Camino(outputs) => ask(outputs),
            _ => return,
        };
        if let Err(err) = asked {
            self.lose(err.to_string());
        }
    }
}

/// A D-Bus error as its sender worded it, without GDBus's prefix for a remote one.
fn refused(err: &glib::Error) -> Error {
    let mut err = err.clone();
    gio::DBusError::strip_remote_error(&mut err);
    Error::Refused(err.message().to_owned())
}
