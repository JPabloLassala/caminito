//! The palette helper under the `helpers` supervisor (28), with fake helper scripts in
//! place of `caminito-gtk-palette`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use caminito_theme::{
    GtkState, PALETTE_STREAM_HEADER, Palette, PaletteCache, PaletteColors, Rgba, StateColors,
    Variant, format_report,
};
use smithay::reexports::calloop::EventLoop;

use crate::decoration::palette::{PaletteSource, PaletteState};
use crate::helpers::{HelperCommand, RestartPolicy, StderrTarget};

struct TestState {
    palette: PaletteSource<TestState>,
    /// Every palette the source put in use, in order.
    changes: Vec<Palette>,
}

impl PaletteState for TestState {
    fn palette_source(&mut self) -> &mut PaletteSource<TestState> {
        &mut self.palette
    }

    fn palette_changed(&mut self) {
        let palette = self.palette.palette().clone();
        self.changes.push(palette);
    }
}

/// A fake helper in a scratch directory. Each run appends to `runs` and records the names
/// line it's sent in `names`, then writes `stream-<run>` to stdout if there is one, and
/// exits if `exit-<run>` exists or waits for stdin to close otherwise.
struct FakeHelper {
    dir: tempfile::TempDir,
}

impl FakeHelper {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-helper");
        fs::write(
            &script,
            r#"#!/bin/sh
dir=$(dirname "$0")
echo run >>"$dir/runs"
run=$(wc -l <"$dir/runs")
IFS= read -r names
printf '%s\n' "$names" >>"$dir/names"
[ -e "$dir/stream-$run" ] && cat "$dir/stream-$run"
[ -e "$dir/exit-$run" ] && exit 1
exec cat >/dev/null
"#,
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        FakeHelper { dir }
    }

    fn command(&self) -> HelperCommand {
        HelperCommand::new("fake-helper", self.dir.path().join("fake-helper"))
    }

    /// Run `run` reports `palettes` for `gtk_theme`.
    fn reports(&self, run: usize, gtk_theme: &str, palettes: &[&Palette]) {
        let mut stream = format!("{PALETTE_STREAM_HEADER}\n");
        for palette in palettes {
            stream.push_str(&format_report(gtk_theme, palette));
        }
        fs::write(self.dir.path().join(format!("stream-{run}")), stream).unwrap();
    }

    /// Run `run` writes this to stdout.
    fn writes(&self, run: usize, stream: &str) {
        fs::write(self.dir.path().join(format!("stream-{run}")), stream).unwrap();
    }

    /// Run `run` exits after its stream.
    fn crashes(&self, run: usize) {
        fs::write(self.dir.path().join(format!("exit-{run}")), "").unwrap();
    }

    fn runs(&self) -> usize {
        fs::read_to_string(self.dir.path().join("runs"))
            .map(|runs| runs.lines().count())
            .unwrap_or(0)
    }

    fn names_lines(&self) -> Vec<String> {
        fs::read_to_string(self.dir.path().join("names"))
            .map(|names| names.lines().map(str::to_owned).collect())
            .unwrap_or_default()
    }

    fn cache_dir(&self) -> PathBuf {
        self.dir.path().join("cache")
    }
}

fn policy() -> RestartPolicy {
    RestartPolicy {
        first_delay: Duration::from_millis(10),
        max_delay: Duration::from_millis(40),
        give_up_after: 5,
        window: Duration::from_secs(60),
    }
}

fn colors(level: u8) -> PaletteColors {
    let states = GtkState::ALL.map(|state| StateColors {
        background: Rgba::from_rgb8(level, level, state as u8),
        text: Rgba::from_rgb8(255 - level, 0, 0),
    });
    let named = [("wm_bg".to_owned(), Rgba::from_rgb8(level, 1, 2))].into();
    PaletteColors::new(states, named)
}

fn palette(level: u8) -> Palette {
    Palette {
        default: colors(level),
        dark: colors(level + 100),
    }
}

struct Run {
    event_loop: EventLoop<'static, TestState>,
    state: TestState,
}

impl Run {
    fn new(fake: &FakeHelper, cache: Option<&Path>, gtk_theme: Option<&str>) -> Self {
        let event_loop = EventLoop::try_new().unwrap();
        let palette = PaletteSource::new(
            event_loop.handle(),
            fake.command(),
            policy(),
            StderrTarget::Prefixed,
            cache.map(PaletteCache::new),
            gtk_theme,
        );
        let mut state = TestState {
            palette,
            changes: Vec::new(),
        };
        state
            .palette
            .set_names(["wm_bg", "wm_title"].map(String::from).into());
        state.palette.start();
        Run { event_loop, state }
    }

    /// Runs the loop until `done`, failing after five seconds.
    fn until(&mut self, what: &str, mut done: impl FnMut(&TestState) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !done(&self.state) {
            assert!(Instant::now() < deadline, "timed out waiting until {what}");
            self.event_loop
                .dispatch(Duration::from_millis(10), &mut self.state)
                .unwrap();
        }
    }

    /// Runs the loop for a while, for things that shouldn't happen.
    fn for_a_while(&mut self) {
        let deadline = Instant::now() + Duration::from_millis(300);
        while Instant::now() < deadline {
            self.event_loop
                .dispatch(Duration::from_millis(10), &mut self.state)
                .unwrap();
        }
    }
}

#[test]
fn a_reported_palette_is_put_in_use() {
    let fake = FakeHelper::new();
    fake.reports(1, "Menta", &[&palette(1)]);
    let mut run = Run::new(&fake, None, None);
    assert_eq!(run.state.palette.palette(), &Palette::neutral());

    run.until("a palette arrives", |state| !state.changes.is_empty());
    assert_eq!(run.state.changes, vec![palette(1)]);
    assert_eq!(run.state.palette.gtk_theme(), Some("Menta"));
    assert_eq!(fake.names_lines(), vec!["names wm_bg wm_title"]);
}

#[test]
fn a_crashed_helper_is_restarted_and_asked_again_while_its_palette_stays() {
    let fake = FakeHelper::new();
    fake.reports(1, "Menta", &[&palette(1)]);
    fake.crashes(1);
    fake.reports(2, "Menta", &[&palette(2)]);
    let mut run = Run::new(&fake, None, None);

    run.until("the restarted run reports", |state| {
        state.changes.len() == 2
    });
    assert_eq!(run.state.changes, vec![palette(1), palette(2)]);
    assert_eq!(fake.runs(), 2);
    assert_eq!(
        fake.names_lines(),
        vec!["names wm_bg wm_title", "names wm_bg wm_title"]
    );
}

#[test]
fn a_helper_that_keeps_exiting_is_given_up_on_and_its_last_palette_stays() {
    let fake = FakeHelper::new();
    fake.reports(1, "Menta", &[&palette(1)]);
    for run in 1..=10 {
        fake.crashes(run);
    }
    let mut run = Run::new(&fake, None, None);

    run.until("five runs", |_| fake.runs() == 5);
    run.for_a_while();
    assert_eq!(fake.runs(), 5, "no run after giving up");
    assert_eq!(run.state.changes, vec![palette(1)]);
    assert_eq!(run.state.palette.palette(), &palette(1));
}

#[test]
fn a_palette_cut_off_by_a_crash_leaves_the_last_one_in_use() {
    let fake = FakeHelper::new();
    let whole = format!(
        "{PALETTE_STREAM_HEADER}\n{}",
        format_report("Menta", &palette(1))
    );
    let cut = format_report("Menta", &palette(2));
    fake.writes(1, &format!("{whole}{}", &cut[..cut.len() / 2]));
    fake.crashes(1);
    let mut run = Run::new(&fake, None, None);

    run.until("the first run exits and restarts", |_| fake.runs() == 2);
    run.for_a_while();
    assert_eq!(run.state.changes, vec![palette(1)]);
}

#[test]
fn the_cached_palette_is_used_until_the_helper_reports_and_reports_are_cached() {
    let fake = FakeHelper::new();
    let cache = PaletteCache::new(fake.cache_dir());
    cache.store("Menta", Variant::Default, &colors(7)).unwrap();
    // The helper never reports.
    fake.crashes(1);
    let mut run = Run::new(&fake, Some(&fake.cache_dir()), Some("Menta"));

    // Only the default variant was cached.
    let cached = Palette {
        default: colors(7),
        dark: Palette::neutral().dark,
    };
    assert_eq!(run.state.palette.palette(), &cached);
    run.until("the first run exits and restarts", |_| fake.runs() == 2);
    assert_eq!(run.state.palette.palette(), &cached);

    drop(run);
    let fake_2 = FakeHelper::new();
    fake_2.reports(1, "BlackMATE", &[&palette(3)]);
    let mut run = Run::new(&fake_2, Some(&fake.cache_dir()), Some("Menta"));
    run.until("a palette arrives", |state| !state.changes.is_empty());
    assert_eq!(
        cache.load("BlackMATE", Variant::Default),
        Ok(Some(colors(3)))
    );
    assert_eq!(
        cache.load("BlackMATE", Variant::Dark),
        Ok(Some(colors(103)))
    );
}
