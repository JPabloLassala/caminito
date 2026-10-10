//! wlr-gamma-control (141): a gamma client, as gammastep and wlsunset are, that holds an
//! output's gamma table and sets it from a file.

use std::fs::File;
use std::io::Write;
use std::os::fd::{AsFd, OwnedFd};

use rustix::fs::{MemfdFlags, memfd_create};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols_wlr::gamma_control::v1::client::zwlr_gamma_control_manager_v1::{
    self, ZwlrGammaControlManagerV1,
};
use wayland_protocols_wlr::gamma_control::v1::client::zwlr_gamma_control_v1::{
    self, ZwlrGammaControlV1,
};

use crate::{Client, State};

/// The index of one of a client's gamma controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GammaControlId(usize);

/// A gamma control, and what it was told.
pub struct GammaControl {
    control: ZwlrGammaControlV1,
    /// The size of each ramp, from `gamma_size`.
    pub size: Option<u32>,
    /// Whether it was told `failed`.
    pub failed: bool,
}

impl Client {
    /// Asks for the gamma control of the `index`th output it bound. Roundtrip before reading
    /// its size.
    pub fn get_gamma_control(&mut self, index: usize) -> GammaControlId {
        let id = GammaControlId(self.state.gamma_controls.len());
        let manager: ZwlrGammaControlManagerV1 =
            self.global(|state| &mut state.gamma_control_manager, 1);
        let output = &self.state.outputs[index].proxy;
        let control = manager.get_gamma_control(output, &self.qh, id);
        self.state.gamma_controls.push(GammaControl {
            control,
            size: None,
            failed: false,
        });
        id
    }

    /// The gamma control, and what it was told.
    pub fn gamma_control(&self, id: GammaControlId) -> &GammaControl {
        &self.state.gamma_controls[id.0]
    }

    /// Sets the gamma table to `table`, red's ramp then green's then blue's, written to a
    /// file as gammastep does.
    pub fn set_gamma(&self, id: GammaControlId, table: &[u16]) {
        self.set_gamma_from(id, gamma_file(table));
    }

    /// Sets the gamma table from `fd`, whatever it holds.
    pub fn set_gamma_from(&self, id: GammaControlId, fd: impl AsFd) {
        self.state.gamma_controls[id.0]
            .control
            .set_gamma(fd.as_fd());
    }

    /// Destroys the gamma control, which gives the table back if it's still valid.
    pub fn destroy_gamma_control(&self, id: GammaControlId) {
        self.state.gamma_controls[id.0].control.destroy();
    }
}

/// A file holding `table` as native-endian `u16`s.
pub fn gamma_file(table: &[u16]) -> OwnedFd {
    let fd = memfd_create("gamma", MemfdFlags::CLOEXEC).unwrap();
    let mut file = File::from(fd);
    let bytes: Vec<u8> = table.iter().flat_map(|entry| entry.to_ne_bytes()).collect();
    file.write_all(&bytes).unwrap();
    file.into()
}

impl Dispatch<ZwlrGammaControlV1, GammaControlId> for State {
    fn event(
        state: &mut Self,
        _: &ZwlrGammaControlV1,
        event: zwlr_gamma_control_v1::Event,
        id: &GammaControlId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let control = &mut state.gamma_controls[id.0];
        match event {
            zwlr_gamma_control_v1::Event::GammaSize { size } => control.size = Some(size),
            zwlr_gamma_control_v1::Event::Failed => control.failed = true,
            _ => {}
        }
    }
}

impl Dispatch<ZwlrGammaControlManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwlrGammaControlManagerV1,
        _: zwlr_gamma_control_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
