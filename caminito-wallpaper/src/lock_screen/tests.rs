use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::*;
use crate::positioning::Positioning;
use crate::settings::Colour;

fn colour(text: &str) -> Colour {
    Colour::parse(text).unwrap()
}

fn shown(connector: &str, x: i32, drawn: Drawn) -> Shown {
    Shown {
        connector: connector.into(),
        monitor: Monitor {
            make: "Dell Inc.".into(),
            model: "DELL U2720Q".into(),
            x,
            y: 0,
        },
        drawn,
    }
}

/// The file `text` gives, read as the locker reads it.
fn read(text: &str) -> glib::KeyFile {
    let file = glib::KeyFile::new();
    file.load_from_data(text, glib::KeyFileFlags::NONE)
        .unwrap_or_else(|err| panic!("{err}: {text}"));
    file
}

fn string(file: &glib::KeyFile, group: &str, key: &str) -> String {
    file.string(group, key).unwrap().into()
}

#[test]
fn each_output_has_its_place_picture_positioning_and_colour() {
    let outputs = [
        shown(
            "DP-1",
            0,
            Drawn::Picture {
                file: "/pictures/comet.jpg".into(),
                positioning: Positioning::Centre,
                colour: colour("#2c001e"),
            },
        ),
        shown("HDMI-A-1", 2560, Drawn::Colour(colour("#608040"))),
    ];

    let file = read(&text(&outputs));

    assert_eq!(file.groups().len(), 2);
    assert_eq!(string(&file, "DP-1", "make"), "Dell Inc.");
    assert_eq!(string(&file, "DP-1", "model"), "DELL U2720Q");
    assert_eq!(file.integer("DP-1", "x").unwrap(), 0);
    assert_eq!(file.integer("HDMI-A-1", "x").unwrap(), 2560);
    assert_eq!(file.integer("HDMI-A-1", "y").unwrap(), 0);
    assert_eq!(
        string(&file, "DP-1", "picture-filename"),
        "/pictures/comet.jpg"
    );
    assert_eq!(string(&file, "DP-1", "picture-options"), "centered");
    assert_eq!(string(&file, "DP-1", "primary-color"), "#2c001e");
    // A colour has no picture.
    assert_eq!(string(&file, "HDMI-A-1", "picture-filename"), "");
    assert_eq!(string(&file, "HDMI-A-1", "primary-color"), "#608040");
}

#[test]
fn a_dynamic_wallpaper_part_way_gives_the_picture_that_shows_more() {
    let blend = |percent| Drawn::Blend {
        from: "/day.jpg".into(),
        to: "/night.jpg".into(),
        percent,
        positioning: Positioning::Fill,
        colour: colour("#000000"),
    };

    let early = read(&text(&[shown("DP-1", 0, blend(30))]));
    let late = read(&text(&[shown("DP-1", 0, blend(70))]));

    assert_eq!(string(&early, "DP-1", "picture-filename"), "/day.jpg");
    assert_eq!(string(&late, "DP-1", "picture-filename"), "/night.jpg");
    assert_eq!(string(&late, "DP-1", "picture-options"), "zoom");
}

#[test]
fn names_with_any_character_come_back_as_they_were() {
    let mut output = shown(
        "DP-1",
        0,
        Drawn::Picture {
            file: " /a picture\nwith; # and \\ and é.png".into(),
            positioning: Positioning::Tile,
            colour: colour("#123456"),
        },
    );
    output.monitor.make = " Make[1]=".into();

    let file = read(&text(&[output]));

    assert_eq!(
        string(&file, "DP-1", "picture-filename"),
        " /a picture\nwith; # and \\ and é.png"
    );
    assert_eq!(string(&file, "DP-1", "make"), " Make[1]=");
}

#[test]
fn the_file_is_written_in_a_directory_made_for_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("caminito-wallpaper").join("lock-screen");
    let text = text(&[shown("DP-1", 0, Drawn::Colour(colour("#608040")))]);

    write(&path, &text).unwrap();

    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
}

#[test]
fn a_reader_has_the_old_file_or_the_new_one_while_it_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lock-screen");
    // Long enough that a write in place would be seen half done.
    let picture = |name: &str| Drawn::Picture {
        file: format!("/{}/{name}.jpg", "a".repeat(64 * 1024)),
        positioning: Positioning::Fill,
        colour: colour("#000000"),
    };
    let texts = [
        text(&[shown("DP-1", 0, picture("one"))]),
        text(&[
            shown("DP-1", 0, picture("two")),
            shown("HDMI-A-1", 2560, picture("three")),
        ]),
    ];
    write(&path, &texts[0]).unwrap();

    let done = Arc::new(AtomicBool::new(false));
    let writer = {
        let (path, texts, done) = (path.clone(), texts.clone(), done.clone());
        std::thread::spawn(move || {
            for i in 0..400 {
                write(&path, &texts[i % 2]).unwrap();
            }
            done.store(true, Ordering::SeqCst);
        })
    };
    let mut reads = 0;
    while !done.load(Ordering::SeqCst) {
        let read = std::fs::read_to_string(&path).unwrap();
        assert!(
            texts.contains(&read),
            "read {} bytes, neither file",
            read.len()
        );
        reads += 1;
    }
    writer.join().unwrap();

    assert!(reads > 1, "the file was read once only");
    let left: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(left.len(), 1, "a temporary file was left: {left:?}");
}
