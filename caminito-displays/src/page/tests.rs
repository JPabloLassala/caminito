//! The page's state, through what the window does to it and reads of it. The monitors are
//! the prototype's.

use caminito_layout::is_contiguous;
use proptest::prelude::*;

use super::*;

fn modes(modes: &[(i32, i32, &[i32])]) -> Vec<Mode> {
    let mut all = Vec::new();
    for &(width, height, rates) in modes {
        for &refresh in rates {
            all.push(Mode {
                width,
                height,
                refresh,
                preferred: all.is_empty(),
            });
        }
    }
    all
}

fn monitor(name: &str, model: &str, modes: Vec<Mode>, shows: Option<(i32, i32, f64)>) -> Monitor {
    Monitor {
        name: name.to_owned(),
        make: Some("Make".to_owned()),
        model: Some(model.to_owned()),
        modes,
        shows: shows.map(|(x, y, scale)| Shows {
            mode: 0,
            x,
            y,
            scale: Scale::from_factor(scale),
        }),
    }
}

/// A laptop's panel, 1920 × 1200 at 135%: 1422 × 889 logical pixels.
fn laptop(x: i32, y: i32) -> Monitor {
    let modes = modes(&[
        (1920, 1200, &[60_000]),
        (1680, 1050, &[60_000]),
        (1280, 800, &[60_000]),
    ]);
    monitor("eDP-1", "Panel", modes, Some((x, y, 1.35)))
}

/// A 4K monitor at 150%: 2560 × 1440 logical pixels.
fn dell(x: i32, y: i32) -> Monitor {
    let modes = modes(&[
        (3840, 2160, &[60_000, 30_000]),
        (2560, 1440, &[60_000]),
        (1920, 1080, &[60_000, 59_940, 50_000]),
    ]);
    monitor("DP-1", "U2723QE", modes, Some((x, y, 1.5)))
}

/// A 1080p monitor at 100%.
fn lg(x: i32, y: i32) -> Monitor {
    let modes = modes(&[
        (1920, 1080, &[144_000, 120_000, 60_000, 59_940]),
        (1280, 720, &[60_000]),
    ]);
    monitor("HDMI-A-1", "24GN600", modes, Some((x, y, 1.0)))
}

/// The three in a row, the laptop first, which is the primary display.
fn three() -> Page {
    Page::new(&[laptop(0, 0), dell(1422, 0), lg(3982, 0)], Some("eDP-1"))
}

/// Each display that's on, with its place and logical size, and a star on the primary one.
fn arrangement(page: &Page) -> String {
    let line = |(index, rect): (usize, Rect)| {
        let star = if page.draft().is_primary(index) {
            " ★"
        } else {
            ""
        };
        let name = &page.draft().displays[index].name;
        format!(
            "{name} at {},{} {}x{}{star}",
            rect.x, rect.y, rect.w, rect.h
        )
    };
    let lines: Vec<String> = page.arrangement().into_iter().map(line).collect();
    lines.join("\n")
}

fn rects(page: &Page) -> Vec<Rect> {
    page.arrangement()
        .into_iter()
        .map(|(_, rect)| rect)
        .collect()
}

#[test]
fn the_page_shows_what_caminito_shows_and_nothing_to_apply() {
    let page = three();

    assert_eq!(
        arrangement(&page),
        "eDP-1 at 0,0 1422x889 ★\nDP-1 at 1422,0 2560x1440\nHDMI-A-1 at 3982,0 1920x1080"
    );
    assert!(!page.is_dirty());
    assert_eq!(page.selected().unwrap().name, "eDP-1");
}

#[test]
fn a_logical_size_rounds_as_caminitos_does() {
    // The LG HDR 4K at 1.7, which Caminito has at 2259 × 1271 (83's tests).
    let mut monitor = dell(0, 0);
    monitor.shows.as_mut().unwrap().scale = Scale::from_factor(1.7);
    let page = Page::new(&[monitor], None);

    assert_eq!(page.draft().displays[0].logical_size(), (2259, 1271));
}

#[test]
fn a_scale_sent_as_a_fixed_snaps_to_what_it_was() {
    assert_eq!(Scale::from_factor(435.0 / 256.0), Scale::from_factor(1.7));
    assert_eq!(
        Scale::from_factor(346.0 / 256.0).label(),
        "135% (automatic)"
    );
}

#[test]
fn a_scale_off_the_grid_is_shown_as_automatic() {
    assert_eq!(Scale::from_factor(1.35).label(), "135% (automatic)");
    assert_eq!(Scale::from_factor(1.5).label(), "150%");
    assert_eq!(Scale::from_factor(3.0).label(), "300%");
    assert_eq!(Scale::from_factor(0.5).label(), "50% (automatic)");
}

#[test]
fn the_slider_offers_100_to_300_percent_in_steps_of_25() {
    assert_eq!(Scale::offered(135.0), Scale::from_factor(1.25));
    assert_eq!(Scale::offered(138.0), Scale::from_factor(1.5));
    assert_eq!(Scale::offered(20.0), Scale::ONE);
    assert_eq!(Scale::offered(400.0), Scale::from_factor(3.0));
}

#[test]
fn a_display_is_called_by_its_make_and_model_and_a_built_in_one_as_that() {
    let mut nameless = lg(3982, 0);
    (nameless.make, nameless.model) = (Some("Unknown".to_owned()), None);
    let page = Page::new(&[laptop(0, 0), dell(1422, 0), nameless], None);

    let titles: Vec<&str> = page
        .draft()
        .displays
        .iter()
        .map(|d| d.title.as_str())
        .collect();
    assert_eq!(titles, ["Built-in display", "Make U2723QE", "HDMI-A-1"]);
}

#[test]
fn a_resolution_row_has_its_aspect_ratio_and_the_native_one_is_marked() {
    let page = three();

    let labels: Vec<String> = page.draft().displays[1]
        .resolutions()
        .iter()
        .map(Resolution::label)
        .collect();
    assert_eq!(
        labels,
        [
            "3840 × 2160 (16:9), native",
            "2560 × 1440 (16:9)",
            "1920 × 1080 (16:9)"
        ]
    );
    assert_eq!(
        page.draft().displays[0].resolutions()[0].label(),
        "1920 × 1200 (16:10), native"
    );
    assert_eq!(aspect_ratio(1366, 768), "16:9");
    assert_eq!(aspect_ratio(3440, 1440), "21:9");
    assert_eq!(aspect_ratio(1280, 1024), "5:4");
    assert_eq!(aspect_ratio(1000, 300), "10:3");
}

#[test]
fn the_rates_are_those_of_the_chosen_resolution_the_highest_first() {
    let mut page = three();
    assert_eq!(page.draft().displays[1].rates(), [60_000, 30_000]);

    page.set_resolution(1, 1920, 1080);

    assert_eq!(page.draft().displays[1].rates(), [60_000, 59_940, 50_000]);
    assert_eq!(rate_label(60_000), "60 Hz");
    assert_eq!(rate_label(59_940), "59.94 Hz");
    assert_eq!(rate_label(143_856), "143.86 Hz");
}

/// The window gives a list new entries only when they change, and can't while the list
/// is being chosen from (85).
#[test]
fn choosing_from_a_list_leaves_its_entries_as_they_were() {
    let mut page = three();
    for index in 0..page.draft().displays.len() {
        let resolutions = page.draft().displays[index].resolutions();
        for resolution in &resolutions {
            page.set_resolution(index, resolution.width, resolution.height);
            assert_eq!(page.draft().displays[index].resolutions(), resolutions);

            let rates = page.draft().displays[index].rates();
            for &rate in &rates {
                page.set_rate(index, rate);
                assert_eq!(page.draft().displays[index].rates(), rates);
            }
        }
    }
}

#[test]
fn a_new_resolution_takes_the_rate_nearest_the_current_one() {
    let mut page = three();
    page.set_rate(2, 120_000);

    page.set_resolution(2, 1280, 720);
    assert_eq!(page.draft().displays[2].mode().unwrap().refresh, 60_000);

    page.set_resolution(2, 1920, 1080);
    assert_eq!(page.draft().displays[2].mode().unwrap().refresh, 60_000);
}

#[test]
fn of_two_rates_equally_near_a_new_resolution_takes_the_higher() {
    let modes = modes(&[(1920, 1080, &[60_000]), (1280, 720, &[50_000, 70_000])]);
    let mut page = Page::new(&[monitor("DP-1", "M", modes, Some((0, 0, 1.0)))], None);

    page.set_resolution(0, 1280, 720);

    assert_eq!(page.draft().displays[0].mode().unwrap().refresh, 70_000);
}

#[test]
fn a_larger_scale_keeps_the_display_on_the_right_on_its_side() {
    let mut page = three();

    page.set_scale(1, Scale::from_factor(2.0));

    assert_eq!(
        arrangement(&page),
        "eDP-1 at 0,0 1422x889 ★\nDP-1 at 1422,0 1920x1080\nHDMI-A-1 at 3342,0 1920x1080"
    );
    assert!(page.is_dirty());
}

#[test]
fn a_smaller_resolution_keeps_the_display_below_on_its_side() {
    let mut page = Page::new(&[dell(0, 0), lg(0, 1440)], Some("DP-1"));

    page.set_resolution(0, 1920, 1080);

    assert_eq!(
        arrangement(&page),
        "DP-1 at 0,0 1280x720 ★\nHDMI-A-1 at 0,720 1920x1080"
    );
}

#[test]
fn turning_the_middle_display_off_closes_the_gap() {
    let mut page = three();

    page.set_enabled(1, false);

    assert_eq!(
        arrangement(&page),
        "eDP-1 at 0,0 1422x889 ★\nHDMI-A-1 at 1422,0 1920x1080"
    );
}

#[test]
fn a_display_turned_on_goes_to_the_right_of_the_others() {
    let mut off = dell(0, 0);
    off.shows = None;
    let mut page = Page::new(&[laptop(0, 0), off, lg(1422, 0)], Some("eDP-1"));
    assert_eq!(page.draft().displays[1].scale, Scale::ONE);

    page.set_enabled(1, true);

    assert_eq!(
        arrangement(&page),
        "eDP-1 at 0,0 1422x889 ★\nDP-1 at 3342,0 3840x2160\nHDMI-A-1 at 1422,0 1920x1080"
    );
}

#[test]
fn the_only_display_that_is_on_cant_be_turned_off() {
    let mut page = three();
    page.set_enabled(0, false);
    page.set_enabled(1, false);
    assert!(!page.can_turn_off(2));
    assert!(page.can_turn_off(0));

    page.set_enabled(2, false);

    assert_eq!(arrangement(&page), "HDMI-A-1 at 0,0 1920x1080 ★");
}

#[test]
fn with_the_primary_display_off_the_built_in_one_is_primary_else_the_leftmost() {
    let mut page = Page::new(&[lg(0, 0), dell(1920, 0), laptop(4480, 0)], Some("DP-1"));

    page.set_enabled(1, false);
    assert_eq!(page.draft().primary.as_deref(), Some("eDP-1"));

    page.set_enabled(1, true);
    page.set_primary(1);
    page.set_enabled(2, false);
    page.set_enabled(1, false);
    assert_eq!(page.draft().primary.as_deref(), Some("HDMI-A-1"));
}

#[test]
fn a_stand_in_for_the_primary_display_isnt_the_users_choice() {
    let mut page = three();
    assert_eq!(page.chosen_primary(), None);

    page.set_primary(2);
    assert_eq!(page.chosen_primary(), Some("HDMI-A-1"));

    page.set_enabled(2, false);
    assert_eq!(page.draft().primary.as_deref(), Some("eDP-1"));
    assert_eq!(page.chosen_primary(), None);

    page.set_primary(1);
    page.reset();
    assert_eq!(page.chosen_primary(), None);
}

#[test]
fn a_display_that_is_off_cant_be_the_primary_one() {
    let mut page = three();
    page.set_enabled(1, false);

    page.set_primary(1);

    assert_eq!(page.draft().primary.as_deref(), Some("eDP-1"));
}

#[test]
fn a_change_of_the_primary_display_alone_is_told_apart() {
    let mut page = three();

    page.set_primary(2);
    assert!(page.is_dirty());
    assert!(page.only_primary_differs());

    page.set_scale(2, Scale::from_factor(1.25));
    assert!(!page.only_primary_differs());

    page.reset();
    assert!(!page.is_dirty());
    assert_eq!(page.draft().primary.as_deref(), Some("eDP-1"));
}

#[test]
fn the_others_close_up_as_soon_as_a_display_is_picked_up() {
    let mut page = three();

    page.pick_up(1);
    page.drag_to(0, 40, 0);

    let others: Vec<Rect> = [0, 2].map(|i| page.draft().displays[i].rect()).to_vec();
    assert!(is_contiguous(&others));
    assert_eq!(page.selected().unwrap().name, "DP-1");
    assert_eq!(page.dragged_from().unwrap().1.x, 1422);
}

#[test]
fn a_dragged_display_docks_to_the_outline_of_the_others() {
    let mut page = Page::new(&[laptop(0, 0), lg(1422, 0)], Some("eDP-1"));

    // Far off to the lower right: it stays against the laptop, sharing 100 pixels.
    page.pick_up(1);
    page.drag_to(900, 2000, 0);
    page.put_down();

    assert_eq!(
        arrangement(&page),
        "eDP-1 at 0,0 1422x889 ★\nHDMI-A-1 at 1322,889 1920x1080"
    );
    assert!(page.dragged_from().is_none());
}

#[test]
fn a_dragged_display_snaps_to_the_matching_edge_of_the_one_it_lies_against() {
    let mut page = Page::new(&[laptop(0, 0), lg(1422, 0)], Some("eDP-1"));

    // 80 pixels short of bottom against bottom, which is at -191.
    page.pick_up(1);
    page.drag_to(0, -111, 100);
    page.put_down();
    assert_eq!(
        arrangement(&page),
        "eDP-1 at 0,191 1422x889 ★\nHDMI-A-1 at 1422,0 1920x1080"
    );

    page.reset();
    page.pick_up(1);
    page.drag_to(0, -111, 12);
    page.put_down();
    assert_eq!(
        arrangement(&page),
        "eDP-1 at 0,111 1422x889 ★\nHDMI-A-1 at 1422,0 1920x1080"
    );
}

#[test]
fn a_display_dragged_left_of_the_primary_one_puts_the_layout_back_at_the_origin() {
    let mut page = Page::new(&[laptop(0, 0), lg(1422, 0)], Some("eDP-1"));

    page.pick_up(1);
    page.drag_to(-4000, 0, 12);
    page.put_down();

    assert_eq!(
        arrangement(&page),
        "eDP-1 at 1920,0 1422x889 ★\nHDMI-A-1 at 0,0 1920x1080"
    );
}

#[test]
fn a_configuration_sends_a_scale_only_where_it_changes() {
    let mut page = three();
    page.set_scale(1, Scale::from_factor(2.0));
    page.set_enabled(0, false);

    let wanted = page.configuration();

    assert_eq!(wanted[0].shows, None);
    let dell = wanted[1].shows.unwrap();
    assert_eq!(
        (dell.mode.width, dell.x, dell.scale),
        (3840, 0, Some(Scale::from_factor(2.0)))
    );
    let lg = wanted[2].shows.unwrap();
    assert_eq!((lg.x, lg.y, lg.scale), (1920, 0, None));
}

#[test]
fn a_display_turned_on_is_sent_its_scale() {
    let mut off = lg(0, 0);
    off.shows = None;
    let mut page = Page::new(&[laptop(0, 0), off], Some("eDP-1"));

    page.set_enabled(1, true);

    assert_eq!(
        page.configuration()[1].shows.unwrap().scale,
        Some(Scale::ONE)
    );
}

#[test]
fn a_hot_plug_shows_the_new_set_and_drops_what_wasnt_applied() {
    let mut page = three();
    page.select(1);
    page.set_scale(1, Scale::from_factor(3.0));

    page.refresh(&[laptop(0, 0), dell(1422, 0)], Some("eDP-1"));

    assert!(!page.is_dirty());
    assert_eq!(
        arrangement(&page),
        "eDP-1 at 0,0 1422x889 ★\nDP-1 at 1422,0 2560x1440"
    );
    assert_eq!(page.selected().unwrap().name, "DP-1");

    page.refresh(&[laptop(0, 0)], Some("eDP-1"));
    assert_eq!(page.selected().unwrap().name, "eDP-1");
}

#[test]
fn hearing_again_of_what_caminito_shows_already_drops_nothing() {
    let mut page = three();
    page.set_scale(1, Scale::from_factor(3.0));
    page.pick_up(1);

    let changed = page.refresh(&[laptop(0, 0), dell(1422, 0), lg(3982, 0)], Some("eDP-1"));

    assert!(!changed);
    assert!(page.is_dirty());
    assert!(page.dragged_from().is_some());
    assert!(page.refresh(&[laptop(0, 0), dell(1422, 0), lg(3982, 0)], Some("DP-1")));
    assert!(!page.is_dirty());
}

#[test]
fn a_display_that_went_off_keeps_the_mode_and_scale_it_had() {
    let mut page = three();
    let mut off = dell(0, 0);
    off.shows = None;

    page.refresh(&[laptop(0, 0), off, lg(1422, 0)], Some("eDP-1"));
    page.set_enabled(1, true);

    assert_eq!(page.draft().displays[1].scale, Scale::from_factor(1.5));
}

#[test]
fn a_page_with_no_monitors_has_nothing_selected() {
    let page = Page::new(&[], None);

    assert!(page.selected().is_none());
    assert!(page.configuration().is_empty());
}

/// What the window can do to the page.
#[derive(Debug, Clone)]
enum Act {
    Enable(usize, bool),
    Primary(usize),
    Resolution(usize, usize),
    Scale(usize, u32),
    Drag(usize, i32, i32, i32),
}

fn act() -> impl Strategy<Value = Act> {
    let place = -6000..6000i32;
    prop_oneof![
        (0..3usize, any::<bool>()).prop_map(|(i, on)| Act::Enable(i, on)),
        (0..3usize).prop_map(Act::Primary),
        (0..3usize, 0..3usize).prop_map(|(i, r)| Act::Resolution(i, r)),
        (0..3usize, 4..=12u32).prop_map(|(i, quarters)| Act::Scale(i, quarters * 25)),
        (0..3usize, place.clone(), place, 0..200i32).prop_map(|(i, x, y, s)| Act::Drag(i, x, y, s)),
    ]
}

proptest! {
    /// The acceptance criterion: no gap, no overlap and no display cut off, by dragging or
    /// by any setting, while a display is in the air too.
    #[test]
    fn whatever_the_window_does_the_arrangement_stays_contiguous(acts in prop::collection::vec(act(), 1..24)) {
        let mut page = three();
        for act in acts {
            match act {
                Act::Enable(i, on) => page.set_enabled(i, on),
                Act::Primary(i) => page.set_primary(i),
                Act::Resolution(i, r) => {
                    let resolutions = page.draft().displays[i].resolutions();
                    let r = resolutions[r % resolutions.len()];
                    page.set_resolution(i, r.width, r.height);
                }
                Act::Scale(i, percent) => page.set_scale(i, Scale::offered(f64::from(percent))),
                Act::Drag(i, x, y, snap) => {
                    page.pick_up(i);
                    page.drag_to(x, y, snap);
                    prop_assert!(is_contiguous(&rects(&page)), "in the air: {}", arrangement(&page));
                    page.put_down();
                }
            }
            let rects = rects(&page);
            prop_assert!(!rects.is_empty());
            prop_assert!(is_contiguous(&rects), "{}", arrangement(&page));
            prop_assert_eq!(rects.iter().map(|r| r.x).min(), Some(0));
            prop_assert_eq!(rects.iter().map(|r| r.y).min(), Some(0));
            let primary = page.draft().primary.clone().unwrap();
            prop_assert!(page.draft().displays.iter().any(|d| d.enabled && d.name == primary));
        }
    }
}
