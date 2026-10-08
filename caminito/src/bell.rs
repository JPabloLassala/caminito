//! The one bell event (105): `gtk_shell1`'s, xdg-system-bell's and XKB's bells, made one
//! event for the window it's for (06). Until M8 shows and plays it (17), the event is counted
//! and the last one kept for the debug dump.

use caminito_policy::WindowId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::compositor::get_parent;

use crate::state::{Caminito, SEAT};

/// Which protocol rang a bell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BellSource {
    /// GTK3's `gtk_shell1.system_bell` (67).
    GtkShell,
    /// xdg-system-bell, which GTK4 and others ring.
    XdgSystemBell,
    /// `XkbBellNotify` on Caminito's X connection, for every X client (38).
    Xkb,
}

impl BellSource {
    /// The source as the debug dump names it.
    pub fn name(self) -> &'static str {
        match self {
            BellSource::GtkShell => "gtk_shell1",
            BellSource::XdgSystemBell => "xdg_system_bell",
            BellSource::Xkb => "xkb",
        }
    }
}

/// What a bell names, which decides its window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BellFor {
    /// Nothing: a display beep, for the focused window (06).
    Focus,
    /// A window beep, for this window, or for none when what the bell named is gone or isn't
    /// a window's.
    Window(Option<WindowId>),
}

/// One bell, as the event has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bell {
    pub source: BellSource,
    pub window: Option<WindowId>,
}

/// The bells rung since Caminito started.
#[derive(Debug, Default)]
pub struct Bells {
    pub count: u64,
    pub last: Option<Bell>,
}

impl Caminito {
    /// Rings the bell from `source`. M8 shows and plays it here, by the settings it reads
    /// (09, 17).
    pub fn ring_bell(&mut self, source: BellSource, bell_for: BellFor) {
        let window = match bell_for {
            BellFor::Focus => self.policy.focus(SEAT),
            BellFor::Window(window) => window,
        };
        self.bells.count += 1;
        self.bells.last = Some(Bell { source, window });

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
    }

    /// The window a bell for `surface` is for: its root surface's, so a subsurface's bell is
    /// its window's, and none for a surface of no window, such as a popup's or a layer
    /// surface's.
    pub fn bell_window(&self, surface: &WlSurface) -> Option<WindowId> {
        let mut root = surface.clone();
        while let Some(parent) = get_parent(&root) {
            root = parent;
        }
        self.mapped_for_surface(&root)
    }
}
