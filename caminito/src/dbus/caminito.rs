//! `org.mate.Caminito` (ADR 0002's third channel): what MATE's components and
//! caminito-displays ask of Caminito that no protocol carries. So far, the primary output (81)
//! and saving the layout (83), both ADR 0014's, showing the desktop (94), the accelerator
//! grabs (98), the window a window screenshot is of (104), the global menu (110), the
//! menu claim (115), Night Light (137), and the sun's position (215).

use std::collections::HashMap;

use futures_channel::oneshot;
use smithay::reexports::calloop::channel::Sender;
use zbus::message::Header;
use zbus::names::BusName;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Str};

use super::global_menu::{GlobalMenu, Published};
use super::{Callers, caller};
use crate::night_light;
use crate::output::{primary, saved};
use crate::state::Caminito;

/// The well-known name, which carried patches look for an owner of (ADR 0011).
pub(crate) const NAME: &str = "org.mate.Caminito";
/// The object the interface is served at.
pub(crate) const PATH: &str = "/org/mate/Caminito";

/// A call, for the event loop to answer.
pub(crate) enum Request {
    /// Reads `PrimaryOutput`.
    PrimaryOutput(oneshot::Sender<String>),
    /// Writes `PrimaryOutput`, with the name written.
    ChoosePrimaryOutput(String, oneshot::Sender<Result<(), primary::Error>>),
    /// Calls `SaveLayout`.
    SaveLayout(oneshot::Sender<Result<(), saved::Error>>),
    /// Reads `ShowingDesktop`.
    ShowingDesktop(oneshot::Sender<bool>),
    /// Writes `ShowingDesktop`, with the value written.
    SetShowingDesktop(bool, oneshot::Sender<()>),
    /// Reads `GlobalMenu`.
    GlobalMenu(oneshot::Sender<GlobalMenu>),
    /// Reads `GlobalMenuOffered`.
    GlobalMenuOffered(oneshot::Sender<bool>),
    /// `ClaimGlobalMenu`, from `caller`, with its process if the bus knows it.
    ClaimGlobalMenu {
        caller: String,
        path: OwnedObjectPath,
        process: Option<u32>,
        reply: oneshot::Sender<()>,
    },
    /// Calls `GetActiveToplevel`, with the caller's process if the bus knows it.
    ActiveToplevel(Option<u32>, oneshot::Sender<Option<String>>),
    /// `GrabAccelerators`, from `caller`, answered with an action id for each.
    GrabAccelerators {
        caller: String,
        accelerators: Vec<String>,
        reply: oneshot::Sender<Vec<u32>>,
    },
    /// `UngrabAccelerators`, from `caller`.
    UngrabAccelerators {
        caller: String,
        actions: Vec<u32>,
        reply: oneshot::Sender<()>,
    },
    /// Reads `NightLight`.
    NightLight(oneshot::Sender<night_light::Published>),
    /// Calls `ToggleNightLight`.
    ToggleNightLight(oneshot::Sender<()>),
    /// Reads `Sun`.
    Sun(oneshot::Sender<night_light::PublishedSun>),
}

/// A property Caminito signals itself, when it changes without a caller writing it, in the
/// order they're signalled.
#[derive(Debug, Clone, Copy)]
enum Property {
    /// The stand-in went in, or the choice returned (81).
    PrimaryOutput,
    /// The key changed it, a window mapped or activated ended it, or the active workspace
    /// is another, which shows its desktop or doesn't for itself (94).
    ShowingDesktop,
    /// Night Light's target or its schedule's next sunrise (137).
    NightLight,
    /// The sun moved, which it does at each minute's working-out, or its location did
    /// (215).
    Sun,
    /// The applet came, or the grace period after it ended (115).
    GlobalMenuOffered,
    /// The focus is another window's, or the focused window said where its menus are
    /// (110).
    GlobalMenu,
}

impl Property {
    /// Each property, in the enum's order, which `Heard` indexes by.
    const ALL: [Self; 6] = [
        Self::PrimaryOutput,
        Self::ShowingDesktop,
        Self::NightLight,
        Self::Sun,
        Self::GlobalMenuOffered,
        Self::GlobalMenu,
    ];
    /// That `ALL` is in the enum's order, checked as `Heard` is first made, so that each
    /// property's value is at its own index.
    const ALL_IN_ORDER: () = {
        let mut index = 0;
        while index < Self::ALL.len() {
            assert!(Self::ALL[index] as usize == index);
            index += 1;
        }
    };

    /// The property as it is now. The global menu and Night Light are compared as the
    /// event loop has them, before they're made into what goes on the bus.
    fn read(self, caminito: &Caminito) -> Value {
        match self {
            Self::PrimaryOutput => Value::PrimaryOutput(caminito.primary_output_name()),
            Self::ShowingDesktop => Value::ShowingDesktop(caminito.policy.is_showing_desktop()),
            Self::NightLight => Value::NightLight(caminito.night_light.published()),
            Self::Sun => Value::Sun(caminito.night_light.published_sun()),
            Self::GlobalMenuOffered => Value::GlobalMenuOffered(caminito.global_menu_offered),
            Self::GlobalMenu => Value::GlobalMenu(caminito.global_menu()),
        }
    }
}

/// A property's value, of the one [`Property`] of the same name.
#[derive(PartialEq)]
enum Value {
    PrimaryOutput(String),
    ShowingDesktop(bool),
    NightLight(night_light::Published),
    Sun(night_light::PublishedSun),
    GlobalMenuOffered(bool),
    GlobalMenu(GlobalMenu),
}

/// The properties as `org.mate.Caminito`'s callers last heard them: a value for each, by
/// [`Property::ALL`]'s order. They're read when the interface is served, since a caller
/// reads them as they are then, so nothing is signalled until one changes (188).
pub(super) struct Heard([Value; Property::ALL.len()]);

impl Heard {
    /// The properties as they are now.
    pub(super) fn now(caminito: &Caminito) -> Self {
        let () = Property::ALL_IN_ORDER;
        Self(Property::ALL.map(|property| property.read(caminito)))
    }

    /// Records that the callers heard `property` as `value`. Returns whether that's news to
    /// them.
    fn record(&mut self, property: Property, value: Value) -> bool {
        let heard = &mut self.0[property as usize];
        if *heard == value {
            return false;
        }
        *heard = value;
        true
    }
}

/// `org.mate.Caminito`, as zbus serves it.
pub(crate) struct CaminitoInterface {
    requests: Sender<Request>,
    callers: Callers,
}

impl CaminitoInterface {
    pub(super) fn new(requests: Sender<Request>, callers: Callers) -> Self {
        Self { requests, callers }
    }
}

#[zbus::interface(name = "org.mate.Caminito")]
impl CaminitoInterface {
    /// The primary output's name, as `wl_output`, output management and RandR have it: the
    /// user's choice, or its stand-in while that monitor is unplugged or disabled. It's
    /// the empty string when there are no outputs.
    #[zbus(property)]
    async fn primary_output(&self) -> zbus::fdo::Result<String> {
        super::ask(&self.requests, Request::PrimaryOutput).await
    }

    /// Chooses the primary output, by the name of an enabled output. The choice is of
    /// its monitor, and returns with it after a stand-in.
    #[zbus(property)]
    async fn set_primary_output(&self, name: String) -> zbus::fdo::Result<()> {
        let choose = |reply| Request::ChoosePrimaryOutput(name, reply);
        super::ask(&self.requests, choose)
            .await?
            .map_err(|err| zbus::fdo::Error::InvalidArgs(err.to_string()))
    }

    /// Saves the layout as it is now, for the monitors connected now: which are enabled,
    /// their modes, places and scales, and the primary output. Caminito restores it when it
    /// next has those monitors, at start-up and as they're plugged in. Nothing else saves
    /// one: a configuration applied over output management lasts for the Session.
    async fn save_layout(&self) -> zbus::fdo::Result<()> {
        super::ask(&self.requests, Request::SaveLayout)
            .await?
            .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))
    }

    /// Whether the active workspace shows its desktop, with Marco's meaning (94): its
    /// windows are hidden, not minimised, until this is written false or one of them is
    /// mapped or activated. It follows the active workspace, and Ctrl+Alt+D changes it too.
    #[zbus(property)]
    async fn showing_desktop(&self) -> zbus::fdo::Result<bool> {
        super::ask(&self.requests, Request::ShowingDesktop).await
    }

    /// Shows the desktop on the active workspace, or brings its windows back.
    #[zbus(property)]
    async fn set_showing_desktop(&self, showing: bool) -> zbus::fdo::Result<()> {
        let show = |reply| Request::SetShowingDesktop(showing, reply);
        super::ask(&self.requests, show).await
    }

    /// The global menu (ADR 0017): the focused window's menu, for the Global menu applet
    /// to draw. It has the kind of menu, the bus name that serves it, its object paths by
    /// what each is, and the application's id, as xdg-shell's `app_id` names its desktop
    /// entry.
    ///
    /// The kind is `gtk` for a `GtkApplication`'s `org.gtk.Menus`, with the paths
    /// `menubar` and `app-menu` for the menus, and `window` and `application` for the
    /// `org.gtk.Actions` of the `win.` and `app.` actions, each only if the window has it.
    ///
    /// The kind is `dbusmenu` for a `com.canonical.dbusmenu`, with the one path `menu`: a Qt
    /// app's and Firefox's, the one an X11 client registered for an XWayland window
    /// (113), and the one a menu claim tied the window to (115), which a menu the window
    /// said it has itself wins over.
    ///
    /// A focused window with no menu has its transient parent's, and so on up, with that
    /// window's application (114). Where none of them has a menu the kind is empty, with
    /// no bus name and no paths, and the application is the focused window's. With no
    /// window focused the application is the empty string too.
    ///
    /// A layer surface that takes the keyboard, as a panel's does for a menu, changes
    /// nothing. No other window's menu is published.
    #[zbus(property)]
    async fn global_menu(&self) -> zbus::fdo::Result<Published> {
        let menu = super::ask(&self.requests, Request::GlobalMenu).await?;
        Ok(menu.published())
    }

    /// Whether the global menu is offered (ADR 0018): whether Caminito tells the apps to
    /// give up their own menubars, as it does while a Global menu applet is on the bus and
    /// for the grace period after it left. A menu claim is taken only while it's true, and
    /// the ties the claims made are all forgotten when it turns false, so a caller claims
    /// again when it turns true.
    #[zbus(property)]
    async fn global_menu_offered(&self) -> zbus::fdo::Result<bool> {
        super::ask(&self.requests, Request::GlobalMenuOffered).await
    }

    /// The menu claim (ADR 0018): the caller, a program with no window of its own, says
    /// that the `com.canonical.dbusmenu` it serves at `path` on this connection is the
    /// menubar of its process's focused window. It's the global menu extension's way to
    /// give its editor's window a global menu, and is promised to no other client: it may
    /// change with the extension.
    ///
    /// The window is the focused one, if its client is of the caller's process or of that
    /// process's parent. It's then tied to the menu, which `GlobalMenu` has while that
    /// window has the focus and says nothing of a menu itself. The last claim on a window
    /// wins, and a caller may hold several windows. A tie lasts until its window is gone,
    /// the caller leaves the bus, or `GlobalMenuOffered` turns false. Nothing releases one.
    ///
    /// A claim is dropped if no such window has the focus, or while `GlobalMenuOffered`
    /// is false. The reply is empty whatever happened, so a claim tells its caller nothing
    /// of the windows (ADR 0002).
    async fn claim_global_menu(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
        path: OwnedObjectPath,
    ) -> zbus::fdo::Result<()> {
        let caller = caller(&header);
        let process = caller_process(connection, &header).await;
        let claim = |reply| Request::ClaimGlobalMenu {
            caller: caller.clone(),
            path,
            process,
            reply,
        };
        super::ask(&self.requests, claim).await?;
        self.callers.report_if_gone(connection, caller).await;
        Ok(())
    }

    /// The ext-foreign-toplevel-list identifier of the window a window screenshot is of:
    /// the window on screen that most recently had the focus and isn't of the caller's
    /// process, since the caller's own dialog may have it. It's the empty string when
    /// there's no such window.
    async fn get_active_toplevel(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<String> {
        let caller = caller_process(connection, &header).await;
        let active = |reply| Request::ActiveToplevel(caller, reply);
        // The empty string is the method's "none".
        Ok(super::ask(&self.requests, active)
            .await?
            .unwrap_or_default())
    }

    /// Grabs key combinations for the caller, in the style of `org.gnome.Shell`'s. Each
    /// accelerator is a string in GTK's syntax, as MATE's keys store it: `<Control><Alt>t`,
    /// `XF86AudioMute`. The answer has an action id for each, in order, which
    /// `AcceleratorActivated` then names. The id is 0 for an accelerator that wasn't
    /// grabbed: the empty string and `disabled`, one that doesn't parse, and one another
    /// caller has. An accelerator the caller already has gets the id it had.
    ///
    /// A grabbed key reaches no client. Caminito's own bindings win over a grab of the same
    /// combination, which is then never signalled. The grabs last until they're released
    /// or the caller leaves the bus.
    async fn grab_accelerators(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
        accelerators: Vec<String>,
    ) -> zbus::fdo::Result<Vec<u32>> {
        let caller = caller(&header);
        let grab = |reply| Request::GrabAccelerators {
            caller: caller.clone(),
            accelerators,
            reply,
        };
        let actions = super::ask(&self.requests, grab).await?;
        self.callers.report_if_gone(connection, caller).await;
        Ok(actions)
    }

    /// Releases the caller's grabs with these action ids. An id that isn't one of the
    /// caller's grabs is passed over.
    async fn ungrab_accelerators(
        &self,
        #[zbus(header)] header: Header<'_>,
        actions: Vec<u32>,
    ) -> zbus::fdo::Result<()> {
        let caller = caller(&header);
        let ungrab = |reply| Request::UngrabAccelerators {
            caller,
            actions,
            reply,
        };
        super::ask(&self.requests, ungrab).await
    }

    /// Night Light (ADR 0023), as caminito-displays's tab shows it:
    ///
    /// - `temperature` (`u`): the temperature the outputs are to have, in kelvin, which they
    ///   fade to over about 2 s. It's 6500, neutral, while Night Light is off.
    /// - `location-source` (`s`): where the sun schedule's location comes from: `stored`
    ///   (`night-light-last-location`), `time-zone` (its coordinates in `zone.tab`), or
    ///   `none`, when the sun schedule uses fixed times, 07:00 to 19:00 (138).
    /// - `next-sunrise` (`x`): the Unix time the sun next climbs past −6°, or 07:00 local
    ///   with no location. It's 0 when it doesn't in the next two days, in a polar night or
    ///   under the midnight sun.
    ///
    /// Its settings are `org.mate.Caminito`'s `night-light-*` keys.
    #[zbus(property)]
    async fn night_light(&self) -> zbus::fdo::Result<HashMap<String, OwnedValue>> {
        let published = super::ask(&self.requests, Request::NightLight).await?;
        Ok(HashMap::from([
            ("temperature".into(), published.temperature.into()),
            (
                "location-source".into(),
                Str::from_static(published.location_source).into(),
            ),
            ("next-sunrise".into(), published.next_sunrise.into()),
        ]))
    }

    /// Where the sun is (215), for a dynamic wallpaper, which does no sun sums of its own.
    /// It's worked out with Night Light's sun schedule, whether or not Night Light is on,
    /// and signalled at each minute's working-out, when it has moved by a quarter of a
    /// degree at most:
    ///
    /// - `elevation` (`d`): above the horizon, in degrees, without refraction.
    /// - `azimuth` (`d`): clockwise from north, in degrees, 0 to 360.
    /// - `location-source` (`s`): as `NightLight`'s. With `none`, the sun is the fixed
    ///   times': below −6° until 07:00 and from 19:00, at 45° due south at 13:00, and
    ///   going round at 15° an hour.
    #[zbus(property)]
    async fn sun(&self) -> zbus::fdo::Result<HashMap<String, OwnedValue>> {
        let published = super::ask(&self.requests, Request::Sun).await?;
        Ok(HashMap::from([
            ("elevation".into(), published.position.elevation.into()),
            ("azimuth".into(), published.position.azimuth.into()),
            (
                "location-source".into(),
                Str::from_static(published.location_source).into(),
            ),
        ]))
    }

    /// Turns Night Light on if it's off, and off if it's on, by writing
    /// `night-light-enabled`. It's what a shortcut the user binds to
    /// `caminito-displays --toggle-night-light` calls.
    async fn toggle_night_light(&self) -> zbus::fdo::Result<()> {
        super::ask(&self.requests, Request::ToggleNightLight).await
    }

    /// A grabbed accelerator was pressed: its action id, and the key event's time in
    /// milliseconds, as the Wayland clients get it. It's sent to the grabber alone, so no
    /// other caller learns the keys pressed. The release isn't signalled, and a repeat is
    /// `AcceleratorRepeated`.
    #[zbus(signal)]
    async fn accelerator_activated(
        emitter: &SignalEmitter<'_>,
        action: u32,
        time: u32,
    ) -> zbus::Result<()>;

    /// A grabbed accelerator's key is still held, and repeated (99): its action id, and
    /// the time a key event would have then. It follows the keyboard's delay and rate, as
    /// a Wayland client's own repeats do, until the key's release, another key's press or
    /// the grab's release. It's sent to the grabber alone, which acts on it for a key that
    /// repeats, as a volume key does, and not for one that toggles.
    #[zbus(signal)]
    async fn accelerator_repeated(
        emitter: &SignalEmitter<'_>,
        action: u32,
        time: u32,
    ) -> zbus::Result<()>;
}

/// The process that sent the call with `header`, as the bus knows it. Without it, none of
/// the windows is taken for the caller's, and a menu claim is dropped.
async fn caller_process(connection: &zbus::Connection, header: &Header<'_>) -> Option<u32> {
    let sender = header.sender()?.to_owned();
    let asked = async {
        let bus = zbus::fdo::DBusProxy::new(connection).await?;
        bus.get_connection_unix_process_id(sender.into()).await
    };
    asked
        .await
        .inspect_err(|err| tracing::warn!("can't ask the bus for a caller's process: {err}"))
        .ok()
}

impl Caminito {
    pub(super) fn caminito_request(&mut self, request: Request) {
        // A caller that has gone away doesn't want the answer.
        match request {
            Request::PrimaryOutput(reply) => {
                let _ = reply.send(self.primary_output_name());
            }
            Request::ChoosePrimaryOutput(name, reply) => {
                let chosen = self.choose_primary_output(&name);
                if chosen.is_ok() {
                    self.property_written(Property::PrimaryOutput);
                }
                let _ = reply.send(chosen);
            }
            Request::SaveLayout(reply) => {
                let _ = reply.send(self.save_layout());
            }
            Request::ShowingDesktop(reply) => {
                let _ = reply.send(self.policy.is_showing_desktop());
            }
            Request::SetShowingDesktop(showing, reply) => {
                self.policy.set_showing_desktop(showing);
                self.property_written(Property::ShowingDesktop);
                let _ = reply.send(());
            }
            Request::GlobalMenu(reply) => {
                let _ = reply.send(self.global_menu());
            }
            Request::GlobalMenuOffered(reply) => {
                let _ = reply.send(self.global_menu_offered);
            }
            Request::ClaimGlobalMenu {
                caller,
                path,
                process,
                reply,
            } => {
                self.claim_global_menu(caller, path, process);
                let _ = reply.send(());
            }
            Request::ActiveToplevel(caller, reply) => {
                let _ = reply.send(self.active_toplevel(caller));
            }
            Request::GrabAccelerators {
                caller,
                accelerators,
                reply,
            } => {
                let grabs = &mut self.accelerator_grabs;
                let actions = accelerators.iter().map(|it| grabs.grab(&caller, it));
                let _ = reply.send(actions.collect());
            }
            Request::UngrabAccelerators {
                caller,
                actions,
                reply,
            } => {
                for action in actions {
                    self.accelerator_grabs.ungrab(&caller, action);
                }
                let _ = reply.send(());
            }
            Request::NightLight(reply) => {
                let _ = reply.send(self.night_light.published());
            }
            Request::ToggleNightLight(reply) => {
                self.toggle_night_light();
                let _ = reply.send(());
            }
            Request::Sun(reply) => {
                let _ = reply.send(self.night_light.published_sun());
            }
        }
    }

    /// Signals `AcceleratorActivated` to the grab's `owner`, by its unique name on the bus,
    /// for a key event at `time`, or `AcceleratorRepeated` for a held key's repeat then.
    pub(crate) fn signal_accelerator(&self, owner: &str, action: u32, time: u32, repeat: bool) {
        let Some(dbus) = &self.dbus else {
            return;
        };
        let connection = dbus.connection().clone();
        let owner = owner.to_owned();
        // On zbus's thread: the main thread never waits on the bus (15).
        let signal = async move {
            let signalled = async {
                let emitter = SignalEmitter::new(&connection, PATH)?
                    .set_destination(BusName::try_from(owner.as_str())?);
                if repeat {
                    CaminitoInterface::accelerator_repeated(&emitter, action, time).await
                } else {
                    CaminitoInterface::accelerator_activated(&emitter, action, time).await
                }
            };
            if let Err(err) = signalled.await {
                tracing::warn!("can't signal the accelerator {action} to {owner}: {err}");
            }
        };
        dbus.connection()
            .executor()
            .spawn(signal, "accelerator activated")
            .detach();
    }

    /// The primary output's name as the property has it.
    fn primary_output_name(&self) -> String {
        self.primary_output()
            .map(|output| output.name())
            .unwrap_or_default()
    }

    /// Signals each property that isn't what the callers last heard, in [`Property::ALL`]'s
    /// order.
    pub(crate) fn refresh_properties(&mut self) {
        for property in Property::ALL {
            let now = property.read(self);
            let Some(dbus) = &mut self.dbus else {
                return;
            };
            if dbus.heard.record(property, now) {
                self.signal(property);
            }
        }
    }

    /// A caller wrote `property`, whose setter's arm calls this once it's written. zbus
    /// signals a property after its own setter, so the callers hear it as it is now and the
    /// refresh pass mustn't signal it again. It's read, not taken from what the caller
    /// wrote, since Policy may have it otherwise.
    fn property_written(&mut self, property: Property) {
        let now = property.read(self);
        if let Some(dbus) = &mut self.dbus {
            dbus.heard.record(property, now);
        }
    }

    /// Signals that `property` changed.
    fn signal(&self, property: Property) {
        let Some(dbus) = &self.dbus else {
            return;
        };
        let connection = dbus.connection().clone();
        // On zbus's thread, which asks the event loop for the value it sends: the main
        // thread never waits on the bus (15).
        let signal = async move {
            let signalled = async {
                let interface = connection
                    .object_server()
                    .interface::<_, CaminitoInterface>(PATH)
                    .await?;
                let emitter = interface.signal_emitter();
                let interface = interface.get().await;
                match property {
                    Property::PrimaryOutput => interface.primary_output_changed(emitter).await,
                    Property::ShowingDesktop => interface.showing_desktop_changed(emitter).await,
                    Property::GlobalMenu => interface.global_menu_changed(emitter).await,
                    Property::GlobalMenuOffered => {
                        interface.global_menu_offered_changed(emitter).await
                    }
                    Property::NightLight => interface.night_light_changed(emitter).await,
                    Property::Sun => interface.sun_changed(emitter).await,
                }
            };
            if let Err(err) = signalled.await {
                tracing::warn!("can't signal {property:?} on D-Bus: {err}");
            }
        };
        dbus.connection()
            .executor()
            .spawn(signal, "property changed")
            .detach();
    }
}
