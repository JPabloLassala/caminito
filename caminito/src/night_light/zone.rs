//! The time zone's coordinates (138), the sun schedule's location when there's no other:
//! the zone is `/etc/localtime`'s target under the zoneinfo directory, and its coordinates
//! are its row in `zone.tab`, following a backward link through `tzdata.zi` when the name
//! isn't there. `zone1970.tab` drops merged zones such as `Europe/Amsterdam`, so it isn't
//! read.

use std::path::Path;

use super::sun::Location;

/// The zone `localtime` links to, as `America/Montevideo`: the part of its target after the
/// last `zoneinfo/`. `None` when it isn't a link into a zoneinfo directory.
pub(crate) fn zone_name(localtime: &Path) -> Option<String> {
    let target = std::fs::read_link(localtime).ok()?;
    let target = target.to_str()?;
    let (_, name) = target.rsplit_once("zoneinfo/")?;
    (!name.is_empty()).then(|| name.to_owned())
}

/// The coordinates of `zone` in `zoneinfo`'s `zone.tab`, or of the zone it's a backward link
/// to in `tzdata.zi`. `None` for a zone with none, such as `Etc/UTC`, or when the files
/// can't be read.
pub(crate) fn coordinates(zoneinfo: &Path, zone: &str) -> Option<Location> {
    let table = std::fs::read_to_string(zoneinfo.join("zone.tab")).ok()?;
    if let Some(location) = row(&table, zone) {
        return Some(location);
    }
    let links = std::fs::read_to_string(zoneinfo.join("tzdata.zi")).ok()?;
    let target = links.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        match (fields.next(), fields.next(), fields.next()) {
            (Some("L"), Some(target), Some(name)) if name == zone => Some(target),
            _ => None,
        }
    })?;
    row(&table, target)
}

/// `zone`'s coordinates in `zone.tab`'s text.
fn row(table: &str, zone: &str) -> Option<Location> {
    table
        .lines()
        .filter(|line| !line.starts_with('#'))
        .find_map(|line| {
            let mut fields = line.split('\t');
            let (_country, coordinates, name) = (fields.next()?, fields.next()?, fields.next()?);
            (name == zone).then(|| iso_6709(coordinates)).flatten()
        })
}

/// `zone.tab`'s ISO 6709 coordinates: `±DDMM±DDDMM` or `±DDMMSS±DDDMMSS`.
fn iso_6709(text: &str) -> Option<Location> {
    let split = text.get(1..)?.find(['+', '-'])? + 1;
    let (latitude, longitude) = text.split_at(split);
    Some(Location {
        latitude: degrees(latitude, 2)?,
        longitude: degrees(longitude, 3)?,
    })
}

/// A signed `DDMM` or `DDMMSS`, the degrees `whole` digits long, in degrees.
fn degrees(text: &str, whole: usize) -> Option<f64> {
    let sign = match text.as_bytes().first()? {
        b'+' => 1.0,
        b'-' => -1.0,
        _ => return None,
    };
    let digits = &text[1..];
    if !digits.bytes().all(|digit| digit.is_ascii_digit()) {
        return None;
    }
    let part = |range: std::ops::Range<usize>| -> Option<f64> { digits.get(range)?.parse().ok() };
    let (degrees, minutes) = (part(0..whole)?, part(whole..whole + 2)?);
    let seconds = match digits.len() - whole {
        2 => 0.0,
        4 => part(whole + 2..whole + 4)?,
        _ => return None,
    };
    Some(sign * (degrees + minutes / 60.0 + seconds / 3600.0))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// The test zoneinfo directory: excerpts of tzdata's `zone.tab` and `tzdata.zi`.
    fn zoneinfo() -> PathBuf {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/zoneinfo").into()
    }

    /// The coordinates of the zone a `localtime` linking to `target` names.
    fn through_localtime(target: &str) -> Option<Location> {
        let dir = tempfile::tempdir().unwrap();
        let localtime = dir.path().join("localtime");
        std::os::unix::fs::symlink(target, &localtime).unwrap();
        coordinates(&zoneinfo(), &zone_name(&localtime)?)
    }

    #[track_caller]
    fn assert_at(location: Option<Location>, latitude: f64, longitude: f64) {
        let location = location.expect("the zone has coordinates");
        assert!(
            (location.latitude - latitude).abs() < 1e-3
                && (location.longitude - longitude).abs() < 1e-3,
            "{location:?}"
        );
    }

    #[test]
    fn montevideo_has_its_zone_tab_coordinates() {
        // -345433-0561245: 34°54′33″ S, 56°12′45″ W.
        assert_at(
            through_localtime("/usr/share/zoneinfo/America/Montevideo"),
            -34.909_17,
            -56.2125,
        );
    }

    #[test]
    fn amsterdam_has_its_zone_tab_coordinates() {
        // +5222+00454: 52°22′ N, 4°54′ E. A relative link, as some distributions make.
        assert_at(
            through_localtime("../usr/share/zoneinfo/Europe/Amsterdam"),
            52.366_67,
            4.9,
        );
    }

    #[test]
    fn calcutta_follows_its_backward_link_to_kolkata() {
        // Asia/Kolkata's +2232+08822.
        assert_at(
            through_localtime("/usr/share/zoneinfo/Asia/Calcutta"),
            22.533_33,
            88.366_67,
        );
    }

    #[test]
    fn utc_has_no_coordinates() {
        assert_eq!(through_localtime("/usr/share/zoneinfo/Etc/UTC"), None);
        assert_eq!(through_localtime("/usr/share/zoneinfo/UTC"), None);
    }

    #[test]
    fn a_localtime_that_isnt_a_zoneinfo_link_names_no_zone() {
        let dir = tempfile::tempdir().unwrap();
        let localtime = dir.path().join("localtime");
        assert_eq!(zone_name(&localtime), None);
        std::fs::write(&localtime, b"TZif").unwrap();
        assert_eq!(zone_name(&localtime), None);
        let elsewhere = dir.path().join("elsewhere");
        std::os::unix::fs::symlink("/opt/zones/Europe/Oslo", &elsewhere).unwrap();
        assert_eq!(zone_name(&elsewhere), None);
    }

    #[test]
    fn a_missing_zoneinfo_directory_gives_no_coordinates() {
        assert_eq!(
            coordinates(Path::new("/nonexistent"), "America/Montevideo"),
            None
        );
    }
}
