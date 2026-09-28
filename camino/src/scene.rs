//! Per-output render-element assembly, in the scene's layer order (15).
//!
//! There's no retained scene graph: each output's elements are rebuilt every frame, and this
//! is the only code that knows the layer order.

use smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement;
use smithay::backend::renderer::element::surface::{
    WaylandSurfaceRenderElement, render_elements_from_surface_tree,
};
use smithay::backend::renderer::element::{AsRenderElements, Kind, render_elements};
use smithay::backend::renderer::{Color32F, ImportAll, ImportMem, Renderer};
use smithay::input::pointer::{CursorImageStatus, CursorImageSurfaceData};
use smithay::output::Output;
use smithay::reexports::wayland_server::Resource;
use smithay::utils::{Logical, Point, Scale};
use smithay::wayland::compositor::with_states;

use crate::output::output_geometry;
use crate::state::Camino;

/// What shows where nothing is drawn.
pub const CLEAR_COLOR: Color32F = Color32F::new(0.2, 0.2, 0.2, 1.0);

render_elements! {
    pub SceneElement<R> where R: ImportAll + ImportMem;
    /// A client's surface: a window's, or the cursor's.
    Window = WaylandSurfaceRenderElement<R>,
    /// A frame's strip, or one of Camino's cursors.
    Frame = MemoryRenderBufferRenderElement<R>,
}

/// The scene's layers, top to bottom.
#[derive(Debug, Clone, Copy)]
enum SceneLayer {
    /// The pointer's cursor, over everything.
    Cursor,
    /// Lock surfaces, or the solid lock background (M7).
    Lock,
    /// The overlay layer-shell layer (M5).
    Overlay,
    /// Override-redirect windows (M3).
    OverrideRedirect,
    /// The top layer-shell layer (M5).
    Top,
    /// The current workspace's windows in policy's stacking order, each with its frame.
    Windows,
    /// The bottom layer-shell layer (M5).
    Bottom,
    /// The background layer-shell layer (M5).
    Background,
}

const LAYERS: [SceneLayer; 8] = [
    SceneLayer::Cursor,
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
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let mut elements = Vec::new();
    for layer in LAYERS {
        match layer {
            SceneLayer::Cursor => cursor_elements(renderer, camino, output, &mut elements),
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

/// The cursor at the pointer, if it's on `output`: Camino's own, or the surface the client
/// set.
fn cursor_elements<R>(
    renderer: &mut R,
    camino: &Camino,
    output: &Output,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let output_geometry = output_geometry(output);
    let pointer = camino.seat.get_pointer().unwrap().current_location();
    if !output_geometry.to_f64().contains(pointer) {
        return;
    }
    let scale = output.current_scale().fractional_scale();
    let at = |hotspot: Point<i32, Logical>| {
        (pointer - hotspot.to_f64() - output_geometry.loc.to_f64()).to_physical_precise_round(scale)
    };
    match camino.cursor() {
        CursorImageStatus::Hidden => {}
        CursorImageStatus::Surface(surface) if surface.is_alive() => {
            let hotspot = with_states(&surface, |states| {
                states
                    .data_map
                    .get::<CursorImageSurfaceData>()
                    .map(|data| data.lock().unwrap().hotspot)
                    .unwrap_or_default()
            });
            elements.extend(
                render_elements_from_surface_tree(
                    renderer,
                    &surface,
                    at(hotspot),
                    scale,
                    1.0,
                    Kind::Cursor,
                )
                .into_iter()
                .map(SceneElement::Window),
            );
        }
        status => {
            let icon = match status {
                CursorImageStatus::Named(icon) => icon,
                // A client's cursor surface that's gone shows the default cursor.
                _ => Default::default(),
            };
            let image = camino.cursors.image(icon, scale);
            let element = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                at(image.hotspot).to_f64(),
                &image.buffer,
                None,
                None,
                None,
                Kind::Cursor,
            );
            match element {
                Ok(element) => elements.push(SceneElement::Frame(element)),
                Err(err) => tracing::warn!("can't draw the cursor: {err:?}"),
            }
        }
    }
}

fn window_elements<R>(
    renderer: &mut R,
    camino: &Camino,
    output: &Output,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
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
        // placed the geometry. A shaded window's surfaces have left the scene.
        let mut bbox = window.bbox_with_popups();
        bbox.loc += geometry.loc - window.geometry().loc;
        if !mapped.is_shaded() && bbox.overlaps(output_geometry) {
            let location = (geometry.loc - window.geometry().loc - output_geometry.loc)
                .to_physical_precise_round(scale);
            elements.extend(window.render_elements(renderer, location, Scale::from(scale), 1.0));
        }

        // The strips lie around the window geometry, behind the window and its popups.
        for (mut rect, buffer) in mapped.frame().strips() {
            rect.loc += geometry.loc;
            if !rect.overlaps(output_geometry) {
                continue;
            }
            let location = (rect.loc - output_geometry.loc)
                .to_physical_precise_round::<_, i32>(scale)
                .to_f64();
            let element = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                location,
                buffer,
                None,
                None,
                Some(rect.size),
                Kind::Unspecified,
            );
            match element {
                Ok(element) => elements.push(SceneElement::Frame(element)),
                Err(err) => tracing::warn!("can't draw a frame strip: {err:?}"),
            }
        }
    }
}
