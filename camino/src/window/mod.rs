//! `Mapped`: a window Camino manages, as policy sees it.
//!
//! Smithay's `Window` covers xdg toplevels and XWayland windows alike (ADR 0010). What
//! differs between the protocols sits behind [`Adapter`], implemented on each protocol's
//! surface, and `Mapped` reaches it through its one `match` on the underlying surface.

mod lifecycle;
mod x11;
mod xdg;

pub(crate) use lifecycle::PendingWindows;
pub(crate) use lifecycle::StateRequest;
pub(crate) use x11::{configure_request_geometry, request_demands_attention, request_modal};
pub(crate) use xdg::owe_configure;

use camino_policy::{
    FrameState, OutputId, PolicyWindow, SizeHints, WindowId, WindowStates, WindowType,
};
use camino_theme::PaletteColors;
use smithay::desktop::space::SpaceElement;
use smithay::desktop::{Window, WindowSurface};
use smithay::output::Output;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{FrameExtents, Logical, Point, Rectangle, Size};
use smithay::wayland::compositor::send_surface_state;
use smithay::wayland::fractional_scale::with_fractional_scale;
use smithay::wayland::seat::WaylandFocus;
use smithay::xwayland::xwm::X11Window;

use crate::decoration::{FrameContent, FrameContext, IconSource, WindowFacts, WindowFrame};
use crate::input::KeyboardFocus;
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
    /// See [`PolicyWindow::requested_position`], with a frame of `extents`.
    fn requested_position(
        &self,
        extents: FrameExtents<i32, Logical>,
    ) -> Option<Point<i32, Logical>>;
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
    /// Whether it asks for the user's attention.
    fn urgent(&self) -> bool;
    /// Whether it takes keyboard input at all.
    fn takes_input(&self) -> bool;
    /// Whether it asks to be left out of the taskbar and the pager.
    fn skips(&self) -> Skips;

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
    /// What the keyboard focuses to give the window the keyboard.
    fn keyboard_target(&self) -> KeyboardFocus;
}

/// Whether a window is left out of the taskbar and the pager.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Skips {
    pub taskbar: bool,
    pub pager: bool,
}

/// Marco's rules (`recalc_window_features`, window.c:6861-6891): what the client asked,
/// then both for every type but normal windows and dialogs, and the taskbar for a dialog
/// with a transient parent.
fn skips(window_type: WindowType, transient: bool, asked: Skips) -> Skips {
    match window_type {
        WindowType::Normal => asked,
        WindowType::Dialog | WindowType::ModalDialog => Skips {
            taskbar: asked.taskbar || transient,
            pager: asked.pager,
        },
        WindowType::Desktop
        | WindowType::Dock
        | WindowType::Toolbar
        | WindowType::Menu
        | WindowType::Utility
        | WindowType::Splashscreen => Skips {
            taskbar: true,
            pager: true,
        },
    }
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
        WindowSurface::X11(surface) => surface,
    }
}

/// How Camino finds a window from what its protocol names it by: for commits, X
/// callbacks, transient parents and destroy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WindowKey {
    /// An xdg toplevel's surface.
    Wayland(WlSurface),
    /// An X window's id. `X11Surface`s aren't compared, since a dead one never compares
    /// equal.
    X11(X11Window),
}

impl WindowKey {
    pub(crate) fn of(window: &Window) -> Self {
        match window.underlying_surface() {
            WindowSurface::Wayland(toplevel) => WindowKey::Wayland(toplevel.wl_surface().clone()),
            WindowSurface::X11(surface) => WindowKey::X11(surface.window_id()),
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

    /// Reads the window's facts again: the managed window it's transient for, and what its
    /// frame is derived from. Policy reads them next.
    pub fn read_facts(&mut self, parent: Option<WindowId>, frame_context: &FrameContext) {
        self.parent = parent;
        let facts = WindowFacts {
            window_type: self.window_type(),
            has_parent: parent.is_some(),
            hints: self.size_hints(),
            wants_frame: self.adapter().wants_frame(),
        };
        self.frame.set_facts(facts, frame_context);
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
        self.frame.is_ssd() && !self.states.fullscreen
    }

    /// Whether the window is shaded, so only its frame's titlebar shows.
    pub fn is_shaded(&self) -> bool {
        self.states.shaded
    }

    /// Draws the frame's strips again if anything they're drawn from changed, or drops them
    /// if the window shows no frame. They're drawn in the state policy last sent, the one
    /// it laid the window out in (ADR 0012). Returns whether the strips changed.
    pub fn refresh_frame(
        &mut self,
        frame_context: &mut FrameContext,
        colors: &PaletteColors,
    ) -> bool {
        if self.states.fullscreen {
            return self.frame.clear();
        }
        let state = FrameState {
            maximized: self.states.maximized,
            focused: self.activated,
            shaded: self.states.shaded,
        };
        let content = FrameContent {
            size: self.size(),
            title: self.title(),
            icon: self.adapter().icon(),
            scale: self.frame_scale,
        };
        self.frame.refresh(frame_context, colors, state, content)
    }

    pub fn title(&self) -> String {
        self.adapter().title()
    }

    /// Whether the window asks for the user's attention, for M5's foreign-toplevel and
    /// M8's Alt+Tab.
    pub(crate) fn is_urgent(&self) -> bool {
        self.adapter().urgent()
    }

    /// Whether the window is left out of the taskbar and the pager.
    pub(crate) fn skips(&self) -> Skips {
        let adapter = self.adapter();
        skips(
            self.window_type(),
            adapter.parent().is_some(),
            adapter.skips(),
        )
    }

    /// What the keyboard focuses to give the window the keyboard.
    pub(crate) fn keyboard_target(&self) -> KeyboardFocus {
        self.adapter().keyboard_target()
    }

    /// The X window's id, for an XWayland window.
    pub(crate) fn x11_window(&self) -> Option<X11Window> {
        self.window.x11_surface().map(|surface| surface.window_id())
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

    fn requested_position(
        &self,
        extents: FrameExtents<i32, Logical>,
    ) -> Option<Point<i32, Logical>> {
        self.adapter().requested_position(extents)
    }

    fn takes_input(&self) -> bool {
        self.adapter().takes_input()
    }

    fn pid(&self) -> Option<u32> {
        self.adapter().pid()
    }

    fn app_id(&self) -> Option<String> {
        self.adapter().app_id()
    }

    fn is_ssd(&self) -> bool {
        self.frame.is_ssd()
    }

    fn frame_extents(&self, state: FrameState) -> FrameExtents<i32, Logical> {
        self.frame.extents(state)
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
    /// The managed window a window says it's transient for.
    pub fn transient_parent(&self, window: &Window) -> Option<WindowId> {
        self.mapped_for_key(&adapter(window).parent()?)
    }

    /// The managed window `key` names.
    pub(crate) fn mapped_for_key(&self, key: &WindowKey) -> Option<WindowId> {
        self.policy
            .windows()
            .find(|mapped| WindowKey::of(mapped.window()) == *key)
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
