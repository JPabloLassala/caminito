//! `State` and `Caminito`: everything the event loop runs on (15).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::Duration;

use caminito_policy::{OutputId, Policy, PolicyWindow, SeatId, WindowId};
use smithay::desktop::{LayerSurface, PopupManager, layer_map_for_output};
use smithay::desktop::utils::{
    OutputPresentationFeedback, send_frames_surface_tree, take_presentation_feedback_surface_tree,
    with_surfaces_surface_tree,
};
use smithay::backend::input::Keycode;
use smithay::input::keyboard::XkbConfig;
use smithay::input::pointer::CursorImageStatus;
use smithay::input::{Seat, SeatState};
use smithay::output::Output;
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{
    EventLoop, Interest, LoopHandle, LoopSignal, Mode, PostAction, RegistrationToken, channel,
};
use smithay::reexports::wayland_server::backend::{
    ClientData, ClientId, DisconnectReason, GlobalId,
};
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
use smithay::wayland::foreign_toplevel_list::{ForeignToplevelHandle, ForeignToplevelListState};
use smithay::wayland::fractional_scale::FractionalScaleManagerState;
use smithay::wayland::idle_inhibit::IdleInhibitManagerState;
use smithay::wayland::image_capture_source::{
    OutputCaptureSourceState, ToplevelCaptureSourceState,
};
use smithay::wayland::image_copy_capture::ImageCopyCaptureState;
use smithay::wayland::input_method::InputMethodManagerState;
use smithay::wayland::keyboard_shortcuts_inhibit::KeyboardShortcutsInhibitState;
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::pointer_constraints::PointerConstraintsState;
use smithay::wayland::pointer_gestures::PointerGesturesState;
use smithay::wayland::presentation::{PresentationState, Refresh};
use smithay::wayland::relative_pointer::RelativePointerManagerState;
use smithay::wayland::security_context::{SecurityContext, SecurityContextState};
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::selection::ext_data_control::DataControlState as ExtDataControlState;
use smithay::wayland::selection::primary_selection::PrimarySelectionState;
use smithay::wayland::selection::wlr_data_control::DataControlState as WlrDataControlState;
use smithay::wayland::session_lock::SessionLockManagerState;
use smithay::reexports::wayland_protocols_misc::server_decoration::server::org_kde_kwin_server_decoration_manager::Mode as KdeDefaultMode;
use smithay::wayland::shell::kde::decoration::KdeDecorationState;
use smithay::wayland::shell::wlr_layer::WlrLayerShellState;
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

use caminito_theme::{PaletteCache, ThemeDirs, Variant};

use crate::backend::{self, Backend};
use crate::bell::Bells;
use crate::cursor::{CursorSource, Cursors};
use crate::dbus::Dbus;
use crate::dbus::a11y::KeyboardMonitor;
use crate::dbus::global_menu::{GlobalMenu, Switch};
use crate::dbus::logind::Logind;
use crate::dbus::menu_claim::Claims;
use crate::dbus::panel::Panel;
use crate::dbus::registrar::Registrations;
use crate::dbus::screensaver::ScreensaverShim;
use crate::deadlines::Deadlines;
use crate::debug::DebugToggles;
use crate::decoration::FrameContext;
use crate::decoration::palette::{PaletteSource, PaletteState};
use crate::handlers::{CaptureSession, DragIcon, sees_privileged_globals};
use crate::helpers::{HelperCommand, RestartPolicy, StderrTarget};
use crate::idle::Idle;
use crate::input::accessx;
use crate::input::bindings::accelerators::Grabs;
use crate::input::devices::InputDevices;
use crate::input::frame::FramePointer;
use crate::input::repeat::Repeat;
use crate::input::{GrabKind, InputSerials, PointerFocus};
use crate::lock::{Lock, Locker};
use crate::minimize_animation::MinimizeAnimations;
use crate::night_light::{self, NightLight};
use crate::output::primary::PrimaryChoice;
use crate::output::saved::{self, SavedLayouts};
use crate::output::{OutputRecord, output_id};
use crate::protocols::appmenu::AppmenuState;
use crate::protocols::ext_workspace::ExtWorkspaceState;
use crate::protocols::foreign_toplevel::ForeignToplevelState;
use crate::protocols::gamma_control::GammaControlState;
use crate::protocols::gtk_shell::GtkShellState;
use crate::protocols::idle_notify::IdleNotifyState;
use crate::protocols::output_management::OutputManagementState;
use crate::protocols::xfce_foreign_toplevel::XfceForeignToplevelState;
use crate::session::Session;
use crate::settings::translate::MARCO_GENERAL;
use crate::settings::{self, Settings, SettingsThread, Value};
use crate::switcher::OpenSwitcher;
use crate::window::{Mapped, PendingWindows};
use crate::workspace_popup::WorkspacePopup;
use crate::xwayland::{self, RunningXwayland, WaylandSelections};

/// How long a window that isn't redrawn waits for a frame callback anyway.
const FRAME_CALLBACK_THROTTLE: Option<Duration> = Some(Duration::from_secs(1));

/// The tests' wall clock at Caminito's start: 2026-10-10 at 12:00 UTC (138).
#[cfg(test)]
const TEST_WALL_CLOCK: f64 = 1_791_633_600.0;

/// Policy's id for Caminito's one seat.
pub const SEAT: SeatId = SeatId(0);

/// The whole compositor: the backend, and everything that doesn't depend on it.
pub struct State {
    pub backend: Backend,
    pub caminito: Caminito,
}

/// Caminito's backend-independent state.
pub struct Caminito {
    pub display_handle: DisplayHandle,
    pub loop_handle: LoopHandle<'static, State>,
    pub loop_signal: LoopSignal,

    /// Monotonic time, which policy's `now` and frame callbacks are measured in.
    pub clock: Clock<Monotonic>,

    pub compositor_state: CompositorState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub output_management_state: OutputManagementState,
    pub seat_state: SeatState<State>,
    pub seat: Seat<State>,
    pub data_device_state: DataDeviceState,
    pub primary_selection_state: PrimarySelectionState,
    // Data-control, for a clipboard manager (68).
    pub ext_data_control_state: ExtDataControlState,
    pub wlr_data_control_state: WlrDataControlState,
    /// The global through which a sandbox engine makes security contexts (68).
    pub security_context_state: SecurityContextState,

    // Shell (06): xdg-shell and the protocols that extend its toplevels.
    pub xdg_shell_state: XdgShellState,
    pub xdg_activation_state: XdgActivationState,
    pub xdg_foreign_state: XdgForeignState,
    pub xdg_dialog_state: XdgDialogState,
    pub gtk_shell_state: GtkShellState,
    pub appmenu_state: AppmenuState,
    pub xdg_toplevel_icon_manager: XdgToplevelIconManager,
    pub xdg_system_bell_state: XdgSystemBellState,
    pub xdg_decoration_state: XdgDecorationState,
    pub kde_decoration_state: KdeDecorationState,
    pub popups: PopupManager,
    /// wlr-layer-shell, for the panel, the desktop and the wallpaper (63).
    pub(crate) layer_shell_state: WlrLayerShellState,

    // The windows as a panel sees them (65).
    pub foreign_toplevel_state: ForeignToplevelState,
    pub foreign_toplevel_list_state: ForeignToplevelListState,
    /// ext-foreign-toplevel-list's handle for each window it lists, whose identifier names
    /// the window to M6's `GetActiveToplevel` and image capture.
    pub(crate) foreign_toplevel_handles: HashMap<WindowId, ForeignToplevelHandle>,
    /// The workspaces as a panel's workspace switcher sees them (66).
    pub ext_workspace_state: ExtWorkspaceState,
    /// The workspace each window is on, as a panel's window list sees it (75).
    pub xfce_foreign_toplevel_state: XfceForeignToplevelState,

    // Image capture, for a screenshot tool (103).
    pub output_capture_source_state: OutputCaptureSourceState,
    pub toplevel_capture_source_state: ToplevelCaptureSourceState,
    pub image_copy_capture_state: ImageCopyCaptureState,
    /// The capture sessions that are open, with the frames their clients wait for.
    pub(crate) captures: Vec<CaptureSession>,

    // Input (06).
    pub pointer_constraints_state: PointerConstraintsState,
    pub relative_pointer_state: RelativePointerManagerState,
    pub pointer_gestures_state: PointerGesturesState,
    pub tablet_manager_state: TabletManagerState,
    pub keyboard_shortcuts_inhibit_state: KeyboardShortcutsInhibitState,
    pub text_input_state: TextInputManagerState,
    pub input_method_state: InputMethodManagerState,
    pub cursor_shape_state: CursorShapeManagerState,

    /// ext-session-lock, for a locker (12).
    pub(crate) session_lock_state: SessionLockManagerState,
    /// The session lock: whether the session is locked, and what the locker shows (12).
    pub(crate) lock: Lock,
    /// The locker Caminito spawns for its own lock requests (126).
    pub(crate) locker: Locker,
    /// ext-idle-notify and idle-inhibit, which `idle` times and counts (128).
    pub idle_notify_state: IdleNotifyState,
    pub idle_inhibit_state: IdleInhibitManagerState,
    /// Idle's timers, the blank and what holds them off (128).
    pub(crate) idle: Idle,
    /// The loop timer for the earliest deadline of what waits on Caminito's clock (174).
    pub(crate) deadlines: Deadlines,
    /// Night Light's temperatures and fades (ADR 0023).
    pub(crate) night_light: NightLight,
    /// `NightLight` as `org.mate.Caminito`'s callers last heard it (137).
    pub(crate) night_light_signalled: Option<night_light::Published>,
    /// wlr-gamma-control, offered only by a backend with gamma tables (141).
    pub(crate) gamma_control_state: GammaControlState,
    /// How far a test has moved Caminito's clock on, so that idle's timers come due
    /// without waiting for them (128).
    #[cfg(test)]
    pub(crate) clock_offset: Duration,
    /// The tests' wall clock: the Unix time it read when `now` was the second value, from
    /// which it runs with Caminito's clock (138). It starts at `TEST_WALL_CLOCK`, so that
    /// what follows the sun is the same on every run, and `Fixture::set_wall_clock` sets it.
    #[cfg(test)]
    pub(crate) wall_clock: (f64, Duration),

    // Rendering (06). linux-dmabuf and drm-syncobj are the tty backend's (57).
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
    /// The layer surface a click gave the keyboard, and policy's focus then (63).
    pub(crate) keyboard_layer: Option<(LayerSurface, Option<WindowId>)>,
    /// The name policy gets for the next layer surface (163).
    pub(crate) next_layer_id: u64,
    /// Keys whose press ran a binding or a grabbed accelerator, so their release is
    /// swallowed too.
    pub suppressed_keys: HashSet<Keycode>,
    /// The accelerators callers of `org.mate.Caminito` have grabbed (98).
    pub(crate) accelerator_grabs: Grabs,
    /// The bells rung (105).
    pub(crate) bells: Bells,
    /// The windows shrinking into their buttons as they're minimised (172).
    pub(crate) minimize_animations: MinimizeAnimations,
    /// The grabbed accelerator whose key is held and repeating (99).
    pub(crate) accelerator_repeat: Option<Repeat>,
    /// Where the pointer is on the frames, and what it's doing with them.
    pub frame_pointer: FramePointer,
    /// The interactive move or resize the pointer is doing, if any.
    pub window_grab: Option<GrabKind>,
    /// Pointer buttons whose press was the Window switcher's (108), or closed the Workspace
    /// popup (152), so their release is swallowed too.
    pub(crate) suppressed_buttons: HashSet<u32>,
    /// The compositor's side of the Window switcher while it's open (108).
    pub(crate) switcher: Option<OpenSwitcher>,
    /// The Workspace popup while it shows (152).
    pub(crate) workspace_popup: Option<WorkspacePopup>,
    /// The cursor the client with pointer focus asked for.
    pub client_cursor: CursorImageStatus,
    /// The icon of the Wayland drag in progress, if it has one (48).
    pub(crate) drag_icon: Option<DragIcon>,
    /// Caminito's own cursors.
    pub cursors: Cursors,

    /// Every output, in the order they were added.
    pub outputs: Vec<Output>,
    pub(crate) next_output_id: u64,
    /// The monitors output management disabled, which it can enable again (58).
    pub(crate) disabled_outputs: Vec<Output>,
    /// What Caminito keeps of each output beside Smithay's state (51).
    pub(crate) output_records: HashMap<OutputId, OutputRecord>,
    /// The `wl_output` globals of the outputs hidden since the last refresh, which the
    /// clients still see (91), and the appmenu global if it stopped being offered (112).
    pub(crate) globals_to_withdraw: Vec<GlobalId>,
    /// The monitor the user chose as the primary output (81), if they chose one.
    pub(crate) primary_choice: Option<PrimaryChoice>,
    /// The layouts the user saved, which are restored as monitors come and go (83).
    pub(crate) saved_layouts: SavedLayouts,
    /// Whether a monitor was plugged in or unplugged since a saved layout was last looked
    /// for.
    pub(crate) monitors_changed: bool,
    /// The primary output's name as `org.mate.Caminito`'s callers last heard it (81).
    pub(crate) primary_output_signalled: Option<String>,
    /// `ShowingDesktop` as `org.mate.Caminito`'s callers last heard it (94).
    pub(crate) showing_desktop_signalled: Option<bool>,
    /// `GlobalMenu` as `org.mate.Caminito`'s callers last heard it (110).
    pub(crate) global_menu_signalled: Option<GlobalMenu>,
    /// Whether the apps should give up their own menubars, which follows the Global menu
    /// applet's bus name and outlasts it by a grace period (ADR 0017, 114).
    pub(crate) global_menu_switch: Switch,
    /// The menus X11 clients registered for their windows (113).
    pub(crate) app_menu_registrations: Registrations,
    /// The toplevels the menu claims tied to their callers' menus (115).
    pub(crate) menu_claims: Claims,
    /// Whether the apps are told to give up their own menubars: while the switch is on,
    /// and after it until the registrar's name is given up (112).
    pub(crate) global_menu_offered: bool,
    /// `GlobalMenuOffered` as `org.mate.Caminito`'s callers last heard it (115).
    pub(crate) global_menu_offered_signalled: Option<bool>,
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
    /// The workspace count last written to `num-workspaces` for `dynamic-workspaces`, until
    /// the key changes (151).
    written_workspace_count: Option<usize>,
    /// The input devices plugged in, which get the settings for their kind (53).
    pub(crate) input_devices: InputDevices,
    /// NumLock's state when the refresh pass last looked, so a change is written back once.
    pub(crate) numlock_seen: bool,
    /// AccessX's filter, stage 2 of the keyboard path (134).
    pub(crate) accessx: accessx::Filter,

    /// Xwayland, from its spawn until it fails (11).
    pub xwayland: Option<RunningXwayland>,
    /// What's kept of Xwayland from one spawn to the next, and when it's restarted (42).
    pub(crate) xwayland_restart: xwayland::Restart,
    /// The seat's Wayland selections, which X is owed as Xwayland starts (37, 42).
    pub(crate) wayland_selections: WaylandSelections,
    pub xwayland_shell_state: XWaylandShellState,
    /// The keyboard grabs Xwayland takes for override-redirect windows (36).
    pub xwayland_keyboard_grab_state: XWaylandKeyboardGrabState,
    /// The keyboard grab an override-redirect window took through Xwayland, if any, and the
    /// serial the keyboard knows it by (36).
    pub(crate) xwayland_keyboard_grab: Option<(ZwpXwaylandKeyboardGrabV1, Serial)>,
    /// The variables Caminito sets for the clients it spawns (08), such as `DISPLAY`. They're
    /// kept here rather than set with `set_var`, which isn't safe once the settings and
    /// D-Bus threads run.
    pub environment: BTreeMap<OsString, OsString>,
    /// `rename-workspace`'s dialog while it shows: the source its answer comes back on
    /// (167). Another isn't shown meanwhile, as in Marco.
    pub(crate) workspace_rename: Option<RegistrationToken>,

    /// The Wayland socket's source, until a Session closes it as it ends (61).
    socket_source: Option<RegistrationToken>,
    /// Where the helpers' stderr goes: the journal in a Session that logs there (14).
    pub(crate) helper_stderr: StderrTarget,
    /// The Session, under `--session` (61).
    pub(crate) session: Option<Session>,

    /// The connection to the session bus, once `start_dbus` has made it.
    pub dbus: Option<Dbus>,
    /// Caminito's session in logind, on the system bus, once `start_logind` has found it
    /// (130).
    pub(crate) logind: Option<Logind>,
    /// The task that follows UPower's `OnBattery` on the system bus, once `start_upower`
    /// has made it (129).
    pub(crate) upower: Option<zbus::Task<()>>,
    /// The calls to the panel for Alt+F1 and Alt+F2, and the popup grab they allow (95).
    pub(crate) panel: Panel,
    /// The key and button events clients were last sent, which a popup grab names one of.
    pub(crate) input_serials: InputSerials,
    /// The cookies the Screensaver shim has handed out (62).
    pub screensaver: ScreensaverShim,
    /// The Keyboard monitor's callers, and the keys it holds back (135).
    pub(crate) keyboard_monitor: KeyboardMonitor,
    pub debug: DebugToggles,
}

/// The palette helper, `caminito-gtk-palette` or what `CAMINITO_GTK_PALETTE` names, as a
/// client on `socket`. Until it reports, the palette is the one `cache` has for `gtk_theme`,
/// or neutral.
fn palette_source(
    handle: LoopHandle<'static, State>,
    socket: Option<&OsStr>,
    cache: Option<PaletteCache>,
    gtk_theme: &str,
    stderr: StderrTarget,
) -> PaletteSource<State> {
    let program = std::env::var_os("CAMINITO_GTK_PALETTE").unwrap_or("caminito-gtk-palette".into());
    let mut command =
        HelperCommand::new("caminito-gtk-palette", program).env("GDK_BACKEND", "wayland");
    if let Some(socket) = socket {
        command = command.env("WAYLAND_DISPLAY", socket);
    }
    PaletteSource::new(
        handle,
        command,
        RestartPolicy::default(),
        stderr,
        cache,
        Some(gtk_theme),
    )
}

impl PaletteState for State {
    fn palette_source(&mut self) -> &mut PaletteSource<State> {
        &mut self.caminito.palette
    }

    fn palette_changed(&mut self) {
        tracing::debug!(
            "palette for {} in use",
            self.caminito
                .palette
                .gtk_theme()
                .unwrap_or("an unknown GTK theme")
        );
        self.caminito.frame_context.palette_changed();
        self.caminito.queue_redraw_all();
    }
}

/// Per-client data.
#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
    /// The security context the client connected through, which makes it sandboxed (68).
    pub(crate) security_context: Option<SecurityContext>,
    /// The process a test has Caminito take the client for, in place of the one its
    /// socket's credentials name: a fixture's clients are all of the test's process (95).
    #[cfg(test)]
    pub(crate) process: Option<u32>,
}

/// The process `client` is of, from its socket's credentials.
pub(crate) fn client_process(
    client: &smithay::reexports::wayland_server::Client,
    display: &DisplayHandle,
) -> Option<u32> {
    #[cfg(test)]
    if let Some(process) = client
        .get_data::<ClientState>()
        .and_then(|data| data.process)
    {
        return Some(process);
    }
    let credentials = client.get_credentials(display).ok()?;
    u32::try_from(credentials.pid).ok()
}

impl ClientData for ClientState {
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

impl State {
    /// Starts on `kind`'s backend, with the installed settings: a winit window as the one
    /// output, or the machine's monitors from the tty.
    pub fn new(
        event_loop: &EventLoop<'static, State>,
        display: Display<State>,
        kind: backend::Kind,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut caminito = Caminito::new(
            event_loop,
            display,
            settings::Source::System,
            ThemeDirs::from_env(),
            PaletteCache::from_env(),
            CursorSource::Xcursor,
        )?;
        // Before the backend finds the monitors, so that they're restored as they're found.
        caminito.saved_layouts = SavedLayouts::load(saved::default_path());
        let backend = match kind {
            backend::Kind::Winit => {
                Backend::Winit(Box::new(backend::winit::Winit::new(&mut caminito)?))
            }
            backend::Kind::Tty => Backend::Tty(Box::new(backend::tty::Tty::new(&mut caminito)?)),
        };
        let mut state = Self { backend, caminito };
        // Before the first frame, which the event loop draws (ADR 0014).
        state.monitors_scanned();
        Ok(state)
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
        let caminito = Caminito::new(
            event_loop,
            display,
            settings,
            themes,
            None,
            CursorSource::BuiltIn,
        )
        .expect("the settings start");
        let backend = Backend::Headless(crate::backend::headless::Headless::default());
        Self { backend, caminito }
    }

    /// New settings from the settings thread. The keymap is set here, as the keyboard
    /// needs all of `State` to send it.
    fn settings_changed(&mut self, settings: Settings) {
        if settings.keymap != self.caminito.settings.keymap {
            self.keymap_changed(&settings.keymap);
        }
        // Here too: the modifiers sticky keys let go of are keys for the keyboard.
        self.accessx_settings_changed(settings.accessx);
        let idle = settings.idle;
        self.caminito.settings_changed(settings);
        // And here, as a lock may come of it, with the new `disable-lock-screen`.
        self.idle_settings_changed(idle);
    }

    /// Plugs in a headless monitor whose EDID says it's `physical_size` millimetres, with
    /// modes of `modes`, the first of which it prefers, built in if `internal`.
    #[cfg(test)]
    pub fn plug_headless_output(
        &mut self,
        physical_size: smithay::utils::Size<i32, smithay::utils::Raw>,
        modes: &[smithay::utils::Size<i32, smithay::utils::Physical>],
        internal: bool,
    ) -> Output {
        let output =
            self.backend
                .headless_mut()
                .plug(&mut self.caminito, physical_size, modes, internal);
        self.monitors_scanned();
        output
    }

    /// Plugs a headless monitor that was unplugged in again, at the connector it was at.
    #[cfg(test)]
    pub fn replug_headless_output(&mut self, unplugged: &Output) -> Output {
        let modes: Vec<_> = unplugged.modes().iter().map(|mode| mode.size).collect();
        let output = self.backend.headless_mut().plug_at(
            &mut self.caminito,
            unplugged.name(),
            unplugged.physical_properties().size,
            &modes,
        );
        self.monitors_scanned();
        output
    }

    /// Switches a headless monitor to a mode of `size`.
    #[cfg(test)]
    pub fn change_headless_mode(
        &mut self,
        output: &Output,
        size: smithay::utils::Size<i32, smithay::utils::Physical>,
    ) {
        self.backend
            .headless_mut()
            .change_mode(&mut self.caminito, output, size);
    }

    /// Switches a headless monitor to its mode at another refresh rate, in millihertz.
    #[cfg(test)]
    pub fn change_headless_refresh(&mut self, output: &Output, refresh: i32) {
        let size = output.current_mode().expect("a lit output has a mode").size;
        let mode = smithay::output::Mode { size, refresh };
        let headless = self.backend.headless_mut();
        assert!(
            headless.set_mode(&mut self.caminito, output, mode),
            "the mode is shown"
        );
    }

    /// Unplugs a headless monitor.
    #[cfg(test)]
    pub fn unplug_headless_output(&mut self, output: &Output) {
        self.backend
            .headless_mut()
            .unplug(&mut self.caminito, output);
        self.monitors_scanned();
    }

    /// The number of render elements the scene has for `output`.
    #[cfg(test)]
    pub fn scene_element_count(&mut self, output: &Output) -> usize {
        let renderer = self.backend.headless_mut().renderer();
        crate::scene::output_elements(renderer, &self.caminito, output).len()
    }

    /// The layer of each of the scene's render elements for `output`, front to back.
    #[cfg(test)]
    pub(crate) fn scene_layers(&mut self, output: &Output) -> Vec<crate::scene::SceneLayer> {
        let renderer = self.backend.headless_mut().renderer();
        crate::scene::output_element_layers(renderer, &self.caminito, output)
    }

    /// The refresh pass, after each loop dispatch: brings the outward views up to date with
    /// policy, renders the outputs that asked for it, copies the captures that are due, and
    /// flushes the clients.
    pub fn refresh_and_flush_clients(&mut self) {
        // x11rb queues the X messages that arrive while Caminito waits for a reply, which
        // leaves the socket quiet, so they're read here too.
        self.x_requests();
        self.caminito.notice_xwayland_died_starting();
        self.caminito.advance_session();
        // Before the views and the focus, which follow the lock.
        self.caminito.refresh_lock();
        self.caminito.refresh_views();
        // Before the pointer's focus, which a keyboard move or resize (156) and a manual
        // placement (154) take.
        self.refresh_keyboard_grab();
        // After the views, which show and hide the windows an inhibitor is on.
        self.refresh_idle();
        // After idle and the lock, whose changes it signals.
        self.caminito.refresh_screensaver();
        // Before Night Light, which steps in on an output that went from a gamma client.
        self.refresh_gamma_control();
        self.refresh_night_light();
        self.update_pointer_focus();
        self.caminito.update_button_states();
        self.caminito.refresh_frames();
        // Before the outputs are drawn, with what policy minimised since the last pass.
        self.caminito.refresh_minimize_animations();
        self.caminito.refresh_panel_layers();
        self.caminito.refresh_switcher_popup();
        self.caminito.refresh_workspace_popup();
        self.update_keyboard_focus();
        self.redraw_queued_outputs();
        // After the frames, which may show the lock that logind's sleep waits for.
        self.caminito.refresh_logind();
        self.refresh_captures();
        // After everything that moves a deadline (174): the pointer's crossings, which
        // queue an auto-raise, idle's refresh, the minimise animations' start, and the
        // handlers before this pass.
        self.caminito.refresh_deadlines();
        if let Err(err) = self.caminito.display_handle.flush_clients() {
            tracing::warn!("error flushing clients: {err}");
        }
    }

    fn redraw_queued_outputs(&mut self) {
        let mut rendered = Vec::new();
        for output in self.caminito.take_queued_redraws() {
            match self.backend.render(&mut self.caminito, &output) {
                Some(states) => {
                    self.caminito.lock.frame_drawn(&output);
                    rendered.push((output, states));
                }
                // While the outputs are powered off no frame is drawn, and none shows the
                // session: the lock is on screen as far as a locker and logind can tell, as
                // under the blank (129).
                None if self.caminito.idle.is_powered_off() => {
                    self.caminito.lock.frame_drawn(&output);
                }
                // The backend can't take a frame yet; it'll be drawn when it can.
                None => self.caminito.queue_redraw(&output),
            }
        }
        if rendered.is_empty() {
            return;
        }
        // Every output drawn in this pass has its say in the main outputs before any
        // frame callback goes by them.
        let ids: Vec<WindowId> = self.caminito.policy.windows().map(|m| m.id()).collect();
        for id in ids {
            let mapped = self.caminito.policy.window_mut(id).unwrap();
            mapped.update_main_outputs(&rendered);
        }
        for (output, _) in &rendered {
            self.post_render(output);
        }
    }

    /// After `output` is drawn: frame callbacks and presentation feedback for the windows
    /// whose main output it is, for its layer surfaces and for its lock surface, and their
    /// fifo and commit-timing barriers released.
    fn post_render(&mut self, output: &Output) {
        let now = self.caminito.clock.now();
        let refresh = output
            .current_mode()
            .map(|mode| Refresh::fixed(Duration::from_secs_f64(1000. / f64::from(mode.refresh))))
            .unwrap_or(Refresh::Unknown);
        let mut feedback = OutputPresentationFeedback::new(output);
        let mut blocked = HashMap::new();

        let mut release_barriers = |surface: &WlSurface, states: &SurfaceData| {
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
        };
        let main = |_: &WlSurface, _: &SurfaceData| Some(output.clone());
        let no_flags = |_: &WlSurface, _: &SurfaceData| wp_presentation_feedback::Kind::empty();

        for mapped in self.caminito.policy.windows() {
            if mapped.main_output() != Some(output) {
                continue;
            }
            let window = mapped.window();
            window.with_surfaces(&mut release_barriers);
            window.send_frame(output, now, FRAME_CALLBACK_THROTTLE, main);
            // Every frame is composited; nothing is scanned out directly.
            window.take_presentation_feedback(&mut feedback, main, no_flags);
        }
        // A layer surface is drawn on its one output.
        for layer in layer_map_for_output(output).layers() {
            layer.with_surfaces(&mut release_barriers);
            layer.send_frame(output, now, FRAME_CALLBACK_THROTTLE, main);
            layer.take_presentation_feedback(&mut feedback, main, no_flags);
        }
        if let Some(surface) = self.caminito.lock.surface_on(output) {
            let surface = surface.wl_surface();
            with_surfaces_surface_tree(surface, &mut release_barriers);
            send_frames_surface_tree(surface, output, now, FRAME_CALLBACK_THROTTLE, main);
            take_presentation_feedback_surface_tree(surface, &mut feedback, main, no_flags);
        }
        // Override-redirect windows have no main output: they're told of frames on
        // every output they're drawn on.
        let output_geometry = crate::output::output_geometry(output);
        for window in self.caminito.override_redirect_windows() {
            let Some(surface) = window.wl_surface() else {
                continue;
            };
            if crate::scene::override_redirect_bbox(window).overlaps(output_geometry) {
                send_frames_surface_tree(&surface, output, now, FRAME_CALLBACK_THROTTLE, |_, _| {
                    Some(output.clone())
                });
            }
        }
        // The drag icon is drawn only on the pointer's output.
        if let Some(icon) = &self.caminito.drag_icon
            && crate::scene::pointer_on(&self.caminito, output).is_some()
        {
            icon.send_frame(output, now, FRAME_CALLBACK_THROTTLE);
        }
        feedback.presented(now, refresh, 0, wp_presentation_feedback::Kind::empty());

        let dh = self.caminito.display_handle.clone();
        for client in blocked.into_values() {
            self.client_compositor_state(&client)
                .blocker_cleared(self, &dh);
        }
    }
}

impl Caminito {
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
                channel::Event::Msg(settings) => state.settings_changed(settings),
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

        let panel = Panel::new(&loop_handle);

        let dh = &display_handle;
        let clock = Clock::new();

        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(dh, "seat0");
        seat.add_pointer();
        let keyboard_settings = &settings.keyboard;
        let (delay, rate) = (
            keyboard_settings.repeat_delay,
            keyboard_settings.repeat_rate,
        );
        let xkb = crate::input::keyboard::xkb_config(&settings.keymap);
        let keyboard = seat
            .add_keyboard(xkb, delay, rate)
            .or_else(|err| {
                tracing::warn!(
                    "can't compile the keymap {:?}: {err}; using xkbcommon's default",
                    settings.keymap
                );
                seat.add_keyboard(XkbConfig::default(), delay, rate)
            })
            .expect("the default xkb keymap compiles");
        crate::input::keyboard::restore_numlock(&keyboard, keyboard_settings);
        let numlock_seen = keyboard.modifier_state().num_lock;
        let primary_selection_state = PrimarySelectionState::new::<State>(dh);
        // The privileged globals (68).
        let privileged = sees_privileged_globals;
        let primary = Some(&primary_selection_state);
        let ext_data_control_state = ExtDataControlState::new::<State, _>(dh, primary, privileged);
        let wlr_data_control_state = WlrDataControlState::new::<State, _>(dh, primary, privileged);
        let xwayland_shell_state = XWaylandShellState::new::<State>(dh);
        let xwayland_keyboard_grab_state = XWaylandKeyboardGrabState::new::<State>(dh);

        let frame_context = FrameContext::new(&settings, theme_dirs);
        let cursors = Cursors::new(&settings.mouse, cursor_source);
        let palette = palette_source(
            event_loop.handle(),
            None,
            palette_cache,
            &settings.interface.gtk_theme,
            StderrTarget::Prefixed,
        );
        let mut caminito = Self {
            loop_signal: event_loop.get_signal(),
            compositor_state: CompositorState::new_v6::<State>(dh),
            shm_state: ShmState::new::<State>(dh, Vec::new()),
            output_manager_state: OutputManagerState::new_with_xdg_output::<State>(dh),
            output_management_state: OutputManagementState::new::<State, _>(dh, privileged),
            seat_state,
            seat,
            data_device_state: DataDeviceState::new::<State>(dh),
            primary_selection_state,
            ext_data_control_state,
            wlr_data_control_state,
            security_context_state: SecurityContextState::new::<State, _>(dh, privileged),

            xdg_shell_state: XdgShellState::new::<State>(dh),
            xdg_activation_state: XdgActivationState::new::<State>(dh),
            xdg_foreign_state: XdgForeignState::new::<State>(dh),
            xdg_dialog_state: XdgDialogState::new::<State>(dh),
            gtk_shell_state: GtkShellState::new::<State>(dh),
            appmenu_state: AppmenuState::default(),
            xdg_toplevel_icon_manager: XdgToplevelIconManager::new::<State>(dh),
            xdg_system_bell_state: XdgSystemBellState::new::<State>(dh),
            xdg_decoration_state: XdgDecorationState::new::<State>(dh),
            kde_decoration_state: KdeDecorationState::new::<State>(dh, KdeDefaultMode::Server),
            popups: PopupManager::default(),
            layer_shell_state: WlrLayerShellState::new_with_filter::<State, _>(dh, privileged),

            foreign_toplevel_state: ForeignToplevelState::new::<State, _>(dh, privileged),
            foreign_toplevel_list_state: ForeignToplevelListState::new_with_filter::<State>(
                dh, privileged,
            ),
            foreign_toplevel_handles: HashMap::new(),
            ext_workspace_state: ExtWorkspaceState::new::<State, _>(dh, privileged),
            xfce_foreign_toplevel_state: XfceForeignToplevelState::new::<State, _>(dh, privileged),

            output_capture_source_state: OutputCaptureSourceState::new_with_filter::<State, _>(
                dh, privileged,
            ),
            toplevel_capture_source_state: ToplevelCaptureSourceState::new_with_filter::<State, _>(
                dh, privileged,
            ),
            image_copy_capture_state: ImageCopyCaptureState::new_with_filter::<State, _>(
                dh, privileged,
            ),
            captures: Vec::new(),

            pointer_constraints_state: PointerConstraintsState::new::<State>(dh),
            relative_pointer_state: RelativePointerManagerState::new::<State>(dh),
            pointer_gestures_state: PointerGesturesState::new::<State>(dh),
            tablet_manager_state: TabletManagerState::new::<State>(dh),
            keyboard_shortcuts_inhibit_state: KeyboardShortcutsInhibitState::new::<State>(dh),
            text_input_state: TextInputManagerState::new::<State>(dh),
            input_method_state: InputMethodManagerState::new::<State, _>(dh, |_| true),
            cursor_shape_state: CursorShapeManagerState::new::<State>(dh),

            session_lock_state: SessionLockManagerState::new::<State, _>(dh, privileged),
            lock: Lock::default(),
            locker: Locker::new(loop_handle.clone()),
            idle_notify_state: IdleNotifyState::new::<State>(dh),
            idle_inhibit_state: IdleInhibitManagerState::new::<State>(dh),
            idle: Idle::new(settings.idle, clock.now().into()),
            deadlines: Deadlines::default(),
            night_light: NightLight::new(settings.night_light),
            night_light_signalled: None,
            gamma_control_state: GammaControlState::default(),
            #[cfg(test)]
            clock_offset: Duration::ZERO,
            #[cfg(test)]
            wall_clock: (TEST_WALL_CLOCK, clock.now().into()),

            viewporter_state: ViewporterState::new::<State>(dh),
            fractional_scale_state: FractionalScaleManagerState::new::<State>(dh),
            presentation_state: PresentationState::new::<State>(dh, clock.id() as u32),
            single_pixel_buffer_state: SinglePixelBufferState::new::<State>(dh),
            content_type_state: ContentTypeState::new::<State>(dh),
            alpha_modifier_state: AlphaModifierState::new::<State>(dh),
            fifo_manager_state: FifoManagerState::new::<State>(dh),
            commit_timing_state: CommitTimingManagerState::new::<State>(dh),

            pointer_focus: None,
            keyboard_layer: None,
            next_layer_id: 0,
            suppressed_keys: HashSet::new(),
            accelerator_grabs: Grabs::default(),
            bells: Bells::default(),
            minimize_animations: MinimizeAnimations::default(),
            accelerator_repeat: None,
            frame_pointer: FramePointer::default(),
            window_grab: None,
            suppressed_buttons: HashSet::new(),
            switcher: None,
            workspace_popup: None,
            client_cursor: CursorImageStatus::default_named(),
            drag_icon: None,
            cursors,
            clock,
            display_handle,
            loop_handle,
            outputs: Vec::new(),
            disabled_outputs: Vec::new(),
            next_output_id: 0,
            output_records: HashMap::new(),
            globals_to_withdraw: Vec::new(),
            primary_choice: None,
            saved_layouts: SavedLayouts::default(),
            monitors_changed: false,
            primary_output_signalled: None,
            showing_desktop_signalled: None,
            global_menu_signalled: None,
            global_menu_switch: Switch::default(),
            app_menu_registrations: Registrations::default(),
            menu_claims: Claims::default(),
            global_menu_offered: false,
            global_menu_offered_signalled: None,
            redraws: HashSet::new(),
            policy: Policy::new(settings.policy.clone()),
            pending: PendingWindows::default(),
            next_window_id: 0,
            palette,
            frame_context,
            input_devices: InputDevices::new(settings.devices),
            numlock_seen,
            accessx: accessx::Filter::new(settings.accessx),
            settings,
            settings_thread,
            written_workspace_count: None,
            xwayland: None,
            xwayland_restart: xwayland::Restart::default(),
            wayland_selections: WaylandSelections::default(),
            xwayland_shell_state,
            xwayland_keyboard_grab_state,
            xwayland_keyboard_grab: None,
            environment: BTreeMap::new(),
            workspace_rename: None,
            socket_source: None,
            helper_stderr: StderrTarget::Prefixed,
            session: None,
            dbus: None,
            logind: None,
            upower: None,
            panel,
            input_serials: InputSerials::default(),
            screensaver: ScreensaverShim::default(),
            keyboard_monitor: KeyboardMonitor::default(),
            debug: DebugToggles::default(),
        };
        caminito.ask_palette_for_theme_colors();
        caminito.start_night_light();
        Ok(caminito)
    }

    /// Writes the workspace count back to `num-workspaces` while `dynamic-workspaces` is on,
    /// as Marco's master does through `_NET_NUMBER_OF_DESKTOPS` (screen.c:1138-1168), so the
    /// pager's preferences show it (97). A change of the key meanwhile is answered with the
    /// count, as policy leaves it unread then.
    fn refresh_num_workspaces(&mut self) {
        let count = self.policy.workspace_count();
        if self.settings.policy.dynamic_workspaces
            && count != self.settings.policy.workspace_count
            && self.written_workspace_count != Some(count)
        {
            self.settings_thread
                .set(MARCO_GENERAL, "num-workspaces", Value::Int(count as i32));
            self.written_workspace_count = Some(count);
        }
    }

    /// Asks the palette helper for the `gtk:custom` colours the theme uses.
    fn ask_palette_for_theme_colors(&mut self) {
        self.palette
            .set_names(self.frame_context.theme().custom_color_names());
    }

    /// New settings from the settings thread: passes on what changed.
    fn settings_changed(&mut self, settings: Settings) {
        if settings.policy.workspace_count != self.settings.policy.workspace_count {
            self.written_workspace_count = None;
        }
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
        self.input_devices.settings_changed(settings.devices);
        self.keyboard_settings_changed(&settings.keyboard);
        self.night_light_settings_changed(settings.night_light);
        let factor = settings.interface.window_scaling_factor;
        let factor_changed = factor != self.settings.interface.window_scaling_factor;
        self.settings = settings;
        if factor_changed {
            self.window_scaling_factor_changed(factor);
        }
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
            let flashing = self.bells.flashes_frame(id);
            let (above, sticky) = (self.policy.is_above(id), self.policy.is_sticky(id));
            let mapped = self.policy.window_mut(id).unwrap();
            changed |=
                mapped.refresh_frame(&mut self.frame_context, colors, flashing, above, sticky);
        }
        if changed {
            self.queue_redraw_all();
        }
    }

    /// Starts `caminito-gtk-palette` as a client on the socket `add_socket` opened.
    pub fn start_palette_helper(&mut self, socket: &OsStr) {
        self.palette = palette_source(
            self.loop_handle.clone(),
            Some(socket),
            self.palette.cache().cloned(),
            &self.settings.interface.gtk_theme,
            self.helper_stderr,
        );
        self.ask_palette_for_theme_colors();
        self.palette.start();
    }

    /// Opens a Wayland socket for clients to connect on, and returns its name.
    pub fn add_socket(&mut self) -> Result<OsString, Box<dyn std::error::Error>> {
        let source = ListeningSocketSource::new_auto()?;
        let name = source.socket_name().to_os_string();
        let token = self
            .loop_handle
            .insert_source(source, |stream, _, state| {
                let client = state
                    .caminito
                    .display_handle
                    .insert_client(stream, Arc::new(ClientState::default()));
                if let Err(err) = client {
                    tracing::warn!("error adding a client: {err}");
                }
            })
            .map_err(|err| err.error)?;
        self.socket_source = Some(token);
        Ok(name)
    }

    /// Closes the socket `add_socket` opened, so no new client connects. Those that
    /// are connected stay until Caminito exits.
    pub(crate) fn close_socket(&mut self) {
        if let Some(token) = self.socket_source.take() {
            self.loop_handle.remove(token);
        }
    }

    /// Quits when SIGTERM or SIGINT comes, so Caminito tears down as it does when its winit
    /// window closes: on the tty, that gives the VT back (08). Nothing else quits a bare
    /// `caminito` on the tty, and a Session has no quit binding either: it ends with its
    /// command, which `quit` stops first (61).
    pub fn stop_on_signals(&mut self) -> std::io::Result<()> {
        let (read, write) = UnixStream::pair()?;
        read.set_nonblocking(true)?;
        write.set_nonblocking(true)?;
        for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
            signal_hook::low_level::pipe::register(signal, write.try_clone()?)?;
        }
        let source = Generic::new(read, Interest::READ, Mode::Level);
        self.loop_handle
            .insert_source(source, |_, _, state| {
                tracing::debug!("quitting on a signal");
                state.caminito.quit();
                Ok(PostAction::Remove)
            })
            .map_err(|err| std::io::Error::other(err.error))?;
        Ok(())
    }

    /// The time policy is given as `now`.
    pub fn now(&self) -> Duration {
        let now: Duration = self.clock.now().into();
        #[cfg(test)]
        let now = now + self.clock_offset;
        now
    }

    /// The wall clock, in Unix seconds, for what follows the sun (138). The tests' is
    /// `wall_clock`.
    pub(crate) fn unix_now(&self) -> f64 {
        #[cfg(test)]
        {
            let (unix, at) = self.wall_clock;
            unix + self.now().saturating_sub(at).as_secs_f64()
        }
        #[cfg(not(test))]
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64()
    }

    /// Sends the outward views of policy state whatever changed since they last sent.
    fn refresh_views(&mut self) {
        // First, since windows' outputs and scales follow the outputs' scales.
        self.refresh_lid_switch();
        // Before the work areas, which take the struts as read at the X11 scale, and
        // before anything else that's worked out at it.
        self.refresh_x11_scale();
        // Before the windows are told what policy asks of them.
        self.refresh_work_areas();
        self.refresh_window_outputs();
        self.refresh_layers();
        self.refresh_output_management();
        self.refresh_primary_output_property();
        self.refresh_showing_desktop_property();
        self.refresh_night_light_property();
        self.refresh_global_menu();
        self.refresh_foreign_toplevels();
        self.refresh_foreign_toplevel_list();
        // On both sides of ext-workspace's refresh: a window leaves a workspace that's
        // going while a client still has that workspace's handle, and enters a new one
        // once the client has that one's.
        self.refresh_toplevel_workspaces();
        self.refresh_ext_workspace();
        self.refresh_toplevel_workspaces();
        self.refresh_num_workspaces();
        // After foreign-toplevel and ext-workspace, whose leaves name the `wl_output`s.
        self.refresh_output_globals();
        for mapped in self.policy.windows() {
            mapped.flush();
        }
        self.refresh_x11_stacking();
        self.refresh_x_root();
        self.refresh_x_windows();
        self.refresh_xsettings();
        self.refresh_x_resources();
        self.refresh_x_primary_output();
        self.refresh_x_root_cursor();
        self.refresh_numlock();
        self.popups.cleanup();
    }

    /// The bounds a new toplevel is told to fit in: the work area of the focused window's
    /// output, where policy places new windows, or else the primary output's.
    pub fn placement_bounds(&self) -> Option<Size<i32, Logical>> {
        let focused = self
            .policy
            .focus(SEAT)
            .and_then(|id| self.policy.geometry(id));
        let areas: Vec<_> = self.policy.work_areas().collect();
        let outputs = self.policy.outputs();
        focused
            .and_then(|geometry| outputs.iter().position(|o| o.geometry.overlaps(geometry)))
            .or_else(|| {
                let primary = output_id(self.primary_output()?);
                outputs.iter().position(|o| o.id == primary)
            })
            .map(|output| areas[output].size)
    }
}
