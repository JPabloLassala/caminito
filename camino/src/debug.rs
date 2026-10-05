//! The debug toggles' state and the JSON state dump (14). [`crate::dbus::debug`] only
//! exposes them.

use std::cell::RefCell;
use std::collections::HashMap;

use camino_policy::PolicyWindow;
use camino_theme::cairo::{self, Context, Format, ImageSurface};
use serde::Serialize;
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::desktop::LayerSurface;
use smithay::utils::{FrameExtents, Logical, Rectangle};
use smithay::wayland::shell::wlr_layer::{ExclusiveZone, KeyboardInteractivity, Layer};

use crate::decoration::memory_buffer;
use crate::output::{output_geometry, output_id};
use crate::state::{Camino, SEAT};
use crate::window::{Strut, with_x_facts};

/// The debug toggles, and what they draw with.
#[derive(Default)]
pub struct DebugToggles {
    /// Outlines each frame's strips, invisible resize border and title rect, and labels the
    /// scale its strips were rendered at.
    pub frame_debug: bool,
    /// Frame debug's label for each scale, drawn once.
    scale_labels: RefCell<HashMap<i32, MemoryRenderBuffer>>,
}

impl DebugToggles {
    /// Frame debug's label for strips rendered at `scale`, as a buffer at that scale.
    pub fn scale_label(&self, scale: i32) -> Option<MemoryRenderBuffer> {
        let mut labels = self.scale_labels.borrow_mut();
        if let Some(label) = labels.get(&scale) {
            return Some(label.clone());
        }
        let label = draw_label(&format!("×{scale}"), scale)
            .inspect_err(|err| tracing::warn!("can't draw a frame debug label: {err}"))
            .ok()
            .and_then(|surface| memory_buffer(surface, scale))?;
        labels.insert(scale, label.clone());
        Some(label)
    }
}

/// `text` in white on black, 14 logical pixels high, at `scale`.
fn draw_label(text: &str, scale: i32) -> Result<ImageSurface, cairo::Error> {
    const HEIGHT: i32 = 14;
    let width = 8 * text.chars().count() as i32 + 4;
    let surface = ImageSurface::create(Format::ARgb32, width * scale, HEIGHT * scale)?;
    let cr = Context::new(&surface)?;
    cr.scale(f64::from(scale), f64::from(scale));
    cr.set_source_rgb(0.0, 0.0, 0.0);
    cr.paint()?;
    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.select_font_face(
        "monospace",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
    );
    cr.set_font_size(12.0);
    cr.move_to(2.0, 11.0);
    cr.show_text(text)?;
    drop(cr);
    Ok(surface)
}

#[derive(Serialize)]
struct StateDump {
    /// By id.
    windows: Vec<WindowDump>,
    workspaces: Vec<WorkspaceDump>,
    active_workspace: usize,
    outputs: Vec<OutputDump>,
    /// The primary output's name (81): the user's choice, or its stand-in.
    primary_output: Option<String>,
    /// The one scale of every XWayland window (41).
    x11_scale: u32,
    focus: Option<u64>,
}

#[derive(Serialize)]
struct WindowDump {
    id: u64,
    app_id: Option<String>,
    title: String,
    workspace: Option<usize>,
    geometry: Option<RectDump>,
    decorations: Decorations,
    /// The visible frame around the geometry, while it shows one.
    frame_extents: Option<ExtentsDump>,
    /// The scale the frame's strips were rendered at, while it shows one.
    strip_scale: Option<i32>,
    /// Whether it's an XWayland window.
    xwayland: bool,
    /// For an XWayland window, its X window's id and Marco's type for it.
    #[serde(skip_serializing_if = "Option::is_none")]
    x11: Option<X11Dump>,
    /// Whether it asks for the user's attention.
    urgent: bool,
    skip_taskbar: bool,
    skip_pager: bool,
}

#[derive(Serialize)]
struct X11Dump {
    window: u32,
    window_type: String,
    /// `_NET_FRAME_EXTENTS` as Camino's X connection last published it (39).
    frame_extents: Option<ExtentsDump>,
    /// The struts it reserves (39).
    struts: Vec<StrutDump>,
}

#[derive(Serialize)]
struct StrutDump {
    side: String,
    width: i32,
    /// The first and last pixel along the edge, or none for the whole edge.
    span: Option<(i32, i32)>,
}

impl From<Strut> for StrutDump {
    fn from(strut: Strut) -> Self {
        Self {
            side: format!("{:?}", strut.side).to_lowercase(),
            width: strut.width,
            span: strut.span,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum Decorations {
    Ssd,
    Csd,
}

#[derive(Serialize)]
struct WorkspaceDump {
    name: String,
    /// Window ids, bottom to top.
    stacking: Vec<u64>,
}

#[derive(Serialize)]
struct OutputDump {
    id: u64,
    name: String,
    geometry: RectDump,
    scale: f64,
    /// The output less what layer surfaces' exclusive zones and XWayland windows' struts
    /// reserve along its edges (64).
    work_area: RectDump,
    /// In the order they were made.
    layer_surfaces: Vec<LayerSurfaceDump>,
}

#[derive(Serialize)]
struct LayerSurfaceDump {
    /// `background`, `bottom`, `top` or `overlay`.
    layer: &'static str,
    namespace: String,
    geometry: RectDump,
    /// As the protocol has it: what it reserves on its edge, 0 to stay out of what others
    /// reserve, or -1 to ignore that.
    exclusive_zone: i32,
    /// `none`, `exclusive` or `on_demand`.
    keyboard_interactivity: &'static str,
}

impl LayerSurfaceDump {
    fn new(layer: &LayerSurface, geometry: Rectangle<i32, Logical>) -> Self {
        let state = layer.cached_state();
        Self {
            layer: match state.layer {
                Layer::Background => "background",
                Layer::Bottom => "bottom",
                Layer::Top => "top",
                Layer::Overlay => "overlay",
            },
            namespace: layer.namespace().to_owned(),
            geometry: geometry.into(),
            exclusive_zone: match state.exclusive_zone {
                ExclusiveZone::Exclusive(zone) => zone as i32,
                ExclusiveZone::Neutral => 0,
                ExclusiveZone::DontCare => -1,
            },
            keyboard_interactivity: match state.keyboard_interactivity {
                KeyboardInteractivity::None => "none",
                KeyboardInteractivity::Exclusive => "exclusive",
                KeyboardInteractivity::OnDemand => "on_demand",
            },
        }
    }
}

#[derive(Serialize)]
struct RectDump {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl From<Rectangle<i32, Logical>> for RectDump {
    fn from(rect: Rectangle<i32, Logical>) -> Self {
        Self {
            x: rect.loc.x,
            y: rect.loc.y,
            width: rect.size.w,
            height: rect.size.h,
        }
    }
}

#[derive(Serialize)]
struct ExtentsDump {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

impl From<FrameExtents<i32, Logical>> for ExtentsDump {
    fn from(extents: FrameExtents<i32, Logical>) -> Self {
        Self {
            left: extents.left,
            right: extents.right,
            top: extents.top,
            bottom: extents.bottom,
        }
    }
}

impl Camino {
    /// Windows, with the X window, Marco's type, the published frame extents and the
    /// struts for XWayland windows, workspaces with their names and stacking, outputs with
    /// their scales, work areas and layer surfaces, the X11 scale, and focus, as
    /// pretty-printed JSON. The lock state comes with the milestone that makes it.
    pub fn state_dump(&self) -> String {
        let policy = &self.policy;
        let windows = policy
            .windows()
            .map(|mapped| {
                let id = mapped.id();
                let ssd = mapped.is_ssd();
                let skips = mapped.skips();
                WindowDump {
                    id: id.0,
                    app_id: mapped.app_id(),
                    title: mapped.title(),
                    workspace: policy.workspace_of(id),
                    geometry: policy.geometry(id).map(RectDump::from),
                    decorations: if ssd {
                        Decorations::Ssd
                    } else {
                        Decorations::Csd
                    },
                    frame_extents: mapped
                        .shows_frame()
                        .then(|| policy.frame_extents(id).into()),
                    strip_scale: mapped.frame().drawn_scale(),
                    xwayland: mapped.window().is_x11(),
                    x11: mapped.window().x11_surface().map(|surface| {
                        let window = surface.window_id();
                        X11Dump {
                            window,
                            window_type: format!("{:?}", mapped.window_type()),
                            frame_extents: self.published_frame_extents(window).map(Into::into),
                            struts: with_x_facts(surface, |facts| facts.struts.clone())
                                .into_iter()
                                .map(Into::into)
                                .collect(),
                        }
                    }),
                    urgent: mapped.is_urgent(),
                    skip_taskbar: skips.taskbar,
                    skip_pager: skips.pager,
                }
            })
            .collect();
        let workspaces = policy
            .workspace_names()
            .iter()
            .enumerate()
            .map(|(workspace, name)| WorkspaceDump {
                name: name.clone(),
                stacking: policy.stacking(workspace).iter().map(|id| id.0).collect(),
            })
            .collect();
        let work_area = |output| {
            let id = output_id(output);
            let mut areas = policy.outputs().iter().zip(policy.work_areas());
            let (_, area) = areas
                .find(|(output, _)| output.id == id)
                .expect("policy has every output");
            area
        };
        let outputs = self
            .outputs
            .iter()
            .map(|output| OutputDump {
                id: output_id(output).0,
                name: output.name(),
                geometry: output_geometry(output).into(),
                scale: output.current_scale().fractional_scale(),
                work_area: work_area(output).into(),
                layer_surfaces: self
                    .layer_surfaces(output)
                    .iter()
                    .map(|(layer, geometry)| LayerSurfaceDump::new(layer, *geometry))
                    .collect(),
            })
            .collect();
        let dump = StateDump {
            windows,
            workspaces,
            active_workspace: policy.active_workspace(),
            outputs,
            primary_output: self.primary_output().map(|output| output.name()),
            x11_scale: self.x11_scale(),
            focus: policy.focus(SEAT).map(|id| id.0),
        };
        serde_json::to_string_pretty(&dump).expect("the dump serialises")
    }

    /// Turns frame debug on or off, and returns whether it's now on.
    pub fn toggle_frame_debug(&mut self) -> bool {
        self.debug.frame_debug = !self.debug.frame_debug;
        tracing::info!(
            "frame debug {}",
            if self.debug.frame_debug { "on" } else { "off" }
        );
        self.queue_redraw_all();
        self.debug.frame_debug
    }
}
