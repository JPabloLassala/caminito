//! Per-output render-element assembly, in the scene's layer order (15).
//!
//! There's no retained scene graph: each output's elements are rebuilt every frame, and this
//! is the only code that knows the layer order.
//!
//! What's read from policy here is what `Policy::take_scene_changed` watches, for the
//! refresh pass to draw the outputs again (179). A query of policy that's new here goes in
//! `SCENE_QUERIES` (`caminito-policy/src/tests/invariants.rs`), whose test fails until it
//! does, and in policy's `SceneReads`.

use caminito_policy::{Layer as StackLayer, SwitchEntry, WindowId};
use caminito_theme::{GtkState, Variant};
use smithay::backend::renderer::element::Id;
use smithay::backend::renderer::element::memory::{
    MemoryRenderBuffer, MemoryRenderBufferRenderElement,
};
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
use smithay::utils::{Logical, Physical, Point, Rectangle, Scale, Size};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::wlr_layer::Layer;
use smithay::xwayland::X11Surface;

use crate::output::{output_geometry, output_id};
use crate::overview::{Laid, Overview};
use crate::state::{Caminito, SEAT};
use crate::switcher::layout::{CellStyle, Grid};
use crate::window::Mapped;

/// What shows where nothing is drawn.
pub const CLEAR_COLOR: Color32F = Color32F::new(0.2, 0.2, 0.2, 1.0);

/// What a blanked output shows (128).
pub(crate) const BLANK: Color32F = Color32F::new(0.0, 0.0, 0.0, 1.0);

/// What a locked output shows wherever the locker draws nothing (12).
pub(crate) const LOCK_BACKGROUND: Color32F = Color32F::new(0.0, 0.0, 0.0, 1.0);

/// What the visual bell's full-screen flash shows: white, as Marco fills its flash window
/// (`bell.c:bell_flash_screen`) (169).
pub(crate) const BELL_FLASH: Color32F = Color32F::new(1.0, 1.0, 1.0, 1.0);

/// How opaque the tile preview's fill is: GTK's default `selection-box-alpha`, which Marco
/// fills its preview at (tile-preview.c:47-122) (158).
pub(crate) const TILE_PREVIEW_ALPHA: f32 = 64.0 / 255.0;

/// Frame debug's outline colours (14).
const STRIP_OUTLINE: Color32F = Color32F::new(1.0, 0.0, 0.0, 1.0);
const RESIZE_BORDER_OUTLINE: Color32F = Color32F::new(0.0, 0.5, 1.0, 1.0);
const TITLE_OUTLINE: Color32F = Color32F::new(0.0, 1.0, 0.0, 1.0);

render_elements! {
    /// A part of one window, as a thumbnail (108) or the minimise animation (172) draws it
    /// smaller.
    pub WindowPart<R> where R: ImportAll + ImportMem;
    /// One of the window's surfaces.
    Surface = WaylandSurfaceRenderElement<R>,
    /// One of its frame's strips.
    Strip = MemoryRenderBufferRenderElement<R>,
}

/// A part of a window in a thumbnail of the Window switcher, or in the minimise
/// animation's box: scaled down, moved into its place and cut off at its edge.
type ThumbnailPart<R> =
    CropRenderElement<RelocateRenderElement<RescaleRenderElement<WindowPart<R>>>>;

render_elements! {
    pub SceneElement<R> where R: ImportAll + ImportMem;
    /// A client's surface: a window's, a layer surface's, the cursor's, or a drag icon's.
    Window = WaylandSurfaceRenderElement<R>,
    /// A frame's strip, one of Caminito's cursors, or frame debug's label.
    Frame = MemoryRenderBufferRenderElement<R>,
    /// A side of one of frame debug's outlines, the lock's background, the blank, the
    /// visual bell's flash, or the Overview's dimming.
    Solid = SolidColorRenderElement,
    /// A part of a window in one of the Window switcher's thumbnails (108), or of one
    /// shrinking into its button (172), or of one at its place in the Overview (241).
    Thumbnail = ThumbnailPart<R>,
}

/// The scene's layers, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SceneLayer {
    /// While the outputs are blanked or powered off, black over everything, the cursor and
    /// the lock too, and nothing of the layers below (128, 129).
    Blank,
    /// The pointer's cursor, over everything else.
    Cursor,
    /// A Wayland drag's icon, at the pointer (48).
    DragIcon,
    /// While the session is locked, the output's lock surface over the solid lock
    /// background, and nothing of the layers below (12).
    Lock,
    /// The visual bell's full-screen flash, over everything of the session and under the
    /// lock, which shows nothing of it (169).
    BellFlash,
    /// The Window switcher's popup (108), over everything a client draws: the corner
    /// icons, then the thumbnails, then the rest of its chrome.
    Switcher,
    /// The Workspace popup (152), over everything a client draws, as the Window switcher's
    /// popup is. The two never show together.
    WorkspacePopup,
    /// The window menu (161), over everything a client draws, its submenu over it.
    WindowMenu,
    /// The overlay layer-shell layer (63).
    Overlay,
    /// X's override-redirect windows, its menus and tooltips, in X's stacking order and on
    /// no workspace (36).
    OverrideRedirect,
    /// The windows shrinking into their buttons as they're minimised (172), over the
    /// panel their buttons are on and every window, as Marco's box is its newest
    /// override-redirect window, and under X's menus, which are a client's.
    Minimizing,
    /// The current workspace's windows in Marco's fullscreen layer: a fullscreen window
    /// that has the focus, and its transients. They cover the top layer, as Marco puts
    /// them above docks (63).
    FullscreenWindows,
    /// While the Overview is open on the output (241), all of it: its bar of workspaces
    /// (244), over each listed window at its place in the layout, over its label, over the
    /// dimming, which covers the wallpaper. In its place the output shows none of its
    /// windows as they are, nor one shrinking into its button, nothing of the top and
    /// bottom layers, where the panel is, and of the background layer the wallpaper alone,
    /// without caja's desktop.
    Overview,
    /// The top layer-shell layer (63).
    Top,
    /// The popups of the bottom and background layers' surfaces. They're above the windows,
    /// as sway and niri draw them, so no window cuts off the menu of caja's desktop, and
    /// below the top layer, so a panel covers them. A fullscreen window that has the focus
    /// covers them as it covers the panel (80).
    LayerPopups,
    /// The current workspace's other windows in policy's stacking order, each with its
    /// frame, and the tile preview just beneath the window being moved, as Marco lowers its
    /// preview beneath the focused window (tile-preview.c:182-184) (158). While the output
    /// slides between two workspaces (233), the windows of the one it left are here too,
    /// and those of both are away from their own places.
    Windows,
    /// The bottom layer-shell layer (63), without its surfaces' popups.
    Bottom,
    /// The background layer-shell layer (63), with the wallpaper behind caja's desktop,
    /// and without its surfaces' popups.
    Background,
}

/// The namespace of the wallpaper, which is behind every other surface of the background
/// layer, whichever mapped first: caja's `desktop` covers it where caja draws one (06).
/// caminito-wallpaperd draws it under msd's background plugin's namespace, which it took
/// over (210).
const WALLPAPER_NAMESPACE: &str = "mate-background";

pub(crate) const LAYERS: [SceneLayer; 18] = [
    SceneLayer::Blank,
    SceneLayer::Cursor,
    SceneLayer::DragIcon,
    SceneLayer::Lock,
    SceneLayer::BellFlash,
    SceneLayer::Switcher,
    SceneLayer::WorkspacePopup,
    SceneLayer::WindowMenu,
    SceneLayer::Overlay,
    SceneLayer::OverrideRedirect,
    SceneLayer::Minimizing,
    SceneLayer::FullscreenWindows,
    SceneLayer::Overview,
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

/// The layers that are drawn, top to bottom: all of them, while the session is locked the
/// lock and what's above it, and while the outputs are blanked or powered off the blank
/// alone (128, 129). The lock's background and the blank cover the rest, so they're left
/// out.
fn shown_layers(caminito: &Caminito) -> impl Iterator<Item = SceneLayer> {
    let position = |shown: SceneLayer| {
        LAYERS
            .iter()
            .position(|layer| *layer == shown)
            .expect("every layer is one of the layers")
    };
    let shown = if caminito.idle.shows_blank() {
        position(SceneLayer::Blank) + 1
    } else if caminito.lock.is_locked() {
        position(SceneLayer::Lock) + 1
    } else {
        LAYERS.len()
    };
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
    if overview_replaces(caminito, output, layer) {
        return;
    }
    match layer {
        SceneLayer::Overview => overview_elements(renderer, caminito, output, elements),
        SceneLayer::Blank => blank_elements(caminito, output, elements),
        SceneLayer::Cursor => cursor_elements(renderer, caminito, output, elements),
        SceneLayer::DragIcon => drag_icon_elements(renderer, caminito, output, elements),
        SceneLayer::OverrideRedirect => {
            override_redirect_elements(renderer, caminito, output, elements)
        }
        SceneLayer::Minimizing => minimizing_elements(renderer, caminito, output, elements),
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
        SceneLayer::BellFlash => bell_flash_elements(caminito, output, elements),
        SceneLayer::Switcher => switcher_elements(renderer, caminito, output, elements),
        SceneLayer::WorkspacePopup => {
            workspace_popup_elements(renderer, caminito, output, elements);
        }
        SceneLayer::WindowMenu => window_menu_elements(renderer, caminito, output, elements),
    }
}

/// While the outputs are blanked or powered off, black over all of `output`.
fn blank_elements<R>(caminito: &Caminito, output: &Output, elements: &mut Vec<SceneElement<R>>)
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    if !caminito.idle.shows_blank() {
        return;
    }
    let scale = output.current_scale().fractional_scale();
    let covered = Rectangle::from_size(output_geometry(output).size).to_physical_precise_up(scale);
    elements.push(SceneElement::Solid(SolidColorRenderElement::new(
        caminito.idle.blank_id().clone(),
        covered,
        CommitCounter::default(),
        BLANK,
        Kind::Unspecified,
    )));
}

/// While the visual bell flashes `output`, white over all of it (169).
fn bell_flash_elements<R>(caminito: &Caminito, output: &Output, elements: &mut Vec<SceneElement<R>>)
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let Some(id) = caminito.bells.output_flash(output_id(output)) else {
        return;
    };
    let scale = output.current_scale().fractional_scale();
    let covered = Rectangle::from_size(output_geometry(output).size).to_physical_precise_up(scale);
    elements.push(SceneElement::Solid(SolidColorRenderElement::new(
        id.clone(),
        covered,
        CommitCounter::default(),
        BELL_FLASH,
        Kind::Unspecified,
    )));
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

/// Whether the Overview is open on `output` (241), as policy has it.
pub(crate) fn overview_is_on(caminito: &Caminito, output: &Output) -> bool {
    let open = caminito.policy.overview();
    open.is_some_and(|open| open.output == output_id(output))
}

/// Whether the Overview, open on `output`, is drawn in place of all of `layer`: the
/// windows as they are, and one that was shrinking into its button as it opened, which
/// is in the layout at its place.
fn overview_replaces(caminito: &Caminito, output: &Output, layer: SceneLayer) -> bool {
    let windows = matches!(
        layer,
        SceneLayer::Minimizing | SceneLayer::FullscreenWindows | SceneLayer::Windows
    );
    windows && overview_is_on(caminito, output)
}

/// Whether the Overview, open on `output`, keeps a layer surface of `layer` with this
/// `namespace` from being drawn there, and its popups with it: the panel, which is one of
/// the top or bottom layer's, and caja's desktop, which is one of the background layer's
/// that isn't the wallpaper.
pub(crate) fn overview_hides(
    caminito: &Caminito,
    output: &Output,
    layer: Layer,
    namespace: &str,
) -> bool {
    let hidden = match layer {
        Layer::Top | Layer::Bottom => true,
        Layer::Background => namespace != WALLPAPER_NAMESPACE,
        Layer::Overlay => false,
    };
    hidden && overview_is_on(caminito, output)
}

/// The Overview (241), if it's open on `output`, front to back: its bar of workspaces
/// (244), each listed window's label, the windows at their places in the layout, and the
/// dimming over all of the output. While a Workspace slide runs on the output, each
/// workspace's layout is where the slide has that workspace, the one that leaves too. A
/// window is drawn from its own surfaces at every frame, as a thumbnail of the Window
/// switcher is, so it's live, and a minimised one shows as any other does.
fn overview_elements<R>(
    renderer: &mut R,
    caminito: &Caminito,
    output: &Output,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    if !overview_is_on(caminito, output) {
        return;
    }
    // Policy's Overview is laid out in the refresh pass, before the output is drawn.
    let Some(open) = caminito.overview_on(output) else {
        return;
    };
    let scale = output.current_scale().fractional_scale();
    overview_bar_elements(renderer, caminito, output, open, elements);
    for laid in open.layouts() {
        for shift in caminito.overview_shifts(open, laid) {
            overview_layout_elements(renderer, caminito, laid, shift, scale, elements);
        }
    }
    let covered = Rectangle::from_size(output_geometry(output).size).to_physical_precise_up(scale);
    elements.push(SceneElement::Solid(SolidColorRenderElement::new(
        open.dim.clone(),
        covered,
        CommitCounter::default(),
        Color32F::new(0.0, 0.0, 0.0, crate::overview::DIM),
        Kind::Unspecified,
    )));
}

/// One workspace's layout in the Overview, `shift` away from its places, front to back:
/// each window's label, then the windows.
fn overview_layout_elements<R>(
    renderer: &mut R,
    caminito: &Caminito,
    laid: &Laid,
    shift: Point<i32, Logical>,
    scale: f64,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    for (slot, label) in laid.layout.slots.iter().zip(&laid.labels) {
        let Some(label) = label else {
            continue;
        };
        let location = (slot.label.loc + shift).to_physical_precise_round::<_, i32>(scale);
        let element = MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            location.to_f64(),
            label,
            None,
            None,
            Some(slot.label.size),
            Kind::Unspecified,
        );
        match element {
            Ok(element) => elements.push(SceneElement::Frame(element)),
            // Every frame while the Overview is open, so not above `debug!`.
            Err(err) => tracing::debug!("can't draw the overview's label: {err:?}"),
        }
    }
    for (id, slot) in laid.windows.iter().zip(&laid.layout.slots) {
        let target = Rectangle::new(slot.window.loc + shift, slot.window.size);
        scaled_window_elements(renderer, caminito, *id, |_| target, None, scale, elements);
    }
}

/// Window `id` with its frame, scaled to fill the target `place` gives its frame rect, and
/// cut off at the edge of that target, or of `within` where it's given: as a thumbnail
/// draws it, from its own surfaces.
fn scaled_window_elements<R>(
    renderer: &mut R,
    caminito: &Caminito,
    id: WindowId,
    place: impl Fn(Rectangle<i32, Logical>) -> Rectangle<i32, Logical>,
    within: Option<Rectangle<i32, Logical>>,
    scale: f64,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let policy = &caminito.policy;
    let (Some(mapped), Some(geometry), Some(frame)) = (
        policy.window(id),
        policy.geometry(id),
        policy.frame_rect(id),
    ) else {
        return;
    };
    if frame.size.w <= 0 || frame.size.h <= 0 {
        return;
    }
    let target = place(frame);
    let crop = match within {
        Some(within) => target.intersection(within),
        None => Some(target),
    };
    let Some(crop) = crop else {
        return;
    };
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
    scaled_elements_in(parts, frame.size, target, crop, scale, elements);
}

/// How wide the mark around the active workspace's cell of the Overview's bar is (244).
const CELL_MARK: i32 = 3;

/// What's behind a cell of the Overview's bar where the wallpaper doesn't cover it.
const CELL_BACK: Color32F = Color32F::new(0.0, 0.0, 0.0, 1.0);

/// The Overview's bar of workspaces on `output` (244), front to back: the mark around the
/// active workspace's cell, then in each cell that workspace on the output as a small
/// picture, its windows at their real places over the wallpaper, over the cell's back. It's
/// drawn from the windows' and the wallpaper's own surfaces at every frame, so it's live.
fn overview_bar_elements<R>(
    renderer: &mut R,
    caminito: &Caminito,
    output: &Output,
    open: &Overview,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let output_geometry = output_geometry(output);
    let scale = output.current_scale().fractional_scale();
    let wallpapers: Vec<_> = layer_surfaces_in(caminito, output, SceneLayer::Background)
        .into_iter()
        .filter(|(surface, _)| surface.namespace() == WALLPAPER_NAMESPACE)
        .collect();
    if let Some(cell) = open.bar.cells.get(open.active) {
        let color = selection_color(caminito);
        let around = Rectangle::new(
            cell.loc - Point::from((CELL_MARK, CELL_MARK)),
            cell.size + Size::from((2 * CELL_MARK, 2 * CELL_MARK)),
        );
        let rect = around.to_physical_precise_round::<_, i32>(scale);
        let width = (f64::from(CELL_MARK) * scale).round() as i32;
        let (w, h) = (rect.size.w, rect.size.h);
        let sides = [
            Rectangle::new(rect.loc, (w, width).into()),
            Rectangle::new(rect.loc + Point::from((0, h - width)), (w, width).into()),
            Rectangle::new(rect.loc, (width, h).into()),
            Rectangle::new(rect.loc + Point::from((w - width, 0)), (width, h).into()),
        ];
        for (id, side) in open.mark.iter().zip(sides) {
            elements.push(SceneElement::Solid(SolidColorRenderElement::new(
                id.clone(),
                side,
                CommitCounter::default(),
                color,
                Kind::Unspecified,
            )));
        }
    }
    let cells = open.bar.cells.iter().zip(&open.cell_windows);
    for ((cell, windows), back) in cells.zip(&open.cell_backs) {
        // Where something at `rect` on the output is in the cell, which has the output's
        // aspect.
        let factor = f64::from(cell.size.w) / f64::from(output_geometry.size.w.max(1));
        let in_cell = |rect: Rectangle<i32, Logical>| {
            let at = |n: i32| (f64::from(n) * factor).round() as i32;
            let loc = rect.loc - output_geometry.loc;
            Rectangle::new(
                cell.loc + Point::from((at(loc.x), at(loc.y))),
                Size::from((at(rect.size.w).max(1), at(rect.size.h).max(1))),
            )
        };
        for id in windows.iter().rev() {
            scaled_window_elements(
                renderer,
                caminito,
                *id,
                in_cell,
                Some(*cell),
                scale,
                elements,
            );
        }
        for (wallpaper, geometry) in &wallpapers {
            if geometry.size.w <= 0 || geometry.size.h <= 0 {
                continue;
            }
            let parts = AsRenderElements::<R>::render_elements::<WindowPart<R>>(
                wallpaper,
                renderer,
                Point::default(),
                Scale::from(scale),
                1.0,
            );
            let target = in_cell(*geometry);
            if let Some(crop) = target.intersection(*cell) {
                scaled_elements_in(parts, geometry.size, target, crop, scale, elements);
            }
        }
        elements.push(SceneElement::Solid(SolidColorRenderElement::new(
            back.clone(),
            cell.to_physical_precise_round(scale),
            CommitCounter::default(),
            CELL_BACK,
            Kind::Unspecified,
        )));
    }
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

/// The Workspace popup (152), if it's on `output`: one buffer, drawn in the refresh pass.
fn workspace_popup_elements<R>(
    renderer: &mut R,
    caminito: &Caminito,
    output: &Output,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let popup = caminito.workspace_popup.as_ref();
    let Some(popup) = popup.filter(|popup| popup.output == *output) else {
        return;
    };
    let Some(chrome) = &popup.chrome else {
        return;
    };
    let scale = output.current_scale().fractional_scale();
    let rect = popup.layout.popup;
    let location = rect.loc.to_physical_precise_round::<_, i32>(scale);
    let element = MemoryRenderBufferRenderElement::from_buffer(
        renderer,
        location.to_f64(),
        chrome,
        None,
        None,
        Some(rect.size),
        Kind::Unspecified,
    );
    match element {
        Ok(element) => elements.push(SceneElement::Frame(element)),
        // Every frame while the popup shows, so not above `debug!`.
        Err(err) => tracing::debug!("can't draw the workspace popup: {err:?}"),
    }
}

/// The window menu (161), if it's on `output`, front to back: its submenu, then the menu,
/// each one buffer drawn in the refresh pass.
fn window_menu_elements<R>(
    renderer: &mut R,
    caminito: &Caminito,
    output: &Output,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let shown = caminito.shown_window_menu();
    let Some(shown) = shown.filter(|shown| shown.output == *output) else {
        return;
    };
    let scale = output.current_scale().fractional_scale();
    for pane in shown.submenu.iter().chain([&shown.main]) {
        let Some(chrome) = &pane.chrome else {
            continue;
        };
        let rect = pane.layout.rect;
        let location = rect.loc.to_physical_precise_round::<_, i32>(scale);
        let element = MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            location.to_f64(),
            chrome,
            None,
            None,
            Some(rect.size),
            Kind::Unspecified,
        );
        match element {
            Ok(element) => elements.push(SceneElement::Frame(element)),
            // Every frame while the menu shows, so not above `debug!`.
            Err(err) => tracing::debug!("can't draw the window menu: {err:?}"),
        }
    }
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
    // The grid is laid out again in the refresh pass, so it has a cell for each entry.
    let entries = switcher.entries.iter().enumerate();
    for (index, entry) in entries.take(grid.cells.len()) {
        let mut parts: Vec<WindowPart<R>> = Vec::new();
        // What the thumbnail shows whole: a window's frame, or a Layer surface (163).
        let size = match *entry {
            SwitchEntry::Window(id) => {
                let (Some(mapped), Some(geometry), Some(frame)) = (
                    policy.window(id),
                    policy.geometry(id),
                    policy.frame_rect(id),
                ) else {
                    continue;
                };
                let shown = Rectangle::from_size(frame.size);
                one_window_elements(
                    renderer,
                    mapped,
                    geometry.loc - frame.loc,
                    shown,
                    scale,
                    &mut parts,
                );
                frame.size
            }
            SwitchEntry::Layer(id) => {
                let Some((layer, geometry)) = caminito.layer_named(id) else {
                    continue;
                };
                parts.extend(AsRenderElements::<R>::render_elements::<WindowPart<R>>(
                    &layer,
                    renderer,
                    Point::default(),
                    Scale::from(scale),
                    1.0,
                ));
                geometry.size
            }
        };
        if size.w <= 0 || size.h <= 0 {
            continue;
        }
        let target = grid.thumbnail(index, size);
        scaled_elements(parts, size, target, scale, elements);
    }
}

/// Each window that's shrinking into its button (172), where it overlaps `output`, drawn as
/// a thumbnail is: from the window's own surfaces, which keep their buffers while it's
/// minimised, with its frame, in the box the animation has it in now.
fn minimizing_elements<R>(
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
    for (id, target) in caminito.minimize_animations.boxes(caminito.now()) {
        let (Some(mapped), Some(geometry), Some(frame)) = (
            policy.window(id),
            policy.geometry(id),
            policy.frame_rect(id),
        ) else {
            continue;
        };
        if !target.overlaps(output_geometry) || frame.size.w <= 0 || frame.size.h <= 0 {
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
        let target = Rectangle::new(target.loc - output_geometry.loc, target.size);
        scaled_elements(parts, frame.size, target, scale, elements);
    }
}

/// `parts`, which show something of `size` whole from the origin, scaled to fill `target`,
/// which is from the origin of what's drawn, and cut off at its edge.
fn scaled_elements<R>(
    parts: Vec<WindowPart<R>>,
    size: Size<i32, Logical>,
    target: Rectangle<i32, Logical>,
    scale: f64,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    scaled_elements_in(parts, size, target, target, scale, elements);
}

/// `parts` as `scaled_elements` has them, cut off at the edge of `crop` instead, which is
/// from the origin of what's drawn too.
fn scaled_elements_in<R>(
    parts: Vec<WindowPart<R>>,
    size: Size<i32, Logical>,
    target: Rectangle<i32, Logical>,
    crop: Rectangle<i32, Logical>,
    scale: f64,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let factor = Scale::from((
        f64::from(target.size.w) / f64::from(size.w),
        f64::from(target.size.h) / f64::from(size.h),
    ));
    let location: Point<i32, Physical> = target.loc.to_physical_precise_round(scale);
    // A window's popups and subsurfaces can reach past its frame.
    let crop: Rectangle<i32, Physical> = crop.to_physical_precise_round(scale);
    for part in parts {
        let part = RescaleRenderElement::from_element(part, Point::default(), factor);
        let part = RelocateRenderElement::from_element(part, location, Relocate::Relative);
        if let Some(part) = CropRenderElement::from_element(part, scale, crop) {
            elements.push(SceneElement::Thumbnail(part));
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
    policy.active_stacking().iter().any(|&id| {
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
        if overview_hides(caminito, output, surface.layer(), surface.namespace()) {
            continue;
        }
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

/// The current workspace's windows of `layer`, of the two that hold windows. While
/// `output` slides between two workspaces (233), the windows that arrive are drawn on it
/// away from their places, and so are those that left, which don't show: each in its
/// place in the one stack, with its frame, its shadow and its popups. A window of a
/// workspace the slide has at more than one place is drawn at each (239). Another output
/// draws them as policy has them, so nothing of a slide shows on it.
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
    let now = caminito.now();
    let slide = caminito.workspace_slides.on(output_id(output), now);
    let stacking = match slide {
        Some(_) => policy.stacking_all().to_vec(),
        None => policy.active_stacking(),
    };

    for &id in stacking.iter().rev() {
        let (Some(mapped), Some(geometry)) = (policy.window(id), policy.geometry(id)) else {
            continue;
        };
        let showing = policy.is_showing(id);
        let mut shifts = slide.map_or_else(Vec::new, |slide| slide.shifts(id, showing, now));
        let drawn = showing || !shifts.is_empty();
        if !drawn || !window_is_in(caminito, id, layer) {
            continue;
        }
        if shifts.is_empty() {
            // The slide moves it nowhere.
            shifts.push(Point::default());
        }
        for shift in shifts {
            let drawn_at = geometry.loc + shift;
            if caminito.debug.frame_debug {
                frame_debug_elements(renderer, caminito, mapped, drawn_at, output, elements);
            }

            let origin = drawn_at - output_geometry.loc;
            let shown = Rectangle::from_size(output_geometry.size);
            one_window_elements(renderer, mapped, origin, shown, scale, elements);
            // The shadow is the output's alone: a capture of the window and its thumbnail
            // are the window with its frame (103, 171).
            let shadow = mapped.frame().shadow();
            buffer_elements(renderer, shadow, origin, shown, scale, "shadow", elements);
        }
        if layer == SceneLayer::Windows && policy.grab(SEAT) == Some(id) {
            tile_preview_elements(caminito, output, elements);
        }
    }
}

/// While a move's pointer is in a tiling zone, the tile preview on `output`: where letting
/// go puts the window's frame, in the palette's selected background, translucent inside a
/// line of it one pixel wide, as Marco draws it (tile-preview.c:47-122) (158).
fn tile_preview_elements<R>(
    caminito: &Caminito,
    output: &Output,
    elements: &mut Vec<SceneElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let output_geometry = output_geometry(output);
    let Some(preview) = caminito
        .policy
        .tile_preview(SEAT)
        .filter(|preview| preview.overlaps(output_geometry))
    else {
        return;
    };
    let line = selection_color(caminito);
    let (r, g, b) = (line.r(), line.g(), line.b());
    let a = TILE_PREVIEW_ALPHA;
    let fill = Color32F::new(r * a, g * a, b * a, a);

    let scale = output.current_scale().fractional_scale();
    let local = Rectangle::new(preview.loc - output_geometry.loc, preview.size);
    let rect = local.to_physical_precise_round::<_, i32>(scale);
    let width = (scale.round() as i32).max(1);
    let (w, h) = (rect.size.w, rect.size.h);
    let sides = [
        Rectangle::new(rect.loc, (w, width).into()),
        Rectangle::new(rect.loc + Point::from((0, h - width)), (w, width).into()),
        Rectangle::new(rect.loc, (width, h).into()),
        Rectangle::new(rect.loc + Point::from((w - width, 0)), (width, h).into()),
    ];
    let inside = Rectangle::new(
        rect.loc + Point::from((width, width)),
        (w - 2 * width, h - 2 * width).into(),
    );
    // New ids each frame, as frame debug's outlines have: the preview shows only during a
    // drag, which draws every frame anyway.
    let solid = |area, color| {
        SceneElement::Solid(SolidColorRenderElement::new(
            Id::new(),
            area,
            CommitCounter::default(),
            color,
            Kind::Unspecified,
        ))
    };
    elements.extend(sides.into_iter().map(|side| solid(side, line)));
    if !inside.is_empty() {
        elements.push(solid(inside, fill));
    }
}

/// The theme's selected background, opaque, which the tile preview and the mark of the
/// Overview's active workspace (244) are drawn in. A theme, or the neutral palette, that
/// selects in the background of anything else would hide them: the text colour draws them
/// then, as the Window switcher's selection does.
fn selection_color(caminito: &Caminito) -> Color32F {
    let colors = caminito.palette.palette().variant(Variant::Default);
    let selected = colors.background(GtkState::Selected);
    let color = if selected == colors.background(GtkState::Normal) {
        colors.text(GtkState::Normal)
    } else {
        selected
    };
    Color32F::new(color.red as f32, color.green as f32, color.blue as f32, 1.0)
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
    let strips = mapped.frame().strips();
    buffer_elements(renderer, strips, origin, shown, scale, "strip", elements);
}

/// The elements of the `buffers` of a frame, its strips or its shadow's bands, that are in
/// `shown`, at `scale`. Each has its place from the window geometry's origin, which is at
/// `origin`.
fn buffer_elements<'a, R, E>(
    renderer: &mut R,
    buffers: impl Iterator<Item = (Rectangle<i32, Logical>, &'a MemoryRenderBuffer)>,
    origin: Point<i32, Logical>,
    shown: Rectangle<i32, Logical>,
    scale: f64,
    what: &str,
    elements: &mut Vec<E>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
    E: From<MemoryRenderBufferRenderElement<R>>,
{
    for (mut rect, buffer) in buffers {
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
            Err(err) => tracing::warn!("can't draw a frame's {what}: {err:?}"),
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
