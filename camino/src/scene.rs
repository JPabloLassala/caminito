//! Per-output render-element assembly, in the scene's layer order (15).
//!
//! There's no retained scene graph: each output's elements are rebuilt every frame, and this
//! is the only code that knows the layer order.

use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::{AsRenderElements, render_elements};
use smithay::backend::renderer::{Color32F, ImportAll, Renderer};
use smithay::output::Output;
use smithay::utils::Scale;

use crate::output::output_geometry;
use crate::state::Camino;

/// What shows where nothing is drawn.
pub const CLEAR_COLOR: Color32F = Color32F::new(0.2, 0.2, 0.2, 1.0);

render_elements! {
    pub SceneElement<R> where R: ImportAll;
    Window = WaylandSurfaceRenderElement<R>,
}

/// The scene's layers, top to bottom.
#[derive(Debug, Clone, Copy)]
enum SceneLayer {
    /// Lock surfaces, or the solid lock background (M7).
    Lock,
    /// The overlay layer-shell layer (M5).
    Overlay,
    /// Override-redirect windows (M3).
    OverrideRedirect,
    /// The top layer-shell layer (M5).
    Top,
    /// The current workspace's windows in policy's stacking order.
    Windows,
    /// The bottom layer-shell layer (M5).
    Bottom,
    /// The background layer-shell layer (M5).
    Background,
}

const LAYERS: [SceneLayer; 7] = [
    SceneLayer::Lock,
    SceneLayer::Overlay,
    SceneLayer::OverrideRedirect,
    SceneLayer::Top,
    SceneLayer::Windows,
    SceneLayer::Bottom,
    SceneLayer::Background,
];

/// `output`'s render elements, front to back.
pub fn output_elements<R>(
    renderer: &mut R,
    camino: &Camino,
    output: &Output,
) -> Vec<SceneElement<R>>
where
    R: Renderer + ImportAll,
    R::TextureId: Clone + 'static,
{
    let mut elements = Vec::new();
    for layer in LAYERS {
        match layer {
            SceneLayer::Windows => window_elements(renderer, camino, output, &mut elements),
            SceneLayer::Lock
            | SceneLayer::Overlay
            | SceneLayer::OverrideRedirect
            | SceneLayer::Top
            | SceneLayer::Bottom
            | SceneLayer::Background => {}
        }
    }
    elements
}

fn window_elements<R>(
    renderer: &mut R,
    camino: &Camino,
    output: &Output,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll,
    R::TextureId: Clone + 'static,
{
    let policy = &camino.policy;
    let output_geometry = output_geometry(output);
    let scale = output.current_scale().fractional_scale();

    for &id in policy.stacking(policy.active_workspace()).iter().rev() {
        let (Some(mapped), Some(geometry)) = (policy.window(id), policy.geometry(id)) else {
            continue;
        };
        if !policy.is_showing(id) {
            continue;
        }
        let window = mapped.window();
        // The surface's origin sits the window geometry's offset away from where policy
        // placed the geometry.
        let mut bbox = window.bbox_with_popups();
        bbox.loc += geometry.loc - window.geometry().loc;
        if !bbox.overlaps(output_geometry) {
            continue;
        }
        let location = (geometry.loc - window.geometry().loc - output_geometry.loc)
            .to_physical_precise_round(scale);
        elements.extend(window.render_elements(renderer, location, Scale::from(scale), 1.0));
    }
}
