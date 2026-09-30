//! Per-output render-element assembly, in the scene's layer order (15).
//!
//! There's no retained scene graph: each output's elements are rebuilt every frame, and this
//! is the only code that knows the layer order.

use smithay::backend::renderer::element::Id;
use smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement;
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::surface::{
    WaylandSurfaceRenderElement, render_elements_from_surface_tree,
};
use smithay::backend::renderer::element::{AsRenderElements, Kind, render_elements};
use smithay::backend::renderer::utils::CommitCounter;
use smithay::backend::renderer::{Color32F, ImportAll, ImportMem, Renderer};
use smithay::input::pointer::{CursorImageStatus, CursorImageSurfaceData};
use smithay::output::Output;
use smithay::reexports::wayland_server::Resource;
use smithay::utils::{Logical, Point, Rectangle, Scale};
use smithay::wayland::compositor::with_states;
use smithay::xwayland::X11Surface;

use crate::output::output_geometry;
use crate::state::Camino;
use crate::window::Mapped;

/// What shows where nothing is drawn.
pub const CLEAR_COLOR: Color32F = Color32F::new(0.2, 0.2, 0.2, 1.0);

/// Frame debug's outline colours (14).
const STRIP_OUTLINE: Color32F = Color32F::new(1.0, 0.0, 0.0, 1.0);
const RESIZE_BORDER_OUTLINE: Color32F = Color32F::new(0.0, 0.5, 1.0, 1.0);
const TITLE_OUTLINE: Color32F = Color32F::new(0.0, 1.0, 0.0, 1.0);

render_elements! {
    pub SceneElement<R> where R: ImportAll + ImportMem;
    /// A client's surface: a window's, the cursor's, or a drag icon's.
    Window = WaylandSurfaceRenderElement<R>,
    /// A frame's strip, one of Camino's cursors, or frame debug's label.
    Frame = MemoryRenderBufferRenderElement<R>,
    /// A side of one of frame debug's outlines.
    Outline = SolidColorRenderElement,
}

/// The scene's layers, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SceneLayer {
    /// The pointer's cursor, over everything.
    Cursor,
    /// A Wayland drag's icon, at the pointer (48).
    DragIcon,
    /// Lock surfaces, or the solid lock background (M7).
    Lock,
    /// The overlay layer-shell layer (M5).
    Overlay,
    /// X's override-redirect windows, its menus and tooltips, in X's stacking order and on
    /// no workspace (36).
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

pub(crate) const LAYERS: [SceneLayer; 9] = [
    SceneLayer::Cursor,
    SceneLayer::DragIcon,
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
        layer_elements(layer, renderer, camino, output, &mut elements);
    }
    elements
}

/// The layer of each of `output`'s render elements, front to back.
#[cfg(test)]
pub(crate) fn output_element_layers<R>(
    renderer: &mut R,
    camino: &Camino,
    output: &Output,
) -> Vec<SceneLayer>
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let mut elements = Vec::new();
    let mut layers = Vec::new();
    for layer in LAYERS {
        layer_elements(layer, renderer, camino, output, &mut elements);
        layers.resize(elements.len(), layer);
    }
    layers
}

fn layer_elements<R>(
    layer: SceneLayer,
    renderer: &mut R,
    camino: &Camino,
    output: &Output,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    match layer {
        SceneLayer::Cursor => cursor_elements(renderer, camino, output, elements),
        SceneLayer::DragIcon => drag_icon_elements(renderer, camino, output, elements),
        SceneLayer::OverrideRedirect => {
            override_redirect_elements(renderer, camino, output, elements)
        }
        SceneLayer::Windows => window_elements(renderer, camino, output, elements),
        SceneLayer::Lock
        | SceneLayer::Overlay
        | SceneLayer::Top
        | SceneLayer::Bottom
        | SceneLayer::Background => {}
    }
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
    let Some(pointer) = pointer_on(camino, output) else {
        return;
    };
    let output_geometry = output_geometry(output);
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

/// Where the pointer is, if it's on `output`. The cursor and a drag icon are drawn only on
/// that output, at its scale.
pub(crate) fn pointer_on(camino: &Camino, output: &Output) -> Option<Point<f64, Logical>> {
    let pointer = camino.seat.get_pointer().unwrap().current_location();
    output_geometry(output)
        .to_f64()
        .contains(pointer)
        .then_some(pointer)
}

/// The drag icon at the pointer plus its offset, if the pointer's on `output`.
fn drag_icon_elements<R>(
    renderer: &mut R,
    camino: &Camino,
    output: &Output,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let (Some(icon), Some(pointer)) = (&camino.drag_icon, pointer_on(camino, output)) else {
        return;
    };
    if !icon.surface.is_alive() {
        return;
    }
    let scale = output.current_scale().fractional_scale();
    let location = (pointer + icon.offset.to_f64() - output_geometry(output).loc.to_f64())
        .to_physical_precise_round(scale);
    elements.extend(
        render_elements_from_surface_tree(
            renderer,
            &icon.surface,
            location,
            scale,
            1.0,
            Kind::Unspecified,
        )
        .into_iter()
        .map(SceneElement::Window),
    );
}

/// The override-redirect windows on `output`. They render as other XWayland surfaces do,
/// with the X11 client scale their buffers have.
fn override_redirect_elements<R>(
    renderer: &mut R,
    camino: &Camino,
    output: &Output,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let output_geometry = output_geometry(output);
    let scale = output.current_scale().fractional_scale();
    for window in camino.override_redirect_windows() {
        let bbox = override_redirect_bbox(window);
        if !bbox.overlaps(output_geometry) {
            continue;
        }
        let location = (bbox.loc - output_geometry.loc).to_physical_precise_round(scale);
        elements.extend(AsRenderElements::<R>::render_elements::<SceneElement<R>>(
            window,
            renderer,
            location,
            Scale::from(scale),
            1.0,
        ));
    }
}

/// Where an override-redirect window's surface is, globally: where X last put it.
pub(crate) fn override_redirect_bbox(window: &X11Surface) -> Rectangle<i32, Logical> {
    Rectangle::new(window.last_configure().loc, window.bbox().size)
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
        if camino.debug.frame_debug {
            frame_debug_elements(renderer, camino, mapped, geometry.loc, output, elements);
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

/// Frame debug's outlines of a frame's strips, invisible resize border and title rect, and
/// the label of the scale its strips were rendered at, over the window. `origin` is where
/// the window geometry is, globally.
fn frame_debug_elements<R>(
    renderer: &mut R,
    camino: &Camino,
    mapped: &Mapped,
    origin: Point<i32, Logical>,
    output: &Output,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let frame = mapped.frame();
    let output_geometry = output_geometry(output);
    // Everything outlined lies inside the frame's bounds.
    let Some(bounds) = frame.bounds().filter(|bounds| {
        Rectangle::new(bounds.loc + origin, bounds.size).overlaps(output_geometry)
    }) else {
        return;
    };
    let scale = output.current_scale().fractional_scale();
    let local = |mut rect: Rectangle<i32, Logical>| {
        rect.loc += origin - output_geometry.loc;
        rect
    };

    if let Some(label) = camino.debug.scale_label(frame.scale()) {
        let location = local(bounds)
            .loc
            .to_physical_precise_round::<_, i32>(scale)
            .to_f64();
        let element = MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            location,
            &label,
            None,
            None,
            None,
            Kind::Unspecified,
        );
        match element {
            Ok(element) => elements.push(SceneElement::Frame(element)),
            Err(err) => tracing::warn!("can't draw a frame debug label: {err:?}"),
        }
    }
    let outlines = frame
        .title_rect()
        .map(|rect| (rect, TITLE_OUTLINE))
        .into_iter()
        .chain(frame.strips().map(|(rect, _)| (rect, STRIP_OUTLINE)))
        .chain([(bounds, RESIZE_BORDER_OUTLINE)]);
    for (rect, color) in outlines {
        let rect = local(rect).to_physical_precise_round(scale);
        if rect.is_empty() {
            continue;
        }
        // One physical pixel wide, inside the rect.
        let (w, h) = (rect.size.w, rect.size.h);
        let sides = [
            Rectangle::new(rect.loc, (w, 1).into()),
            Rectangle::new(rect.loc + Point::from((0, h - 1)), (w, 1).into()),
            Rectangle::new(rect.loc, (1, h).into()),
            Rectangle::new(rect.loc + Point::from((w - 1, 0)), (1, h).into()),
        ];
        elements.extend(sides.into_iter().map(|side| {
            SceneElement::Outline(SolidColorRenderElement::new(
                Id::new(),
                side,
                CommitCounter::default(),
                color,
                Kind::Unspecified,
            ))
        }));
    }
}
