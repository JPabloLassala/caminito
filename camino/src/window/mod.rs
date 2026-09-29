//! `Mapped`: a window Camino manages, as policy sees it.
//!
//! Smithay's `Window` covers xdg toplevels and XWayland windows alike (ADR 0010). What
//! differs between the protocols sits behind [`Adapter`], implemented on each protocol's
//! surface, and `Mapped` reaches it through its one `match` on the underlying surface.

mod xdg;

pub(crate) use xdg::owe_configure;

use camino_policy::{OutputId, PolicyWindow, SizeHints, WindowId, WindowStates, WindowType};
use camino_theme::PaletteColors;
use smithay::desktop::space::SpaceElement;
use smithay::desktop::{Window, WindowSurface};
use smithay::output::Output;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{FrameExtents, Logical, Point, Rectangle, Size};
use smithay::wayland::compositor::send_surface_state;
use smithay::wayland::fractional_scale::with_fractional_scale;
use smithay::wayland::seat::WaylandFocus;

use crate::decoration::style::{FrameFacts, frame_style};
use crate::decoration::{FrameContext, FrameInputs, IconSource, WindowFrame};
use crate::output::{output_geometry, output_id};
use crate::state::Camino;

/// What a window is to Camino, whichever protocol it speaks: its facts, and the requests
/// that carry out policy's decisions. Every adapter answers every fact, with a default
/// where its protocol has none.
trait Adapter {
    // Facts.

    fn title(&self) -> String;
    fn app_id(&self) -> Option<String>;
    /// Marco's type, given whether the window is transient for a managed one.
    fn window_type(&self, has_parent: bool) -> WindowType;
    fn size_hints(&self) -> SizeHints;
    /// See [`PolicyWindow::requested_position`].
    fn requested_position(&self) -> Option<Point<i32, Logical>>;
    fn pid(&self) -> Option<u32>;
    /// Whether the window wants Camino's frame.
    fn wants_frame(&self) -> bool;
    /// Whether the window will want Camino's frame once it's had the configure it's being
    /// sent.
    fn wants_frame_pending(&self) -> bool {
        self.wants_frame()
    }
    fn icon(&self) -> IconSource;
    /// The window it says it's transient for.
    fn parent(&self) -> Option<WindowKey>;

    // Requests.

    /// Asks for `geometry` with `states`; [`flush`](Self::flush) sends it.
    fn set_geometry(&self, geometry: Rectangle<i32, Logical>, states: WindowStates);
    /// Sends what's been asked since the last flush. The refresh pass flushes every window,
    /// so each protocol sends once per loop iteration.
    fn flush(&self);
    fn set_activated(&self, activated: bool);
    /// Asks the client to close the window.
    fn close(&self);
    /// Configures a pending window before it maps.
    fn configure_pending(&self, configure: &PreMapConfigure);
}

/// A pending window's configure before it maps, from the states it has asked for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PreMapConfigure {
    /// Where the window will likely map, inside the frame it'll have.
    bounds: Option<Size<i32, Logical>>,
    /// The size it'll map at, if Camino sets it.
    size: Option<Size<i32, Logical>>,
    maximized: bool,
    fullscreen: bool,
}

/// The adapter for a window's protocol.
fn adapter(window: &Window) -> &dyn Adapter {
    match window.underlying_surface() {
        WindowSurface::Wayland(toplevel) => toplevel,
        WindowSurface::X11(_) => unreachable!("XWayland windows aren't managed until 35"),
    }
}

/// How Camino finds a window from what its protocol names it by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WindowKey {
    /// An xdg toplevel's surface.
    Wayland(WlSurface),
}

impl WindowKey {
    fn of(window: &Window) -> Option<Self> {
        match window.underlying_surface() {
            WindowSurface::Wayland(toplevel) => {
                Some(WindowKey::Wayland(toplevel.wl_surface().clone()))
            }
            WindowSurface::X11(_) => None,
        }
    }
}

/// A toplevel the client has made but not yet mapped with a buffer, and the states it asked
/// for meanwhile, which it maps with.
#[derive(Debug)]
pub struct Unmapped {
    pub window: Window,
    pub maximized: bool,
    /// Fullscreen was asked for, on this output if the client named one.
    pub fullscreen: Option<Option<OutputId>>,
}

impl Unmapped {
    pub fn new(window: Window) -> Self {
        Self {
            window,
            maximized: false,
            fullscreen: None,
        }
    }
}

/// Smithay's `Window`, which covers xdg toplevels and XWayland windows alike, plus Camino's
/// per-window data.
#[derive(Debug)]
pub struct Mapped {
    id: WindowId,
    window: Window,
    parent: Option<WindowId>,
    outputs: Vec<OutputId>,
    /// The output the window overlaps most, whose scale and frame timing it follows
    /// (ADR 0008).
    primary_output: Option<Output>,
    activated: bool,
    /// The states policy last asked for.
    states: WindowStates,
    /// Whether Camino draws a frame, and its extents unmaximised and maximised, as policy
    /// last read them.
    ssd: bool,
    extents: FrameExtents<i32, Logical>,
    maximized_extents: FrameExtents<i32, Logical>,
    /// The largest rounded-up scale of the outputs the window is on, which its strips are
    /// drawn at (13).
    frame_scale: i32,
    frame: WindowFrame,
}

impl Mapped {
    pub fn new(id: WindowId, window: Window) -> Self {
        Self {
            id,
            window,
            parent: None,
            outputs: Vec::new(),
            primary_output: None,
            activated: false,
            states: WindowStates::default(),
            ssd: false,
            extents: FrameExtents::default(),
            maximized_extents: FrameExtents::default(),
            frame_scale: 1,
            frame: WindowFrame::default(),
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    fn adapter(&self) -> &dyn Adapter {
        adapter(&self.window)
    }

    /// The window's surface.
    pub fn wl_surface(&self) -> Option<WlSurface> {
        self.window.wl_surface().map(|surface| surface.into_owned())
    }

    /// Records the managed window this one is transient for.
    pub fn set_parent(&mut self, parent: Option<WindowId>) {
        self.parent = parent;
    }

    /// Sends the client what policy has asked of it since the last flush, if anything.
    pub fn flush(&self) {
        self.adapter().flush();
    }

    /// The outputs policy last put the window on.
    pub fn outputs(&self) -> &[OutputId] {
        &self.outputs
    }

    pub fn primary_output(&self) -> Option<&Output> {
        self.primary_output.as_ref()
    }

    /// Sends the surfaces output enter and leave for the outputs policy put the window on,
    /// with its geometry at `geometry`, and the primary output's scale and transform.
    pub fn update_outputs(&mut self, geometry: Rectangle<i32, Logical>, outputs: &[Output]) {
        // Where the surface's origin is, globally.
        let origin = geometry.loc - self.window.geometry().loc;
        let mut bbox = self.window.bbox_with_popups();
        bbox.loc += origin;

        let mut primary: Option<(&Output, i64)> = None;
        for output in outputs {
            let overlap = self
                .outputs
                .contains(&output_id(output))
                .then(|| output_geometry(output).intersection(bbox))
                .flatten();
            let Some(mut overlap) = overlap else {
                self.window.output_leave(output);
                continue;
            };
            let area = i64::from(overlap.size.w) * i64::from(overlap.size.h);
            if primary.is_none_or(|(_, most)| area > most) {
                primary = Some((output, area));
            }
            overlap.loc -= origin;
            self.window.output_enter(output, overlap);
        }

        self.primary_output = primary.map(|(output, _)| output.clone());
        self.frame_scale = outputs
            .iter()
            .filter(|output| self.outputs.contains(&output_id(output)))
            .map(|output| output.current_scale().fractional_scale().ceil() as i32)
            .max()
            .unwrap_or(1);
        if let Some(output) = &self.primary_output {
            let scale = output.current_scale();
            let transform = output.current_transform();
            self.window.with_surfaces(|surface, states| {
                send_surface_state(surface, states, scale.integer_scale(), transform);
                with_fractional_scale(states, |fractional| {
                    fractional.set_preferred_scale(scale.fractional_scale());
                });
            });
        }
    }

    pub fn frame(&self) -> &WindowFrame {
        &self.frame
    }

    pub fn frame_mut(&mut self) -> &mut WindowFrame {
        &mut self.frame
    }

    /// Whether the window shows a frame now: SSD, and not fullscreen.
    pub fn shows_frame(&self) -> bool {
        self.ssd && !self.states.fullscreen
    }

    /// Whether the window is shaded, so only its frame's titlebar shows.
    pub fn is_shaded(&self) -> bool {
        self.states.shaded
    }

    fn frame_facts(&self, maximized: bool) -> FrameFacts {
        FrameFacts {
            window_type: self.window_type(),
            has_parent: self.parent.is_some(),
            hints: self.size_hints(),
            activated: self.activated,
            maximized,
            fullscreen: self.states.fullscreen,
            shaded: self.states.shaded,
        }
    }

    /// Asks the adapter whether the window wants a frame, and works out the frame's
    /// extents, for policy to read next.
    pub fn update_frame_facts(&mut self, frame_context: &FrameContext) {
        let attach = frame_context.attach_modal_dialogs();
        let style = |maximized| frame_style(&self.frame_facts(maximized), attach);
        let extents = |maximized| {
            style(maximized)
                .map(|(frame_type, flags)| frame_context.extents(frame_type, &flags))
                .unwrap_or_default()
        };
        let ssd = self.adapter().wants_frame() && style(false).is_some();
        (self.extents, self.maximized_extents) = (extents(false), extents(true));
        self.ssd = ssd;
    }

    /// Draws the frame's strips again if anything they're drawn from changed, or drops them
    /// if the window shows no frame. Returns whether the strips changed.
    pub fn refresh_frame(
        &mut self,
        frame_context: &mut FrameContext,
        colors: &PaletteColors,
    ) -> bool {
        let style = frame_style(
            &self.frame_facts(self.states.maximized),
            frame_context.attach_modal_dialogs(),
        );
        let Some((frame_type, flags)) = style.filter(|_| self.shows_frame()) else {
            return self.frame.clear();
        };
        let inputs = FrameInputs {
            frame_type,
            flags,
            size: self.size(),
            title: self.title(),
            icon: self.adapter().icon(),
            scale: self.frame_scale,
        };
        self.frame.update(frame_context, colors, inputs)
    }

    pub fn title(&self) -> String {
        self.adapter().title()
    }
}

impl PolicyWindow for Mapped {
    fn id(&self) -> WindowId {
        self.id
    }

    fn window_type(&self) -> WindowType {
        self.adapter().window_type(self.parent.is_some())
    }

    fn transient_parent(&self) -> Option<WindowId> {
        self.parent
    }

    fn size(&self) -> Size<i32, Logical> {
        self.window.geometry().size
    }

    fn size_hints(&self) -> SizeHints {
        self.adapter().size_hints()
    }

    fn requested_position(&self) -> Option<Point<i32, Logical>> {
        self.adapter().requested_position()
    }

    fn pid(&self) -> Option<u32> {
        self.adapter().pid()
    }

    fn app_id(&self) -> Option<String> {
        self.adapter().app_id()
    }

    fn is_ssd(&self) -> bool {
        self.ssd
    }

    fn frame_extents(&self, maximized: bool) -> FrameExtents<i32, Logical> {
        if maximized {
            self.maximized_extents
        } else {
            self.extents
        }
    }

    fn set_geometry(&mut self, geometry: Rectangle<i32, Logical>, states: WindowStates) {
        self.states = states;
        self.adapter().set_geometry(geometry, states);
    }

    fn set_activated(&mut self, activated: bool) {
        self.activated = activated;
        self.adapter().set_activated(activated);
    }

    fn close(&mut self) {
        self.adapter().close();
    }

    fn set_outputs(&mut self, outputs: &[OutputId]) {
        self.outputs = outputs.to_vec();
    }
}

impl Camino {
    /// Starts managing a window the client has mapped, and returns policy's id for it.
    pub fn map_window(&mut self, unmapped: Unmapped) -> WindowId {
        let id = WindowId(self.next_window_id);
        self.next_window_id += 1;

        let Unmapped {
            window,
            maximized,
            fullscreen,
        } = unmapped;
        let mut mapped = Mapped::new(id, window);
        mapped.set_parent(self.transient_parent(mapped.window()));
        mapped.update_frame_facts(&self.frame_context);

        let now = self.now();
        self.policy.map_window(mapped, now);
        if let Some(output) = fullscreen {
            self.policy.set_fullscreen(id, true, output);
        } else if maximized {
            self.policy.set_maximized(id, true);
        }
        self.queue_redraw_all();
        id
    }

    /// Configures an unmapped window from the states it has asked for: the size it'll map
    /// at if it's maximised or fullscreen, and the bounds of the output it'll likely map on,
    /// both inside the frame it'll have.
    pub fn configure_unmapped(&self, unmapped: &Unmapped) {
        let output = self.placement_bounds();
        let (extents, maximized_extents) = self.unmapped_extents(unmapped);
        let bounds = output.map(|size| inside(size, extents));
        let fullscreen_size = unmapped.fullscreen.map(|output| {
            output
                .and_then(|id| self.policy.outputs().iter().find(|o| o.id == id))
                .map(|o| o.geometry.size)
                .or(bounds)
        });
        let size = if let Some(size) = fullscreen_size {
            size
        } else if unmapped.maximized {
            output.map(|size| inside(size, maximized_extents))
        } else {
            None
        };
        adapter(&unmapped.window).configure_pending(&PreMapConfigure {
            bounds,
            size,
            maximized: unmapped.maximized,
            fullscreen: unmapped.fullscreen.is_some(),
        });
    }

    /// The extents, unmaximised and maximised, of the frame a window that isn't mapped yet
    /// will have, if it'll want a frame once configured.
    fn unmapped_extents(
        &self,
        unmapped: &Unmapped,
    ) -> (FrameExtents<i32, Logical>, FrameExtents<i32, Logical>) {
        let window = &unmapped.window;
        let adapter = adapter(window);
        if !adapter.wants_frame_pending() {
            return Default::default();
        }
        let has_parent = self.transient_parent(window).is_some();
        let extents = |maximized| {
            let facts = FrameFacts {
                window_type: adapter.window_type(has_parent),
                has_parent,
                hints: SizeHints::default(),
                // A new window is focused.
                activated: true,
                maximized,
                fullscreen: false,
                shaded: false,
            };
            frame_style(&facts, self.frame_context.attach_modal_dialogs())
                .map(|(frame_type, flags)| self.frame_context.extents(frame_type, &flags))
                .unwrap_or_default()
        };
        (extents(false), extents(true))
    }

    /// Stops managing a window that the client unmapped or destroyed, and hands it back.
    pub fn unmap_window(&mut self, id: WindowId) -> Option<Mapped> {
        let mapped = self.policy.unmap_window(id)?;
        for output in &self.outputs {
            mapped.window().output_leave(output);
        }
        self.queue_redraw_all();
        Some(mapped)
    }

    /// The managed window a window says it's transient for.
    pub fn transient_parent(&self, window: &Window) -> Option<WindowId> {
        self.mapped_for_key(&adapter(window).parent()?)
    }

    /// The managed window `key` names.
    fn mapped_for_key(&self, key: &WindowKey) -> Option<WindowId> {
        self.policy
            .windows()
            .find(|mapped| WindowKey::of(mapped.window()).as_ref() == Some(key))
            .map(|mapped| mapped.id())
    }

    /// The managed window whose surface is `surface`.
    pub fn mapped_for_surface(&self, surface: &WlSurface) -> Option<WindowId> {
        self.policy
            .windows()
            .find(|mapped| mapped.wl_surface().as_ref() == Some(surface))
            .map(|mapped| mapped.id())
    }
}

/// The size inside a frame of `extents` that fills `size`.
fn inside(size: Size<i32, Logical>, extents: FrameExtents<i32, Logical>) -> Size<i32, Logical> {
    Size::new(
        (size.w - extents.left - extents.right).max(1),
        (size.h - extents.top - extents.bottom).max(1),
    )
}
