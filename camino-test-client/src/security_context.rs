//! `wp_security_context_v1` as a sandbox engine such as Flatpak uses it (68): the client
//! hands the compositor a listening socket, and whoever connects to it is sandboxed.

use std::os::fd::AsFd;
use std::os::unix::net::UnixListener;

use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::wp::security_context::v1::client::wp_security_context_manager_v1::{
    self, WpSecurityContextManagerV1,
};
use wayland_protocols::wp::security_context::v1::client::wp_security_context_v1::{
    self, WpSecurityContextV1,
};

use crate::{Client, State};

impl Client {
    /// Makes a security context for the instance `instance_id` of the app `app_id`. The
    /// compositor takes connections on `listener` for it until `close` is readable or hangs
    /// up. Roundtrip before connecting to the listener.
    pub fn create_security_context(
        &self,
        listener: &UnixListener,
        close: impl AsFd,
        app_id: &str,
        instance_id: &str,
    ) {
        let manager: WpSecurityContextManagerV1 =
            crate::bind(&self.registry, &self.state.globals, &self.qh, 1);
        let context = manager.create_listener(listener.as_fd(), close.as_fd(), &self.qh, ());
        context.set_sandbox_engine("org.camino.TestClient".to_owned());
        context.set_app_id(app_id.to_owned());
        context.set_instance_id(instance_id.to_owned());
        context.commit();
        context.destroy();
        manager.destroy();
    }
}

impl Dispatch<WpSecurityContextManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &WpSecurityContextManagerV1,
        _: wp_security_context_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WpSecurityContextV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &WpSecurityContextV1,
        _: wp_security_context_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
