//! The global menu (110, ADR 0017): what `org.mate.Caminito` publishes of the focused
//! window's menu, and the Global menu applet as a peer, whose bus name is the switch for
//! the apps giving up their own menubars.
//!
//! So far the menus are a `GtkApplication`'s, from `gtk_surface1.set_dbus_properties`, a
//! classic `GtkMenuBar`'s that `appmenu-gtk-module` exports the same way (111), a Qt app's
//! or Firefox's dbusmenu, from `org_kde_kwin_appmenu` (112), and the dbusmenu an X11
//! client registered for its window with `registrar` (113). The switch turns
//! `gtk_shell1`'s capability, the appmenu global and the registrar's name, the three
//! together, and stays on for a grace period after the applet's name loses its owner (114).
//! A window with no menu has its transient parent's (114).

use std::collections::HashMap;
use std::time::Duration;

use caminito_policy::{PolicyWindow, WindowId};
use smithay::reexports::calloop::channel;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use zbus::export::ordered_stream::OrderedStreamExt;
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::names::BusName;
use zbus::zvariant::ObjectPath;

use super::registrar::Registration;
use crate::protocols::appmenu::{self, Address};
use crate::protocols::gtk_shell::{self, DbusProperties};
use crate::state::{Caminito, SEAT, State};

/// The well-known name the Global menu applet owns while it's on a panel, as its carried
/// patch has it. The applets of one panel share the one name.
pub(crate) const APPLET_NAME: &str = "org.mate.Caminito.GlobalMenuApplet";

/// The registrar's name, which Qt, Firefox, Electron and `appmenu-gtk-module` each look for
/// an owner of before they give up their menubars (ADR 0017).
pub(crate) const REGISTRAR_NAME: &str = "com.canonical.AppMenu.Registrar";

/// How long the switch stays on after the applet's name loses its owner (ADR 0017): longer
/// than mate-session takes to restart a panel that was killed, since a Qt app, Firefox or
/// an Electron app that got its menubar back would keep it until it's restarted itself.
pub(crate) const GRACE: Duration = Duration::from_secs(10);

/// Whether the apps should give up their own menubars, which follows the applet's bus
/// name. Like policy, it's given the time as `now` and reads no clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Switch {
    /// No applet is on the bus, and none was within the grace period.
    #[default]
    Off,
    /// An applet is on the bus.
    On,
    /// The applet's name lost its owner, as it does while the panel restarts: on until
    /// `until`, and then off unless an owner has come back.
    Leaving { until: Duration },
}

impl Switch {
    /// The applet's name got an owner, or lost it, at `now`. Returns the grace period's
    /// length if one starts.
    fn applet_changed(&mut self, present: bool, now: Duration) -> Option<Duration> {
        match (*self, present) {
            (_, true) => *self = Self::On,
            (Self::On, false) => {
                *self = Self::Leaving { until: now + GRACE };
                return Some(GRACE);
            }
            // Losing the name again starts no new grace period, and an applet that was
            // never there leaves nothing to wait for.
            (Self::Leaving { .. } | Self::Off, false) => {}
        }
        None
    }

    /// Ends the grace period if it's over at `now`. Returns how long is left of it, if it's
    /// still running.
    fn time_passed(&mut self, now: Duration) -> Option<Duration> {
        let Self::Leaving { until } = *self else {
            return None;
        };
        if now < until {
            return Some(until - now);
        }
        *self = Self::Off;
        None
    }

    /// Whether something shows the apps' menubars, or did within the grace period.
    pub(crate) fn is_on(self) -> bool {
        self != Self::Off
    }

    /// The switch's name in the debug dump.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::On => "on",
            Self::Leaving { .. } => "leaving",
        }
    }
}

/// The kind of a menu that's `org.gtk.Menus`, with its actions as `org.gtk.Actions`.
const GTK: &str = "gtk";

/// The kind of a menu that's a `com.canonical.dbusmenu`.
const DBUSMENU: &str = "dbusmenu";

/// What a [`GlobalMenu`] has of a window's menu: its kind, bus name and paths.
type Menu = (&'static str, String, HashMap<String, String>);

/// `GlobalMenu` as it is on the bus, `(ssa{ss}s)`.
pub(super) type Published = (String, String, HashMap<String, String>, String);

/// The focused window's menu, as `org.mate.Caminito` publishes it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GlobalMenu {
    /// The kind of menu, which says what `paths` has, or the empty string for none.
    kind: &'static str,
    /// The connection that serves the menu.
    bus_name: String,
    /// The menu's objects, by what each is.
    paths: HashMap<String, String>,
    /// The application of the window the menu is of, or of the focused window if there's
    /// no menu. It's the app id and not the name ADR 0017 has, on purpose: the applet
    /// finds the desktop entry's name in the user's locale, which Caminito doesn't read.
    application: String,
}

impl GlobalMenu {
    /// The menu of a `GtkApplication`'s window, if it has one: a `GtkApplicationWindow`
    /// names its actions even with neither a menubar nor an app menu.
    fn gtk(properties: DbusProperties) -> Option<Menu> {
        let bus_name = properties.unique_bus_name?;
        if properties.menubar_path.is_none() && properties.app_menu_path.is_none() {
            return None;
        }
        let paths = [
            ("menubar", properties.menubar_path),
            ("app-menu", properties.app_menu_path),
            ("window", properties.window_object_path),
            ("application", properties.application_object_path),
        ];
        let paths = paths
            .into_iter()
            .filter_map(|(key, path)| Some((key.to_owned(), path?)))
            .collect();
        Some((GTK, bus_name, paths))
    }

    /// The menu of a window that gave its dbusmenu's address, if that's a bus name and an
    /// object path: the applet takes what's published to be both, and any client can say
    /// anything.
    fn dbusmenu(address: Address) -> Option<Menu> {
        BusName::try_from(address.service_name.as_str()).ok()?;
        ObjectPath::try_from(address.object_path.as_str()).ok()?;
        let paths = HashMap::from([("menu".to_owned(), address.object_path)]);
        Some((DBUSMENU, address.service_name, paths))
    }

    /// The dbusmenu an X11 client registered for its window (113). The bus named its
    /// owner, and the path came as one.
    fn registered(registration: &Registration) -> Menu {
        let path = registration.path.to_string();
        let paths = HashMap::from([("menu".to_owned(), path)]);
        (DBUSMENU, registration.owner.clone(), paths)
    }

    pub(super) fn published(self) -> Published {
        let kind = self.kind.to_owned();
        (kind, self.bus_name, self.paths, self.application)
    }
}

/// Tells the event loop whether the applet's name has an owner, now and at each change,
/// until the task is dropped. It returns once the bus is sending the changes, and the future
/// it returns then passes them on.
pub(super) async fn watch_applet(
    connection: &zbus::Connection,
    present: channel::Sender<bool>,
) -> zbus::Result<impl Future<Output = ()> + Send + 'static> {
    let bus = zbus::fdo::DBusProxy::new(connection).await?;
    let mut changes = bus
        .receive_name_owner_changed_with_args(&[(0, APPLET_NAME)])
        .await?;
    // After the changes are asked for, so that an applet that comes meanwhile isn't missed.
    let now = bus.name_has_owner(BusName::try_from(APPLET_NAME)?).await?;
    // Caminito is stopping if this fails, and the task goes with it.
    let _ = present.send(now);
    Ok(async move {
        // The proxy stays with the stream, which needs its match rule kept.
        let _bus = bus;
        while let Some(change) = changes.next().await {
            let Ok(args) = change.args() else { continue };
            if present.send(args.new_owner().is_some()).is_err() {
                break;
            }
        }
    })
}

/// Caminito's hold on the registrar's name, which it asks the bus for and gives up without
/// waiting.
pub(super) struct Registrar {
    /// Whether the name was last asked for, and not given up.
    asked: bool,
    /// How many times the name was asked for or given up, which tells the last time from
    /// the ones before it.
    changes: u64,
    /// The last asking or giving up, which the next one waits for, so that they reach the
    /// bus in order.
    last: Option<zbus::Task<()>>,
    /// Tells the event loop that the name was given up, and at which change.
    released: channel::Sender<u64>,
}

impl Registrar {
    pub(super) fn new(released: channel::Sender<u64>) -> Self {
        Self {
            asked: false,
            changes: 0,
            last: None,
            released,
        }
    }

    /// Asks the bus for the name, or gives it up, if that's not what was last done.
    pub(super) fn set_owned(&mut self, connection: &zbus::Connection, owned: bool) {
        if self.asked == owned {
            return;
        }
        self.asked = owned;
        self.changes += 1;
        let change = self.changes;
        let last = self.last.take();
        let released = self.released.clone();
        let bus = connection.clone();
        let changed = async move {
            if let Some(last) = last {
                let _ = last.await;
            }
            if owned {
                // In the queue behind another registrar, if there is one that won't be
                // replaced, so that the name comes when that one goes.
                let flags = RequestNameFlags::ReplaceExisting.into();
                match bus.request_name_with_flags(REGISTRAR_NAME, flags).await {
                    Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => {}
                    Ok(_) => tracing::warn!("another registrar has {REGISTRAR_NAME}"),
                    Err(err) => tracing::warn!("can't own {REGISTRAR_NAME}: {err}"),
                }
            } else {
                if let Err(err) = bus.release_name(REGISTRAR_NAME).await {
                    tracing::warn!("can't give up {REGISTRAR_NAME}: {err}");
                }
                // Caminito is stopping if this fails.
                let _ = released.send(change);
            }
        };
        self.last = Some(connection.executor().spawn(changed, "registrar's name"));
    }

    /// Whether giving the name up at `change` was the last thing asked of the bus, so that
    /// the name isn't about to be owned again.
    pub(super) fn is_given_up(&self, change: u64) -> bool {
        !self.asked && self.changes == change
    }
}

impl Caminito {
    /// The applet's name got an owner, or lost it, at `now`. What makes the apps give up
    /// their menubars comes at once, and goes only after the grace period, once the
    /// registrar's name is given up.
    pub(super) fn global_menu_applet_changed(&mut self, present: bool, now: Duration) {
        tracing::debug!("the global menu applet is on the bus: {present}");
        if present {
            self.global_menu_offered = true;
        }
        let Some(grace) = self.global_menu_switch.applet_changed(present, now) else {
            return;
        };
        // One timer for each grace period. One that outlives its own, because an owner
        // came back, finds nothing to end, or waits for the end of the next.
        let timer = Timer::from_duration(grace);
        self.loop_handle
            .insert_source(timer, |_, (), state| {
                let now = state.caminito.now();
                match state.caminito.global_menu_time_passed(now) {
                    Some(left) => TimeoutAction::ToDuration(left),
                    None => TimeoutAction::Drop,
                }
            })
            .expect("a timer joins the event loop");
    }

    /// The bus took the registrar's name back, at the registrar's `change`. For an applet
    /// that came meanwhile the name has been asked for again, or is about to be, and
    /// nothing goes.
    pub(super) fn registrar_released(&mut self, change: u64) {
        if !self.global_menu_switch.is_on() && self.registrar_is_given_up(change) {
            self.global_menu_offered = false;
        }
    }

    /// Takes the switch off if the grace period after the applet's name lost its owner is
    /// over at `now`. Returns how long is left of it, if it's still running.
    pub(crate) fn global_menu_time_passed(&mut self, now: Duration) -> Option<Duration> {
        self.global_menu_switch.time_passed(now)
    }

    /// The menu `id` says it has, with its application.
    fn menu_of(&self, id: WindowId) -> Option<GlobalMenu> {
        let mapped = self.policy.window(id)?;
        let surface = mapped.wl_surface();
        let gtk = surface
            .as_ref()
            .and_then(gtk_shell::dbus_properties)
            .and_then(GlobalMenu::gtk);
        let dbusmenu = || {
            let address = surface.as_ref().and_then(appmenu::address)?;
            GlobalMenu::dbusmenu(address)
        };
        let registered = || {
            let registration = self.app_menu_registrations.menu(mapped.x11_window()?)?;
            Some(GlobalMenu::registered(registration))
        };
        let (kind, bus_name, paths) = gtk.or_else(dbusmenu).or_else(registered)?;
        Some(GlobalMenu {
            kind,
            bus_name,
            paths,
            application: mapped.app_id().unwrap_or_default(),
        })
    }

    /// The focused window's menu, as the property has it. A window with no menu has its
    /// transient parent's, and so on up, so that the menus stay while a dialog is open.
    pub(super) fn global_menu(&self) -> GlobalMenu {
        // Policy's focus, which a layer surface that has the keyboard leaves as it is: a
        // menu opened from the panel changes nothing.
        let Some(focused) = self.policy.focus(SEAT) else {
            return GlobalMenu::default();
        };
        // Policy's parents make no cycle.
        let mut ancestors = std::iter::successors(Some(focused), |id| self.policy.parent(*id));
        if let Some(menu) = ancestors.find_map(|id| self.menu_of(id)) {
            return menu;
        }
        let application = self.policy.window(focused).and_then(PolicyWindow::app_id);
        GlobalMenu {
            application: application.unwrap_or_default(),
            ..GlobalMenu::default()
        }
    }

    /// Tells the apps whether an applet shows their menubars, and signals `GlobalMenu`
    /// if it's not what the callers last heard: the focus is another window's, or the
    /// focused window said where its menus are.
    ///
    /// The registrar's name is never there without the appmenu global: a Qt app that finds
    /// the name alone has no menu anywhere (112). So the name is asked for after the global
    /// is offered, and the global goes after the name is given up.
    pub(crate) fn refresh_global_menu(&mut self) {
        let offered = self.global_menu_offered;
        self.gtk_shell_state.set_global_menu_bar(offered);
        let withdrawn = self
            .appmenu_state
            .set_offered::<State>(&self.display_handle, offered);
        self.globals_to_withdraw.extend(withdrawn);
        self.refresh_registrar(self.global_menu_switch.is_on());
        self.refresh_global_menu_property();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: Duration = Duration::from_secs(1);
    /// When the applet's name loses its owner in these.
    const LEFT: Duration = Duration::from_secs(100);

    /// A switch that an applet turned on, and whose applet left the bus at `LEFT`.
    fn left() -> Switch {
        let mut switch = Switch::default();
        switch.applet_changed(true, SECOND);
        assert_eq!(switch.applet_changed(false, LEFT), Some(GRACE));
        switch
    }

    #[test]
    fn the_switch_is_off_until_an_applet_comes() {
        let mut switch = Switch::default();
        assert!(!switch.is_on());

        // What the watch says first where no applet is on the bus.
        assert_eq!(switch.applet_changed(false, SECOND), None);

        assert_eq!(switch, Switch::Off);
        assert_eq!(switch.time_passed(SECOND + GRACE), None);

        switch.applet_changed(true, SECOND * 2);

        assert!(switch.is_on());
    }

    #[test]
    fn the_switch_stays_on_for_the_grace_period_after_the_applet_leaves() {
        let mut switch = left();

        assert!(switch.is_on());
        assert_eq!(switch.time_passed(LEFT + SECOND), Some(GRACE - SECOND));
        assert!(switch.is_on());
        assert_eq!(switch.time_passed(LEFT + GRACE - SECOND), Some(SECOND));
        assert!(switch.is_on());
    }

    #[test]
    fn the_switch_goes_off_when_the_grace_period_ends_with_no_applet() {
        let mut switch = left();

        assert_eq!(switch.time_passed(LEFT + GRACE), None);

        assert_eq!(switch, Switch::Off);
    }

    #[test]
    fn an_applet_that_comes_back_within_the_grace_period_keeps_the_switch_on() {
        let mut switch = left();

        switch.applet_changed(true, LEFT + SECOND);

        // The timer of the grace period it ended finds nothing to do.
        assert_eq!(switch.time_passed(LEFT + GRACE), None);
        assert_eq!(switch, Switch::On);
    }

    #[test]
    fn each_time_the_applet_leaves_has_a_grace_period_of_its_own() {
        let mut switch = left();
        switch.applet_changed(true, LEFT + SECOND);
        let again = LEFT + SECOND * 5;

        switch.applet_changed(false, again);

        // The first one's timer fires within the second, and has to wait for its end.
        let rest = again - LEFT;
        assert_eq!(switch.time_passed(LEFT + GRACE), Some(rest));
        assert!(switch.is_on());
        assert_eq!(switch.time_passed(again + GRACE), None);
        assert_eq!(switch, Switch::Off);
    }
}
