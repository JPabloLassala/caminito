//! Bluetooth's switch (255), which the popup and the window both show: off is the rfkill soft
//! block on every Bluetooth adapter, and on lifts it and powers them all. Every adapter is
//! switched together.

use std::fmt;

use crate::bluez::{self, Bluez};
use crate::rfkill::{self, Rfkill};

/// Turning Bluetooth on or off, failed.
#[derive(Debug)]
pub enum Error {
    /// The rfkill block couldn't be read or changed.
    Rfkill(rfkill::Error),
    /// An adapter couldn't be powered.
    Power(bluez::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Rfkill(err) => err.fmt(f),
            Error::Power(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Rfkill(err) => Some(err),
            Error::Power(err) => Some(err),
        }
    }
}

/// Whether Bluetooth is on: no Bluetooth switch is soft-blocked.
pub fn is_on(rfkill: &dyn Rfkill) -> Result<bool, Error> {
    rfkill
        .blocks()
        .map(|blocks| !blocks.soft)
        .map_err(Error::Rfkill)
}

/// Turns Bluetooth on or off. On also powers every adapter BlueZ has, since an adapter
/// that was off before the block stays off after it's lifted.
pub async fn set_on(bluez: &Bluez, rfkill: &dyn Rfkill, on: bool) -> Result<(), Error> {
    rfkill.set_soft_blocked(!on).map_err(Error::Rfkill)?;
    if !on {
        return Ok(());
    }
    for adapter in bluez.adapters() {
        if !adapter.powered {
            bluez
                .set_powered(&adapter.path, true)
                .await
                .map_err(Error::Power)?;
        }
    }
    Ok(())
}
