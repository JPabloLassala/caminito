//! ext-idle-notify and idle-inhibit (128): a client that hears when the seat goes idle, as
//! swayidle does, and one that holds idle off while its surface shows, as a video player does.

use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::ext::idle_notify::v1::client::ext_idle_notification_v1::{
    self, ExtIdleNotificationV1,
};
use wayland_protocols::ext::idle_notify::v1::client::ext_idle_notifier_v1::{
    self, ExtIdleNotifierV1,
};
use wayland_protocols::wp::idle_inhibit::zv1::client::zwp_idle_inhibit_manager_v1::{
    self, ZwpIdleInhibitManagerV1,
};
use wayland_protocols::wp::idle_inhibit::zv1::client::zwp_idle_inhibitor_v1::{
    self, ZwpIdleInhibitorV1,
};

use crate::{Client, State};

/// The index of one of a client's idle notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdleNotificationId(usize);

/// The index of one of a client's idle inhibitors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InhibitorId(usize);

/// What an idle notification was told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleEvent {
    /// The seat went idle.
    Idled,
    /// It no longer is.
    Resumed,
}

/// An idle notification, and what it was told, oldest first.
pub struct IdleNotification {
    notification: ExtIdleNotificationV1,
    /// What it was told, oldest first.
    pub events: Vec<IdleEvent>,
}

impl Client {
    /// Asks to be told once the seat has gone `timeout_ms` without activity, and when that
    /// ends. Inhibitors hold it off.
    pub fn idle_notification(&mut self, timeout_ms: u32) -> IdleNotificationId {
        let id = IdleNotificationId(self.state.idle_notifications.len());
        let notifier: ExtIdleNotifierV1 = self.global(|state| &mut state.idle_notifier, 1);
        let seat = self.seat();
        let notification = notifier.get_idle_notification(timeout_ms, &seat, &self.qh, id);
        self.state.idle_notifications.push(IdleNotification {
            notification,
            events: Vec::new(),
        });
        id
    }

    /// What the idle notification was told, oldest first.
    pub fn idle_events(&self, id: IdleNotificationId) -> &[IdleEvent] {
        &self.state.idle_notifications[id.0].events
    }

    /// Destroys the idle notification.
    pub fn destroy_idle_notification(&self, id: IdleNotificationId) {
        self.state.idle_notifications[id.0].notification.destroy();
    }

    /// Asks for idle to be held off while `surface` shows.
    pub fn inhibit_idle(&mut self, surface: &WlSurface) -> InhibitorId {
        let id = InhibitorId(self.state.idle_inhibitors.len());
        let manager: ZwpIdleInhibitManagerV1 =
            self.global(|state| &mut state.idle_inhibit_manager, 1);
        let inhibitor = manager.create_inhibitor(surface, &self.qh, ());
        self.state.idle_inhibitors.push(inhibitor);
        id
    }

    /// Destroys the idle inhibitor.
    pub fn uninhibit_idle(&self, id: InhibitorId) {
        self.state.idle_inhibitors[id.0].destroy();
    }
}

impl Dispatch<ExtIdleNotificationV1, IdleNotificationId> for State {
    fn event(
        state: &mut Self,
        _: &ExtIdleNotificationV1,
        event: ext_idle_notification_v1::Event,
        id: &IdleNotificationId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let event = match event {
            ext_idle_notification_v1::Event::Idled => IdleEvent::Idled,
            ext_idle_notification_v1::Event::Resumed => IdleEvent::Resumed,
            _ => return,
        };
        state.idle_notifications[id.0].events.push(event);
    }
}

impl Dispatch<ExtIdleNotifierV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ExtIdleNotifierV1,
        _: ext_idle_notifier_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpIdleInhibitManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwpIdleInhibitManagerV1,
        _: zwp_idle_inhibit_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpIdleInhibitorV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwpIdleInhibitorV1,
        _: zwp_idle_inhibitor_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
