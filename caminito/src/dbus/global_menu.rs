//! The global menu (110, ADR 0017): what `org.mate.Caminito` publishes of the focused
//! window's menu, and the Global menu applet as a peer, whose bus name is the switch for
//! the apps giving up their own menubars.
//!
//! So far the menus are a `GtkApplication`'s, from `gtk_surface1.set_dbus_properties`, and
//! the switch turns `gtk_shell1`'s capability alone. The other channels are tickets 111 to
//! 113's, and the grace period after the applet's name loses its owner is 114's.

use std::collections::HashMap;

use caminito_policy::PolicyWindow;
use smithay::reexports::calloop::channel;
use zbus::export::ordered_stream::OrderedStreamExt;
use zbus::names::BusName;

use crate::protocols::gtk_shell::{self, DbusProperties};
use crate::state::{Caminito, SEAT};

/// The well-known name the Global menu applet owns while it's on a panel, as its carried
/// patch has it. The applets of one panel share the one name.
pub(crate) const APPLET_NAME: &str = "org.mate.Caminito.GlobalMenuApplet";

/// The kind of a menu that's `org.gtk.Menus`, with its actions as `org.gtk.Actions`.
const GTK: &str = "gtk";

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
    /// The focused window's application, whether it has a menu or not. It's the app id
    /// and not the name ADR 0017 has, on purpose: the applet finds the desktop entry's
    /// name in the user's locale, which Caminito doesn't read.
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

impl Caminito {
    /// The applet's name got an owner, or lost it.
    pub(super) fn global_menu_applet_changed(&mut self, present: bool) {
        tracing::debug!("the global menu applet is on the bus: {present}");
        self.global_menu_applet = present;
    }

    /// The focused window's menu, as the property has it.
    pub(super) fn global_menu(&self) -> GlobalMenu {
        // Policy's focus, which a layer surface that has the keyboard leaves as it is: a
        // menu opened from the panel changes nothing.
        let focused = self.policy.focus(SEAT);
        let Some(mapped) = focused.and_then(|id| self.policy.window(id)) else {
            return GlobalMenu::default();
        };
        let application = mapped.app_id().unwrap_or_default();
        let menu = mapped
            .wl_surface()
            .and_then(|surface| gtk_shell::dbus_properties(&surface))
            .and_then(GlobalMenu::gtk);
        match menu {
            Some((kind, bus_name, paths)) => GlobalMenu {
                kind,
                bus_name,
                paths,
                application,
            },
            None => GlobalMenu {
                application,
                ..GlobalMenu::default()
            },
        }
    }

    /// Tells the GTK apps whether an applet shows their menubars, and signals `GlobalMenu`
    /// if it's not what the callers last heard: the focus is another window's, or the
    /// focused window said where its menus are.
    pub(crate) fn refresh_global_menu(&mut self) {
        self.gtk_shell_state
            .set_global_menu_bar(self.global_menu_applet);
        self.refresh_global_menu_property();
    }
}
