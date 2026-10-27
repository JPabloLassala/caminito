//! A colour temperature as a gamma table (ADR 0023): the white point of a blackbody at that
//! temperature, from formulas, scaling a linear ramp per channel.
//!
//! The formulas are the ones wlsunset (MIT) uses, `main.c`'s `illuminant_d`,
//! `planckian_locus` and `calc_whitepoint`: the CIE daylight locus from
//! 2500 K, Kim et al.'s cubic fit of the Planckian locus up to 4000 K, blended between the
//! two from 2500 to 4000 K, then XYZ to sRGB. They're published formulas, not tables:
//! nothing comes from redshift's or gammastep's tables. Two things
//! differ from wlsunset: the sRGB transfer function is the standard's, and the white point
//! is taken relative to 6500 K's, so 6500 K is the identity ramp and the temperatures near it
//! don't jump to it.

/// Neutral: the identity ramp, as with Night Light off.
pub(crate) const NEUTRAL: u32 = 6500;

/// The lowest temperature the Planckian fit holds at. Below it, its white point is used.
const PLANCKIAN_MIN: f64 = 1667.0;

/// A white point: how much of each of red, green and blue is kept, from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WhitePoint {
    pub(crate) red: f64,
    pub(crate) green: f64,
    pub(crate) blue: f64,
}

impl WhitePoint {
    /// The white point of a blackbody at `kelvin`, relative to 6500 K's, with its brightest
    /// channel at 1.
    pub(crate) fn of(kelvin: u32) -> Self {
        if kelvin == NEUTRAL {
            return WhitePoint {
                red: 1.0,
                green: 1.0,
                blue: 1.0,
            };
        }
        let [r, g, b] = srgb_white(f64::from(kelvin));
        let [nr, ng, nb] = srgb_white(f64::from(NEUTRAL));
        let (r, g, b) = (r / nr, g / ng, b / nb);
        let max = r.max(g).max(b);
        WhitePoint {
            red: r / max,
            green: g / max,
            blue: b / max,
        }
    }
}

/// A CRTC's gamma table of `size` entries per channel for `kelvin`: red's entries, then
/// green's, then blue's, as `wlr-gamma-control` and the legacy gamma call lay them out.
pub(crate) fn ramp(kelvin: u32, size: usize) -> Vec<u16> {
    let white = WhitePoint::of(kelvin);
    let mut table = Vec::with_capacity(size * 3);
    for scale in [white.red, white.green, white.blue] {
        table.extend((0..size).map(|i| entry(i, size, scale)));
    }
    table
}

/// Entry `i` of a channel's linear ramp of `size` entries, scaled by `scale`.
fn entry(i: usize, size: usize, scale: f64) -> u16 {
    let level = if size <= 1 {
        1.0
    } else {
        i as f64 / (size - 1) as f64
    };
    (level * scale * f64::from(u16::MAX)).round() as u16
}

/// A blackbody's white point at `kelvin` in sRGB, with its brightest channel at 1, as
/// wlsunset's `calc_whitepoint` works it out.
fn srgb_white(kelvin: f64) -> [f64; 3] {
    let (mut x, mut y) = (1.0, 1.0);
    if kelvin >= 2500.0 {
        (x, y) = daylight(kelvin);
    }
    if kelvin <= 4000.0 {
        let (px, py) = planckian(kelvin.max(PLANCKIAN_MIN));
        if kelvin >= 2500.0 {
            let factor = (4000.0 - kelvin) / 1500.0;
            let sine = ((std::f64::consts::PI * factor).cos() + 1.0) / 2.0;
            x = x * sine + px * (1.0 - sine);
            y = y * sine + py * (1.0 - sine);
        } else {
            (x, y) = (px, py);
        }
    }
    let z = 1.0 - x - y;
    // XYZ to linear sRGB, Bruce Lindbloom's matrix for D65.
    let rgb = [
        3.2404542 * x - 1.5371385 * y - 0.4985314 * z,
        -0.9692660 * x + 1.8760108 * y + 0.0415560 * z,
        0.0556434 * x - 0.2040259 * y + 1.0572252 * z,
    ]
    .map(|c| encode(c.clamp(0.0, 1.0)));
    let max = rgb[0].max(rgb[1]).max(rgb[2]);
    rgb.map(|c| c / max)
}

/// The CIE daylight locus's chromaticity at `kelvin`, from 2500 K.
fn daylight(kelvin: f64) -> (f64, f64) {
    let t = kelvin;
    let x = if t <= 7000.0 {
        0.244063 + 0.09911e3 / t + 2.9678e6 / t.powi(2) - 4.6070e9 / t.powi(3)
    } else {
        0.237040 + 0.24748e3 / t + 1.9018e6 / t.powi(2) - 2.0064e9 / t.powi(3)
    };
    let y = -3.0 * x.powi(2) + 2.870 * x - 0.275;
    (x, y)
}

/// The Planckian locus's chromaticity at `kelvin`, from 1667 K: Kim et al.'s cubic fit.
fn planckian(kelvin: f64) -> (f64, f64) {
    let t = kelvin;
    if t <= 4000.0 {
        let x = -0.2661239e9 / t.powi(3) - 0.2343589e6 / t.powi(2) + 0.8776956e3 / t + 0.179910;
        let y = if t <= 2222.0 {
            -1.1064814 * x.powi(3) - 1.34811020 * x.powi(2) + 2.18555832 * x - 0.20219683
        } else {
            -0.9549476 * x.powi(3) - 1.37418593 * x.powi(2) + 2.09137015 * x - 0.16748867
        };
        (x, y)
    } else {
        let x = -3.0258469e9 / t.powi(3) + 2.1070379e6 / t.powi(2) + 0.2226347e3 / t + 0.240390;
        let y = 3.0817580 * x.powi(3) - 5.87338670 * x.powi(2) + 3.75112997 * x - 0.37001483;
        (x, y)
    }
}

/// sRGB's transfer function, linear light to the encoded value.
fn encode(linear: f64) -> f64 {
    if linear <= 0.0031308 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sizes CRTCs report: the legacy call's 256, and `GAMMA_LUT_SIZE`s of 17 to 4096.
    const SIZES: [usize; 7] = [2, 17, 33, 256, 1024, 4096, 1];

    fn identity(size: usize) -> Vec<u16> {
        (0..3)
            .flat_map(|_| (0..size).map(move |i| entry(i, size, 1.0)))
            .collect()
    }

    #[test]
    fn neutral_is_the_identity_ramp() {
        for size in SIZES {
            assert_eq!(ramp(NEUTRAL, size), identity(size), "size {size}");
        }
        assert_eq!(ramp(NEUTRAL, 256)[255], u16::MAX);
        assert_eq!(ramp(NEUTRAL, 256)[128], 32896);
    }

    #[test]
    fn near_neutral_is_nearly_the_identity() {
        let white = WhitePoint::of(6450);
        for channel in [white.red, white.green, white.blue] {
            assert!(channel > 0.98, "{white:?}");
        }
    }

    #[test]
    fn lower_temperatures_dim_blue_more_than_green_and_green_more_than_red() {
        for kelvin in (1000..6500).step_by(50) {
            let white = WhitePoint::of(kelvin);
            assert_eq!(white.red, 1.0, "{kelvin} K: {white:?}");
            assert!(white.green < white.red, "{kelvin} K: {white:?}");
            assert!(white.blue < white.green, "{kelvin} K: {white:?}");
        }
    }

    #[test]
    fn the_warmer_the_temperature_the_less_blue_and_green() {
        let mut last = WhitePoint::of(NEUTRAL);
        for kelvin in (1000..6500).rev().step_by(100) {
            let white = WhitePoint::of(kelvin);
            assert!(
                white.blue <= last.blue,
                "{kelvin} K: {white:?} after {last:?}"
            );
            assert!(
                white.green <= last.green,
                "{kelvin} K: {white:?} after {last:?}"
            );
            last = white;
        }
    }

    #[test]
    fn four_thousand_kelvin_is_warm_but_not_orange() {
        let white = WhitePoint::of(4000);
        assert!((0.75..0.9).contains(&white.green), "{white:?}");
        assert!((0.5..0.7).contains(&white.blue), "{white:?}");
    }

    #[test]
    fn the_ramp_is_monotonic_for_every_size_a_crtc_reports() {
        for size in SIZES {
            for kelvin in (1000..=6500).step_by(250) {
                let table = ramp(kelvin, size);
                assert_eq!(table.len(), size * 3);
                for channel in table.chunks(size) {
                    assert!(
                        channel.windows(2).all(|pair| pair[0] <= pair[1]),
                        "{kelvin} K at size {size}: {channel:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_ramp_starts_at_black_and_ends_at_its_white_point() {
        let table = ramp(3400, 256);
        let white = WhitePoint::of(3400);
        assert_eq!([table[0], table[256], table[512]], [0, 0, 0]);
        assert_eq!(table[255], u16::MAX);
        assert_eq!(table[511], (white.green * 65535.0).round() as u16);
        assert_eq!(table[767], (white.blue * 65535.0).round() as u16);
    }
}
