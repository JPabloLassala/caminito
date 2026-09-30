//! The backends Camino runs on (15). `Tty` comes in M4.

use smithay::backend::renderer::element::RenderElementStates;
use smithay::output::Output;

use crate::state::Camino;

#[cfg(test)]
pub mod headless;
pub mod winit;

/// What Camino draws with and takes input from.
pub enum Backend {
    /// A window inside another desktop, as one output.
    Winit(Box<winit::Winit>),
    /// Virtual outputs rendered with pixman, for tests.
    #[cfg(test)]
    Headless(headless::Headless),
}

impl Backend {
    /// Renders `output` from the scene, and returns how each element was drawn. Returns
    /// `None`, drawing nothing, if the output can't take a new frame yet.
    pub fn render(&mut self, camino: &mut Camino, output: &Output) -> Option<RenderElementStates> {
        match self {
            Backend::Winit(winit) => winit.render(camino, output),
            #[cfg(test)]
            Backend::Headless(headless) => Some(headless.render(camino, output)),
        }
    }

    #[cfg(test)]
    pub fn headless_mut(&mut self) -> &mut headless::Headless {
        match self {
            Backend::Headless(headless) => headless,
            _ => panic!("not the headless backend"),
        }
    }
}
