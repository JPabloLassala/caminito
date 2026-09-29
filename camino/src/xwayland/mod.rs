//! Xwayland (08, 11, 15): spawned by Smithay at start-up in every mode, eagerly, with
//! Smithay's `X11Wm` attached once it's ready. If it can't start, Camino logs it once and
//! runs without X; the restart path is 42's. Its windows are managed through `xwm`, and X's
//! stacking order follows Camino's scene. Camino's own X connection, `xconn`, publishes the
//! root properties Smithay doesn't and runs the root messages it drops.

mod selection;
mod stack;
mod xconn;
mod xwm;

use std::ffi::OsString;
use std::os::unix::net::UnixStream;
use std::process::Stdio;

use camino_policy::{Action, MAX_WORKSPACES};
use smithay::reexports::calloop::RegistrationToken;
use smithay::reexports::wayland_server::Client;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::xwayland::xwm::X11Window;
use smithay::xwayland::{X11Surface, X11Wm, XWayland, XWaylandEvent};

use self::stack::RootStack;
use self::xconn::{RootProperties, RootRequest, XConnection};
use crate::helpers::{self, StderrTarget};
use crate::settings::Value;
use crate::settings::translate::{MARCO_GENERAL, Mouse};
use crate::state::{Camino, SEAT, State};

/// The identifier Xwayland's stderr is logged under.
const IDENTIFIER: &str = "Xwayland";

/// A spawned Xwayland.
pub struct RunningXwayland {
    /// The display it serves, `:n`.
    pub display: u32,
    /// Its Wayland connection to Camino.
    client: Client,
    /// Smithay's window manager, from `Ready` on.
    pub wm: Option<X11Wm>,
    /// Camino's own X connection (11), from `Ready` on, if it connected.
    xconn: Option<XConnection>,
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

/// What Xwayland is spawned with besides `PATH` and `XDG_RUNTIME_DIR`, since Smithay clears
/// the rest of its environment (11): the cursor its root window shows.
fn envs(mouse: &Mouse) -> Vec<(&'static str, String)> {
    let mut envs = vec![("XCURSOR_SIZE", mouse.cursor_size.to_string())];
    if !mouse.cursor_theme.is_empty() {
        envs.push(("XCURSOR_THEME", mouse.cursor_theme.clone()));
    }
    envs
}

impl Camino {
    /// Spawns Xwayland on a free display. It's ready once `Ready` has attached `X11Wm`.
    pub fn start_xwayland(&mut self) {
        let stderr = match helpers::stderr_to(&self.loop_handle, IDENTIFIER, StderrTarget::Prefixed)
        {
            Ok(stderr) => stderr,
            Err(err) => {
                tracing::warn!("{IDENTIFIER}: can't pipe its stderr, so it's dropped: {err}");
                Stdio::null()
            }
        };
        let spawned = XWayland::spawn(
            &self.display_handle,
            None,
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
                tracing::warn!("can't start Xwayland, so Camino runs without X: {err}");
                return;
            }
        };
        let display = xwayland.display_number();
        let source = self
            .loop_handle
            .insert_source(xwayland, |event, _, state| match event {
                XWaylandEvent::Ready { x11_socket, .. } => state.xwayland_ready(x11_socket),
                XWaylandEvent::Error => {
                    tracing::warn!("Xwayland exited while starting, so Camino runs without X");
                    state.camino.stop_xwayland();
                }
            });
        match source {
            Ok(source) => {
                self.xwayland = Some(RunningXwayland {
                    display,
                    client,
                    wm: None,
                    xconn: None,
                    stacking: Vec::new(),
                    root_stack: RootStack::default(),
                    override_redirect: Vec::new(),
                    owed_selections: Vec::new(),
                    source,
                })
            }
            Err(err) => {
                tracing::warn!(
                    "can't watch Xwayland, so Camino runs without X: {}",
                    err.error
                )
            }
        }
    }

    /// Xwayland's display, `:n`, once it's ready for clients.
    pub fn xwayland_display(&self) -> Option<u32> {
        let xwayland = self.xwayland.as_ref()?;
        xwayland.wm.is_some().then_some(xwayland.display)
    }

    /// The override-redirect windows Camino shows, top to bottom in X's stacking order.
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

    /// The override-redirect window Camino shows whose surface is `surface`.
    pub(crate) fn override_redirect_for_surface(&self, surface: &WlSurface) -> Option<&X11Surface> {
        self.override_redirect_windows()
            .find(|window| window.wl_surface().as_ref() == Some(surface))
    }

    /// Smithay's window manager, once Xwayland is ready.
    pub fn xwm(&mut self) -> Option<&mut X11Wm> {
        self.xwayland.as_mut()?.wm.as_mut()
    }

    /// Camino's scene is the truth (11): after a restack that involves an XWayland window,
    /// X's stacking order follows it, so that X clients and `_NET_CLIENT_LIST_STACKING` see
    /// what the user sees.
    pub(crate) fn refresh_x11_stacking(&mut self) {
        let policy = &self.policy;
        // Bottom to top: Smithay moves each window above the one before it, whatever its
        // doc comment says. Windows on other workspaces aren't in the scene, and go below.
        let active = policy.active_workspace();
        let order: Vec<X11Surface> = (0..policy.workspace_count())
            .filter(|workspace| *workspace != active)
            .chain([active])
            .flat_map(|workspace| policy.stacking(workspace))
            .filter_map(|id| policy.window(*id)?.window().x11_surface().cloned())
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

    /// Brings the root properties Camino publishes up to date with policy (11).
    pub(crate) fn refresh_x_root(&mut self) {
        let properties = self.root_properties();
        self.with_xconn("publish the root properties", |xconn| {
            xconn.publish(properties)
        });
    }

    /// Runs the root messages X has sent as Camino's own bindings and the pager would (11).
    fn x_root_requests(&mut self) {
        let requests = self.with_xconn("read X's root messages", XConnection::requests);
        for request in requests.into_iter().flatten() {
            match request {
                // Marco ignores a desktop it doesn't have.
                RootRequest::CurrentDesktop(n) if n < self.policy.workspace_count() => {
                    let now = self.now();
                    self.policy.action(SEAT, Action::SwitchToWorkspace(n), now);
                    self.queue_redraw_all();
                }
                RootRequest::CurrentDesktop(n) => {
                    tracing::debug!("an X client asked for desktop {n}, which isn't one");
                }
                // Marco ignores 0. GSettings would refuse more than the schema's most.
                RootRequest::NumberOfDesktops(n) if n > 0 => {
                    let n = n.min(MAX_WORKSPACES) as i32;
                    self.settings_thread
                        .set(MARCO_GENERAL, "num-workspaces", Value::Int(n));
                }
                RootRequest::NumberOfDesktops(_) => {}
            }
        }
    }

    /// Runs `use_it` on Camino's X connection, if there is one. A connection that fails is
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
                tracing::warn!("can't {what}, so Camino's X connection is closed: {err}");
                xwayland.xconn = None;
                None
            }
        }
    }

    fn root_properties(&self) -> RootProperties {
        RootProperties {
            desktop_names: self.policy.workspace_names().to_vec(),
            current_desktop: self.policy.active_workspace(),
            // X client space is logical space until the X11 scale (41).
            work_areas: self.policy.work_areas().collect(),
        }
    }

    /// Drops Xwayland and its window manager, which ends it.
    fn stop_xwayland(&mut self) {
        if let Some(xwayland) = self.xwayland.take() {
            self.loop_handle.remove(xwayland.source);
        }
    }

    /// Stops Xwayland for a test fixture that's being dropped, which gives its display's
    /// lock back. Xwayland itself outlives it; see the fixture's `Drop`.
    #[cfg(test)]
    pub(crate) fn stop_xwayland_in_test(&mut self) {
        self.stop_xwayland();
    }
}

impl State {
    /// Xwayland is up. In 11's start-up order: attaches `X11Wm`, connects Camino's X
    /// connection, which claims Camino's identity, publishes the root properties, and only
    /// then sets `DISPLAY` for the clients Camino spawns, which read them at start-up.
    fn xwayland_ready(&mut self, x11_socket: UnixStream) {
        let camino = &mut self.camino;
        let Some(xwayland) = camino.xwayland.as_mut() else {
            return;
        };
        let started = X11Wm::start_wm(
            camino.loop_handle.clone(),
            &camino.display_handle,
            x11_socket,
            xwayland.client.clone(),
        );
        match started {
            Ok(wm) => {
                xwayland.wm = Some(wm);
                match XConnection::connect(xwayland.display, &camino.loop_handle) {
                    Ok(xconn) => xwayland.xconn = Some(xconn),
                    Err(err) => tracing::warn!(
                        "can't make Camino's X connection, so X clients see no desktops: {err}"
                    ),
                }
                let value = format!(":{}", xwayland.display);
                // Published here rather than left to the refresh pass, since X clients read
                // them as they start (11): `DISPLAY` must not come first.
                camino.refresh_x_root();
                tracing::info!("Xwayland is ready on DISPLAY={value}");
                camino
                    .environment
                    .insert(OsString::from("DISPLAY"), OsString::from(value));
            }
            Err(err) => {
                tracing::warn!("can't manage Xwayland's windows, so Camino runs without X: {err}");
                camino.stop_xwayland();
            }
        }
    }
}
