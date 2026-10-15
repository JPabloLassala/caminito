//! The Overview's timing, for comparing its first opening in a Session with later ones:
//! when the shortcut fired, what the layout and each label took, and each frame of the
//! transition, how long its elements and its rendering took and when its vblank came.
//! Logged at `debug!` under [`TARGET`], so `RUST_LOG=info,caminito::overview::timing=debug`
//! shows these lines alone.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// The target of the timing's lines.
pub(crate) const TARGET: &str = "caminito::overview::timing";

/// What the timing remembers between the shortcut and the transition's frames.
#[derive(Default)]
pub(crate) struct Timing {
    /// How many times the shortcut opened the Overview, this one included.
    openings: u32,
    /// When the shortcut last fired.
    pressed: Option<Instant>,
    /// Each output's last frame since the shortcut, by its name: its number and when it
    /// was drawn.
    frames: HashMap<String, (u32, Instant)>,
}

impl Timing {
    /// The shortcut fired, and it opens the Overview if `opens`, or closes it.
    pub(crate) fn pressed(&mut self, opens: bool) {
        self.pressed = Some(Instant::now());
        self.frames.clear();
        if opens {
            self.openings += 1;
            tracing::debug!(target: TARGET, "shortcut: opening #{}", self.openings);
        } else {
            tracing::debug!(target: TARGET, "shortcut: closing after opening #{}", self.openings);
        }
    }

    /// How long ago the shortcut fired, in milliseconds.
    pub(crate) fn since_pressed(&self) -> f64 {
        self.pressed
            .map_or(f64::NAN, |pressed| ms(pressed.elapsed()))
    }

    /// The opening's layouts on every output are ready, `took` after the refresh pass began
    /// them, with `drawn` labels drawn anew and `kept` kept.
    pub(crate) fn laid_out(&self, outputs: usize, took: Duration, drawn: usize, kept: usize) {
        tracing::debug!(
            target: TARGET,
            "laid out on {outputs} outputs in {:.2} ms, {drawn} labels drawn, {kept} kept, \
             {:.2} ms after the shortcut",
            ms(took),
            self.since_pressed(),
        );
    }

    /// A held opening's clock started: its first frame is on screen on every output.
    pub(crate) fn started(&self) {
        tracing::debug!(
            target: TARGET,
            "clock started, {:.2} ms after the shortcut",
            self.since_pressed(),
        );
    }

    /// `output` couldn't take a frame of the transition yet: its last is still queued.
    pub(crate) fn busy(&self, output: &str) {
        tracing::debug!(
            target: TARGET,
            "{output}: busy, {:.2} ms after the shortcut",
            self.since_pressed(),
        );
    }

    /// A frame of the transition on `output`, drawn `part` of the way open: its `elements`
    /// took `build` to make and `render` to render, and it was queued unless `empty`.
    /// Returns its number since the shortcut.
    pub(crate) fn frame(
        &mut self,
        output: &str,
        part: f64,
        elements: usize,
        build: Duration,
        render: Duration,
        empty: bool,
    ) -> u32 {
        let now = Instant::now();
        let (number, gap) = match self.frames.get(output) {
            Some((last, at)) => (last + 1, ms(now - *at)),
            None => (1, f64::NAN),
        };
        self.frames.insert(output.to_owned(), (number, now));
        tracing::debug!(
            target: TARGET,
            "{output}: frame {number} at part {part:.3}, {:.2} ms after the shortcut, \
             {gap:.2} ms after the last, {elements} elements built in {:.2} ms, rendered in \
             {:.2} ms{}",
            self.since_pressed(),
            ms(build),
            ms(render),
            if empty { ", no damage" } else { "" },
        );
        number
    }
}

/// A frame of the transition on `output`, number `number`, reached the screen `queued`
/// after it was queued.
pub(crate) fn vblank(output: &str, number: u32, queued: Instant) {
    tracing::debug!(
        target: TARGET,
        "{output}: frame {number} shown {:.2} ms after it was queued",
        ms(queued.elapsed()),
    );
}

/// What one label took to draw, the `index`th of its layout: its icon's load, its title's
/// layout and its drawing.
pub(crate) fn label(index: usize, icon: Duration, title: Duration, draw: Duration, found: bool) {
    tracing::debug!(
        target: TARGET,
        "label {index}: icon in {:.2} ms{}, title in {:.2} ms, drawn in {:.2} ms",
        ms(icon),
        if found { "" } else { " (none)" },
        ms(title),
        ms(draw),
    );
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}
