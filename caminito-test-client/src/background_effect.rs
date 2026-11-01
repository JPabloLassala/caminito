//! ext-background-effect-v1 (CMN-238): a client that asks for Esmeril behind its surface.

use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_region::{self, WlRegion};
pub use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum};
use wayland_protocols::ext::background_effect::v1::client::ext_background_effect_manager_v1::{
    self, Capability, ExtBackgroundEffectManagerV1,
};
use wayland_protocols::ext::background_effect::v1::client::ext_background_effect_surface_v1;
pub use wayland_protocols::ext::background_effect::v1::client::ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1;

use crate::{Client, State};

/// A rectangle of a region: x, y, width and height, in the surface's coordinates.
pub type RegionRect = (i32, i32, i32, i32);

impl Client {
    /// Binds the manager, and so is told its capabilities. Roundtrip before reading them from
    /// [`State::blur_capabilities`].
    pub fn bind_background_effect(&mut self) -> ExtBackgroundEffectManagerV1 {
        self.global(|state| &mut state.background_effect_manager, 1)
    }

    /// The background effect object of `surface`.
    pub fn get_background_effect(&mut self, surface: &WlSurface) -> ExtBackgroundEffectSurfaceV1 {
        self.bind_background_effect()
            .get_background_effect(surface, &self.qh, ())
    }

    /// Sets the pending blur region of `effect` to `rects`, or to no region with `None`. The
    /// surface must commit for it to apply.
    pub fn set_blur_region(
        &mut self,
        effect: &ExtBackgroundEffectSurfaceV1,
        rects: Option<&[RegionRect]>,
    ) {
        let Some(rects) = rects else {
            effect.set_blur_region(None);
            return;
        };
        let compositor: WlCompositor = self.global(|state| &mut state.compositor, 6);
        let region = compositor.create_region(&self.qh, ());
        for &(x, y, width, height) in rects {
            region.add(x, y, width, height);
        }
        effect.set_blur_region(Some(&region));
        region.destroy();
    }
}

impl Dispatch<ExtBackgroundEffectManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtBackgroundEffectManagerV1,
        event: ext_background_effect_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_background_effect_manager_v1::Event::Capabilities {
            flags: WEnum::Value(flags),
        } = event
        {
            state
                .blur_capabilities
                .push(flags.contains(Capability::Blur));
        }
    }
}

impl Dispatch<ExtBackgroundEffectSurfaceV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ExtBackgroundEffectSurfaceV1,
        _: ext_background_effect_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlRegion, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlRegion,
        _: wl_region::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
