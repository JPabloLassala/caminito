//! The winit backend: Camino as a window inside another desktop, for nested runs (14).

use smithay::backend::renderer::damage::OutputDamageTracker;
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

        // GLES draws upside down relative to winit's window.
        let output = virtual_output(
            "winit".to_owned(),
            "Winit",
            backend.window_size(),
            1.0,
            Transform::Flipped180,
        );
        camino.add_output(output.clone());

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
    pub fn render(&mut self, camino: &mut Camino, output: &Output) -> bool {
        if self.frame_pending {
            return false;
        }
        let age = self.backend.buffer_age().unwrap_or(0);
        let damage = {
            let (renderer, mut framebuffer) = match self.backend.bind() {
                Ok(bound) => bound,
                Err(err) => {
                    tracing::warn!("error binding the winit window: {err}");
                    return false;
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
                Ok(result) => result.damage.cloned(),
                Err(err) => {
                    tracing::warn!("error rendering the winit output: {err:?}");
                    return false;
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
        true
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
        camino.output_changed(&self.output);
    }
}

// `Headless` is the other variant, under `cfg(test)`.
#[allow(clippy::infallible_destructuring_match)]
fn handle_event(state: &mut State, event: WinitEvent) {
    let winit = match &mut state.backend {
        Backend::Winit(winit) => winit,
        #[cfg(test)]
        _ => unreachable!("winit's events come only on the winit backend"),
    };
    match event {
        WinitEvent::Resized { size, .. } => winit.resize(&mut state.camino, size),
        WinitEvent::Redraw => winit.frame_pending = false,
        WinitEvent::CloseRequested => state.camino.loop_signal.stop(),
        WinitEvent::Input(event) => state.process_input_event(event),
        WinitEvent::Focus(false) => state.release_pressed_keys(),
        WinitEvent::Focus(true) => {}
    }
}
