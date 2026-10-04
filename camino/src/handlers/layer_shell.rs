//! `zwlr_layer_shell_v1` (63). The handlers resolve the output a client names, and
//! otherwise only translate its events into `layer`'s.

use smithay::output::Output;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::wayland::shell::wlr_layer::{
    Layer, LayerSurface, WlrLayerShellHandler, WlrLayerShellState,
};
use smithay::wayland::shell::xdg::PopupSurface;

use crate::state::State;

impl WlrLayerShellHandler for State {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.camino.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: LayerSurface,
        output: Option<WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        let camino = &mut self.camino;
        let output = match output {
            Some(named) => {
                Output::from_resource(&named).filter(|output| camino.outputs.contains(output))
            }
            None => camino.default_layer_output(),
        };
        match output {
            Some(output) => camino.layer_surface_created(surface, &output, namespace),
            // The output it names is gone, or there's none to show it on.
            None => surface.send_close(),
        }
    }

    fn new_popup(&mut self, _parent: LayerSurface, popup: PopupSurface) {
        // xdg-shell's `new_popup` saw it without a parent, so it's constrained only now.
        self.camino.unconstrain_popup(&popup);
    }

    fn layer_destroyed(&mut self, surface: LayerSurface) {
        self.camino.layer_surface_destroyed(&surface);
    }
}
