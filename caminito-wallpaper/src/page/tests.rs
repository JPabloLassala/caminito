use super::*;
use crate::settings::Memory;

fn aoc() -> Output {
    Output::new(
        "HDMI-A-1",
        "AOC",
        "G2490W1G4",
        "AB12",
        (1920, 0),
        (1920, 1080),
    )
}

fn lg() -> Output {
    Output::new("DP-1", "LG", "HDR 4K", "CD34", (0, 0), (3840, 2160))
}

fn laptop() -> Output {
    Output::new("eDP-1", "BOE", "0x0BCA", "", (3840, 200), (2560, 1600))
}

fn picture(file: &str) -> OutputSettings {
    OutputSettings {
        kind: Kind::Picture,
        picture: file.into(),
        ..OutputSettings::default()
    }
}

/// A store where every output's is `/all.png`, the AOC's own `/aoc.png` and the LG's own
/// `/lg.png`, and both were seen.
fn store(same_on_all_outputs: bool) -> Memory {
    let mut store = Memory::default();
    store.write(&Set::All, &picture("/all.png"));
    store.write(&Set::Output(aoc().identity), &picture("/aoc.png"));
    store.write(&Set::Output(lg().identity), &picture("/lg.png"));
    store.set_seen(&[aoc().identity.path_element(), lg().identity.path_element()]);
    store.set_same_on_all_outputs(same_on_all_outputs);
    store
}

#[test]
fn the_outputs_are_in_the_layouts_left_to_right_order() {
    let mut store = store(false);
    let page = Page::new(&mut store, vec![laptop(), aoc(), lg()], None);

    let order: Vec<&str> = page.outputs().iter().map(|o| o.title.as_str()).collect();
    assert_eq!(order, ["LG HDR 4K", "AOC G2490W1G4", "Built-in display"]);
}

#[test]
fn the_primary_output_is_selected_first_or_else_the_leftmost() {
    let mut store = store(false);

    let page = Page::new(&mut store, vec![aoc(), lg()], Some("HDMI-A-1"));
    assert_eq!(page.selected(), 1);
    assert_eq!(page.current(&store), picture("/aoc.png"));

    let page = Page::new(&mut store, vec![aoc(), lg()], None);
    assert_eq!(page.selected(), 0);
    assert_eq!(page.current(&store), picture("/lg.png"));
}

#[test]
fn selecting_an_output_shows_its_wallpaper_and_its_kind() {
    let mut store = store(false);
    store.write(
        &Set::Output(aoc().identity),
        &OutputSettings {
            kind: Kind::Colour,
            ..picture("/aoc.png")
        },
    );
    let mut page = Page::new(&mut store, vec![aoc(), lg()], None);
    assert_eq!(page.kind(), Kind::Picture);

    page.select(&store, 1);

    assert_eq!(page.selected(), 1);
    assert_eq!(page.kind(), Kind::Colour);
    assert_eq!(page.current(&store).picture, "/aoc.png");
}

#[test]
fn with_the_switch_off_a_click_changes_the_selected_output_alone() {
    let mut store = store(false);
    let mut page = Page::new(&mut store, vec![aoc(), lg()], None);
    page.select(&store, 1);

    page.choose_picture(&mut store, "/new.png");

    assert_eq!(page.wallpaper(&store, 1), picture("/new.png"));
    assert_eq!(page.wallpaper(&store, 0), picture("/lg.png"));
    assert_eq!(store.read(&Set::All), picture("/all.png"));
}

#[test]
fn with_the_switch_on_a_click_changes_every_output() {
    let mut store = store(true);
    let mut page = Page::new(&mut store, vec![aoc(), lg()], None);

    page.choose_picture(&mut store, "/new.png");

    assert_eq!(page.wallpaper(&store, 0), picture("/new.png"));
    assert_eq!(page.wallpaper(&store, 1), picture("/new.png"));
    assert_eq!(store.read(&Set::Output(lg().identity)), picture("/lg.png"));
}

#[test]
fn turning_the_switch_on_copies_the_selected_outputs_wallpaper_to_all() {
    let mut store = store(false);
    let mut page = Page::new(&mut store, vec![aoc(), lg()], None);
    page.select(&store, 1);

    page.set_same_on_all_outputs(&mut store, true);

    assert!(store.same_on_all_outputs());
    assert_eq!(store.read(&Set::All), picture("/aoc.png"));
    assert_eq!(page.wallpaper(&store, 1), picture("/aoc.png"));
    assert_eq!(page.wallpaper(&store, 1), picture("/aoc.png"));
}

#[test]
fn turning_the_switch_off_leaves_each_output_what_it_shows() {
    let mut store = store(true);
    let mut page = Page::new(&mut store, vec![aoc(), lg()], None);

    page.set_same_on_all_outputs(&mut store, false);

    assert!(!store.same_on_all_outputs());
    assert_eq!(page.wallpaper(&store, 0), picture("/all.png"));
    assert_eq!(page.wallpaper(&store, 1), picture("/all.png"));

    page.choose_picture(&mut store, "/lg-only.png");
    assert_eq!(page.wallpaper(&store, 1), picture("/all.png"));
}

#[test]
fn the_switch_shows_with_two_outputs_and_not_with_one() {
    let mut store = store(true);
    assert!(Page::new(&mut store, vec![aoc(), lg()], None).has_switch());
    assert!(!Page::new(&mut store, vec![aoc()], None).has_switch());
    assert!(!Page::new(&mut store, Vec::new(), None).has_switch());
}

#[test]
fn a_monitor_the_drawer_hasnt_seen_shows_what_it_will() {
    let mut store = store(false);
    let page = Page::new(&mut store, vec![aoc(), lg(), laptop()], Some("DP-1"));

    assert_eq!(page.wallpaper(&store, 2), picture("/lg.png"));
    assert!(store.seen().contains(&laptop().identity.path_element()));
}

#[test]
fn a_colour_and_a_positioning_are_kept_with_the_picture() {
    let mut store = store(false);
    let mut page = Page::new(&mut store, vec![aoc(), lg()], None);
    let red = Colour::parse("#c03030").unwrap();

    page.choose_positioning(&mut store, Positioning::Tile);
    page.choose_colour(&mut store, red);

    assert_eq!(page.kind(), Kind::Colour);
    assert_eq!(
        page.current(&store),
        OutputSettings {
            kind: Kind::Colour,
            picture: "/lg.png".into(),
            positioning: Positioning::Tile,
            colour: red,
            ..OutputSettings::default()
        }
    );

    page.show_kind(&mut store, Kind::Picture);
    assert_eq!(page.current(&store).kind, Kind::Picture);
}

#[test]
fn a_kind_not_built_yet_is_shown_and_not_written() {
    let mut store = store(false);
    let mut page = Page::new(&mut store, vec![aoc(), lg()], None);

    page.show_kind(&mut store, Kind::Slideshow);

    assert_eq!(page.kind(), Kind::Slideshow);
    assert_eq!(page.current(&store), picture("/lg.png"));
}

#[test]
fn picture_of_the_day_is_written_at_once_with_its_source() {
    let mut store = store(false);
    let mut page = Page::new(&mut store, vec![aoc(), lg()], None);

    page.show_kind(&mut store, Kind::PictureOfTheDay);
    assert_eq!(page.kind(), Kind::PictureOfTheDay);
    assert_eq!(
        page.current(&store),
        OutputSettings {
            kind: Kind::PictureOfTheDay,
            source: Source::Bing,
            ..picture("/lg.png")
        }
    );

    page.choose_source(&mut store, Source::Apod);
    assert_eq!(page.current(&store).source, Source::Apod);
    // The other output keeps its own.
    assert_eq!(page.wallpaper(&store, 1), picture("/aoc.png"));
}

#[test]
fn a_kept_picture_of_the_day_is_added_once_and_still_shown_as_the_day_s() {
    let mut store = store(true);
    let mut page = Page::new(&mut store, vec![aoc(), lg()], None);
    page.choose_source(&mut store, Source::Commons);

    let kept = "/home/me/Pictures/Wallpapers/2026-10-15 A heron.jpg";
    page.keep_picture(&mut store, kept);
    page.keep_picture(&mut store, kept);

    assert_eq!(store.pictures(), vec![kept.to_owned()]);
    assert_eq!(page.kind(), Kind::PictureOfTheDay);
    assert_eq!(page.current(&store).kind, Kind::PictureOfTheDay);
}

#[test]
fn picture_with_no_picture_chosen_is_shown_and_not_written() {
    let mut store = store(false);
    store.write(&Set::Output(lg().identity), &OutputSettings::default());
    let mut page = Page::new(&mut store, vec![aoc(), lg()], None);

    page.show_kind(&mut store, Kind::Picture);

    assert_eq!(page.kind(), Kind::Picture);
    assert_eq!(page.current(&store).kind, Kind::Colour);
}

#[test]
fn an_added_picture_is_listed_once_and_chosen() {
    let mut store = store(true);
    let mut page = Page::new(&mut store, vec![aoc(), lg()], None);

    page.add_picture(&mut store, "/home/me/sea.jpg");
    page.add_picture(&mut store, "/home/me/sea.jpg");
    page.add_folder(&mut store, "/home/me/Walls");
    page.add_folder(&mut store, "/home/me/Walls");

    assert_eq!(store.pictures(), ["/home/me/sea.jpg"]);
    assert_eq!(store.folders(), ["/home/me/Walls"]);
    assert_eq!(page.current(&store), picture("/home/me/sea.jpg"));
}

#[test]
fn a_dynamic_chosen_is_the_selected_outputs_and_keeps_its_positioning() {
    let mut store = store(false);
    let mut page = Page::new(&mut store, vec![aoc(), lg()], None);
    page.choose_positioning(&mut store, Positioning::Fit);

    page.choose_dynamic(&mut store, "/usr/share/wallpapers/Next");

    assert_eq!(page.kind(), Kind::Dynamic);
    assert_eq!(
        page.current(&store),
        OutputSettings {
            kind: Kind::Dynamic,
            dynamic: "/usr/share/wallpapers/Next".into(),
            positioning: Positioning::Fit,
            ..picture("/lg.png")
        }
    );
    assert_eq!(page.wallpaper(&store, 1), picture("/aoc.png"));

    // Back to Picture, and to Dynamic again: each shows what it had.
    page.show_kind(&mut store, Kind::Picture);
    assert_eq!(page.current(&store).kind, Kind::Picture);
    page.show_kind(&mut store, Kind::Dynamic);
    assert_eq!(page.current(&store).kind, Kind::Dynamic);
}

#[test]
fn dynamic_with_none_chosen_is_shown_and_not_written() {
    let mut store = store(false);
    let mut page = Page::new(&mut store, vec![aoc(), lg()], None);

    page.show_kind(&mut store, Kind::Dynamic);

    assert_eq!(page.kind(), Kind::Dynamic);
    assert_eq!(page.current(&store), picture("/lg.png"));
}

#[test]
fn an_added_dynamic_is_listed_once_and_chosen() {
    let mut store = store(true);
    let mut page = Page::new(&mut store, vec![aoc(), lg()], None);

    page.add_dynamic(&mut store, "/home/me/cosmos.xml");
    page.add_dynamic(&mut store, "/home/me/cosmos.xml");

    assert_eq!(store.dynamics(), ["/home/me/cosmos.xml"]);
    assert_eq!(page.current(&store).kind, Kind::Dynamic);
    assert_eq!(page.current(&store).dynamic, "/home/me/cosmos.xml");
}

#[test]
fn an_output_is_titled_as_displays_titles_it() {
    assert_eq!(laptop().title, "Built-in display");
    assert_eq!(
        Output::new("DP-3", "Unknown", "", "", (0, 0), (1, 1)).title,
        "DP-3"
    );
    assert_eq!(
        Output::new("DP-3", "", "Q27", "", (0, 0), (1, 1)).title,
        "Q27"
    );
}
