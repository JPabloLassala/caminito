//! The backends Camino runs on (15).

use std::ffi::OsStr;

use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::renderer::element::RenderElementStates;
use smithay::input::keyboard::LedState;
use smithay::output::{Mode, Output};
use smithay::wayland::dmabuf::DmabufState;
use smithay::wayland::drm_syncobj::DrmSyncobjState;

use crate::output::OutputChange;
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

    /// Shows the seat's lock LEDs on the keyboards. Nested, the host's keyboard shows its
    /// own.
    pub fn update_leds(&mut self, leds: LedState) {
        if let Backend::Tty(tty) = self {
            tty.update_leds(leds);
        }
    }

    /// linux-dmabuf's state. Its global is offered only on the tty (57), so only there do
    /// clients reach it.
    pub fn dmabuf_state(&mut self) -> &mut DmabufState {
        match self {
            Backend::Tty(tty) => tty.dmabuf_state(),
            _ => unreachable!("linux-dmabuf is offered only on the tty"),
        }
    }

    /// Imports a client's dmabuf for rendering, and returns whether it was taken.
    pub fn import_dmabuf(&mut self, dmabuf: &Dmabuf) -> bool {
        match self {
            Backend::Tty(tty) => tty.import_dmabuf(dmabuf),
            _ => false,
        }
    }

    /// drm-syncobj's state, if it's offered: only on the tty, where there's a DRM device to
    /// import timelines on (57).
    pub fn syncobj_state(&mut self) -> Option<&mut DrmSyncobjState> {
        match self {
            Backend::Tty(tty) => tty.syncobj_state(),
            _ => None,
        }
    }

    /// Whether the backend can show what `changes` ask, as far as it can tell without
    /// trying (58).
    pub(crate) fn can_change_outputs(&self, changes: &[OutputChange]) -> bool {
        match self {
            Backend::Tty(tty) => tty.can_change_outputs(),
            // The host owns the window: it can't be turned off, and its size is its only
            // mode.
            Backend::Winit(_) => changes.iter().all(|change| {
                change
                    .to
                    .is_some_and(|settings| Some(settings.mode) == change.output.current_mode())
            }),
            #[cfg(test)]
            Backend::Headless(_) => true,
        }
    }

    /// Switches an output to another of its modes, and returns whether it did.
    pub fn set_output_mode(&mut self, camino: &mut Camino, output: &Output, mode: Mode) -> bool {
        match self {
            Backend::Tty(tty) => tty.set_output_mode(camino, output, mode),
            Backend::Winit(_) => false,
            #[cfg(test)]
            Backend::Headless(headless) => headless.set_mode(camino, output, mode),
        }
    }

    /// Lights again, in `mode`, a monitor output management disabled, and returns whether
    /// it did.
    pub fn enable_output(&mut self, camino: &mut Camino, output: &Output, mode: Mode) -> bool {
        match self {
            Backend::Tty(tty) => tty.enable_output(camino, output, mode),
            Backend::Winit(_) => false,
            #[cfg(test)]
            Backend::Headless(headless) => headless.enable(camino, output, mode),
        }
    }

    /// Turns a monitor off, as output management asks. Nested, the host's window stays.
    pub fn disable_output(&mut self, camino: &mut Camino, output: &Output) {
        match self {
            Backend::Tty(tty) => tty.disable_output(camino, output),
            Backend::Winit(_) => {}
            #[cfg(test)]
            Backend::Headless(_) => camino.disable_output(output),
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
