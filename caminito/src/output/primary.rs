//! The primary output (81, ADR 0014): the user's choice of it, kept by monitor so that it
//! returns when its monitor does, and the stand-in while that monitor is away. Policy is
//! told which output it is; the refresh pass tells `org.mate.Caminito` and XWayland's RandR.

use std::fmt;

use smithay::output::Output;

use super::{UNKNOWN_EDID_TEXT, is_internal, output_geometry, output_id};
use crate::state::Caminito;

/// What Caminito knows a monitor by when it's plugged in again (ADR 0014).
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
    /// What the monitor an output shows is known by.
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

/// The user's choice of primary output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrimaryChoice {
    monitor: MonitorIdentity,
    /// The connector it was at when chosen, which tells it from another monitor with the
    /// same EDID, as two of a model with one dummy serial have.
    connector: String,
}

impl PrimaryChoice {
    /// The choice of the monitor `output` shows.
    fn of(output: &Output) -> Self {
        PrimaryChoice {
            monitor: MonitorIdentity::of(output),
            connector: output.name(),
        }
    }
}

/// Why an output can't be made the primary output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Error {
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

/// The primary output among `outputs`, the enabled ones: the monitor the user chose, at
/// the connector it was at if two are alike, or while it's away or none was chosen, the
/// first built-in panel, or else the leftmost output, and of those the topmost.
fn primary<'a>(outputs: &'a [Output], choice: Option<&PrimaryChoice>) -> Option<&'a Output> {
    let chosen = choice.and_then(|choice| {
        let alike = || {
            outputs
                .iter()
                .filter(|output| MonitorIdentity::of(output) == choice.monitor)
        };
        alike()
            .find(|output| output.name() == choice.connector)
            .or_else(|| alike().next())
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

impl Caminito {
    /// The primary output: the user's choice, or its stand-in. `None` with no outputs.
    pub(crate) fn primary_output(&self) -> Option<&Output> {
        primary(&self.outputs, self.primary_choice.as_ref())
    }

    /// Makes the enabled output named `name` the user's choice of primary output, for the
    /// Session, and for longer once the layout is saved (83). It's the monitor that's
    /// chosen: unplugged or disabled, it has a stand-in until it returns.
    pub(crate) fn choose_primary_output(&mut self, name: &str) -> Result<(), Error> {
        let output = self
            .outputs
            .iter()
            .find(|output| output.name() == name)
            .ok_or_else(|| Error::NotEnabled(name.to_owned()))?;
        let output = output.clone();
        self.restore_primary_choice(&output);
        Ok(())
    }

    /// Makes the monitor `output` shows the user's choice again, as a saved layout has it
    /// (83). The output may be disabled still, on its way to being enabled.
    pub(super) fn restore_primary_choice(&mut self, output: &Output) {
        self.primary_choice = Some(PrimaryChoice::of(output));
        self.tell_policy_the_primary_output();
    }

    /// Tells policy which output is the primary one now, after the choice or the outputs
    /// changed.
    pub(super) fn tell_policy_the_primary_output(&mut self) {
        let primary = self.primary_output().map(output_id);
        self.policy.set_primary_output(primary);
    }
}

#[cfg(test)]
mod tests {
    //! The choice among monitors with EDID serials, which the headless backend's have
    //! none of.

    use smithay::output::{Output, PhysicalProperties, Subpixel};

    use super::{PrimaryChoice, primary};

    /// An output at `connector` whose monitor's EDID has `serial`.
    fn output(connector: &str, serial: &str) -> Output {
        Output::new(
            connector.to_owned(),
            PhysicalProperties {
                size: (600, 340).into(),
                subpixel: Subpixel::Unknown,
                make: "LG".to_owned(),
                model: "HDR 4K".to_owned(),
                serial_number: serial.to_owned(),
            },
        )
    }

    #[test]
    fn a_chosen_monitor_is_known_again_at_another_connector() {
        let chosen = PrimaryChoice::of(&output("DP-1", "A1"));
        let outputs = [output("HDMI-A-1", "B2"), output("DP-2", "A1")];

        assert_eq!(primary(&outputs, Some(&chosen)), Some(&outputs[1]));
    }

    #[test]
    fn of_two_monitors_with_one_serial_the_one_at_the_chosen_connector_is_primary() {
        let outputs = [output("DP-1", "0"), output("DP-2", "0")];
        let chosen = PrimaryChoice::of(&outputs[1]);

        assert_eq!(primary(&outputs, Some(&chosen)), Some(&outputs[1]));
    }

    #[test]
    fn a_monitor_without_a_serial_is_known_by_its_connector() {
        let chosen = PrimaryChoice::of(&output("DP-1", "Unknown"));
        let outputs = [output("HDMI-A-1", "Unknown"), output("DP-1", "")];

        assert_eq!(primary(&outputs, Some(&chosen)), Some(&outputs[1]));
    }
}
