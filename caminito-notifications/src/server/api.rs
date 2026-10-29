//! The daemon's private API, `org.mate.Caminito.Notifications` at
//! `/org/mate/Caminito/Notifications` (ADR 0051, CMN-259): the Notification history for the
//! Notifications applet and the Dock applet. It is unversioned, since the daemon and both
//! applets ship in one package; Do Not Disturb and the settings window add their calls to
//! this interface (CMN-261, CMN-263).
//!
//! An entry is a [`ENTRY_TYPE`] struct, newest first in `List`. `HistoryChanged` tells a
//! client to list again, and `CountsChanged` carries each Notifying app's new count.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use gio::glib::{self, Variant};
use gio::prelude::*;

use super::{Error, Reason, State, close, close_all, invoke, reply};
use crate::history::ENTRY_TYPE;

/// The interface's name, which the signals are emitted on.
pub(super) const NAME: &str = "org.mate.Caminito.Notifications";
/// The object's path.
pub(super) const PATH: &str = "/org/mate/Caminito/Notifications";

const INVALID_ARGS: &str = "org.freedesktop.DBus.Error.InvalidArgs";

const INTERFACE: &str = r#"
<node>
  <interface name="org.mate.Caminito.Notifications">
    <!-- The history, newest first. An entry is: id, the Notifying app's key, its name, its
         icon (a path or a theme's name), summary, body as Pango markup, image (a string like
         the icon, or the pixels of image-data as (iiibiiay), in a variant), actions without
         the default as (key, label), has a default action, resident, has a reply field, and
         the time in seconds since the epoch. -->
    <method name="List">
      <arg type="a(usssssva(ss)bbbt)" name="entries" direction="out"/>
    </method>
    <method name="Dismiss">
      <arg type="u" name="id" direction="in"/>
    </method>
    <method name="ClearApp">
      <arg type="s" name="app" direction="in"/>
    </method>
    <method name="ClearAll"/>
    <!-- The key "default" is a click. The token is an xdg-activation token, or empty. -->
    <method name="InvokeAction">
      <arg type="u" name="id" direction="in"/>
      <arg type="s" name="key" direction="in"/>
      <arg type="s" name="token" direction="in"/>
    </method>
    <method name="Reply">
      <arg type="u" name="id" direction="in"/>
      <arg type="s" name="text" direction="in"/>
    </method>
    <!-- Each Notifying app's entries in the history, by its key. -->
    <method name="Counts">
      <arg type="a{su}" name="counts" direction="out"/>
    </method>
    <signal name="HistoryChanged"/>
    <signal name="CountsChanged">
      <arg type="a{su}" name="counts"/>
    </signal>
  </interface>
</node>
"#;

/// Exports the API on the state's connection.
pub(super) fn register(
    connection: &gio::DBusConnection,
    state: &Rc<RefCell<State>>,
) -> Result<gio::RegistrationId, Error> {
    let node = gio::DBusNodeInfo::for_xml(INTERFACE).expect("the API's interface is valid");
    let interface = node
        .lookup_interface(NAME)
        .expect("the node has the API's interface");
    connection
        .register_object(PATH, &interface)
        .method_call({
            let state = Rc::downgrade(state);
            move |_, _, _, _, method, parameters, invocation| {
                handle(&state, method, &parameters, invocation);
            }
        })
        .build()
        .map_err(Error::Export)
}

fn missing(id: u32) -> String {
    format!("there's no notification {id} in the history")
}

/// Answers a client's call. gio has checked its arguments against [`INTERFACE`].
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
    // What a week passed over isn't listed or counted, whenever the timer would have run.
    state.borrow_mut().prune_history();
    let refuse = |message: String| invocation.clone().return_dbus_error(INVALID_ARGS, &message);
    let id = || parameters.child_value(0).get::<u32>().unwrap_or(0);
    let text = |index| parameters.child_value(index).str().unwrap_or("").to_owned();
    match method {
        "List" => {
            let entry_type = glib::VariantTy::new(ENTRY_TYPE).expect("the entry type is valid");
            let entries = state
                .borrow()
                .history
                .entries()
                .map(|entry| entry.to_variant())
                .collect::<Vec<_>>();
            let list = Variant::array_from_iter_with_type(entry_type, entries);
            invocation.return_value(Some(&Variant::tuple_from_iter([list])));
        }
        "Counts" => {
            let counts = state.borrow().history.counts();
            invocation.return_value(Some(&(counts,).to_variant()));
        }
        "Dismiss" => {
            let id = id();
            if state.borrow().history.get(id).is_none() {
                return refuse(missing(id));
            }
            close(&state, id, Reason::Dismissed);
            invocation.return_value(None);
        }
        "ClearApp" => {
            let ids = state.borrow().history.ids_of(&text(0));
            close_all(&state, &ids, Reason::Dismissed);
            invocation.return_value(None);
        }
        "ClearAll" => {
            let ids: Vec<u32> = state
                .borrow()
                .history
                .entries()
                .map(|entry| entry.id)
                .collect();
            close_all(&state, &ids, Reason::Dismissed);
            invocation.return_value(None);
        }
        "InvokeAction" => {
            let (id, key, token) = (id(), text(1), text(2));
            let has_action = state.borrow().history.get(id).map(|entry| {
                (key == crate::notification::DEFAULT_ACTION && entry.has_default)
                    || entry.bubble.actions.iter().any(|action| action.key == key)
            });
            match has_action {
                None => return refuse(missing(id)),
                Some(false) => return refuse(format!("notification {id} has no action {key}")),
                Some(true) => {}
            }
            invoke(
                &state,
                id,
                &key,
                Some(token).filter(|token| !token.is_empty()),
            );
            invocation.return_value(None);
        }
        "Reply" => {
            let id = id();
            match state.borrow().history.get(id).map(|entry| entry.reply) {
                None => return refuse(missing(id)),
                Some(false) => return refuse(format!("notification {id} has no reply field")),
                Some(true) => {}
            }
            reply(&state, id, &text(1));
            invocation.return_value(None);
        }
        _ => invocation.return_dbus_error(
            "org.freedesktop.DBus.Error.UnknownMethod",
            &format!("there's no method {method}"),
        ),
    }
}
