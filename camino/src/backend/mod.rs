//! The backends Camino runs on (15).

use std::ffi::OsStr;

use smithay::backend::renderer::element::RenderElementStates;
use smithay::output::Output;

use crate::state::Camino;

#[cfg(test)]
pub mod headless;
pub mod tty;
pub mod winit;

/// Which backend a bare `camino` runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Nested, inside the desktop that set `WAYLAND_DISPLAY` or `DISPLAY`.
    Winit,
    /// On the tty, driving the machine's monitors.
    Tty,
}

impl Kind {
    /// Nested whenever there's a desktop to nest in, and the tty otherwise, as niri
    /// chooses (55).
    pub fn choose(wayland_display: Option<&OsStr>, display: Option<&OsStr>) -> Self {
        if wayland_display.is_some() || display.is_some() {
            Kind::Winit
        } else {
            Kind::Tty
        }
    }
}

/// What Camino draws with and takes input from.
pub enum Backend {
    /// The machine's monitors, from a spare tty.
    Tty(Box<tty::Tty>),
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
            Backend::Tty(tty) => tty.render(camino, output),
            Backend::Winit(winit) => winit.render(camino, output),
            #[cfg(test)]
            Backend::Headless(headless) => Some(headless.render(camino, output)),
        }
    }

    /// Switches to VT `vt`. Nested, there are no VTs to switch.
    pub fn change_vt(&mut self, vt: i32) {
        if let Backend::Tty(tty) = self {
            tty.change_vt(vt);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camino_runs_on_the_tty_when_no_display_is_set() {
        assert_eq!(Kind::choose(None, None), Kind::Tty);
    }

    #[test]
    fn camino_runs_nested_inside_a_wayland_desktop() {
        assert_eq!(Kind::choose(Some("wayland-0".as_ref()), None), Kind::Winit);
    }

    #[test]
    fn camino_runs_nested_inside_an_x11_desktop() {
        assert_eq!(Kind::choose(None, Some(":0".as_ref())), Kind::Winit);
    }

    #[test]
    fn camino_runs_nested_when_both_displays_are_set() {
        assert_eq!(
            Kind::choose(Some("wayland-0".as_ref()), Some(":0".as_ref())),
            Kind::Winit
        );
    }
}
