//! The primary output (81, ADR 0014): the user's choice of it, kept by monitor so that it
//! returns when its monitor does, and the stand-in while that monitor is away. Policy is
//! told which output it is; the refresh pass tells `org.mate.Camino` and XWayland's RandR.

use std::fmt;

use smithay::output::Output;

use super::{UNKNOWN_EDID_TEXT, is_internal, output_geometry, output_id};
use crate::state::Camino;

/// What Camino knows a monitor by when it's plugged in again (ADR 0014).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MonitorIdentity {
    /// Its EDID's make, model and serial.
    Edid {
        make: String,
        model: String,
        serial: String,
    },
    /// Its connector, when its EDID has no serial to tell it from another of its model.
    Connector(String),
}

impl MonitorIdentity {
    pub(crate) fn of(output: &Output) -> Self {
        let properties = output.physical_properties();
        let serial = properties.serial_number;
        if serial.is_empty() || serial == UNKNOWN_EDID_TEXT {
            MonitorIdentity::Connector(output.name())
        } else {
            MonitorIdentity::Edid {
                make: properties.make,
                model: properties.model,
                serial,
            }
        }
    }
}

/// Why an output can't be made the primary output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// No enabled output has this name.
    NotEnabled(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotEnabled(name) => write!(f, "{name:?} isn't an enabled output"),
        }
    }
}

impl std::error::Error for Error {}

/// The primary output among `outputs`, the enabled ones: the monitor the user chose, or
/// while it's away or none was chosen, the first built-in panel, or else the leftmost
/// output, and of those the topmost.
fn primary<'a>(outputs: &'a [Output], choice: Option<&MonitorIdentity>) -> Option<&'a Output> {
    let chosen = choice.and_then(|choice| {
        outputs
            .iter()
            .find(|output| MonitorIdentity::of(output) == *choice)
    });
    let built_in = || outputs.iter().find(|output| is_internal(&output.name()));
    let leftmost = || {
        outputs.iter().min_by_key(|output| {
            let loc = output_geometry(output).loc;
            (loc.x, loc.y)
        })
    };
    chosen.or_else(built_in).or_else(leftmost)
}

impl Camino {
    /// The primary output: the user's choice, or its stand-in. `None` with no outputs.
    pub fn primary_output(&self) -> Option<&Output> {
        primary(&self.outputs, self.primary_choice.as_ref())
    }

    /// Makes the enabled output named `name` the user's choice of primary output, for the
    /// Session (83 keeps it longer). It's the monitor that's chosen: unplugged or
    /// disabled, it has a stand-in until it returns.
    pub fn choose_primary_output(&mut self, name: &str) -> Result<(), Error> {
        let output = self
            .outputs
            .iter()
            .find(|output| output.name() == name)
            .ok_or_else(|| Error::NotEnabled(name.to_owned()))?;
        self.primary_choice = Some(MonitorIdentity::of(output));
        self.tell_policy_the_primary_output();
        Ok(())
    }

    /// Tells policy which output is the primary one now, after the choice or the outputs
    /// changed.
    pub(super) fn tell_policy_the_primary_output(&mut self) {
        let primary = self.primary_output().map(output_id);
        self.policy.set_primary_output(primary);
    }
}
