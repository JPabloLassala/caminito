//! The page's state, with no toolkit and no protocol in it: the layout Caminito shows, the
//! layout in the window, which the user edits and nothing has applied yet, and every way
//! the page changes it. Each change leaves the layout in the window contiguous (82).
//!
//! The user reads "display" in caminito-displays, so an output is a display here (CONTEXT.md).
//!
//! Ported from the page of the caminito-displays prototype
//! (`.scratch/caminito-displays-prototype/index.html` at 8a4981d).

use caminito_layout::{Rect, at_origin, at_the_right, dock, heal, resized};

#[cfg(test)]
mod tests;

/// One of a monitor's modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Mode {
    /// In pixels.
    pub width: i32,
    /// In pixels.
    pub height: i32,
    /// In mHz.
    pub refresh: i32,
    /// Whether it's the one the monitor prefers, its native one.
    pub preferred: bool,
}

/// An output scale, in the 120ths of fractional-scale-v1, which Caminito snaps every scale
/// to (13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Scale(u32);

impl Scale {
    /// 100%.
    pub const ONE: Scale = Scale(120);
    /// The smallest scale the page offers, in percent.
    pub const MIN_PERCENT: u32 = 100;
    /// The largest scale the page offers, in percent.
    pub const MAX_PERCENT: u32 = 300;
    /// The step between the scales the page offers, in percent (ADR 0014).
    pub const STEP_PERCENT: u32 = 25;

    /// The scale nearest to `factor`. Output management sends a scale as a 24.8 fixed, so
    /// 1.7 comes as 435/256, and snaps back to what it was.
    pub fn from_factor(factor: f64) -> Scale {
        Scale((factor * 120.0).round().max(1.0) as u32)
    }

    /// The scale the page offers nearest to `percent`.
    pub fn offered(percent: f64) -> Scale {
        let step = f64::from(Self::STEP_PERCENT);
        let percent = (percent / step).round() * step;
        let percent = percent.clamp(f64::from(Self::MIN_PERCENT), f64::from(Self::MAX_PERCENT));
        Scale::from_factor(percent / 100.0)
    }

    /// From logical to physical pixels.
    pub fn factor(self) -> f64 {
        f64::from(self.0) / 120.0
    }

    /// In percent, to the nearest.
    pub fn percent(self) -> u32 {
        (self.0 * 100 + 60) / 120
    }

    /// Whether the page offers it.
    pub fn is_offered(self) -> bool {
        let percent = self.0 * 100;
        percent.is_multiple_of(120 * Self::STEP_PERCENT)
            && (Self::MIN_PERCENT..=Self::MAX_PERCENT).contains(&(percent / 120))
    }

    /// As the page shows it. A scale it doesn't offer is one Caminito's default rule picked
    /// (ADR 0008), or `wlr-randr` set.
    pub fn label(self) -> String {
        if self.is_offered() {
            format!("{}%", self.percent())
        } else {
            format!("{}% (automatic)", self.percent())
        }
    }
}

/// What Caminito shows of one connected monitor, as output management has it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monitor {
    /// The output's name, its connector's.
    pub name: String,
    /// Its EDID's make, if it says.
    pub make: Option<String>,
    /// Its EDID's model, if it says.
    pub model: Option<String>,
    /// Its modes, in the order Caminito lists them.
    pub modes: Vec<Mode>,
    /// What it shows, unless it's disabled.
    pub shows: Option<Shows>,
}

/// What an enabled output shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shows {
    /// Which of the monitor's modes.
    pub mode: usize,
    /// Its left edge in the layout.
    pub x: i32,
    /// Its top edge in the layout.
    pub y: i32,
    /// Its output scale.
    pub scale: Scale,
}

/// One display of a layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Display {
    /// The output's name, its connector's.
    pub name: String,
    /// What the user reads for it.
    pub title: String,
    /// Its modes.
    pub modes: Vec<Mode>,
    /// Whether it's on.
    pub enabled: bool,
    /// Which of its modes it shows, or would if it were on.
    pub mode: usize,
    /// Its scale, or the one it would have if it were on.
    pub scale: Scale,
    /// Its left edge in the layout, while it's on.
    pub x: i32,
    /// Its top edge in the layout, while it's on.
    pub y: i32,
}

impl Display {
    /// The mode it shows, or would if it were on. `None` for a monitor with no modes.
    pub fn mode(&self) -> Option<Mode> {
        self.modes.get(self.mode).copied()
    }

    /// Its size in logical pixels, rounded as Caminito's `output_geometry` rounds it (82):
    /// each axis of the mode over the scale, half away from zero.
    pub fn logical_size(&self) -> (i32, i32) {
        let logical = |pixels: i32| (f64::from(pixels) / self.scale.factor()).round() as i32;
        self.mode()
            .map_or((0, 0), |mode| (logical(mode.width), logical(mode.height)))
    }

    /// Its place and size in the layout.
    pub fn rect(&self) -> Rect {
        let (w, h) = self.logical_size();
        Rect {
            x: self.x,
            y: self.y,
            w,
            h,
        }
    }

    /// Whether it's a laptop's or a phone's own panel, by its connector, as Caminito tells
    /// (81).
    pub fn is_built_in(&self) -> bool {
        is_built_in(&self.name)
    }

    /// The sizes of its modes, each once, in the order of the modes.
    pub fn resolutions(&self) -> Vec<Resolution> {
        let mut resolutions: Vec<Resolution> = Vec::new();
        for mode in &self.modes {
            let same = |r: &&mut Resolution| (r.width, r.height) == (mode.width, mode.height);
            match resolutions.iter_mut().find(same) {
                Some(resolution) => resolution.native |= mode.preferred,
                None => resolutions.push(Resolution {
                    width: mode.width,
                    height: mode.height,
                    native: mode.preferred,
                }),
            }
        }
        resolutions
    }

    /// The refresh rates of the resolution it shows, in mHz, the highest first.
    pub fn rates(&self) -> Vec<i32> {
        let Some(shown) = self.mode() else {
            return Vec::new();
        };
        let mut rates: Vec<i32> = self
            .modes_of(shown.width, shown.height)
            .map(|(_, mode)| mode.refresh)
            .collect();
        rates.sort_unstable_by(|a, b| b.cmp(a));
        rates.dedup();
        rates
    }

    fn modes_of(&self, width: i32, height: i32) -> impl Iterator<Item = (usize, &Mode)> {
        let of_size =
            move |(_, mode): &(usize, &Mode)| (mode.width, mode.height) == (width, height);
        self.modes.iter().enumerate().filter(of_size)
    }
}

fn is_built_in(connector: &str) -> bool {
    ["eDP-", "LVDS-", "DSI-"]
        .iter()
        .any(|kind| connector.starts_with(kind))
}

/// What the user reads for a monitor: a built-in panel as that, another by its make and
/// model, and one whose EDID says neither by its connector.
fn title(monitor: &Monitor) -> String {
    if is_built_in(&monitor.name) {
        return "Built-in display".to_owned();
    }
    // What Caminito's outputs say for what the EDID doesn't.
    let said = |text: &Option<String>| {
        let text = text.as_deref().unwrap_or_default().trim();
        (!text.is_empty() && text != "Unknown").then(|| text.to_owned())
    };
    match (said(&monitor.make), said(&monitor.model)) {
        (Some(make), Some(model)) => format!("{make} {model}"),
        (None, Some(model)) => model,
        (Some(make), None) => make,
        (None, None) => monitor.name.clone(),
    }
}

/// One size among a display's modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    /// In pixels.
    pub width: i32,
    /// In pixels.
    pub height: i32,
    /// Whether the monitor prefers a mode of this size.
    pub native: bool,
}

impl Resolution {
    /// As a row of the page has it: "1920 × 1200 (16:10), native".
    pub fn label(&self) -> String {
        let native = if self.native { ", native" } else { "" };
        format!(
            "{} × {} ({}){native}",
            self.width,
            self.height,
            aspect_ratio(self.width, self.height)
        )
    }
}

/// A size's aspect ratio as monitors are sold by it. 1366 × 768 is 16:9 to its buyer, and
/// 8:5 is 16:10.
fn aspect_ratio(width: i32, height: i32) -> String {
    const SOLD_AS: [(i32, i32); 8] = [
        (16, 9),
        (16, 10),
        (4, 3),
        (5, 4),
        (3, 2),
        (21, 9),
        (32, 9),
        (1, 1),
    ];
    if width <= 0 || height <= 0 {
        return "?".to_owned();
    }
    // Within 1.5%: 21:9 monitors are 2.37 or 2.39 to 1, not 2.33.
    let near = |&&(w, h): &&(i32, i32)| {
        let (have, sold) = (
            i64::from(width) * i64::from(h),
            i64::from(height) * i64::from(w),
        );
        (have - sold).abs() * 1000 <= sold * 15
            || (w, h) == (21, 9) && (have - sold).abs() * 100 <= sold * 4
    };
    if let Some((w, h)) = SOLD_AS.iter().find(near) {
        return format!("{w}:{h}");
    }
    let divisor = gcd(width, height);
    format!("{}:{}", width / divisor, height / divisor)
}

fn gcd(a: i32, b: i32) -> i32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// A refresh rate in mHz as a row of the page has it: "60 Hz", "59.94 Hz".
pub fn rate_label(refresh: i32) -> String {
    let hundredths = (refresh + 5) / 10;
    if hundredths % 100 == 0 {
        format!("{} Hz", hundredths / 100)
    } else {
        format!("{}.{:02} Hz", hundredths / 100, hundredths % 100)
    }
}

/// The connected monitors' displays, and which of them is the primary one.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Layout {
    /// One per connected monitor, the ones that are off among them.
    pub displays: Vec<Display>,
    /// The primary display's name. `None` when nobody says, as without Caminito.
    pub primary: Option<String>,
}

impl Layout {
    /// The indices of the displays that are on.
    fn enabled(&self) -> Vec<usize> {
        let on = |&index: &usize| self.displays[index].enabled;
        (0..self.displays.len()).filter(on).collect()
    }

    /// Whether `index` is the primary display.
    pub fn is_primary(&self, index: usize) -> bool {
        self.primary.as_deref() == Some(self.displays[index].name.as_str())
    }

    /// The names of the displays, on or off.
    fn names(&self) -> Vec<&str> {
        self.displays.iter().map(|d| d.name.as_str()).collect()
    }

    /// What decides whether two layouts differ: a display that's off has no place, and
    /// the mode and scale it would have aren't shown.
    fn shown(&self) -> Vec<Option<(usize, Scale, i32, i32)>> {
        let shown = |d: &Display| d.enabled.then_some((d.mode, d.scale, d.x, d.y));
        self.displays.iter().map(shown).collect()
    }

    /// The configuration that makes a compositor showing `shown` show this layout. A
    /// scale is sent only where it changes, so that one that was Caminito's to choose stays
    /// so (58).
    pub fn configuration(&self, shown: &Layout) -> Vec<Wanted> {
        let wanted = |display: &Display| {
            let had = shown
                .displays
                .iter()
                .find(|other| other.name == display.name && other.enabled)
                .map(|other| other.scale);
            let shows = display
                .mode()
                .filter(|_| display.enabled)
                .map(|mode| WantedShows {
                    mode,
                    x: display.x,
                    y: display.y,
                    scale: (had != Some(display.scale)).then_some(display.scale),
                });
            Wanted {
                name: display.name.clone(),
                shows,
            }
        };
        self.displays.iter().map(wanted).collect()
    }
}

/// What a configuration asks of one monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    /// The output's name.
    pub name: String,
    /// What it's to show, or `None` to turn it off.
    pub shows: Option<WantedShows>,
}

/// What a configuration asks an enabled output to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WantedShows {
    /// One of the monitor's modes.
    pub mode: Mode,
    /// Its left edge in the layout.
    pub x: i32,
    /// Its top edge in the layout.
    pub y: i32,
    /// Its scale, if it changes.
    pub scale: Option<Scale>,
}

/// A display that's picked up, and the layout it was picked up from.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Drag {
    index: usize,
    /// Where it was, and its size.
    origin: Rect,
    /// The other displays that are on, where they were.
    others: Vec<(usize, Rect)>,
}

/// The page: what Caminito shows, and what the window shows, which differs until Apply.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Page {
    draft: Layout,
    applied: Layout,
    selected: usize,
    drag: Option<Drag>,
    /// Whether the window's primary display is one the user chose here, and not the one
    /// Caminito has or a stand-in for one that was turned off.
    chose_primary: bool,
}

impl Page {
    /// The page for what Caminito shows now.
    pub fn new(monitors: &[Monitor], primary: Option<&str>) -> Page {
        let mut page = Page::default();
        page.refresh(monitors, primary);
        page
    }

    /// Caminito says what it shows. If that's something else, a hot-plug, a configuration
    /// applied or another primary output, the window shows it and what wasn't applied is
    /// dropped, with the selected display still selected if it's there. If it's what
    /// Caminito showed already, nothing changes. Returns whether it was something else.
    pub fn refresh(&mut self, monitors: &[Monitor], primary: Option<&str>) -> bool {
        let selected = self.selected().map(|display| display.name.clone());
        let display = |monitor: &Monitor| {
            // A monitor that's off says no mode and no scale. It takes the ones it had
            // while this window has been open, else its native mode at 100%.
            let was = self
                .applied
                .displays
                .iter()
                .find(|d| d.name == monitor.name && d.modes == monitor.modes);
            let native = monitor.modes.iter().position(|mode| mode.preferred);
            let off = Shows {
                mode: was.map_or(native.unwrap_or(0), |d| d.mode),
                scale: was.map_or(Scale::ONE, |d| d.scale),
                x: 0,
                y: 0,
            };
            let shows = monitor.shows.unwrap_or(off);
            Display {
                name: monitor.name.clone(),
                title: title(monitor),
                modes: monitor.modes.clone(),
                enabled: monitor.shows.is_some(),
                mode: shows.mode,
                scale: shows.scale,
                x: shows.x,
                y: shows.y,
            }
        };
        let shown = Layout {
            displays: monitors.iter().map(display).collect(),
            primary: primary.filter(|name| !name.is_empty()).map(str::to_owned),
        };
        if shown == self.applied {
            return false;
        }
        self.applied = shown;
        self.draft = self.applied.clone();
        self.drag = None;
        self.chose_primary = false;
        let still_there = |name: String| self.draft.displays.iter().position(|d| d.name == name);
        self.selected = selected.and_then(still_there).unwrap_or(0);
        true
    }

    /// The layout in the window.
    pub fn draft(&self) -> &Layout {
        &self.draft
    }

    /// The layout Caminito shows.
    pub fn applied(&self) -> &Layout {
        &self.applied
    }

    /// Which display's settings the page shows.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// The display whose settings the page shows. `None` with no monitors.
    pub fn selected(&self) -> Option<&Display> {
        self.draft.displays.get(self.selected)
    }

    /// Shows the settings of the display at `index`.
    pub fn select(&mut self, index: usize) {
        if index < self.draft.displays.len() {
            self.selected = index;
        }
    }

    /// Whether the window shows what Caminito doesn't yet.
    pub fn is_dirty(&self) -> bool {
        self.draft.primary != self.applied.primary || self.draft.shown() != self.applied.shown()
    }

    /// Whether the primary display is all that the window changes, which is applied and
    /// saved at once, with nothing to keep or revert.
    pub fn only_primary_differs(&self) -> bool {
        self.draft.primary != self.applied.primary && self.draft.shown() == self.applied.shown()
    }

    /// Whether the two layouts are of the same monitors.
    pub fn same_monitors(&self, layout: &Layout) -> bool {
        self.applied.names() == layout.names()
    }

    /// Drops what wasn't applied.
    pub fn reset(&mut self) {
        self.draft = self.applied.clone();
        self.drag = None;
        self.chose_primary = false;
    }

    /// The configuration Apply sends.
    pub fn configuration(&self) -> Vec<Wanted> {
        self.draft.configuration(&self.applied)
    }

    /// Whether the display at `index` can be turned off: the only one that's on can't.
    pub fn can_turn_off(&self, index: usize) -> bool {
        self.draft.enabled().len() > 1 || !self.draft.displays[index].enabled
    }

    /// Turns the display at `index` on or off. A display that's turned on goes to the
    /// right of the others, as a new output does in Caminito (82). When the primary display
    /// is turned off, the built-in one stands in, else the leftmost, as in Caminito (81).
    pub fn set_enabled(&mut self, index: usize, enabled: bool) {
        if self.draft.displays[index].enabled == enabled || !(enabled || self.can_turn_off(index)) {
            return;
        }
        self.drag = None;
        if enabled {
            let (w, h) = self.draft.displays[index].logical_size();
            let place = at_the_right(w, h, &self.rects());
            let display = &mut self.draft.displays[index];
            (display.x, display.y, display.enabled) = (place.x, place.y, true);
        } else {
            self.draft.displays[index].enabled = false;
            // Before the others close up: the leftmost is the one that was.
            if self.draft.is_primary(index) {
                self.draft.primary = self.stand_in();
                self.chose_primary = false;
            }
        }
        self.heal();
    }

    /// The display that's primary when the chosen one is off.
    fn stand_in(&self) -> Option<String> {
        let enabled = self.draft.enabled();
        let displays = &self.draft.displays;
        let built_in = enabled.iter().find(|&&i| displays[i].is_built_in());
        let leftmost = enabled.iter().min_by_key(|&&i| displays[i].x);
        built_in.or(leftmost).map(|&i| displays[i].name.clone())
    }

    /// Makes the display at `index` the primary one, if it's on and Caminito has one.
    pub fn set_primary(&mut self, index: usize) {
        if self.draft.displays[index].enabled && self.draft.primary.is_some() {
            self.draft.primary = Some(self.draft.displays[index].name.clone());
            self.chose_primary = self.draft.primary != self.applied.primary;
        }
    }

    /// The primary display, if the user chose it in this window. A stand-in for a display
    /// that was turned off isn't a choice: Caminito puts its own in (81), and the user's
    /// choice returns with its monitor.
    pub fn chosen_primary(&self) -> Option<&str> {
        self.draft.primary.as_deref().filter(|_| self.chose_primary)
    }

    /// Gives the display at `index` the resolution of `width` by `height`, at the rate
    /// nearest the one it has, the higher of two equally near.
    pub fn set_resolution(&mut self, index: usize, width: i32, height: i32) {
        let display = &self.draft.displays[index];
        let has = display.mode().map_or(0, |mode| mode.refresh);
        let nearest = display
            .modes_of(width, height)
            .min_by_key(|(_, mode)| (mode.refresh.abs_diff(has), -mode.refresh))
            .map(|(mode, _)| mode);
        if let Some(mode) = nearest {
            self.resize(index, |display| display.mode = mode);
        }
    }

    /// Gives the display at `index` the mode of its resolution at `refresh` mHz.
    pub fn set_rate(&mut self, index: usize, refresh: i32) {
        let display = &self.draft.displays[index];
        let Some(shown) = display.mode() else {
            return;
        };
        let at_rate = display
            .modes_of(shown.width, shown.height)
            .find(|(_, mode)| mode.refresh == refresh)
            .map(|(mode, _)| mode);
        if let Some(mode) = at_rate {
            self.draft.displays[index].mode = mode;
        }
    }

    /// Gives the display at `index` the scale `scale`.
    pub fn set_scale(&mut self, index: usize, scale: Scale) {
        self.resize(index, |display| display.scale = scale);
    }

    /// Changes what decides a display's size. The displays to its right and below keep
    /// their side of it, and then the layout is healed.
    fn resize(&mut self, index: usize, change: impl FnOnce(&mut Display)) {
        self.drag = None;
        let enabled = self.draft.enabled();
        let before = self.rects();
        change(&mut self.draft.displays[index]);
        let Some(among) = enabled.iter().position(|&i| i == index) else {
            return;
        };
        let (w, h) = self.draft.displays[index].logical_size();
        self.place(&enabled, &resized(&before, among, w, h));
        self.heal();
    }

    /// The displays that are on, as the arrangement draws them, with their indices.
    pub fn arrangement(&self) -> Vec<(usize, Rect)> {
        let displays = &self.draft.displays;
        let rect = |index: usize| (index, displays[index].rect());
        self.draft.enabled().into_iter().map(rect).collect()
    }

    fn rects(&self) -> Vec<Rect> {
        self.arrangement()
            .into_iter()
            .map(|(_, rect)| rect)
            .collect()
    }

    fn place(&mut self, indices: &[usize], rects: &[Rect]) {
        for (&index, rect) in indices.iter().zip(rects) {
            let display = &mut self.draft.displays[index];
            (display.x, display.y) = (rect.x, rect.y);
        }
    }

    /// Among `indices`, the one healing holds still: the primary display, else the first.
    fn anchor(&self, indices: &[usize]) -> usize {
        let primary = |&index: &usize| self.draft.is_primary(index);
        indices.iter().position(primary).unwrap_or(0)
    }

    /// Heals the layout in the window, and puts its top-left corner at 0,0.
    fn heal(&mut self) {
        let enabled = self.draft.enabled();
        let healed = heal(&self.rects(), self.anchor(&enabled));
        self.place(&enabled, &at_origin(&healed));
    }

    /// Picks up the display at `index`, to drag it, and selects it.
    pub fn pick_up(&mut self, index: usize) {
        self.select(index);
        if !self.draft.displays[index].enabled {
            return;
        }
        let arrangement = self.arrangement();
        let others = arrangement.iter().filter(|(i, _)| *i != index);
        self.drag = Some(Drag {
            index,
            origin: self.draft.displays[index].rect(),
            others: others.copied().collect(),
        });
    }

    /// Where the display being dragged was picked up, if one is.
    pub fn dragged_from(&self) -> Option<(usize, Rect)> {
        self.drag.as_ref().map(|drag| (drag.index, drag.origin))
    }

    /// Moves the display being dragged to `dx`, `dy` logical pixels from where it was
    /// picked up. The others close up without it, and it attaches to them, within `snap`
    /// logical pixels of a matching edge on that edge (82).
    pub fn drag_to(&mut self, dx: i32, dy: i32, snap: i32) {
        let Some(drag) = self.drag.clone() else {
            return;
        };
        let (indices, rects): (Vec<usize>, Vec<Rect>) = drag.others.iter().copied().unzip();
        if indices.is_empty() {
            return;
        }
        let others = heal(&rects, self.anchor(&indices));
        let wanted = Rect {
            x: drag.origin.x.saturating_add(dx),
            y: drag.origin.y.saturating_add(dy),
            ..drag.origin
        };
        let docked = dock(wanted, &others, snap);
        self.place(&indices, &others);
        self.place(&[drag.index], &[docked]);
    }

    /// Puts down the display being dragged, where it is.
    pub fn put_down(&mut self) {
        if self.drag.take().is_some() {
            self.heal();
        }
    }
}
