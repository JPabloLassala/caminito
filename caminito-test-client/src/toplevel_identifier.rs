//! A `caminito_toplevel_identifier_manager_v1` client, as the Dock applet is (CMN-243, ADR
//! 0049): it asks, with a window's foreign toplevel management handle, for the identifier
//! the foreign toplevel list gives the window. The protocol is generated from the XML
//! Caminito's own code is generated from.

use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};

use self::raw::caminito_toplevel_identifier_manager_v1::{
    self, CaminitoToplevelIdentifierManagerV1,
};
use self::raw::caminito_toplevel_identifier_v1::{self, CaminitoToplevelIdentifierV1};
use crate::{Client, State};

/// Caminito's two interfaces, from `wayland-scanner`.
#[allow(
    dead_code,
    non_camel_case_types,
    non_upper_case_globals,
    unused_imports,
    clippy::all,
    reason = "generated code"
)]
pub(crate) mod raw {
    use wayland_client;
    use wayland_client::protocol::*;
    use wayland_protocols_wlr::foreign_toplevel::v1::client::*;

    pub mod __interfaces {
        use wayland_client::backend as wayland_backend;
        use wayland_client::protocol::__interfaces::*;
        use wayland_protocols_wlr::foreign_toplevel::v1::client::__interfaces::*;
        wayland_scanner::generate_interfaces!(
            "../caminito/resources/caminito-toplevel-identifier-v1.xml"
        );
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!(
        "../caminito/resources/caminito-toplevel-identifier-v1.xml"
    );
}

/// What the compositor answered to a request for a window's identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentifierAnswer {
    /// Not answered yet: roundtrip.
    Pending,
    /// The window's identifier.
    Identifier(String),
    /// The window is gone.
    Gone,
}

/// The manager, once bound, and the answers to the requests made.
#[derive(Default)]
pub struct ToplevelIdentifiers {
    manager: Option<CaminitoToplevelIdentifierManagerV1>,
    /// One per request, in the order they were made.
    answers: Vec<IdentifierAnswer>,
}

impl Client {
    /// Whether the compositor offers `caminito_toplevel_identifier_manager_v1`, as of the
    /// last roundtrip.
    pub fn has_toplevel_identifier_manager(&self) -> bool {
        let interface = CaminitoToplevelIdentifierManagerV1::interface().name;
        self.state.globals.iter().any(|g| g.interface == interface)
    }

    /// Asks for the identifier of the toplevel at `index` of foreign toplevel management,
    /// which may be one that has closed if the client kept its handle. Returns what to
    /// read the answer by, after a roundtrip.
    pub fn ask_toplevel_identifier(&mut self, index: usize) -> usize {
        let manager = self.global(|state| &mut state.toplevel_identifiers.manager, 1);
        let handle = &self.state.foreign_toplevels.toplevels[index].proxy;
        let answers = &mut self.state.toplevel_identifiers.answers;
        answers.push(IdentifierAnswer::Pending);
        manager.get_identifier(handle, &self.qh, answers.len() - 1);
        answers.len() - 1
    }

    /// The answer to the request `asked`, which `ask_toplevel_identifier` returned.
    pub fn toplevel_identifier(&self, asked: usize) -> &IdentifierAnswer {
        &self.state.toplevel_identifiers.answers[asked]
    }
}

impl Dispatch<CaminitoToplevelIdentifierManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &CaminitoToplevelIdentifierManagerV1,
        event: caminito_toplevel_identifier_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // The interface has no events.
        match event {}
    }
}

impl Dispatch<CaminitoToplevelIdentifierV1, usize> for State {
    fn event(
        state: &mut Self,
        _: &CaminitoToplevelIdentifierV1,
        event: caminito_toplevel_identifier_v1::Event,
        asked: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use caminito_toplevel_identifier_v1::Event;

        state.toplevel_identifiers.answers[*asked] = match event {
            Event::Identifier { identifier } => IdentifierAnswer::Identifier(identifier),
            Event::Gone => IdentifierAnswer::Gone,
        };
    }
}
