//! `zwlr_output_management_unstable_v1` version 4 (06, 58): clients such as `wlr-randr`,
//! and msd's wlrandr plugin, see the monitors as heads with their modes, and change them
//! with configurations that are tested or applied whole.
//!
//! This module owns the protocol's objects and its rules: which head and mode objects each
//! client has, the serial, and what makes a request a protocol error. It knows nothing of
//! outputs. The compositor gives it the heads there are in its refresh pass, through
//! [`OutputManagementState::refresh`], and answers configurations through
//! [`OutputManagementHandler`].
//!
//! Written from the protocol's XML. niri's and cosmic-comp's implementations are GPL-3, so
//! they weren't used (03).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use smithay::output::Mode;
use smithay::reexports::wayland_protocols_wlr::output_management::v1::server::zwlr_output_configuration_head_v1::{
    self, ZwlrOutputConfigurationHeadV1,
};
use smithay::reexports::wayland_protocols_wlr::output_management::v1::server::zwlr_output_configuration_v1::{
    self, ZwlrOutputConfigurationV1,
};
use smithay::reexports::wayland_protocols_wlr::output_management::v1::server::zwlr_output_head_v1::{
    self, AdaptiveSyncState, ZwlrOutputHeadV1,
};
use smithay::reexports::wayland_protocols_wlr::output_management::v1::server::zwlr_output_manager_v1::{
    self, ZwlrOutputManagerV1,
};
use smithay::reexports::wayland_protocols_wlr::output_management::v1::server::zwlr_output_mode_v1::{
    self, ZwlrOutputModeV1,
};
use smithay::reexports::wayland_server::backend::{ClientId, GlobalId};
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource, WEnum,
};
use smithay::utils::{Logical, Point, Raw, Size, Transform};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

/// The version of the protocol Camino offers.
const VERSION: u32 = 4;
/// `zwlr_output_mode_v1`'s own version, which is lower than the manager's.
const MODE_VERSION: u32 = 3;
/// `zwlr_output_head_v1.make`, `model` and `serial_number` are since version 2.
const SINCE_MAKE: u32 = 2;
/// `zwlr_output_head_v1.adaptive_sync` is since version 4.
const SINCE_ADAPTIVE_SYNC: u32 = 4;

/// A monitor as output management shows it, lit or not.
#[derive(Debug, Clone, PartialEq)]
pub struct Head {
    /// The connector's name, which is the `wl_output`'s too. No two heads share one.
    pub name: String,
    pub description: String,
    pub make: String,
    pub model: String,
    pub serial_number: String,
    /// The EDID's size in millimetres, or zero if it has none.
    pub physical_size: Size<i32, Raw>,
    pub modes: Vec<Mode>,
    pub preferred_mode: Option<Mode>,
    /// What the head shows, or `None` if it's disabled.
    pub enabled: Option<HeadState>,
}

impl Head {
    /// Whether `other` is this monitor still, whatever it shows: a head's name and what it
    /// says of the monitor never change, so a monitor that differs in them is a new head.
    fn is_same_monitor(&self, other: &Head) -> bool {
        self.name == other.name
            && self.description == other.description
            && self.make == other.make
            && self.model == other.model
            && self.serial_number == other.serial_number
            && self.physical_size == other.physical_size
    }
}

/// What an enabled head shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadState {
    pub mode: Option<Mode>,
    pub position: Point<i32, Logical>,
    pub scale: f64,
    pub transform: Transform,
}

/// What a configuration asks of one head.
#[derive(Debug, Clone, PartialEq)]
pub struct HeadRequest {
    /// The head's name.
    pub name: String,
    /// What it's to show, or `None` to disable it.
    pub enabled: Option<EnabledHead>,
}

/// What a configuration asks an enabled head to show. A property the client left out is
/// `None`, and the compositor chooses it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EnabledHead {
    /// One of the head's modes, or a custom mode, whose refresh rate is 0 if the client
    /// gave none.
    pub mode: Option<Mode>,
    pub position: Option<Point<i32, Logical>>,
    /// Snapped to 1/120, the step of fractional-scale-v1 (13).
    pub scale: Option<f64>,
    pub transform: Option<Transform>,
    pub adaptive_sync: Option<bool>,
}

/// What the compositor does with clients' configurations.
pub trait OutputManagementHandler: 'static {
    /// The compositor's output management state.
    fn output_management_state(&mut self) -> &mut OutputManagementState;

    /// Whether `configuration` could be applied. It changes nothing.
    fn test_output_configuration(&mut self, configuration: &[HeadRequest]) -> bool;

    /// Changes the outputs to `configuration`, and returns whether it did. If it didn't,
    /// nothing has changed. Heads the configuration leaves out stay as they are.
    fn apply_output_configuration(&mut self, configuration: &[HeadRequest]) -> bool;
}

/// The manager's global, the heads last sent, and every client's objects for them.
#[derive(Debug)]
pub struct OutputManagementState {
    /// Kept for M5's security-context filter (58).
    _global: GlobalId,
    /// The serial of the next or the last `done`. A configuration made at an earlier one is
    /// cancelled.
    serial: u32,
    /// Whether a configuration was applied since the last `done`. The serial has moved on
    /// already, so that a second configuration made before the change is cancelled, and
    /// clients are still owed it.
    applied: bool,
    heads: Vec<Head>,
    managers: Vec<Manager>,
}

/// One client's manager, and the head objects it was sent.
#[derive(Debug)]
struct Manager {
    resource: ZwlrOutputManagerV1,
    heads: Vec<ManagerHead>,
}

#[derive(Debug)]
struct ManagerHead {
    name: String,
    resource: ZwlrOutputHeadV1,
    modes: Vec<(Mode, ZwlrOutputModeV1)>,
}

/// The manager global's data.
#[derive(Debug)]
pub struct ManagerGlobalData;

/// A manager's data.
#[derive(Debug)]
pub struct ManagerData;

/// A head object's data.
#[derive(Debug)]
pub struct HeadData {
    name: String,
    /// Whether the head is gone. Its name may be another head's by now.
    finished: AtomicBool,
}

/// A mode object's data.
#[derive(Debug)]
pub struct ModeData {
    /// The name of the head it's a mode of.
    head: String,
    mode: Mode,
}

/// A configuration's data.
#[derive(Debug)]
pub struct ConfigurationData(Mutex<Configuration>);

#[derive(Debug)]
struct Configuration {
    /// The serial the client made it at.
    serial: u32,
    /// Whether it was applied or tested, after which it takes no more requests.
    used: bool,
    /// Whether it names a head that's gone.
    stale: bool,
    heads: Vec<(String, Option<Arc<Mutex<EnabledHead>>>)>,
}

/// An enabled head's data, in a configuration.
#[derive(Debug)]
pub struct ConfigurationHeadData {
    head: String,
    /// Shared with the configuration, which reads it when it's applied.
    request: Arc<Mutex<EnabledHead>>,
}

impl OutputManagementState {
    /// Offers the manager's global to every client. The security-context filter comes in M5
    /// with the other privileged globals (58).
    pub fn new<D>(display: &DisplayHandle) -> Self
    where
        D: GlobalDispatch<ZwlrOutputManagerV1, ManagerGlobalData> + 'static,
    {
        let global = display.create_global::<D, ZwlrOutputManagerV1, _>(VERSION, ManagerGlobalData);
        Self {
            _global: global,
            serial: 0,
            applied: false,
            heads: Vec::new(),
            managers: Vec::new(),
        }
    }

    /// Tells every client what changed since the heads were last sent, if anything did or a
    /// configuration was applied, and ends the change with `done` and a new serial.
    pub fn refresh<D>(&mut self, display: &DisplayHandle, heads: Vec<Head>)
    where
        D: Dispatch<ZwlrOutputHeadV1, HeadData> + Dispatch<ZwlrOutputModeV1, ModeData> + 'static,
    {
        if heads == self.heads && !self.applied {
            return;
        }
        for old in &self.heads {
            if !heads.iter().any(|new| new.is_same_monitor(old)) {
                for manager in &mut self.managers {
                    manager.finish_head(&old.name);
                }
            }
        }
        for new in &heads {
            match self.heads.iter().find(|old| old.is_same_monitor(new)) {
                Some(old) if old == new => {}
                Some(old) => {
                    for manager in &mut self.managers {
                        manager.update_head::<D>(display, old, new);
                    }
                }
                None => {
                    for manager in &mut self.managers {
                        manager.add_head::<D>(display, new);
                    }
                }
            }
        }
        self.heads = heads;
        if !self.applied {
            self.serial = self.serial.wrapping_add(1);
        }
        self.applied = false;
        for manager in &self.managers {
            manager.resource.done(self.serial);
        }
    }
}

impl Manager {
    /// Sends a new head: what the monitor is, its modes, and what it shows.
    fn add_head<D>(&mut self, display: &DisplayHandle, head: &Head)
    where
        D: Dispatch<ZwlrOutputHeadV1, HeadData> + Dispatch<ZwlrOutputModeV1, ModeData> + 'static,
    {
        let Some(client) = self.resource.client() else {
            return;
        };
        let data = HeadData {
            name: head.name.clone(),
            finished: AtomicBool::new(false),
        };
        let version = self.resource.version();
        let Ok(resource) = client.create_resource::<ZwlrOutputHeadV1, _, D>(display, version, data)
        else {
            return;
        };
        self.resource.head(&resource);
        resource.name(head.name.clone());
        resource.description(head.description.clone());
        if head.physical_size.w > 0 && head.physical_size.h > 0 {
            resource.physical_size(head.physical_size.w, head.physical_size.h);
        }
        if version >= SINCE_MAKE {
            resource.make(head.make.clone());
            resource.model(head.model.clone());
            resource.serial_number(head.serial_number.clone());
        }
        let mut sent = ManagerHead {
            name: head.name.clone(),
            resource,
            modes: Vec::new(),
        };
        for &mode in &head.modes {
            sent.add_mode::<D>(display, &client, mode, head.preferred_mode == Some(mode));
        }
        sent.send_state(head);
        if version >= SINCE_ADAPTIVE_SYNC {
            // Camino has no adaptive sync yet.
            sent.resource.adaptive_sync(AdaptiveSyncState::Disabled);
        }
        self.heads.push(sent);
    }

    /// Sends what changed of a head: the modes it gained first, since what it shows may be
    /// one of them, and the modes it lost last.
    fn update_head<D>(&mut self, display: &DisplayHandle, old: &Head, new: &Head)
    where
        D: Dispatch<ZwlrOutputModeV1, ModeData> + 'static,
    {
        let Some(client) = self.resource.client() else {
            return;
        };
        let Some(sent) = self.heads.iter_mut().find(|sent| sent.name == new.name) else {
            return;
        };
        for &mode in &new.modes {
            if !old.modes.contains(&mode) {
                sent.add_mode::<D>(display, &client, mode, new.preferred_mode == Some(mode));
            }
        }
        if old.enabled != new.enabled {
            sent.send_state(new);
        }
        sent.modes.retain(|(mode, resource)| {
            let kept = new.modes.contains(mode);
            if !kept {
                resource.finished();
            }
            kept
        });
    }

    /// Tells the client a head and its modes are gone.
    fn finish_head(&mut self, name: &str) {
        self.heads.retain(|sent| {
            if sent.name != name {
                return true;
            }
            for (_, mode) in &sent.modes {
                mode.finished();
            }
            if let Some(data) = sent.resource.data::<HeadData>() {
                data.finished.store(true, Ordering::Relaxed);
            }
            sent.resource.finished();
            false
        });
    }
}

impl ManagerHead {
    fn add_mode<D>(&mut self, display: &DisplayHandle, client: &Client, mode: Mode, preferred: bool)
    where
        D: Dispatch<ZwlrOutputModeV1, ModeData> + 'static,
    {
        let data = ModeData {
            head: self.name.clone(),
            mode,
        };
        let version = self.resource.version().min(MODE_VERSION);
        let Ok(resource) = client.create_resource::<ZwlrOutputModeV1, _, D>(display, version, data)
        else {
            return;
        };
        self.resource.mode(&resource);
        resource.size(mode.size.w, mode.size.h);
        resource.refresh(mode.refresh);
        if preferred {
            resource.preferred();
        }
        self.modes.push((mode, resource));
    }

    /// Sends whether the head is enabled and, if it is, what it shows.
    fn send_state(&self, head: &Head) {
        let Some(state) = head.enabled else {
            self.resource.enabled(0);
            return;
        };
        self.resource.enabled(1);
        let current = self
            .modes
            .iter()
            .find(|(mode, _)| Some(*mode) == state.mode);
        if let Some((_, mode)) = current {
            self.resource.current_mode(mode);
        }
        self.resource.position(state.position.x, state.position.y);
        self.resource.transform(state.transform.into());
        self.resource.scale(state.scale);
    }
}

impl<D> GlobalDispatch2<ZwlrOutputManagerV1, D> for ManagerGlobalData
where
    D: OutputManagementHandler
        + Dispatch<ZwlrOutputManagerV1, ManagerData>
        + Dispatch<ZwlrOutputHeadV1, HeadData>
        + Dispatch<ZwlrOutputModeV1, ModeData>,
{
    fn bind(
        &self,
        state: &mut D,
        display: &DisplayHandle,
        _client: &Client,
        resource: New<ZwlrOutputManagerV1>,
        data_init: &mut DataInit<'_, D>,
    ) {
        let mut manager = Manager {
            resource: data_init.init(resource, ManagerData),
            heads: Vec::new(),
        };
        let state = state.output_management_state();
        for head in &state.heads {
            manager.add_head::<D>(display, head);
        }
        manager.resource.done(state.serial);
        state.managers.push(manager);
    }
}

impl<D> Dispatch2<ZwlrOutputManagerV1, D> for ManagerData
where
    D: OutputManagementHandler + Dispatch<ZwlrOutputConfigurationV1, ConfigurationData>,
{
    fn request(
        &self,
        state: &mut D,
        _client: &Client,
        resource: &ZwlrOutputManagerV1,
        request: zwlr_output_manager_v1::Request,
        _display: &DisplayHandle,
        data_init: &mut DataInit<'_, D>,
    ) {
        match request {
            zwlr_output_manager_v1::Request::CreateConfiguration { id, serial } => {
                let configuration = Configuration {
                    serial,
                    used: false,
                    stale: false,
                    heads: Vec::new(),
                };
                data_init.init(id, ConfigurationData(Mutex::new(configuration)));
            }
            zwlr_output_manager_v1::Request::Stop => {
                let managers = &mut state.output_management_state().managers;
                managers.retain(|manager| manager.resource != *resource);
                resource.finished();
            }
            _ => unreachable!("zwlr_output_manager_v1 has no other request"),
        }
    }

    fn destroyed(&self, state: &mut D, _client: ClientId, resource: &ZwlrOutputManagerV1) {
        let managers = &mut state.output_management_state().managers;
        managers.retain(|manager| manager.resource != *resource);
    }
}

impl<D: OutputManagementHandler> Dispatch2<ZwlrOutputHeadV1, D> for HeadData {
    fn request(
        &self,
        _state: &mut D,
        _client: &Client,
        _resource: &ZwlrOutputHeadV1,
        request: zwlr_output_head_v1::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        match request {
            // The object is forgotten in `destroyed`.
            zwlr_output_head_v1::Request::Release => {}
            _ => unreachable!("zwlr_output_head_v1 has no other request"),
        }
    }

    fn destroyed(&self, state: &mut D, _client: ClientId, resource: &ZwlrOutputHeadV1) {
        for manager in &mut state.output_management_state().managers {
            manager.heads.retain(|sent| sent.resource != *resource);
        }
    }
}

impl<D: OutputManagementHandler> Dispatch2<ZwlrOutputModeV1, D> for ModeData {
    fn request(
        &self,
        _state: &mut D,
        _client: &Client,
        _resource: &ZwlrOutputModeV1,
        request: zwlr_output_mode_v1::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        match request {
            // The object is forgotten in `destroyed`.
            zwlr_output_mode_v1::Request::Release => {}
            _ => unreachable!("zwlr_output_mode_v1 has no other request"),
        }
    }

    fn destroyed(&self, state: &mut D, _client: ClientId, resource: &ZwlrOutputModeV1) {
        let heads = state
            .output_management_state()
            .managers
            .iter_mut()
            .flat_map(|manager| &mut manager.heads);
        for head in heads {
            head.modes.retain(|(_, mode)| mode != resource);
        }
    }
}

impl<D> Dispatch2<ZwlrOutputConfigurationV1, D> for ConfigurationData
where
    D: OutputManagementHandler + Dispatch<ZwlrOutputConfigurationHeadV1, ConfigurationHeadData>,
{
    fn request(
        &self,
        state: &mut D,
        _client: &Client,
        resource: &ZwlrOutputConfigurationV1,
        request: zwlr_output_configuration_v1::Request,
        _display: &DisplayHandle,
        data_init: &mut DataInit<'_, D>,
    ) {
        let mut configuration = self.0.lock().unwrap();
        match request {
            zwlr_output_configuration_v1::Request::EnableHead { id, head } => {
                let request = Arc::new(Mutex::new(EnabledHead::default()));
                // A new object is always initialised, whatever is wrong with the request.
                let name = head_name(&head);
                let data = ConfigurationHeadData {
                    head: name.clone(),
                    request: Arc::clone(&request),
                };
                data_init.init(id, data);
                configuration.add_head(resource, &head, name, Some(request));
            }
            zwlr_output_configuration_v1::Request::DisableHead { head } => {
                configuration.add_head(resource, &head, head_name(&head), None);
            }
            zwlr_output_configuration_v1::Request::Apply => {
                if let Some(requests) = configuration.take(state, resource) {
                    // The handler may run requests of this client; none is this one's.
                    drop(configuration);
                    let applied = state.apply_output_configuration(&requests);
                    if applied {
                        let state = state.output_management_state();
                        if !state.applied {
                            state.serial = state.serial.wrapping_add(1);
                        }
                        state.applied = true;
                    }
                    answer(resource, applied);
                }
            }
            zwlr_output_configuration_v1::Request::Test => {
                if let Some(requests) = configuration.take(state, resource) {
                    drop(configuration);
                    answer(resource, state.test_output_configuration(&requests));
                }
            }
            zwlr_output_configuration_v1::Request::Destroy => {}
            _ => unreachable!("zwlr_output_configuration_v1 has no other request"),
        }
    }
}

/// The name of the head a head object is for.
fn head_name(head: &ZwlrOutputHeadV1) -> String {
    head.data::<HeadData>()
        .map(|data| data.name.clone())
        .unwrap_or_default()
}

fn answer(configuration: &ZwlrOutputConfigurationV1, succeeded: bool) {
    if succeeded {
        configuration.succeeded();
    } else {
        configuration.failed();
    }
}

impl Configuration {
    /// Notes what the configuration asks of a head. Naming a head twice, or after the
    /// configuration was used, is a protocol error.
    fn add_head(
        &mut self,
        resource: &ZwlrOutputConfigurationV1,
        head: &ZwlrOutputHeadV1,
        name: String,
        request: Option<Arc<Mutex<EnabledHead>>>,
    ) {
        if self.used {
            resource.post_error(
                zwlr_output_configuration_v1::Error::AlreadyUsed,
                "the configuration was already applied or tested",
            );
            return;
        }
        // A head that's gone can't be named twice: its name may be a new head's.
        let finished = head
            .data::<HeadData>()
            .is_none_or(|data| data.finished.load(Ordering::Relaxed));
        if finished {
            self.stale = true;
            return;
        }
        if self.heads.iter().any(|(configured, _)| *configured == name) {
            resource.post_error(
                zwlr_output_configuration_v1::Error::AlreadyConfiguredHead,
                format!("{name} is already in the configuration"),
            );
            return;
        }
        self.heads.push((name, request));
    }

    /// Uses the configuration up, and returns what it asks if it's to be answered by the
    /// handler. One made at an old serial, or that names a head that's gone, is cancelled
    /// here.
    ///
    /// The protocol makes leaving a head out an error. Camino leaves such a head as it is
    /// instead, as wlroots does, since clients are written against wlroots.
    fn take<D: OutputManagementHandler>(
        &mut self,
        state: &mut D,
        resource: &ZwlrOutputConfigurationV1,
    ) -> Option<Vec<HeadRequest>> {
        if self.used {
            resource.post_error(
                zwlr_output_configuration_v1::Error::AlreadyUsed,
                "the configuration was already applied or tested",
            );
            return None;
        }
        self.used = true;
        if self.stale || self.serial != state.output_management_state().serial {
            resource.cancelled();
            return None;
        }
        let requests = self
            .heads
            .drain(..)
            .map(|(name, request)| HeadRequest {
                name,
                enabled: request.map(|request| *request.lock().unwrap()),
            })
            .collect();
        Some(requests)
    }
}

impl<D: OutputManagementHandler> Dispatch2<ZwlrOutputConfigurationHeadV1, D>
    for ConfigurationHeadData
{
    fn request(
        &self,
        _state: &mut D,
        _client: &Client,
        resource: &ZwlrOutputConfigurationHeadV1,
        request: zwlr_output_configuration_head_v1::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        use zwlr_output_configuration_head_v1::{Error, Request};

        let mut head = self.request.lock().unwrap();
        let already_set = |property: &str| {
            resource.post_error(Error::AlreadySet, format!("the {property} is already set"));
        };
        match request {
            Request::SetMode { mode } => {
                let Some(data) = mode
                    .data::<ModeData>()
                    .filter(|data| data.head == self.head)
                else {
                    resource.post_error(Error::InvalidMode, "the mode isn't one of the head's");
                    return;
                };
                if head.mode.is_some() {
                    return already_set("mode");
                }
                head.mode = Some(data.mode);
            }
            Request::SetCustomMode {
                width,
                height,
                refresh,
            } => {
                if width <= 0 || height <= 0 || refresh < 0 {
                    resource.post_error(
                        Error::InvalidCustomMode,
                        format!("{width}x{height}@{refresh} isn't a mode"),
                    );
                    return;
                }
                if head.mode.is_some() {
                    return already_set("mode");
                }
                head.mode = Some(Mode {
                    size: (width, height).into(),
                    refresh,
                });
            }
            Request::SetPosition { x, y } => {
                if head.position.is_some() {
                    return already_set("position");
                }
                head.position = Some((x, y).into());
            }
            Request::SetTransform { transform } => {
                let WEnum::Value(transform) = transform else {
                    resource.post_error(Error::InvalidTransform, "the transform is unknown");
                    return;
                };
                if head.transform.is_some() {
                    return already_set("transform");
                }
                head.transform = Some(transform.into());
            }
            Request::SetScale { scale } => {
                if scale <= 0.0 {
                    resource.post_error(Error::InvalidScale, format!("{scale} isn't a scale"));
                    return;
                }
                if head.scale.is_some() {
                    return already_set("scale");
                }
                head.scale = Some(snap_scale(scale));
            }
            Request::SetAdaptiveSync { state } => {
                let WEnum::Value(state) = state else {
                    resource.post_error(
                        Error::InvalidAdaptiveSyncState,
                        "the adaptive sync state is unknown",
                    );
                    return;
                };
                if head.adaptive_sync.is_some() {
                    return already_set("adaptive sync state");
                }
                head.adaptive_sync = Some(state == AdaptiveSyncState::Enabled);
            }
            _ => unreachable!("zwlr_output_configuration_head_v1 has no other request"),
        }
    }
}

/// A scale a client asked for, snapped to fractional-scale-v1's 1/120 steps (13), and never
/// to zero.
fn snap_scale(scale: f64) -> f64 {
    (scale * 120.0).round().max(1.0) / 120.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_snap_to_120ths() {
        assert_eq!(snap_scale(1.5), 1.5);
        assert_eq!(snap_scale(1.333), 160.0 / 120.0);
    }

    #[test]
    fn a_scale_sent_back_as_a_fixed_snaps_to_what_it_was() {
        // 1.7 as a `wl_fixed` is 435/256.
        assert_eq!(snap_scale(435.0 / 256.0), 1.7);
    }

    #[test]
    fn the_smallest_scale_is_a_120th() {
        assert_eq!(snap_scale(0.001), 1.0 / 120.0);
    }
}
