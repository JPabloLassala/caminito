//! `Mapped`: a window Camino manages, as policy sees it.

use std::cell::Cell;

use camino_policy::{OutputId, PolicyWindow, SizeHints, WindowId, WindowStates, WindowType};
use camino_theme::PaletteColors;
use smithay::desktop::Window;
use smithay::desktop::space::SpaceElement;
use smithay::output::Output;
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as DecorationMode;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{FrameExtents, Logical, Point, Rectangle, Size};
use smithay::wayland::compositor::{send_surface_state, with_states};
use smithay::wayland::fractional_scale::with_fractional_scale;
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::shell::xdg::dialog::ToplevelDialogHint;
use smithay::wayland::shell::xdg::{SurfaceCachedState, ToplevelSurface, XdgToplevelSurfaceData};
use smithay::wayland::xdg_toplevel_icon::ToplevelIconCachedState;

use crate::decoration::style::{FrameFacts, frame_style};
use crate::decoration::{FrameContext, FrameInputs, IconSource, WindowFrame};
use crate::handlers::kde_server_side;

use crate::output::{output_geometry, output_id};
use crate::state::Camino;

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
    pid: Option<u32>,
    outputs: Vec<OutputId>,
    /// The output the window overlaps most, whose scale and frame timing it follows
    /// (ADR 0008).
    primary_output: Option<Output>,
    /// The client asked for a state, so it's owed a configure even if nothing changed.
    configure_owed: Cell<bool>,
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
    /// `pid` comes from the client's credentials.
    pub fn new(id: WindowId, window: Window, pid: Option<u32>) -> Self {
        Self {
            id,
            window,
            parent: None,
            pid,
            outputs: Vec::new(),
            primary_output: None,
            configure_owed: Cell::new(false),
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

    /// The toplevel's surface.
    pub fn wl_surface(&self) -> Option<WlSurface> {
        self.window.wl_surface().map(|surface| surface.into_owned())
    }

    /// Records the managed window this one is transient for.
    pub fn set_parent(&mut self, parent: Option<WindowId>) {
        self.parent = parent;
    }

    /// Owes the client a configure in reply to a request, whether or not policy changed
    /// anything, as xdg-shell expects.
    pub fn owe_configure(&self) {
        self.configure_owed.set(true);
    }

    /// Sends the client what policy has asked of it since the last configure, if anything,
    /// or a configure it's owed.
    pub fn send_pending_configure(&self) {
        let Some(toplevel) = self.toplevel() else {
            return;
        };
        if !toplevel.is_initial_configure_sent() {
            return;
        }
        if self.configure_owed.take() {
            toplevel.send_configure();
        } else {
            toplevel.send_pending_configure();
        }
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

    fn toplevel(&self) -> Option<&ToplevelSurface> {
        self.window.toplevel()
    }

    pub fn frame(&self) -> &WindowFrame {
        &self.frame
    }

    /// Whether the window shows a frame now: SSD, and not fullscreen.
    pub fn shows_frame(&self) -> bool {
        self.ssd && !self.states.fullscreen
    }

    fn frame_facts(&self, maximized: bool) -> FrameFacts {
        FrameFacts {
            window_type: self.window_type(),
            has_parent: self.parent.is_some(),
            hints: self.size_hints(),
            activated: self.activated,
            maximized,
            fullscreen: self.states.fullscreen,
        }
    }

    /// Reads the decoration mode the client last acked, and works out the frame's extents,
    /// for policy to read next.
    pub fn update_frame_facts(&mut self, frame_context: &FrameContext) {
        let attach = frame_context.attach_modal_dialogs();
        let style = |maximized| frame_style(&self.frame_facts(maximized), attach);
        let extents = |maximized| {
            style(maximized)
                .map(|(frame_type, flags)| frame_context.extents(frame_type, &flags))
                .unwrap_or_default()
        };
        let ssd = self.toplevel().is_some_and(server_side) && style(false).is_some();
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
            icon: self.icon_source(),
            scale: self.frame_scale,
        };
        self.frame.update(frame_context, colors, inputs)
    }

    fn title(&self) -> String {
        let Some(toplevel) = self.toplevel() else {
            return String::new();
        };
        with_states(toplevel.wl_surface(), |states| {
            let data = states.data_map.get::<XdgToplevelSurfaceData>()?;
            data.lock().unwrap().title.clone()
        })
        .unwrap_or_default()
    }

    fn icon_source(&self) -> IconSource {
        let app_id = self.app_id();
        let toplevel_icon = self.toplevel().and_then(|toplevel| {
            with_states(toplevel.wl_surface(), |states| {
                let mut cached = states.cached_state.get::<ToplevelIconCachedState>();
                let icon = cached.current();
                let name = icon.icon_name().map(str::to_owned);
                let buffers = icon.buffers().to_vec();
                (name.is_some() || !buffers.is_empty()).then(|| IconSource::Toplevel {
                    name,
                    buffers,
                    app_id: app_id.clone(),
                })
            })
        });
        toplevel_icon.unwrap_or(IconSource::AppId(app_id))
    }
}

/// Whether the toplevel has server-side decorations: the last mode it acked from
/// xdg-decoration, or KDE server-decoration's.
fn server_side(toplevel: &ToplevelSurface) -> bool {
    let xdg = toplevel.with_committed_state(|state| state.and_then(|s| s.decoration_mode));
    xdg == Some(DecorationMode::ServerSide) || kde_server_side(toplevel.wl_surface())
}

/// Whether the toplevel is asking for server-side decorations in its next configure.
fn pending_server_side(toplevel: &ToplevelSurface) -> bool {
    let xdg = toplevel.with_pending_state(|state| state.decoration_mode);
    xdg == Some(DecorationMode::ServerSide) || kde_server_side(toplevel.wl_surface())
}

/// Marco's window type for a toplevel: a modal dialog from xdg-dialog, a dialog if it has a
/// transient parent, or else normal.
fn window_type(window: &Window, has_parent: bool) -> WindowType {
    let modal = window.toplevel().is_some_and(|toplevel| {
        with_states(toplevel.wl_surface(), |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .is_some_and(|data| data.lock().unwrap().dialog_hint == ToplevelDialogHint::Modal)
        })
    });
    if modal {
        WindowType::ModalDialog
    } else if has_parent {
        WindowType::Dialog
    } else {
        WindowType::Normal
    }
}

impl PolicyWindow for Mapped {
    fn id(&self) -> WindowId {
        self.id
    }

    fn window_type(&self) -> WindowType {
        window_type(&self.window, self.parent.is_some())
    }

    fn transient_parent(&self) -> Option<WindowId> {
        self.parent
    }

    fn size(&self) -> Size<i32, Logical> {
        self.window.geometry().size
    }

    fn size_hints(&self) -> SizeHints {
        let Some(toplevel) = self.toplevel() else {
            return SizeHints::default();
        };
        with_states(toplevel.wl_surface(), |states| {
            let mut cached = states.cached_state.get::<SurfaceCachedState>();
            let current = cached.current();
            SizeHints {
                min: current.min_size,
                max: current.max_size,
            }
        })
    }

    fn requested_position(&self) -> Option<Point<i32, Logical>> {
        // xdg toplevels can't ask for a position; XWayland windows come in M3.
        None
    }

    fn pid(&self) -> Option<u32> {
        self.pid
    }

    fn app_id(&self) -> Option<String> {
        let toplevel = self.toplevel()?;
        with_states(toplevel.wl_surface(), |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()?
                .lock()
                .unwrap()
                .app_id
                .clone()
        })
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
        let Some(toplevel) = self.toplevel() else {
            return;
        };
        // The refresh pass sends what's pending, once per loop iteration.
        toplevel.with_pending_state(|pending| {
            pending.size = Some(geometry.size);
            for (state, on) in [
                (xdg_toplevel::State::Maximized, states.maximized),
                (xdg_toplevel::State::Fullscreen, states.fullscreen),
                (xdg_toplevel::State::Resizing, states.resizing),
            ] {
                if on {
                    pending.states.set(state);
                } else {
                    pending.states.unset(state);
                }
            }
        });
    }

    fn set_activated(&mut self, activated: bool) {
        self.activated = activated;
        self.window.set_activated(activated);
    }

    fn close(&mut self) {
        if let Some(toplevel) = self.toplevel() {
            toplevel.send_close();
        }
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
        let dh = &self.display_handle;
        let pid = window
            .wl_surface()
            .and_then(|surface| dh.get_client(surface.id()).ok())
            .and_then(|client| client.get_credentials(dh).ok())
            .and_then(|credentials| u32::try_from(credentials.pid).ok());
        let mut mapped = Mapped::new(id, window, pid);
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

    /// Sets an unmapped toplevel's pending configure from the states it has asked for: the
    /// size it'll map at if it's maximised or fullscreen, and the bounds of the output it'll
    /// likely map on, both inside the frame it'll have.
    pub fn set_unmapped_pending_state(&self, unmapped: &Unmapped) {
        let Some(toplevel) = unmapped.window.toplevel() else {
            return;
        };
        let output = self.placement_bounds();
        let (extents, maximized_extents) = self.unmapped_extents(unmapped);
        let bounds = output.map(|size| inside(size, extents));
        let fullscreen_size = unmapped.fullscreen.map(|output| {
            output
                .and_then(|id| self.policy.outputs().iter().find(|o| o.id == id))
                .map(|o| o.geometry.size)
                .or(bounds)
        });
        toplevel.with_pending_state(|pending| {
            pending.bounds = bounds;
            pending.size = if let Some(size) = fullscreen_size {
                size
            } else if unmapped.maximized {
                output.map(|size| inside(size, maximized_extents))
            } else {
                None
            };
            for (state, on) in [
                (xdg_toplevel::State::Maximized, unmapped.maximized),
                (
                    xdg_toplevel::State::Fullscreen,
                    unmapped.fullscreen.is_some(),
                ),
            ] {
                if on {
                    pending.states.set(state);
                } else {
                    pending.states.unset(state);
                }
            }
        });
    }

    /// The extents, unmaximised and maximised, of the frame a toplevel that isn't mapped yet
    /// will have, if the decoration mode it's being configured with is server side.
    fn unmapped_extents(
        &self,
        unmapped: &Unmapped,
    ) -> (FrameExtents<i32, Logical>, FrameExtents<i32, Logical>) {
        let window = &unmapped.window;
        if !window.toplevel().is_some_and(pending_server_side) {
            return Default::default();
        }
        let has_parent = self.transient_parent(window).is_some();
        let extents = |maximized| {
            let facts = FrameFacts {
                window_type: window_type(window, has_parent),
                has_parent,
                hints: SizeHints::default(),
                // A new window is focused.
                activated: true,
                maximized,
                fullscreen: false,
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

    /// The managed window a toplevel says it's transient for.
    pub fn transient_parent(&self, window: &Window) -> Option<WindowId> {
        let parent = window.toplevel()?.parent()?;
        self.mapped_for_surface(&parent)
    }

    /// The managed window whose toplevel is `surface`.
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
