//! Output lifecycle: making outputs, laying them out, picking their default scales, and
//! telling policy about them (15).

use camino_policy::{OutputId, PolicyWindow, WindowId};
use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::utils::{Logical, Physical, Raw, Rectangle, Size, Transform};

use crate::settings::translate::WindowScalingFactor;
use crate::state::{Camino, State};

/// The refresh rate backends without a real one report, in mHz.
pub const DEFAULT_REFRESH: i32 = 60_000;

/// An output with one mode of `size` at `scale`, made by Camino rather than a monitor.
pub fn virtual_output(
    name: String,
    model: &str,
    size: Size<i32, Physical>,
    scale: f64,
    transform: Transform,
) -> Output {
    let output = Output::new(
        name,
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Camino".to_owned(),
            model: model.to_owned(),
            serial_number: String::new(),
        },
    );
    let mode = Mode {
        size,
        refresh: DEFAULT_REFRESH,
    };
    output.change_current_state(
        Some(mode),
        Some(transform),
        Some(Scale::Fractional(scale)),
        None,
    );
    output.set_preferred(mode);
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

/// The scale Camino gives a monitor no output-management client has set (13, ADR 0008):
/// `window-scaling-factor`'s integer when it forces one, and otherwise KWin's rule.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the output lifecycle (51) calls it")
)]
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

impl Camino {
    /// The X11 scale for the outputs there are now.
    pub(crate) fn x11_scale(&self) -> u32 {
        x11_scale(
            self.outputs
                .iter()
                .map(|output| output.current_scale().fractional_scale()),
        )
    }

    /// Starts showing an output whose mode and scale the backend has set. It goes to the
    /// right of the outputs already there.
    pub fn add_output(&mut self, output: Output) {
        let id = OutputId(self.next_output_id);
        self.next_output_id += 1;
        output.user_data().insert_if_missing(|| id);

        let x = self
            .outputs
            .iter()
            .map(|o| {
                let geometry = output_geometry(o);
                geometry.loc.x + geometry.size.w
            })
            .max()
            .unwrap_or(0);
        output.change_current_state(None, None, None, Some((x, 0).into()));
        output.create_global::<State>(&self.display_handle);

        self.outputs.push(output.clone());
        self.output_changed(&output);
    }

    /// Tells policy an output's mode, scale or place changed, and redraws it.
    pub fn output_changed(&mut self, output: &Output) {
        self.policy.add_output(camino_policy::Output {
            id: self.output_id(output),
            geometry: output_geometry(output),
            scale: output.current_scale().fractional_scale(),
        });
        self.queue_redraw(output);
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
            let mapped = self.policy.window_mut(id).unwrap();
            mapped.update_outputs(geometry, &self.outputs);
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
