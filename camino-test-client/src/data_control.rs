//! A data-control client, as a clipboard manager is one (68): it's offered every selection
//! whether or not it has the keyboard, reads it through a pipe, and sets it from memory. It
//! speaks `ext_data_control_v1` or `wlr_data_control_unstable_v1`, which differ in name only.

use std::os::fd::{AsFd, OwnedFd};

use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, event_created_child};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_device_v1::{
    self, ExtDataControlDeviceV1,
};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_manager_v1::{
    self, ExtDataControlManagerV1,
};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_offer_v1::{
    self, ExtDataControlOfferV1,
};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_source_v1::{
    self, ExtDataControlSourceV1,
};
use wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_device_v1::{
    self, ZwlrDataControlDeviceV1,
};
use wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_manager_v1::{
    self, ZwlrDataControlManagerV1,
};
use wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_offer_v1::{
    self, ZwlrDataControlOfferV1,
};
use wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_source_v1::{
    self, ZwlrDataControlSourceV1,
};

use crate::selection::{MimeTypes, Selection, TEXT, send};
use crate::{Client, State};

/// Which data-control protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Ext,
    Wlr,
}

/// A protocol's manager and the seat's device, made together.
enum Bound {
    Ext(ExtDataControlManagerV1, ExtDataControlDeviceV1),
    Wlr(ZwlrDataControlManagerV1, ZwlrDataControlDeviceV1),
}

enum Offer {
    Ext(ExtDataControlOfferV1),
    Wlr(ZwlrDataControlOfferV1),
}

impl Offer {
    fn mime_types(&self) -> Option<&MimeTypes> {
        match self {
            Offer::Ext(offer) => offer.data(),
            Offer::Wlr(offer) => offer.data(),
        }
    }

    fn receive(&self, mime_type: String, fd: &OwnedFd) {
        match self {
            Offer::Ext(offer) => offer.receive(mime_type, fd.as_fd()),
            Offer::Wlr(offer) => offer.receive(mime_type, fd.as_fd()),
        }
    }

    fn destroy(&self) {
        match self {
            Offer::Ext(offer) => offer.destroy(),
            Offer::Wlr(offer) => offer.destroy(),
        }
    }
}

/// The client's data-control device, and the selections it was last offered.
#[derive(Default)]
pub(crate) struct DataControl {
    bound: Option<Bound>,
    clipboard: Option<Offer>,
    primary: Option<Offer>,
}

impl DataControl {
    fn offer(&self, selection: Selection) -> Option<&Offer> {
        match selection {
            Selection::Clipboard => self.clipboard.as_ref(),
            Selection::Primary => self.primary.as_ref(),
        }
    }

    /// The compositor offers `offer` as `selection`, or nothing. The offer it replaces is
    /// destroyed, as the protocol asks.
    fn set_offer(&mut self, selection: Selection, offer: Option<Offer>) {
        let slot = match selection {
            Selection::Clipboard => &mut self.clipboard,
            Selection::Primary => &mut self.primary,
        };
        if let Some(old) = std::mem::replace(slot, offer) {
            old.destroy();
        }
    }
}

impl Client {
    /// Binds `protocol`'s manager and makes the seat's data-control device. Roundtrip before
    /// reading what it's offered.
    pub fn bind_data_control(&mut self, protocol: Protocol) {
        let seat = self.seat();
        let (registry, globals, qh) = (&self.registry, &self.state.globals, &self.qh);
        let bound = match protocol {
            Protocol::Ext => {
                let manager: ExtDataControlManagerV1 = crate::bind(registry, globals, qh, 1);
                let device = manager.get_data_device(&seat, qh, ());
                Bound::Ext(manager, device)
            }
            Protocol::Wlr => {
                // Version 2 added the primary selection.
                let manager: ZwlrDataControlManagerV1 = crate::bind(registry, globals, qh, 2);
                let device = manager.get_data_device(&seat, qh, ());
                Bound::Wlr(manager, device)
            }
        };
        self.state.data_control.bound = Some(bound);
    }

    /// Sets `selection` to `text`, offered as [`TEXT`], through data-control, which needs
    /// neither the keyboard nor a serial.
    pub fn set_selection_through_data_control(&mut self, selection: Selection, text: &str) {
        let bound = self.state.data_control.bound.as_ref();
        let text = text.to_owned();
        match bound.expect("bind_data_control first") {
            Bound::Ext(manager, device) => {
                let source = manager.create_data_source(&self.qh, text);
                source.offer(TEXT.to_owned());
                match selection {
                    Selection::Clipboard => device.set_selection(Some(&source)),
                    Selection::Primary => device.set_primary_selection(Some(&source)),
                }
            }
            Bound::Wlr(manager, device) => {
                let source = manager.create_data_source(&self.qh, text);
                source.offer(TEXT.to_owned());
                match selection {
                    Selection::Clipboard => device.set_selection(Some(&source)),
                    Selection::Primary => device.set_primary_selection(Some(&source)),
                }
            }
        }
    }

    /// Asks data-control for the offered selection as `mime_type`, and returns the pipe's
    /// read end, which reaches its end once the selection's owner has written it all.
    pub fn receive_through_data_control(&self, selection: Selection, mime_type: &str) -> OwnedFd {
        let (read, write) = rustix::pipe::pipe().expect("a pipe");
        let offer = self.state.data_control.offer(selection);
        offer
            .expect("a data-control offer")
            .receive(mime_type.to_owned(), &write);
        self.flush();
        read
    }
}

impl State {
    /// The mime types of the selection data-control last offered, or `None` if it offers
    /// none.
    pub fn offered_through_data_control(&self, selection: Selection) -> Option<Vec<String>> {
        let mime_types = self.data_control.offer(selection)?.mime_types()?;
        Some(mime_types.lock().unwrap().clone())
    }
}

/// The client's side of one data-control protocol.
macro_rules! data_control_client {
    (
        $protocol:ident,
        $manager_mod:ident::$manager:ident,
        $device_mod:ident::$device:ident,
        $offer_mod:ident::$offer:ident,
        $source_mod:ident::$source:ident
    ) => {
        impl Dispatch<$manager, ()> for State {
            fn event(
                _: &mut Self,
                _: &$manager,
                _: $manager_mod::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
            }
        }

        impl Dispatch<$device, ()> for State {
            fn event(
                state: &mut Self,
                _: &$device,
                event: $device_mod::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
                let offer = |id: Option<$offer>| id.map(Offer::$protocol);
                match event {
                    $device_mod::Event::Selection { id } => {
                        (state.data_control).set_offer(Selection::Clipboard, offer(id));
                    }
                    $device_mod::Event::PrimarySelection { id } => {
                        (state.data_control).set_offer(Selection::Primary, offer(id));
                    }
                    _ => {}
                }
            }

            event_created_child!(State, $device, [
                $device_mod::EVT_DATA_OFFER_OPCODE => ($offer, MimeTypes::default()),
            ]);
        }

        impl Dispatch<$offer, MimeTypes> for State {
            fn event(
                _: &mut Self,
                _: &$offer,
                event: $offer_mod::Event,
                mime_types: &MimeTypes,
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
                if let $offer_mod::Event::Offer { mime_type } = event {
                    mime_types.lock().unwrap().push(mime_type);
                }
            }
        }

        impl Dispatch<$source, String> for State {
            fn event(
                _: &mut Self,
                source: &$source,
                event: $source_mod::Event,
                text: &String,
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
                match event {
                    $source_mod::Event::Send { fd, .. } => send(text, fd),
                    $source_mod::Event::Cancelled => source.destroy(),
                    _ => {}
                }
            }
        }
    };
}

data_control_client!(
    Ext,
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_device_v1::ExtDataControlDeviceV1,
    ext_data_control_offer_v1::ExtDataControlOfferV1,
    ext_data_control_source_v1::ExtDataControlSourceV1
);
data_control_client!(
    Wlr,
    zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
    zwlr_data_control_device_v1::ZwlrDataControlDeviceV1,
    zwlr_data_control_offer_v1::ZwlrDataControlOfferV1,
    zwlr_data_control_source_v1::ZwlrDataControlSourceV1
);
