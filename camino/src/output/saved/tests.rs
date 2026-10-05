//! Which saved layout is for which monitors, among monitors with EDID serials, which the
//! headless backend's have none of, and where the file is kept.

use std::path::PathBuf;

use smithay::output::{Output, PhysicalProperties, Subpixel};

use super::{SavedMonitor, assign, path_from};

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

/// The monitors of a layout saved with `outputs` connected, all disabled.
fn saved(outputs: &[Output]) -> Vec<SavedMonitor> {
    let monitor = |output| SavedMonitor::of(output, None, false);
    outputs.iter().map(monitor).collect()
}

/// The connector each of a layout's `monitors` is now at, among `connected`, in the
/// layout's order, if the layout is for those monitors.
fn connectors(monitors: &[SavedMonitor], connected: &[Output]) -> Option<Vec<String>> {
    let assigned = assign(monitors, connected)?;
    Some(assigned.iter().map(|(output, _)| output.name()).collect())
}

fn names(names: &[&str]) -> Option<Vec<String>> {
    Some(names.iter().map(|&name| name.to_owned()).collect())
}

#[test]
fn a_layout_is_for_its_monitors_at_whichever_connectors_they_are() {
    let monitors = saved(&[output("DP-1", "A1"), output("HDMI-A-1", "B2")]);
    let connected = [output("DP-3", "B2"), output("DP-2", "A1")];

    assert_eq!(connectors(&monitors, &connected), names(&["DP-2", "DP-3"]));
}

#[test]
fn a_layout_isnt_for_another_monitor_at_the_same_connector() {
    let monitors = saved(&[output("DP-1", "A1"), output("HDMI-A-1", "B2")]);
    let connected = [output("DP-1", "A1"), output("HDMI-A-1", "C3")];

    assert_eq!(connectors(&monitors, &connected), None);
}

#[test]
fn a_layout_isnt_for_more_or_fewer_monitors_than_it_has() {
    let both = [output("DP-1", "A1"), output("HDMI-A-1", "B2")];
    let monitors = saved(&both);

    assert_eq!(connectors(&monitors, &both[..1]), None);
    let three = [both[0].clone(), both[1].clone(), output("DP-2", "C3")];
    assert_eq!(connectors(&monitors, &three), None);
}

#[test]
fn two_monitors_with_one_serial_are_told_apart_by_their_connectors() {
    let monitors = saved(&[output("DP-1", "0"), output("DP-2", "0")]);
    let connected = [output("DP-2", "0"), output("DP-1", "0")];

    assert_eq!(connectors(&monitors, &connected), names(&["DP-1", "DP-2"]));
}

#[test]
fn of_two_monitors_with_one_serial_the_one_moved_to_a_new_connector_is_the_one_left_over() {
    let monitors = saved(&[output("DP-1", "0"), output("DP-2", "0")]);
    let connected = [output("DP-2", "0"), output("DP-3", "0")];

    assert_eq!(connectors(&monitors, &connected), names(&["DP-3", "DP-2"]));
}

#[test]
fn a_monitor_without_a_serial_is_known_by_its_connector() {
    let monitors = saved(&[output("DP-1", "Unknown"), output("DP-2", "")]);

    let same = [output("DP-2", "Unknown"), output("DP-1", "")];
    assert_eq!(connectors(&monitors, &same), names(&["DP-1", "DP-2"]));
    let moved = [output("DP-1", ""), output("DP-3", "")];
    assert_eq!(connectors(&monitors, &moved), None);
}

#[test]
fn the_layouts_are_kept_under_xdg_config_home() {
    assert_eq!(
        path_from(Some("/home/u/.cfg".into()), Some("/home/u".into())),
        Some(PathBuf::from("/home/u/.cfg/camino/layouts.json"))
    );
}

#[test]
fn the_layouts_are_kept_under_the_homes_config_without_an_absolute_xdg_config_home() {
    let under_home = Some(PathBuf::from("/home/u/.config/camino/layouts.json"));

    assert_eq!(path_from(None, Some("/home/u".into())), under_home);
    assert_eq!(
        path_from(Some("".into()), Some("/home/u".into())),
        under_home
    );
    assert_eq!(
        path_from(Some("cfg".into()), Some("/home/u".into())),
        under_home
    );
}

#[test]
fn the_layouts_are_kept_nowhere_without_a_home() {
    assert_eq!(path_from(None, None), None);
    assert_eq!(path_from(None, Some("".into())), None);
}
