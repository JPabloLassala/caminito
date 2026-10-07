//! A `zwlr_layer_shell_v1` client, as mate-panel, caja's desktop and msd's background are
//! through gtk-layer-shell (63): it makes layer surfaces, and records how the compositor
//! configures and closes them.

use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::xdg::shell::client::xdg_surface::XdgSurface;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::{self, ZwlrLayerShellV1};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    self, ZwlrLayerSurfaceV1,
};

pub use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
pub use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor as LayerAnchor, KeyboardInteractivity,
};

use crate::{Buffer, Client, Popup, PopupConfigure, PopupId, Positioner, State};

/// The index of one of a client's layer surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerId(pub(crate) usize);

/// A layer surface for [`Client::create_layer_surface`] to make.
#[derive(Debug, Clone)]
pub struct LayerSpec {
    pub layer: Layer,
    pub namespace: &'static str,
    /// The `output`th bound output, or none to leave the choice to the compositor.
    pub output: Option<usize>,
    pub anchor: LayerAnchor,
    /// The size it asks for; 0 on an axis leaves that to the compositor.
    pub size: (u32, u32),
    pub exclusive_zone: i32,
    pub keyboard: KeyboardInteractivity,
}

impl LayerSpec {
    /// A bar along the top edge, `height` high and as wide as its output, that reserves its
    /// height and never takes the keyboard, as mate-panel's is.
    pub fn top_bar(height: u32) -> Self {
        LayerSpec {
            layer: Layer::Top,
            namespace: "panel",
            output: None,
            anchor: LayerAnchor::Top | LayerAnchor::Left | LayerAnchor::Right,
            size: (0, height),
            exclusive_zone: height as i32,
            keyboard: KeyboardInteractivity::None,
        }
    }

    /// A surface on `layer` that covers its output and reserves nothing.
    pub fn covering(layer: Layer, namespace: &'static str) -> Self {
        LayerSpec {
            layer,
            namespace,
            output: None,
            anchor: LayerAnchor::all(),
            size: (0, 0),
            exclusive_zone: 0,
            keyboard: KeyboardInteractivity::None,
        }
    }

    /// A `size` surface in the middle of its output on `layer`, that reserves nothing.
    pub fn floating(layer: Layer, size: (u32, u32)) -> Self {
        LayerSpec {
            layer,
            namespace: "test",
            output: None,
            anchor: LayerAnchor::empty(),
            size,
            exclusive_zone: 0,
            keyboard: KeyboardInteractivity::None,
        }
    }
}

/// A layer surface.
pub struct LayerSurface {
    pub surface: WlSurface,
    pub layer_surface: ZwlrLayerSurfaceV1,
    /// The size of every configure received, oldest first.
    pub configures: Vec<(u32, u32)>,
    /// The serial of the last configure, which mapping acks.
    last_serial: Option<u32>,
    /// Whether the compositor closed it.
    pub closed: bool,
    /// The surface's `enter` and `leave` events and the scales it's asked to draw at, as a
    /// window's `output_events` are.
    pub output_events: Vec<String>,
    buffers: Vec<Buffer>,
}

impl Client {
    /// Creates a layer surface as `spec` says, and commits it without a buffer, which asks
    /// for its initial configure.
    pub fn create_layer_surface(&mut self, spec: &LayerSpec) -> LayerId {
        let id = LayerId(self.state.layer_surfaces.len());
        let shell: ZwlrLayerShellV1 = self.global(|state| &mut state.layer_shell, 4);
        let surface = self.compositor().create_surface(&self.qh, ());
        let output = spec.output.map(|i| &self.state.outputs[i].proxy);
        let layer_surface = shell.get_layer_surface(
            &surface,
            output,
            spec.layer,
            spec.namespace.to_owned(),
            &self.qh,
            id,
        );
        layer_surface.set_anchor(spec.anchor);
        layer_surface.set_size(spec.size.0, spec.size.1);
        layer_surface.set_exclusive_zone(spec.exclusive_zone);
        layer_surface.set_keyboard_interactivity(spec.keyboard);
        surface.commit();
        self.state.layer_surfaces.push(LayerSurface {
            surface,
            layer_surface,
            configures: Vec::new(),
            last_serial: None,
            closed: false,
            output_events: Vec::new(),
            buffers: Vec::new(),
        });
        id
    }

    pub fn layer_surface(&self, id: LayerId) -> &LayerSurface {
        &self.state.layer_surfaces[id.0]
    }

    /// Acks the layer surface's latest configure and commits a buffer of the configured
    /// size filled with `argb`, which maps it.
    pub fn map_layer_surface(&mut self, id: LayerId, argb: u32) {
        let layer = self.layer_surface(id);
        let size = *layer.configures.last().expect("configured");
        let serial = layer.last_serial.expect("configured");
        layer.layer_surface.ack_configure(serial);
        let size = (size.0 as i32, size.1 as i32);
        let (pool, buffer) = self.shm_buffer(size, argb);
        let layer = &mut self.state.layer_surfaces[id.0];
        layer.surface.attach(Some(&buffer), 0, 0);
        layer.surface.damage_buffer(0, 0, size.0, size.1);
        layer.surface.commit();
        layer.buffers.push(Buffer { buffer, pool });
    }

    /// Attaches no buffer and commits, which unmaps the layer surface.
    pub fn unmap_layer_surface(&self, id: LayerId) {
        let surface = &self.layer_surface(id).surface;
        surface.attach(None, 0, 0);
        surface.commit();
    }

    /// Creates a popup of a layer surface placed by `positioner`, with an explicit grab as
    /// if `grab_serial`'s button press opened it, if given, and commits it without a buffer,
    /// which asks for its initial configure.
    pub fn create_layer_popup(
        &mut self,
        parent: LayerId,
        positioner: Positioner,
        grab_serial: Option<u32>,
    ) -> PopupId {
        let id = PopupId(self.state.popups.len());
        let surface = self.compositor().create_surface(&self.qh, ());
        let xdg_surface: XdgSurface = self.wm_base().get_xdg_surface(&surface, &self.qh, id);
        let xdg_positioner = self.positioner(positioner);
        let popup = xdg_surface.get_popup(None, &xdg_positioner, &self.qh, id);
        xdg_positioner.destroy();
        self.layer_surface(parent).layer_surface.get_popup(&popup);
        if let Some(serial) = grab_serial {
            let seat = self.seat();
            popup.grab(&seat, serial);
        }
        surface.commit();
        self.state.popups.push(Popup {
            surface,
            xdg_surface,
            popup,
            configures: Vec::new(),
            pending: PopupConfigure::default(),
            done: false,
            buffers: Vec::new(),
        });
        id
    }
}

/// The configures' sizes as one line each.
pub fn format_layer_configures(configures: &[(u32, u32)]) -> String {
    configures
        .iter()
        .map(|(w, h)| format!("size: {w} × {h}"))
        .collect::<Vec<_>>()
        .join("\n")
}

impl Dispatch<ZwlrLayerSurfaceV1, LayerId> for State {
    fn event(
        state: &mut Self,
        _: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        id: &LayerId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let layer = &mut state.layer_surfaces[id.0];
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                layer.configures.push((width, height));
                layer.last_serial = Some(serial);
            }
            zwlr_layer_surface_v1::Event::Closed => layer.closed = true,
            _ => {}
        }
    }
}

impl Dispatch<ZwlrLayerShellV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwlrLayerShellV1,
        _: zwlr_layer_shell_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
