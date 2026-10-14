use std::collections::HashSet;
use std::fs;

use super::*;

const MINUTE: i64 = 60;

/// A slideshow of `count` pictures named `00.png` up, as a folder of them would be.
fn slideshow(count: usize, interval: Interval, in_order: bool) -> Slideshow {
    let pictures = (0..count)
        .map(|index| PathBuf::from(format!("/pictures/{index:02}.png")))
        .collect();
    Slideshow::new(Path::new("/pictures"), pictures, interval, in_order)
}

/// The local time `days` days, `hours` hours and `minutes` minutes into 2026.
fn at(days: i64, hours: i64, minutes: i64) -> i64 {
    // 2026-01-01 00:00, local.
    1_767_225_600 + days * DAY + hours * HOUR + minutes * MINUTE
}

#[test]
fn the_next_change_is_at_the_next_whole_interval_of_the_clock() {
    let cases = [
        (Interval::FiveMinutes, at(0, 9, 3), 2 * MINUTE),
        (Interval::FiveMinutes, at(0, 9, 5), 5 * MINUTE),
        (Interval::FifteenMinutes, at(0, 9, 3), 12 * MINUTE),
        (
            Interval::FifteenMinutes,
            at(0, 9, 45) + 59,
            15 * MINUTE - 59,
        ),
        (Interval::Hour, at(0, 9, 3), 57 * MINUTE),
        (Interval::Hour, at(0, 23, 0), HOUR),
        (Interval::Day, at(0, 9, 3), 15 * HOUR - 3 * MINUTE),
        (Interval::Day, at(3, 0, 0), DAY),
    ];
    for (interval, now, seconds) in cases {
        let slideshow = slideshow(4, interval, false);
        assert_eq!(
            slideshow.next_change(now),
            Some(seconds as u64),
            "{interval:?} at {now}"
        );
    }
}

#[test]
fn each_interval_is_its_seconds_and_its_nick() {
    let seconds: Vec<i64> = Interval::ALL.iter().map(|i| i.seconds()).collect();
    assert_eq!(seconds, [5 * MINUTE, 15 * MINUTE, HOUR, DAY]);
    for interval in Interval::ALL {
        assert_eq!(Interval::from_nick(interval.nick()), interval);
    }
    assert_eq!(Interval::from_nick("fortnight"), Interval::default());
}

#[test]
fn the_picture_changes_at_the_interval_and_holds_between() {
    let slideshow = slideshow(5, Interval::FifteenMinutes, true);
    let first = slideshow.picture(at(0, 9, 0), 0);
    assert_eq!(slideshow.picture(at(0, 9, 14) + 59, 0), first);
    assert_ne!(slideshow.picture(at(0, 9, 15), 0), first);
}

#[test]
fn in_order_it_goes_by_name_and_starts_again_after_the_last() {
    let slideshow = slideshow(3, Interval::Hour, true);
    let shown: Vec<String> = (0..7)
        .map(|hour| {
            let picture = slideshow.picture(at(0, hour, 0), 0).unwrap();
            picture.file_name().unwrap().to_string_lossy().into_owned()
        })
        .collect();
    let start = shown.iter().position(|name| name == "00.png").unwrap();
    let names = ["00.png", "01.png", "02.png"];
    for (offset, name) in shown.iter().enumerate() {
        let expected = names[(offset + 3 - start) % 3];
        assert_eq!(name, expected, "{shown:?}");
    }
}

#[test]
fn at_random_every_picture_shows_once_before_any_repeats() {
    let count = 7;
    let slideshow = slideshow(count, Interval::FiveMinutes, false);
    let interval = Interval::FiveMinutes.seconds();
    // Slots counted from 1970 local: the first slot of a round is a multiple of the count.
    let round_start = |round: i64| {
        let slot = at(0, 0, 0).div_euclid(interval);
        (slot - slot.rem_euclid(count as i64) + round * count as i64) * interval
    };
    let mut orders = HashSet::new();
    for rank in 0..3 {
        for round in 0..20 {
            let shown: Vec<&Path> = (0..count as i64)
                .map(|slot| {
                    slideshow
                        .picture(round_start(round) + slot * interval, rank)
                        .unwrap()
                })
                .collect();
            let distinct: HashSet<&Path> = shown.iter().copied().collect();
            assert_eq!(
                distinct.len(),
                count,
                "rank {rank}, round {round}: {shown:?}"
            );
            orders.insert(shown);
        }
    }
    // Shuffled anew each round, not one order over and over.
    assert!(orders.len() > 20, "{} orders", orders.len());
}

#[test]
fn the_random_order_is_the_same_for_the_same_folder_at_the_same_time() {
    let one = slideshow(9, Interval::Hour, false);
    let other = slideshow(9, Interval::Hour, false);
    for hour in 0..30 {
        assert_eq!(
            one.picture(at(0, hour, 0), 0),
            other.picture(at(0, hour, 0), 0)
        );
    }
}

#[test]
fn two_outputs_on_one_folder_show_different_pictures() {
    for in_order in [false, true] {
        let slideshow = slideshow(4, Interval::FiveMinutes, in_order);
        for slot in 0..50 {
            let now = at(0, 0, 0) + slot * Interval::FiveMinutes.seconds();
            let first = slideshow.picture(now, 0);
            let second = slideshow.picture(now, 1);
            assert!(first.is_some());
            assert_ne!(first, second, "in order {in_order}, slot {slot}");
        }
    }
}

#[test]
fn one_picture_is_shown_on_every_output_and_never_changes() {
    let slideshow = slideshow(1, Interval::FiveMinutes, false);
    let only = Path::new("/pictures/00.png");
    assert_eq!(slideshow.picture(at(0, 9, 0), 0), Some(only));
    assert_eq!(slideshow.picture(at(0, 9, 5), 1), Some(only));
    assert_eq!(slideshow.next_change(at(0, 9, 0)), None);
}

#[test]
fn an_empty_or_missing_folder_has_no_picture_and_no_change() {
    let empty = tempfile::tempdir().unwrap();
    fs::write(empty.path().join("notes.txt"), "not a picture").unwrap();
    fs::create_dir(empty.path().join("more")).unwrap();
    fs::write(empty.path().join("more").join("deeper.png"), "").unwrap();
    let missing = empty.path().join("gone");
    for folder in [empty.path(), missing.as_path()] {
        let slideshow = Slideshow::read(folder, Interval::FiveMinutes, false);
        assert!(slideshow.is_empty());
        assert_eq!(slideshow.len(), 0);
        assert_eq!(slideshow.picture(at(0, 9, 0), 0), None);
        assert_eq!(slideshow.next_change(at(0, 9, 0)), None);
    }
}

#[test]
fn a_folder_is_read_for_its_own_pictures_by_name_and_read_again_for_new_ones() {
    let folder = tempfile::tempdir().unwrap();
    for name in ["b.jpg", "a.PNG", "c.txt"] {
        fs::write(folder.path().join(name), "").unwrap();
    }
    let read = Slideshow::read(folder.path(), Interval::Hour, true);
    assert_eq!(
        read.pictures(),
        [folder.path().join("a.PNG"), folder.path().join("b.jpg")]
    );

    fs::write(folder.path().join("0.webp"), "").unwrap();
    fs::remove_file(folder.path().join("b.jpg")).unwrap();
    let read = Slideshow::read(folder.path(), Interval::Hour, true);
    assert_eq!(
        read.pictures(),
        [folder.path().join("0.webp"), folder.path().join("a.PNG")]
    );
}

#[test]
fn outputs_are_ranked_among_those_on_the_same_folder_by_identity() {
    let shown = [
        ("edid-LG", "/a"),
        ("edid-AOC", "/a"),
        ("connector-eDP_2d1", "/b"),
        ("edid-BOE", "/a"),
    ];
    assert_eq!(ranks(&shown), [2, 0, 0, 1]);
}
