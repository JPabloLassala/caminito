//! caminito-displays against Caminito (84, ADR 0014): its page and its client of output
//! management, without its window. What the page drafts and the client applies is what
//! Caminito then shows, as the debug dump has it, and as the client reads it back. The
//! window's own part, the countdown and the calls to `org.mate.Caminito`, isn't here: 81 and
//! 83 test those calls. The monitors are the developer's (50), as in `saved_layouts`.

use caminito_displays::outputs::{Event, Label, Outcome, Outputs};
use caminito_displays::page::{Layout, Page, Scale};
use smithay::output::Output;

use super::fixture::Fixture;
use crate::settings::Value;
use crate::settings::translate::INTERFACE;

/// The AOC G2490W1G4: its EDID's size and preferred mode. Its default scale is 1.
const AOC: ((i32, i32), (i32, i32)) = ((527, 296), (1920, 1080));
/// The LG HDR 4K, whose default scale is 1.7.
const LG: ((i32, i32), (i32, i32)) = ((600, 340), (3840, 2160));

fn dump(f: &Fixture) -> serde_json::Value {
    serde_json::from_str(&f.state.caminito.state_dump()).unwrap()
}

/// Each output's name, place, logical size and scale, one per line, with a star on the
/// primary output, from the dump.
fn shown(f: &Fixture) -> String {
    let dump = dump(f);
    let primary = dump["primary_output"].as_str().unwrap_or_default();
    let line = |output: &serde_json::Value| {
        let (name, g) = (output["name"].as_str().unwrap(), &output["geometry"]);
        let star = if name == primary { " ★" } else { "" };
        let (x, y, w, h) = (&g["x"], &g["y"], &g["width"], &g["height"]);
        format!("{name} at {x},{y} {w}x{h} scale {}{star}", output["scale"])
    };
    let outputs = dump["outputs"].as_array().unwrap();
    outputs.iter().map(line).collect::<Vec<_>>().join("\n")
}

/// A layout of the page's as `shown` has Caminito's, its displays that are on.
fn drafted(layout: &Layout) -> String {
    let displays = layout.displays.iter().enumerate();
    let line = |(index, display): (usize, &caminito_displays::page::Display)| {
        let (name, rect) = (&display.name, display.rect());
        let star = if layout.is_primary(index) { " ★" } else { "" };
        let (x, y, w, h) = (rect.x, rect.y, rect.w, rect.h);
        let scale = display.scale.factor();
        display
            .enabled
            .then(|| format!("{name} at {x},{y} {w}x{h} scale {scale:?}{star}"))
    };
    displays.filter_map(line).collect::<Vec<_>>().join("\n")
}

/// caminito-displays, open: its connection, and its page.
struct Displays {
    outputs: Outputs,
    page: Page,
}

impl Displays {
    /// Opens caminito-displays on `f`'s Caminito, with the page showing what Caminito shows.
    fn open(f: &mut Fixture) -> Self {
        let outputs = Outputs::over(f.connect_stream()).unwrap();
        let mut displays = Displays {
            outputs,
            page: Page::default(),
        };
        displays.wait_for(f, Event::Changed);
        displays
    }

    /// Dispatches Caminito and the connection until the connection has `event`, and the
    /// page shows what Caminito shows by then, as the window does on a change.
    fn wait_for(&mut self, f: &mut Fixture, event: Event) {
        let mut events = Vec::new();
        f.dispatch_until_mut(|_| {
            events.extend(self.outputs.dispatch().unwrap());
            events.contains(&event)
        });
        self.look(f);
    }

    /// The page shows what Caminito shows, as the window has it after output management's
    /// `done` or `PrimaryOutput`'s signal.
    fn look(&mut self, f: &mut Fixture) {
        // Whatever else Caminito says of a change it has said by the next pass.
        f.dispatch();
        self.outputs.dispatch().unwrap();
        let primary = dump(f)["primary_output"].as_str().map(str::to_owned);
        self.page
            .refresh(&self.outputs.monitors(), primary.as_deref());
    }

    /// Presses Apply, with no primary display chosen in the window: the configuration
    /// goes over output management, and the primary output stays Caminito's to say.
    fn apply(&mut self, f: &mut Fixture) {
        let layout = self.page.draft().clone();
        self.show(f, &layout);
    }

    /// Goes back to `before`, as Revert and the end of the countdown do.
    fn revert(&mut self, f: &mut Fixture, before: &Layout) {
        self.show(f, before);
    }

    fn show(&mut self, f: &mut Fixture, layout: &Layout) {
        let configuration = layout.configuration(self.page.applied());
        self.outputs.apply(&configuration).unwrap();
        self.wait_for(f, Event::Applied(Outcome::Succeeded));
    }
}

/// Caminito with the AOC as `headless-1` and the LG as `headless-2`, and caminito-displays
/// open.
fn open_with_aoc_and_lg() -> (Fixture, Displays, Output, Output) {
    let mut f = Fixture::new();
    let aoc = f.plug(AOC.0, AOC.1);
    let lg = f.plug(LG.0, LG.1);
    let displays = Displays::open(&mut f);
    (f, displays, aoc, lg)
}

#[test]
fn the_page_shows_what_caminito_shows() {
    let (f, displays, _aoc, _lg) = open_with_aoc_and_lg();

    insta::assert_snapshot!(shown(&f), @"
    headless-1 at 0,0 1920x1080 scale 1.0 ★
    headless-2 at 1920,0 2259x1271 scale 1.7
    ");
    assert_eq!(drafted(displays.page.draft()), shown(&f));
    assert!(!displays.page.is_dirty());
    assert_eq!(
        displays.page.draft().displays[1].scale.label(),
        "170% (automatic)"
    );
}

/// A layout as text without its star: where the displays are.
fn places(layout: &str) -> String {
    layout.replace(" ★", "")
}

#[test]
fn a_scale_and_a_place_changed_and_applied_are_what_caminito_then_shows() {
    let (mut f, mut displays, _aoc, _lg) = open_with_aoc_and_lg();

    // The LG to 200%, and then to the left of the AOC.
    displays.page.set_scale(1, Scale::offered(200.0));
    displays.page.pick_up(1);
    displays.page.drag_to(-5000, 0, 0);
    displays.page.put_down();
    let drafted_layout = drafted(displays.page.draft());
    displays.apply(&mut f);

    // No primary output was chosen, so it's the leftmost (81), which is now the LG.
    insta::assert_snapshot!(shown(&f), @"
    headless-1 at 1920,0 1920x1080 scale 1.0
    headless-2 at 0,0 1920x1080 scale 2.0 ★
    ");
    // Caminito healed nothing: the page docks, heals and rounds as it does.
    assert_eq!(places(&drafted_layout), places(&shown(&f)));
    assert_eq!(drafted(displays.page.applied()), shown(&f));
    assert!(!displays.page.is_dirty());
}

#[test]
fn a_change_of_scale_alone_is_drafted_as_caminito_heals_it() {
    let (mut f, mut displays, _aoc, _lg) = open_with_aoc_and_lg();
    let third = f.plug(AOC.0, AOC.1);
    displays.wait_for(&mut f, Event::Changed);
    assert_eq!(third.name(), "headless-3");

    // The middle one grows: the one on its right keeps its side of it.
    displays.page.set_scale(1, Scale::offered(125.0));
    let drafted_layout = drafted(displays.page.draft());
    displays.apply(&mut f);

    insta::assert_snapshot!(shown(&f), @"
    headless-1 at 0,0 1920x1080 scale 1.0 ★
    headless-2 at 1920,0 3072x1728 scale 1.25
    headless-3 at 4992,0 1920x1080 scale 1.0
    ");
    assert_eq!(drafted_layout, shown(&f));
}

#[test]
fn reverting_puts_the_old_scale_and_place_back() {
    let (mut f, mut displays, _aoc, _lg) = open_with_aoc_and_lg();
    let before = displays.page.applied().clone();
    displays.page.set_scale(1, Scale::offered(200.0));
    displays.page.pick_up(0);
    displays.page.drag_to(0, 3000, 0);
    displays.page.put_down();
    displays.apply(&mut f);
    assert_ne!(drafted(&before), shown(&f));

    displays.revert(&mut f, &before);

    insta::assert_snapshot!(shown(&f), @"
    headless-1 at 0,0 1920x1080 scale 1.0 ★
    headless-2 at 1920,0 2259x1271 scale 1.7
    ");
    assert_eq!(displays.page.applied(), &before);
}

#[test]
fn a_scale_that_wasnt_changed_stays_caminitos_to_choose() {
    let (mut f, mut displays, _aoc, _lg) = open_with_aoc_and_lg();

    // The AOC's scale is set and the LG is only moved, so the LG's scale isn't sent. It
    // still follows `window-scaling-factor`, as a scale at Caminito's default does (58).
    displays.page.set_scale(0, Scale::offered(125.0));
    displays.page.pick_up(1);
    displays.page.drag_to(-9000, 0, 0);
    displays.page.put_down();
    displays.apply(&mut f);
    f.set_setting(INTERFACE, "window-scaling-factor", Value::Int(2));
    f.dispatch_until(|f| shown(f).contains("scale 2.0"));

    insta::assert_snapshot!(shown(&f), @"
    headless-1 at 1920,0 1536x864 scale 1.25
    headless-2 at 0,0 1920x1080 scale 2.0 ★
    ");
}

#[test]
fn a_display_turned_off_and_on_again_is_what_caminito_shows() {
    let (mut f, mut displays, _aoc, _lg) = open_with_aoc_and_lg();

    displays.page.set_enabled(0, false);
    displays.apply(&mut f);
    insta::assert_snapshot!(shown(&f), @"headless-2 at 0,0 2259x1271 scale 1.7 ★");
    assert!(!displays.page.draft().displays[0].enabled);
    assert!(!displays.page.can_turn_off(1));

    displays.page.set_enabled(0, true);
    let drafted_layout = drafted(displays.page.draft());
    displays.apply(&mut f);
    insta::assert_snapshot!(shown(&f), @"
    headless-2 at 0,0 2259x1271 scale 1.7 ★
    headless-1 at 2259,0 1920x1080 scale 1.0
    ");
    // Caminito lists the outputs in the order they were shown, the page by the monitors.
    let mut lines: Vec<&str> = drafted_layout.lines().collect();
    lines.reverse();
    assert_eq!(lines.join("\n"), shown(&f));
}

#[test]
fn a_resolution_changed_and_applied_is_the_mode_caminito_shows() {
    let mut f = Fixture::new();
    f.plug_with_modes(AOC.0, &[(1920, 1080), (1280, 720)]);
    f.plug(LG.0, LG.1);
    let mut displays = Displays::open(&mut f);
    let labels: Vec<String> = displays.page.draft().displays[0]
        .resolutions()
        .iter()
        .map(|resolution| resolution.label())
        .collect();
    assert_eq!(labels, ["1920 × 1080 (16:9), native", "1280 × 720 (16:9)"]);

    displays.page.set_resolution(0, 1280, 720);
    let drafted_layout = drafted(displays.page.draft());
    displays.apply(&mut f);

    insta::assert_snapshot!(shown(&f), @"
    headless-1 at 0,0 1280x720 scale 1.0 ★
    headless-2 at 1280,0 2259x1271 scale 1.7
    ");
    assert_eq!(drafted_layout, shown(&f));
}

#[test]
fn a_hot_plug_while_the_window_is_open_shows_the_new_set() {
    let (mut f, mut displays, _aoc, lg) = open_with_aoc_and_lg();
    displays.page.set_scale(0, Scale::offered(150.0));
    assert!(displays.page.is_dirty());

    f.unplug(&lg);
    displays.wait_for(&mut f, Event::Changed);

    assert_eq!(
        drafted(displays.page.draft()),
        "headless-1 at 0,0 1920x1080 scale 1.0 ★"
    );
    assert_eq!(displays.page.draft().displays.len(), 1);
    assert!(!displays.page.is_dirty());

    f.replug(&lg);
    displays.wait_for(&mut f, Event::Changed);
    assert_eq!(drafted(displays.page.draft()), shown(&f));
    assert_eq!(displays.page.draft().displays.len(), 2);
}

/// Each output's layer surfaces from the dump, one per line.
fn layers(f: &Fixture) -> String {
    let dump = dump(f);
    let mut lines = Vec::new();
    for output in dump["outputs"].as_array().unwrap() {
        for layer in output["layer_surfaces"].as_array().unwrap() {
            let g = &layer["geometry"];
            lines.push(format!(
                "{}: {} {} at {},{} {}x{} keyboard {}",
                output["name"].as_str().unwrap(),
                layer["layer"].as_str().unwrap(),
                layer["namespace"].as_str().unwrap(),
                g["x"],
                g["y"],
                g["width"],
                g["height"],
                layer["keyboard_interactivity"].as_str().unwrap(),
            ));
        }
    }
    lines.join("\n")
}

#[test]
fn identify_shows_a_label_in_the_middle_of_each_output_and_takes_it_away() {
    let (mut f, mut displays, _aoc, _lg) = open_with_aoc_and_lg();
    assert!(displays.outputs.can_identify());
    // 200 by 100 logical pixels each, the LG's drawn at twice that for its scale of 1.7.
    let label = |output: &str, scale: i32| Label {
        output: output.to_owned(),
        width: 200 * scale,
        height: 100 * scale,
        scale,
        pixels: vec![0xff; (200 * scale * 100 * scale * 4) as usize],
    };

    displays
        .outputs
        .identify(&[label("headless-1", 1), label("headless-2", 2)])
        .unwrap();
    f.dispatch_until_mut(|f| {
        displays.outputs.dispatch().unwrap();
        // Until each has its picture, and so its size.
        layers(f).lines().count() == 2 && !layers(f).contains(" 0x0 ")
    });

    insta::assert_snapshot!(layers(&f), @"
    headless-1: overlay caminito-displays-identify at 860,490 200x100 keyboard none
    headless-2: overlay caminito-displays-identify at 2949,585 200x100 keyboard none
    ");

    displays.outputs.end_identify().unwrap();
    f.dispatch_until_mut(|f| {
        displays.outputs.dispatch().unwrap();
        layers(f).is_empty()
    });
}
