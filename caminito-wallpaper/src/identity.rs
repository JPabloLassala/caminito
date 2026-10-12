//! What a monitor is known by, as a saved layout knows it (ADR 0014): its EDID's make, model
//! and serial, or its connector when it reports no serial. Caminito's `MonitorIdentity`
//! makes the same choice, so a monitor has the same identity for both.

use std::fmt::Write;

/// The serial Caminito reports for an EDID that has none (`UNKNOWN_EDID_TEXT`).
const UNKNOWN: &str = "Unknown";

/// What a monitor is known by.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Identity {
    /// Its EDID's make, model and serial.
    Edid {
        make: String,
        model: String,
        serial: String,
    },
    /// Its connector, as `DP-1`, when it reports no serial to tell it from another of its
    /// model.
    Connector(String),
}

impl Identity {
    /// The identity of the monitor at `connector`, which reports `make`, `model` and
    /// `serial`, as output management has them.
    pub fn of(make: &str, model: &str, serial: &str, connector: &str) -> Self {
        if serial.is_empty() || serial == UNKNOWN {
            Identity::Connector(connector.to_owned())
        } else {
            Identity::Edid {
                make: make.to_owned(),
                model: model.to_owned(),
                serial: serial.to_owned(),
            }
        }
    }

    /// The identity as one element of a GSettings path: `edid-<make>-<model>-<serial>` or
    /// `connector-<connector>`. A dconf path element is safe with letters and digits alone,
    /// so anything else, `-` included, is `_` and its byte in hex, and the parts can't run
    /// into each other.
    pub fn path_element(&self) -> String {
        match self {
            Identity::Edid {
                make,
                model,
                serial,
            } => format!("edid-{}-{}-{}", escape(make), escape(model), escape(serial)),
            Identity::Connector(connector) => format!("connector-{}", escape(connector)),
        }
    }
}

/// `text` with every byte but an ASCII letter or digit as `_` and its two hex digits.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() {
            escaped.push(byte as char);
        } else {
            let _ = write!(escaped, "_{byte:02X}");
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_monitor_with_a_serial_is_known_by_its_edid_wherever_it_is_plugged_in() {
        let at_dp = Identity::of("AOC", "G2490W1G4", "AB12", "DP-1");
        let at_hdmi = Identity::of("AOC", "G2490W1G4", "AB12", "HDMI-A-1");

        assert_eq!(at_dp, at_hdmi);
        assert_eq!(at_dp.path_element(), "edid-AOC-G2490W1G4-AB12");
    }

    #[test]
    fn a_monitor_with_no_serial_is_known_by_its_connector() {
        let empty = Identity::of("Caminito", "Headless", "", "headless-1");
        let unknown = Identity::of("LG", "HDR 4K", "Unknown", "DP-2");

        assert_eq!(empty, Identity::Connector("headless-1".into()));
        assert_eq!(empty.path_element(), "connector-headless_2D1");
        assert_eq!(unknown, Identity::Connector("DP-2".into()));
    }

    #[test]
    fn a_path_element_has_letters_digits_and_escapes_alone() {
        let identity = Identity::of("Dell Inc.", "U2720Q/b", "-x-", "DP-1");

        assert_eq!(
            identity.path_element(),
            "edid-Dell_20Inc_2E-U2720Q_2Fb-_2Dx_2D"
        );
    }
}
