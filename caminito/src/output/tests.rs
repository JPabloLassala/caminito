//! The default output scale (13, 50), and the X11 scale (ADR 0008): the largest of the
//! outputs' rounded-up scales.

use super::{Monitor, default_scale, round_scale, x11_scale};
use crate::settings::translate::WindowScalingFactor;

/// A monitor of `mm` millimetres showing a mode of `px` pixels.
fn monitor(mm: (i32, i32), px: (i32, i32), internal: bool, lid_switch: bool) -> Monitor {
    Monitor {
        physical_size: mm.into(),
        mode_size: px.into(),
        internal,
        lid_switch,
    }
}

fn external(mm: (i32, i32), px: (i32, i32)) -> Monitor {
    monitor(mm, px, false, false)
}

fn laptop(mm: (i32, i32), px: (i32, i32)) -> Monitor {
    monitor(mm, px, true, true)
}

/// Internal, with no lid switch: a phone or tablet.
fn phone(mm: (i32, i32), px: (i32, i32)) -> Monitor {
    monitor(mm, px, true, false)
}

fn auto(monitor: Monitor) -> f64 {
    default_scale(&monitor, WindowScalingFactor::Auto)
}

/// KWin's `testGenerateConfigs` rows (`autotests/integration/outputchanges_test.cpp`, at
/// commit 79ea0aa5), each with the mode KWin picks for it, since that's the mode the
/// scale is chosen for.
#[test]
fn kwins_table_gives_kwins_scales() {
    let rows = [
        ("1080p 27\"", external((598, 336), (1920, 1080)), 1.0),
        ("2160p 27\"", external((598, 336), (3840, 2160)), 1.70),
        (
            "1080p 27\" with non-preferred high refresh option",
            external((598, 336), (1920, 1080)),
            1.0,
        ),
        (
            "2160p 27\" with 30Hz preferred and a generated 60Hz mode",
            external((598, 336), (3840, 2160)),
            1.70,
        ),
        ("2160p invalid size", external((0, 0), (3840, 2160)), 1.0),
        (
            "2160p impossibly tiny size",
            external((1, 1), (3840, 2160)),
            1.0,
        ),
        (
            "2160p 27\" with 30Hz preferred mode",
            external((598, 336), (2560, 1440)),
            1.0,
        ),
        (
            "1440p 32:9 49\" with two preferred modes",
            external((1190, 340), (5120, 1440)),
            1.0,
        ),
        (
            "2160p 32:9 57\" with non-native preferred mode",
            external((1400, 400), (7680, 2160)),
            1.45,
        ),
        (
            "Framework 1920p 13.5\"",
            laptop((285, 190), (2880, 1920)),
            2.0,
        ),
        (
            "DELL XPS 13 1080p 13\"",
            laptop((293, 162), (1920, 1080)),
            1.35,
        ),
        (
            "DELL XPS 13 2160p 13\"",
            laptop((294, 165), (3840, 2160)),
            2.65,
        ),
        (
            "ThinkPad T14 2400p 14\"",
            laptop((301, 188), (3840, 2400)),
            2.60,
        ),
        ("SteamDeck OLED", laptop((100, 160), (800, 1280)), 1.0),
        ("Pixel 3a", phone((62, 128), (1080, 2220)), 3.0),
        ("OnePlus 6", phone((68, 145), (1080, 2280)), 2.65),
        (
            "Samsung Odyssey G5",
            external((698, 393), (2560, 1440)),
            1.0,
        ),
        ("LG C4 77\"", external((1600, 900), (3840, 2160)), 2.0),
        (
            "Acer 24 CB242Ybmiprx",
            external((527, 296), (1920, 1080)),
            1.0,
        ),
        ("Dell P2415Q", external((527, 296), (3840, 2160)), 2.0),
    ];
    for (name, monitor, scale) in rows {
        assert_eq!(auto(monitor), scale, "{name}");
    }
}

/// The developer's monitors, read with `edid-decode /sys/class/drm/*/edid` on 2026-09-30.
/// The desktop has no internal output and no lid switch.
#[test]
fn the_developers_monitors_get_the_rules_scales() {
    // AOC G2490W1G4 on DP-1: 527 × 296 mm (the first DTD; the base block says 53 × 30 cm),
    // 1920×1080. 93 DPI.
    assert_eq!(auto(external((527, 296), (1920, 1080))), 1.0);
    assert_eq!(auto(external((530, 300), (1920, 1080))), 1.0);
    // LG HDR 4K on HDMI-A-1: 600 × 340 mm in both the DTD and the base block, 3840×2160.
    // 161 DPI.
    assert_eq!(auto(external((600, 340), (3840, 2160))), 1.70);
}

#[test]
fn a_size_under_3_mm_on_either_axis_gives_1() {
    assert_eq!(auto(laptop((2, 190), (2880, 1920))), 1.0);
    assert_eq!(auto(laptop((285, 2), (2880, 1920))), 1.0);
}

#[test]
fn a_size_of_3_mm_is_believed() {
    assert_eq!(auto(external((3, 3), (2400, 2400))), 3.0);
}

#[test]
fn an_external_output_more_than_500_mm_tall_is_a_tv() {
    // 3840 px over 889 mm is 110 DPI: 1.14 of a monitor's 96, but 3.6 of a TV's 30.5.
    assert_eq!(auto(external((889, 500), (3840, 2160))), 1.0);
    // The TV's height allows at most 2160 ÷ 800 = 2.7.
    assert_eq!(auto(external((889, 501), (3840, 2160))), 2.7);
}

#[test]
fn an_internal_output_is_never_a_tv() {
    // 110 DPI against a laptop's 125 is under 1.
    assert_eq!(auto(laptop((889, 501), (3840, 2160))), 1.0);
}

#[test]
fn a_laptop_panel_targets_125_dpi_and_a_panel_without_a_lid_150() {
    // 254 mm is 10 in, so 2500 px across is 250 DPI on both axes.
    assert_eq!(auto(laptop((254, 254), (2500, 2500))), 2.0);
    assert_eq!(auto(phone((254, 254), (2500, 2500))), 1.65);
}

#[test]
fn at_least_800_logical_pixels_remain() {
    // 1280 px over 100 mm is 325 DPI, but 800 logical pixels allow only 1.
    assert_eq!(auto(laptop((100, 62), (1280, 800))), 1.0);
    // 1600 px across allows 2.
    assert_eq!(auto(laptop((100, 100), (1600, 1600))), 2.0);
}

/// KWin's one departure from 800: its phones keep 360 logical pixels (ticket 13's summary
/// said 800 for every output; KWin's code is what 13 chose to port).
#[test]
fn a_panel_without_a_lid_keeps_360_logical_pixels() {
    assert_eq!(auto(phone((50, 50), (900, 900))), 2.5);
}

#[test]
fn the_scale_is_at_most_3() {
    assert_eq!(auto(laptop((100, 56), (7680, 4320))), 3.0);
}

#[test]
fn the_smaller_axis_wins() {
    // 250 DPI across but 125 DPI down: 2 and 1 of a laptop's 125.
    assert_eq!(auto(laptop((254, 254), (2500, 1250))), 1.0);
    assert_eq!(auto(laptop((254, 254), (1250, 2500))), 1.0);
}

#[test]
fn scales_round_to_5_percent_steps() {
    assert_eq!(round_scale(1.32), 1.3);
    assert_eq!(round_scale(1.33), 1.35);
    assert_eq!(round_scale(1.674), 1.65);
    assert_eq!(round_scale(1.676), 1.7);
    assert_eq!(round_scale(2.5), 2.5);
}

#[test]
fn scales_that_round_below_1_20_become_1() {
    assert_eq!(round_scale(1.0), 1.0);
    assert_eq!(round_scale(1.17), 1.0);
    assert_eq!(round_scale(1.174), 1.0);
    assert_eq!(round_scale(1.176), 1.2);
    assert_eq!(round_scale(1.2), 1.2);
}

#[test]
fn scales_within_0_06_of_an_integer_snap_to_it() {
    assert_eq!(round_scale(1.95), 2.0);
    assert_eq!(round_scale(2.05), 2.0);
    assert_eq!(round_scale(2.95), 3.0);
    assert_eq!(round_scale(1.9), 1.9);
    assert_eq!(round_scale(2.1), 2.1);
    assert_eq!(round_scale(2.9), 2.9);
}

/// Every default scale is a whole number of fractional-scale-v1's 1/120 steps, so clients
/// are told it exactly.
#[test]
fn every_rounded_scale_is_a_whole_number_of_120ths() {
    for hundredths in 100..=300 {
        let scale = round_scale(hundredths as f64 / 100.0);
        assert_eq!((scale * 120.0).round() / 120.0, scale, "{scale}");
    }
}

#[test]
fn a_forced_window_scaling_factor_overrides_the_rule() {
    let dense = external((598, 336), (3840, 2160));
    assert_eq!(default_scale(&dense, WindowScalingFactor::Forced(1)), 1.0);
    let plain = external((598, 336), (1920, 1080));
    assert_eq!(default_scale(&plain, WindowScalingFactor::Forced(2)), 2.0);
    let bad = external((0, 0), (1920, 1080));
    assert_eq!(default_scale(&bad, WindowScalingFactor::Forced(2)), 2.0);
}

#[test]
fn a_single_output_gives_its_scale_rounded_up() {
    assert_eq!(x11_scale([1.0]), 1);
    assert_eq!(x11_scale([1.25]), 2);
    assert_eq!(x11_scale([1.5]), 2);
    assert_eq!(x11_scale([2.0]), 2);
}

#[test]
fn mixed_outputs_give_the_largest_rounded_up_scale() {
    assert_eq!(x11_scale([1.0, 2.0]), 2);
    assert_eq!(x11_scale([2.0, 1.0]), 2);
    assert_eq!(x11_scale([1.0, 1.25]), 2);
    assert_eq!(x11_scale([1.5, 2.25, 1.0]), 3);
}

#[test]
fn no_outputs_give_1() {
    assert_eq!(x11_scale([]), 1);
}
