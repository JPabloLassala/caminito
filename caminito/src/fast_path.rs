//! Fast-path eligibility (CMN-312): whether an output's frame is one opaque fullscreen window
//! that covers the output with nothing drawn over it, and if not, why not. It's worked out
//! from the scene's elements as each frame of the output is built, so it changes with the
//! next frame, and the debug dump reports the last frame's. Nothing about the rendering
//! depends on it yet.
//!
//! The pointer's cursor doesn't count as drawn over the window: it goes on its own plane.

use caminito_policy::WindowId;
use smithay::backend::renderer::element::{Element, Id};
use smithay::backend::renderer::{ImportAll, ImportMem, Renderer};
use smithay::desktop::PopupManager;
use smithay::output::Output;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Physical, Rectangle, Scale};
use smithay::wayland::compositor::{TraversalAction, with_surface_tree_downward};
use smithay::wayland::seat::WaylandFocus;

use crate::esmeril::DrawsEsmeril;
use crate::output::{output_geometry, output_id};
use crate::scene::{SceneElement, SceneLayer, window_is_in};
use crate::state::Caminito;

/// Whether an output's frame can take the fast path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FastPath {
    /// The window is all the frame shows.
    Eligible(WindowId),
    NotEligible(NotEligible),
}

/// Why an output's frame can't take the fast path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotEligible {
    /// No fullscreen window in the fullscreen layer shows on the output.
    NoFullscreenWindow,
    /// The fullscreen window doesn't cover all of the output.
    DoesNotCover,
    /// The fullscreen window covers the output, but some of it can be seen through.
    Translucent,
    /// Something is drawn over the fullscreen window.
    DrawnOver(Over),
}

/// What is drawn over the fullscreen window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Over {
    /// A Workspace slide runs on the output (233).
    WorkspaceSlide,
    /// One of the window's popups, or a closed one's picture as it slides away (ADR 0037).
    Popup,
    /// One of the window's subsurfaces.
    Subsurface,
    /// Another window of the fullscreen layer, such as one of its transients.
    Window,
    /// Something of a layer above the fullscreen layer, or one drawn in its place.
    Layer(SceneLayer),
}

/// Whether the frame of `output` made of `elements`, front to back, each of the scene layer
/// `layers` has at its index, can take the fast path.
pub(crate) fn fast_path<R>(
    caminito: &Caminito,
    output: &Output,
    elements: &[SceneElement<R>],
    layers: &[SceneLayer],
) -> FastPath
where
    R: Renderer + ImportAll + ImportMem + DrawsEsmeril,
    R::TextureId: Send + Clone + 'static,
{
    let not = FastPath::NotEligible;
    // A Workspace slide moves every window of the output off its place (233).
    if caminito
        .workspace_slides
        .on(output_id(output), caminito.now())
        .is_some()
    {
        return not(NotEligible::DrawnOver(Over::WorkspaceSlide));
    }
    let Some(window) = fullscreen_window(caminito, output) else {
        return not(NotEligible::NoFullscreenWindow);
    };
    let Some(mapped) = caminito.policy.window(window) else {
        return not(NotEligible::NoFullscreenWindow);
    };
    let Some(root) = mapped.window().wl_surface() else {
        return not(NotEligible::NoFullscreenWindow);
    };
    let root_id = Id::from_wayland_resource(&*root);
    let scale = Scale::from(output.current_scale().fractional_scale());
    let shown = output_geometry(output)
        .size
        .to_physical_precise_round(scale);
    let shown: Rectangle<i32, Physical> = Rectangle::from_size(shown);
    // What's off the output, as another output's windows are, isn't drawn on it.
    let front = elements
        .iter()
        .zip(layers)
        .filter(|(_, layer)| **layer != SceneLayer::Cursor)
        .find(|(element, _)| element.geometry(scale).overlaps(shown));
    let Some((element, layer)) = front else {
        return not(NotEligible::NoFullscreenWindow);
    };
    if *element.id() != root_id {
        return not(NotEligible::DrawnOver(over(&root, element, *layer)));
    }
    let geometry = element.geometry(scale);
    if !geometry.contains_rect(shown) {
        return not(NotEligible::DoesNotCover);
    }
    let opaque = element.opaque_regions(scale).into_iter().map(|mut region| {
        region.loc += geometry.loc;
        region
    });
    if !shown.subtract_rects(opaque).is_empty() {
        return not(NotEligible::Translucent);
    }
    FastPath::Eligible(window)
}

/// The front-most window in the fullscreen layer that's fullscreen, shows and is on `output`.
fn fullscreen_window(caminito: &Caminito, output: &Output) -> Option<WindowId> {
    let policy = &caminito.policy;
    let output_geometry = output_geometry(output);
    let stacking = policy.active_stacking();
    stacking.iter().rev().copied().find(|&id| {
        policy.is_showing(id)
            && policy.is_fullscreen(id)
            && window_is_in(caminito, id, SceneLayer::FullscreenWindows)
            && policy
                .geometry(id)
                .is_some_and(|geometry| geometry.overlaps(output_geometry))
    })
}

/// What `element`, of `layer`, which is drawn over the window whose surface is `root`, is.
fn over<R>(root: &WlSurface, element: &SceneElement<R>, layer: SceneLayer) -> Over
where
    R: Renderer + ImportAll + ImportMem + DrawsEsmeril,
    R::TextureId: Send + Clone + 'static,
{
    if layer != SceneLayer::FullscreenWindows {
        return Over::Layer(layer);
    }
    if matches!(element, SceneElement::PopupPicture(_)) {
        return Over::Popup;
    }
    let id = element.id();
    let popups = PopupManager::popups_for_surface(root);
    if popups
        .into_iter()
        .any(|(popup, _)| tree_ids(popup.wl_surface()).contains(id))
    {
        return Over::Popup;
    }
    if tree_ids(root).contains(id) {
        return Over::Subsurface;
    }
    Over::Window
}

/// The element ids of the surfaces of `surface`'s tree.
fn tree_ids(surface: &WlSurface) -> Vec<Id> {
    let mut ids = Vec::new();
    with_surface_tree_downward(
        surface,
        (),
        |_, _, _| TraversalAction::DoChildren(()),
        |surface, _, _| ids.push(Id::from_wayland_resource(surface)),
        |_, _, _| true,
    );
    ids
}
