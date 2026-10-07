//! A `zwlr_output_management_unstable_v1` client, as `wlr-randr` is one (58): it records the
//! heads and modes the compositor shows, and sends configurations that say what every head
//! is to be, as `wlr-randr` does.

use wayland_client::protocol::wl_output::Transform;
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum, event_created_child};
use wayland_protocols_wlr::output_management::v1::client::zwlr_output_configuration_head_v1::{
    self, ZwlrOutputConfigurationHeadV1,
};
use wayland_protocols_wlr::output_management::v1::client::zwlr_output_configuration_v1::{
    self, ZwlrOutputConfigurationV1,
};
use wayland_protocols_wlr::output_management::v1::client::zwlr_output_head_v1::{
    self, AdaptiveSyncState, ZwlrOutputHeadV1,
};
use wayland_protocols_wlr::output_management::v1::client::zwlr_output_manager_v1::{
    self, ZwlrOutputManagerV1,
};
use wayland_protocols_wlr::output_management::v1::client::zwlr_output_mode_v1::{
    self, ZwlrOutputModeV1,
};

use crate::{Client, State};

/// What output management has told the client.
#[derive(Default)]
pub struct OutputManagement {
    manager: Option<ZwlrOutputManagerV1>,
    /// The heads there are, in the order the compositor announced them.
    pub heads: Vec<Head>,
    /// The serial of the last `done`.
    pub serial: Option<u32>,
    /// How the compositor answered each configuration, oldest first.
    pub outcomes: Vec<Outcome>,
}

/// A head: a monitor, lit or not.
pub struct Head {
    proxy: ZwlrOutputHeadV1,
    pub name: String,
    pub description: String,
    /// In millimetres.
    pub physical_size: Option<(i32, i32)>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub modes: Vec<Mode>,
    pub enabled: bool,
    current_mode: Option<ZwlrOutputModeV1>,
    pub position: (i32, i32),
    pub transform: Transform,
    pub scale: f64,
    pub adaptive_sync: Option<bool>,
}

impl Head {
    /// The mode the head shows, if it's enabled.
    pub fn current_mode(&self) -> Option<&Mode> {
        let current = self.current_mode.as_ref()?;
        self.modes.iter().find(|mode| mode.proxy == *current)
    }
}

/// One of a head's modes.
pub struct Mode {
    proxy: ZwlrOutputModeV1,
    pub size: (i32, i32),
    /// In mHz.
    pub refresh: i32,
    pub preferred: bool,
}

/// The compositor's answer to a configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Succeeded,
    Failed,
    Cancelled,
}

/// What a configuration changes of one head. What it doesn't change is sent as the head is.
#[derive(Debug, Clone, PartialEq)]
pub struct HeadChange {
    name: String,
    enabled: Option<bool>,
    mode: Option<ModeChange>,
    position: Option<(i32, i32)>,
    scale: Option<f64>,
    transform: Option<Transform>,
    adaptive_sync: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ModeChange {
    /// The first of the head's modes of this size.
    Listed((i32, i32)),
    /// A custom mode: its size, and its refresh rate in mHz.
    Custom((i32, i32), i32),
}

impl HeadChange {
    /// No change to the head named `name`.
    pub fn of(name: &str) -> Self {
        HeadChange {
            name: name.to_owned(),
            enabled: None,
            mode: None,
            position: None,
            scale: None,
            transform: None,
            adaptive_sync: false,
        }
    }

    /// Enables the head, in its preferred mode unless a mode is given.
    pub fn enable(mut self) -> Self {
        self.enabled = Some(true);
        self
    }

    /// Disables the head.
    pub fn disable(mut self) -> Self {
        self.enabled = Some(false);
        self
    }

    /// The first of the head's modes of `size`.
    pub fn mode(mut self, size: (i32, i32)) -> Self {
        self.mode = Some(ModeChange::Listed(size));
        self
    }

    /// A mode of `size` at `refresh` mHz that needn't be one of the head's.
    pub fn custom_mode(mut self, size: (i32, i32), refresh: i32) -> Self {
        self.mode = Some(ModeChange::Custom(size, refresh));
        self
    }

    /// Places the head at `position` in the global space.
    pub fn position(mut self, position: (i32, i32)) -> Self {
        self.position = Some(position);
        self
    }

    /// Sets the head's scale.
    pub fn scale(mut self, scale: f64) -> Self {
        self.scale = Some(scale);
        self
    }

    /// Sets the head's transform.
    pub fn transform(mut self, transform: Transform) -> Self {
        self.transform = Some(transform);
        self
    }

    /// Asks for adaptive sync.
    pub fn adaptive_sync(mut self) -> Self {
        self.adaptive_sync = true;
        self
    }
}

impl Client {
    /// Binds output management. Roundtrip before reading the heads.
    pub fn bind_output_management(&mut self) {
        let manager = crate::bind(&self.registry, &self.state.globals, &self.qh, 4);
        self.state.output_management.manager = Some(manager);
    }

    /// What output management has told the client.
    pub fn output_management(&self) -> &OutputManagement {
        &self.state.output_management
    }

    /// The serial of the last `done`.
    pub fn output_serial(&self) -> u32 {
        self.output_management()
            .serial
            .expect("bind_output_management and roundtrip first")
    }

    /// Applies a configuration, made at the last serial, that changes the heads as `changes`
    /// say and keeps the rest as it is. Roundtrip, then read the outcome.
    pub fn apply_outputs(&mut self, changes: &[HeadChange]) {
        let configuration = self.output_configuration(changes, self.output_serial());
        configuration.apply();
    }

    /// Applies a configuration made at `serial`, as `apply_outputs` does at the last one.
    pub fn apply_outputs_at(&mut self, changes: &[HeadChange], serial: u32) {
        self.output_configuration(changes, serial).apply();
    }

    /// Tests a configuration, as `apply_outputs` applies one.
    pub fn test_outputs(&mut self, changes: &[HeadChange]) {
        let configuration = self.output_configuration(changes, self.output_serial());
        configuration.test();
    }

    /// A configuration that says what every head is to be, as `wlr-randr` makes one.
    fn output_configuration(
        &mut self,
        changes: &[HeadChange],
        serial: u32,
    ) -> ZwlrOutputConfigurationV1 {
        let management = &self.state.output_management;
        let manager = management
            .manager
            .as_ref()
            .expect("bind_output_management first");
        for change in changes {
            assert!(
                management.heads.iter().any(|head| head.name == change.name),
                "there's a head named {}",
                change.name
            );
        }
        let configuration = manager.create_configuration(serial, &self.qh, ());
        for head in &management.heads {
            let unchanged = HeadChange::of(&head.name);
            let change = changes
                .iter()
                .find(|change| change.name == head.name)
                .unwrap_or(&unchanged);
            if !change.enabled.unwrap_or(head.enabled) {
                configuration.disable_head(&head.proxy);
                continue;
            }
            let enabled = configuration.enable_head(&head.proxy, &self.qh, ());
            match change.mode {
                Some(ModeChange::Listed(size)) => {
                    let mode = head
                        .modes
                        .iter()
                        .find(|mode| mode.size == size)
                        .unwrap_or_else(|| panic!("{} has a mode of {size:?}", head.name));
                    enabled.set_mode(&mode.proxy);
                }
                Some(ModeChange::Custom((width, height), refresh)) => {
                    enabled.set_custom_mode(width, height, refresh);
                }
                None => {
                    if let Some(mode) = head.current_mode() {
                        enabled.set_mode(&mode.proxy);
                    }
                }
            }
            // A head that was disabled has no place, scale or transform to send back.
            if let Some((x, y)) = change.position.or(head.enabled.then_some(head.position)) {
                enabled.set_position(x, y);
            }
            if let Some(scale) = change.scale.or(head.enabled.then_some(head.scale)) {
                enabled.set_scale(scale);
            }
            if let Some(transform) = change.transform.or(head.enabled.then_some(head.transform)) {
                enabled.set_transform(transform);
            }
            if change.adaptive_sync {
                enabled.set_adaptive_sync(AdaptiveSyncState::Enabled);
            }
        }
        configuration
    }
}

/// The heads as text: each head's name and what the monitor is, its modes, and what it
/// shows.
pub fn format_heads(heads: &[Head]) -> String {
    let text = |text: &Option<String>| {
        match text.as_deref() {
            None => "none",
            Some("") => "(empty)",
            Some(text) => text,
        }
        .to_owned()
    };
    let mode = |mode: &Mode| format!("{}x{}@{}", mode.size.0, mode.size.1, mode.refresh);
    heads
        .iter()
        .map(|head| {
            let size = head
                .physical_size
                .map_or("no size".to_owned(), |(w, h)| format!("{w}x{h} mm"));
            let modes: Vec<String> = head
                .modes
                .iter()
                .map(|m| {
                    let preferred = if m.preferred { " preferred" } else { "" };
                    format!("{}{preferred}", mode(m))
                })
                .collect();
            let shows = if head.enabled {
                format!(
                    "enabled: {} at {},{}, scale {}, transform {:?}",
                    head.current_mode().map_or("no mode".to_owned(), mode),
                    head.position.0,
                    head.position.1,
                    head.scale,
                    head.transform,
                )
            } else {
                "disabled".to_owned()
            };
            let adaptive_sync = match head.adaptive_sync {
                Some(true) => "on",
                Some(false) => "off",
                None => "unknown",
            };
            format!(
                "{}: {}, {size}, make {}, model {}, serial {}\n  modes: {}\n  {shows}\n  adaptive sync {adaptive_sync}",
                head.name,
                head.description,
                text(&head.make),
                text(&head.model),
                text(&head.serial_number),
                modes.join(", "),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl Dispatch<ZwlrOutputManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ZwlrOutputManagerV1,
        event: zwlr_output_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let management = &mut state.output_management;
        match event {
            zwlr_output_manager_v1::Event::Head { head } => management.heads.push(Head {
                proxy: head,
                name: String::new(),
                description: String::new(),
                physical_size: None,
                make: None,
                model: None,
                serial_number: None,
                modes: Vec::new(),
                enabled: false,
                current_mode: None,
                position: (0, 0),
                transform: Transform::Normal,
                scale: 1.0,
                adaptive_sync: None,
            }),
            zwlr_output_manager_v1::Event::Done { serial } => management.serial = Some(serial),
            zwlr_output_manager_v1::Event::Finished => management.manager = None,
            _ => {}
        }
    }

    event_created_child!(State, ZwlrOutputManagerV1, [
        zwlr_output_manager_v1::EVT_HEAD_OPCODE => (ZwlrOutputHeadV1, ()),
    ]);
}

impl Dispatch<ZwlrOutputHeadV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ZwlrOutputHeadV1,
        event: zwlr_output_head_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let heads = &mut state.output_management.heads;
        let Some(index) = heads.iter().position(|head| head.proxy == *proxy) else {
            return;
        };
        let head = &mut heads[index];
        match event {
            zwlr_output_head_v1::Event::Name { name } => head.name = name,
            zwlr_output_head_v1::Event::Description { description } => {
                head.description = description;
            }
            zwlr_output_head_v1::Event::PhysicalSize { width, height } => {
                head.physical_size = Some((width, height));
            }
            zwlr_output_head_v1::Event::Mode { mode } => head.modes.push(Mode {
                proxy: mode,
                size: (0, 0),
                refresh: 0,
                preferred: false,
            }),
            zwlr_output_head_v1::Event::Enabled { enabled } => {
                head.enabled = enabled != 0;
                if !head.enabled {
                    head.current_mode = None;
                }
            }
            zwlr_output_head_v1::Event::CurrentMode { mode } => head.current_mode = Some(mode),
            zwlr_output_head_v1::Event::Position { x, y } => head.position = (x, y),
            zwlr_output_head_v1::Event::Transform {
                transform: WEnum::Value(transform),
            } => head.transform = transform,
            zwlr_output_head_v1::Event::Scale { scale } => head.scale = scale,
            zwlr_output_head_v1::Event::Make { make } => head.make = Some(make),
            zwlr_output_head_v1::Event::Model { model } => head.model = Some(model),
            zwlr_output_head_v1::Event::SerialNumber { serial_number } => {
                head.serial_number = Some(serial_number);
            }
            zwlr_output_head_v1::Event::AdaptiveSync {
                state: WEnum::Value(sync),
            } => head.adaptive_sync = Some(sync == AdaptiveSyncState::Enabled),
            zwlr_output_head_v1::Event::Finished => {
                let head = heads.remove(index);
                for mode in head.modes {
                    mode.proxy.release();
                }
                head.proxy.release();
            }
            _ => {}
        }
    }

    event_created_child!(State, ZwlrOutputHeadV1, [
        zwlr_output_head_v1::EVT_MODE_OPCODE => (ZwlrOutputModeV1, ()),
    ]);
}

impl Dispatch<ZwlrOutputModeV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ZwlrOutputModeV1,
        event: zwlr_output_mode_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some((head, index)) = state.output_management.heads.iter_mut().find_map(|head| {
            let index = head.modes.iter().position(|mode| mode.proxy == *proxy)?;
            Some((head, index))
        }) else {
            return;
        };
        let mode = &mut head.modes[index];
        match event {
            zwlr_output_mode_v1::Event::Size { width, height } => mode.size = (width, height),
            zwlr_output_mode_v1::Event::Refresh { refresh } => mode.refresh = refresh,
            zwlr_output_mode_v1::Event::Preferred => mode.preferred = true,
            zwlr_output_mode_v1::Event::Finished => {
                head.modes.remove(index).proxy.release();
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrOutputConfigurationV1, ()> for State {
    fn event(
        state: &mut Self,
        configuration: &ZwlrOutputConfigurationV1,
        event: zwlr_output_configuration_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let outcome = match event {
            zwlr_output_configuration_v1::Event::Succeeded => Outcome::Succeeded,
            zwlr_output_configuration_v1::Event::Failed => Outcome::Failed,
            zwlr_output_configuration_v1::Event::Cancelled => Outcome::Cancelled,
            _ => return,
        };
        state.output_management.outcomes.push(outcome);
        configuration.destroy();
    }
}

impl Dispatch<ZwlrOutputConfigurationHeadV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwlrOutputConfigurationHeadV1,
        _: zwlr_output_configuration_head_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
