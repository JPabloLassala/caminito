//! `zwlr_gamma_control_manager_v1` version 1 (141, ADR 0023): a **gamma client**, such as
//! gammastep or wlsunset, holds an output's gamma table and sets it from a file.
//!
//! This module owns the protocol's objects and its rules: one live control per output, and
//! what makes a table unreadable. It knows nothing of gamma tables or Night Light. The
//! compositor offers the global only where outputs have tables, through
//! [`GammaControlState::offer`], answers through [`GammaControlHandler`], and fails the
//! controls of outputs that went in its refresh pass, through
//! [`GammaControlState::fail_gone`].
//!
//! Written from the protocol's XML, after niri's `src/protocols/gamma_control.rs` (at
//! d0cd58e) in its shape: the handler's three calls, the live controls by output, and a
//! failed control forgotten at once. None of niri's code was copied (03).
//! Unlike niri, the table is read with `pread` from the file's start, as wlroots'
//! `types/wlr_gamma_control_v1.c` does: a client needn't seek back after writing it, and a
//! pipe, which could block the loop, is refused.

use std::fmt;
use std::io;
use std::os::fd::AsFd;

use smithay::output::Output;
use smithay::reexports::wayland_protocols_wlr::gamma_control::v1::server::zwlr_gamma_control_manager_v1::{
    self, ZwlrGammaControlManagerV1,
};
use smithay::reexports::wayland_protocols_wlr::gamma_control::v1::server::zwlr_gamma_control_v1::{
    self, ZwlrGammaControlV1,
};
use smithay::reexports::wayland_server::backend::{ClientId, GlobalId};
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New,
};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use super::GlobalFilter;

/// The version Caminito offers.
const VERSION: u32 = 1;

/// What the compositor does for gamma clients.
pub trait GammaControlHandler: 'static {
    fn gamma_control_state(&mut self) -> &mut GammaControlState;

    /// The number of entries in each of `output`'s ramps, or `None` if it has no gamma
    /// table.
    fn gamma_size(&mut self, output: &Output) -> Option<u32>;

    /// Sets `output`'s table, red's ramp then green's then blue's, each of `gamma_size`
    /// entries, at once. Returns whether it was set.
    fn set_gamma(&mut self, output: &Output, table: Vec<u16>) -> bool;

    /// The gamma client let go of `output`: it destroyed its control, the control failed, or
    /// the client went. The table is the compositor's again.
    fn gamma_released(&mut self, output: &Output);
}

/// The global, once offered, and the live control holding each output.
#[derive(Debug, Default)]
pub struct GammaControlState {
    global: Option<GlobalId>,
    /// One control at most per output. A control that failed is no longer here.
    controls: Vec<(Output, ZwlrGammaControlV1)>,
}

/// The global's data.
pub struct GammaControlGlobalData {
    filter: GlobalFilter,
}

/// Each manager's data.
#[derive(Debug)]
pub struct GammaControlManagerData;

/// Each control's data: the size of its output's ramps, as `gamma_size` told it, or 0 for a
/// control that failed at once.
#[derive(Debug)]
pub struct GammaControlData {
    size: u32,
}

impl GammaControlState {
    /// Offers `zwlr_gamma_control_manager_v1` to the clients `filter` allows. Only a
    /// backend whose outputs have gamma tables does: the tty (ADR 0023).
    pub fn offer<D, F>(&mut self, display: &DisplayHandle, filter: F)
    where
        D: GlobalDispatch<ZwlrGammaControlManagerV1, GammaControlGlobalData> + 'static,
        F: for<'c> Fn(&'c Client) -> bool + Send + Sync + 'static,
    {
        if self.global.is_some() {
            return;
        }
        let data = GammaControlGlobalData {
            filter: Box::new(filter),
        };
        self.global = Some(display.create_global::<D, ZwlrGammaControlManagerV1, _>(VERSION, data));
    }

    /// Whether a gamma client holds `output`.
    pub fn holds(&self, output: &Output) -> bool {
        self.controls.iter().any(|(held, _)| held == output)
    }

    /// Fails the controls of outputs that aren't in `shown`, as the protocol has it for an
    /// output that went, and returns those outputs, which no client holds now.
    pub fn fail_gone(&mut self, shown: &[Output]) -> Vec<Output> {
        let mut gone = Vec::new();
        self.controls.retain(|(output, control)| {
            if shown.contains(output) {
                return true;
            }
            control.failed();
            gone.push(output.clone());
            false
        });
        gone
    }

    fn output_of(&self, control: &ZwlrGammaControlV1) -> Option<Output> {
        let held = self.controls.iter().find(|(_, held)| held == control);
        held.map(|(output, _)| output.clone())
    }

    /// Forgets `control`, and returns the output it held.
    fn forget(&mut self, control: &ZwlrGammaControlV1) -> Option<Output> {
        let output = self.output_of(control)?;
        self.controls.retain(|(_, held)| held != control);
        Some(output)
    }
}

impl<D> GlobalDispatch2<ZwlrGammaControlManagerV1, D> for GammaControlGlobalData
where
    D: Dispatch<ZwlrGammaControlManagerV1, GammaControlManagerData>,
{
    fn bind(
        &self,
        _state: &mut D,
        _display: &DisplayHandle,
        _client: &Client,
        resource: New<ZwlrGammaControlManagerV1>,
        data_init: &mut DataInit<'_, D>,
    ) {
        data_init.init(resource, GammaControlManagerData);
    }

    fn can_view(&self, client: &Client) -> bool {
        (self.filter)(client)
    }
}

impl<D> Dispatch2<ZwlrGammaControlManagerV1, D> for GammaControlManagerData
where
    D: GammaControlHandler + Dispatch<ZwlrGammaControlV1, GammaControlData>,
{
    fn request(
        &self,
        state: &mut D,
        _client: &Client,
        _resource: &ZwlrGammaControlManagerV1,
        request: zwlr_gamma_control_manager_v1::Request,
        _display: &DisplayHandle,
        data_init: &mut DataInit<'_, D>,
    ) {
        let zwlr_gamma_control_manager_v1::Request::GetGammaControl { id, output } = request else {
            // `destroy`, which leaves the controls as they are.
            return;
        };
        // An output that went, one with no table, or one another control holds.
        let output = Output::from_resource(&output)
            .filter(|output| !state.gamma_control_state().holds(output));
        let size = output
            .as_ref()
            .and_then(|output| state.gamma_size(output))
            .filter(|&size| size > 0);
        let (Some(output), Some(size)) = (output, size) else {
            data_init.init(id, GammaControlData { size: 0 }).failed();
            return;
        };
        let control = data_init.init(id, GammaControlData { size });
        control.gamma_size(size);
        state.gamma_control_state().controls.push((output, control));
    }
}

impl<D: GammaControlHandler> Dispatch2<ZwlrGammaControlV1, D> for GammaControlData {
    fn request(
        &self,
        state: &mut D,
        _client: &Client,
        control: &ZwlrGammaControlV1,
        request: zwlr_gamma_control_v1::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        let zwlr_gamma_control_v1::Request::SetGamma { fd } = request else {
            // `destroy`, and `destroyed` follows it.
            return;
        };
        // A control that failed holds nothing.
        let Some(output) = state.gamma_control_state().output_of(control) else {
            return;
        };
        let set = match read_table(fd, self.size) {
            Ok(table) => state.set_gamma(&output, table),
            Err(err) => {
                tracing::debug!("can't read a gamma table for {}: {err}", output.name());
                false
            }
        };
        if !set {
            control.failed();
            state.gamma_control_state().forget(control);
            state.gamma_released(&output);
        }
    }

    fn destroyed(&self, state: &mut D, _client: ClientId, control: &ZwlrGammaControlV1) {
        if let Some(output) = state.gamma_control_state().forget(control) {
            state.gamma_released(&output);
        }
    }
}

/// Why a client's gamma table can't be taken.
#[derive(Debug)]
enum ReadError {
    /// The file can't be read from its start: it isn't a file, or isn't open for reading.
    Io(io::Error),
    /// It has fewer entries than three ramps.
    Short,
    /// It has more.
    Long,
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::Io(err) => write!(f, "can't read the file: {err}"),
            ReadError::Short => f.write_str("the file is shorter than three ramps"),
            ReadError::Long => f.write_str("the file is longer than three ramps"),
        }
    }
}

impl std::error::Error for ReadError {}

/// Reads three ramps of `size` native-endian `u16`s from the start of `fd`, which must hold
/// exactly that.
fn read_table(fd: impl AsFd, size: u32) -> Result<Vec<u16>, ReadError> {
    let len = size as usize * 3 * size_of::<u16>();
    // One byte more than the table, to tell a longer file.
    let mut bytes = vec![0; len + 1];
    let mut read = 0;
    while read < bytes.len() {
        match rustix::io::pread(&fd, &mut bytes[read..], read as u64) {
            Ok(0) => break,
            Ok(n) => read += n,
            Err(rustix::io::Errno::INTR) => {}
            Err(err) => return Err(ReadError::Io(err.into())),
        }
    }
    if read < len {
        return Err(ReadError::Short);
    }
    if read > len {
        return Err(ReadError::Long);
    }
    let (entries, _) = bytes[..len].as_chunks::<2>();
    Ok(entries
        .iter()
        .map(|&entry| u16::from_ne_bytes(entry))
        .collect())
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Write;
    use std::os::fd::OwnedFd;

    use rustix::fs::{MemfdFlags, memfd_create};

    use super::*;

    /// A file holding `entries`, its offset at its end as the client's write left it.
    fn file_of(entries: &[u16]) -> OwnedFd {
        let fd = memfd_create("gamma", MemfdFlags::CLOEXEC).unwrap();
        let mut file = File::from(fd);
        let bytes: Vec<u8> = entries.iter().flat_map(|e| e.to_ne_bytes()).collect();
        file.write_all(&bytes).unwrap();
        file.into()
    }

    #[test]
    fn three_ramps_of_the_gamma_size_are_read_from_the_files_start() {
        let table = [0, 1, 2, 10, 11, 12, 20, 21, 22];
        assert_eq!(read_table(file_of(&table), 3).unwrap(), table);
    }

    #[test]
    fn a_short_ramp_is_refused() {
        let err = read_table(file_of(&[0, 1, 2, 10, 11, 12, 20, 21]), 3).unwrap_err();
        assert!(matches!(err, ReadError::Short), "{err}");
    }

    #[test]
    fn a_long_ramp_is_refused() {
        let err = read_table(file_of(&[0, 1, 2, 10, 11, 12, 20, 21, 22, 23]), 3).unwrap_err();
        assert!(matches!(err, ReadError::Long), "{err}");
    }

    #[test]
    fn an_unreadable_fd_is_refused() {
        // A pipe can't be read at an offset, so it can't hold the loop waiting for a write.
        let (reader, mut writer) = std::io::pipe().unwrap();
        writer.write_all(&[0; 18]).unwrap();
        let err = read_table(reader, 3).unwrap_err();
        assert!(matches!(err, ReadError::Io(_)), "{err}");
    }

    #[test]
    fn a_file_open_only_for_writing_is_refused() {
        let file = File::from(file_of(&[0; 9]));
        let path = format!("/proc/self/fd/{}", std::os::fd::AsRawFd::as_raw_fd(&file));
        let write_only = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        let err = read_table(write_only, 3).unwrap_err();
        assert!(matches!(err, ReadError::Io(_)), "{err}");
    }
}
