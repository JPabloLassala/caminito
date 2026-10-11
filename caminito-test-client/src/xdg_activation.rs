//! xdg-activation (165): a client that asks for a token, as a terminal launching an app or
//! a toolkit presenting a window does, and asks for a window to be activated with one.

use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::xdg::activation::v1::client::xdg_activation_token_v1::{
    self, XdgActivationTokenV1,
};
use wayland_protocols::xdg::activation::v1::client::xdg_activation_v1::{self, XdgActivationV1};

use crate::{Client, State, WindowId};

/// The index of one of a client's activation tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenId(usize);

impl Client {
    /// Asks for a token, with the seat's serial `serial` and the window `from`'s surface if
    /// given. The token arrives once the client has dispatched Caminito's answer.
    pub fn request_activation_token(
        &mut self,
        serial: Option<u32>,
        from: Option<WindowId>,
    ) -> TokenId {
        let id = TokenId(self.state.activation_tokens.len());
        let activation: XdgActivationV1 = self.global(|state| &mut state.activation, 1);
        let token = activation.get_activation_token(&self.qh, id);
        if let Some(serial) = serial {
            token.set_serial(serial, &self.seat());
        }
        if let Some(from) = from {
            token.set_surface(&self.window(from).surface);
        }
        token.commit();
        self.state.activation_tokens.push(None);
        id
    }

    /// The token, once Caminito has issued it.
    pub fn activation_token(&self, id: TokenId) -> Option<&str> {
        self.state.activation_tokens[id.0].as_deref()
    }

    /// Asks for `window` to be activated with `token`.
    pub fn activate(&mut self, token: &str, window: WindowId) {
        let activation: XdgActivationV1 = self.global(|state| &mut state.activation, 1);
        activation.activate(token.to_owned(), &self.window(window).surface);
    }
}

impl Dispatch<XdgActivationV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &XdgActivationV1,
        _: xdg_activation_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<XdgActivationTokenV1, TokenId> for State {
    fn event(
        state: &mut Self,
        token: &XdgActivationTokenV1,
        event: xdg_activation_token_v1::Event,
        id: &TokenId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_activation_token_v1::Event::Done { token: issued } = event {
            state.activation_tokens[id.0] = Some(issued);
            token.destroy();
        }
    }
}
