//! Bluetooth's switch (255), which the popup and the window both show: off is the rfkill soft
//! block on every Bluetooth adapter, and on lifts it and powers them all. Every adapter is
//! switched together.
//!
//! bluetoothd learns of a lifted block on its own time: until it has, powering an adapter
//! fails as blocked, and an adapter whose radio the block had switched off altogether only
//! comes back afterwards. So after on, the switch powers each adapter as it appears or
//! changes while unpowered, not only those BlueZ had when the block was lifted.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;

use gio::glib;

use crate::DOMAIN;
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

/// What on still owes the adapters.
#[derive(Default)]
struct Powering {
    /// Whether Bluetooth was last turned on, and not off since.
    on: bool,
    /// The adapters powered, or being powered, since. An adapter the user powers off
    /// afterwards stays off; one that goes and comes back is powered again.
    powered: BTreeSet<String>,
}

/// Bluetooth's switch over BlueZ's model and the rfkill block.
pub struct Switch {
    bluez: Bluez,
    rfkill: Rc<dyn Rfkill>,
    powering: Rc<RefCell<Powering>>,
}

impl Switch {
    /// The switch, which follows `bluez` to power the adapters that come after on.
    pub fn new(bluez: &Bluez, rfkill: Rc<dyn Rfkill>) -> Switch {
        let powering = Rc::new(RefCell::new(Powering::default()));
        bluez.connect_changed({
            let powering = Rc::downgrade(&powering);
            move |bluez| {
                if let Some(powering) = powering.upgrade() {
                    power_the_rest(bluez, &powering);
                }
            }
        });
        Switch {
            bluez: bluez.clone(),
            rfkill,
            powering,
        }
    }

    /// Whether Bluetooth is on: no Bluetooth switch is soft-blocked. A hard block alone
    /// doesn't make it off, since the switch can't lift it; [`rfkill::Blocks::hard`] says
    /// whether there's one.
    pub fn is_on(&self) -> Result<bool, Error> {
        self.rfkill
            .blocks()
            .map(|blocks| !blocks.soft)
            .map_err(Error::Rfkill)
    }

    /// Turns Bluetooth on or off. On powers every adapter BlueZ has, since one that was off
    /// before the block stays off after it's lifted, and then those that come after. An
    /// adapter bluetoothd still holds blocked is left for when it changes.
    pub async fn set_on(&self, on: bool) -> Result<(), Error> {
        self.rfkill.set_soft_blocked(!on).map_err(Error::Rfkill)?;
        *self.powering.borrow_mut() = Powering {
            on,
            powered: BTreeSet::new(),
        };
        if !on {
            return Ok(());
        }
        for adapter in self.bluez.adapters() {
            if adapter.powered
                || !self
                    .powering
                    .borrow_mut()
                    .powered
                    .insert(adapter.path.clone())
            {
                continue;
            }
            match self.bluez.set_powered(&adapter.path, true).await {
                Ok(()) => {}
                Err(err) if still_blocked(&err) => {
                    self.powering.borrow_mut().powered.remove(&adapter.path);
                }
                Err(err) => return Err(Error::Power(err)),
            }
        }
        Ok(())
    }
}

/// Powers each adapter that on still owes.
fn power_the_rest(bluez: &Bluez, powering: &Rc<RefCell<Powering>>) {
    let adapters = bluez.adapters();
    let mut owed = powering.borrow_mut();
    if !owed.on {
        return;
    }
    owed.powered
        .retain(|path| adapters.iter().any(|adapter| &adapter.path == path));
    for adapter in adapters {
        if adapter.powered || !owed.powered.insert(adapter.path.clone()) {
            continue;
        }
        let bluez = bluez.clone();
        let powering = Rc::downgrade(powering);
        glib::spawn_future_local(async move {
            let Err(err) = bluez.set_powered(&adapter.path, true).await else {
                return;
            };
            // Left to be tried again at the adapter's next change.
            if let Some(powering) = powering.upgrade() {
                powering.borrow_mut().powered.remove(&adapter.path);
            }
            if !still_blocked(&err) {
                glib::g_warning!(DOMAIN, "{err}");
            }
        });
    }
}

/// Whether BlueZ refused to power an adapter because it hasn't seen the block lifted yet.
fn still_blocked(err: &bluez::Error) -> bool {
    match err {
        bluez::Error::Call { source, .. } => {
            gio::DBusError::remote_error(source).as_deref() == Some("org.bluez.Error.Blocked")
        }
        bluez::Error::Path(_) => false,
    }
}
