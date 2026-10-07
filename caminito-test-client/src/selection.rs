//! The clipboard (`wl_data_device`) and the primary selection (`zwp_primary_selection_v1`)
//! as a Wayland client sees them: it offers text from memory, and reads what the
//! compositor offers it through a pipe. It drags text too, with an icon if asked.

use std::io::Write;
use std::os::fd::{AsFd, OwnedFd};
use std::sync::Mutex;

use wayland_client::protocol::wl_callback::{self, WlCallback};
use wayland_client::protocol::wl_data_device::{self, WlDataDevice};
use wayland_client::protocol::wl_data_device_manager::{self, DndAction, WlDataDeviceManager};
use wayland_client::protocol::wl_data_offer::{self, WlDataOffer};
use wayland_client::protocol::wl_data_source::{self, WlDataSource};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, event_created_child};
use wayland_protocols::wp::primary_selection::zv1::client::zwp_primary_selection_device_manager_v1::{
    self, ZwpPrimarySelectionDeviceManagerV1,
};
use wayland_protocols::wp::primary_selection::zv1::client::zwp_primary_selection_device_v1::{
    self, ZwpPrimarySelectionDeviceV1,
};
use wayland_protocols::wp::primary_selection::zv1::client::zwp_primary_selection_offer_v1::{
    self, ZwpPrimarySelectionOfferV1,
};
use wayland_protocols::wp::primary_selection::zv1::client::zwp_primary_selection_source_v1::{
    self, ZwpPrimarySelectionSourceV1,
};

use crate::{Buffer, Client, State, WindowId};

/// The mime type text is offered as, which Xwayland's `UTF8_STRING` maps to.
pub const TEXT: &str = "text/plain;charset=utf-8";

/// Which selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    Clipboard,
    Primary,
}

/// The mime types an offer lists, as they arrive.
pub(crate) type MimeTypes = Mutex<Vec<String>>;

/// The selections the compositor last offered the client.
#[derive(Default)]
pub(crate) struct Offers {
    clipboard: Option<WlDataOffer>,
    primary: Option<ZwpPrimarySelectionOfferV1>,
}

/// A drag's icon as [`Client::start_drag`] draws it.
#[derive(Debug, Clone, Copy)]
pub struct DragIconSpec {
    /// In surface pixels, at buffer scale 1.
    pub size: (i32, i32),
    /// What it's filled with.
    pub argb: u32,
    /// Where its top left corner is from the pointer, set with `wl_surface.offset` before
    /// the drag starts, as GTK does.
    pub offset: (i32, i32),
}

/// The icon surface of the client's last drag, and its buffer, which lives as long as it.
pub(crate) struct DragIcon {
    surface: WlSurface,
    _buffer: Buffer,
}

/// A drag icon's frame callback's user data.
struct DragIconFrame;

impl Client {
    /// Starts a drag of `text` from `origin`, as if `serial`'s button press started it,
    /// with `icon` drawn under the pointer if it's given.
    pub fn start_drag(
        &mut self,
        origin: WindowId,
        serial: u32,
        text: &str,
        icon: Option<DragIconSpec>,
    ) {
        self.bind_selections();
        let manager: WlDataDeviceManager = self.global(|s| &mut s.data_device_manager, 3);
        let source = manager.create_data_source(&self.qh, text.to_owned());
        source.offer(TEXT.to_owned());
        source.set_actions(DndAction::Copy);
        let icon = icon.map(|spec| {
            let surface = self.compositor().create_surface(&self.qh, ());
            let (pool, buffer) = self.shm_buffer(spec.size, spec.argb);
            surface.attach(Some(&buffer), 0, 0);
            surface.offset(spec.offset.0, spec.offset.1);
            surface.damage_buffer(0, 0, spec.size.0, spec.size.1);
            surface.commit();
            DragIcon {
                surface,
                _buffer: Buffer { buffer, pool },
            }
        });
        self.state.data_device.as_ref().unwrap().start_drag(
            Some(&source),
            &self.window(origin).surface,
            icon.as_ref().map(|icon| &icon.surface),
            serial,
        );
        self.state.drag_icon = icon;
    }

    /// Moves the last drag's icon by `(dx, dy)` with `wl_surface.offset`, and commits.
    pub fn offset_drag_icon(&self, dx: i32, dy: i32) {
        let surface = &self.drag_icon().surface;
        surface.offset(dx, dy);
        surface.commit();
    }

    /// Asks for a frame callback on the drag icon, and commits.
    pub fn request_drag_icon_frame(&self) {
        let surface = &self.drag_icon().surface;
        surface.frame(&self.qh, DragIconFrame);
        surface.commit();
    }

    fn drag_icon(&self) -> &DragIcon {
        self.state.drag_icon.as_ref().expect("a drag with an icon")
    }

    /// Makes the seat's data device and primary selection device, so the client is told of
    /// the selections while it has the keyboard.
    pub fn bind_selections(&mut self) {
        let seat = self.seat();
        if self.state.data_device.is_none() {
            let manager: WlDataDeviceManager = self.global(|s| &mut s.data_device_manager, 3);
            self.state.data_device = Some(manager.get_data_device(&seat, &self.qh, ()));
        }
        if self.state.primary_device.is_none() {
            let manager: ZwpPrimarySelectionDeviceManagerV1 =
                self.global(|s| &mut s.primary_manager, 1);
            self.state.primary_device = Some(manager.get_device(&seat, &self.qh, ()));
        }
    }

    /// Sets `selection` to `text`, offered as [`TEXT`]. The compositor only takes it while
    /// the client has the keyboard; it doesn't check the serial.
    pub fn set_selection(&mut self, selection: Selection, text: &str) {
        self.bind_selections();
        let text = text.to_owned();
        match selection {
            Selection::Clipboard => {
                let manager: WlDataDeviceManager = self.global(|s| &mut s.data_device_manager, 3);
                let source = manager.create_data_source(&self.qh, text);
                source.offer(TEXT.to_owned());
                self.state
                    .data_device
                    .as_ref()
                    .unwrap()
                    .set_selection(Some(&source), 0);
            }
            Selection::Primary => {
                let manager: ZwpPrimarySelectionDeviceManagerV1 =
                    self.global(|s| &mut s.primary_manager, 1);
                let source = manager.create_source(&self.qh, text);
                source.offer(TEXT.to_owned());
                self.state
                    .primary_device
                    .as_ref()
                    .unwrap()
                    .set_selection(Some(&source), 0);
            }
        }
    }

    /// Asks for the offered selection as `mime_type`, and returns the pipe's read end, which
    /// reaches its end once the selection's owner has written it all.
    pub fn receive(&self, selection: Selection, mime_type: &str) -> OwnedFd {
        let (read, write) = rustix::pipe::pipe().expect("a pipe");
        let offers = &self.state.offers;
        match selection {
            Selection::Clipboard => offers
                .clipboard
                .as_ref()
                .expect("a clipboard offer")
                .receive(mime_type.to_owned(), write.as_fd()),
            Selection::Primary => offers
                .primary
                .as_ref()
                .expect("a primary selection offer")
                .receive(mime_type.to_owned(), write.as_fd()),
        }
        self.flush();
        read
    }
}

impl State {
    /// The mime types of the selection the compositor last offered, or `None` if it offers
    /// none.
    pub fn offered(&self, selection: Selection) -> Option<Vec<String>> {
        let mime_types = match selection {
            Selection::Clipboard => self.offers.clipboard.as_ref()?.data::<MimeTypes>(),
            Selection::Primary => self.offers.primary.as_ref()?.data::<MimeTypes>(),
        };
        Some(mime_types?.lock().unwrap().clone())
    }
}

/// Writes a source's text to the pipe a reader sent, which closes it.
pub(crate) fn send(text: &str, fd: OwnedFd) {
    let mut file = std::fs::File::from(fd);
    let _ = file.write_all(text.as_bytes());
}

impl Dispatch<WlDataDevice, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlDataDevice,
        event: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_device::Event::Selection { id } = event
            && let Some(old) = std::mem::replace(&mut state.offers.clipboard, id)
        {
            old.destroy();
        }
    }

    event_created_child!(State, WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (WlDataOffer, MimeTypes::default()),
    ]);
}

impl Dispatch<WlDataOffer, MimeTypes> for State {
    fn event(
        _: &mut Self,
        _: &WlDataOffer,
        event: wl_data_offer::Event,
        mime_types: &MimeTypes,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_offer::Event::Offer { mime_type } = event {
            mime_types.lock().unwrap().push(mime_type);
        }
    }
}

impl Dispatch<WlDataSource, String> for State {
    fn event(
        _: &mut Self,
        source: &WlDataSource,
        event: wl_data_source::Event,
        text: &String,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_source::Event::Send { fd, .. } => send(text, fd),
            wl_data_source::Event::Cancelled => source.destroy(),
            _ => {}
        }
    }
}

impl Dispatch<WlCallback, DragIconFrame> for State {
    fn event(
        state: &mut Self,
        _: &WlCallback,
        event: wl_callback::Event,
        _: &DragIconFrame,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { .. } = event {
            state.drag_icon_frames_done += 1;
        }
    }
}

impl Dispatch<ZwpPrimarySelectionDeviceV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ZwpPrimarySelectionDeviceV1,
        event: zwp_primary_selection_device_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwp_primary_selection_device_v1::Event::Selection { id } = event
            && let Some(old) = std::mem::replace(&mut state.offers.primary, id)
        {
            old.destroy();
        }
    }

    event_created_child!(State, ZwpPrimarySelectionDeviceV1, [
        zwp_primary_selection_device_v1::EVT_DATA_OFFER_OPCODE => (ZwpPrimarySelectionOfferV1, MimeTypes::default()),
    ]);
}

impl Dispatch<ZwpPrimarySelectionOfferV1, MimeTypes> for State {
    fn event(
        _: &mut Self,
        _: &ZwpPrimarySelectionOfferV1,
        event: zwp_primary_selection_offer_v1::Event,
        mime_types: &MimeTypes,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwp_primary_selection_offer_v1::Event::Offer { mime_type } = event {
            mime_types.lock().unwrap().push(mime_type);
        }
    }
}

impl Dispatch<ZwpPrimarySelectionSourceV1, String> for State {
    fn event(
        _: &mut Self,
        source: &ZwpPrimarySelectionSourceV1,
        event: zwp_primary_selection_source_v1::Event,
        text: &String,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwp_primary_selection_source_v1::Event::Send { fd, .. } => send(text, fd),
            zwp_primary_selection_source_v1::Event::Cancelled => source.destroy(),
            _ => {}
        }
    }
}

impl Dispatch<WlDataDeviceManager, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlDataDeviceManager,
        _: wl_data_device_manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpPrimarySelectionDeviceManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwpPrimarySelectionDeviceManagerV1,
        _: zwp_primary_selection_device_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
