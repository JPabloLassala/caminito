//! Each CRTC's gamma table, which Night Light writes (ADR 0023).
//!
//! Adapted from niri's `src/backend/tty.rs` (`GammaProps`, `set_gamma`, `restore_gamma`):
//! the atomic `GAMMA_LUT` property where the CRTC has it, as a property blob set outside
//! DrmCompositor's frame commits, which never touch it, with the old blob destroyed; the
//! legacy gamma call otherwise, which can't reset a table, so neutral is written out as a
//! linear ramp; and neither where the CRTC has no gamma.
//!
//! The table is the CRTC's, not the output's: an output that moves to another CRTC starts
//! on a table that was never written.

use std::io;
use std::os::fd::AsFd;

use smithay::backend::drm::DrmDevice;
use smithay::reexports::drm::control::{Device as _, crtc, property};

use crate::night_light::{NEUTRAL, ramp};

/// One CRTC's gamma table: how it's set, what Night Light wants on it, and what was written.
pub(super) struct CrtcGamma {
    way: Way,
    /// The temperature Night Light wants, in kelvin.
    wanted: u32,
    /// The temperature last written, or tried, while Caminito knows the table has it. A
    /// write that failed isn't tried again until Night Light wants another, so it doesn't
    /// fail at every refresh pass.
    written: Option<u32>,
    /// Whether the last write failed, which is said once until one doesn't.
    failing: bool,
    /// Whether it was said that this CRTC has no gamma, which is said once.
    said_none: bool,
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
            written: None,
            failing: false,
            said_none: false,
        };
        // A CRTC that drives nothing may refuse it; it's written again when it's lit.
        match gamma.set_table(device, crtc, NEUTRAL) {
            Ok(()) => gamma.written = Some(NEUTRAL),
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

    /// Writes the table Night Light wants on `crtc`, for the output `name`, if it doesn't
    /// have it. The caller holds the DRM master: while the session is paused, `want` keeps
    /// the temperature for the resume.
    pub(super) fn write(&mut self, device: &DrmDevice, crtc: crtc::Handle, name: &str) {
        if self.written == Some(self.wanted) {
            return;
        }
        self.written = Some(self.wanted);
        if let Way::None = self.way {
            if !self.said_none {
                tracing::warn!("{name} has no gamma table, so Night Light can't tint it");
                self.said_none = true;
            }
            return;
        }
        match self.set_table(device, crtc, self.wanted) {
            Ok(()) => self.failing = false,
            Err(err) if !self.failing => {
                tracing::warn!("can't set the gamma table of {name}: {err}");
                self.failing = true;
            }
            Err(err) => tracing::debug!("can't set the gamma table of {name}: {err}"),
        }
    }

    fn set_table(&mut self, device: &DrmDevice, crtc: crtc::Handle, kelvin: u32) -> io::Result<()> {
        match &mut self.way {
            Way::Lut {
                property,
                size,
                blob,
            } => {
                let table = (kelvin != NEUTRAL).then(|| ramp(kelvin, *size));
                set_lut(device, crtc, *property, blob, table.as_deref())
            }
            Way::Legacy { size } => {
                let table = ramp(kelvin, *size);
                let (red, rest) = table.split_at(*size);
                let (green, blue) = rest.split_at(*size);
                device.set_gamma(crtc, red, green, blue)
            }
            Way::None => Ok(()),
        }
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
