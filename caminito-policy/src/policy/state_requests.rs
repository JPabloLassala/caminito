//! The states a window asks for: policy's to decide for a managed window, and kept for a
//! Pending one, which maps with them (189).

use super::*;

/// A state a window asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateRequest {
    Maximized(bool),
    /// Fullscreen, on this output if the client named one.
    Fullscreen(Option<OutputId>),
    Unfullscreen,
    Minimized,
    /// Kept above others, or not (155).
    Above(bool),
    Below(bool),
    Sticky(bool),
    /// The active window, as `_NET_ACTIVE_WINDOW` asks: if policy honours the one who asks,
    /// it switches to its workspace, unminimises it, raises it and focuses it (166).
    Active(ActivationSource),
    /// `_NET_WM_STATE_DEMANDS_ATTENTION`, set or unset (166).
    DemandsAttention(bool),
}

/// The states a window asks to map with, kept while it's Pending: those that change how
/// it's placed (154), those it has once it's placed, and its claim to the focus (165).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MapStates {
    /// Maximised both ways.
    pub maximized: bool,
    /// Fullscreen, on the output named if one is.
    pub fullscreen: Option<Option<OutputId>>,
    pub above: bool,
    pub below: bool,
    pub sticky: bool,
    /// It asked to be the active window, which policy judges once it has mapped (166).
    pub active: Option<ActivationSource>,
    /// It asked for attention with `_NET_WM_STATE_DEMANDS_ATTENTION` (166).
    pub demands_attention: bool,
    /// What it claims the focus with (165).
    pub focus: FocusClaim,
}

impl MapStates {
    /// Keeps a Pending window's request, to map with. Returns whether the window is to be
    /// configured again: only maximised and fullscreen change how it's configured before it
    /// maps, and a window that isn't managed has nothing to minimise.
    pub fn request(&mut self, request: StateRequest) -> bool {
        match request {
            StateRequest::Maximized(maximized) => self.maximized = maximized,
            StateRequest::Fullscreen(output) => self.fullscreen = Some(output),
            StateRequest::Unfullscreen => self.fullscreen = None,
            StateRequest::Minimized => return false,
            StateRequest::Above(above) => {
                self.above = above;
                return false;
            }
            StateRequest::Below(below) => {
                self.below = below;
                return false;
            }
            StateRequest::Sticky(sticky) => {
                self.sticky = sticky;
                return false;
            }
            StateRequest::Active(source) => {
                self.active = Some(source);
                return false;
            }
            StateRequest::DemandsAttention(demands_attention) => {
                self.demands_attention = demands_attention;
                return false;
            }
        }
        true
    }
}

impl<W: PolicyWindow> Policy<W> {
    /// A managed window asks for a state, on `seat` at `now`, which an activation is judged
    /// by.
    pub fn request_state(
        &mut self,
        seat: SeatId,
        id: WindowId,
        request: StateRequest,
        now: Duration,
    ) {
        match request {
            StateRequest::Maximized(maximized) => self.set_maximized(id, maximized),
            StateRequest::Fullscreen(output) => self.set_fullscreen(id, true, output),
            StateRequest::Unfullscreen => self.set_fullscreen(id, false, None),
            StateRequest::Minimized => self.minimize(id),
            StateRequest::Above(above) => self.set_above(id, above),
            StateRequest::Below(below) => self.set_below(id, below),
            StateRequest::Sticky(sticky) => self.set_sticky(id, sticky),
            StateRequest::Active(source) => self.request_activation_from(seat, id, source, now),
            StateRequest::DemandsAttention(demands_attention) => {
                self.set_demands_attention(id, demands_attention)
            }
        }
    }
}
