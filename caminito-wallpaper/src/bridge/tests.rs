use super::*;
use crate::identity::Identity;
use crate::positioning::Positioning;
use crate::settings::tests::{Memory, aoc, colour, lg, picture, store_with_both};
use crate::settings::{OutputSettings, settings_for};

/// MATE's keys as they are with nothing written.
fn mate_defaults() -> MateBackground {
    MateBackground {
        picture: "/usr/share/backgrounds/mate/desktop/Stripes.png".into(),
        options: "zoom".into(),
        colour: "#5891BC".into(),
    }
}

/// Writes `writes` to `mate` as GSettings would, the bridge hearing `picture-filename`
/// change as the drawer does.
fn write(bridge: &mut Bridge, mate: &mut MateBackground, writes: Writes) {
    if let Some(options) = writes.options {
        mate.options = options.into();
    }
    if let Some(colour) = writes.colour {
        mate.colour = colour.text();
    }
    if let Some(picture) = writes.picture {
        mate.picture = picture;
        bridge.heard(&mate.picture);
    }
}

/// The drawer's round: what was heard brought in, then the primary output's written out.
fn round(bridge: &mut Bridge, store: &mut Memory, mate: &mut MateBackground, primary: &Identity) {
    bridge.bring_in(store);
    let drawn = settings_for(store, primary, Some(primary)).drawn();
    let writes = bridge.out(mate, &drawn);
    write(bridge, mate, writes);
}

/// What each of the AOC and the LG has.
fn both(store: &mut Memory) -> [OutputSettings; 2] {
    [
        settings_for(store, &aoc(), Some(&aoc())),
        settings_for(store, &lg(), Some(&aoc())),
    ]
}

#[test]
fn the_primary_outputs_picture_and_positioning_go_out() {
    let mut store = store_with_both(false);
    let mut mate = mate_defaults();
    let mut bridge = Bridge::default();

    round(&mut bridge, &mut store, &mut mate, &aoc());

    assert_eq!(mate.picture, "/aoc.png");
    assert_eq!(mate.options, "zoom");
    assert_eq!(mate.colour, "#5891bc");

    let mut fit = picture("/aoc.png");
    fit.positioning = Positioning::Tile;
    store.write(&Set::Output(aoc()), &fit);
    round(&mut bridge, &mut store, &mut mate, &aoc());
    assert_eq!(mate.options, "wallpaper");
}

#[test]
fn a_colour_goes_out_as_its_colour_and_no_picture() {
    let mut store = store_with_both(true);
    let mut mate = mate_defaults();
    mate.options = "centered".into();
    let mut bridge = Bridge::default();

    round(&mut bridge, &mut store, &mut mate, &aoc());

    assert_eq!(mate.picture, "");
    assert_eq!(mate.colour, "#608040");
    assert_eq!(mate.options, "centered");
}

#[test]
fn nothing_goes_out_when_mate_says_so_already() {
    let mut bridge = Bridge::default();
    let mate = MateBackground {
        picture: "/aoc.png".into(),
        options: "zoom".into(),
        colour: "#5891BC".into(),
    };

    let writes = bridge.out(&mate, &picture("/aoc.png").drawn());

    assert!(writes.is_empty(), "{writes:?}");
}

#[test]
fn the_drawers_own_write_changes_no_outputs_settings() {
    let mut store = store_with_both(false);
    let mut mate = mate_defaults();
    let mut bridge = Bridge::default();
    let before = both(&mut store);

    // The AOC's picture goes out, comes back as a change, and is passed over.
    round(&mut bridge, &mut store, &mut mate, &aoc());
    assert_eq!(mate.picture, "/aoc.png");
    assert!(!bridge.bring_in(&mut store));
    assert_eq!(both(&mut store), before);

    // And again when the AOC's picture changes.
    store.write(&Set::Output(aoc()), &picture("/new.png"));
    round(&mut bridge, &mut store, &mut mate, &aoc());
    round(&mut bridge, &mut store, &mut mate, &aoc());
    assert_eq!(mate.picture, "/new.png");
    assert_eq!(both(&mut store), [picture("/new.png"), picture("/lg.png")]);
}

#[test]
fn anothers_write_of_the_file_the_primary_shows_changes_the_other_outputs() {
    let mut store = store_with_both(false);
    let mut mate = mate_defaults();
    let mut bridge = Bridge::default();
    round(&mut bridge, &mut store, &mut mate, &aoc());
    assert_eq!(mate.picture, "/aoc.png");

    // caja's "Set as wallpaper" on the picture the AOC shows: the value doesn't change, but
    // dconf signals the write.
    bridge.heard("/aoc.png");
    round(&mut bridge, &mut store, &mut mate, &aoc());

    assert_eq!(both(&mut store), [picture("/aoc.png"), picture("/aoc.png")]);
}

#[test]
fn anothers_write_makes_the_file_every_outputs_picture_keeping_each_positioning() {
    let mut store = store_with_both(false);
    let mut tiled = colour("#c03030");
    tiled.positioning = Positioning::Tile;
    store.write(&Set::Output(lg()), &tiled);
    let mut mate = mate_defaults();
    let mut bridge = Bridge::default();
    round(&mut bridge, &mut store, &mut mate, &aoc());

    mate.picture = "/caja.png".into();
    bridge.heard("/caja.png");
    round(&mut bridge, &mut store, &mut mate, &aoc());

    let mut lg_now = tiled.clone();
    lg_now.kind = Kind::Picture;
    lg_now.picture = "/caja.png".into();
    assert_eq!(both(&mut store), [picture("/caja.png"), lg_now]);
    // Every output's too, for "same on all outputs" and a monitor never seen.
    assert_eq!(store.read(&Set::All).drawn(), {
        let mut all = colour("#608040");
        all.kind = Kind::Picture;
        all.picture = "/caja.png".into();
        all.drawn()
    });
    // And MATE's isn't written again: it says so already.
    assert_eq!(mate.picture, "/caja.png");
}

#[test]
fn anothers_empty_filename_is_no_picture_to_bring_in() {
    let mut store = store_with_both(false);
    let mut bridge = Bridge::default();
    let before = both(&mut store);

    bridge.heard("");

    assert!(!bridge.bring_in(&mut store));
    assert_eq!(both(&mut store), before);
}

#[test]
fn a_write_that_failed_isnt_taken_for_the_next_anothers() {
    let mut store = store_with_both(false);
    let mut bridge = Bridge::default();

    let writes = bridge.out(&mate_defaults(), &picture("/aoc.png").drawn());
    assert_eq!(writes.picture.as_deref(), Some("/aoc.png"));
    bridge.not_written();
    bridge.heard("/aoc.png");

    assert!(bridge.bring_in(&mut store));
    assert_eq!(both(&mut store), [picture("/aoc.png"), picture("/aoc.png")]);
}
