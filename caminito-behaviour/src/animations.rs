//! The Animations page's keys (ADR 0055): `org.mate.Caminito`'s Popup animation and its
//! switches for the other animations, which Caminito follows as they change, and the three
//! keys of MATE's that turn every animation off, which the page only reads.

use gio::prelude::*;

use crate::SCHEMA;

#[cfg(test)]
mod tests;

/// The Popup animation's key.
const POPUP_ANIMATION: &str = "popup-animation";

/// How a popup comes on screen and leaves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupAnimation {
    /// The Popup slide.
    Slide,
    /// The Popup fade.
    Fade,
    /// There and gone at once.
    None,
}

impl PopupAnimation {
    /// The three, in the drop-down's order.
    pub const ALL: [PopupAnimation; 3] = [
        PopupAnimation::Slide,
        PopupAnimation::Fade,
        PopupAnimation::None,
    ];

    /// What the user reads for it.
    pub fn label(self) -> &'static str {
        match self {
            PopupAnimation::Slide => "Slide",
            PopupAnimation::Fade => "Fade",
            PopupAnimation::None => "None",
        }
    }

    /// Its nick in `org.mate.Caminito.PopupAnimation`.
    pub fn nick(self) -> &'static str {
        match self {
            PopupAnimation::Slide => "slide",
            PopupAnimation::Fade => "fade",
            PopupAnimation::None => "none",
        }
    }

    /// The animation of `nick`, if it's one.
    fn from_nick(nick: &str) -> Option<Self> {
        PopupAnimation::ALL
            .into_iter()
            .find(|animation| animation.nick() == nick)
    }
}

/// An animation with a switch of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Switch {
    /// The Workspace slide.
    WorkspaceSlide,
    /// The Overview's transition.
    Overview,
    /// The Opening and Closing animations, which share a key.
    OpenClose,
    /// The Maximise animation.
    Maximise,
}

impl Switch {
    /// The four, in the page's order.
    pub const ALL: [Switch; 4] = [
        Switch::WorkspaceSlide,
        Switch::Overview,
        Switch::OpenClose,
        Switch::Maximise,
    ];

    /// Its key.
    pub fn key(self) -> &'static str {
        match self {
            Switch::WorkspaceSlide => "workspace-slide",
            Switch::Overview => "overview-animation",
            Switch::OpenClose => "open-close-animation",
            Switch::Maximise => "maximize-animation",
        }
    }

    /// What the user reads for it.
    pub fn label(self) -> &'static str {
        match self {
            Switch::WorkspaceSlide => "Workspace slide",
            Switch::Overview => "Overview transition",
            Switch::OpenClose => "Opening and closing windows",
            Switch::Maximise => "Maximising windows",
        }
    }
}

/// A key of MATE's that turns every animation off, whatever the page's keys say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocker {
    /// `org.mate.Marco.general compositing-manager` is off.
    Compositing,
    /// `org.mate.Marco.general reduced-resources` is on.
    ReducedResources,
    /// `org.mate.interface enable-animations` is off.
    Interface,
}

impl Blocker {
    /// Why the page's controls are greyed out.
    pub fn reason(self) -> &'static str {
        match self {
            Blocker::Compositing => "Animations are off because compositing is turned off.",
            Blocker::ReducedResources => {
                "Animations are off because reduced resources is turned on."
            }
            Blocker::Interface => "Animations are off in MATE's interface settings.",
        }
    }
}

/// Why the keys can't be opened, or one can't be set.
#[derive(Debug)]
pub enum Error {
    /// `org.mate.Caminito` isn't installed, or isn't compiled.
    NoSchema,
    /// Its key is locked down, or the backend failed.
    Key(&'static str, gio::glib::BoolError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoSchema => write!(f, "the schema {SCHEMA} isn't installed"),
            Error::Key(key, err) => write!(f, "can't write {key}: {err}"),
        }
    }
}

impl std::error::Error for Error {}

/// The animations' keys, and MATE's three, whose schemas may be missing: Caminito takes such
/// a key's default then, which turns nothing off.
pub struct Animations {
    settings: gio::Settings,
    marco: Option<gio::Settings>,
    interface: Option<gio::Settings>,
}

impl Animations {
    /// The animations of `settings`, which are `org.mate.Caminito`'s, with
    /// `org.mate.Marco.general` and `org.mate.interface` if they're there.
    pub fn new(
        settings: gio::Settings,
        marco: Option<gio::Settings>,
        interface: Option<gio::Settings>,
    ) -> Self {
        Animations {
            settings,
            marco,
            interface,
        }
    }

    /// Opens the installed schemas.
    pub fn open() -> Result<Self, Error> {
        let settings = crate::installed().ok_or(Error::NoSchema)?;
        Ok(Animations::new(
            settings,
            crate::installed_schema("org.mate.Marco.general"),
            crate::installed_schema("org.mate.interface"),
        ))
    }

    /// The Popup animation.
    pub fn popup_animation(&self) -> PopupAnimation {
        read_popup_animation(&self.settings)
    }

    /// Writes the Popup animation at once: there's no Apply.
    pub fn set_popup_animation(&self, animation: PopupAnimation) -> Result<(), Error> {
        self.settings
            .set_string(POPUP_ANIMATION, animation.nick())
            .map_err(|err| Error::Key(POPUP_ANIMATION, err))
    }

    /// Calls `f` with the Popup animation each time its key changes.
    pub fn connect_popup_animation_changed(&self, f: impl Fn(PopupAnimation) + 'static) {
        self.settings
            .connect_changed(Some(POPUP_ANIMATION), move |settings, _| {
                f(read_popup_animation(settings));
            });
    }

    /// Whether `switch` is on.
    pub fn is_on(&self, switch: Switch) -> bool {
        self.settings.boolean(switch.key())
    }

    /// Writes `switch` at once.
    pub fn set(&self, switch: Switch, on: bool) -> Result<(), Error> {
        self.settings
            .set_boolean(switch.key(), on)
            .map_err(|err| Error::Key(switch.key(), err))
    }

    /// Calls `f` with a switch and whether it's on each time its key changes.
    pub fn connect_changed(&self, f: impl Fn(Switch, bool) + 'static) {
        self.settings.connect_changed(None, move |settings, key| {
            let Some(switch) = Switch::ALL.into_iter().find(|switch| switch.key() == key) else {
                return;
            };
            f(switch, settings.boolean(switch.key()));
        });
    }

    /// The first of MATE's keys that turns every animation off, if one does.
    pub fn blocker(&self) -> Option<Blocker> {
        blocker(self.marco.as_ref(), self.interface.as_ref())
    }

    /// Calls `f` with what [`blocker`](Self::blocker) says each time one of MATE's keys
    /// changes.
    pub fn connect_blocker_changed(&self, f: impl Fn(Option<Blocker>) + 'static) {
        let f = std::rc::Rc::new(f);
        let (marco, interface) = (self.marco.clone(), self.interface.clone());
        let listen = |settings: &gio::Settings, key: &'static str| {
            let (f, marco, interface) = (f.clone(), marco.clone(), interface.clone());
            settings.connect_changed(Some(key), move |_, _| {
                f(blocker(marco.as_ref(), interface.as_ref()));
            });
        };
        if let Some(settings) = &self.marco {
            listen(settings, "compositing-manager");
            listen(settings, "reduced-resources");
        }
        if let Some(settings) = &self.interface {
            listen(settings, "enable-animations");
        }
    }
}

/// The Popup animation of `settings`.
fn read_popup_animation(settings: &gio::Settings) -> PopupAnimation {
    let nick = settings.string(POPUP_ANIMATION);
    PopupAnimation::from_nick(&nick).expect("the schema's enum has the three animations")
}

/// The first of the keys that turns every animation off, if one does, in the order the
/// compositor reads them.
fn blocker(marco: Option<&gio::Settings>, interface: Option<&gio::Settings>) -> Option<Blocker> {
    if let Some(marco) = marco {
        if !marco.boolean("compositing-manager") {
            return Some(Blocker::Compositing);
        }
        if marco.boolean("reduced-resources") {
            return Some(Blocker::ReducedResources);
        }
    }
    let off = interface.is_some_and(|interface| !interface.boolean("enable-animations"));
    off.then_some(Blocker::Interface)
}
