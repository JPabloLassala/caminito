//! Bluetooth's rfkill soft block (255), which is what off means: it lasts across reboots
//! through systemd-rfkill, and it agrees with the airplane-mode key. The block is read and
//! set through [`Rfkill`], the seam the tests fake; [`SystemRfkill`] is the kernel's.

use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The kernel's type for Bluetooth, `RFKILL_TYPE_BLUETOOTH` in `<linux/rfkill.h>`.
const TYPE_BLUETOOTH: u8 = 2;
/// `RFKILL_OP_CHANGE_ALL`: the event changes every switch of its type.
const OP_CHANGE_ALL: u8 = 3;

/// The Bluetooth switches' blocks, all of them taken together.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Blocks {
    /// Whether software has turned Bluetooth off: any of its switches is soft-blocked.
    pub soft: bool,
    /// Whether a hardware switch has, which software can't lift.
    pub hard: bool,
}

/// An rfkill call that failed.
#[derive(Debug)]
pub enum Error {
    /// The switches couldn't be read from sysfs.
    Read(io::Error),
    /// The block couldn't be written to the rfkill device.
    Write(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Read(err) => write!(f, "can't read the rfkill switches: {err}"),
            Error::Write(err) => write!(f, "can't change the rfkill block: {err}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Read(err) | Error::Write(err) => Some(err),
        }
    }
}

/// Reads and sets the soft block on every Bluetooth switch together.
pub trait Rfkill {
    /// The switches' blocks. With no Bluetooth switch, neither is blocked.
    fn blocks(&self) -> Result<Blocks, Error>;

    /// Blocks every Bluetooth switch, or lifts the block from them all.
    fn set_soft_blocked(&self, blocked: bool) -> Result<(), Error>;
}

/// The kernel's rfkill: the switches read from sysfs, and the block written to the rfkill
/// device, which logind opens to the user of the seat.
#[derive(Clone, Debug)]
pub struct SystemRfkill {
    switches: PathBuf,
    device: PathBuf,
}

impl SystemRfkill {
    /// The kernel's own, at `/sys/class/rfkill` and `/dev/rfkill`.
    pub fn new() -> SystemRfkill {
        SystemRfkill::at("/sys/class/rfkill", "/dev/rfkill")
    }

    /// Switches under `switches` laid out as `/sys/class/rfkill` is, and a device that
    /// takes `/dev/rfkill`'s events.
    pub fn at(switches: impl Into<PathBuf>, device: impl Into<PathBuf>) -> SystemRfkill {
        SystemRfkill {
            switches: switches.into(),
            device: device.into(),
        }
    }
}

impl Default for SystemRfkill {
    fn default() -> SystemRfkill {
        SystemRfkill::new()
    }
}

impl Rfkill for SystemRfkill {
    fn blocks(&self) -> Result<Blocks, Error> {
        let entries = match fs::read_dir(&self.switches) {
            Ok(entries) => entries,
            // A kernel without rfkill has no switch to block.
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Blocks::default()),
            Err(err) => return Err(Error::Read(err)),
        };
        let mut blocks = Blocks::default();
        for entry in entries {
            let switch = entry.map_err(Error::Read)?.path();
            if read(&switch, "type")? != "bluetooth" {
                continue;
            }
            blocks.soft |= read(&switch, "soft")? == "1";
            blocks.hard |= read(&switch, "hard")? == "1";
        }
        Ok(blocks)
    }

    fn set_soft_blocked(&self, blocked: bool) -> Result<(), Error> {
        let mut device = fs::OpenOptions::new()
            .write(true)
            .open(&self.device)
            .map_err(Error::Write)?;
        device.write_all(&event(blocked)).map_err(Error::Write)
    }
}

/// A switch's attribute, without its newline.
fn read(switch: &Path, attribute: &str) -> Result<String, Error> {
    let value = fs::read_to_string(switch.join(attribute)).map_err(Error::Read)?;
    Ok(value.trim_end().to_owned())
}

/// The `struct rfkill_event` that soft-blocks every Bluetooth switch, or unblocks them: its
/// first version, of 8 bytes, which every kernel since 2.6.31 takes.
fn event(blocked: bool) -> [u8; 8] {
    let index = 0u32.to_ne_bytes();
    [
        index[0],
        index[1],
        index[2],
        index[3],
        TYPE_BLUETOOTH,
        OP_CHANGE_ALL,
        u8::from(blocked),
        0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn switch(root: &Path, name: &str, kind: &str, soft: bool, hard: bool) {
        let switch = root.join(name);
        fs::create_dir(&switch).unwrap();
        fs::write(switch.join("type"), format!("{kind}\n")).unwrap();
        fs::write(switch.join("soft"), format!("{}\n", u8::from(soft))).unwrap();
        fs::write(switch.join("hard"), format!("{}\n", u8::from(hard))).unwrap();
    }

    #[test]
    fn only_bluetooth_switches_count_and_any_blocked_one_blocks_bluetooth() {
        let root = tempfile::tempdir().unwrap();
        switch(root.path(), "rfkill0", "wlan", true, true);
        switch(root.path(), "rfkill1", "bluetooth", false, false);
        let rfkill = SystemRfkill::at(root.path(), root.path().join("device"));
        assert_eq!(rfkill.blocks().unwrap(), Blocks::default());

        switch(root.path(), "rfkill2", "bluetooth", true, false);
        assert_eq!(
            rfkill.blocks().unwrap(),
            Blocks {
                soft: true,
                hard: false
            }
        );
    }

    #[test]
    fn without_rfkill_nothing_is_blocked() {
        let root = tempfile::tempdir().unwrap();
        let rfkill = SystemRfkill::at(root.path().join("absent"), root.path().join("device"));
        assert_eq!(rfkill.blocks().unwrap(), Blocks::default());
    }

    #[test]
    fn the_block_is_one_change_all_event_for_bluetooth() {
        let root = tempfile::tempdir().unwrap();
        let device = root.path().join("device");
        fs::write(&device, b"").unwrap();
        let rfkill = SystemRfkill::at(root.path(), &device);

        rfkill.set_soft_blocked(true).unwrap();
        assert_eq!(fs::read(&device).unwrap(), [0, 0, 0, 0, 2, 3, 1, 0]);
        rfkill.set_soft_blocked(false).unwrap();
        assert_eq!(fs::read(&device).unwrap(), [0, 0, 0, 0, 2, 3, 0, 0]);
    }
}
