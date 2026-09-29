//! `State` and `Camino`: everything the event loop runs on (15).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::sync::Arc;
use std::time::Duration;

use camino_policy::{OutputId, Policy, PolicyWindow, SeatId, WindowId};
use smithay::desktop::PopupManager;
use smithay::desktop::utils::{OutputPresentationFeedback, send_frames_surface_tree};
use smithay::backend::input::Keycode;
use smithay::input::keyboard::XkbConfig;
use smithay::input::pointer::CursorImageStatus;
use smithay::input::{Seat, SeatState};
use smithay::output::Output;
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{
    EventLoop, Interest, LoopHandle, LoopSignal, Mode, PostAction, channel,
};
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_protocols::wp::presentation_time::server::wp_presentation_feedback;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Display, DisplayHandle, Resource};
use smithay::utils::{Clock, Logical, Monotonic, Point, Serial, Size};
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
use smithay::reexports::wayland_protocols::xwayland::keyboard_grab::zv1::server::zwp_xwayland_keyboard_grab_v1::ZwpXwaylandKeyboardGrabV1;
use smithay::wayland::xwayland_keyboard_grab::XWaylandKeyboardGrabState;
use smithay::wayland::xwayland_shell::XWaylandShellState;

use camino_theme::{PaletteCache, ThemeDirs, Variant};

use crate::backend::Backend;
use crate::cursor::{CursorSource, Cursors};
use crate::dbus::Dbus;
use crate::debug::DebugToggles;
use crate::decoration::FrameContext;
use crate::decoration::palette::{PaletteSource, PaletteState};
use crate::helpers::{HelperCommand, RestartPolicy, StderrTarget};
use crate::input::frame::FramePointer;
use crate::input::{GrabKind, PointerFocus};
use crate::settings::{self, Settings, SettingsThread};
use crate::window::{Mapped, PendingWindows};
use crate::xwayland::RunningXwayland;

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

    /// What pointer focus was last given, and its origin then.
    pub pointer_focus: Option<(PointerFocus, Point<f64, Logical>)>,
    /// Keys whose press ran a binding, so their release is swallowed too.
    pub suppressed_keys: HashSet<Keycode>,
    /// Where the pointer is on the frames, and what it's doing with them.
    pub frame_pointer: FramePointer,
    /// The interactive move or resize the pointer is doing, if any.
    pub window_grab: Option<GrabKind>,
    /// The cursor the client with pointer focus asked for.
    pub client_cursor: CursorImageStatus,
    /// Camino's own cursors.
    pub cursors: Cursors,

    /// Every output, in the order they were added.
    pub outputs: Vec<Output>,
    pub(crate) next_output_id: u64,
    /// Outputs to render in the next refresh pass.
    pub(crate) redraws: HashSet<OutputId>,

    pub policy: Policy<Mapped>,
    /// Windows asked to show that have no content yet.
    pub(crate) pending: PendingWindows,
    pub(crate) next_window_id: u64,

    /// The GTK colours frames are drawn with.
    pub palette: PaletteSource<State>,
    /// Everything else frames are drawn with.
    pub frame_context: FrameContext,

    /// The settings as last read, and the GLib thread that reads them.
    pub settings: Settings,
    pub settings_thread: SettingsThread,

    /// Xwayland, from its spawn until it fails (11).
    pub xwayland: Option<RunningXwayland>,
    pub xwayland_shell_state: XWaylandShellState,
    /// The keyboard grabs Xwayland takes for override-redirect windows (36).
    pub xwayland_keyboard_grab_state: XWaylandKeyboardGrabState,
    /// The keyboard grab an override-redirect window took through Xwayland, if any, and the
    /// serial the keyboard knows it by (36).
    pub(crate) xwayland_keyboard_grab: Option<(ZwpXwaylandKeyboardGrabV1, Serial)>,
    /// The variables Camino sets for the clients it spawns (08), such as `DISPLAY`. They're
    /// kept here rather than set with `set_var`, which isn't safe once the settings and
    /// D-Bus threads run.
    pub environment: BTreeMap<OsString, OsString>,

    /// The connection to the session bus, once `start_dbus` has made it.
    pub dbus: Option<Dbus>,
    pub debug: DebugToggles,
}

/// The palette helper, `camino-gtk-palette` or what `CAMINO_GTK_PALETTE` names, as a
/// client on `socket`. Until it reports, the palette is the one `cache` has for `gtk_theme`,
/// or neutral.
fn palette_source(
    handle: LoopHandle<'static, State>,
    socket: Option<&OsStr>,
    cache: Option<PaletteCache>,
    gtk_theme: &str,
) -> PaletteSource<State> {
    let program = std::env::var_os("CAMINO_GTK_PALETTE").unwrap_or("camino-gtk-palette".into());
    let mut command =
        HelperCommand::new("camino-gtk-palette", program).env("GDK_BACKEND", "wayland");
    if let Some(socket) = socket {
        command = command.env("WAYLAND_DISPLAY", socket);
    }
    PaletteSource::new(
        handle,
        command,
        RestartPolicy::default(),
        StderrTarget::Prefixed,
        cache,
        Some(gtk_theme),
    )
}

impl PaletteState for State {
    fn palette_source(&mut self) -> &mut PaletteSource<State> {
        &mut self.camino.palette
    }

    fn palette_changed(&mut self) {
        tracing::debug!(
            "palette for {} in use",
            self.camino
                .palette
                .gtk_theme()
                .unwrap_or("an unknown GTK theme")
        );
        self.camino.frame_context.palette_changed();
        self.camino.queue_redraw_all();
    }
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
    /// Opens a winit window as the one output, with the installed settings.
    pub fn new_winit(
        event_loop: &EventLoop<'static, State>,
        display: Display<State>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut camino = Camino::new(
            event_loop,
            display,
            settings::Source::System,
            ThemeDirs::from_env(),
            PaletteCache::from_env(),
            CursorSource::Xcursor,
        )?;
        let backend = Backend::Winit(Box::new(crate::backend::winit::Winit::new(&mut camino)?));
        Ok(Self { backend, camino })
    }

    /// Starts with no outputs; tests add them with `add_headless_output`. Frames use the
    /// themes vendored for the goldens (14), no palette cache, and only the built-in
    /// cursor.
    #[cfg(test)]
    pub fn new_headless(
        event_loop: &EventLoop<'static, State>,
        display: Display<State>,
        settings: settings::Source,
    ) -> Self {
        let themes = ThemeDirs::new(vec![
            concat!(env!("CARGO_MANIFEST_DIR"), "/../tests").into(),
        ]);
        let camino = Camino::new(
            event_loop,
            display,
            settings,
            themes,
            None,
            CursorSource::BuiltIn,
        )
        .expect("the settings start");
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

    /// The layer of each of the scene's render elements for `output`, front to back.
    #[cfg(test)]
    pub(crate) fn scene_layers(&mut self, output: &Output) -> Vec<crate::scene::SceneLayer> {
        let renderer = self.backend.headless_mut().renderer();
        crate::scene::output_element_layers(renderer, &self.camino, output)
    }

    /// The refresh pass, after each loop dispatch: brings the outward views up to date with
    /// policy, renders the outputs that asked for it, and flushes the clients.
    pub fn refresh_and_flush_clients(&mut self) {
        // x11rb queues the X messages that arrive while Camino waits for a reply, which
        // leaves the socket quiet, so they're read here too.
        self.x_requests();
        self.camino.refresh_views();
        self.update_pointer_focus();
        self.camino.update_button_states();
        self.camino.refresh_frames();
        self.update_keyboard_focus();
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
        // Override-redirect windows have no primary output: they're told of frames on
        // every output they're drawn on.
        let output_geometry = crate::output::output_geometry(output);
        for window in self.camino.override_redirect_windows() {
            let Some(surface) = window.wl_surface() else {
                continue;
            };
            if crate::scene::override_redirect_bbox(window).overlaps(output_geometry) {
                send_frames_surface_tree(&surface, output, now, FRAME_CALLBACK_THROTTLE, |_, _| {
                    Some(output.clone())
                });
            }
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
    /// Waits for the settings thread's first reading, so the settings are complete before
    /// any client connects.
    fn new(
        event_loop: &EventLoop<'static, State>,
        display: Display<State>,
        settings: settings::Source,
        theme_dirs: ThemeDirs,
        palette_cache: Option<PaletteCache>,
        cursor_source: CursorSource,
    ) -> Result<Self, settings::Error> {
        let (settings, settings_thread, changes) = settings::start(settings)?;
        let display_handle = display.handle();
        let loop_handle = event_loop.handle();
        loop_handle
            .insert_source(changes, |event, _, state| match event {
                channel::Event::Msg(settings) => state.camino.settings_changed(settings),
                channel::Event::Closed => {
                    tracing::error!("the settings thread stopped; changes are no longer seen")
                }
            })
            .expect("the settings channel joins the event loop");

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
        let xwayland_shell_state = XWaylandShellState::new::<State>(dh);
        let xwayland_keyboard_grab_state = XWaylandKeyboardGrabState::new::<State>(dh);

        let frame_context = FrameContext::new(&settings, theme_dirs);
        let cursors = Cursors::new(&settings.mouse, cursor_source);
        let palette = palette_source(
            event_loop.handle(),
            None,
            palette_cache,
            &settings.interface.gtk_theme,
        );
        let mut camino = Self {
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
            kde_decoration_state: KdeDecorationState::new::<State>(dh, KdeDefaultMode::Server),
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
            frame_pointer: FramePointer::default(),
            window_grab: None,
            client_cursor: CursorImageStatus::default_named(),
            cursors,
            clock,
            display_handle,
            loop_handle,
            outputs: Vec::new(),
            next_output_id: 0,
            redraws: HashSet::new(),
            policy: Policy::new(settings.policy.clone()),
            pending: PendingWindows::default(),
            next_window_id: 0,
            palette,
            frame_context,
            settings,
            settings_thread,
            xwayland: None,
            xwayland_shell_state,
            xwayland_keyboard_grab_state,
            xwayland_keyboard_grab: None,
            environment: BTreeMap::new(),
            dbus: None,
            debug: DebugToggles::default(),
        };
        camino.ask_palette_for_theme_colors();
        Ok(camino)
    }

    /// Asks the palette helper for the `gtk:custom` colours the theme uses.
    fn ask_palette_for_theme_colors(&mut self) {
        self.palette
            .set_names(self.frame_context.theme().custom_color_names());
    }

    /// New settings from the settings thread: passes on what changed.
    fn settings_changed(&mut self, settings: Settings) {
        if settings.policy != self.settings.policy {
            tracing::debug!(
                "{} workspaces: {:?}",
                settings.policy.workspace_count,
                settings.policy.workspace_names
            );
            self.policy.settings_changed(settings.policy.clone());
        }
        if settings.interface.gtk_theme != self.settings.interface.gtk_theme {
            // The helper reports the new theme's palette; the cached one is used meanwhile.
            if self.palette.use_cached(&settings.interface.gtk_theme) {
                self.frame_context.palette_changed();
                self.queue_redraw_all();
            }
        }
        if self.frame_context.settings_changed(&settings) {
            self.frames_changed();
        }
        if self.cursors.settings_changed(&settings.mouse) {
            self.queue_redraw_all();
        }
        self.settings = settings;
    }

    /// Something every frame is drawn with changed: frames may have new extents, which
    /// policy lays their windows out by, and are drawn again.
    fn frames_changed(&mut self) {
        self.ask_palette_for_theme_colors();
        let ids: Vec<WindowId> = self.policy.windows().map(|mapped| mapped.id()).collect();
        for id in ids {
            self.update_frame(id);
        }
        self.queue_redraw_all();
    }

    /// Reads a window's frame again, and has policy lay it out by it.
    fn update_frame(&mut self, id: WindowId) {
        if self.read_window_facts(id) {
            self.policy.frame_changed(id);
        }
    }

    /// A surface's decoration mode changed without a commit, as KDE server-decoration's
    /// does.
    pub fn decoration_changed(&mut self, surface: &WlSurface) {
        if let Some(id) = self.mapped_for_surface(surface) {
            self.update_frame(id);
            self.queue_redraw_all();
        }
    }

    /// Draws again the strips of every frame whose inputs changed, and redraws if any did.
    fn refresh_frames(&mut self) {
        let colors = self.palette.palette().variant(Variant::Default);
        let ids: Vec<WindowId> = self.policy.windows().map(|mapped| mapped.id()).collect();
        let mut changed = false;
        for id in ids {
            let mapped = self.policy.window_mut(id).unwrap();
            changed |= mapped.refresh_frame(&mut self.frame_context, colors);
        }
        if changed {
            self.queue_redraw_all();
        }
    }

    /// Starts `camino-gtk-palette` as a client on the socket `add_socket` opened.
    pub fn start_palette_helper(&mut self, socket: &OsStr) {
        self.palette = palette_source(
            self.loop_handle.clone(),
            Some(socket),
            self.palette.cache().cloned(),
            &self.settings.interface.gtk_theme,
        );
        self.ask_palette_for_theme_colors();
        self.palette.start();
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
            mapped.flush();
        }
        self.refresh_x11_stacking();
        self.refresh_x_root();
        self.refresh_x_windows();
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
