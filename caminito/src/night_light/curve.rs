//! The sun schedule's curve (138): the temperature for the sun's elevation, and the fixed
//! times' stand-in for it when there's no location, and for the sun itself (215).
//! Temperatures are interpolated in mireds, 10⁶ / K, in which equal steps look equal.

use super::sun::{Position, TWILIGHT};

/// The sun schedule's temperatures, in kelvin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Temperatures {
    /// At the day's highest elevation.
    pub(crate) noon: u32,
    /// While the sun is below civil twilight.
    pub(crate) night: u32,
}

/// The temperature `daylight` of the way from night to noon, 0 to 1, in mireds.
fn between(temperatures: Temperatures, daylight: f64) -> u32 {
    let night = 1e6 / f64::from(temperatures.night);
    let noon = 1e6 / f64::from(temperatures.noon);
    let mireds = night + (noon - night) * daylight.clamp(0.0, 1.0);
    (1e6 / mireds).round() as u32
}

/// The temperature with the sun at `elevation` on a day whose highest is `highest`, in
/// degrees: night's below −6°, noon's at the highest, and linear in elevation between. A
/// day the sun never climbs past −6° is night all day; under the midnight sun it bottoms out
/// where the sun does.
pub(crate) fn sun_temperature(temperatures: Temperatures, elevation: f64, highest: f64) -> u32 {
    if highest <= TWILIGHT {
        return temperatures.night;
    }
    between(temperatures, (elevation - TWILIGHT) / (highest - TWILIGHT))
}

const HOUR: f64 = 3600.0;

/// The fixed times' sunrise and sunset, in seconds of the local day: as though the sun rose
/// at 07:00 and set at 19:00.
const FIXED_RISE: f64 = 7.0 * HOUR;
const FIXED_SET: f64 = 19.0 * HOUR;

/// The temperature at `local`, seconds into the local day, with no location: night's until
/// 07:00 and from 19:00, and a half-sine between that peaks at 13:00 at noon's.
pub(crate) fn fixed_times_temperature(temperatures: Temperatures, local: f64) -> u32 {
    let local = local.rem_euclid(24.0 * HOUR);
    if !(FIXED_RISE..FIXED_SET).contains(&local) {
        return temperatures.night;
    }
    let daylight = (std::f64::consts::PI * (local - FIXED_RISE) / (FIXED_SET - FIXED_RISE)).sin();
    between(temperatures, daylight)
}

/// The fixed times' sun at its highest, at 13:00, in degrees.
pub(crate) const FIXED_HIGHEST: f64 = 45.0;

/// The fixed times' sun at `local`, seconds into the local day, for a dynamic wallpaper with
/// no location (215). Its elevation is a sine about −6° that crosses it at 07:00 and 19:00
/// and peaks at 13:00 at [`FIXED_HIGHEST`], so that [`sun_temperature`] of it is
/// [`fixed_times_temperature`]: the two agree about dusk. Its azimuth goes round at 15° an
/// hour, east at 07:00, south at 13:00 and west at 19:00, as the sun does north of the
/// tropics.
pub(crate) fn fixed_times_position(local: f64) -> Position {
    let local = local.rem_euclid(24.0 * HOUR);
    let phase = std::f64::consts::PI * (local - FIXED_RISE) / (FIXED_SET - FIXED_RISE);
    Position {
        elevation: TWILIGHT + (FIXED_HIGHEST - TWILIGHT) * phase.sin(),
        azimuth: (90.0 + 15.0 * (local - FIXED_RISE) / HOUR).rem_euclid(360.0),
    }
}

/// The next 07:00 after `unix`, local time being `offset` seconds ahead of UTC: the fixed
/// times' sunrise.
pub(crate) fn fixed_times_next_rise(unix: f64, offset: f64) -> f64 {
    let local = unix + offset;
    let day = 24.0 * HOUR;
    let mut rise = (local / day).floor() * day + FIXED_RISE;
    if rise <= local {
        rise += day;
    }
    rise - offset
}

#[cfg(test)]
mod tests {
    use super::super::sun::{Location, elevation, highest, solar_noon};
    use super::*;

    const DEFAULTS: Temperatures = Temperatures {
        noon: 5500,
        night: 3400,
    };

    const MONTEVIDEO: Location = Location {
        latitude: -34.9011,
        longitude: -56.1645,
    };
    const TROMSO: Location = Location {
        latitude: 69.6492,
        longitude: 18.9553,
    };
    const LONGYEARBYEN: Location = Location {
        latitude: 78.2232,
        longitude: 15.6267,
    };

    /// The sun schedule's temperature at `location` and `unix`.
    fn at(location: Location, unix: f64) -> u32 {
        sun_temperature(DEFAULTS, elevation(location, unix), highest(location, unix))
    }

    /// The temperatures every half hour of the 24 h from `unix`.
    fn day_from(location: Location, unix: f64) -> Vec<u32> {
        (0..48)
            .map(|half_hour| at(location, unix + f64::from(half_hour) * 1800.0))
            .collect()
    }

    // 2026-06-21 and 2026-12-21 at 00:00 UTC.
    const JUNE: f64 = 1_782_000_000.0;
    const DECEMBER: f64 = 1_797_811_200.0;

    #[test]
    fn below_civil_twilight_is_the_night_temperature() {
        assert_eq!(sun_temperature(DEFAULTS, -6.5, 40.0), 3400);
        assert_eq!(sun_temperature(DEFAULTS, -30.0, 40.0), 3400);
        // Montevideo at 03:00 UTC, midnight there.
        assert_eq!(at(MONTEVIDEO, DECEMBER + 3.0 * HOUR), 3400);
    }

    #[test]
    fn the_days_highest_is_the_noon_temperature_in_june_and_december() {
        for day in [JUNE, DECEMBER] {
            let noon = solar_noon(MONTEVIDEO, day + 12.0 * HOUR);
            assert_eq!(at(MONTEVIDEO, noon), 5500);
        }
        // Its short winter day, in June, reaches the noon temperature too.
        assert!(highest(MONTEVIDEO, JUNE + 12.0 * HOUR) < 35.0);
    }

    #[test]
    fn halfway_up_is_halfway_in_mireds() {
        // 10⁶ / ((10⁶ / 5500 + 10⁶ / 3400) / 2) = 4202 K, where halfway in kelvin would be
        // 4450.
        assert_eq!(sun_temperature(DEFAULTS, 17.0, 40.0), 4202);
    }

    #[test]
    fn tromsos_polar_night_is_the_night_temperature_but_at_its_twilight_noon() {
        // 2026-12-15: the sun stays below the horizon, and climbs only to about −3°.
        let day = day_from(TROMSO, 1_797_292_800.0);
        let nights = day.iter().filter(|kelvin| **kelvin == 3400).count();
        assert!(nights >= 36, "{day:?}");
        let noon = solar_noon(TROMSO, 1_797_292_800.0 + 12.0 * HOUR);
        assert_eq!(at(TROMSO, noon), 5500);
    }

    #[test]
    fn a_day_the_sun_never_passes_civil_twilight_is_night_all_day() {
        // Longyearbyen, 2026-12-15: the sun climbs to about −11.5°.
        let day = day_from(LONGYEARBYEN, 1_797_292_800.0);
        assert!(day.iter().all(|kelvin| *kelvin == 3400), "{day:?}");
    }

    #[test]
    fn the_midnight_sun_never_reaches_the_night_temperature() {
        // Longyearbyen, 2026-06-21: the sun is about 11.7° up at midnight, 35.2° at noon.
        let day = day_from(LONGYEARBYEN, JUNE);
        let coolest = *day.iter().min().unwrap();
        assert!(coolest > 3400, "{day:?}");
        assert!((4000..4500).contains(&coolest), "{day:?}");
        assert_eq!(*day.iter().max().unwrap(), 5500, "{day:?}");
    }

    #[test]
    fn the_fixed_times_are_night_before_seven_and_from_nineteen() {
        assert_eq!(fixed_times_temperature(DEFAULTS, 0.0), 3400);
        assert_eq!(fixed_times_temperature(DEFAULTS, 6.9 * HOUR), 3400);
        assert_eq!(fixed_times_temperature(DEFAULTS, 19.0 * HOUR), 3400);
        assert_eq!(fixed_times_temperature(DEFAULTS, 13.0 * HOUR), 5500);
        // sin(30°) = 1/2 of the way, in mireds, at 09:00 and at 17:00.
        assert_eq!(fixed_times_temperature(DEFAULTS, 9.0 * HOUR), 4202);
        assert_eq!(fixed_times_temperature(DEFAULTS, 17.0 * HOUR), 4202);
    }

    #[test]
    fn the_fixed_times_sunrise_is_the_next_seven_oclock_local() {
        // 2026-12-21 at 00:00 UTC, three hours behind: 21:00 local, so 07:00 local is 10:00 UTC.
        let offset = -3.0 * HOUR;
        assert_eq!(
            fixed_times_next_rise(DECEMBER, offset),
            DECEMBER + 10.0 * HOUR
        );
        // At 07:00 local itself, it's the next day's.
        let rise = DECEMBER + 10.0 * HOUR;
        assert_eq!(fixed_times_next_rise(rise, offset), rise + 24.0 * HOUR);
    }

    #[test]
    fn the_fixed_times_sun_crosses_civil_twilight_at_seven_and_nineteen() {
        let at = |hour: f64| fixed_times_position(hour * HOUR);
        assert_eq!(at(7.0).elevation, TWILIGHT);
        assert!((at(19.0).elevation - TWILIGHT).abs() < 1e-9);
        assert_eq!(at(13.0).elevation, FIXED_HIGHEST);
        assert_eq!(at(1.0).elevation, 2.0 * TWILIGHT - FIXED_HIGHEST);
        // East at 07:00, south at 13:00, west at 19:00 and north at 01:00, a day later too.
        assert_eq!(at(7.0).azimuth, 90.0);
        assert_eq!(at(13.0).azimuth, 180.0);
        assert_eq!(at(19.0).azimuth, 270.0);
        assert_eq!(at(1.0).azimuth, 0.0);
        assert_eq!(at(25.0).azimuth, 0.0);
    }

    #[test]
    fn the_fixed_times_sun_gives_the_fixed_times_temperature() {
        for half_hour in 0..48 {
            let local = f64::from(half_hour) * 1800.0;
            let elevation = fixed_times_position(local).elevation;
            assert_eq!(
                sun_temperature(DEFAULTS, elevation, FIXED_HIGHEST),
                fixed_times_temperature(DEFAULTS, local),
                "{half_hour}"
            );
        }
    }
}
