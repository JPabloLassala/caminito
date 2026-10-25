//! Output lifecycle: making outputs, picking their default scales, keeping their layout
//! contiguous (82), and telling policy about them (15, 51), and changing them as output
//! management asks (58) and as a saved layout has them (83).

pub(crate) mod primary;
pub(crate) mod saved;

use std::time::Duration;

use caminito_layout::Rect;
use caminito_policy::{OutputId, PolicyWindow, WindowId};
use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::wayland_server::backend::GlobalId;
use smithay::utils::{Logical, Physical, Point, Raw, Rectangle, Size, Transform};

use crate::protocols::output_management::{Head, HeadRequest, HeadState};
use crate::settings::translate::WindowScalingFactor;
use crate::state::{Caminito, State};

/// What an output's make, model or serial says when its monitor's EDID doesn't.
pub(crate) const UNKNOWN_EDID_TEXT: &str = "Unknown";

/// The refresh rate backends without a real one report, in mHz.
pub const DEFAULT_REFRESH: i32 = 60_000;

/// An output made by Caminito rather than a monitor's connector, whose EDID says it's
/// `physical_size` millimetres, with one mode of `size`, which it prefers.
pub fn virtual_output(
    name: String,
    model: &str,
    physical_size: Size<i32, Raw>,
    size: Size<i32, Physical>,
    transform: Transform,
) -> Output {
    let output = Output::new(
        name,
        PhysicalProperties {
            size: physical_size,
            subpixel: Subpixel::Unknown,
            make: "Caminito".to_owned(),
            model: model.to_owned(),
            serial_number: String::new(),
        },
    );
    output.change_current_state(None, Some(transform), None, None);
    output.set_preferred(Mode {
        size,
        refresh: DEFAULT_REFRESH,
    });
    output
}

/// Policy's id for an output added through `Caminito::add_output`.
pub fn output_id(output: &Output) -> OutputId {
    *output
        .user_data()
        .get::<OutputId>()
        .expect("the output was added through add_output")
}

/// An output's place and size in the global logical space.
pub fn output_geometry(output: &Output) -> Rectangle<i32, Logical> {
    let size = output
        .current_mode()
        .map(|mode| {
            output
                .current_transform()
                .transform_size(mode.size)
                .to_f64()
                .to_logical(output.current_scale().fractional_scale())
                .to_i32_round()
        })
        .unwrap_or_default();
    Rectangle::new(output.current_location(), size)
}

/// What Caminito knows of a monitor when it picks the monitor's default scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Monitor {
    /// The EDID's size, in millimetres.
    pub(crate) physical_size: Size<i32, Raw>,
    /// The size of the mode the monitor will show.
    pub(crate) mode_size: Size<i32, Physical>,
    /// Whether it's built in, such as a laptop's panel.
    pub(crate) internal: bool,
    /// Whether libinput has a lid switch, which makes an internal monitor a laptop's.
    pub(crate) lid_switch: bool,
}

impl Monitor {
    /// What an output's EDID and the mode it shows say, and whether libinput has a lid
    /// switch (56).
    fn of(output: &Output, lid_switch: bool) -> Self {
        Monitor {
            physical_size: output.physical_properties().size,
            mode_size: output
                .current_mode()
                .map(|mode| mode.size)
                .unwrap_or_default(),
            internal: is_internal(&output.name()),
            lid_switch,
        }
    }
}

/// Whether a connector is built in, by its DRM connector type: a laptop's or a phone's
/// panel.
fn is_internal(connector: &str) -> bool {
    ["eDP-", "LVDS-", "DSI-"]
        .iter()
        .any(|kind| connector.starts_with(kind))
}

/// The scale Caminito gives a monitor no output-management client has set (13, ADR 0008):
/// `window-scaling-factor`'s integer when it forces one, and otherwise KWin's rule.
pub(crate) fn default_scale(monitor: &Monitor, factor: WindowScalingFactor) -> f64 {
    match factor {
        WindowScalingFactor::Auto => kwin_scale(monitor),
        WindowScalingFactor::Forced(scale) => f64::from(scale),
    }
}

/// KWin's `chooseScale`, ported as it is (13), from `src/outputconfigurationstore.cpp:859-931`
/// at KWin commit 79ea0aa5. Each device type has its own target DPI, for how far the eyes
/// are from it. Ticket 13's summary gives every output 800 logical pixels, but KWin keeps
/// 360 for an internal output without a lid, a phone's.
fn kwin_scale(monitor: &Monitor) -> f64 {
    let Monitor {
        physical_size: mm,
        mode_size: px,
        internal,
        lid_switch,
    } = *monitor;
    // Less than 3 mm is a monitor misreporting its size.
    if mm.w < 3 || mm.h < 3 {
        return 1.0;
    }
    let (target_dpi, min_size) = match (internal, lid_switch) {
        (true, true) => (125.0, 800.0),
        (true, false) => (150.0, 360.0),
        // Height rather than the diagonal, so ultrawide monitors aren't TVs. 30.5 gives a
        // 77" 4K TV a scale of 2.
        (false, _) if mm.h > 500 => (30.5, 800.0),
        (false, _) => (96.0, 800.0),
    };
    let axis = |px: i32, mm: i32| {
        let px = f64::from(px);
        let dpi = px / (f64::from(mm) / 25.4);
        let max = (px / min_size).clamp(1.0, 3.0);
        (dpi / target_dpi).clamp(1.0, max)
    };
    round_scale(axis(px.w, mm.w).min(axis(px.h, mm.h)))
}

/// Rounds a raw scale to KWin's 5% steps (6/120, exact in fractional-scale-v1). Scales just
/// above 1 look blurry for little gain, so they become 1, and integer scales are sharper
/// than their near neighbours, so those snap to the integer.
fn round_scale(raw: f64) -> f64 {
    let scale = (100.0 * raw / 5.0).round() * 5.0 / 100.0;
    if scale < 1.20 {
        return 1.0;
    }
    let integer = scale.round();
    if (integer - scale).abs() < 0.06 {
        integer
    } else {
        scale
    }
}

/// The X11 scale for outputs at `scales` (ADR 0008): the largest of their rounded-up
/// scales, so X11 windows are sharp on the densest output, or 1 with no outputs.
pub(crate) fn x11_scale(scales: impl IntoIterator<Item = f64>) -> u32 {
    scales
        .into_iter()
        .map(|scale| scale.ceil() as u32)
        .fold(1, u32::max)
}

/// What a configuration changes of one output (58).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OutputChange {
    pub(crate) output: Output,
    /// What it's to show, or `None` to disable it.
    pub(crate) to: Option<OutputSettings>,
}

/// What a configuration has an enabled output show.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OutputSettings {
    /// One of the output's modes.
    pub(crate) mode: Mode,
    /// The place, if the configuration moves the output. Caminito heals the layout around
    /// it (82).
    pub(crate) position: Option<Point<i32, Logical>>,
    /// The scale, if the configuration sets it. Unset, it stays as it is, and stays
    /// Caminito's to choose if it was.
    pub(crate) scale: Option<f64>,
}

/// The scales a configuration may set. Outside them, an output's logical size is of no use,
/// and Caminito's own defaults are from 1 to 3 (50).
const SETTABLE_SCALES: std::ops::RangeInclusive<f64> = 0.5..=8.0;

/// The coordinates a configuration may place an output at. Healing brings any place back
/// to the others, so these only keep the sums of places and sizes within an `i32`.
const SETTABLE_COORDINATES: std::ops::RangeInclusive<i32> = -(1 << 24)..=1 << 24;

/// The one of an output's `modes` a configuration means by `asked`: that mode, or for a
/// custom mode with no refresh rate, the first of its size.
fn matching_mode(modes: &[Mode], asked: Mode) -> Option<Mode> {
    modes
        .iter()
        .find(|mode| {
            mode.size == asked.size && (asked.refresh == 0 || mode.refresh == asked.refresh)
        })
        .copied()
}

/// Who chose an output's scale (13). Its place has no such mark: whoever placed it, Caminito
/// heals the layout around it (82).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChosenBy {
    /// Caminito's default, which it may change.
    Caminito,
    /// A client, over output management, or the saved layout (83), which Caminito leaves
    /// alone.
    OutputManagement,
}

/// What Caminito keeps of an output beside Smithay's state. It goes with the output, so a
/// monitor plugged in again starts at the defaults (06).
#[derive(Debug)]
pub(crate) struct OutputRecord {
    /// What the default scale is picked from.
    monitor: Monitor,
    scale: ChosenBy,
    /// Its logical size when the outputs were last laid out, which tells laying out that
    /// it changed size since.
    size: Size<i32, Logical>,
    global: GlobalId,
}

/// How long a removed output's global stays disabled before it's destroyed. A client may
/// bind it before it hears the global is gone, and binding a destroyed global is an error.
const GLOBAL_REMOVAL_DELAY: Duration = Duration::from_secs(10);

impl Caminito {
    /// The X11 scale for the outputs there are now.
    pub(crate) fn x11_scale(&self) -> u32 {
        x11_scale(
            self.outputs
                .iter()
                .map(|output| output.current_scale().fractional_scale()),
        )
    }

    /// Starts showing an output the backend found: in its preferred mode, at its default
    /// scale, and to the right of the outputs there are, at the layout's top (82).
    pub fn add_output(&mut self, output: Output) {
        if self.lock.is_locked() {
            tracing::debug!(target: crate::lock::TARGET, "{} was plugged in while locked", output.name());
        }
        self.monitors_changed = true;
        let mode = output.preferred_mode();
        self.show_output(output, mode);
    }

    /// Starts showing again an output that output management disabled, in `mode`, at the
    /// defaults as a new output is (51).
    pub fn enable_output(&mut self, output: &Output, mode: Mode) {
        self.disabled_outputs.retain(|o| o != output);
        self.show_output(output.clone(), Some(mode));
    }

    fn show_output(&mut self, output: Output, mode: Option<Mode>) {
        // An output enabled again keeps its id.
        let next = &mut self.next_output_id;
        let id = *output.user_data().get_or_insert(|| {
            *next += 1;
            OutputId(*next - 1)
        });

        output.change_current_state(mode, None, None, None);
        let monitor = Monitor::of(&output, self.input_devices.has_lid_switch());
        let scale = default_scale(&monitor, self.settings.interface.window_scaling_factor);
        output.change_current_state(None, None, Some(Scale::Fractional(scale)), None);
        let size = output_geometry(&output).size;
        // Where it may touch no other output, or too little of one. Laying out docks it.
        let place = caminito_layout::at_the_right(size.w, size.h, &self.layout());
        output.change_current_state(None, None, None, Some((place.x, place.y).into()));
        let global = output.create_global::<State>(&self.display_handle);
        self.output_records.insert(
            id,
            OutputRecord {
                monitor,
                scale: ChosenBy::Caminito,
                size,
                global,
            },
        );
        self.outputs.push(output);
        self.night_light_output_added(id);
        self.lay_out_outputs();
    }

    /// Stops showing an output that output management disabled, as if it was unplugged
    /// (51), but keeps it as a head that can be enabled again (58).
    pub fn disable_output(&mut self, output: &Output) {
        self.hide_output(output);
        self.disabled_outputs.push(output.clone());
    }

    /// Stops showing an output that was unplugged. A disabled output is only forgotten.
    pub fn remove_output(&mut self, output: &Output) {
        if self.lock.is_locked() {
            tracing::debug!(target: crate::lock::TARGET, "{} was unplugged while locked", output.name());
        }
        self.monitors_changed = true;
        if let Some(index) = self.disabled_outputs.iter().position(|o| o == output) {
            self.disabled_outputs.remove(index);
            return;
        }
        self.hide_output(output);
    }

    /// Stops showing an output. Policy moves its windows onto the others, and the others
    /// close the gap it leaves (82).
    fn hide_output(&mut self, output: &Output) {
        let id = self.output_id(output);
        self.outputs.retain(|o| o != output);
        self.redraws.remove(&id);
        self.night_light_output_removed(id);
        let record = self
            .output_records
            .remove(&id)
            .expect("every output has a record");

        let ids: Vec<WindowId> = self.policy.windows().map(|mapped| mapped.id()).collect();
        for window in ids {
            self.policy.window_mut(window).unwrap().leave_output(output);
        }
        self.close_layers(output);
        output.leave_all();

        self.globals_to_withdraw.push(record.global);

        self.lay_out_outputs();
    }

    /// Tells the clients that the globals of the outputs hidden since the last refresh
    /// are gone. It's after the views that name a `wl_output` said the output left:
    /// libxfce4windowing can't take a monitor out of a workspace group once the global
    /// is removed, and reads it, freed, at the group's next change (91). The appmenu
    /// global goes the same way when it stops being offered (112).
    pub(crate) fn refresh_output_globals(&mut self) {
        for global in std::mem::take(&mut self.globals_to_withdraw) {
            self.display_handle.disable_global::<State>(global.clone());
            self.loop_handle
                .insert_source(
                    Timer::from_duration(GLOBAL_REMOVAL_DELAY),
                    move |_, _, state| {
                        state
                            .caminito
                            .display_handle
                            .remove_global::<State>(global.clone());
                        TimeoutAction::Drop
                    },
                )
                .expect("the timer joins the loop");
        }
    }

    /// Sets an output's scale, as output management does. `window-scaling-factor` leaves
    /// it alone from now on.
    pub fn set_output_scale(&mut self, output: &Output, scale: f64) {
        output.change_current_state(None, None, Some(Scale::Fractional(scale)), None);
        self.record_mut(output).scale = ChosenBy::OutputManagement;
        self.lay_out_outputs();
    }

    /// Asks for an output's place, as output management does. It ends up the nearest to
    /// there that keeps the layout contiguous (82).
    pub fn set_output_position(&mut self, output: &Output, position: Point<i32, Logical>) {
        output.change_current_state(None, None, None, Some(position));
        self.lay_out_outputs();
    }

    /// `window-scaling-factor` changed: the outputs still at Caminito's default scale take
    /// the new default (13).
    pub(crate) fn window_scaling_factor_changed(&mut self, factor: WindowScalingFactor) {
        self.take_default_scales(factor);
    }

    /// If a lid switch came or went since the outputs' scales were picked, the outputs
    /// still at Caminito's default scale take the default for a laptop's panel, or for a panel
    /// without a lid (50). In the refresh pass, since switching VTs unplugs the lid switch
    /// and plugs it in again in one dispatch, and outputs shouldn't flip scales and back.
    pub(crate) fn refresh_lid_switch(&mut self) {
        let lid_switch = self.input_devices.has_lid_switch();
        let mut changed = false;
        for record in self.output_records.values_mut() {
            changed |= record.monitor.lid_switch != lid_switch;
            record.monitor.lid_switch = lid_switch;
        }
        if changed {
            self.take_default_scales(self.settings.interface.window_scaling_factor);
        }
    }

    /// The outputs still at Caminito's default scale take the default for `factor` and what
    /// Caminito now knows of their monitors, and are laid out again.
    fn take_default_scales(&mut self, factor: WindowScalingFactor) {
        for output in &self.outputs {
            let record = self.record(output);
            if record.scale == ChosenBy::Caminito {
                let scale = default_scale(&record.monitor, factor);
                output.change_current_state(None, None, Some(Scale::Fractional(scale)), None);
            }
        }
        self.lay_out_outputs();
    }

    /// The backend changed an output's mode. At Caminito's default scale, the output takes
    /// the default for its new mode, and the outputs are laid out again for its new size.
    pub fn output_mode_changed(&mut self, output: &Output) {
        let factor = self.settings.interface.window_scaling_factor;
        let lid_switch = self.input_devices.has_lid_switch();
        let record = self.record_mut(output);
        record.monitor = Monitor::of(output, lid_switch);
        if record.scale == ChosenBy::Caminito {
            let scale = default_scale(&record.monitor, factor);
            output.change_current_state(None, None, Some(Scale::Fractional(scale)), None);
        }
        self.lay_out_outputs();
    }

    /// Each output's place, with the size it had when the outputs were last laid out.
    fn layout(&self) -> Vec<Rect> {
        let rect = |output: &Output| {
            let (place, size) = (output.current_location(), self.record(output).size);
            Rect {
                x: place.x,
                y: place.y,
                w: size.w,
                h: size.h,
            }
        };
        self.outputs.iter().map(rect).collect()
    }

    /// Makes the layout contiguous again (82, ADR 0014). The outputs right of and below
    /// one that changed size since the last time keep their side of it. Then the layout
    /// is healed around the primary output, and its top-left corner goes to the origin.
    fn heal_layout(&mut self) {
        let mut layout = self.layout();
        let sizes: Vec<_> = self
            .outputs
            .iter()
            .map(|o| output_geometry(o).size)
            .collect();
        for (index, size) in sizes.iter().enumerate() {
            if (layout[index].w, layout[index].h) != (size.w, size.h) {
                layout = caminito_layout::resized(&layout, index, size.w, size.h);
            }
        }
        // The primary output is the one held still (81), as the places stand before
        // healing: with no choice made and no built-in panel, that's the leftmost.
        let held = self
            .primary_output()
            .and_then(|primary| self.outputs.iter().position(|output| output == primary))
            .unwrap_or(0);
        let layout = caminito_layout::at_origin(&caminito_layout::heal(&layout, held));

        for ((output, place), size) in self.outputs.iter().zip(layout).zip(sizes) {
            let place = Point::from((place.x, place.y));
            if output.current_location() != place {
                output.change_current_state(None, None, None, Some(place));
            }
            let id = output_id(output);
            let record = self.output_records.get_mut(&id);
            record.expect("every output has a record").size = size;
        }
    }

    /// Heals the layout, arranges the outputs' layer surfaces for their new sizes, tells
    /// policy where every output is now, and redraws them all.
    fn lay_out_outputs(&mut self) {
        self.heal_layout();

        // First, since what the layer surfaces reserve of their outputs' new sizes is in
        // what policy is told (64).
        self.arrange_layers();
        // Before the outputs, so the windows of an output that's gone move to the primary
        // output there is now (81).
        self.tell_policy_the_primary_output();
        self.policy.set_outputs(self.policy_outputs());
        self.queue_redraw_all();
    }

    /// The monitors as output management shows them: the outputs, then the ones it
    /// disabled.
    fn heads(&self) -> Vec<Head> {
        let head = |output: &Output, enabled: bool| {
            let properties = output.physical_properties();
            Head {
                name: output.name(),
                description: output.description(),
                make: properties.make,
                model: properties.model,
                serial_number: properties.serial_number,
                physical_size: properties.size,
                modes: output.modes(),
                preferred_mode: output.preferred_mode(),
                enabled: enabled.then(|| HeadState {
                    mode: output.current_mode(),
                    position: output.current_location(),
                    scale: output.current_scale().fractional_scale(),
                    transform: output.current_transform(),
                }),
            }
        };
        let enabled = self.outputs.iter().map(|output| head(output, true));
        let disabled = self
            .disabled_outputs
            .iter()
            .map(|output| head(output, false));
        enabled.chain(disabled).collect()
    }

    /// Tells output management's clients of the outputs that changed.
    pub(crate) fn refresh_output_management(&mut self) {
        let heads = self.heads();
        self.output_management_state
            .refresh::<State>(&self.display_handle, heads);
    }

    /// What a configuration would change, if every head it names is a monitor that can
    /// show what's asked of it: one of its modes, in the transform it has, at a scale and
    /// a place of use, without adaptive sync. Its places needn't be contiguous, since
    /// `wlr-randr` and kanshi users write them by hand: Caminito heals them (82). Caminito
    /// doesn't rotate outputs yet, and has no adaptive sync. The backend still has to
    /// agree.
    pub(crate) fn plan_output_changes(
        &self,
        requests: &[HeadRequest],
    ) -> Option<Vec<OutputChange>> {
        requests
            .iter()
            .map(|request| {
                let enabled = self.outputs.iter().find(|o| o.name() == request.name);
                let disabled = || {
                    self.disabled_outputs
                        .iter()
                        .find(|o| o.name() == request.name)
                };
                let output = enabled.or_else(disabled)?.clone();
                let Some(asked) = request.enabled else {
                    return Some(OutputChange { output, to: None });
                };
                if asked.adaptive_sync == Some(true)
                    || asked.scale.is_some_and(|s| !SETTABLE_SCALES.contains(&s))
                    || asked.position.is_some_and(|p| {
                        !SETTABLE_COORDINATES.contains(&p.x) || !SETTABLE_COORDINATES.contains(&p.y)
                    })
                    || asked
                        .transform
                        .is_some_and(|t| t != output.current_transform())
                {
                    return None;
                }
                let mode = match asked.mode {
                    Some(mode) => matching_mode(&output.modes(), mode)?,
                    // A disabled output's current mode is the one it last showed.
                    None if enabled.is_some() => output.current_mode()?,
                    None => output.preferred_mode()?,
                };
                // `wlr-randr` sends every head's place back. The place an output has is no
                // new place, so an output that grows beside it still pushes it along.
                let has = enabled.map(|output| output.current_location());
                let settings = OutputSettings {
                    mode,
                    position: asked.position.filter(|&place| Some(place) != has),
                    scale: asked.scale,
                };
                Some(OutputChange {
                    output,
                    to: Some(settings),
                })
            })
            .collect()
    }

    /// Gives the outputs the scales and places `changes` set, once the backend shows their
    /// modes, and lays the outputs out, which heals the places (82).
    pub(crate) fn finish_output_changes(&mut self, changes: &[OutputChange]) {
        let settings = |change: &OutputChange| change.to.map(|to| (change.output.clone(), to));
        let set: Vec<(Output, OutputSettings)> = changes.iter().filter_map(settings).collect();
        for (output, settings) in &set {
            if let Some(scale) = settings.scale {
                output.change_current_state(None, None, Some(Scale::Fractional(scale)), None);
                self.record_mut(output).scale = ChosenBy::OutputManagement;
            }
        }
        // The outputs that changed size move their neighbours before the places are set,
        // so that only an output the configuration doesn't move is moved by them.
        self.heal_layout();
        for (output, settings) in &set {
            if let Some(position) = settings.position {
                output.change_current_state(None, None, None, Some(position));
            }
        }
        self.lay_out_outputs();
    }

    fn record(&self, output: &Output) -> &OutputRecord {
        &self.output_records[&output_id(output)]
    }

    fn record_mut(&mut self, output: &Output) -> &mut OutputRecord {
        self.output_records
            .get_mut(&output_id(output))
            .expect("every output has a record")
    }

    /// Policy's id for one of Caminito's outputs.
    pub fn output_id(&self, output: &Output) -> OutputId {
        output_id(output)
    }

    /// Brings each window's output enter and leave, scale and transform up to date with
    /// where policy put it.
    pub(crate) fn refresh_window_outputs(&mut self) {
        let ids: Vec<WindowId> = self.policy.windows().map(|mapped| mapped.id()).collect();
        for id in ids {
            let geometry = self.policy.geometry(id).unwrap();
            let mapped = self.policy.window(id).unwrap();
            let fallback = mapped
                .main_output()
                .is_none()
                .then(|| self.main_output_until_drawn(mapped.transient_parent()))
                .flatten();
            let mapped = self.policy.window_mut(id).unwrap();
            mapped.update_outputs(geometry, &self.outputs, fallback);
        }
    }

    pub fn queue_redraw(&mut self, output: &Output) {
        self.redraws.insert(self.output_id(output));
    }

    pub fn queue_redraw_all(&mut self) {
        for output in self.outputs.clone() {
            self.queue_redraw(&output);
        }
    }

    /// Queues every output if what the Scene reads from policy changed since the last
    /// refresh pass (179). Policy doesn't say which outputs show the change.
    pub(crate) fn queue_redraw_for_policy(&mut self) {
        if self.policy.take_scene_changed() {
            self.queue_redraw_all();
        }
    }

    /// The outputs queued for redraw since the last call.
    pub fn take_queued_redraws(&mut self) -> Vec<Output> {
        let queued = std::mem::take(&mut self.redraws);
        self.outputs
            .iter()
            .filter(|output| queued.contains(&self.output_id(output)))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests;
