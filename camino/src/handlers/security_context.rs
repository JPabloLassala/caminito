//! `wp_security_context_v1`, and the one rule that hides the privileged globals from
//! sandboxed clients (06, 68). A sandbox engine such as Flatpak hands Camino a listening
//! socket, and a client that connects through it is sandboxed for as long as it lives.

use std::sync::Arc;

use smithay::reexports::wayland_server::Client;
use smithay::wayland::security_context::{
    SecurityContext, SecurityContextHandler, SecurityContextListenerSource,
};

use smithay::xwayland::XWaylandClientData;

use crate::state::{ClientState, State};

/// Whether `client` is offered the privileged globals, which show or change what belongs to
/// other clients: layer-shell, the two foreign-toplevel protocols, ext-workspace, output
/// management, the two data-control protocols, and security-context itself, so that a
/// sandboxed client can't make a context of its own. Every such global's filter is this
/// function. M6's image capture and M7's session lock join them.
pub(crate) fn sees_privileged_globals(client: &Client) -> bool {
    match client.get_data::<ClientState>() {
        Some(data) => data.security_context.is_none(),
        // Xwayland's client data is Smithay's, and Xwayland is never sandboxed. A client
        // with any other data is one this function doesn't know, and is offered nothing.
        None => client.get_data::<XWaylandClientData>().is_some(),
    }
}

impl SecurityContextHandler for State {
    fn context_created(&mut self, source: SecurityContextListenerSource, context: SecurityContext) {
        tracing::debug!(
            "security context for {:?}, instance {:?}, from {:?}",
            context.app_id,
            context.instance_id,
            context.sandbox_engine
        );
        // The source leaves the loop by itself once the engine closes the context.
        let inserted = self
            .camino
            .loop_handle
            .insert_source(source, move |stream, _, state| {
                let data = ClientState {
                    security_context: Some(context.clone()),
                    ..ClientState::default()
                };
                let client = state
                    .camino
                    .display_handle
                    .insert_client(stream, Arc::new(data));
                if let Err(err) = client {
                    tracing::warn!("can't add a sandboxed client: {err}");
                }
            });
        if let Err(err) = inserted {
            tracing::warn!("can't listen for a security context's clients: {err}");
        }
    }
}
