//! Layer surfaces: the panel, caja's desktop, the wallpaper and notification bubbles
//! (63). Each output's are kept in Smithay's `layer_map_for_output`, which arranges them by
//! their anchors, margins, sizes and exclusive zones (15). `scene` decides their order.

use caminito_policy::WindowId;
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::desktop::{LayerSurface, WindowSurfaceType, layer_map_for_output};
use smithay::output::Output;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{IsAlive, Logical, Rectangle};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::wlr_layer::{
    KeyboardInteractivity, Layer, LayerSurface as WlrLayerSurface, LayerSurfaceData,
};

use crate::output::output_geometry;
use crate::scene;
use crate::state::{Caminito, SEAT};
use crate::window::send_preferred_scale;

impl Caminito {
    /// The output for a layer surface that names none: the one under the pointer, or else
    /// the primary output.
    pub(crate) fn default_layer_output(&self) -> Option<Output> {
        self.outputs
            .iter()
            .find(|output| scene::pointer_on(self, output).is_some())
            .or(self.primary_output())
            .cloned()
    }

    /// Puts a new layer surface on `output`. Its initial configure goes out on its initial
    /// commit, but it's told the scale to draw at before that, so its first buffer is sharp.
    pub(crate) fn layer_surface_created(
        &mut self,
        surface: WlrLayerSurface,
        output: &Output,
        namespace: String,
    ) {
        let layer = LayerSurface::new(surface, namespace);
        with_states(layer.wl_surface(), |data| {
            send_preferred_scale(layer.wl_surface(), data, output);
        });
        layer_map_for_output(output)
            .map_layer(&layer)
            .expect("a new layer surface is in no layer map");
    }

    /// Forgets a layer surface, and gives what it reserved back to the others on its output.
    pub(crate) fn layer_surface_destroyed(&mut self, surface: &WlrLayerSurface) {
        let Some((output, layer)) = self.layer_for_surface(surface.wl_surface()) else {
            return;
        };
        layer_map_for_output(&output).unmap_layer(&layer);
        self.queue_redraw(&output);
    }

    /// The layer surface whose main surface is `surface`, and its output.
    pub(crate) fn layer_for_surface(&self, surface: &WlSurface) -> Option<(Output, LayerSurface)> {
        self.outputs.iter().find_map(|output| {
            let map = layer_map_for_output(output);
            let layer = map.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)?;
            Some((output.clone(), layer.clone()))
        })
    }

    /// Handles a commit to a layer surface's main surface, and returns whether `surface` is
    /// one: arranges its output's layer surfaces again by what it committed, and sends its
    /// initial configure in answer to its initial commit.
    pub(crate) fn layer_commit(&mut self, surface: &WlSurface) -> bool {
        let Some((output, layer)) = self.layer_for_surface(surface) else {
            return false;
        };
        // Before the initial configure, which carries the size the arrangement gives it.
        layer_map_for_output(&output).arrange();
        let initial_configure_sent = with_states(surface, |states| {
            states
                .data_map
                .get::<LayerSurfaceData>()
                .is_some_and(|data| data.lock().unwrap().initial_configure_sent)
        });
        if !initial_configure_sent {
            layer.layer_surface().send_configure();
        }
        self.queue_redraw(&output);
        true
    }

    /// Arranges every output's layer surfaces again, after the outputs' modes, scales or
    /// places changed. The ones whose size changes are configured.
    pub(crate) fn arrange_layers(&self) {
        for output in &self.outputs {
            layer_map_for_output(output).arrange();
        }
    }

    /// Closes the layer surfaces of an output that's going away.
    pub(crate) fn close_layers(&self, output: &Output) {
        let mut map = layer_map_for_output(output);
        let layers: Vec<LayerSurface> = map.layers().cloned().collect();
        for layer in layers {
            layer.layer_surface().send_close();
            map.unmap_layer(&layer);
        }
    }

    /// An output's layer surfaces in the order they were made, each with where it is,
    /// globally.
    pub(crate) fn layer_surfaces(
        &self,
        output: &Output,
    ) -> Vec<(LayerSurface, Rectangle<i32, Logical>)> {
        let origin = output_geometry(output).loc;
        let map = layer_map_for_output(output);
        map.layers()
            .filter_map(|layer| {
                let mut geometry = map.layer_geometry(layer)?;
                geometry.loc += origin;
                Some((layer.clone(), geometry))
            })
            .collect()
    }

    /// Tells every layer surface's surfaces the scale of its output to draw at, as windows
    /// are told their main output's (52).
    pub(crate) fn refresh_layers(&self) {
        for output in &self.outputs {
            for layer in layer_map_for_output(output).layers() {
                layer.with_surfaces(|surface, data| send_preferred_scale(surface, data, output));
            }
        }
    }

    /// The work area a popup of the layer surface `root` is kept in, relative to the layer
    /// surface's origin. It's the output less the bars, the surface's own among them, as
    /// GTK keeps a menu in the monitor's work area on X11: a panel's menu that's higher
    /// than the room between the panels scrolls rather than run under the other panel.
    pub(crate) fn layer_popup_bounds(&self, root: &WlSurface) -> Option<Rectangle<i32, Logical>> {
        let (output, layer) = self.layer_for_surface(root)?;
        let origin = layer_map_for_output(&output).layer_geometry(&layer)?.loc;
        let mut bounds = self.work_area(&output);
        // The layer surface's geometry is from its output's origin.
        bounds.loc -= output_geometry(&output).loc + origin;
        Some(bounds)
    }

    /// A button was pressed on a layer surface. One with `on_demand` keyboard
    /// interactivity takes the keyboard; any other leaves it where it is.
    pub(crate) fn layer_pressed(&mut self, layer: &LayerSurface) {
        if takes_keyboard_on_demand(layer) {
            self.keyboard_layer = Some((layer.clone(), self.policy.focus(SEAT)));
        }
    }

    /// A button was pressed on a window, which takes the keyboard back from an `on_demand`
    /// layer surface.
    pub(crate) fn window_pressed(&mut self) {
        self.keyboard_layer = None;
    }

    /// The layer surface that has the keyboard rather than policy's focused window, which
    /// keeps the focus meanwhile, and its frame drawn focused, as under Marco while the
    /// panel's menu is open. It's the topmost mapped `exclusive` one of the overlay and top
    /// layers, or else the `on_demand` one that was clicked, which is forgotten here once
    /// policy's focus changes or it can't have the keyboard any more.
    pub(crate) fn layer_with_keyboard(&mut self) -> Option<LayerSurface> {
        let exclusive = scene::layer_surfaces_above_windows(self)
            .into_iter()
            .find(|layer| {
                is_mapped(layer) && interactivity(layer) == KeyboardInteractivity::Exclusive
            });
        if exclusive.is_some() {
            return exclusive;
        }
        let focus = self.policy.focus(SEAT);
        let keeps = |(layer, focused): &(LayerSurface, Option<WindowId>)| {
            *focused == focus
                && layer.alive()
                && is_mapped(layer)
                && takes_keyboard_on_demand(layer)
                && self
                    .layer_for_surface(layer.wl_surface())
                    .is_some_and(|(output, _)| {
                        // One a fullscreen window covers gives the keyboard back to it.
                        layer.layer() != Layer::Top || !scene::top_layer_is_covered(self, &output)
                    })
        };
        if !self.keyboard_layer.as_ref().is_some_and(keeps) {
            self.keyboard_layer = None;
        }
        self.keyboard_layer.as_ref().map(|(layer, _)| layer.clone())
    }
}

fn interactivity(layer: &LayerSurface) -> KeyboardInteractivity {
    layer.cached_state().keyboard_interactivity
}

/// Whether a click gives a layer surface the keyboard. The protocol leaves `exclusive` on
/// the layers below the windows to the compositor's usual focus rules, so there it's as
/// `on_demand`.
fn takes_keyboard_on_demand(layer: &LayerSurface) -> bool {
    match interactivity(layer) {
        KeyboardInteractivity::OnDemand => true,
        KeyboardInteractivity::Exclusive => !scene::is_above_windows(layer.layer()),
        KeyboardInteractivity::None => false,
    }
}

/// Whether a layer surface has content. Without it, it's still waiting for its initial
/// configure, or it was unmapped.
fn is_mapped(layer: &LayerSurface) -> bool {
    with_renderer_surface_state(layer.wl_surface(), |state| state.buffer().is_some())
        .unwrap_or(false)
}
