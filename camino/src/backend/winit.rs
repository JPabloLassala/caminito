//! The winit backend: Camino as a window inside another desktop, for nested runs (14).

use std::ffi::OsStr;

use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::RenderElementStates;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::winit::{self, WinitEvent, WinitGraphicsBackend};
use smithay::output::{Mode, Output};
use smithay::utils::{Physical, Size, Transform};

use crate::backend::Backend;
use crate::output::{DEFAULT_REFRESH, virtual_output};
use crate::scene::{self, CLEAR_COLOR};
use crate::state::{Camino, State};

/// The winit window, its GLES renderer, and the output it shows.
pub struct Winit {
    backend: WinitGraphicsBackend<GlesRenderer>,
    output: Output,
    damage_tracker: OutputDamageTracker,
    /// A frame went to the host and it hasn't asked for the next one yet.
    frame_pending: bool,
}

impl Winit {
    /// Opens the window, adds it to `camino` as an output, and joins winit's events to the
    /// loop.
    pub fn new(camino: &mut Camino) -> Result<Self, winit::Error> {
        let (backend, events) = winit::init::<GlesRenderer>()?;
        // Camino draws its own cursor.
        backend.window().set_cursor_visible(false);

        // GLES draws upside down relative to winit's window.
        let output = virtual_output(
            "winit".to_owned(),
            "Winit",
            (0, 0).into(),
            backend.window_size(),
            Transform::Flipped180,
        );
        camino.add_output(output.clone());
        match winit_output_scale(std::env::var_os("CAMINO_WINIT_SCALE").as_deref()) {
            Ok(Some(scale)) => camino.set_output_scale(&output, scale),
            Ok(None) => {}
            Err(err) => tracing::warn!("{err}; using the default scale"),
        }

        camino
            .loop_handle
            .insert_source(events, |event, _, state| handle_event(state, event))
            .expect("winit's events join the loop");

        Ok(Self {
            backend,
            damage_tracker: OutputDamageTracker::from_output(&output),
            output,
            frame_pending: false,
        })
    }

    /// Draws a frame, unless the host hasn't asked for the next one yet: frames, and so
    /// clients' frame callbacks, are paced by the host's redraw requests.
    pub fn render(&mut self, camino: &mut Camino, output: &Output) -> Option<RenderElementStates> {
        if self.frame_pending {
            return None;
        }
        let age = self.backend.buffer_age().unwrap_or(0);
        let (damage, states) = {
            let (renderer, mut framebuffer) = match self.backend.bind() {
                Ok(bound) => bound,
                Err(err) => {
                    tracing::warn!("error binding the winit window: {err}");
                    return None;
                }
            };
            let elements = scene::output_elements(renderer, camino, output);
            match self.damage_tracker.render_output(
                renderer,
                &mut framebuffer,
                age,
                &elements,
                CLEAR_COLOR,
            ) {
                Ok(result) => (result.damage.cloned(), result.states),
                Err(err) => {
                    tracing::warn!("error rendering the winit output: {err:?}");
                    return None;
                }
            }
        };
        if let Some(damage) = damage
            && let Err(err) = self.backend.submit(Some(&damage))
        {
            tracing::warn!("error submitting the winit frame: {err}");
        }
        self.backend.window().request_redraw();
        self.frame_pending = true;
        Some(states)
    }

    fn resize(&mut self, camino: &mut Camino, size: Size<i32, Physical>) {
        let mode = Mode {
            size,
            refresh: DEFAULT_REFRESH,
        };
        self.output.delete_mode(self.output.current_mode().unwrap());
        self.output
            .change_current_state(Some(mode), None, None, None);
        self.output.set_preferred(mode);
        camino.output_mode_changed(&self.output);
    }
}

fn handle_event(state: &mut State, event: WinitEvent) {
    let winit = match &mut state.backend {
        Backend::Winit(winit) => winit,
        _ => unreachable!("winit's events come only on the winit backend"),
    };
    match event {
        WinitEvent::Resized { size, .. } => winit.resize(&mut state.camino, size),
        WinitEvent::Redraw => winit.frame_pending = false,
        WinitEvent::CloseRequested => state.camino.quit(),
        WinitEvent::Input(event) => state.process_input_event(event),
        WinitEvent::Focus(false) => state.release_pressed_keys(),
        WinitEvent::Focus(true) => {}
    }
}

/// The winit output's scale from `CAMINO_WINIT_SCALE`, for checking frames at other scales
/// in nested runs: a positive number snapped to fractional-scale-v1's 1/120 steps, set as
/// output management would set it. Unset, the output has its default scale (51): 1, since
/// winit's window has no EDID, or `window-scaling-factor`'s.
fn winit_output_scale(var: Option<&OsStr>) -> Result<Option<f64>, String> {
    let Some(var) = var else {
        return Ok(None);
    };
    let scale = var
        .to_str()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|scale| scale.is_finite())
        .map(|scale| (scale * 120.0).round() / 120.0)
        .filter(|&scale| scale > 0.0);
    scale.map(Some).ok_or_else(|| {
        format!(
            "CAMINO_WINIT_SCALE={} isn't a positive number",
            var.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scale(value: &str) -> Result<f64, String> {
        winit_output_scale(Some(value.as_ref())).map(Option::unwrap)
    }

    #[test]
    fn the_default_scale_is_kept_when_unset() {
        assert_eq!(winit_output_scale(None), Ok(None));
    }

    #[test]
    fn integer_and_fractional_scales_are_taken() {
        assert_eq!(scale("2"), Ok(2.0));
        assert_eq!(scale("1.5"), Ok(1.5));
    }

    #[test]
    fn scales_snap_to_120ths() {
        assert_eq!(scale("1.333"), Ok(160.0 / 120.0));
    }

    #[test]
    fn scales_that_arent_positive_numbers_are_refused() {
        for value in ["", "two", "0", "-1", "NaN", "inf", "0.001"] {
            assert!(scale(value).is_err(), "{value:?}");
        }
    }
}
