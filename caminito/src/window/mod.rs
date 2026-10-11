//! `Mapped`: a window Caminito manages, as policy sees it.
//!
//! Smithay's `Window` covers xdg toplevels and XWayland windows alike (ADR 0010). What
//! differs between the protocols sits behind [`Adapter`], implemented on each protocol's
//! surface, and `Mapped` reaches it through its one `match` on the underlying surface.

mod lifecycle;
mod x11;
mod xdg;

pub(crate) use lifecycle::PendingWindows;
pub(crate) use lifecycle::StateRequest;
pub(crate) use x11::{
    STICKY, Side, Strut, configure_request_geometry, marco_type, request_demands_attention,
    request_modal, request_skips, struts, takes_pointer_at, with_x_facts,
};
pub(crate) use xdg::owe_configure;

use std::cell::Cell;
use std::sync::Mutex;

use caminito_policy::{
    FrameState, Functions, OutputId, PolicyWindow, SizeHints, WindowId, WindowStates, WindowType,
};
use caminito_theme::PaletteColors;
use smithay::backend::renderer::element::{
    PrimaryScanoutOutput, RenderElementState, RenderElementStates,
};
use smithay::desktop::space::SpaceElement;
use smithay::desktop::utils::{
    surface_primary_scanout_output, update_surface_primary_scanout_output,
};
use smithay::desktop::{Window, WindowSurface};
use smithay::output::Output;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{FrameExtents, Logical, Point, Rectangle, Serial, Size};
use smithay::wayland::compositor::{SurfaceData, send_surface_state, with_states};
use smithay::wayland::fractional_scale::with_fractional_scale;
use smithay::wayland::seat::WaylandFocus;
use smithay::xwayland::xwm::X11Window;

use crate::decoration::{FrameContent, FrameContext, IconSource, WindowFacts, WindowFrame};
use crate::input::KeyboardFocus;
use crate::output::{output_geometry, output_id};
use crate::state::Caminito;

/// What a window is to Caminito, whichever protocol it speaks: its facts, and the requests
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
    /// See [`PolicyWindow::requested_workspace`].
    fn requested_workspace(&self) -> Option<usize>;
    /// Whether the window wants Caminito's frame.
    fn wants_frame(&self) -> bool;
    /// Whether the window will want Caminito's frame once it's had the configure it's being
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
    /// Sends what's been asked since the last flush, to a client that draws the window at
    /// `size`. The refresh pass flushes every window, so each protocol sends once per loop
    /// iteration.
    fn flush(&self, size: Size<i32, Logical>);
    fn set_activated(&self, activated: bool);
    /// Asks the client to close the window.
    fn close(&self);
    /// Pings the client, with `serial` to tell its answer by. Whether the client owes an
    /// answer now: to this ping, or to one before it that it hasn't answered.
    fn ping(&self, serial: Serial) -> bool;
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
    /// The size it'll map at, if Caminito sets it.
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

/// How Caminito finds a window from what its protocol names it by: for commits, X
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

/// Smithay's `Window`, which covers xdg toplevels and XWayland windows alike, plus Caminito's
/// per-window data.
#[derive(Debug)]
pub struct Mapped {
    id: WindowId,
    window: Window,
    parent: Option<WindowId>,
    outputs: Vec<OutputId>,
    /// The output whose frame timing the window follows, and whose scale its surfaces
    /// prefer until they have main outputs of their own (13): its surface's main
    /// output once it's drawn, and before that its parent's, or the pointer's. It stays
    /// while the window isn't drawn anywhere.
    main_output: Option<Output>,
    activated: bool,
    /// The states policy last asked for.
    states: WindowStates,
    /// The functions policy last sent, which the frame shows (177).
    functions: Functions,
    /// The largest rounded-up scale of the outputs the window is on, which its strips are
    /// drawn at (13).
    frame_scale: i32,
    frame: WindowFrame,
    /// Where a foreign-toplevel client shows the window, as a rectangle on one of its
    /// surfaces (65): the target of the minimise animation (172).
    minimize_rectangle: Option<(WlSurface, Rectangle<i32, Logical>)>,
    /// Whether policy has asked the window to close since the refresh pass last pinged its
    /// client (168).
    close_asked: Cell<bool>,
}

/// Which of two outputs a surface drawn on both has as its main output: the one it has,
/// until the other shows at least twice the pixels. It's the visible-area half of Smithay's
/// `default_primary_scanout_output_compare` (`renderer/element/mod.rs:428-460`), without
/// the half that hands the surface to the output with the higher refresh rate however
/// little of it is there: on outputs that draw in turn, the slower one took the surface
/// back with twice the pixels at its next frame, and a window on both was sent their
/// scales in turn, frame after frame (92).
fn main_output_compare<'a>(
    current_output: &'a Output,
    current_state: &RenderElementState,
    next_output: &'a Output,
    next_state: &RenderElementState,
) -> &'a Output {
    if next_state.visible_area >= current_state.visible_area * 2 {
        next_output
    } else {
        current_output
    }
}

impl Mapped {
    pub fn new(id: WindowId, window: Window) -> Self {
        Self {
            id,
            window,
            parent: None,
            outputs: Vec::new(),
            main_output: None,
            activated: false,
            states: WindowStates::default(),
            functions: Functions::default(),
            frame_scale: 1,
            frame: WindowFrame::default(),
            minimize_rectangle: None,
            close_asked: Cell::new(false),
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
    /// frame is derived from besides what policy sends. Policy reads them next.
    pub fn read_facts(&mut self, parent: Option<WindowId>, frame_context: &FrameContext) {
        self.parent = parent;
        let facts = WindowFacts {
            window_type: self.window_type(),
            wants_frame: self.adapter().wants_frame(),
        };
        self.frame.set_facts(facts, frame_context);
    }

    /// Sends the client what policy has asked of it since the last flush, if anything.
    pub fn flush(&self) {
        self.adapter().flush(self.window.geometry().size);
    }

    /// The outputs policy last put the window on.
    pub fn outputs(&self) -> &[OutputId] {
        &self.outputs
    }

    pub fn main_output(&self) -> Option<&Output> {
        self.main_output.as_ref()
    }

    /// Sends the surfaces output enter and leave for the outputs policy put the window on,
    /// with its geometry at `geometry`, and their preferred scales. A window not drawn yet
    /// takes `fallback` as its main output.
    pub fn update_outputs(
        &mut self,
        geometry: Rectangle<i32, Logical>,
        outputs: &[Output],
        fallback: Option<Output>,
    ) {
        // Where the surface's origin is, globally.
        let origin = geometry.loc - self.window.geometry().loc;
        let mut bbox = self.window.bbox_with_popups();
        bbox.loc += origin;

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
            overlap.loc -= origin;
            self.window.output_enter(output, overlap);
        }

        self.frame_scale = outputs
            .iter()
            .filter(|output| self.outputs.contains(&output_id(output)))
            .map(|output| output.current_scale().fractional_scale().ceil() as i32)
            .max()
            .unwrap_or(1);
        if self.main_output.is_none() {
            self.main_output = fallback;
        }
        self.send_preferred_scales();
    }

    /// Picks each surface's main output from the frames just drawn on `rendered`, in
    /// the order they were drawn: another output takes over once it shows at least twice
    /// the pixels (13). Run after rendering, as cosmic-comp does (`state.rs:957-1000`).
    ///
    /// Only an output the window is on can be its main output. The Window switcher draws a
    /// thumbnail of the window on its popup's output with the window's own surfaces (108),
    /// which mustn't count as the window there: on an output the window isn't on, its
    /// surfaces are taken as not drawn at all.
    pub(crate) fn update_main_outputs(&mut self, rendered: &[(Output, RenderElementStates)]) {
        let outputs = &self.outputs;
        let nothing_drawn = RenderElementStates {
            states: Default::default(),
        };
        self.window.with_surfaces(|surface, data| {
            let update = || {
                for (output, states) in rendered {
                    let on_it = outputs.contains(&output_id(output));
                    let states = if on_it { states } else { &nothing_drawn };
                    update_surface_primary_scanout_output(
                        surface,
                        output,
                        data,
                        None,
                        states,
                        main_output_compare,
                    );
                }
            };
            update();
            // Smithay drops the main output of a surface that's no longer on it, even
            // when it was drawn on an output earlier in the pass; a second pass lets that
            // output take over.
            if surface_primary_scanout_output(surface, data).is_none() {
                update();
            }
        });
        let main = self.wl_surface().and_then(|surface| {
            with_states(&surface, |data| {
                surface_primary_scanout_output(&surface, data)
            })
        });
        if main.is_some() {
            self.main_output = main;
        }
        self.send_preferred_scales();
    }

    /// Sends each surface its main output's scale, fractional and rounded up, and
    /// transform. Smithay sends only what changed.
    fn send_preferred_scales(&self) {
        let fallback = self.main_output.as_ref();
        self.window.with_surfaces(|surface, data| {
            let main = surface_primary_scanout_output(surface, data);
            if let Some(output) = main.as_ref().or(fallback) {
                send_preferred_scale(surface, data, output);
            }
        });
    }

    /// Sends the surfaces output leave for an output that's gone, and forgets it as their
    /// main output.
    pub(crate) fn leave_output(&mut self, output: &Output) {
        self.window.output_leave(output);
        self.window.with_surfaces(|surface, data| {
            if surface_primary_scanout_output(surface, data).as_ref() == Some(output) {
                // Smithay would keep comparing other outputs with it while the output
                // lives on elsewhere.
                *data
                    .data_map
                    .get::<Mutex<PrimaryScanoutOutput>>()
                    .expect("Smithay keeps a surface's main output in its data map")
                    .lock()
                    .unwrap() = PrimaryScanoutOutput::default();
            }
        });
        if self.main_output.as_ref() == Some(output) {
            self.main_output = None;
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
    /// it laid the window out in (ADR 0012), with the functions it last sent (177), in the
    /// other focus state while the visual bell
    /// flashes it (169), with the buttons of a window kept `above` and `sticky` or not
    /// (155). Returns whether the strips changed.
    pub fn refresh_frame(
        &mut self,
        frame_context: &mut FrameContext,
        colors: &PaletteColors,
        flashing: bool,
        above: bool,
        sticky: bool,
    ) -> bool {
        if self.states.fullscreen {
            return self.frame.clear();
        }
        let state = self.states.frame_state(self.activated, self.functions);
        let content = FrameContent {
            size: self.size(),
            title: self.title(),
            icon: self.adapter().icon(),
            scale: self.frame_scale,
            flashing,
            above,
            stuck: sticky,
        };
        self.frame
            .refresh(frame_context, colors, state, self.functions, content)
    }

    pub fn title(&self) -> String {
        self.adapter().title()
    }

    /// Whether policy has asked the window to close since this was last asked (168).
    pub(crate) fn take_close_asked(&self) -> bool {
        self.close_asked.take()
    }

    /// Pings the window's client, as [`Adapter::ping`] says.
    pub(crate) fn ping(&self, serial: Serial) -> bool {
        self.adapter().ping(serial)
    }

    /// Where the window's icon comes from.
    pub(crate) fn icon_source(&self) -> IconSource {
        self.adapter().icon()
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

    /// Where a foreign-toplevel client last said it shows the window, if its surface is
    /// still there.
    pub(crate) fn minimize_rectangle(&self) -> Option<(&WlSurface, Rectangle<i32, Logical>)> {
        let (surface, rectangle) = self.minimize_rectangle.as_ref()?;
        surface.is_alive().then_some((surface, *rectangle))
    }

    pub(crate) fn set_minimize_rectangle(
        &mut self,
        rectangle: Option<(WlSurface, Rectangle<i32, Logical>)>,
    ) {
        self.minimize_rectangle = rectangle;
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

    fn requested_workspace(&self) -> Option<usize> {
        self.adapter().requested_workspace()
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

    /// The client's own ask for attention. `Policy::is_urgent` adds a refused focus to it,
    /// and that is what the Window switcher marks (108) and the debug dump lists (165).
    fn urgent(&self) -> bool {
        self.adapter().urgent()
    }

    fn skips_taskbar(&self) -> bool {
        self.skips().taskbar
    }

    fn is_ssd(&self) -> bool {
        self.frame.is_ssd()
    }

    fn frame_extents(&self, state: FrameState) -> FrameExtents<i32, Logical> {
        self.frame.extents(state)
    }

    fn set_functions(&mut self, functions: Functions) {
        self.functions = functions;
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
        self.close_asked.set(true);
    }

    fn set_outputs(&mut self, outputs: &[OutputId]) {
        self.outputs = outputs.to_vec();
    }
}

/// Sends `surface` the scale and transform of `output` to draw at: its fractional scale,
/// and its scale rounded up as `preferred_buffer_scale`.
pub(crate) fn send_preferred_scale(surface: &WlSurface, data: &SurfaceData, output: &Output) {
    let scale = output.current_scale();
    send_surface_state(
        surface,
        data,
        scale.integer_scale(),
        output.current_transform(),
    );
    with_fractional_scale(data, |fractional| {
        fractional.set_preferred_scale(scale.fractional_scale());
    });
}

impl Caminito {
    /// The main output of a window not drawn yet (13): its parent's, or else the one
    /// under the pointer.
    pub(crate) fn main_output_until_drawn(&self, parent: Option<WindowId>) -> Option<Output> {
        let parents = parent
            .and_then(|parent| self.policy.window(parent))
            .and_then(|parent| parent.main_output().cloned());
        parents.or_else(|| {
            let pointer = self.seat.get_pointer().unwrap().current_location();
            self.outputs
                .iter()
                .find(|output| output_geometry(output).to_f64().contains(pointer))
                .cloned()
        })
    }

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
