// SPDX-FileCopyrightText: Ivan Molodetskikh and the niri contributors
// SPDX-FileCopyrightText: 2025-2026 Juan Pablo Lassala
// SPDX-License-Identifier: GPL-3.0-or-later

//! Each CRTC's gamma table, which Night Light writes (ADR 0023).
//!
//! Adapted from niri's `src/backend/tty.rs` (`GammaProps`, `set_gamma`, `restore_gamma`):
//! the atomic `GAMMA_LUT` property where the CRTC has it, as a property blob set outside
//! DrmCompositor's frame commits, which never touch it, with the old blob destroyed; the
//! legacy gamma call otherwise, which can't reset a table, so neutral is written out as a
//! linear ramp; and neither where the CRTC has no gamma.
//!
//! A gamma client's table (141) is written in place of Night Light's while the client holds
//! the output, and is the one written again after a resume or a modeset.
//!
//! The table is the CRTC's, not the output's: an output that moves to another CRTC starts
//! on a table that was never written.

use std::io;
use std::os::fd::AsFd;

use smithay::backend::drm::DrmDevice;
use smithay::reexports::drm::control::{Device as _, crtc, property};

use crate::night_light::{NEUTRAL, ramp};

/// One CRTC's gamma table: how it's set, what Night Light or a gamma client wants on it, and
/// what was written.
pub(super) struct CrtcGamma {
    way: Way,
    /// The temperature Night Light wants, in kelvin.
    wanted: u32,
    /// A gamma client's table, red's ramp then green's then blue's, which is written in
    /// place of Night Light's while the client holds the output (141).
    client: Option<Vec<u16>>,
    /// What was last written, or tried, while Caminito knows the table has it. A write
    /// that failed isn't tried again until another is wanted, so it doesn't fail at every
    /// refresh pass.
    written: Option<Source>,
    /// Whether the last write failed, which is said once until one doesn't.
    failing: bool,
    /// Whether it was said that this CRTC has no gamma, which is said once.
    said_none: bool,
}

/// Whose table a CRTC has.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Source {
    /// Night Light's, at a temperature in kelvin.
    NightLight(u32),
    /// The gamma client's.
    Client,
}

/// How a CRTC's table is set.
enum Way {
    /// The atomic `GAMMA_LUT` property, of `size` entries, and the blob it was last set to.
    Lut {
        property: property::Handle,
        size: usize,
        blob: Option<u64>,
    },
    /// The legacy gamma call, of `size` entries.
    Legacy { size: usize },
    /// The CRTC has no gamma.
    None,
}

impl CrtcGamma {
    /// Finds how `crtc`'s table is set, and resets it to neutral: a previous compositor's
    /// table survives `DrmDevice::new`.
    pub(super) fn take(device: &DrmDevice, crtc: crtc::Handle) -> Self {
        let way = way(device, crtc).unwrap_or_else(|err| {
            tracing::debug!("can't read the gamma of CRTC {crtc:?}: {err}");
            Way::None
        });
        let mut gamma = CrtcGamma {
            way,
            wanted: NEUTRAL,
            client: None,
            written: None,
            failing: false,
            said_none: false,
        };
        // A CRTC that drives nothing may refuse it; it's written again when it's lit.
        match set_table(&mut gamma.way, device, crtc, Table::Kelvin(NEUTRAL)) {
            Ok(()) => gamma.written = Some(Source::NightLight(NEUTRAL)),
            Err(err) => tracing::debug!("can't reset the gamma table of CRTC {crtc:?}: {err}"),
        }
        gamma
    }

    /// Night Light wants `kelvin` on this CRTC.
    pub(super) fn want(&mut self, kelvin: u32) {
        self.wanted = kelvin;
    }

    /// The table may no longer be what was written: after a session resume, a modeset, or a
    /// new output on the CRTC. The next `write` writes it again.
    pub(super) fn forget_written(&mut self) {
        self.written = None;
    }

    /// The number of entries in each of the table's ramps, or `None` if the CRTC has no
    /// gamma.
    pub(super) fn size(&self) -> Option<usize> {
        match self.way {
            Way::Lut { size, .. } | Way::Legacy { size } => Some(size),
            Way::None => None,
        }
    }

    /// A gamma client's `table` is to be on this CRTC, or, with `None`, Night Light's again.
    /// The next `write` writes it. Returns whether the table fits the CRTC.
    pub(super) fn hand_to_client(&mut self, table: Option<Vec<u16>>) -> bool {
        if let Some(table) = &table
            && Some(table.len()) != self.size().map(|size| size * 3)
        {
            return false;
        }
        self.client = table;
        self.written = None;
        true
    }

    /// Writes the table wanted on `crtc`, the gamma client's or Night Light's, for the
    /// output `name`, if it doesn't have it. The caller holds the DRM master: while the
    /// session is paused, what's wanted is kept for the resume. Returns whether the table
    /// has it.
    pub(super) fn write(&mut self, device: &DrmDevice, crtc: crtc::Handle, name: &str) -> bool {
        let source = match self.client {
            Some(_) => Source::Client,
            None => Source::NightLight(self.wanted),
        };
        if self.written == Some(source) {
            return !self.failing;
        }
        self.written = Some(source);
        if let Way::None = self.way {
            if !self.said_none {
                tracing::warn!("{name} has no gamma table, so Night Light can't tint it");
                self.said_none = true;
            }
            return false;
        }
        let table = match &self.client {
            Some(table) => Table::Client(table),
            None => Table::Kelvin(self.wanted),
        };
        match set_table(&mut self.way, device, crtc, table) {
            Ok(()) => self.failing = false,
            Err(err) if !self.failing => {
                tracing::warn!("can't set the gamma table of {name}: {err}");
                self.failing = true;
            }
            Err(err) => tracing::debug!("can't set the gamma table of {name}: {err}"),
        }
        !self.failing
    }
}

/// A table to write.
#[derive(Clone, Copy)]
enum Table<'a> {
    /// Night Light's, at a temperature in kelvin.
    Kelvin(u32),
    /// A gamma client's, red's ramp then green's then blue's.
    Client(&'a [u16]),
}

fn set_table(
    way: &mut Way,
    device: &DrmDevice,
    crtc: crtc::Handle,
    table: Table,
) -> io::Result<()> {
    let ramps = |size| match table {
        Table::Kelvin(kelvin) => std::borrow::Cow::Owned(ramp(kelvin, size)),
        Table::Client(table) => std::borrow::Cow::Borrowed(table),
    };
    match way {
        Way::Lut {
            property,
            size,
            blob,
        } => {
            // Neutral needs no blob.
            let ramps = (!matches!(table, Table::Kelvin(NEUTRAL))).then(|| ramps(*size));
            set_lut(device, crtc, *property, blob, ramps.as_deref())
        }
        Way::Legacy { size } => {
            let ramps = ramps(*size);
            let (red, rest) = ramps.split_at(*size);
            let (green, blue) = rest.split_at(*size);
            device.set_gamma(crtc, red, green, blue)
        }
        Way::None => Ok(()),
    }
}

/// How `crtc`'s table is set: `GAMMA_LUT` with `GAMMA_LUT_SIZE` if it has them, else the
/// legacy call if its `gamma_length` isn't 0.
fn way(device: &DrmDevice, crtc: crtc::Handle) -> io::Result<Way> {
    let mut lut = None;
    let mut lut_size = None;
    for (&handle, &value) in device.get_properties(crtc)?.iter() {
        let info = device.get_property(handle)?;
        match info.name().to_bytes() {
            b"GAMMA_LUT" => lut = Some(handle),
            b"GAMMA_LUT_SIZE" => lut_size = Some(value as usize),
            _ => {}
        }
    }
    if let (Some(property), Some(size)) = (lut, lut_size)
        && size > 0
    {
        return Ok(Way::Lut {
            property,
            size,
            blob: None,
        });
    }
    let size = device.get_crtc(crtc)?.gamma_length() as usize;
    Ok(if size > 0 {
        Way::Legacy { size }
    } else {
        Way::None
    })
}

/// Sets `GAMMA_LUT` to a new blob of `table`, red's entries then green's then blue's, and
/// destroys the blob it had. Neutral, `None`, needs no blob: 0 is the kernel's linear ramp.
fn set_lut(
    device: &DrmDevice,
    crtc: crtc::Handle,
    property: property::Handle,
    blob: &mut Option<u64>,
    table: Option<&[u16]>,
) -> io::Result<()> {
    let new = if let Some(table) = table {
        let size = table.len() / 3;
        // `struct drm_color_lut`: red, green, blue and a reserved u16, each in the
        // machine's byte order.
        let (red, rest) = table.split_at(size);
        let (green, blue) = rest.split_at(size);
        let mut data = Vec::with_capacity(size * 8);
        for i in 0..size {
            for value in [red[i], green[i], blue[i], 0] {
                data.extend_from_slice(&value.to_ne_bytes());
            }
        }
        let created = drm_ffi::mode::create_property_blob(device.as_fd(), &mut data)?;
        Some(u64::from(created.blob_id))
    } else {
        None
    };
    let set = device.set_property(
        crtc,
        property,
        property::Value::Blob(new.unwrap_or(0)).into(),
    );
    if let Err(err) = set {
        if let Some(new) = new {
            let _ = device.destroy_property_blob(new);
        }
        return Err(err);
    }
    if let Some(old) = std::mem::replace(blob, new)
        && let Err(err) = device.destroy_property_blob(old)
    {
        tracing::debug!("can't destroy an old gamma blob: {err}");
    }
    Ok(())
}
