//! Bluetooth's switch (255): off is the rfkill soft block, and on lifts it and powers every
//! adapter. The rfkill block is the tests' fake, and BlueZ is python-dbusmock's.

mod mock;

use std::cell::Cell;

use caminito_bluetooth::bluez::Bluez;
use caminito_bluetooth::rfkill::{self, Blocks, Rfkill};
use caminito_bluetooth::switch;

/// The kernel's rfkill, as far as the switch can tell.
#[derive(Default)]
struct FakeRfkill {
    soft: Cell<bool>,
    hard: Cell<bool>,
}

impl Rfkill for FakeRfkill {
    fn blocks(&self) -> Result<Blocks, rfkill::Error> {
        Ok(Blocks {
            soft: self.soft.get(),
            hard: self.hard.get(),
        })
    }

    fn set_soft_blocked(&self, blocked: bool) -> Result<(), rfkill::Error> {
        self.soft.set(blocked);
        Ok(())
    }
}

#[test]
fn off_is_the_soft_block_and_on_lifts_it_and_powers_every_adapter() {
    mock::run(|mock| {
        let first = mock.add_adapter("hci0", "laptop");
        let second = mock.add_adapter("hci1", "dongle");
        let bluez = Bluez::new(&mock.connection);
        mock.wait_until("BlueZ's objects", || bluez.is_ready());
        let rfkill = FakeRfkill::default();
        assert!(switch::is_on(&rfkill).unwrap());

        mock.block_on(switch::set_on(&bluez, &rfkill, false))
            .unwrap();
        assert!(rfkill.soft.get());
        assert!(!switch::is_on(&rfkill).unwrap());

        // An adapter powered off before the block stays off after it, unless on powers it.
        mock.block_on(bluez.set_powered(&first, false)).unwrap();
        mock.block_on(bluez.set_powered(&second, false)).unwrap();
        mock.wait_until("both adapters to power off", || {
            bluez.adapters().iter().all(|adapter| !adapter.powered)
        });

        mock.block_on(switch::set_on(&bluez, &rfkill, true))
            .unwrap();
        assert!(!rfkill.soft.get());
        assert!(switch::is_on(&rfkill).unwrap());
        mock.wait_until("both adapters to power on", || {
            bluez.adapters().iter().all(|adapter| adapter.powered)
        });
    });
}

#[test]
fn a_hard_block_doesnt_count_as_off() {
    let rfkill = FakeRfkill::default();
    rfkill.hard.set(true);
    assert!(switch::is_on(&rfkill).unwrap());
}
