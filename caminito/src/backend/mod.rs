//! The backends Caminito runs on (15).

use std::ffi::OsStr;

use caminito_policy::WindowId;
use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::renderer::element::RenderElementStates;
use smithay::backend::renderer::gles::GlesTexture;
use smithay::input::keyboard::LedState;
use smithay::output::{Mode, Output};
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Buffer, Rectangle};
use smithay::wayland::dmabuf::DmabufState;
use smithay::wayland::drm_syncobj::DrmSyncobjState;

use crate::capture::{self, Shot};
use crate::open_close_animation::{Picture, take_picture, take_popup_picture};
use crate::output::OutputChange;
use crate::state::Caminito;

#[cfg(test)]
pub mod headless;
pub mod tty;
pub mod winit;

/// Which backend a bare `caminito` runs on.
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

/// What Caminito draws with and takes input from.
pub enum Backend {
    /// The machine's monitors, from a spare tty.
    Tty(Box<tty::Tty>),
    /// A window inside another desktop, as one output.
    Winit(Box<winit::Winit>),
    /// Virtual outputs rendered into memory, for tests.
    #[cfg(test)]
    Headless(Box<headless::Headless>),
}

impl Backend {
    /// Renders `output` from the scene, and returns how each element was drawn. Returns
    /// `None`, drawing nothing, if the output can't take a new frame yet.
    pub fn render(
        &mut self,
        caminito: &mut Caminito,
        output: &Output,
    ) -> Option<RenderElementStates> {
        match self {
            Backend::Tty(tty) => tty.render(caminito, output),
            Backend::Winit(winit) => winit.render(caminito, output),
            #[cfg(test)]
            Backend::Headless(headless) => {
                (!headless.is_powered_off()).then(|| headless.render(caminito, output))
            }
        }
    }

    /// Whether `output` can take a new frame now: the last one drawn is on screen.
    pub(crate) fn takes_frame(&self, output: &Output) -> bool {
        match self {
            Backend::Tty(tty) => tty.takes_frame(output),
            Backend::Winit(winit) => winit.takes_frame(),
            #[cfg(test)]
            Backend::Headless(headless) => !headless.is_powered_off(),
        }
    }

    /// Copies a capture's frame into a client's buffer with the backend's renderer, if what
    /// it shows changed since the session's last, and returns what changed (103).
    pub(crate) fn copy_capture(
        &mut self,
        caminito: &Caminito,
        shot: Shot<'_>,
        buffer: &WlBuffer,
    ) -> Result<Option<Vec<Rectangle<i32, Buffer>>>, capture::Error> {
        match self {
            Backend::Tty(tty) => {
                capture::copy::<_, GlesTexture>(tty.renderer(), caminito, shot, buffer)
            }
            Backend::Winit(winit) => {
                capture::copy::<_, GlesTexture>(winit.renderer(), caminito, shot, buffer)
            }
            #[cfg(test)]
            Backend::Headless(headless) => {
                headless::drawing!(headless.drawing(), renderer, _memory, T => {
                    capture::copy::<_, T>(renderer, caminito, shot, buffer)
                })
            }
        }
    }

    /// Draws window `id`'s picture for its closing with the backend's renderer, or returns
    /// `None` if it has no buffer left to draw from (252).
    pub(crate) fn take_picture(&mut self, caminito: &Caminito, id: WindowId) -> Option<Picture> {
        match self {
            Backend::Tty(tty) => take_picture::<_, GlesTexture>(tty.renderer(), caminito, id),
            Backend::Winit(winit) => take_picture::<_, GlesTexture>(winit.renderer(), caminito, id),
            #[cfg(test)]
            Backend::Headless(headless) => {
                headless::drawing!(headless.drawing(), renderer, _memory, T => {
                    take_picture::<_, T>(renderer, caminito, id)
                })
            }
        }
    }

    /// Draws the picture of the popup whose surface is `surface` for its slide back, at
    /// `scale`, with the backend's renderer, or returns `None` if it has no buffer left to
    /// draw from (ADR 0037).
    pub(crate) fn take_popup_picture(
        &mut self,
        surface: &WlSurface,
        scale: f64,
    ) -> Option<Picture> {
        match self {
            Backend::Tty(tty) => {
                take_popup_picture::<_, GlesTexture>(tty.renderer(), surface, scale)
            }
            Backend::Winit(winit) => {
                take_popup_picture::<_, GlesTexture>(winit.renderer(), surface, scale)
            }
            #[cfg(test)]
            Backend::Headless(headless) => {
                headless::drawing!(headless.drawing(), renderer, _memory, T => {
                    take_popup_picture::<_, T>(renderer, surface, scale)
                })
            }
        }
    }

    /// Switches to VT `vt`. Nested, there are no VTs to switch.
    pub fn change_vt(&mut self, vt: i32) {
        if let Backend::Tty(tty) = self {
            tty.change_vt(vt);
        }
    }

    /// Sets `output`'s gamma table to Night Light's `kelvin` (ADR 0023). Only the tty has
    /// gamma tables: nested and headless, Night Light tints nothing.
    pub(crate) fn set_temperature(&mut self, output: &Output, kelvin: u32) {
        if let Backend::Tty(tty) = self {
            tty.set_temperature(output, kelvin);
        }
    }

    /// The number of entries in each of `output`'s gamma ramps, or `None` if it has no
    /// gamma table: nested, it never has (141).
    pub(crate) fn gamma_size(&self, output: &Output) -> Option<u32> {
        match self {
            Backend::Tty(tty) => tty.gamma_size(output),
            Backend::Winit(_) => None,
            #[cfg(test)]
            Backend::Headless(headless) => headless.gamma_size(output),
        }
    }

    /// Puts a gamma client's `table` on `output` at once, or, with `None`, gives the table
    /// back to Night Light, which the refresh pass writes from neutral. Returns whether it
    /// was set (141).
    pub(crate) fn set_gamma(&mut self, output: &Output, table: Option<Vec<u16>>) -> bool {
        match self {
            Backend::Tty(tty) => tty.set_gamma(output, table),
            Backend::Winit(_) => false,
            #[cfg(test)]
            Backend::Headless(headless) => headless.set_gamma(output, table),
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
    pub fn set_output_mode(
        &mut self,
        caminito: &mut Caminito,
        output: &Output,
        mode: Mode,
    ) -> bool {
        match self {
            Backend::Tty(tty) => tty.set_output_mode(caminito, output, mode),
            Backend::Winit(_) => false,
            #[cfg(test)]
            Backend::Headless(headless) => headless.set_mode(caminito, output, mode),
        }
    }

    /// Lights again, in `mode`, a monitor output management disabled, and returns whether
    /// it did.
    pub fn enable_output(&mut self, caminito: &mut Caminito, output: &Output, mode: Mode) -> bool {
        match self {
            Backend::Tty(tty) => tty.enable_output(caminito, output, mode),
            Backend::Winit(_) => false,
            #[cfg(test)]
            Backend::Headless(headless) => headless.enable(caminito, output, mode),
        }
    }

    /// Turns a monitor off, as output management asks. Nested, the host's window stays.
    pub fn disable_output(&mut self, caminito: &mut Caminito, output: &Output) {
        match self {
            Backend::Tty(tty) => tty.disable_output(caminito, output),
            Backend::Winit(_) => {}
            #[cfg(test)]
            Backend::Headless(headless) => headless.disable(caminito, output),
        }
    }

    /// Powers every lit monitor off, or on again in the mode it had and with a new frame,
    /// with no output going or coming (129). Nested, powering off is the blank, which the
    /// scene draws, and nothing more.
    pub(crate) fn set_outputs_powered(&mut self, caminito: &mut Caminito, on: bool) {
        match self {
            Backend::Tty(tty) => tty.set_powered(caminito, on),
            Backend::Winit(_) => {}
            #[cfg(test)]
            Backend::Headless(headless) => headless.set_powered(on),
        }
    }

    /// The monitors the backend found and added as outputs, and hasn't lit yet: it waits
    /// for the saved layout, so that each is lit once, in the mode it's to show (87).
    /// Nested, the host's window is there from the start.
    pub(crate) fn unlit_outputs(&self) -> Vec<Output> {
        match self {
            Backend::Tty(tty) => tty.unlit_outputs(),
            Backend::Winit(_) => Vec::new(),
            #[cfg(test)]
            Backend::Headless(headless) => headless.unlit_outputs(),
        }
    }

    /// Lights a monitor the backend found, in the mode its output has, and returns whether
    /// it did.
    pub(crate) fn light_output(&mut self, output: &Output) -> bool {
        match self {
            Backend::Tty(tty) => tty.light_output(output),
            Backend::Winit(_) => false,
            #[cfg(test)]
            Backend::Headless(headless) => headless.light(output),
        }
    }

    /// Forgets a monitor the backend found and can't light, and removes its output.
    pub(crate) fn remove_unlit_output(&mut self, caminito: &mut Caminito, output: &Output) {
        match self {
            Backend::Tty(tty) => tty.remove_unlit_output(caminito, output),
            Backend::Winit(_) => {}
            #[cfg(test)]
            Backend::Headless(headless) => headless.unplug(caminito, output),
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
    fn caminito_runs_on_the_tty_when_no_display_is_set() {
        assert_eq!(Kind::choose(None, None), Kind::Tty);
    }

    #[test]
    fn caminito_runs_nested_inside_a_wayland_desktop() {
        assert_eq!(Kind::choose(Some("wayland-0".as_ref()), None), Kind::Winit);
    }

    #[test]
    fn caminito_runs_nested_inside_an_x11_desktop() {
        assert_eq!(Kind::choose(None, Some(":0".as_ref())), Kind::Winit);
    }

    #[test]
    fn caminito_runs_nested_when_both_displays_are_set() {
        assert_eq!(
            Kind::choose(Some("wayland-0".as_ref()), Some(":0".as_ref())),
            Kind::Winit
        );
    }
}
