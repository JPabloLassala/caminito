//! The Notification history's calls of the private API (ADR 0051, CMN-259), for the
//! Notifications applet and the Dock applet. `private_api` exports the interface and routes
//! these calls here, since they need the server's state. An entry is a
//! [`ENTRY_TYPE`](crate::history::ENTRY_TYPE) struct, newest first in `List`.
//! `HistoryChanged` tells a client to list again, and `CountsChanged` carries each Notifying
//! app's new count. `Ungrouped` gives the Notifying apps with grouping off (CMN-265), which
//! `UngroupedChanged` carries as they change. Do Not Disturb's calls (CMN-263) give its state
//! and set it, and `DoNotDisturbChanged` carries its new state.

use std::cell::RefCell;
use std::rc::Weak;

use gio::glib::{self, Variant};
use gio::prelude::*;

use super::{Reason, State, click, close, close_all, invoke, open_center, reply, set_center_open};
use crate::do_not_disturb::DoNotDisturb;
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
    // What a week passed over isn't listed or counted, and a Do Not Disturb that ended is
    // off, whenever the timers would have run.
    state.borrow_mut().prune_history();
    state.borrow_mut().settle_do_not_disturb();
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
        "Ungrouped" => {
            let apps = state.borrow().ungrouped.apps.clone();
            invocation.return_value(Some(&(apps,).to_variant()));
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
            // "default" is a click, which every entry takes.
            let is_click = key == crate::notification::DEFAULT_ACTION;
            let has_action = state.borrow().history.get(id).map(|entry| {
                is_click || entry.bubble.actions.iter().any(|action| action.key == key)
            });
            match has_action {
                None => return refuse(missing(id)),
                Some(false) => return refuse(format!("notification {id} has no action {key}")),
                Some(true) => {}
            }
            let token = Some(token).filter(|token| !token.is_empty());
            if is_click {
                click(&state, id, token);
            } else {
                invoke(&state, id, &key, token);
            }
            invocation.return_value(None);
        }
        "Reply" => {
            let id = id();
            let has_field = state
                .borrow()
                .history
                .get(id)
                .map(|entry| entry.bubble.reply.is_some());
            match has_field {
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
        "DoNotDisturb" => {
            let state = state.borrow().do_not_disturb.to_wire();
            invocation.return_value(Some(&state.to_variant()));
        }
        "SetDoNotDisturb" => {
            let nick = text(0);
            let mut guard = state.borrow_mut();
            let (now, zone) = (guard.clock.now(), guard.clock.zone());
            let Some(to) = DoNotDisturb::of_nick(&nick, now, &zone) else {
                drop(guard);
                return refuse(format!("Do Not Disturb has no duration {nick}"));
            };
            guard.set_do_not_disturb(to);
            invocation.return_value(None);
        }
        "ToggleDoNotDisturb" => {
            let mut guard = state.borrow_mut();
            let to = if guard.do_not_disturb.is_on() {
                DoNotDisturb::Off
            } else {
                DoNotDisturb::On { end: None }
            };
            guard.set_do_not_disturb(to);
            invocation.return_value(None);
        }
        _ => invocation.return_dbus_error(
            "org.freedesktop.DBus.Error.UnknownMethod",
            &format!("there's no method {method}"),
        ),
    }
}
