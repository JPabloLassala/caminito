//! Bluetooth's switch (255): off is the rfkill soft block, and on lifts it and powers every
//! adapter. The rfkill block is the tests' fake, and BlueZ is python-dbusmock's.

mod mock;

use std::cell::Cell;
use std::rc::Rc;

use caminito_bluetooth::bluez::Bluez;
use caminito_bluetooth::rfkill::{self, Blocks, Rfkill};
use caminito_bluetooth::switch::Switch;

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

/// BlueZ's model once it has read the mock's objects, and the switch over it.
fn switch(mock: &mock::Mock) -> (Bluez, Rc<FakeRfkill>, Switch) {
    let bluez = Bluez::new(&mock.connection);
    mock.wait_until("BlueZ's objects", || bluez.is_ready());
    let rfkill = Rc::new(FakeRfkill::default());
    let switch = Switch::new(&bluez, rfkill.clone());
    (bluez, rfkill, switch)
}

#[test]
fn off_is_the_soft_block_and_on_lifts_it_and_powers_every_adapter() {
    mock::run(|mock| {
        let first = mock.add_adapter("hci0", "laptop");
        let second = mock.add_adapter("hci1", "dongle");
        let (bluez, rfkill, switch) = switch(mock);
        assert!(switch.is_on().unwrap());

        mock.block_on(switch.set_on(false)).unwrap();
        assert!(rfkill.soft.get());
        assert!(!switch.is_on().unwrap());

        // An adapter powered off before the block stays off after it, unless on powers it.
        mock.block_on(bluez.set_powered(&first, false)).unwrap();
        mock.block_on(bluez.set_powered(&second, false)).unwrap();
        mock.wait_until("both adapters to power off", || {
            bluez.adapters().iter().all(|adapter| !adapter.powered)
        });

        mock.block_on(switch.set_on(true)).unwrap();
        assert!(!rfkill.soft.get());
        assert!(switch.is_on().unwrap());
        mock.wait_until("both adapters to power on", || {
            bluez.adapters().iter().all(|adapter| adapter.powered)
        });
    });
}

#[test]
fn an_adapter_that_comes_back_unpowered_after_on_is_powered() {
    mock::run(|mock| {
        let (bluez, _rfkill, switch) = switch(mock);
        mock.block_on(switch.set_on(true)).unwrap();

        // As an adapter whose radio the block had cut comes back once it's lifted.
        let adapter = mock.add_adapter("hci0", "laptop");
        mock.set_property(&adapter, "org.bluez.Adapter1", "Powered", false);
        mock.wait_until("the adapter to power on", || {
            bluez
                .adapters()
                .first()
                .is_some_and(|adapter| adapter.powered)
        });
    });
}

#[test]
fn after_off_an_adapter_is_left_unpowered() {
    mock::run(|mock| {
        let adapter = mock.add_adapter("hci0", "laptop");
        let (bluez, _rfkill, switch) = switch(mock);
        mock.block_on(switch.set_on(false)).unwrap();

        mock.block_on(bluez.set_powered(&adapter, false)).unwrap();
        mock.wait_until("the adapter to power off", || !bluez.adapters()[0].powered);
        // A second change after the first, which the switch has seen by then: had it meant
        // to power the adapter, its call would have been sent before this one.
        mock.block_on(bluez.set_alias(&adapter, "workstation"))
            .unwrap();
        mock.wait_until("the adapter's new name", || {
            bluez.adapters()[0].alias == "workstation"
        });
        assert!(!bluez.adapters()[0].powered);
    });
}

#[test]
fn a_hard_block_doesnt_count_as_off() {
    mock::run(|mock| {
        let (_bluez, rfkill, switch) = switch(mock);
        rfkill.hard.set(true);
        assert!(switch.is_on().unwrap());
    });
}
