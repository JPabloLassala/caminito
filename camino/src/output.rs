//! Output lifecycle: making outputs, laying them out, picking their default scales, and
//! telling policy about them (15, 51), and changing them as output management asks (58).

pub(crate) mod primary;

use std::time::Duration;

use camino_policy::{OutputId, PolicyWindow, WindowId};
use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::wayland_server::backend::GlobalId;
use smithay::utils::{Logical, Physical, Point, Raw, Rectangle, Size, Transform};

use crate::protocols::output_management::{Head, HeadRequest, HeadState};
use crate::settings::translate::WindowScalingFactor;
use crate::state::{Camino, State};

/// What an output's make, model or serial says when its monitor's EDID doesn't.
pub(crate) const UNKNOWN_EDID_TEXT: &str = "Unknown";

/// The refresh rate backends without a real one report, in mHz.
pub const DEFAULT_REFRESH: i32 = 60_000;

/// An output made by Camino rather than a monitor's connector, whose EDID says it's
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
            make: "Camino".to_owned(),
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

/// Policy's id for an output added through `Camino::add_output`.
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

/// What Camino knows of a monitor when it picks the monitor's default scale.
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

/// The scale Camino gives a monitor no output-management client has set (13, ADR 0008):
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
    /// The place and the scale, if the configuration sets them. Unset, they stay as they
    /// are, and stay Camino's to choose if they were.
    pub(crate) position: Option<Point<i32, Logical>>,
    pub(crate) scale: Option<f64>,
}

/// The scales a configuration may set. Outside them, an output's logical size is of no use,
/// and Camino's own defaults are from 1 to 3 (50).
const SETTABLE_SCALES: std::ops::RangeInclusive<f64> = 0.5..=8.0;

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

/// Who chose an output's scale or place (13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChosenBy {
    /// Camino's default, which it may change.
    Camino,
    /// A client, over output management, which Camino leaves alone.
    OutputManagement,
}

/// What Camino keeps of an output beside Smithay's state. It goes with the output, so a
/// monitor plugged in again starts at the defaults (06).
#[derive(Debug)]
pub(crate) struct OutputRecord {
    /// What the default scale is picked from.
    monitor: Monitor,
    scale: ChosenBy,
    position: ChosenBy,
    global: GlobalId,
}

/// How long a removed output's global stays disabled before it's destroyed. A client may
/// bind it before it hears the global is gone, and binding a destroyed global is an error.
const GLOBAL_REMOVAL_DELAY: Duration = Duration::from_secs(10);

impl Camino {
    /// The X11 scale for the outputs there are now.
    pub(crate) fn x11_scale(&self) -> u32 {
        x11_scale(
            self.outputs
                .iter()
                .map(|output| output.current_scale().fractional_scale()),
        )
    }

    /// Starts showing an output the backend found: in its preferred mode, at its default
    /// scale, and at the default place, to the right of the outputs there are.
    pub fn add_output(&mut self, output: Output) {
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
        let global = output.create_global::<State>(&self.display_handle);
        self.output_records.insert(
            id,
            OutputRecord {
                monitor,
                scale: ChosenBy::Camino,
                position: ChosenBy::Camino,
                global,
            },
        );
        self.outputs.push(output);
        self.lay_out_outputs();
    }

    /// Stops showing an output that output management disabled, as if it was unplugged
    /// (51), but keeps it as a head that can be enabled again (58).
    pub fn disable_output(&mut self, output: &Output) {
        self.remove_output(output);
        self.disabled_outputs.push(output.clone());
    }

    /// Stops showing an output that was unplugged. Policy moves its windows onto the
    /// others, and the outputs at the default place close the gap. A disabled output is
    /// only forgotten.
    pub fn remove_output(&mut self, output: &Output) {
        if let Some(index) = self.disabled_outputs.iter().position(|o| o == output) {
            self.disabled_outputs.remove(index);
            return;
        }
        let id = self.output_id(output);
        self.outputs.retain(|o| o != output);
        self.redraws.remove(&id);
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

        self.display_handle
            .disable_global::<State>(record.global.clone());
        self.loop_handle
            .insert_source(
                Timer::from_duration(GLOBAL_REMOVAL_DELAY),
                move |_, _, state| {
                    state
                        .camino
                        .display_handle
                        .remove_global::<State>(record.global.clone());
                    TimeoutAction::Drop
                },
            )
            .expect("the timer joins the loop");

        self.lay_out_outputs();
    }

    /// Sets an output's scale, as output management does. `window-scaling-factor` leaves
    /// it alone from now on.
    pub fn set_output_scale(&mut self, output: &Output, scale: f64) {
        output.change_current_state(None, None, Some(Scale::Fractional(scale)), None);
        self.record_mut(output).scale = ChosenBy::OutputManagement;
        self.lay_out_outputs();
    }

    /// Sets an output's place, as output management does. Laying out leaves it alone from
    /// now on.
    pub fn set_output_position(&mut self, output: &Output, position: Point<i32, Logical>) {
        output.change_current_state(None, None, None, Some(position));
        self.record_mut(output).position = ChosenBy::OutputManagement;
        self.lay_out_outputs();
    }

    /// Sets an output's place and scale, those of them a configuration gave, as
    /// `set_output_position` and `set_output_scale` do.
    fn set_output_place_and_scale(
        &mut self,
        output: &Output,
        position: Option<Point<i32, Logical>>,
        scale: Option<f64>,
    ) {
        let scale = scale.map(Scale::Fractional);
        output.change_current_state(None, None, scale, position);
        let record = self.record_mut(output);
        if scale.is_some() {
            record.scale = ChosenBy::OutputManagement;
        }
        if position.is_some() {
            record.position = ChosenBy::OutputManagement;
        }
    }

    /// `window-scaling-factor` changed: the outputs still at Camino's default scale take
    /// the new default (13).
    pub(crate) fn window_scaling_factor_changed(&mut self, factor: WindowScalingFactor) {
        self.take_default_scales(factor);
    }

    /// If a lid switch came or went since the outputs' scales were picked, the outputs
    /// still at Camino's default scale take the default for a laptop's panel, or for a panel
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

    /// The outputs still at Camino's default scale take the default for `factor` and what
    /// Camino now knows of their monitors, and are laid out again.
    fn take_default_scales(&mut self, factor: WindowScalingFactor) {
        for output in &self.outputs {
            let record = self.record(output);
            if record.scale == ChosenBy::Camino {
                let scale = default_scale(&record.monitor, factor);
                output.change_current_state(None, None, Some(Scale::Fractional(scale)), None);
            }
        }
        self.lay_out_outputs();
    }

    /// The backend changed an output's mode. At Camino's default scale, the output takes
    /// the default for its new mode, and the outputs are laid out again for its new size.
    pub fn output_mode_changed(&mut self, output: &Output) {
        let factor = self.settings.interface.window_scaling_factor;
        let lid_switch = self.input_devices.has_lid_switch();
        let record = self.record_mut(output);
        record.monitor = Monitor::of(output, lid_switch);
        if record.scale == ChosenBy::Camino {
            let scale = default_scale(&record.monitor, factor);
            output.change_current_state(None, None, Some(Scale::Fractional(scale)), None);
        }
        self.lay_out_outputs();
    }

    /// Lays the outputs at the default place out left to right, in the order they were
    /// added, after the ones output management placed, so they never overlap those.
    /// Arranges their layer surfaces for their new sizes, tells policy where every output
    /// is now, and redraws them all.
    fn lay_out_outputs(&mut self) {
        let is_set = |output: &Output| self.record(output).position == ChosenBy::OutputManagement;
        let mut x = self
            .outputs
            .iter()
            .filter(|output| is_set(output))
            .map(|output| output_geometry(output).loc.x + output_geometry(output).size.w)
            .max()
            .unwrap_or(0);
        for output in self.outputs.iter().filter(|output| !is_set(output)) {
            let position = Point::from((x, 0));
            if output.current_location() != position {
                output.change_current_state(None, None, None, Some(position));
            }
            x += output_geometry(output).size.w;
        }

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
    /// show what's asked of it: one of its modes, in the transform it has, at a scale of
    /// use, without adaptive sync. Camino doesn't rotate outputs yet, and has no adaptive sync. The backend still
    /// has to agree.
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
                let settings = OutputSettings {
                    mode,
                    position: asked.position,
                    scale: asked.scale,
                };
                Some(OutputChange {
                    output,
                    to: Some(settings),
                })
            })
            .collect()
    }

    /// Gives the outputs the places and scales `changes` set, once the backend shows their
    /// modes, and lays the outputs out.
    pub(crate) fn finish_output_changes(&mut self, changes: &[OutputChange]) {
        for change in changes {
            if let Some(settings) = &change.to {
                self.set_output_place_and_scale(&change.output, settings.position, settings.scale);
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

    /// Policy's id for one of Camino's outputs.
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
