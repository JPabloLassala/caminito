//! The sun's position (138): its elevation at a place and a time, the day's highest, and when
//! it next climbs past civil twilight. NOAA's solar position equations, as in its solar
//! calculator's spreadsheet (`NOAA_Solar_Calculations_day.xls`), which are good to about a
//! minute of arc for the years around now. Time is an argument, as a Unix time in seconds.

/// Where the sun is looked at from, in degrees: north and east are positive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Location {
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
}

/// Civil twilight: the sun's centre this far below the horizon, in degrees. Below it is
/// night.
pub(crate) const TWILIGHT: f64 = -6.0;

const DAY: f64 = 86_400.0;

/// The sun's declination and the equation of time, in degrees and minutes, at `unix`.
fn declination_and_equation_of_time(unix: f64) -> (f64, f64) {
    let julian_day = unix / DAY + 2_440_587.5;
    let century = (julian_day - 2_451_545.0) / 36_525.0;

    let mean_longitude = (280.466_46 + century * (36_000.769_83 + century * 0.000_303_2)) % 360.0;
    let mean_anomaly = 357.529_11 + century * (35_999.050_29 - 0.000_153_7 * century);
    let eccentricity = 0.016_708_634 - century * (0.000_042_037 + 0.000_000_126_7 * century);
    let m = mean_anomaly.to_radians();
    let centre = m.sin() * (1.914_602 - century * (0.004_817 + 0.000_014 * century))
        + (2.0 * m).sin() * (0.019_993 - 0.000_101 * century)
        + (3.0 * m).sin() * 0.000_289;
    let true_longitude = mean_longitude + centre;
    let omega = (125.04 - 1934.136 * century).to_radians();
    let apparent_longitude = true_longitude - 0.005_69 - 0.004_78 * omega.sin();
    let mean_obliquity = 23.0
        + (26.0
            + (21.448 - century * (46.815 + century * (0.000_59 - century * 0.001_813))) / 60.0)
            / 60.0;
    let obliquity = (mean_obliquity + 0.002_56 * omega.cos()).to_radians();

    let declination = (obliquity.sin() * apparent_longitude.to_radians().sin()).asin();

    let y = (obliquity / 2.0).tan().powi(2);
    let l = mean_longitude.to_radians();
    let equation_of_time = 4.0
        * (y * (2.0 * l).sin() - 2.0 * eccentricity * m.sin()
            + 4.0 * eccentricity * y * m.sin() * (2.0 * l).cos()
            - 0.5 * y * y * (4.0 * l).sin()
            - 1.25 * eccentricity * eccentricity * (2.0 * m).sin())
        .to_degrees();
    (declination.to_degrees(), equation_of_time)
}

/// The sun's elevation above the horizon at `location` and `unix`, in degrees, without
/// the atmosphere's refraction, which civil twilight's −6° doesn't count either.
pub(crate) fn elevation(location: Location, unix: f64) -> f64 {
    let (declination, equation_of_time) = declination_and_equation_of_time(unix);
    let minutes = unix.rem_euclid(DAY) / 60.0;
    let true_solar_time =
        (minutes + equation_of_time + 4.0 * location.longitude).rem_euclid(1440.0);
    let hour_angle = (true_solar_time / 4.0 - 180.0).to_radians();
    let latitude = location.latitude.to_radians();
    let declination = declination.to_radians();
    let cos_zenith =
        latitude.sin() * declination.sin() + latitude.cos() * declination.cos() * hour_angle.cos();
    90.0 - cos_zenith.clamp(-1.0, 1.0).acos().to_degrees()
}

/// The Unix time of solar noon on the day of `unix`, the day running from one solar midnight
/// at `location` to the next.
pub(crate) fn solar_noon(location: Location, unix: f64) -> f64 {
    // Local mean time, by longitude, and the day's noon in it.
    let offset = location.longitude * 240.0;
    let noon = ((unix + offset) / DAY).floor() * DAY + DAY / 2.0 - offset;
    let (_, equation_of_time) = declination_and_equation_of_time(noon);
    noon - equation_of_time * 60.0
}

/// The sun's highest elevation on the day of `unix` at `location`, at its solar noon.
pub(crate) fn highest(location: Location, unix: f64) -> f64 {
    elevation(location, solar_noon(location, unix))
}

/// How far ahead `next_rise` looks: past the longest night that ends, in a polar night's
/// last days.
const HORIZON: f64 = 2.0 * DAY;

/// The step `next_rise` walks in: the sun climbs past −6° once at most in it.
const STEP: f64 = 600.0;

/// The next Unix time after `unix` that the sun climbs past −6° at `location`, to the
/// second, or `None` in a polar night that lasts past the next two days.
pub(crate) fn next_rise(location: Location, unix: f64) -> Option<f64> {
    let above = |t: f64| elevation(location, t) >= TWILIGHT;
    let mut before = unix;
    let mut was_above = above(before);
    while before < unix + HORIZON {
        let after = before + STEP;
        let is_above = above(after);
        if !was_above && is_above {
            let (mut low, mut high) = (before, after);
            while high - low > 1.0 {
                let middle = (low + high) / 2.0;
                if above(middle) {
                    high = middle;
                } else {
                    low = middle;
                }
            }
            return Some(high);
        }
        (before, was_above) = (after, is_above);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // The expected values are PyEphem 4.2's, with no refraction, as NOAA's calculator
    // (gml.noaa.gov) wasn't reachable from where these were written. The two agree to a
    // hundredth of a degree away from the horizon.

    const MONTEVIDEO: Location = Location {
        latitude: -34.9011,
        longitude: -56.1645,
    };
    const OSLO: Location = Location {
        latitude: 59.9139,
        longitude: 10.7522,
    };
    const TROMSO: Location = Location {
        latitude: 69.6492,
        longitude: 18.9553,
    };
    const LONGYEARBYEN: Location = Location {
        latitude: 78.2232,
        longitude: 15.6267,
    };

    #[track_caller]
    fn assert_near(actual: f64, expected: f64, within: f64) {
        assert!(
            (actual - expected).abs() <= within,
            "{actual} isn't within {within} of {expected}"
        );
    }

    #[test]
    fn montevideos_sun_at_the_december_solstice() {
        // 2026-12-21 at 15:00, 22:30 and 08:00 UTC.
        assert_near(elevation(MONTEVIDEO, 1_797_865_200.0), 75.234, 0.5);
        assert_near(elevation(MONTEVIDEO, 1_797_892_200.0), 4.250, 0.5);
        assert_near(elevation(MONTEVIDEO, 1_797_840_000.0), -5.643, 0.5);
    }

    #[test]
    fn oslos_sun_at_the_march_equinox() {
        // 2026-03-20 at 11:00, 06:00 and 16:30 UTC.
        assert_near(elevation(OSLO, 1_774_004_400.0), 29.834, 0.5);
        assert_near(elevation(OSLO, 1_773_986_400.0), 4.308, 0.5);
        assert_near(elevation(OSLO, 1_774_024_200.0), 6.789, 0.5);
    }

    #[test]
    fn tromsos_sun_in_december() {
        // 2026-12-15 at 11:00 and 00:00 UTC.
        assert_near(elevation(TROMSO, 1_797_332_400.0), -3.003, 0.5);
        assert_near(elevation(TROMSO, 1_797_292_800.0), -42.065, 0.5);
    }

    #[test]
    fn the_days_highest_is_at_solar_noon() {
        // 2026-12-21 at 05:00 UTC, past Montevideo's solar midnight at about 03:43, the
        // night before its noon at 15:42:48 UTC.
        let night = 1_797_829_200.0;
        // At 03:00 UTC, it's still the day before's.
        assert_near(
            solar_noon(MONTEVIDEO, 1_797_822_000.0),
            1_797_867_768.0 - DAY,
            120.0,
        );
        assert_near(solar_noon(MONTEVIDEO, night), 1_797_867_768.0, 120.0);
        assert_near(highest(MONTEVIDEO, night), 78.536, 0.5);
        // Longyearbyen's midsummer and midwinter, 2026-06-21 and 2026-12-15.
        assert_near(highest(LONGYEARBYEN, 1_782_000_000.0), 35.213, 0.5);
        assert_near(highest(LONGYEARBYEN, 1_797_292_800.0), -11.5, 0.5);
    }

    #[test]
    fn the_sun_climbs_past_civil_twilight() {
        // From midnight UTC on each day.
        let rise = next_rise(MONTEVIDEO, 1_797_811_200.0).unwrap();
        assert_near(rise, 1_797_839_873.9, 120.0);
        let rise = next_rise(OSLO, 1_773_964_800.0).unwrap();
        assert_near(rise, 1_773_981_454.4, 120.0);
        let rise = next_rise(TROMSO, 1_797_292_800.0).unwrap();
        assert_near(rise, 1_797_323_077.7, 120.0);
    }

    #[test]
    fn after_the_rise_the_next_is_the_next_days() {
        let rise = next_rise(OSLO, 1_773_964_800.0).unwrap();
        let next = next_rise(OSLO, rise + 60.0).unwrap();
        assert_near(next - rise, DAY, 10.0 * 60.0);
    }

    #[test]
    fn the_polar_night_and_the_midnight_sun_have_no_rise() {
        assert_eq!(next_rise(LONGYEARBYEN, 1_797_292_800.0), None);
        assert_eq!(next_rise(LONGYEARBYEN, 1_782_000_000.0), None);
    }
}
