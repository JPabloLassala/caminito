//! Xwayland (08, 11, 15): spawned by Smithay at start-up in every mode, eagerly, with
//! Smithay's `X11Wm` attached once it's ready. If it can't start, Camino logs it once and
//! runs without X; the restart path is 42's. Its windows are managed through `xwm`, and X's
//! stacking order follows Camino's scene.

mod xwm;

use std::ffi::OsString;
use std::os::unix::net::UnixStream;
use std::process::Stdio;

use smithay::reexports::calloop::RegistrationToken;
use smithay::reexports::wayland_server::Client;
use smithay::xwayland::xwm::X11Window;
use smithay::xwayland::{X11Surface, X11Wm, XWayland, XWaylandEvent};

use crate::helpers::{self, StderrTarget};
use crate::settings::translate::Mouse;
use crate::state::{Camino, State};

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
    /// The XWayland windows in the order `X11Wm` was last told to stack them, bottom to
    /// top.
    stacking: Vec<X11Window>,
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
                    stacking: Vec::new(),
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
        // doc comment says.
        let order: Vec<X11Surface> = policy
            .stacking(policy.active_workspace())
            .iter()
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

    /// Drops Xwayland and its window manager, which ends it.
    fn stop_xwayland(&mut self) {
        if let Some(xwayland) = self.xwayland.take() {
            self.loop_handle.remove(xwayland.source);
        }
    }
}

impl State {
    /// Xwayland is up: attaches `X11Wm`, then sets `DISPLAY` for the clients Camino spawns.
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
                // 38 moves this after the root properties.
                let value = format!(":{}", xwayland.display);
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
