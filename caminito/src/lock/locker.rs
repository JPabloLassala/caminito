//! The locker command (12, 126): a lock request that isn't a locker's own spawns
//! `locker-command` through the `helpers` supervisor, and the session stays locked whatever
//! the locker does. A locker that exits while locked is started again with the
//! supervisor's backoff, and takes the lock over (125's rule, matched by its process). After
//! the supervisor's five failed starts within a minute Caminito gives up on it and draws one
//! line on the lock: how to unlock from a text console with logind.
//!
//! A locker that forks into the background, as `swaylock -f` does, can't take the lock: the
//! process that asks for it isn't the one Caminito spawned, and the one it spawned exits.

use std::ffi::OsString;
use std::path::Path;

use caminito_theme::cairo::{self, Context, Format, ImageSurface};
use caminito_theme::{pango, pangocairo};
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::output::Output;
use smithay::reexports::calloop::LoopHandle;
use smithay::utils::{Logical, Size};

use crate::decoration::memory_buffer;
use crate::helpers::{Helper, HelperCommand, HelperEvent, RestartPolicy, StderrTarget};
use crate::output::output_geometry;
use crate::settings::translate::LockerSettings;
use crate::state::{Caminito, State};

use super::LockState;

/// The identifier the locker's stderr is logged under when its command names no program.
const DEFAULT_IDENTIFIER: &str = "caminito-locker";

/// The colour of the give-up message, on the lock's black background.
const MESSAGE_COLOR: (f64, f64, f64) = (1.0, 1.0, 1.0);
/// The space kept clear on either side of the message, in logical pixels.
const MESSAGE_MARGIN: i32 = 32;

/// The locker Caminito spawns, and what it shows once it gave up on it.
pub(crate) struct Locker {
    /// The locker of the current lock, spawned with the command the settings had when the
    /// lock was asked for. A new lock gets a new one, so its failed starts are its own.
    helper: Helper<State>,
    supervision: Supervision,
    /// When a locker that exited is started again, and when it's given up on.
    pub(crate) policy: RestartPolicy,
    /// The logind session's ID, `XDG_SESSION_ID`, which the give-up message names.
    pub(crate) session_id: Option<String>,
    /// Caminito's Wayland socket, the locker's `WAYLAND_DISPLAY`.
    pub(crate) socket: Option<OsString>,
    /// The give-up message as each output shows it, with the scale it's drawn at and its
    /// size.
    messages: Vec<Message>,
}

/// Whether Caminito keeps a locker of its own running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Supervision {
    /// No: none was asked for, or the session unlocked.
    None,
    /// One is about to be spawned, the first or again.
    Starting,
    /// One runs.
    Running,
    /// It failed to start too often, and won't be started again.
    GaveUp,
}

/// The give-up message drawn for one output.
struct Message {
    output: Output,
    scale: i32,
    size: Size<i32, Logical>,
    buffer: MemoryRenderBuffer,
}

impl Locker {
    pub(crate) fn new(handle: LoopHandle<'static, State>) -> Self {
        let command = HelperCommand::new(DEFAULT_IDENTIFIER, DEFAULT_IDENTIFIER);
        Locker {
            helper: helper(
                handle,
                command,
                RestartPolicy::default(),
                StderrTarget::Prefixed,
            ),
            supervision: Supervision::None,
            policy: RestartPolicy::default(),
            session_id: std::env::var("XDG_SESSION_ID")
                .ok()
                .filter(|id| !id.is_empty()),
            socket: None,
            messages: Vec::new(),
        }
    }

    /// The supervision's name in the debug dump.
    pub(crate) fn state_name(&self) -> &'static str {
        match self.supervision {
            Supervision::None => "none",
            Supervision::Starting => "starting",
            Supervision::Running => "running",
            Supervision::GaveUp => "gave-up",
        }
    }

    pub(crate) fn gave_up(&self) -> bool {
        self.supervision == Supervision::GaveUp
    }

    /// The line drawn on the lock once Caminito gave up on the locker (12). A bare
    /// `loginctl unlock-session` from a text console acts on the console's own session, so
    /// it names this one, or else unlocks every session.
    pub(crate) fn message(&self) -> String {
        let command = match &self.session_id {
            Some(id) => format!("loginctl unlock-session {id}"),
            None => "loginctl unlock-sessions".to_owned(),
        };
        format!(
            "The screen locker can't start. To unlock, switch to a text console and run: {command}"
        )
    }

    /// The give-up message on `output`, with its size, if it's drawn.
    pub(crate) fn message_on(
        &self,
        output: &Output,
    ) -> Option<(&MemoryRenderBuffer, Size<i32, Logical>)> {
        let mut messages = self.messages.iter();
        let message = messages.find(|message| message.output == *output)?;
        Some((&message.buffer, message.size))
    }

    /// The session unlocked: the locker that ran is let be, and none is started again.
    pub(crate) fn finish(&mut self) {
        self.helper.finish();
        self.supervision = Supervision::None;
        self.messages.clear();
    }

    /// Stops the locker for good, as Caminito tears down (08).
    pub(crate) fn stop(&mut self) {
        self.helper.stop();
        self.supervision = Supervision::None;
    }
}

/// A helper for the locker, which isn't started.
fn helper(
    handle: LoopHandle<'static, State>,
    command: HelperCommand,
    policy: RestartPolicy,
    stderr: StderrTarget,
) -> Helper<State> {
    Helper::new(
        handle,
        command,
        policy,
        stderr,
        |state: &mut State| &mut state.caminito.locker.helper,
        locker_event,
    )
}

/// The locker's command from the settings, run on Caminito's socket with the variables
/// Caminito sets for the clients it spawns (08). An empty argv is a program that can't be
/// found, so a failed start.
fn command(
    settings: &LockerSettings,
    environment: &std::collections::BTreeMap<OsString, OsString>,
    socket: Option<&OsString>,
) -> HelperCommand {
    let (program, args) = match settings.command.split_first() {
        Some((program, args)) => (program.as_str(), args),
        None => ("", &[][..]),
    };
    let identifier = Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(DEFAULT_IDENTIFIER);
    let mut command = HelperCommand::new(identifier, program)
        .args(args)
        .env("GDK_BACKEND", "wayland");
    for (name, value) in environment {
        command = command.env(name, value);
    }
    if let Some(socket) = socket {
        command = command.env("WAYLAND_DISPLAY", socket);
    }
    command
}

fn locker_event(state: &mut State, event: HelperEvent<'_>) {
    let caminito = &mut state.caminito;
    let locker = &mut caminito.locker;
    match event {
        HelperEvent::Started => {
            caminito.lock.spawned_locker = locker.helper.process();
            if locker.supervision == Supervision::Starting {
                locker.supervision = Supervision::Running;
            }
        }
        HelperEvent::Stdout(bytes) => {
            tracing::debug!("locker: {}", String::from_utf8_lossy(bytes).trim_end());
        }
        HelperEvent::Exited => {
            caminito.lock.spawned_locker = None;
            if locker.supervision == Supervision::Running {
                locker.supervision = Supervision::Starting;
            }
        }
        HelperEvent::GaveUp => {
            locker.supervision = Supervision::GaveUp;
            tracing::error!(
                "gave up on the locker; the session stays locked: {}",
                locker.message()
            );
            caminito.queue_redraw_all();
        }
    }
}

impl State {
    /// Asks for the session lock, as Caminito's own request (12): the lock D-Bus asks for,
    /// idle's and logind's. The session is locked at once, and the locker is spawned to take
    /// the lock. Returns whether the request was accepted: `org.mate.lockdown
    /// disable-lock-screen` refuses every one, as mate-screensaver does.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "127, 128 and 130 are its callers")
    )]
    pub(crate) fn request_lock(&mut self) -> bool {
        let caminito = &mut self.caminito;
        if caminito.settings.locker.disabled {
            tracing::warn!("can't lock the session: org.mate.lockdown disable-lock-screen is on");
            return false;
        }
        // The locker that held the lock may have gone in this same dispatch.
        caminito.notice_locker_gone();
        let was_locked = caminito.lock.is_locked();
        caminito.lock.state = caminito.lock.state.requested();
        if !was_locked {
            caminito.queue_redraw_all();
            self.session_locked();
        }
        // A lock that a locker holds is its own, and one Caminito keeps a locker for already
        // has one coming.
        if self.caminito.lock.state() != LockState::Locked {
            self.caminito.spawn_locker();
        }
        true
    }
}

impl Caminito {
    /// Spawns the locker for a lock no locker holds, with the command the settings have
    /// now, unless one runs or is coming for it, or Caminito gave up on it.
    fn spawn_locker(&mut self) {
        let locker = &mut self.locker;
        if locker.supervision != Supervision::None {
            return;
        }
        let settings = &self.settings.locker;
        if settings.command.is_empty() {
            tracing::warn!("org.mate.Caminito locker-command is empty, so no locker can start");
        }
        let command = command(settings, &self.environment, locker.socket.as_ref());
        // The locker of an earlier lock that's still exiting is ended here.
        locker.helper = helper(
            self.loop_handle.clone(),
            command,
            locker.policy,
            self.helper_stderr,
        );
        locker.supervision = Supervision::Starting;
        self.lock.spawned_locker = None;
        locker.helper.start();
    }

    /// The refresh pass's part of the locker: once Caminito gave up on it, the message is
    /// drawn for each output at its scale.
    pub(crate) fn refresh_locker_message(&mut self) {
        let locker = &mut self.locker;
        if !locker.gave_up() {
            locker.messages.clear();
            return;
        }
        let outputs = &self.outputs;
        locker.messages.retain(|message| {
            outputs.contains(&message.output) && message.scale == drawn_scale(&message.output)
        });
        let text = locker.message();
        let font = pango::FontDescription::from_string(&self.settings.interface.font_name);
        let mut drawn = Vec::new();
        for output in outputs {
            if locker
                .messages
                .iter()
                .any(|message| message.output == *output)
            {
                continue;
            }
            let scale = drawn_scale(output);
            let width = output_geometry(output).size.w - 2 * MESSAGE_MARGIN;
            let context = self.frame_context.text_context();
            match draw_message(context, &font, &text, width, scale) {
                Ok((buffer, size)) => locker.messages.push(Message {
                    output: output.clone(),
                    scale,
                    size,
                    buffer,
                }),
                Err(err) => tracing::warn!("can't draw the locker's give-up message: {err}"),
            }
            drawn.push(output.clone());
        }
        for output in &drawn {
            self.queue_redraw(output);
        }
    }
}

/// The scale a buffer for `output` is drawn at: its fractional scale rounded up, which the
/// renderer scales down.
fn drawn_scale(output: &Output) -> i32 {
    output.current_scale().fractional_scale().ceil() as i32
}

/// Draws `text` in `font`, wrapped to `width` logical pixels if it's wider, at `scale`.
fn draw_message(
    context: &pango::Context,
    font: &pango::FontDescription,
    text: &str,
    width: i32,
    scale: i32,
) -> Result<(MemoryRenderBuffer, Size<i32, Logical>), cairo::Error> {
    let layout = pango::Layout::new(context);
    layout.set_font_description(Some(font));
    layout.set_text(text);
    layout.set_width(width.max(1) * pango::SCALE);
    layout.set_wrap(pango::WrapMode::WordChar);
    let (w, h) = layout.pixel_size();
    let size = Size::from((w.max(1), h.max(1)));
    let surface = ImageSurface::create(Format::ARgb32, size.w * scale, size.h * scale)?;
    {
        let cr = Context::new(&surface)?;
        cr.scale(f64::from(scale), f64::from(scale));
        let (red, green, blue) = MESSAGE_COLOR;
        cr.set_source_rgb(red, green, blue);
        pangocairo::functions::show_layout(&cr, &layout);
    }
    let buffer = memory_buffer(surface, scale).ok_or(cairo::Error::NoMemory)?;
    Ok((buffer, size))
}
