//! The tty backend: Caminito on a spare tty, driving the machine's monitors through DRM/KMS,
//! with the devices libseat grants (08, 15, 55), and reading its input devices through
//! libinput (56).
//!
//! Adapted from Smithay's anvil at Smithay commit 928d4a9, `anvil/src/udev.rs`: start-up
//! from `run_udev` (`:219-600`), the GPU from `device_added` (`:774-898`), monitors from
//! `connector_connected`, `connector_disconnected` and `device_changed` (`:900-1164`).
//! Cut down to the primary GPU, which v0.1 drives alone, and composited with GLES every
//! frame, as on winit, so nothing is scanned out directly. libinput is set up as in
//! `run_udev` (`:294-320`), and its keyboards' LEDs kept as `update_led_state` does
//! (`:212-216`). linux-dmabuf and drm-syncobj are offered as `run_udev` does (`:451-498`),
//! and dmabufs imported as `dmabuf_imported` does (`:170-185`), on the primary GPU (57).

mod libinput;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use smithay::backend::allocator::Fourcc;
use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::allocator::gbm::{GbmAllocator, GbmBufferFlags, GbmDevice};
use smithay::backend::drm::compositor::FrameFlags;
use smithay::backend::drm::exporter::gbm::{GbmFramebufferExporter, NodeFilter};
use smithay::backend::drm::output::{DrmOutput, DrmOutputManager, DrmOutputRenderElements};
use smithay::backend::drm::{DrmDevice, DrmDeviceFd, DrmError, DrmEvent, DrmNode, NodeType};
use smithay::backend::egl::{self, EGLContext, EGLDisplay};
use smithay::backend::input::{DeviceCapability, InputEvent};
use smithay::backend::libinput::{LibinputInputBackend, LibinputSessionInterface};
use smithay::backend::renderer::ImportDma;
use smithay::backend::renderer::element::RenderElementStates;
use smithay::backend::renderer::gles::{GlesError, GlesRenderer};
use smithay::backend::session::libseat::{self, LibSeatSession};
use smithay::backend::session::{Event as SessionEvent, Session};
use smithay::backend::udev::{self, UdevBackend, UdevEvent};
use smithay::input::keyboard::LedState;
use smithay::output::{Mode, Output, PhysicalProperties};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::{LoopHandle, RegistrationToken};
use smithay::reexports::drm::control::{Mode as DrmMode, ModeTypeFlags, connector, crtc};
use smithay::reexports::input::event::{DeviceEvent, EventTrait, KeyboardEvent, PointerEvent};
use smithay::reexports::input::{self, Libinput};
use smithay::reexports::rustix::fs::OFlags;
use smithay::utils::DeviceFd;
use smithay::wayland::dmabuf::{DmabufFeedbackBuilder, DmabufState};
use smithay::wayland::drm_syncobj::{DrmSyncobjState, supports_syncobj_eventfd};
use smithay_drm_extras::display_info;
use smithay_drm_extras::drm_scanner::{DrmScanEvent, DrmScanner};

use crate::backend::Backend;
use crate::input::devices::ConfigurableDevice;
use crate::output::{DEFAULT_REFRESH, UNKNOWN_EDID_TEXT};
use crate::scene::{self, CLEAR_COLOR, SceneElement};
use crate::state::{Caminito, State};

/// The formats Caminito renders the outputs in, in order of preference. 8 bits per channel,
/// as anvil's `SUPPORTED_FORMATS_8BIT_ONLY`: every GPU and monitor takes them.
const COLOR_FORMATS: [Fourcc; 2] = [Fourcc::Abgr8888, Fourcc::Argb8888];

type TtyOutputManager = DrmOutputManager<
    GbmAllocator<DrmDeviceFd>,
    GbmFramebufferExporter<DrmDeviceFd>,
    (),
    DrmDeviceFd,
>;

type TtyOutput =
    DrmOutput<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, (), DrmDeviceFd>;

/// Why the tty backend can't start.
#[derive(Debug)]
pub enum Error {
    /// Neither seatd nor logind gave Caminito a seat.
    Seat(libseat::Error),
    /// udev can't list or watch the seat's devices.
    Udev(std::io::Error),
    /// libinput can't take the seat's input devices.
    Libinput,
    /// The seat has no GPU.
    NoGpu,
    /// The GPU udev named isn't a DRM device.
    NotDrm(PathBuf),
    /// libseat won't open the GPU.
    Open(libseat::Error),
    /// The GPU's DRM device can't be set up.
    Drm(DrmError),
    /// GBM can't start on the GPU.
    Gbm(std::io::Error),
    /// EGL can't start on the GPU.
    Egl(egl::Error),
    /// The GLES renderer can't start.
    Gles(GlesError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Seat(err) => write!(f, "can't take a seat from seatd or logind: {err}"),
            Error::Udev(err) => write!(f, "can't list the seat's devices: {err}"),
            Error::Libinput => f.write_str("can't read the seat's input devices"),
            Error::NoGpu => f.write_str("the seat has no GPU"),
            Error::NotDrm(path) => write!(f, "{} isn't a DRM device", path.display()),
            Error::Open(err) => write!(f, "can't open the GPU: {err}"),
            Error::Drm(err) => write!(f, "can't set up the GPU's DRM device: {err}"),
            Error::Gbm(err) => write!(f, "can't start GBM on the GPU: {err}"),
            Error::Egl(err) => write!(f, "can't start EGL on the GPU: {err}"),
            Error::Gles(err) => write!(f, "can't start the GLES renderer: {err}"),
        }
    }
}

impl std::error::Error for Error {}

/// libseat's hold on the seat, the primary GPU and the input devices.
pub struct Tty {
    /// Dropped before the seat, which gives the VT back when it's released.
    gpu: Gpu,
    libinput: Libinput,
    /// The keyboards plugged in, whose LEDs show the seat's.
    keyboards: Vec<input::Device>,
    seat: LibSeatSession,
    /// linux-dmabuf, through which clients hand over the buffers they render on the GPU. Its
    /// global is offered only here, where there's a GPU to import them on (57).
    dmabuf: DmabufState,
    /// drm-syncobj, offered only with linux-dmabuf and when the GPU has syncobj timelines
    /// (57).
    syncobj: Option<DrmSyncobjState>,
}

/// The GPU's DRM device, its renderer and the monitors lit on it.
struct Gpu {
    /// The GPU's primary node, which udev's events name.
    node: DrmNode,
    /// The GPU's render node, which clients render on, or its primary node if it has none.
    render_node: DrmNode,
    manager: TtyOutputManager,
    renderer: GlesRenderer,
    scanner: DrmScanner,
    /// Each lit monitor, by the CRTC driving it.
    monitors: HashMap<crtc::Handle, LitMonitor>,
    /// Each monitor output management turned off, by the CRTC that drove it and will again
    /// (58).
    dark: HashMap<crtc::Handle, DarkMonitor>,
    /// Each monitor a scan found, by the CRTC that will drive it, until the saved layout
    /// has said which mode it's lit in, if at all (87).
    unlit: HashMap<crtc::Handle, DarkMonitor>,
}

/// One lit monitor's output, and the swapchain it's shown from.
struct LitMonitor {
    output: Output,
    connector: Connector,
    drm_output: TtyOutput,
    frame: Frame,
}

/// A connected monitor that isn't lit: one output management turned off, or one on its way
/// to being lit.
struct DarkMonitor {
    output: Output,
    connector: Connector,
}

/// What lighting a monitor, or switching its mode, needs of its connector.
struct Connector {
    handle: connector::Handle,
    modes: Vec<DrmMode>,
}

impl Connector {
    /// The first of the connector's modes that the output lists as `mode`.
    fn drm_mode(&self, mode: Mode) -> Option<DrmMode> {
        self.modes
            .iter()
            .copied()
            .find(|&drm_mode| Mode::from(drm_mode) == mode)
    }
}

/// Where a monitor's last frame is.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Frame {
    /// Shown; the next can be drawn.
    Shown,
    /// Queued for the next vblank.
    Queued,
    /// It had no damage, so no vblank will come: this timer waits out a refresh instead.
    Unchanged(RegistrationToken),
}

impl Tty {
    /// Takes a seat, opens the primary GPU, adds each connected monitor as an output of
    /// `caminito`, to be lit by `State::monitors_scanned`, and joins the seat's, udev's and
    /// the GPU's events to the loop.
    pub fn new(caminito: &mut Caminito) -> Result<Self, Error> {
        let (mut seat, notifier) = LibSeatSession::new().map_err(Error::Seat)?;
        let seat_name = seat.seat();
        tracing::info!("running on the tty, on {seat_name}");

        let udev = UdevBackend::new(&seat_name).map_err(Error::Udev)?;
        let node = primary_node(&seat_name)?;
        let path = udev
            .device_list()
            .find(|&(id, _)| id == node.dev_id())
            .map(|(_, path)| path.to_owned())
            .ok_or(Error::NoGpu)?;
        tracing::info!("using {} as the primary GPU", path.display());
        let mut gpu = Gpu::open(&mut seat, node, &path, &caminito.loop_handle)?;

        let mut libinput = Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(
            seat.clone().into(),
        );
        libinput
            .udev_assign_seat(&seat_name)
            .map_err(|()| Error::Libinput)?;

        let mut dmabuf = DmabufState::new();
        let syncobj = if gpu.offer_dmabuf(caminito, &mut dmabuf) {
            gpu.offer_syncobj(caminito)
        } else {
            None
        };
        let mut tty = Tty {
            gpu,
            libinput: libinput.clone(),
            keyboards: Vec::new(),
            seat,
            dmabuf,
            syncobj,
        };
        tty.scan_connectors(caminito);

        caminito
            .loop_handle
            .insert_source(notifier, |event, _, state| {
                let paused = matches!(event, SessionEvent::PauseSession);
                tty_mut(&mut state.backend).seat_event(&mut state.caminito, event);
                // Switching back finds the monitors plugged in and unplugged meanwhile.
                state.monitors_scanned();
                // The keys held to switch VTs come up on the other VT, so no release reaches
                // Caminito.
                if paused {
                    state.release_pressed_keys();
                }
            })
            .expect("the seat's events join the loop");
        caminito
            .loop_handle
            .insert_source(LibinputInputBackend::new(libinput), input_event)
            .expect("libinput's events join the loop");
        caminito.loop_handle.insert_idle(queued_events);
        caminito
            .loop_handle
            .insert_source(udev, |event, _, state| {
                tty_mut(&mut state.backend).udev_event(&mut state.caminito, event);
                state.monitors_scanned();
            })
            .expect("udev's events join the loop");
        Ok(tty)
    }

    /// The GPU's renderer, for what's drawn off the monitors: a capture's frame (103).
    pub(crate) fn renderer(&mut self) -> &mut GlesRenderer {
        &mut self.gpu.renderer
    }

    /// Switches to VT `vt`, as `XF86Switch_VT_n` asks.
    pub fn change_vt(&mut self, vt: i32) {
        if let Err(err) = self.seat.change_vt(vt) {
            tracing::warn!("can't switch to VT {vt}: {err}");
        }
    }

    /// linux-dmabuf's state, whose global the tty offers.
    pub fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf
    }

    /// drm-syncobj's state, if the GPU has syncobj timelines.
    pub fn syncobj_state(&mut self) -> Option<&mut DrmSyncobjState> {
        self.syncobj.as_mut()
    }

    /// Imports a client's dmabuf on the GPU, so it's known to render before the client
    /// uses it. Returns whether the GPU took it.
    pub fn import_dmabuf(&mut self, dmabuf: &Dmabuf) -> bool {
        if let Err(err) = self.gpu.renderer.import_dmabuf(dmabuf, None) {
            tracing::debug!("can't import a client's dmabuf: {err}");
            return false;
        }
        if dmabuf.node().is_none() {
            dmabuf.set_node(self.gpu.render_node);
        }
        true
    }

    /// Whether monitors can be lit and their modes switched: only with the DRM master,
    /// which another VT has while Caminito is switched away.
    pub fn can_change_outputs(&self) -> bool {
        self.seat.is_active()
    }

    /// Switches a lit monitor to another of its modes, and returns whether the GPU took it.
    /// A monitor that isn't lit yet will be lit in `mode`, if it lists it: whether the GPU
    /// takes it is known only then.
    pub fn set_output_mode(
        &mut self,
        caminito: &mut Caminito,
        output: &Output,
        mode: Mode,
    ) -> bool {
        let gpu = &mut self.gpu;
        let Some(crtc) = output.user_data().get::<crtc::Handle>() else {
            return false;
        };
        if let Some(monitor) = gpu.unlit.get(crtc) {
            if monitor.connector.drm_mode(mode).is_none() {
                return false;
            }
            output.change_current_state(Some(mode), None, None, None);
            caminito.output_mode_changed(output);
            return true;
        }
        let Some(monitor) = gpu.monitors.get_mut(crtc) else {
            return false;
        };
        let Some(drm_mode) = monitor.connector.drm_mode(mode) else {
            return false;
        };
        let elements = DrmOutputRenderElements::<_, SceneElement<GlesRenderer>>::default();
        if let Err(err) = monitor
            .drm_output
            .use_mode(drm_mode, &mut gpu.renderer, &elements)
        {
            tracing::warn!("can't switch {} to {mode:?}: {err}", output.name());
            return false;
        }
        // A frame on its way is still shown, and its vblank still comes: the mode is set
        // with the next frame.
        output.change_current_state(Some(mode), None, None, None);
        caminito.output_mode_changed(output);
        true
    }

    /// Turns a lit monitor off, as output management asks. It stays connected, so it can be
    /// lit again. One that isn't lit yet stays dark.
    pub fn disable_output(&mut self, caminito: &mut Caminito, output: &Output) {
        let Some(&crtc) = output.user_data().get::<crtc::Handle>() else {
            return;
        };
        if let Some(monitor) = self.gpu.unlit.remove(&crtc) {
            tracing::debug!("{} stays off", output.name());
            caminito.disable_output(output);
            self.gpu.dark.insert(crtc, monitor);
            return;
        }
        let Some(mut monitor) = self.gpu.monitors.remove(&crtc) else {
            return;
        };
        tracing::debug!("{} was turned off", output.name());
        monitor.stop_waiting(&caminito.loop_handle);
        // Dropping the monitor's DRM output turns its CRTC off.
        let LitMonitor {
            output, connector, ..
        } = monitor;
        caminito.disable_output(&output);
        self.gpu
            .dark
            .insert(crtc, DarkMonitor { output, connector });
    }

    /// Lights again, in `mode`, a monitor output management turned off, and returns whether
    /// the GPU took it.
    pub fn enable_output(&mut self, caminito: &mut Caminito, output: &Output, mode: Mode) -> bool {
        let Some(&crtc) = output.user_data().get::<crtc::Handle>() else {
            return false;
        };
        let Some(monitor) = self.gpu.dark.remove(&crtc) else {
            return false;
        };
        let Some(drm_mode) = monitor.connector.drm_mode(mode) else {
            self.gpu.dark.insert(crtc, monitor);
            return false;
        };
        caminito.enable_output(output, mode);
        match self.gpu.light(crtc, monitor, drm_mode) {
            Ok(()) => true,
            Err(monitor) => {
                caminito.disable_output(output);
                self.gpu.dark.insert(crtc, monitor);
                false
            }
        }
    }

    /// The outputs of the monitors a scan found, which aren't lit yet.
    pub(crate) fn unlit_outputs(&self) -> Vec<Output> {
        let unlit = self.gpu.unlit.values();
        unlit.map(|monitor| monitor.output.clone()).collect()
    }

    /// Lights a monitor a scan found, in the mode its output has, and returns whether the
    /// GPU took it.
    pub(crate) fn light_output(&mut self, output: &Output) -> bool {
        let Some(&crtc) = output.user_data().get::<crtc::Handle>() else {
            return false;
        };
        let Some(monitor) = self.gpu.unlit.remove(&crtc) else {
            return false;
        };
        let drm_mode = output
            .current_mode()
            .and_then(|mode| monitor.connector.drm_mode(mode));
        let lit = match drm_mode {
            Some(drm_mode) => self.gpu.light(crtc, monitor, drm_mode),
            None => Err(monitor),
        };
        match lit {
            Ok(()) => true,
            Err(monitor) => {
                self.gpu.unlit.insert(crtc, monitor);
                false
            }
        }
    }

    /// Forgets a monitor a scan found that the GPU can't light, and removes its output.
    pub(crate) fn remove_unlit_output(&mut self, caminito: &mut Caminito, output: &Output) {
        if let Some(crtc) = output.user_data().get::<crtc::Handle>()
            && let Some(monitor) = self.gpu.unlit.remove(crtc)
        {
            caminito.remove_output(&monitor.output);
        }
    }

    /// A keyboard was plugged in: it shows `leds` now, and the seat's on each change.
    fn add_keyboard(&mut self, mut keyboard: input::Device, leds: LedState) {
        keyboard.led_update(leds.into());
        self.keyboards.push(keyboard);
    }

    fn remove_keyboard(&mut self, keyboard: &input::Device) {
        self.keyboards.retain(|k| k != keyboard);
    }

    /// Shows the seat's Caps, Num and Scroll Lock on every keyboard.
    pub fn update_leds(&mut self, leds: LedState) {
        for keyboard in &mut self.keyboards {
            keyboard.led_update(leds.into());
        }
    }

    /// Draws a frame of `output`, unless its last frame hasn't been shown yet: frames, and
    /// so clients' frame callbacks, are paced by the monitor's vblanks.
    pub fn render(
        &mut self,
        caminito: &mut Caminito,
        output: &Output,
    ) -> Option<RenderElementStates> {
        if !self.seat.is_active() {
            return None;
        }
        let gpu = &mut self.gpu;
        let crtc = *output.user_data().get::<crtc::Handle>()?;
        let monitor = gpu.monitors.get_mut(&crtc)?;
        if monitor.frame != Frame::Shown {
            return None;
        }

        let elements = scene::output_elements(&mut gpu.renderer, caminito, output);
        let result = monitor.drm_output.render_frame(
            &mut gpu.renderer,
            &elements,
            CLEAR_COLOR,
            FrameFlags::empty(),
        );
        let (states, is_empty) = match result {
            Ok(result) => (result.states, result.is_empty),
            Err(err) => {
                tracing::debug!("can't render {}: {err}", output.name());
                return None;
            }
        };
        if is_empty {
            // A client redrawing to no effect is still held to the monitor's rate.
            let refresh = output
                .current_mode()
                .map_or(DEFAULT_REFRESH, |mode| mode.refresh);
            let delay = Duration::from_secs_f64(1000. / f64::from(refresh));
            let token = caminito
                .loop_handle
                .insert_source(Timer::from_duration(delay), move |_, _, state| {
                    tty_mut(&mut state.backend).gpu.frame_shown(crtc);
                    TimeoutAction::Drop
                })
                .expect("the timer joins the loop");
            monitor.frame = Frame::Unchanged(token);
        } else if let Err(err) = monitor.drm_output.queue_frame(()) {
            tracing::debug!("can't queue a frame of {}: {err}", output.name());
            return None;
        } else {
            monitor.frame = Frame::Queued;
        }
        Some(states)
    }

    /// Switching VTs away drops the DRM master and the input devices; switching back takes
    /// them again, catches up with monitors plugged or unplugged meanwhile, and redraws
    /// every output (08). libinput adds its devices again, so they get their settings anew.
    fn seat_event(&mut self, caminito: &mut Caminito, event: SessionEvent) {
        match event {
            SessionEvent::PauseSession => {
                tracing::debug!("the VT was switched away");
                self.libinput.suspend();
                self.gpu.manager.pause();
            }
            SessionEvent::ActivateSession => {
                tracing::debug!("the VT was switched back");
                if self.libinput.resume().is_err() {
                    tracing::warn!("can't take the input devices back");
                }
                caminito.loop_handle.insert_idle(queued_events);
                if let Err(err) = self.gpu.manager.lock().activate(false) {
                    tracing::warn!("can't take the GPU back: {err}");
                }
                // Activating reset every CRTC, so no frame is still on its way.
                for monitor in self.gpu.monitors.values_mut() {
                    monitor.stop_waiting(&caminito.loop_handle);
                }
                self.scan_connectors(caminito);
                caminito.queue_redraw_all();
            }
        }
    }

    fn udev_event(&mut self, caminito: &mut Caminito, event: UdevEvent) {
        // Without the DRM master, a monitor can't be lit; switching back scans anyway.
        if let UdevEvent::Changed { device_id } = event
            && device_id == self.gpu.node.dev_id()
            && self.seat.is_active()
        {
            self.scan_connectors(caminito);
        }
    }

    /// Adds the monitors connected since the last scan, to be lit once the saved layout is
    /// restored (87), and removes the ones unplugged (51). One whose modes changed is
    /// removed and added again, with its new modes.
    fn scan_connectors(&mut self, caminito: &mut Caminito) {
        let gpu = &mut self.gpu;
        let events = match gpu.scanner.scan_connectors(gpu.manager.device()) {
            Ok(result) => result,
            Err(err) => {
                tracing::warn!("can't scan the GPU's connectors: {err}");
                return;
            }
        };
        for event in events {
            match event {
                DrmScanEvent::Connected {
                    connector,
                    crtc: Some(crtc),
                } => gpu.connect(caminito, &connector, crtc),
                DrmScanEvent::Disconnected {
                    crtc: Some(crtc), ..
                } => gpu.disconnect(caminito, crtc),
                DrmScanEvent::Changed {
                    connector,
                    crtc: Some(crtc),
                } => {
                    gpu.disconnect(caminito, crtc);
                    gpu.connect(caminito, &connector, crtc);
                }
                DrmScanEvent::Connected {
                    connector,
                    crtc: None,
                } => tracing::warn!(
                    "can't light {}: no CRTC is free",
                    connector_name(&connector)
                ),
                _ => {}
            }
        }
    }
}

impl Gpu {
    fn open(
        seat: &mut LibSeatSession,
        node: DrmNode,
        path: &Path,
        loop_handle: &LoopHandle<'static, State>,
    ) -> Result<Self, Error> {
        let fd = seat
            .open(
                path,
                OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
            )
            .map_err(Error::Open)?;
        let fd = DrmDeviceFd::new(DeviceFd::from(fd));
        let (drm, notifier) = DrmDevice::new(fd.clone(), true).map_err(Error::Drm)?;
        let gbm = GbmDevice::new(fd).map_err(Error::Gbm)?;

        // Safety: the GBM device outlives the display, which holds a clone of it.
        let display = unsafe { EGLDisplay::new(gbm.clone()) }.map_err(Error::Egl)?;
        let context = EGLContext::new(&display).map_err(Error::Egl)?;
        let render_formats = context.dmabuf_render_formats().clone();
        // Safety: the context is new, and only this renderer makes it current.
        let renderer = unsafe { GlesRenderer::new(context) }.map_err(Error::Gles)?;

        let allocator = GbmAllocator::new(
            gbm.clone(),
            GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
        );
        let exporter = GbmFramebufferExporter::new(gbm, NodeFilter::None);
        // No GBM device: the cursor is drawn in the scene (15), never on a cursor plane.
        let manager = DrmOutputManager::new(
            drm,
            allocator,
            exporter,
            None,
            COLOR_FORMATS,
            render_formats,
        );

        loop_handle
            .insert_source(notifier, |event, _, state| match event {
                DrmEvent::VBlank(crtc) => tty_mut(&mut state.backend).gpu.vblank(crtc),
                DrmEvent::Error(err) => tracing::debug!("can't show a frame: {err}"),
            })
            .expect("the GPU's events join the loop");

        let render_node = node
            .node_with_type(NodeType::Render)
            .and_then(Result::ok)
            .unwrap_or(node);
        Ok(Gpu {
            node,
            render_node,
            manager,
            renderer,
            scanner: DrmScanner::new(),
            monitors: HashMap::new(),
            dark: HashMap::new(),
            unlit: HashMap::new(),
        })
    }

    /// Offers linux-dmabuf with the formats the GPU renders from, on its render node.
    /// Returns whether it's offered: without it, GPU clients fall back to shared memory.
    fn offer_dmabuf(&mut self, caminito: &Caminito, dmabuf: &mut DmabufState) -> bool {
        let formats = self.renderer.dmabuf_formats();
        match DmabufFeedbackBuilder::new(self.render_node.dev_id(), formats).build() {
            Ok(feedback) => {
                dmabuf.create_global_with_default_feedback::<State>(
                    &caminito.display_handle,
                    &feedback,
                );
                true
            }
            Err(err) => {
                tracing::warn!("can't offer linux-dmabuf: {err}");
                false
            }
        }
    }

    /// Offers drm-syncobj if the GPU can wait on syncobj timelines' points, which the
    /// acquire points' blockers need.
    fn offer_syncobj(&self, caminito: &Caminito) -> Option<DrmSyncobjState> {
        let device = self.manager.device().device_fd();
        if !supports_syncobj_eventfd(device) {
            tracing::info!("the GPU has no syncobj timelines, so explicit sync isn't offered");
            return None;
        }
        Some(DrmSyncobjState::new::<State>(
            &caminito.display_handle,
            device.clone(),
        ))
    }

    /// A queued frame reached the screen. A vblank of a frame queued before the VT was
    /// switched away can come after switching back, when no frame is queued: that one's
    /// ignored.
    fn vblank(&mut self, crtc: crtc::Handle) {
        let Some(monitor) = self.monitors.get_mut(&crtc) else {
            return;
        };
        if monitor.frame != Frame::Queued {
            return;
        }
        if let Err(err) = monitor.drm_output.frame_submitted() {
            tracing::debug!("can't finish a frame of {}: {err}", monitor.output.name());
        }
        monitor.frame = Frame::Shown;
    }

    /// A frame with no damage waited out its refresh.
    fn frame_shown(&mut self, crtc: crtc::Handle) {
        if let Some(monitor) = self.monitors.get_mut(&crtc)
            && matches!(monitor.frame, Frame::Unchanged(_))
        {
            monitor.frame = Frame::Shown;
        }
    }

    /// Adds a monitor as a new output (50, 51), in its preferred mode, that lists every
    /// mode the monitor has (58). It isn't lit here: lighting it now and switching it to
    /// its saved mode afterwards would set its mode twice, and each mode set blanks the
    /// monitor and holds up the others' frames (87).
    fn connect(
        &mut self,
        caminito: &mut Caminito,
        connector: &connector::Info,
        crtc: crtc::Handle,
    ) {
        let name = connector_name(connector);
        let modes = connector.modes();
        let Some(&drm_mode) = modes
            .iter()
            .find(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
            .or_else(|| modes.first())
        else {
            tracing::warn!("can't light {name}: it has no modes");
            return;
        };

        let edid = display_info::for_connector(self.manager.device(), connector.handle());
        let edid_text = |text: Option<String>| text.unwrap_or_else(|| UNKNOWN_EDID_TEXT.to_owned());
        let (width, height) = connector.size().unwrap_or((0, 0));
        let output = Output::new(
            name,
            PhysicalProperties {
                size: (width as i32, height as i32).into(),
                subpixel: connector.subpixel().into(),
                make: edid_text(edid.as_ref().and_then(|info| info.make())),
                model: edid_text(edid.as_ref().and_then(|info| info.model())),
                serial_number: edid_text(edid.as_ref().and_then(|info| info.serial())),
            },
        );
        output.set_preferred(Mode::from(drm_mode));
        for &mode in modes {
            output.add_mode(Mode::from(mode));
        }
        output.user_data().insert_if_missing(|| crtc);
        caminito.add_output(output.clone());

        let monitor = DarkMonitor {
            output,
            connector: Connector {
                handle: connector.handle(),
                modes: modes.to_vec(),
            },
        };
        self.unlit.insert(crtc, monitor);
    }

    /// Lights a monitor in `drm_mode`, or gives it back if the GPU can't.
    fn light(
        &mut self,
        crtc: crtc::Handle,
        monitor: DarkMonitor,
        drm_mode: DrmMode,
    ) -> Result<(), DarkMonitor> {
        let name = monitor.output.name();
        let drm_output = self
            .manager
            .lock()
            .initialize_output::<_, SceneElement<GlesRenderer>>(
                crtc,
                drm_mode,
                &[monitor.connector.handle],
                &monitor.output,
                None,
                &mut self.renderer,
                &DrmOutputRenderElements::default(),
            );
        match drm_output {
            Ok(drm_output) => {
                tracing::debug!("lit {name}");
                let DarkMonitor { output, connector } = monitor;
                self.monitors.insert(
                    crtc,
                    LitMonitor {
                        output,
                        connector,
                        drm_output,
                        frame: Frame::Shown,
                    },
                );
                Ok(())
            }
            Err(err) => {
                tracing::warn!("can't light {name}: {err}");
                Err(monitor)
            }
        }
    }

    /// A monitor was unplugged: its output goes, and policy moves its windows off it (51).
    /// One that was turned off, or that isn't lit yet, is only forgotten.
    fn disconnect(&mut self, caminito: &mut Caminito, crtc: crtc::Handle) {
        if let Some(mut monitor) = self.monitors.remove(&crtc) {
            tracing::debug!("{} was unplugged", monitor.output.name());
            monitor.stop_waiting(&caminito.loop_handle);
            caminito.remove_output(&monitor.output);
        }
        let dark = self.dark.remove(&crtc);
        if let Some(monitor) = dark.or_else(|| self.unlit.remove(&crtc)) {
            tracing::debug!("{} was unplugged", monitor.output.name());
            caminito.remove_output(&monitor.output);
        }
    }
}

impl LitMonitor {
    /// Forgets the frame on its way, and the timer of an unchanged one.
    fn stop_waiting(&mut self, loop_handle: &LoopHandle<'static, State>) {
        if let Frame::Unchanged(token) = self.frame {
            loop_handle.remove(token);
        }
        self.frame = Frame::Shown;
    }
}

/// One event from libinput. A device plugged in gets its settings (53) and, if it's a
/// keyboard, the seat's LEDs; then the event goes the way every backend's does.
fn input_event(event: InputEvent<LibinputInputBackend>, _: &mut (), state: &mut State) {
    match &event {
        InputEvent::DeviceAdded { device } => {
            tracing::debug!("{} was plugged in", device.name());
            if device.has_capability(DeviceCapability::Keyboard.into()) {
                let leds = state.caminito.seat.get_keyboard().unwrap().led_state();
                tty_mut(&mut state.backend).add_keyboard(device.clone(), leds);
            }
            state.caminito.add_input_device(Box::new(device.clone()));
        }
        InputEvent::DeviceRemoved { device } => {
            tracing::debug!("{} was unplugged", device.name());
            tty_mut(&mut state.backend).remove_keyboard(device);
            state
                .caminito
                .remove_input_device(&ConfigurableDevice::id(device));
        }
        _ => {}
    }
    state.process_input_event(event);
}

/// Handles what libinput has queued. Assigning the seat and resuming queue each device as
/// plugged in without making libinput readable, so the loop wouldn't hear of them until the
/// first key or motion: until then no lid switch would reach the default scale's rule (50),
/// and no keyboard would show the seat's LEDs. wlroots' `backend_start`
/// (`backend/libinput/backend.c`) reads them at once as well. Nothing is read from the
/// devices here, so the queue holds only that and what suspending left: each device
/// unplugged, after the releases of its keys and buttons, which go on as Smithay's source
/// would pass them.
fn queued_events(state: &mut State) {
    let mut libinput = tty_mut(&mut state.backend).libinput.clone();
    for event in &mut libinput {
        let event = match event {
            input::Event::Device(DeviceEvent::Added(event)) => InputEvent::DeviceAdded {
                device: event.device(),
            },
            input::Event::Device(DeviceEvent::Removed(event)) => InputEvent::DeviceRemoved {
                device: event.device(),
            },
            input::Event::Keyboard(KeyboardEvent::Key(event)) => InputEvent::Keyboard { event },
            input::Event::Pointer(PointerEvent::Button(event)) => {
                InputEvent::PointerButton { event }
            }
            _ => continue,
        };
        input_event(event, &mut (), state);
    }
}

/// The seat's primary GPU, or its first if udev marks none as primary, by its primary node.
fn primary_node(seat: &str) -> Result<DrmNode, Error> {
    let path = udev::primary_gpu(seat)
        .map_err(Error::Udev)?
        .or_else(|| udev::all_gpus(seat).ok()?.into_iter().next())
        .ok_or(Error::NoGpu)?;
    DrmNode::from_path(&path)
        .ok()
        .and_then(|node| node.node_with_type(NodeType::Primary)?.ok())
        .ok_or(Error::NotDrm(path))
}

/// A connector's name as the kernel gives it, such as `DP-1` or `eDP-1`, which says
/// whether it's built in (50).
fn connector_name(connector: &connector::Info) -> String {
    format!(
        "{}-{}",
        connector.interface().as_str(),
        connector.interface_id()
    )
}

// `Winit` and `Headless` are the other variants.
fn tty_mut(backend: &mut Backend) -> &mut Tty {
    match backend {
        Backend::Tty(tty) => tty,
        _ => unreachable!("the tty's events come only on the tty backend"),
    }
}
