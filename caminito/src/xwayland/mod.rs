//! Xwayland (08, 11, 15): spawned by Smithay at start-up in every mode, eagerly, with
//! Smithay's `X11Wm` attached once it's ready. If it can't start, Caminito logs it once and
//! runs without X. Once it has been ready, `restart` brings it back on the same display
//! when it goes (42). Its windows are managed through `xwm`, and X's
//! stacking order follows Caminito's scene. Caminito's own X connection, `xconn`, publishes the
//! root properties Smithay doesn't and runs the root messages it drops. Caminito is the
//! XSETTINGS manager, through `X11Wm` (40). Every X11 window has one scale, `scale` (41).

mod restart;
mod scale;
mod selection;
mod stack;
mod xconn;
mod xsettings;
mod xwm;

use std::ffi::OsString;
use std::os::unix::net::UnixStream;
use std::process::Stdio;

use caminito_policy::{Action, FrameState, MAX_WORKSPACES, PolicyWindow, SizeHints};
use smithay::backend::input::InputTime;
use smithay::reexports::calloop::RegistrationToken;
use smithay::reexports::wayland_server::Client;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::x11rb::errors::ConnectionError;
use smithay::utils::{FrameExtents, Logical, SERIAL_COUNTER};
use smithay::xwayland::xwm::X11Window;
use smithay::xwayland::xwm::settings::Value as XSettingValue;
use smithay::xwayland::{X11Surface, X11Wm, XWayland, XWaylandEvent};

pub(crate) use self::restart::Restart;
#[cfg(test)]
pub(crate) use self::restart::process;
use self::scale::X11Scale;
pub(crate) use self::selection::WaylandSelections;
use self::stack::RootStack;
use self::xconn::{Request, RootProperties, WindowProperties, WindowProperty, XConnection};
use crate::bell::{BellFor, BellSource};
use crate::decoration::{NetWmIcon, WindowFacts};
use crate::helpers;
use crate::settings::Value;
use crate::settings::translate::{MARCO_GENERAL, Mouse};
use crate::state::{Caminito, SEAT, State};
use crate::window::{STICKY, WindowKey, marco_type, request_skips, with_x_facts};
use crate::work_area::screen_work_area;

/// One of Smithay's setters for a state in `_NET_WM_STATE`.
type SetState = fn(&X11Surface, bool) -> Result<(), ConnectionError>;

/// The identifier Xwayland's stderr is logged under.
const IDENTIFIER: &str = "Xwayland";

/// A spawned Xwayland.
pub struct RunningXwayland {
    /// The display it serves, `:n`.
    pub display: u32,
    /// Its Wayland connection to Caminito.
    client: Client,
    /// Smithay's window manager, from `Ready` on.
    pub wm: Option<X11Wm>,
    /// Caminito's own X connection (11), from `Ready` on, if it connected.
    xconn: Option<XConnection>,
    /// The XSETTINGS table `wm` publishes (40). Smithay's starts empty.
    xsettings: xsettings::Table,
    /// The X11 scale `client` was given (41).
    scale: X11Scale,
    /// The cursor theme, size and X11 scale of the root window's cursor, once `wm` has set
    /// it.
    root_cursor: Option<(String, u32, X11Scale)>,
    /// The XWayland windows in the order `X11Wm` was last told to stack them, bottom to
    /// top.
    stacking: Vec<X11Window>,
    /// X's stacking order of the root's children, which override-redirect windows are drawn
    /// in.
    root_stack: RootStack,
    /// The mapped override-redirect windows (36), in the order they mapped.
    override_redirect: Vec<X11Surface>,
    /// Wayland selections X is told of when one of its windows next takes the keyboard.
    owed_selections: Vec<selection::Owed>,
    /// The `XWayland` source, which kills Xwayland when it's removed.
    source: RegistrationToken,
}

/// Sends what `wm` has buffered. Smithay's setters, like `set_xsettings`
/// (`xwm/mod.rs:1372-1386`) and `set_cursor`, leave their writes in the connection's buffer,
/// which it flushes only when X next sends it an event. A restack flushes whatever it's
/// given, and one of no windows changes nothing (`:1110-1165`).
fn flush_wm(wm: &mut X11Wm) -> Result<(), ConnectionError> {
    wm.update_stacking_order_upwards(std::iter::empty::<&X11Surface>())
}

/// What Xwayland is spawned with besides `PATH` and `XDG_RUNTIME_DIR`, since Smithay clears
/// the rest of its environment (11): the cursor its root window shows.
fn envs(mouse: &Mouse) -> Vec<(&'static str, String)> {
    let mut envs = vec![("XCURSOR_SIZE", mouse.cursor_size.to_string())];
    if !mouse.cursor_theme.is_empty() {
        envs.push(("XCURSOR_THEME", mouse.cursor_theme.clone()));
    }
    envs
}

impl Caminito {
    /// Spawns Xwayland on a free display, or on the one it had if it's a restart. It's
    /// ready once `Ready` has attached `X11Wm`.
    pub fn start_xwayland(&mut self) {
        let stderr = match helpers::stderr_to(&self.loop_handle, IDENTIFIER, self.helper_stderr) {
            Ok(stderr) => stderr,
            Err(err) => {
                tracing::warn!("{IDENTIFIER}: can't pipe its stderr, so it's dropped: {err}");
                Stdio::null()
            }
        };
        self.xwayland_restart.release_display_lock();
        let spawned = XWayland::spawn(
            &self.display_handle,
            self.xwayland_restart.display(),
            envs(&self.settings.mouse),
            std::iter::empty::<&str>(),
            true,
            Stdio::null(),
            stderr,
            |_| {},
        );
        let (xwayland, client) = match spawned {
            Ok(spawned) => spawned,
            Err(err) => {
                self.xwayland_exited(format_args!("can't start Xwayland: {err}"));
                return;
            }
        };
        let display = xwayland.display_number();
        // Given before Xwayland binds the outputs, so its screen starts in client space.
        let scale = X11Scale(self.x11_scale());
        scale.apply(&client);
        let source = self
            .loop_handle
            .insert_source(xwayland, |event, _, state| match event {
                XWaylandEvent::Ready { x11_socket, .. } => state.xwayland_ready(x11_socket),
                XWaylandEvent::Error => {
                    state
                        .caminito
                        .xwayland_exited("Xwayland exited while starting");
                }
            });
        match source {
            Ok(source) => {
                self.xwayland = Some(RunningXwayland {
                    display,
                    client,
                    wm: None,
                    xconn: None,
                    xsettings: xsettings::Table::new(),
                    scale,
                    root_cursor: None,
                    stacking: Vec::new(),
                    root_stack: RootStack::default(),
                    override_redirect: Vec::new(),
                    // A new Xwayland knows none of them, and a restarted one has forgotten.
                    owed_selections: self.wayland_selections.owed(),
                    source,
                })
            }
            Err(err) => {
                self.xwayland_exited(format_args!("can't watch Xwayland: {}", err.error));
            }
        }
    }

    /// Xwayland's display, `:n`, once it's ready for clients.
    pub fn xwayland_display(&self) -> Option<u32> {
        let xwayland = self.xwayland.as_ref()?;
        xwayland.wm.is_some().then_some(xwayland.display)
    }

    /// The override-redirect windows Caminito shows, top to bottom in X's stacking order.
    /// They're in global logical coordinates, on no workspace, and policy doesn't know them.
    pub(crate) fn override_redirect_windows(&self) -> impl Iterator<Item = &X11Surface> {
        let mut windows: Vec<&X11Surface> = Vec::new();
        if let Some(xwayland) = &self.xwayland {
            windows.extend(&xwayland.override_redirect);
            let stack = &xwayland.root_stack;
            windows.sort_by_key(|window| std::cmp::Reverse(stack.index(window.window_id())));
        }
        windows.into_iter()
    }

    /// The override-redirect window Caminito shows whose surface is `surface`.
    pub(crate) fn override_redirect_for_surface(&self, surface: &WlSurface) -> Option<&X11Surface> {
        self.override_redirect_windows()
            .find(|window| window.wl_surface().as_ref() == Some(surface))
    }

    /// Smithay's window manager, once Xwayland is ready.
    pub fn xwm(&mut self) -> Option<&mut X11Wm> {
        self.xwayland.as_mut()?.wm.as_mut()
    }

    /// Caminito's scene is the truth (11): after a restack that involves an XWayland window,
    /// X's stacking order follows it, so that X clients and `_NET_CLIENT_LIST_STACKING` see
    /// what the user sees.
    pub(crate) fn refresh_x11_stacking(&mut self) {
        let policy = &self.policy;
        // Bottom to top: Smithay moves each window above the one before it, whatever its
        // doc comment says. Windows on other workspaces aren't in the scene, and go below.
        // A sticky window is on the active workspace, and goes there once (155).
        let on_active = policy.stacking(policy.active_workspace());
        let others = policy
            .stacking_all()
            .iter()
            .copied()
            .filter(|id| !on_active.contains(id));
        let order: Vec<X11Surface> = others
            .chain(on_active.iter().copied())
            .filter_map(|id| policy.window(id)?.window().x11_surface().cloned())
            .collect();
        let ids: Vec<X11Window> = order.iter().map(X11Surface::window_id).collect();
        let Some(xwayland) = self.xwayland.as_mut() else {
            return;
        };
        let Some(wm) = xwayland.wm.as_mut() else {
            return;
        };
        if xwayland.stacking == ids {
            return;
        }
        match wm.update_stacking_order_upwards(order.iter()) {
            Ok(()) => xwayland.stacking = ids,
            Err(err) => tracing::debug!("can't restack X windows: {err}"),
        }
    }

    /// Brings the root properties Caminito publishes up to date with policy (11).
    pub(crate) fn refresh_x_root(&mut self) {
        let properties = self.root_properties();
        self.with_xconn("publish the root properties", |xconn| {
            xconn.publish(properties)
        });
    }

    /// Brings RandR's primary output, which X11 apps place themselves by, up to date with
    /// the primary output (81).
    pub(crate) fn refresh_x_primary_output(&mut self) {
        let name = self.primary_output().map(|output| output.name());
        self.with_xconn("set RandR's primary output", |xconn| {
            xconn.publish_primary_output(name.as_deref())
        });
    }

    /// Brings XSETTINGS up to date with the settings (40), sending only what changed.
    pub(crate) fn refresh_xsettings(&mut self) {
        let Some(xwayland) = self.xwayland.as_mut() else {
            return;
        };
        let Some(wm) = xwayland.wm.as_mut() else {
            return;
        };
        let table = xsettings::table(&self.settings, xwayland.scale.0);
        let changed: Vec<(String, XSettingValue)> = table
            .iter()
            .filter(|(name, setting)| xwayland.xsettings.get(*name) != Some(*setting))
            .map(|(name, setting)| (name.to_string(), setting.to_smithay()))
            .collect();
        if changed.is_empty() {
            return;
        }
        if let Err(err) = wm.set_xsettings(changed.into_iter()) {
            tracing::warn!("can't publish XSETTINGS: {err}");
            return;
        }
        // Smithay has the table now, and sends it with its next write if this fails.
        xwayland.xsettings = table;
        if let Err(err) = flush_wm(wm) {
            tracing::warn!("can't send XSETTINGS to X: {err}");
        }
    }

    /// Brings the Xft resources in `RESOURCE_MANAGER` up to date with the settings and the
    /// X11 scale (47), writing only when they changed.
    pub(crate) fn refresh_x_resources(&mut self) {
        let scale = self.applied_x11_scale();
        let resources = xsettings::resources(&self.settings, scale.0);
        self.with_xconn("publish the Xft resources", |xconn| {
            xconn.publish_resources(resources)
        });
    }

    /// Brings the root window's cursor, which X windows without their own show, up to date
    /// with the cursor settings and the X11 scale (41). Xwayland's own, from
    /// `XCURSOR_SIZE`, would be in client space, and small at a scale above 1.
    pub(crate) fn refresh_x_root_cursor(&mut self) {
        let Some(xwayland) = self.xwayland.as_mut() else {
            return;
        };
        let Some(wm) = xwayland.wm.as_mut() else {
            return;
        };
        let mouse = &self.settings.mouse;
        let wanted = (
            mouse.cursor_theme.clone(),
            mouse.cursor_size,
            xwayland.scale,
        );
        if xwayland.root_cursor.as_ref() == Some(&wanted) {
            return;
        }
        let image = self.cursors.x_root_cursor(xwayland.scale.0);
        if let Err(err) = wm.set_cursor(&image.pixels, image.size, image.hotspot) {
            tracing::warn!("can't set X's root cursor: {err}");
            return;
        }
        xwayland.root_cursor = Some(wanted);
        if let Err(err) = flush_wm(wm) {
            tracing::warn!("can't send X's root cursor: {err}");
        }
    }

    /// Brings the properties Caminito publishes on each XWayland window up to date with
    /// policy (39).
    pub(crate) fn refresh_x_windows(&mut self) {
        let policy = &self.policy;
        let scale = self.applied_x11_scale();
        let windows: Vec<(X11Window, WindowProperties)> = policy
            .windows()
            .filter_map(|mapped| {
                let window = mapped.x11_window()?;
                let id = mapped.id();
                let desktop = if policy.is_sticky(id) {
                    STICKY
                } else {
                    policy.workspace_of(id)? as u32
                };
                let frame_extents = scale.extents_to_x(policy.frame_extents(id));
                Some((
                    window,
                    WindowProperties {
                        desktop,
                        frame_extents,
                    },
                ))
            })
            .collect();
        self.with_xconn("publish X windows' properties", |xconn| {
            xconn.publish_windows(windows)
        });
        self.refresh_x_states();
    }

    /// Tells each XWayland window the states policy keeps that X shows in `_NET_WM_STATE`
    /// and `WM_STATE` (35): minimised as hidden and iconic, and the above, below and sticky
    /// states policy records. They're Smithay's properties, so they go through its API,
    /// only when they differ from what it last wrote.
    fn refresh_x_states(&self) {
        let policy = &self.policy;
        for mapped in policy.windows() {
            let Some(surface) = mapped.window().x11_surface() else {
                continue;
            };
            let id = mapped.id();
            let states: [(bool, bool, SetState); 4] = [
                (
                    policy.is_minimized(id),
                    surface.is_hidden(),
                    X11Surface::set_hidden,
                ),
                (
                    policy.is_above(id),
                    surface.is_above(),
                    X11Surface::set_above,
                ),
                (
                    policy.is_below(id),
                    surface.is_below(),
                    X11Surface::set_below,
                ),
                (
                    policy.is_sticky(id),
                    surface.is_sticky(),
                    X11Surface::set_sticky,
                ),
            ];
            for (wanted, told, tell) in states {
                if wanted != told
                    && let Err(err) = tell(surface, wanted)
                {
                    tracing::debug!("can't set an X window's state: {err}");
                }
            }
        }
    }

    /// Runs a message X has sent that only needs Caminito's state.
    fn x_request(&mut self, request: Request) {
        match request {
            // Marco ignores a desktop it doesn't have.
            Request::CurrentDesktop(n) if n < self.policy.workspace_count() => {
                let now = self.now();
                self.policy.action(SEAT, Action::SwitchToWorkspace(n), now);
                self.queue_redraw_all();
            }
            Request::CurrentDesktop(n) => {
                tracing::debug!("an X client asked for desktop {n}, which isn't one");
            }
            // Marco ignores 0. GSettings would refuse more than the schema's most.
            Request::NumberOfDesktops(n) if n > 0 => {
                let n = n.min(MAX_WORKSPACES) as i32;
                self.settings_thread
                    .set(MARCO_GENERAL, "num-workspaces", Value::Int(n));
            }
            Request::NumberOfDesktops(_) => {}
            Request::Desktop { window, desktop } => self.x_desktop_request(window, desktop),
            Request::FrameExtents { window } => self.x_frame_extents_request(window),
            // Marco takes the focused window for an X window it doesn't manage
            // (`bell.c:meta_bell_notify`); the bell event takes none, as it does for a
            // surface that isn't a window's (105).
            Request::Bell { window } => {
                let bell_for = match window {
                    Some(window) => BellFor::Window(self.mapped_for_key(&WindowKey::X11(window))),
                    None => BellFor::Focus,
                };
                self.ring_bell(BellSource::Xkb, bell_for);
            }
            Request::Close { window } => {
                if let Some(id) = self.mapped_for_key(&WindowKey::X11(window)) {
                    self.policy.close(id);
                }
            }
            Request::MoveResize {
                window,
                gravity,
                loc,
                size,
            } => {
                if let Some(surface) = self.managed_x11_surface(window) {
                    // Smithay converts a `ConfigureRequest`'s values, but not these.
                    let scale = self.applied_x11_scale();
                    let loc_to_logical = |n: Option<i32>| n.map(|n| scale.to_logical(n));
                    let size_to_logical =
                        |n: Option<u32>| n.map(|n| scale.to_logical(n as i32).max(1) as u32);
                    let loc = (loc_to_logical(loc.0), loc_to_logical(loc.1));
                    let size = (size_to_logical(size.0), size_to_logical(size.1));
                    self.x_configure_request(&surface, gravity, loc, size);
                }
            }
            Request::Skips {
                window,
                action,
                taskbar,
                pager,
            } => {
                if let Some(surface) = self.known_x11_surface(window) {
                    request_skips(&surface, action, taskbar, pager);
                }
            }
            Request::InputShape { window, shaped } => {
                let Some(surface) = self.known_x11_surface(window) else {
                    return;
                };
                if shaped {
                    self.read_input_shape(&surface);
                } else {
                    with_x_facts(&surface, |facts| facts.input_shape = None);
                }
            }
            Request::KeyboardMoveResize { .. }
            | Request::CancelMoveResize { .. }
            | Request::ShowWindowMenu { .. } => {
                unreachable!("State::x_requests runs the grabs and the window menu")
            }
        }
    }

    /// `_NET_REQUEST_FRAME_EXTENTS` (11). A managed window's are policy's, which the
    /// refresh pass writes again. Any other window isn't in policy, so the answer is
    /// written here: the frame it would map with, focused, from the theme and what it says
    /// of itself, which it waits for before it maps.
    fn x_frame_extents_request(&mut self, window: X11Window) {
        if self.mapped_for_key(&WindowKey::X11(window)).is_some() {
            if let Some(xconn) = self.xwayland.as_mut().and_then(|x| x.xconn.as_mut()) {
                xconn.republish_frame_extents(window);
            }
            return;
        }
        let request = self.with_xconn("read an X window's frame hints", |xconn| {
            xconn.frame_request(window)
        });
        let Some(Some(request)) = request else {
            return;
        };
        let has_parent = request
            .transient_for
            .is_some_and(|parent| self.mapped_for_key(&WindowKey::X11(parent)).is_some());
        let facts = WindowFacts {
            window_type: marco_type(request.window_type, request.transient_for.is_some(), false),
            has_parent,
            hints: SizeHints::default(),
            // A fullscreen window shows no frame.
            wants_frame: request.decorated && !request.fullscreen,
        };
        let state = FrameState {
            maximized: request.maximized,
            focused: true,
            ..FrameState::default()
        };
        let extents = self
            .applied_x11_scale()
            .extents_to_x(self.frame_context.extents(&facts, state));
        self.with_xconn("answer an X window's frame request", |xconn| {
            xconn.answer_frame_extents(window, extents)
        });
    }

    /// The XWayland window whose X window is `window`.
    fn managed_x11_surface(&self, window: X11Window) -> Option<X11Surface> {
        let id = self.mapped_for_key(&WindowKey::X11(window))?;
        self.policy.window(id)?.window().x11_surface().cloned()
    }

    /// `_NET_WM_DESKTOP` for a managed window, as Marco takes it: every desktop makes it
    /// sticky, and one it has moves it there, with its transient family (window.c's
    /// `meta_window_client_message`).
    fn x_desktop_request(&mut self, window: X11Window, desktop: u32) {
        let Some(id) = self.mapped_for_key(&WindowKey::X11(window)) else {
            return;
        };
        let policy = &mut self.policy;
        if desktop == STICKY {
            policy.set_sticky(id, true);
        } else if (desktop as usize) < policy.workspace_count() {
            policy.set_sticky(id, false);
            policy.move_to_workspace(id, desktop as usize);
        }
        self.queue_redraw_all();
    }

    /// Runs `use_it` on Caminito's X connection, if there is one. A connection that fails is
    /// closed, and X clients see no more changes.
    fn with_xconn<T>(
        &mut self,
        what: &str,
        use_it: impl FnOnce(&mut XConnection) -> Result<T, xconn::Error>,
    ) -> Option<T> {
        let xwayland = self.xwayland.as_mut()?;
        match use_it(xwayland.xconn.as_mut()?) {
            Ok(value) => Some(value),
            Err(err) => {
                tracing::warn!("can't {what}, so Caminito's X connection is closed: {err}");
                xwayland.xconn = None;
                None
            }
        }
    }

    /// Reads what Caminito's X connection knows of a window Smithay doesn't, as it asks to
    /// map (39).
    pub(crate) fn read_x_facts(&mut self, window: &X11Surface) {
        let id = window.window_id();
        let desktop = self
            .with_xconn("read an X window's desktop", |xconn| xconn.desktop(id))
            .flatten();
        // Skips asked after an earlier map are forgotten: `_NET_WM_STATE` says them now.
        with_x_facts(window, |facts| {
            facts.desktop = desktop;
            facts.skips = None;
        });
        self.read_x_property(window, WindowProperty::Icon);
        self.read_x_property(window, WindowProperty::Struts);
        // Watched first, so a change after it's read isn't missed.
        self.with_xconn("watch an X window's input region", |xconn| {
            xconn.watch_shape(id)
        });
        self.read_input_shape(window);
    }

    /// Reads a window's input region again.
    pub(super) fn read_input_shape(&mut self, window: &X11Surface) {
        let id = window.window_id();
        let shape = self
            .with_xconn("read an X window's input region", |xconn| {
                xconn.input_shape(id)
            })
            .flatten();
        let scale = self.applied_x11_scale();
        let shape = shape.map(|rects| {
            rects
                .into_iter()
                .map(|r| scale.rect_to_logical(r))
                .collect()
        });
        with_x_facts(window, |facts| facts.input_shape = shape);
    }

    /// The pending, managed or override-redirect window whose X window is `window`.
    fn known_x11_surface(&self, window: X11Window) -> Option<X11Surface> {
        self.window_for_key(&WindowKey::X11(window))
            .and_then(|known| known.x11_surface())
            .or_else(|| {
                self.override_redirect_windows()
                    .find(|surface| surface.window_id() == window)
            })
            .cloned()
    }

    /// The frame extents Caminito's X connection last published on an XWayland window.
    pub(crate) fn published_frame_extents(
        &self,
        window: X11Window,
    ) -> Option<FrameExtents<i32, Logical>> {
        let xconn = self.xwayland.as_ref()?.xconn.as_ref()?;
        xconn.published_frame_extents(window)
    }

    /// Reads one of the window properties Caminito keeps again.
    fn read_x_property(&mut self, window: &X11Surface, property: WindowProperty) {
        let id = window.window_id();
        match property {
            WindowProperty::Icon => {
                let icon = self
                    .with_xconn("read an X window's icon", |xconn| xconn.icon(id))
                    .flatten()
                    .and_then(|value| NetWmIcon::parse(&value));
                with_x_facts(window, |facts| facts.icon = icon);
            }
            WindowProperty::Struts => {
                let scale = self.applied_x11_scale();
                let struts = self
                    .with_xconn("read an X window's struts", |xconn| xconn.struts(id))
                    .unwrap_or_default()
                    .into_iter()
                    .map(|strut| scale.strut_to_logical(strut))
                    .collect();
                with_x_facts(window, |facts| facts.struts = struts);
            }
        }
    }

    /// A property Smithay doesn't parse changed on a window, pending or managed. Returns
    /// whether it's one Caminito reads, which it has read again.
    pub(crate) fn x_property_changed(&mut self, window: &X11Surface, atom: u32) -> bool {
        let property = self
            .xwayland
            .as_ref()
            .and_then(|xwayland| xwayland.xconn.as_ref())
            .and_then(|xconn| xconn.window_property(atom));
        let Some(property) = property else {
            return false;
        };
        self.read_x_property(window, property);
        true
    }

    /// The X11 scale Xwayland's client has, which X client space is at.
    fn applied_x11_scale(&self) -> X11Scale {
        self.xwayland
            .as_ref()
            .map_or(X11Scale(1), |xwayland| xwayland.scale)
    }

    fn root_properties(&self) -> RootProperties {
        let scale = self.applied_x11_scale();
        RootProperties {
            desktop_names: self.policy.workspace_names().to_vec(),
            current_desktop: self.policy.active_workspace(),
            work_areas: self
                .policy
                .work_areas()
                .map(|area| scale.to_x(area))
                .collect(),
            screen_work_area: scale.to_x(screen_work_area(self.policy.outputs())),
        }
    }

    /// Drops Xwayland and its window manager, which ends it.
    fn stop_xwayland(&mut self) {
        if let Some(xwayland) = self.xwayland.take() {
            self.loop_handle.remove(xwayland.source);
        }
    }

    /// Waits until Xwayland has handled everything Caminito's X connection has sent, for a
    /// test client that reads what it wrote: Xwayland orders requests per connection only.
    #[cfg(test)]
    pub(crate) fn sync_x_connection_in_test(&mut self) {
        self.with_xconn("sync with Xwayland", XConnection::sync);
    }

    /// Stops Xwayland for a test fixture that's being dropped, which gives its display's
    /// lock back. Xwayland itself outlives it; see the fixture's `Drop`.
    #[cfg(test)]
    pub(crate) fn stop_xwayland_in_test(&mut self) {
        self.stop_xwayland();
    }
}

impl State {
    /// Runs the messages X has sent as Caminito's own bindings and the pager would (11, 39).
    pub(crate) fn x_requests(&mut self) {
        let requests = self
            .caminito
            .with_xconn("read X's messages", XConnection::requests);
        for request in requests.into_iter().flatten() {
            match request {
                Request::KeyboardMoveResize { window, resize } => {
                    self.x_keyboard_move_resize(window, resize);
                }
                Request::CancelMoveResize { window } => self.x_cancel_move_resize(window),
                Request::ShowWindowMenu { window, x, y } => self.x_show_window_menu(window, x, y),
                request => self.caminito.x_request(request),
            }
        }
    }

    /// `_NET_WM_MOVERESIZE`'s keyboard move (10) or resize (9): the keyboard grab of
    /// `begin-move` and `begin-resize` (156), as Marco's `meta_window_begin_grab_op` starts
    /// for them (window.c:5415-5450). The refresh pass holds the pointer for it.
    fn x_keyboard_move_resize(&mut self, window: X11Window, resize: bool) {
        let caminito = &mut self.caminito;
        let Some(id) = caminito.mapped_for_key(&WindowKey::X11(window)) else {
            return;
        };
        if caminito.lock.is_locked() {
            return;
        }
        if resize {
            caminito.policy.begin_keyboard_resize(SEAT, id);
        } else {
            caminito.policy.begin_keyboard_move(SEAT, id);
        }
        caminito.queue_redraw_all();
    }

    /// `_GTK_SHOW_WINDOW_MENU`: an X client that draws its own titlebar asks for the
    /// window menu at a point of the root, as GTK3 does for a right click on a header bar.
    /// Marco raises the window first with `raise-on-click` (window.c:5611-5624).
    fn x_show_window_menu(&mut self, window: X11Window, x: i32, y: i32) {
        let caminito = &mut self.caminito;
        let Some(id) = caminito.mapped_for_key(&WindowKey::X11(window)) else {
            return;
        };
        let scale = caminito.applied_x11_scale();
        let at = (scale.to_logical(x), scale.to_logical(y)).into();
        if caminito.settings.policy.raise_on_click {
            caminito.policy.raise(id);
        }
        let time = InputTime::from_micros(caminito.now().as_micros() as u64);
        self.open_window_menu(id, at, time);
    }

    /// `_NET_WM_MOVERESIZE`'s cancel: ends the move or resize of the window, if one is in
    /// progress, where it is, as Marco's `meta_display_end_grab_op` does.
    fn x_cancel_move_resize(&mut self, window: X11Window) {
        let caminito = &self.caminito;
        let Some(id) = caminito.mapped_for_key(&WindowKey::X11(window)) else {
            return;
        };
        if caminito.policy.grab(SEAT) != Some(id) {
            return;
        }
        let pointer = caminito.seat.get_pointer().unwrap();
        let time = InputTime::from_micros(caminito.now().as_micros() as u64);
        pointer.unset_grab(self, SERIAL_COUNTER.next_serial(), time);
    }

    /// Xwayland is up. In 11's start-up order: attaches `X11Wm`, connects Caminito's X
    /// connection, which claims Caminito's identity, publishes the root properties,
    /// XSETTINGS, the Xft resources and the root cursor, all at the X11 scale Xwayland's
    /// client has, and only then sets `DISPLAY` for the clients Caminito spawns, which read
    /// them at start-up.
    fn xwayland_ready(&mut self, x11_socket: UnixStream) {
        let caminito = &mut self.caminito;
        let Some(xwayland) = caminito.xwayland.as_mut() else {
            return;
        };
        let started = X11Wm::start_wm(
            caminito.loop_handle.clone(),
            &caminito.display_handle,
            x11_socket,
            xwayland.client.clone(),
        );
        match started {
            Ok(wm) => {
                xwayland.wm = Some(wm);
                caminito.xwayland_restart.ready(xwayland.display);
                // Each `start_wm` claims XSETTINGS with an empty table (ADR 0006), and has
                // the root window's cursor Xwayland made.
                xwayland.xsettings.clear();
                xwayland.root_cursor = None;
                match XConnection::connect(xwayland.display, &caminito.loop_handle) {
                    Ok(xconn) => xwayland.xconn = Some(xconn),
                    Err(err) => tracing::warn!(
                        "can't make Caminito's X connection, so X clients see no desktops: {err}"
                    ),
                }
                let value = format!(":{}", xwayland.display);
                // Published here rather than left to the refresh pass, since X clients read
                // them as they start (11): `DISPLAY` must not come first.
                caminito.refresh_x_root();
                caminito.refresh_xsettings();
                caminito.refresh_x_resources();
                caminito.refresh_x_root_cursor();
                tracing::info!("Xwayland is ready on DISPLAY={value}");
                caminito
                    .environment
                    .insert(OsString::from("DISPLAY"), OsString::from(value));
            }
            Err(err) => {
                caminito.xwayland_exited(format_args!("can't manage Xwayland's windows: {err}"));
            }
        }
    }
}
