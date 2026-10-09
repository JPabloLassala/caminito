//! Per-output render-element assembly, in the scene's layer order (15).
//!
//! There's no retained scene graph: each output's elements are rebuilt every frame, and this
//! is the only code that knows the layer order.

use caminito_policy::{Layer as StackLayer, WindowId};
use smithay::backend::renderer::element::Id;
use smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement;
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::surface::{
    WaylandSurfaceRenderElement, render_elements_from_surface_tree,
};
use smithay::backend::renderer::element::utils::{
    CropRenderElement, Relocate, RelocateRenderElement, RescaleRenderElement,
};
use smithay::backend::renderer::element::{AsRenderElements, Kind, render_elements};
use smithay::backend::renderer::utils::CommitCounter;
use smithay::backend::renderer::{Color32F, ImportAll, ImportMem, Renderer};
use smithay::desktop::{LayerSurface, PopupManager, WindowSurfaceType};
use smithay::input::pointer::{CursorImageStatus, CursorImageSurfaceData};
use smithay::output::Output;
use smithay::reexports::wayland_server::Resource;
use smithay::utils::{Logical, Physical, Point, Rectangle, Scale};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::wlr_layer::Layer;
use smithay::xwayland::X11Surface;

use crate::output::output_geometry;
use crate::state::Caminito;
use crate::switcher::layout::{CellStyle, Grid};
use crate::window::Mapped;

/// What shows where nothing is drawn.
pub const CLEAR_COLOR: Color32F = Color32F::new(0.2, 0.2, 0.2, 1.0);

/// What a locked output shows wherever the locker draws nothing (12).
pub(crate) const LOCK_BACKGROUND: Color32F = Color32F::new(0.0, 0.0, 0.0, 1.0);

/// Frame debug's outline colours (14).
const STRIP_OUTLINE: Color32F = Color32F::new(1.0, 0.0, 0.0, 1.0);
const RESIZE_BORDER_OUTLINE: Color32F = Color32F::new(0.0, 0.5, 1.0, 1.0);
const TITLE_OUTLINE: Color32F = Color32F::new(0.0, 1.0, 0.0, 1.0);

render_elements! {
    /// A part of one window, as a thumbnail draws it smaller (108).
    pub WindowPart<R> where R: ImportAll + ImportMem;
    /// One of the window's surfaces.
    Surface = WaylandSurfaceRenderElement<R>,
    /// One of its frame's strips.
    Strip = MemoryRenderBufferRenderElement<R>,
}

/// A part of a window in a thumbnail of the Window switcher: scaled down, moved into its
/// cell and cut off at the thumbnail's edge.
type ThumbnailPart<R> =
    CropRenderElement<RelocateRenderElement<RescaleRenderElement<WindowPart<R>>>>;

render_elements! {
    pub SceneElement<R> where R: ImportAll + ImportMem;
    /// A client's surface: a window's, a layer surface's, the cursor's, or a drag icon's.
    Window = WaylandSurfaceRenderElement<R>,
    /// A frame's strip, one of Caminito's cursors, or frame debug's label.
    Frame = MemoryRenderBufferRenderElement<R>,
    /// A side of one of frame debug's outlines, or the lock's background.
    Solid = SolidColorRenderElement,
    /// A part of a window in one of the Window switcher's thumbnails (108).
    Thumbnail = ThumbnailPart<R>,
}

/// The scene's layers, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SceneLayer {
    /// The pointer's cursor, over everything.
    Cursor,
    /// A Wayland drag's icon, at the pointer (48).
    DragIcon,
    /// While the session is locked, the output's lock surface over the solid lock
    /// background, and nothing of the layers below (12).
    Lock,
    /// The Window switcher's popup (108), over everything a client draws: the corner
    /// icons, then the thumbnails, then the rest of its chrome.
    Switcher,
    /// The overlay layer-shell layer (63).
    Overlay,
    /// X's override-redirect windows, its menus and tooltips, in X's stacking order and on
    /// no workspace (36).
    OverrideRedirect,
    /// The current workspace's windows in Marco's fullscreen layer: a fullscreen window
    /// that has the focus, and its transients. They cover the top layer, as Marco puts
    /// them above docks (63).
    FullscreenWindows,
    /// The top layer-shell layer (63).
    Top,
    /// The popups of the bottom and background layers' surfaces. They're above the windows,
    /// as sway and niri draw them, so no window cuts off the menu of caja's desktop, and
    /// below the top layer, so a panel covers them. A fullscreen window that has the focus
    /// covers them as it covers the panel (80).
    LayerPopups,
    /// The current workspace's other windows in policy's stacking order, each with its
    /// frame.
    Windows,
    /// The bottom layer-shell layer (63), without its surfaces' popups.
    Bottom,
    /// The background layer-shell layer (63), with msd's wallpaper behind caja's desktop,
    /// and without its surfaces' popups.
    Background,
}

/// The namespace of msd's wallpaper, which is behind every other surface of the background
/// layer, whichever mapped first: caja's `desktop` covers it where caja draws one (06).
const WALLPAPER_NAMESPACE: &str = "mate-background";

pub(crate) const LAYERS: [SceneLayer; 12] = [
    SceneLayer::Cursor,
    SceneLayer::DragIcon,
    SceneLayer::Lock,
    SceneLayer::Switcher,
    SceneLayer::Overlay,
    SceneLayer::OverrideRedirect,
    SceneLayer::FullscreenWindows,
    SceneLayer::Top,
    SceneLayer::LayerPopups,
    SceneLayer::Windows,
    SceneLayer::Bottom,
    SceneLayer::Background,
];

/// `output`'s render elements, front to back.
pub fn output_elements<R>(
    renderer: &mut R,
    caminito: &Caminito,
    output: &Output,
) -> Vec<SceneElement<R>>
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    output_capture_elements(renderer, caminito, output, true)
}

/// What a capture of `output` shows, front to back: what the output shows, with the
/// pointer's cursor or without it (103).
pub(crate) fn output_capture_elements<R>(
    renderer: &mut R,
    caminito: &Caminito,
    output: &Output,
    paint_cursors: bool,
) -> Vec<SceneElement<R>>
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let mut elements = Vec::new();
    for layer in shown_layers(caminito) {
        if layer == SceneLayer::Cursor && !paint_cursors {
            continue;
        }
        layer_elements(layer, renderer, caminito, output, &mut elements);
    }
    elements
}

/// The layers that are drawn, top to bottom: all of them, or while the session is locked
/// the lock and what's above it. The background covers the rest, so they're left out.
fn shown_layers(caminito: &Caminito) -> impl Iterator<Item = SceneLayer> {
    let locked = caminito.lock.is_locked();
    let lock = LAYERS
        .iter()
        .position(|layer| *layer == SceneLayer::Lock)
        .expect("the lock is one of the layers");
    let shown = if locked { lock + 1 } else { LAYERS.len() };
    LAYERS.into_iter().take(shown)
}

/// What a capture of a window shows, front to back, at `scale`: the window alone with its
/// frame, from the top left corner of policy's frame rect, whatever covers it and
/// whichever workspace it's on, and the pointer's cursor if it's asked for and is over a
/// window that's showing (103). A shaded window is its frame's titlebar, as it's drawn.
/// While the session is locked it's the lock background alone, of the same size, since
/// the window isn't shown to anyone (133).
pub(crate) fn window_capture_elements<R>(
    renderer: &mut R,
    caminito: &Caminito,
    id: WindowId,
    scale: f64,
    paint_cursors: bool,
) -> Vec<SceneElement<R>>
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let policy = &caminito.policy;
    let mut elements = Vec::new();
    let (Some(mapped), Some(geometry), Some(frame)) = (
        policy.window(id),
        policy.geometry(id),
        policy.frame_rect(id),
    ) else {
        return elements;
    };
    let shown = Rectangle::from_size(frame.size);
    if caminito.lock.is_locked() {
        let covered = shown.to_physical_precise_up(scale);
        elements.push(lock_background(caminito, covered));
        return elements;
    }
    let pointer = caminito.seat.get_pointer().unwrap().current_location() - frame.loc.to_f64();
    if paint_cursors && policy.is_showing(id) && shown.to_f64().contains(pointer) {
        cursor_elements_at(renderer, caminito, pointer, scale, &mut elements);
    }
    let origin = geometry.loc - frame.loc;
    one_window_elements(renderer, mapped, origin, shown, scale, &mut elements);
    elements
}

/// The layer of each of `output`'s render elements, front to back.
#[cfg(test)]
pub(crate) fn output_element_layers<R>(
    renderer: &mut R,
    caminito: &Caminito,
    output: &Output,
) -> Vec<SceneLayer>
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let mut elements = Vec::new();
    let mut layers = Vec::new();
    for layer in shown_layers(caminito) {
        layer_elements(layer, renderer, caminito, output, &mut elements);
        layers.resize(elements.len(), layer);
    }
    layers
}

fn layer_elements<R>(
    layer: SceneLayer,
    renderer: &mut R,
    caminito: &Caminito,
    output: &Output,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    match layer {
        SceneLayer::Cursor => cursor_elements(renderer, caminito, output, elements),
        SceneLayer::DragIcon => drag_icon_elements(renderer, caminito, output, elements),
        SceneLayer::OverrideRedirect => {
            override_redirect_elements(renderer, caminito, output, elements)
        }
        SceneLayer::FullscreenWindows | SceneLayer::Windows => {
            window_elements(layer, renderer, caminito, output, elements)
        }
        SceneLayer::Overlay
        | SceneLayer::Top
        | SceneLayer::LayerPopups
        | SceneLayer::Bottom
        | SceneLayer::Background => {
            layer_shell_elements(layer, renderer, caminito, output, elements)
        }
        SceneLayer::Lock => lock_elements(renderer, caminito, output, elements),
        SceneLayer::Switcher => switcher_elements(renderer, caminito, output, elements),
    }
}

/// While the session is locked, `output`'s lock surface, or the message Caminito shows once
/// it gave up on the locker, over the lock background, which covers the output from the
/// frame after the lock request, whatever the locker does (12).
fn lock_elements<R>(
    renderer: &mut R,
    caminito: &Caminito,
    output: &Output,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let lock = &caminito.lock;
    if !lock.is_locked() {
        return;
    }
    let scale = output.current_scale().fractional_scale();
    if let Some(surface) = lock.surface_on(output) {
        elements.extend(
            render_elements_from_surface_tree(
                renderer,
                surface.wl_surface(),
                Point::default(),
                scale,
                1.0,
                Kind::Unspecified,
            )
            .into_iter()
            .map(SceneElement::Window),
        );
    }
    // Once Caminito gave up on the locker (126), what to do about it.
    if let Some((buffer, size)) = caminito.locker.message_on(output) {
        let area = output_geometry(output).size;
        let location = Point::<i32, Logical>::from(((area.w - size.w) / 2, (area.h - size.h) / 2));
        let element = MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            location.to_physical_precise_round::<_, i32>(scale).to_f64(),
            buffer,
            None,
            None,
            Some(size),
            Kind::Unspecified,
        );
        match element {
            Ok(element) => elements.push(SceneElement::Frame(element)),
            Err(err) => tracing::debug!("can't draw the locker's give-up message: {err:?}"),
        }
    }
    let covered = Rectangle::from_size(output_geometry(output).size).to_physical_precise_up(scale);
    elements.push(lock_background(caminito, covered));
}

/// The lock background over `covered`.
fn lock_background<R>(caminito: &Caminito, covered: Rectangle<i32, Physical>) -> SceneElement<R>
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    SceneElement::Solid(SolidColorRenderElement::new(
        caminito.lock.background().clone(),
        covered,
        CommitCounter::default(),
        LOCK_BACKGROUND,
        Kind::Unspecified,
    ))
}

/// The Window switcher's popup, if it's on `output`, front to back: the thumbnails'
/// corner icons, the thumbnails, then the rest of the chrome.
fn switcher_elements<R>(
    renderer: &mut R,
    caminito: &Caminito,
    output: &Output,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let Some(popup) = caminito
        .visible_switcher()
        .filter(|popup| popup.output == *output)
    else {
        return;
    };
    let Some(chrome) = &popup.chrome else {
        return;
    };
    let grid = &popup.grid;
    let scale = output.current_scale().fractional_scale();
    let chrome_element = |renderer: &mut R, buffer, elements: &mut Vec<SceneElement<R>>| {
        let location = grid.popup.loc.to_physical_precise_round::<_, i32>(scale);
        let element = MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            location.to_f64(),
            buffer,
            None,
            None,
            Some(grid.popup.size),
            Kind::Unspecified,
        );
        match element {
            Ok(element) => elements.push(SceneElement::Frame(element)),
            // Every frame while the popup shows, so not above `debug!`.
            Err(err) => tracing::debug!("can't draw the window switcher: {err:?}"),
        }
    };
    if let Some(over) = &chrome.over {
        chrome_element(renderer, over, elements);
    }
    if grid.style == CellStyle::Thumbnails {
        thumbnail_elements(renderer, caminito, grid, scale, elements);
    }
    chrome_element(renderer, &chrome.under, elements);
}

/// Each listed window in its cell of `grid`, as a capture of the window shows it (103): its
/// surfaces over its frame, whichever workspace it's on and whether or not it's minimised,
/// since its surfaces keep their buffers. It's drawn from the window's own surfaces at
/// every frame, so it's live.
fn thumbnail_elements<R>(
    renderer: &mut R,
    caminito: &Caminito,
    grid: &Grid,
    scale: f64,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let policy = &caminito.policy;
    let Some(switcher) = policy.switcher() else {
        return;
    };
    // The grid is laid out again in the refresh pass, so it has a cell for each window.
    for (index, &id) in switcher.windows.iter().enumerate().take(grid.cells.len()) {
        let (Some(mapped), Some(geometry), Some(frame)) = (
            policy.window(id),
            policy.geometry(id),
            policy.frame_rect(id),
        ) else {
            continue;
        };
        if frame.size.w <= 0 || frame.size.h <= 0 {
            continue;
        }
        let mut parts: Vec<WindowPart<R>> = Vec::new();
        let shown = Rectangle::from_size(frame.size);
        one_window_elements(
            renderer,
            mapped,
            geometry.loc - frame.loc,
            shown,
            scale,
            &mut parts,
        );
        let target = grid.thumbnail(index, frame.size);
        let factor = Scale::from((
            f64::from(target.size.w) / f64::from(frame.size.w),
            f64::from(target.size.h) / f64::from(frame.size.h),
        ));
        let location: Point<i32, Physical> = target.loc.to_physical_precise_round(scale);
        // A window's popups and subsurfaces can reach past its frame.
        let crop: Rectangle<i32, Physical> = target.to_physical_precise_round(scale);
        for part in parts {
            let part = RescaleRenderElement::from_element(part, Point::default(), factor);
            let part = RelocateRenderElement::from_element(part, location, Relocate::Relative);
            if let Some(part) = CropRenderElement::from_element(part, scale, crop) {
                elements.push(SceneElement::Thumbnail(part));
            }
        }
    }
}

/// The layer-shell layer a scene layer shows the surfaces of.
fn shell_layer(layer: SceneLayer) -> Option<Layer> {
    match layer {
        SceneLayer::Overlay => Some(Layer::Overlay),
        SceneLayer::Top => Some(Layer::Top),
        SceneLayer::Bottom => Some(Layer::Bottom),
        SceneLayer::Background => Some(Layer::Background),
        _ => None,
    }
}

/// The layer-shell layers whose surfaces have a part in a scene layer, front to back.
fn shell_layers(layer: SceneLayer) -> Vec<Layer> {
    match layer {
        SceneLayer::LayerPopups => vec![Layer::Bottom, Layer::Background],
        _ => shell_layer(layer).into_iter().collect(),
    }
}

/// The part of a layer surface that a scene layer holds: a layer above the windows has its
/// surfaces whole, and a layer below them has them without their popups, which are in a
/// layer of their own above the windows (80).
pub(crate) fn layer_surface_part(layer: SceneLayer) -> WindowSurfaceType {
    match layer {
        SceneLayer::LayerPopups => WindowSurfaceType::POPUP | WindowSurfaceType::SUBSURFACE,
        SceneLayer::Bottom | SceneLayer::Background => {
            WindowSurfaceType::TOPLEVEL | WindowSurfaceType::SUBSURFACE
        }
        _ => WindowSurfaceType::ALL,
    }
}

/// `output`'s layer surfaces that have a part in a scene layer, front to back, each with
/// where it is, globally.
pub(crate) fn layer_surfaces_in(
    caminito: &Caminito,
    output: &Output,
    layer: SceneLayer,
) -> Vec<(LayerSurface, Rectangle<i32, Logical>)> {
    shell_layers(layer)
        .into_iter()
        .flat_map(|shell_layer| layer_surfaces_on(caminito, output, shell_layer))
        .collect()
}

/// `output`'s layer surfaces on `layer`, front to back, each with where it is, globally:
/// the newest in front, and the wallpaper behind the rest.
fn layer_surfaces_on(
    caminito: &Caminito,
    output: &Output,
    layer: Layer,
) -> Vec<(LayerSurface, Rectangle<i32, Logical>)> {
    let mut surfaces = caminito.layer_surfaces(output);
    surfaces.retain(|(surface, _)| surface.layer() == layer);
    surfaces.reverse();
    if layer == Layer::Background {
        surfaces.sort_by_key(|(surface, _)| surface.namespace() == WALLPAPER_NAMESPACE);
    }
    surfaces
}

/// Every output's layer surfaces of the layers above the windows, overlay then top, front
/// to back, without the top layer of an output a fullscreen window covers. An `exclusive`
/// one of them takes the keyboard (63).
pub(crate) fn layer_surfaces_above_windows(caminito: &Caminito) -> Vec<LayerSurface> {
    LAYERS
        .into_iter()
        .filter_map(shell_layer)
        .filter(|layer| is_above_windows(*layer))
        .flat_map(|layer| {
            caminito
                .outputs
                .iter()
                .filter(move |output| {
                    layer != Layer::Top || !top_layer_is_covered(caminito, output)
                })
                .flat_map(move |output| layer_surfaces_on(caminito, output, layer))
        })
        .map(|(surface, _)| surface)
        .collect()
}

/// Whether a layer-shell layer is drawn above the windows.
pub(crate) fn is_above_windows(layer: Layer) -> bool {
    matches!(layer, Layer::Overlay | Layer::Top)
}

/// Whether a fullscreen window in the fullscreen layer hides `output`'s top layer.
pub(crate) fn top_layer_is_covered(caminito: &Caminito, output: &Output) -> bool {
    let policy = &caminito.policy;
    let output_geometry = output_geometry(output);
    policy
        .stacking(policy.active_workspace())
        .iter()
        .any(|&id| {
            policy.is_showing(id)
                && policy.is_fullscreen(id)
                && window_is_in(caminito, id, SceneLayer::FullscreenWindows)
                && policy
                    .geometry(id)
                    .is_some_and(|geometry| geometry.contains_rect(output_geometry))
        })
}

/// Whether a window is drawn in `layer`, of the two that hold windows.
pub(crate) fn window_is_in(caminito: &Caminito, id: WindowId, layer: SceneLayer) -> bool {
    let fullscreen = caminito.policy.layer(id) == Some(StackLayer::Fullscreen);
    fullscreen == (layer == SceneLayer::FullscreenWindows)
}

/// `output`'s layer surfaces in one of the scene's layers that hold them, each under its
/// popups where the layer has both.
fn layer_shell_elements<R>(
    layer: SceneLayer,
    renderer: &mut R,
    caminito: &Caminito,
    output: &Output,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let output_geometry = output_geometry(output);
    let scale = output.current_scale().fractional_scale();
    let part = layer_surface_part(layer);
    for (surface, geometry) in layer_surfaces_in(caminito, output, layer) {
        let origin = geometry.loc - output_geometry.loc;
        let wl_surface = surface.wl_surface();
        let popups = PopupManager::popups_for_surface(wl_surface)
            .map(|(popup, offset)| (popup.wl_surface().clone(), offset - popup.geometry().loc))
            .filter(|_| part.contains(WindowSurfaceType::POPUP));
        let main = part
            .contains(WindowSurfaceType::TOPLEVEL)
            .then(|| (wl_surface.clone(), Point::default()));
        for (tree, offset) in popups.chain(main) {
            elements.extend(
                render_elements_from_surface_tree(
                    renderer,
                    &tree,
                    (origin + offset).to_physical_precise_round(scale),
                    scale,
                    1.0,
                    Kind::Unspecified,
                )
                .into_iter()
                .map(SceneElement::Window),
            );
        }
    }
}

/// The cursor at the pointer, if it's on `output`: Caminito's own, or the surface the client
/// set.
fn cursor_elements<R>(
    renderer: &mut R,
    caminito: &Caminito,
    output: &Output,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let Some(pointer) = pointer_on(caminito, output) else {
        return;
    };
    let pointer = pointer - output_geometry(output).loc.to_f64();
    let scale = output.current_scale().fractional_scale();
    cursor_elements_at(renderer, caminito, pointer, scale, elements);
}

/// The cursor with its hotspot at `pointer`, which is from the origin of what's drawn, at
/// `scale`.
fn cursor_elements_at<R>(
    renderer: &mut R,
    caminito: &Caminito,
    pointer: Point<f64, Logical>,
    scale: f64,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let at = |hotspot: Point<i32, Logical>| {
        (pointer - hotspot.to_f64()).to_physical_precise_round(scale)
    };
    match caminito.cursor() {
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
            let image = caminito.cursors.image(icon, scale);
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
pub(crate) fn pointer_on(caminito: &Caminito, output: &Output) -> Option<Point<f64, Logical>> {
    let pointer = caminito.seat.get_pointer().unwrap().current_location();
    output_geometry(output)
        .to_f64()
        .contains(pointer)
        .then_some(pointer)
}

/// The drag icon at the pointer plus its offset, if the pointer's on `output`.
fn drag_icon_elements<R>(
    renderer: &mut R,
    caminito: &Caminito,
    output: &Output,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let (Some(icon), Some(pointer)) = (&caminito.drag_icon, pointer_on(caminito, output)) else {
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
    caminito: &Caminito,
    output: &Output,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let output_geometry = output_geometry(output);
    let scale = output.current_scale().fractional_scale();
    for window in caminito.override_redirect_windows() {
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

/// The current workspace's windows of `layer`, of the two that hold windows.
fn window_elements<R>(
    layer: SceneLayer,
    renderer: &mut R,
    caminito: &Caminito,
    output: &Output,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let policy = &caminito.policy;
    let output_geometry = output_geometry(output);
    let scale = output.current_scale().fractional_scale();

    for &id in policy.stacking(policy.active_workspace()).iter().rev() {
        let (Some(mapped), Some(geometry)) = (policy.window(id), policy.geometry(id)) else {
            continue;
        };
        if !policy.is_showing(id) || !window_is_in(caminito, id, layer) {
            continue;
        }
        if caminito.debug.frame_debug {
            frame_debug_elements(renderer, caminito, mapped, geometry.loc, output, elements);
        }

        let origin = geometry.loc - output_geometry.loc;
        let shown = Rectangle::from_size(output_geometry.size);
        one_window_elements(renderer, mapped, origin, shown, scale, elements);
    }
}

/// One window's elements, front to back, at `scale`: its surfaces with their popups, over
/// its frame's strips. `origin` is where its geometry is from the origin of what's drawn,
/// and `shown` is the part of that space that's drawn.
fn one_window_elements<R, E>(
    renderer: &mut R,
    mapped: &Mapped,
    origin: Point<i32, Logical>,
    shown: Rectangle<i32, Logical>,
    scale: f64,
    elements: &mut Vec<E>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
    E: From<WaylandSurfaceRenderElement<R>> + From<MemoryRenderBufferRenderElement<R>>,
{
    let window = mapped.window();
    // The surface's origin sits the window geometry's offset away from where policy
    // placed the geometry. A shaded window's surfaces have left the scene.
    let surface_origin = origin - window.geometry().loc;
    let mut bbox = window.bbox_with_popups();
    bbox.loc += surface_origin;
    if !mapped.is_shaded() && bbox.overlaps(shown) {
        let location = surface_origin.to_physical_precise_round(scale);
        elements.extend(window.render_elements(renderer, location, Scale::from(scale), 1.0));
    }

    // The strips lie around the window geometry, behind the window and its popups.
    for (mut rect, buffer) in mapped.frame().strips() {
        rect.loc += origin;
        if !rect.overlaps(shown) {
            continue;
        }
        let location = rect.loc.to_physical_precise_round::<_, i32>(scale).to_f64();
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
            Ok(element) => elements.push(E::from(element)),
            Err(err) => tracing::warn!("can't draw a frame strip: {err:?}"),
        }
    }
}

/// Frame debug's outlines of a frame's strips, invisible resize border and title rect, and
/// the label of the scale its strips were rendered at, over the window. `origin` is where
/// the window geometry is, globally.
fn frame_debug_elements<R>(
    renderer: &mut R,
    caminito: &Caminito,
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

    if let Some(label) = caminito.debug.scale_label(frame.scale()) {
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
            SceneElement::Solid(SolidColorRenderElement::new(
                Id::new(),
                side,
                CommitCounter::default(),
                color,
                Kind::Unspecified,
            ))
        }));
    }
}
