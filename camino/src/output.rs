//! Output lifecycle: making outputs, laying them out, and telling policy about them (15).

use camino_policy::{OutputId, PolicyWindow, WindowId};
use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::utils::{Logical, Physical, Rectangle, Size, Transform};

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

impl Camino {
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
