//! ext-idle-notify (06, 128): a client asks to hear when the seat has gone a while without
//! activity, and when it no longer has. Smithay's own state runs a timer of its own for each
//! notification, on the loop's clock; this one only hands each notification to the
//! compositor, whose `idle` module times them with the blank and the lock, on Caminito's
//! clock, so that a test can drive them all with its own (15).
//!
//! It's offered to every client, sandboxed ones too (68): it says nothing of the session but
//! when it's idle. Caminito has one seat, so the seat a notification names is the one.

use std::time::Duration;

use smithay::reexports::wayland_protocols::ext::idle_notify::v1::server::ext_idle_notification_v1::{
    self, ExtIdleNotificationV1,
};
use smithay::reexports::wayland_protocols::ext::idle_notify::v1::server::ext_idle_notifier_v1::{
    self, ExtIdleNotifierV1,
};
use smithay::reexports::wayland_server::backend::{ClientId, GlobalId};
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New,
};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

/// The version offered: 2 adds `get_input_idle_notification`, which inhibitors don't hold
/// off.
const VERSION: u32 = 2;

/// What the compositor does with the notifications.
pub trait IdleNotifyHandler: 'static {
    /// A client asked to be told `idled` once the seat has gone `timeout` without activity,
    /// and `resumed` when that ends. Inhibitors hold it off unless it's `input_only`.
    fn new_idle_notification(
        &mut self,
        notification: ExtIdleNotificationV1,
        timeout: Duration,
        input_only: bool,
    );

    /// The client destroyed the notification, or went away.
    fn idle_notification_destroyed(&mut self, notification: &ExtIdleNotificationV1);
}

/// The `ext_idle_notifier_v1` global.
#[derive(Debug)]
pub struct IdleNotifyState {
    _global: GlobalId,
}

/// The global's and each notifier's data.
#[derive(Debug)]
pub struct IdleNotifierData;

/// A notification's data.
#[derive(Debug)]
pub struct IdleNotificationData;

impl IdleNotifyState {
    /// Offers `ext_idle_notifier_v1` to every client.
    pub fn new<D>(display: &DisplayHandle) -> Self
    where
        D: GlobalDispatch<ExtIdleNotifierV1, IdleNotifierData> + 'static,
    {
        let global = display.create_global::<D, ExtIdleNotifierV1, _>(VERSION, IdleNotifierData);
        Self { _global: global }
    }
}

impl<D> GlobalDispatch2<ExtIdleNotifierV1, D> for IdleNotifierData
where
    D: Dispatch<ExtIdleNotifierV1, IdleNotifierData>,
{
    fn bind(
        &self,
        _state: &mut D,
        _display: &DisplayHandle,
        _client: &Client,
        resource: New<ExtIdleNotifierV1>,
        data_init: &mut DataInit<'_, D>,
    ) {
        data_init.init(resource, IdleNotifierData);
    }
}

impl<D> Dispatch2<ExtIdleNotifierV1, D> for IdleNotifierData
where
    D: IdleNotifyHandler + Dispatch<ExtIdleNotificationV1, IdleNotificationData>,
{
    fn request(
        &self,
        state: &mut D,
        _client: &Client,
        _resource: &ExtIdleNotifierV1,
        request: ext_idle_notifier_v1::Request,
        _display: &DisplayHandle,
        data_init: &mut DataInit<'_, D>,
    ) {
        let (id, timeout, input_only) = match request {
            ext_idle_notifier_v1::Request::GetIdleNotification { id, timeout, .. } => {
                (id, timeout, false)
            }
            ext_idle_notifier_v1::Request::GetInputIdleNotification { id, timeout, .. } => {
                (id, timeout, true)
            }
            // `destroy`, and the requests of a later version.
            _ => return,
        };
        let notification = data_init.init(id, IdleNotificationData);
        let timeout = Duration::from_millis(u64::from(timeout));
        state.new_idle_notification(notification, timeout, input_only);
    }
}

impl<D: IdleNotifyHandler> Dispatch2<ExtIdleNotificationV1, D> for IdleNotificationData {
    fn request(
        &self,
        _state: &mut D,
        _client: &Client,
        _resource: &ExtIdleNotificationV1,
        _request: ext_idle_notification_v1::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        // `destroy` is its only request, and `destroyed` follows it.
    }

    fn destroyed(&self, state: &mut D, _client: ClientId, notification: &ExtIdleNotificationV1) {
        state.idle_notification_destroyed(notification);
    }
}
