//! `State` and `Camino`: everything the event loop runs on (15).

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::sync::Arc;
use std::time::Duration;

use camino_policy::{Config, OutputId, Policy, SeatId};
use smithay::desktop::PopupManager;
use smithay::desktop::utils::OutputPresentationFeedback;
use smithay::backend::input::Keycode;
use smithay::input::keyboard::XkbConfig;
use smithay::input::{Seat, SeatState};
use smithay::output::Output;
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{EventLoop, Interest, LoopHandle, LoopSignal, Mode, PostAction};
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_protocols::wp::presentation_time::server::wp_presentation_feedback;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Display, DisplayHandle, Resource};
use smithay::utils::{Clock, Logical, Monotonic, Point, Size};
use smithay::wayland::alpha_modifier::AlphaModifierState;
use smithay::wayland::commit_timing::{CommitTimerBarrierStateUserData, CommitTimingManagerState};
use smithay::wayland::compositor::{
    CompositorClientState, CompositorHandler, CompositorState, SurfaceData,
};
use smithay::wayland::content_type::ContentTypeState;
use smithay::wayland::cursor_shape::CursorShapeManagerState;
use smithay::wayland::fifo::{FifoBarrierCachedState, FifoManagerState};
use smithay::wayland::fractional_scale::FractionalScaleManagerState;
use smithay::wayland::input_method::InputMethodManagerState;
use smithay::wayland::keyboard_shortcuts_inhibit::KeyboardShortcutsInhibitState;
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::pointer_constraints::PointerConstraintsState;
use smithay::wayland::pointer_gestures::PointerGesturesState;
use smithay::wayland::presentation::{PresentationState, Refresh};
use smithay::wayland::relative_pointer::RelativePointerManagerState;
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::selection::primary_selection::PrimarySelectionState;
use smithay::reexports::wayland_protocols_misc::server_decoration::server::org_kde_kwin_server_decoration_manager::Mode as KdeDefaultMode;
use smithay::wayland::shell::kde::decoration::KdeDecorationState;
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
use smithay::wayland::shell::xdg::dialog::XdgDialogState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::single_pixel_buffer::SinglePixelBufferState;
use smithay::wayland::socket::ListeningSocketSource;
use smithay::wayland::tablet_manager::TabletManagerState;
use smithay::wayland::text_input::TextInputManagerState;
use smithay::wayland::viewporter::ViewporterState;
use smithay::wayland::xdg_activation::XdgActivationState;
use smithay::wayland::xdg_foreign::XdgForeignState;
use smithay::wayland::xdg_system_bell::XdgSystemBellState;
use smithay::wayland::xdg_toplevel_icon::XdgToplevelIconManager;

use crate::backend::Backend;
use crate::window::{Mapped, Unmapped};

/// How long a window that isn't redrawn waits for a frame callback anyway.
const FRAME_CALLBACK_THROTTLE: Option<Duration> = Some(Duration::from_secs(1));

/// Policy's id for Camino's one seat.
pub const SEAT: SeatId = SeatId(0);

/// The whole compositor: the backend, and everything that doesn't depend on it.
pub struct State {
    pub backend: Backend,
    pub camino: Camino,
}

/// Camino's backend-independent state.
pub struct Camino {
    pub display_handle: DisplayHandle,
    pub loop_handle: LoopHandle<'static, State>,
    pub loop_signal: LoopSignal,

    /// Monotonic time, which policy's `now` and frame callbacks are measured in.
    pub clock: Clock<Monotonic>,

    pub compositor_state: CompositorState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<State>,
    pub seat: Seat<State>,
    pub data_device_state: DataDeviceState,
    pub primary_selection_state: PrimarySelectionState,

    // Shell (06): xdg-shell and the protocols that extend its toplevels.
    pub xdg_shell_state: XdgShellState,
    pub xdg_activation_state: XdgActivationState,
    pub xdg_foreign_state: XdgForeignState,
    pub xdg_dialog_state: XdgDialogState,
    pub xdg_toplevel_icon_manager: XdgToplevelIconManager,
    pub xdg_system_bell_state: XdgSystemBellState,
    /// xdg-decoration and KDE server-decoration answer client side until M2 (17).
    pub xdg_decoration_state: XdgDecorationState,
    pub kde_decoration_state: KdeDecorationState,
    pub popups: PopupManager,

    // Input (06).
    pub pointer_constraints_state: PointerConstraintsState,
    pub relative_pointer_state: RelativePointerManagerState,
    pub pointer_gestures_state: PointerGesturesState,
    pub tablet_manager_state: TabletManagerState,
    pub keyboard_shortcuts_inhibit_state: KeyboardShortcutsInhibitState,
    pub text_input_state: TextInputManagerState,
    pub input_method_state: InputMethodManagerState,
    pub cursor_shape_state: CursorShapeManagerState,

    // Rendering (06). drm-syncobj comes with M4's DRM.
    pub viewporter_state: ViewporterState,
    pub fractional_scale_state: FractionalScaleManagerState,
    pub presentation_state: PresentationState,
    pub single_pixel_buffer_state: SinglePixelBufferState,
    pub content_type_state: ContentTypeState,
    pub alpha_modifier_state: AlphaModifierState,
    pub fifo_manager_state: FifoManagerState,
    pub commit_timing_state: CommitTimingManagerState,

    /// The surface pointer focus was last given, and its origin then.
    pub pointer_focus: Option<(WlSurface, Point<f64, Logical>)>,
    /// Keys whose press ran a binding, so their release is swallowed too.
    pub suppressed_keys: HashSet<Keycode>,

    /// Every output, in the order they were added.
    pub outputs: Vec<Output>,
    pub(crate) next_output_id: u64,
    /// Outputs to render in the next refresh pass.
    pub(crate) redraws: HashSet<OutputId>,

    pub policy: Policy<Mapped>,
    /// Toplevels made but not yet mapped.
    pub unmapped: Vec<Unmapped>,
    pub(crate) next_window_id: u64,
}

/// Per-client data.
#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

impl State {
    /// Opens a winit window as the one output.
    pub fn new_winit(
        event_loop: &EventLoop<'static, State>,
        display: Display<State>,
    ) -> Result<Self, smithay::backend::winit::Error> {
        let mut camino = Camino::new(event_loop, display);
        let backend = Backend::Winit(Box::new(crate::backend::winit::Winit::new(&mut camino)?));
        Ok(Self { backend, camino })
    }

    /// Starts with no outputs; tests add them with `add_headless_output`.
    #[cfg(test)]
    pub fn new_headless(event_loop: &EventLoop<'static, State>, display: Display<State>) -> Self {
        let camino = Camino::new(event_loop, display);
        let backend = Backend::Headless(crate::backend::headless::Headless::default());
        Self { backend, camino }
    }

    #[cfg(test)]
    pub fn add_headless_output(
        &mut self,
        size: smithay::utils::Size<i32, smithay::utils::Physical>,
        scale: f64,
    ) -> Output {
        self.backend
            .headless_mut()
            .add_output(&mut self.camino, size, scale)
    }

    /// The number of render elements the scene has for `output`.
    #[cfg(test)]
    pub fn scene_element_count(&mut self, output: &Output) -> usize {
        let renderer = self.backend.headless_mut().renderer();
        crate::scene::output_elements(renderer, &self.camino, output).len()
    }

    /// The refresh pass, after each loop dispatch: brings the outward views up to date with
    /// policy, renders the outputs that asked for it, and flushes the clients.
    pub fn refresh_and_flush_clients(&mut self) {
        self.camino.refresh_views();
        self.update_keyboard_focus();
        self.update_pointer_focus();
        self.redraw_queued_outputs();
        if let Err(err) = self.camino.display_handle.flush_clients() {
            tracing::warn!("error flushing clients: {err}");
        }
    }

    fn redraw_queued_outputs(&mut self) {
        for output in self.camino.take_queued_redraws() {
            if self.backend.render(&mut self.camino, &output) {
                self.post_render(&output);
            } else {
                // The backend can't take a frame yet; it'll be drawn when it can.
                self.camino.queue_redraw(&output);
            }
        }
    }

    /// After `output` is drawn: frame callbacks and presentation feedback for the windows
    /// whose primary output it is, and their fifo and commit-timing barriers released.
    fn post_render(&mut self, output: &Output) {
        let now = self.camino.clock.now();
        let refresh = output
            .current_mode()
            .map(|mode| Refresh::fixed(Duration::from_secs_f64(1000. / f64::from(mode.refresh))))
            .unwrap_or(Refresh::Unknown);
        let mut feedback = OutputPresentationFeedback::new(output);
        let mut blocked = HashMap::new();

        for mapped in self.camino.policy.windows() {
            if mapped.primary_output() != Some(output) {
                continue;
            }
            let window = mapped.window();
            window.with_surfaces(|surface, states| {
                let mut signalled = false;
                if let Some(timer) = states.data_map.get::<CommitTimerBarrierStateUserData>() {
                    signalled |= timer.lock().unwrap().signal_until(now);
                }
                let fifo = states
                    .cached_state
                    .get::<FifoBarrierCachedState>()
                    .current()
                    .barrier
                    .take();
                if let Some(fifo) = fifo {
                    fifo.signal();
                    signalled = true;
                }
                if signalled && let Some(client) = surface.client() {
                    blocked.insert(client.id(), client);
                }
            });
            let primary = |_: &WlSurface, _: &SurfaceData| Some(output.clone());
            window.send_frame(output, now, FRAME_CALLBACK_THROTTLE, primary);
            // Every frame is composited; nothing is scanned out directly.
            window.take_presentation_feedback(&mut feedback, primary, |_, _| {
                wp_presentation_feedback::Kind::empty()
            });
        }
        feedback.presented(now, refresh, 0, wp_presentation_feedback::Kind::empty());

        let dh = self.camino.display_handle.clone();
        for client in blocked.into_values() {
            self.client_compositor_state(&client)
                .blocker_cleared(self, &dh);
        }
    }
}

impl Camino {
    fn new(event_loop: &EventLoop<'static, State>, display: Display<State>) -> Self {
        let display_handle = display.handle();
        let loop_handle = event_loop.handle();

        let source = Generic::new(display, Interest::READ, Mode::Level);
        loop_handle
            .insert_source(source, |_, display, state| {
                // Safety: the display is never dropped while the loop holds it.
                if let Err(err) = unsafe { display.get_mut().dispatch_clients(state) } {
                    tracing::warn!("error dispatching clients: {err}");
                }
                Ok(PostAction::Continue)
            })
            .expect("the display joins the event loop");

        let dh = &display_handle;
        let clock = Clock::new();

        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(dh, "seat0");
        seat.add_pointer();
        seat.add_keyboard(XkbConfig::default(), 600, 25)
            .expect("the default xkb keymap compiles");
        let primary_selection_state = PrimarySelectionState::new::<State>(dh);

        Self {
            loop_signal: event_loop.get_signal(),
            compositor_state: CompositorState::new::<State>(dh),
            shm_state: ShmState::new::<State>(dh, Vec::new()),
            output_manager_state: OutputManagerState::new_with_xdg_output::<State>(dh),
            seat_state,
            seat,
            data_device_state: DataDeviceState::new::<State>(dh),
            primary_selection_state,

            xdg_shell_state: XdgShellState::new::<State>(dh),
            xdg_activation_state: XdgActivationState::new::<State>(dh),
            xdg_foreign_state: XdgForeignState::new::<State>(dh),
            xdg_dialog_state: XdgDialogState::new::<State>(dh),
            xdg_toplevel_icon_manager: XdgToplevelIconManager::new::<State>(dh),
            xdg_system_bell_state: XdgSystemBellState::new::<State>(dh),
            xdg_decoration_state: XdgDecorationState::new::<State>(dh),
            kde_decoration_state: KdeDecorationState::new::<State>(dh, KdeDefaultMode::Client),
            popups: PopupManager::default(),

            pointer_constraints_state: PointerConstraintsState::new::<State>(dh),
            relative_pointer_state: RelativePointerManagerState::new::<State>(dh),
            pointer_gestures_state: PointerGesturesState::new::<State>(dh),
            tablet_manager_state: TabletManagerState::new::<State>(dh),
            keyboard_shortcuts_inhibit_state: KeyboardShortcutsInhibitState::new::<State>(dh),
            text_input_state: TextInputManagerState::new::<State>(dh),
            input_method_state: InputMethodManagerState::new::<State, _>(dh, |_| true),
            cursor_shape_state: CursorShapeManagerState::new::<State>(dh),

            viewporter_state: ViewporterState::new::<State>(dh),
            fractional_scale_state: FractionalScaleManagerState::new::<State>(dh),
            presentation_state: PresentationState::new::<State>(dh, clock.id() as u32),
            single_pixel_buffer_state: SinglePixelBufferState::new::<State>(dh),
            content_type_state: ContentTypeState::new::<State>(dh),
            alpha_modifier_state: AlphaModifierState::new::<State>(dh),
            fifo_manager_state: FifoManagerState::new::<State>(dh),
            commit_timing_state: CommitTimingManagerState::new::<State>(dh),

            pointer_focus: None,
            suppressed_keys: HashSet::new(),
            clock,
            display_handle,
            loop_handle,
            outputs: Vec::new(),
            next_output_id: 0,
            redraws: HashSet::new(),
            policy: Policy::new(Config::default()),
            unmapped: Vec::new(),
            next_window_id: 0,
        }
    }

    /// Opens a Wayland socket for clients to connect on, and returns its name.
    pub fn add_socket(&mut self) -> Result<OsString, Box<dyn std::error::Error>> {
        let source = ListeningSocketSource::new_auto()?;
        let name = source.socket_name().to_os_string();
        self.loop_handle
            .insert_source(source, |stream, _, state| {
                let client = state
                    .camino
                    .display_handle
                    .insert_client(stream, Arc::new(ClientState::default()));
                if let Err(err) = client {
                    tracing::warn!("error adding a client: {err}");
                }
            })
            .map_err(|err| err.error)?;
        Ok(name)
    }

    /// The time policy is given as `now`.
    pub fn now(&self) -> Duration {
        self.clock.now().into()
    }

    /// Sends the outward views of policy state whatever changed since they last sent.
    fn refresh_views(&mut self) {
        self.refresh_window_outputs();
        for mapped in self.policy.windows() {
            mapped.send_pending_configure();
        }
        self.popups.cleanup();
    }

    /// The bounds a new toplevel is told to fit in: the output of the focused window, where
    /// policy places new windows, or else the first output.
    pub fn placement_bounds(&self) -> Option<Size<i32, Logical>> {
        let focused = self
            .policy
            .focus(SEAT)
            .and_then(|id| self.policy.geometry(id));
        let outputs = self.policy.outputs();
        focused
            .and_then(|geometry| outputs.iter().find(|o| o.geometry.overlaps(geometry)))
            .or(outputs.first())
            .map(|output| output.geometry.size)
    }
}
