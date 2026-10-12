use super::*;

fn aoc() -> Identity {
    Identity::of("AOC", "G2490W1G4", "AB12", "HDMI-A-1")
}

fn lg() -> Identity {
    Identity::of("LG", "HDR 4K", "CD34", "DP-1")
}

fn picture(file: &str) -> OutputSettings {
    OutputSettings {
        kind: Kind::Picture,
        picture: file.into(),
        ..OutputSettings::default()
    }
}

fn colour(text: &str) -> OutputSettings {
    OutputSettings {
        kind: Kind::Colour,
        colour: Colour::parse(text).unwrap(),
        ..OutputSettings::default()
    }
}

/// A store where the AOC and the LG were seen, each with a picture of its own, and every
/// output's is a colour.
fn store_with_both(same_on_all_outputs: bool) -> Memory {
    let mut store = Memory {
        different: !same_on_all_outputs,
        seen: vec![aoc().path_element(), lg().path_element()],
        ..Memory::default()
    };
    store.write(&Set::All, &colour("#608040"));
    store.write(&Set::Output(aoc()), &picture("/aoc.png"));
    store.write(&Set::Output(lg()), &picture("/lg.png"));
    store
}

#[test]
fn with_same_on_all_outputs_off_each_monitor_has_its_own() {
    let mut store = store_with_both(false);

    assert_eq!(
        settings_for(&mut store, &aoc(), Some(&aoc())),
        picture("/aoc.png")
    );
    assert_eq!(
        settings_for(&mut store, &lg(), Some(&aoc())),
        picture("/lg.png")
    );
}

#[test]
fn with_same_on_all_outputs_on_every_monitor_has_every_outputs() {
    let mut store = store_with_both(true);

    assert_eq!(
        settings_for(&mut store, &aoc(), Some(&aoc())),
        colour("#608040")
    );
    assert_eq!(
        settings_for(&mut store, &lg(), Some(&aoc())),
        colour("#608040")
    );
}

#[test]
fn a_monitor_is_known_by_its_identity_at_any_connector() {
    let mut store = store_with_both(false);
    let lg_elsewhere = Identity::of("LG", "HDR 4K", "CD34", "HDMI-A-2");

    assert_eq!(
        settings_for(&mut store, &lg_elsewhere, Some(&aoc())),
        picture("/lg.png")
    );
    assert_eq!(store.seen.len(), 2);
}

#[test]
fn a_monitor_never_seen_takes_the_primary_outputs_and_keeps_it() {
    let mut store = store_with_both(false);
    let dell = Identity::of("DEL", "U2720Q", "EF56", "DP-2");

    assert_eq!(
        settings_for(&mut store, &dell, Some(&lg())),
        picture("/lg.png")
    );
    assert!(store.seen.contains(&dell.path_element()));

    // It's the Dell's own from then on: the LG's changing doesn't change it.
    store.write(&Set::Output(lg()), &picture("/other.png"));
    assert_eq!(
        settings_for(&mut store, &dell, Some(&lg())),
        picture("/lg.png")
    );
}

#[test]
fn a_monitor_never_seen_with_same_on_all_outputs_on_takes_every_outputs() {
    let mut store = store_with_both(true);
    let dell = Identity::of("DEL", "U2720Q", "EF56", "DP-2");

    assert_eq!(
        settings_for(&mut store, &dell, Some(&lg())),
        colour("#608040")
    );
    // Turned off later, it keeps what it showed.
    store.different = true;
    assert_eq!(
        settings_for(&mut store, &dell, Some(&lg())),
        colour("#608040")
    );
}

#[test]
fn a_monitor_never_seen_with_no_primary_seen_takes_every_outputs() {
    let mut store = store_with_both(false);
    let dell = Identity::of("DEL", "U2720Q", "EF56", "DP-2");
    let new_primary = Identity::Connector("eDP-1".into());

    assert_eq!(
        settings_for(&mut store, &dell, Some(&new_primary)),
        colour("#608040")
    );
    assert_eq!(settings_for(&mut store, &dell, None), colour("#608040"));
}

#[test]
fn the_first_run_takes_mates_picture_and_the_second_doesnt() {
    let mut store = Memory::default();
    let mate = MateBackground {
        picture: "/usr/share/backgrounds/mate/desktop/Stripes.png".into(),
        options: "scaled".into(),
        colour: "#5891BC".into(),
    };

    assert!(take_mate_background(&mut store, &mate));
    let taken = OutputSettings {
        kind: Kind::Picture,
        picture: mate.picture.clone(),
        positioning: Positioning::Fit,
        colour: Colour::parse("#5891bc").unwrap(),
    };
    assert_eq!(settings_for(&mut store, &aoc(), None), taken);
    store.different = true;
    assert_eq!(settings_for(&mut store, &lg(), Some(&aoc())), taken);

    let changed = MateBackground {
        picture: "/elsewhere.png".into(),
        ..mate
    };
    assert!(!take_mate_background(&mut store, &changed));
    assert_eq!(store.read(&Set::All), taken);
}

#[test]
fn mates_colour_alone_is_a_colour() {
    let mut store = Memory::default();
    let mate = MateBackground {
        picture: String::new(),
        options: "zoom".into(),
        colour: "#000000000000".into(),
    };

    take_mate_background(&mut store, &mate);

    assert_eq!(
        store.read(&Set::All).drawn(),
        Drawn::Colour(Colour::default())
    );
}

#[test]
fn a_kind_drawn_by_a_later_ticket_is_its_colour_until_then() {
    for kind in [Kind::Slideshow, Kind::PictureOfTheDay, Kind::Dynamic] {
        let settings = OutputSettings {
            kind,
            ..colour("#123456")
        };
        assert_eq!(
            settings.drawn(),
            Drawn::Colour(Colour::parse("#123456").unwrap())
        );
    }
    assert_eq!(picture("").drawn(), Drawn::Colour(Colour::default()));
}

#[test]
fn colours_are_read_in_three_lengths() {
    let expected = Colour {
        red: 0x60,
        green: 0x80,
        blue: 0x40,
    };
    assert_eq!(Colour::parse("#608040"), Some(expected));
    assert_eq!(Colour::parse("#60ff80004000"), Some(expected));
    assert_eq!(
        Colour::parse("#684").map(Colour::text).as_deref(),
        Some("#668844")
    );
    assert_eq!(Colour::parse("608040"), None);
    assert_eq!(Colour::parse("#60804g"), None);
    assert_eq!(expected.text(), "#608040");
}
