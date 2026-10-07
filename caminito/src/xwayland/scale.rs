//! The X11 scale (41, ADR 0008): one integer scale for every XWayland window, applied
//! through Smithay's `set_client_scale` on the Xwayland client. What Caminito's X connection
//! writes is in X client space, logical × the scale, and what it reads from X is divided
//! by it. Smithay converts what goes through `X11Wm` itself.

use smithay::reexports::wayland_server::Client;
use smithay::utils::{FrameExtents, Logical, Rectangle};
use smithay::xwayland::XWaylandClientData;

use crate::state::Caminito;
use crate::window::{Strut, WindowKey};

use super::WindowProperty;

/// The X11 scale Xwayland's client was last given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct X11Scale(pub(super) u32);

impl X11Scale {
    /// Gives it to Xwayland's client, which Smithay converts its surfaces and input by.
    pub(super) fn apply(self, client: &Client) {
        let data = client
            .get_data::<XWaylandClientData>()
            .expect("Xwayland's client is inserted with XWaylandClientData");
        data.compositor_state.set_client_scale(f64::from(self.0));
    }

    fn factor(self) -> i32 {
        self.0 as i32
    }

    /// A logical rectangle in X client space.
    pub(super) fn to_x(self, rect: Rectangle<i32, Logical>) -> Rectangle<i32, Logical> {
        rect.upscale(self.factor())
    }

    /// Logical frame extents in X client space.
    pub(super) fn extents_to_x(
        self,
        extents: FrameExtents<i32, Logical>,
    ) -> FrameExtents<i32, Logical> {
        let s = self.factor();
        FrameExtents::new(
            extents.left * s,
            extents.right * s,
            extents.top * s,
            extents.bottom * s,
        )
    }

    /// A place or size in X client space as a logical one, rounded as Smithay rounds a
    /// `ConfigureRequest`'s (`xwm/mod.rs:1754-1776`).
    pub(super) fn to_logical(self, n: i32) -> i32 {
        (f64::from(n) / f64::from(self.0)).round() as i32
    }

    /// A rectangle in X client space as the logical one that covers it.
    pub(super) fn rect_to_logical(self, rect: Rectangle<i32, Logical>) -> Rectangle<i32, Logical> {
        let s = self.factor();
        let loc = (rect.loc.x.div_euclid(s), rect.loc.y.div_euclid(s));
        let far = (
            ceil_div(rect.loc.x + rect.size.w, s),
            ceil_div(rect.loc.y + rect.size.h, s),
        );
        Rectangle::new(loc.into(), (far.0 - loc.0, far.1 - loc.1).into())
    }

    /// A strut in X client space as a logical one: it reserves at least what it asked, and
    /// its span keeps the pixels it names, its end being the last one.
    pub(super) fn strut_to_logical(self, strut: Strut) -> Strut {
        let s = self.factor();
        Strut {
            side: strut.side,
            width: ceil_div(strut.width, s),
            span: strut
                .span
                .map(|(start, end)| (start.div_euclid(s), end.div_euclid(s))),
        }
    }
}

/// `n / d` rounded up, for a positive `d`.
fn ceil_div(n: i32, d: i32) -> i32 {
    -(-n).div_euclid(d)
}

impl Caminito {
    /// Applies the outputs' X11 scale when it changes, which is only when outputs or their
    /// scales do (13). XSETTINGS, the root cursor, the work area and frame extents are
    /// worked out at the scale later in the same refresh pass. Here, Xwayland is told the
    /// outputs' sizes in the new client space, every XWayland window is configured to the
    /// same logical rectangle in it, and what X said in client space is read again.
    pub(crate) fn refresh_x11_scale(&mut self) {
        let scale = X11Scale(self.x11_scale());
        let Some(xwayland) = self.xwayland.as_mut() else {
            return;
        };
        if xwayland.scale == scale {
            return;
        }
        tracing::debug!("the X11 scale is {}", scale.0);
        xwayland.scale = scale;
        scale.apply(&xwayland.client);
        // Smithay sends xdg-output's client-space sizes, which Xwayland's screen is made
        // from, only when an output's state changes (`output/xdg.rs:115-140`).
        for output in &self.outputs {
            output.change_current_state(None, None, None, None);
        }

        let surfaces: Vec<_> = self
            .known_windows()
            .filter_map(|window| window.x11_surface().cloned())
            .collect();
        for surface in surfaces {
            // Smithay converts the logical rectangle it last sent at the scale it has now.
            if let Err(err) = surface.configure(None) {
                tracing::debug!("can't configure an XWayland window: {err}");
            }
            self.read_x_property(&surface, WindowProperty::Struts);
            self.read_input_shape(&surface);
            // Smithay gives size hints in logical space, so they're read again too.
            if let Some(id) = self.mapped_for_key(&WindowKey::X11(surface.window_id())) {
                self.window_changed(id);
            }
        }
    }
}

#[cfg(test)]
mod tests;
