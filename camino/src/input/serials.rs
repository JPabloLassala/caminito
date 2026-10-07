//! The serials of the key and button events clients were last sent. A popup's explicit
//! grab names the event that opened it (95), and toolkits name the newest they have: GTK
//! the later of its last key event and its last button press, Qt its last input event.

use std::collections::VecDeque;

use smithay::reexports::wayland_server::Client;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::Serial;

/// How many events are remembered: the newest a client has is among the last few sent,
/// however slow the client.
const REMEMBERED: usize = 32;

/// The last key and button events sent, each with the client it went to.
#[derive(Default)]
pub(crate) struct InputSerials(VecDeque<(Serial, ClientId)>);

impl InputSerials {
    /// A key or button event with `serial` went to `surface`'s client, if to any.
    pub(crate) fn sent(&mut self, serial: Serial, surface: Option<&WlSurface>) {
        let Some(client) = surface.and_then(|surface| surface.client()) else {
            return;
        };
        if self.0.len() == REMEMBERED {
            self.0.pop_front();
        }
        self.0.push_back((serial, client.id()));
    }

    /// Whether `serial` is of one of the last events sent, and `client` got it.
    pub(crate) fn has(&self, serial: Serial, client: &Client) -> bool {
        self.0.contains(&(serial, client.id()))
    }
}
