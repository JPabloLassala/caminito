//! The Notification history's calls of the private API (ADR 0051, CMN-259), for the
//! Notifications applet and the Dock applet. `private_api` exports the interface and routes
//! these calls here, since they need the server's state. An entry is a
//! [`ENTRY_TYPE`](crate::history::ENTRY_TYPE) struct, newest first in `List`.
//! `HistoryChanged` tells a client to list again, and `CountsChanged` carries each Notifying
//! app's new count.

use std::cell::RefCell;
use std::rc::Weak;

use gio::glib::{self, Variant};
use gio::prelude::*;

use super::{Reason, State, close, close_all, invoke, open_center, reply, set_center_open};
use crate::history::ENTRY_TYPE;

const INVALID_ARGS: &str = "org.freedesktop.DBus.Error.InvalidArgs";

fn missing(id: u32) -> String {
    format!("there's no notification {id} in the history")
}

/// Answers a client's call. gio has checked its arguments against the interface `private_api` exports.
pub(super) fn handle(
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
        "OpenCenter" => {
            open_center(&state);
            invocation.return_value(None);
        }
        "SetCenterOpen" => {
            let open = parameters.child_value(0).get::<bool>().unwrap_or(false);
            set_center_open(&state, open, invocation.sender().as_deref());
            invocation.return_value(None);
        }
        _ => invocation.return_dbus_error(
            "org.freedesktop.DBus.Error.UnknownMethod",
            &format!("there's no method {method}"),
        ),
    }
}
