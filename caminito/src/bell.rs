//! The one bell event (105): `gtk_shell1`'s, xdg-system-bell's and XKB's bells, and AccessX's
//! beeps (134), made one event for the window it's for (06). The event is counted and the
//! last one kept for the debug dump, the visual bell flashes for it (169), and the audible
//! bell plays it (170).

use std::time::Duration;

use caminito_policy::{OutputId, WindowId};
use smithay::backend::renderer::element::Id;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::compositor::get_parent;

use crate::output::output_id;
use crate::scene::pointer_on;
use crate::settings::translate::VisualBellType;
use crate::sound::Sound;
use crate::state::{Caminito, SEAT};

/// How long one flash of the visual bell lasts: Marco's frame flash's 100 ms
/// (`bell.c:bell_flash_window_frame`), which the full-screen flash takes too (169).
pub(crate) const FLASH: Duration = Duration::from_millis(100);

/// Which protocol rang a bell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BellSource {
    /// GTK3's `gtk_shell1.system_bell` (67).
    GtkShell,
    /// xdg-system-bell, which GTK4 and others ring.
    XdgSystemBell,
    /// `XkbBellNotify` on Caminito's X connection, for every X client (38).
    Xkb,
    /// One of AccessX's beeps, from Caminito's own keyboard filter (134).
    AccessX,
}

impl BellSource {
    /// The source as the debug dump names it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            BellSource::GtkShell => "gtk_shell1",
            BellSource::XdgSystemBell => "xdg_system_bell",
            BellSource::Xkb => "xkb",
            BellSource::AccessX => "accessx",
        }
    }
}

/// What a bell names, which decides its window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BellFor {
    /// Nothing: a display beep, for the focused window (06).
    Focus,
    /// A window beep, for this window, or for none when what the bell named is gone or isn't
    /// a window's.
    Window(Option<WindowId>),
}

/// One bell, as the event has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BellEvent {
    /// The protocol that rang it.
    pub(crate) source: BellSource,
    /// The window it's for, if any.
    pub(crate) window: Option<WindowId>,
}

/// What one flash of the visual bell flashes (169).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flashed {
    /// Every output, for `fullscreen`.
    Outputs,
    /// One output, for `frame_flash` where there's no frame to flash: the window's, or the
    /// pointer's for a bell for no window.
    Output(OutputId),
    /// A window's frame, for `frame_flash`.
    Frame(WindowId),
}

/// The visual bell's flash, while one shows.
#[derive(Debug)]
struct Flash {
    /// What it flashes.
    flashed: Flashed,
    /// When it's over, on Caminito's clock.
    until: Duration,
    /// The full-screen flash's id in the scene, so it's damaged once and not at every frame.
    id: Id,
}

/// The bells rung since Caminito started, and the flash of the last, while it shows.
#[derive(Debug, Default)]
pub(crate) struct Bells {
    /// How many have rung.
    pub(crate) count: u64,
    /// The last one rung.
    pub(crate) last: Option<BellEvent>,
    /// The visual bell's flash of the last, while it shows (169).
    flash: Option<Flash>,
}

impl Bells {
    /// When the flash is over, while one shows: `deadlines` ends it then (174).
    pub(crate) fn deadline(&self) -> Option<Duration> {
        self.flash.as_ref().map(|flash| flash.until)
    }

    /// The id of the full-screen flash the scene draws over `output`, while it flashes.
    pub(crate) fn output_flash(&self, output: OutputId) -> Option<&Id> {
        let flash = self.flash.as_ref()?;
        match flash.flashed {
            Flashed::Outputs => Some(&flash.id),
            Flashed::Output(flashed) => (flashed == output).then_some(&flash.id),
            Flashed::Frame(_) => None,
        }
    }

    /// Whether the visual bell flashes `window`'s frame, which is then drawn in the other
    /// focus state, as Marco flashes it (`frame.c:meta_frame_get_flags`'s `META_FRAME_IS_FLASHING`).
    pub(crate) fn flashes_frame(&self, window: WindowId) -> bool {
        self.flash
            .as_ref()
            .is_some_and(|flash| flash.flashed == Flashed::Frame(window))
    }
}

impl Caminito {
    /// Rings the bell from `source`: flashes it if the visual bell is on (169), and plays the
    /// sound theme's bell if the audible bell is (170). Marco's audible bell goes through
    /// GTK's canberra context, which `event-sounds` turns off; Caminito's doesn't, as 09 has
    /// it.
    pub(crate) fn ring_bell(&mut self, source: BellSource, bell_for: BellFor) {
        let window = match bell_for {
            BellFor::Focus => self.policy.focus(SEAT),
            BellFor::Window(window) => window,
        };
        self.bells.count += 1;
        self.bells.last = Some(BellEvent { source, window });

        let settings = &self.settings.bell;
        tracing::debug!(
            "{} rang the bell for {window:?}: audible {} with the {:?} sound theme, visual {} \
             as {:?}",
            source.name(),
            settings.audible,
            self.settings.sound.theme_name,
            settings.visual,
            settings.visual_type,
        );
        let audible = settings.audible;
        if settings.visual {
            self.flash_bell(settings.visual_type, window);
        }
        if audible {
            self.play_sound(Sound::Bell);
        }
    }

    /// Flashes the visual bell as `visual_type` says, for one flash from now. A bell during
    /// another's flash starts the flash again, for its own window.
    fn flash_bell(&mut self, visual_type: VisualBellType, window: Option<WindowId>) {
        let flashed = match visual_type {
            VisualBellType::Fullscreen => Flashed::Outputs,
            VisualBellType::FrameFlash => match window.filter(|&id| self.shows_frame(id)) {
                Some(id) => Flashed::Frame(id),
                None => match self.bell_output(window) {
                    Some(output) => Flashed::Output(output),
                    None => Flashed::Outputs,
                },
            },
        };
        self.end_flash();
        self.bells.flash = Some(Flash {
            flashed,
            until: self.now() + FLASH,
            id: Id::new(),
        });
        self.queue_redraw_all();
    }

    /// Whether `window` shows a frame to flash.
    fn shows_frame(&self, window: WindowId) -> bool {
        self.policy.is_showing(window)
            && self
                .policy
                .window(window)
                .is_some_and(|mapped| mapped.shows_frame())
    }

    /// The output a `frame_flash` bell with no frame to flash flashes: `window`'s, or the
    /// pointer's, as Marco flashes the screen where there's no frame
    /// (`bell.c:bell_visual_notify`).
    fn bell_output(&self, window: Option<WindowId>) -> Option<OutputId> {
        let window_output = window
            .and_then(|id| self.policy.window(id))
            .and_then(|mapped| mapped.main_output());
        let output = window_output.or_else(|| {
            self.outputs
                .iter()
                .find(|output| pointer_on(self, output).is_some())
        })?;
        Some(output_id(output))
    }

    /// Ends the flash if it's due at `now`, for `deadlines`.
    pub(crate) fn bell_elapse(&mut self, now: Duration) {
        if self.bells.deadline().is_some_and(|until| until <= now) {
            self.end_flash();
        }
    }

    /// Ends the flash, if one shows.
    fn end_flash(&mut self) {
        if self.bells.flash.take().is_some() {
            self.queue_redraw_all();
        }
    }

    /// The window a bell for `surface` is for: its root surface's, so a subsurface's bell is
    /// its window's, and none for a surface of no window, such as a popup's or a layer
    /// surface's.
    pub(crate) fn bell_window(&self, surface: &WlSurface) -> Option<WindowId> {
        let mut root = surface.clone();
        while let Some(parent) = get_parent(&root) {
            root = parent;
        }
        self.mapped_for_surface(&root)
    }
}
