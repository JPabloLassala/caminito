use std::path::{Path, PathBuf};

use super::*;

/// mate-screensaver's `/usr/share/backgrounds/cosmos/background-1.xml`, as installed: nine
/// pictures, each half an hour with a five-second transition to the next, from midnight on
/// 2009-08-04, so a cycle of four and a half hours.
const COSMOS: &str = include_str!("cosmos.xml");

/// GNOME 3.38's `adwaita-timed.xml`, its files given per size and relative to it.
const ADWAITA: &str = r#"<background>
  <starttime>
    <year>2011</year><month>11</month><day>24</day>
    <hour>7</hour><minute>00</minute><second>00</second>
  </starttime>
  <!-- Sunrise from 7 AM for an hour. -->
  <static>
    <duration>3600.0</duration>
    <file>
      <size width="1024" height="768">morning-small.jpg</size>
      <size width="4096" height="2304">morning.jpg</size>
    </file>
  </static>
  <transition type="overlay">
    <duration>18000.0</duration>
    <from>morning.jpg</from>
    <to>day.jpg</to>
  </transition>
  <static><duration>18000.0</duration><file>day.jpg</file></static>
  <transition type="overlay">
    <duration>21600.0</duration>
    <from>day.jpg</from>
    <to>night.jpg</to>
  </transition>
  <static><duration>18000.0</duration><file>night &amp; stars.jpg</file></static>
  <transition type="overlay">
    <duration>7200.0</duration>
    <from>night &amp; stars.jpg</from>
    <to>morning.jpg</to>
  </transition>
</background>"#;

/// The local seconds of a date and time.
fn local(year: i64, month: i64, day: i64, hour: i64, minute: i64, second: i64) -> i64 {
    days_from_civil(year, month, day) * DAY_SECONDS + hour * HOUR + minute * 60 + second
}

fn cosmos() -> Timed {
    Timed::parse(COSMOS, Path::new("/elsewhere")).unwrap()
}

fn cosmic(name: &str) -> PathBuf {
    PathBuf::from(format!("/usr/share/backgrounds/cosmos/{name}.jpg"))
}

/// What `timed` shows at a local time, as the picture that shows most and the blend.
fn shown(timed: &Timed, at: i64) -> (PathBuf, PathBuf, f64) {
    let moment = timed.at(at);
    (moment.from, moment.to, moment.blend)
}

#[test]
fn the_civil_dates_count_days_from_1970() {
    assert_eq!(days_from_civil(1970, 1, 1), 0);
    assert_eq!(days_from_civil(2000, 3, 1), 11_017);
    assert_eq!(days_from_civil(2026, 10, 14), 20_740);
    assert_eq!(days_from_civil(1969, 12, 31), -1);
}

#[test]
fn a_pair_is_light_by_day_dark_by_night_and_fades_through_twilight() {
    // The sun's elevation through a day, from midnight, and how dark the pair is for each.
    let day = [
        (-40.0, 1.0),
        (-12.0, 1.0),
        (-6.0, 1.0),
        (-3.0, 0.75),
        (0.0, 0.5),
        (3.0, 0.25),
        (6.0, 0.0),
        (35.0, 0.0),
        (6.0, 0.0),
        (1.5, 0.375),
        (-6.0, 1.0),
        (-30.0, 1.0),
    ];
    let pair = Dynamic::Pair {
        light: "/light.png".into(),
        dark: "/dark.png".into(),
    };
    for (elevation, darkness) in day {
        let sky = Sky {
            elevation: Some(elevation),
            local: 0,
        };
        let moment = pair.at(&sky);
        assert_eq!(moment.from, Path::new("/light.png"));
        assert_eq!(moment.to, Path::new("/dark.png"));
        assert!(
            (moment.blend - darkness).abs() < 1e-9,
            "at {elevation}° the blend is {}, not {darkness}",
            moment.blend
        );
    }
    assert!(pair.follows_sun());
}

#[test]
fn a_pair_without_the_sun_is_dark_until_seven_and_from_nineteen() {
    let darkness = |hour, minute| {
        Sky {
            elevation: None,
            local: local(2026, 10, 14, hour, minute, 0),
        }
        .darkness()
    };
    assert_eq!(darkness(0, 0), 1.0);
    assert_eq!(darkness(6, 59), 1.0);
    assert_eq!(darkness(7, 0), 0.0);
    assert_eq!(darkness(18, 59), 0.0);
    assert_eq!(darkness(19, 0), 1.0);
}

/// The start of a cycle of cosmos's, a thousand cycles after its own start.
fn cosmos_cycle() -> i64 {
    local(2009, 8, 4, 0, 0, 0) + 1000 * 16_200
}

#[test]
fn cosmos_shows_each_picture_for_half_an_hour() {
    let cosmos = cosmos();
    let order = [
        "cloud",
        "comet",
        "earth-horizon",
        "blue-marble-west",
        "galaxy-ngc3370",
        "helix-nebula",
        "jupiter",
        "sombrero",
        "whirlpool",
    ];
    for (index, name) in order.iter().enumerate() {
        let start = cosmos_cycle() + index as i64 * 1800;
        for into in [0, 1, 900, 1794] {
            assert_eq!(
                shown(&cosmos, start + into),
                (cosmic(name), cosmic(name), 0.0),
                "{into} s into {name}'s half hour"
            );
        }
    }
    // A day is five cycles and an hour and a half, so the next day's midnight is in blue
    // marble's half hour.
    assert_eq!(
        shown(&cosmos, local(2009, 8, 5, 0, 0, 0)).0,
        cosmic("blue-marble-west")
    );
    assert!(!Dynamic::Timed(cosmos).follows_sun());
}

#[test]
fn cosmos_cross_fades_over_its_five_second_transitions() {
    let cosmos = cosmos();
    let half_past = cosmos_cycle() + 1795;
    assert_eq!(
        shown(&cosmos, half_past),
        (cosmic("cloud"), cosmic("comet"), 0.0)
    );
    assert_eq!(
        shown(&cosmos, half_past + 2),
        (cosmic("cloud"), cosmic("comet"), 0.4)
    );
    assert_eq!(
        shown(&cosmos, half_past + 4),
        (cosmic("cloud"), cosmic("comet"), 0.8)
    );
    assert_eq!(
        shown(&cosmos, half_past + 5),
        (cosmic("comet"), cosmic("comet"), 0.0)
    );
    // The last transition goes round to the first picture.
    let end = cosmos_cycle() + 16_200;
    assert_eq!(
        shown(&cosmos, end - 3),
        (cosmic("whirlpool"), cosmic("cloud"), 0.4)
    );
    assert_eq!(shown(&cosmos, end), (cosmic("cloud"), cosmic("cloud"), 0.0));
}

#[test]
fn before_its_start_a_timed_wallpaper_is_where_its_cycles_would_have_it() {
    // An hour before cosmos's start is three and a half hours into a cycle: the sombrero's
    // half hour.
    assert_eq!(
        shown(&cosmos(), local(2009, 8, 3, 23, 0, 0)).0,
        cosmic("sombrero")
    );
}

#[test]
fn a_timed_wallpapers_day_long_transitions_follow_the_clock() {
    let adwaita = Timed::parse(ADWAITA, Path::new("/usr/share/backgrounds/gnome")).unwrap();
    let file = |name: &str| PathBuf::from(format!("/usr/share/backgrounds/gnome/{name}"));
    let at = |hour, minute| shown(&adwaita, local(2026, 10, 14, hour, minute, 0));

    assert_eq!(at(7, 30), (file("morning.jpg"), file("morning.jpg"), 0.0));
    assert_eq!(at(10, 30), (file("morning.jpg"), file("day.jpg"), 0.5));
    assert_eq!(at(15, 0), (file("day.jpg"), file("day.jpg"), 0.0));
    assert_eq!(at(21, 0), (file("day.jpg"), file("night.jpg"), 0.5));
    assert_eq!(
        at(2, 0),
        (file("night & stars.jpg"), file("night & stars.jpg"), 0.0)
    );
    assert_eq!(
        at(6, 0),
        (file("night & stars.jpg"), file("morning.jpg"), 0.5)
    );
}

#[test]
fn a_file_given_per_size_is_its_largest() {
    let adwaita = Timed::parse(ADWAITA, Path::new("/gnome")).unwrap();
    assert_eq!(
        shown(&adwaita, local(2026, 10, 14, 7, 0, 0)).0,
        Path::new("/gnome/morning.jpg")
    );
}

#[test]
fn a_timed_wallpaper_is_shown_by_its_noon_and_its_midnight() {
    let adwaita = Dynamic::Timed(Timed::parse(ADWAITA, Path::new("/gnome")).unwrap());
    assert_eq!(
        adwaita.day_and_night(),
        (
            PathBuf::from("/gnome/day.jpg"),
            PathBuf::from("/gnome/night & stars.jpg")
        )
    );
}

#[test]
fn a_malformed_xml_is_refused() {
    let start = "<starttime><year>2009</year><month>8</month><day>4</day></starttime>";
    let still = "<static><duration>60</duration><file>/a.jpg</file></static>";
    let cases = [
        String::new(),
        "not XML at all".into(),
        "\u{0}\u{1}\u{fffd}<<<>>>".into(),
        "<background>".into(),
        format!("<background>{start}{still}"),
        format!("<background>{start}{still}</wallpapers>"),
        format!("<background>{start}{still}</background><background/>"),
        "<wallpapers><wallpaper><filename>/a.jpg</filename></wallpaper></wallpapers>".into(),
        format!("<background>{still}</background>"),
        format!("<background>{start}</background>"),
        format!(
            "<background>{}{still}</background>",
            start.replace("<month>8</month>", "<month>13</month>")
        ),
        format!(
            "<background>{}{still}</background>",
            start.replace("2009", "two thousand and nine")
        ),
        format!(
            "<background>{start}{}</background>",
            still.replace("60", "soon")
        ),
        format!(
            "<background>{start}{}</background>",
            still.replace("60", "-60")
        ),
        format!(
            "<background>{start}{}</background>",
            still.replace("60", "NaN")
        ),
        format!(
            "<background>{start}{}</background>",
            still.replace("60", "0")
        ),
        format!(
            "<background>{start}{}</background>",
            still.replace("<file>/a.jpg</file>", "")
        ),
        format!(
            "<background>{start}<transition><duration>5</duration><from>/a.jpg</from></transition></background>"
        ),
        format!(
            "<background>{start}{}</background>",
            still.replace("/a.jpg", "&nonsense;")
        ),
    ];
    for text in cases {
        assert!(
            Timed::parse(&text, Path::new("/")).is_err(),
            "{text:?} was taken"
        );
    }
}

#[test]
fn an_unreadable_file_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        Dynamic::read(&dir.path().join("missing.xml")),
        Err(Error::Read(_))
    ));
}

/// Makes an empty file at `path`, and its folders.
fn touch(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"").unwrap();
}

#[test]
fn a_plasma_package_with_light_and_dark_pictures_is_a_pair() {
    let disk = tempfile::tempdir().unwrap();
    let package = disk.path().join("Next");
    touch(&package.join("contents/images/1920x1080.png"));
    touch(&package.join("contents/images/5120x2880.png"));
    touch(&package.join("contents/images_dark/1920x1080.png"));
    touch(&package.join("contents/images_dark/5120x2880.png"));
    touch(&package.join("contents/images_dark/2880x5120.png"));
    let light_only = disk.path().join("Flow");
    touch(&light_only.join("contents/images/3840x2160.jpg"));

    assert_eq!(
        Dynamic::read(&package),
        Ok(Dynamic::Pair {
            light: package.join("contents/images/5120x2880.png"),
            dark: package.join("contents/images_dark/5120x2880.png"),
        })
    );
    assert_eq!(Dynamic::read(&light_only), Err(Error::NotPair));
}
