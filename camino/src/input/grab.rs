//! The pointer grab for an interactive move or resize. Policy does the geometry; the grab
//! feeds it the pointer and ends it when the buttons are released.

use camino_policy::{ResizeEdges, WindowId};
use smithay::input::pointer::{
    AxisFrame, ButtonEvent, Focus, GestureHoldBeginEvent, GestureHoldEndEvent,
    GesturePinchBeginEvent, GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent,
    GestureSwipeEndEvent, GestureSwipeUpdateEvent, GrabStartData, MotionEvent, PointerGrab,
    PointerInnerHandle, RelativeMotionEvent,
};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Serial};

use crate::state::{SEAT, State};

/// What an interactive grab does to its window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrabKind {
    Move,
    /// Resizes by moving these edges.
    Resize(ResizeEdges),
}

pub struct WindowGrab {
    start_data: GrabStartData<State>,
}

impl State {
    /// Starts moving or resizing a window from the pointer as `start_data` found it. Does
    /// nothing if policy won't move or resize the window.
    pub fn begin_window_grab(
        &mut self,
        id: WindowId,
        kind: GrabKind,
        start_data: GrabStartData<State>,
        serial: Serial,
    ) {
        let policy = &mut self.camino.policy;
        let started = match kind {
            GrabKind::Move => policy.begin_move(SEAT, id, start_data.location),
            GrabKind::Resize(edges) => policy.begin_resize(SEAT, id, edges, start_data.location),
        };
        if started {
            let pointer = self.camino.seat.get_pointer().unwrap();
            pointer.set_grab(self, WindowGrab { start_data }, serial, Focus::Clear);
        }
    }
}

impl PointerGrab<State> for WindowGrab {
    fn motion(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        _focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        // No client has the pointer during the grab.
        handle.motion(data, None, event);
        data.camino.policy.grab_motion(SEAT, event.location);
        data.camino.queue_redraw_all();
        // Policy ends the grab itself if the window goes away or is maximised.
        if data.camino.policy.grab(SEAT).is_none() {
            handle.unset_grab(self, data, event.serial, event.time, false);
        }
    }

    fn relative_motion(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        _focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &RelativeMotionEvent,
    ) {
        handle.relative_motion(data, None, event);
    }

    fn button(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &ButtonEvent,
    ) {
        handle.button(data, event);
        // Focus isn't restored here: the window has moved since the pointer last found its
        // surface, and the refresh pass finds it again where it is now.
        if handle.current_pressed().is_empty() {
            handle.unset_grab(self, data, event.serial, event.time, false);
        }
    }

    fn axis(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        details: AxisFrame,
    ) {
        handle.axis(data, details);
    }

    fn frame(&mut self, data: &mut State, handle: &mut PointerInnerHandle<'_, State>) {
        handle.frame(data);
    }

    fn gesture_swipe_begin(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GestureSwipeBeginEvent,
    ) {
        handle.gesture_swipe_begin(data, event);
    }

    fn gesture_swipe_update(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GestureSwipeUpdateEvent,
    ) {
        handle.gesture_swipe_update(data, event);
    }

    fn gesture_swipe_end(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GestureSwipeEndEvent,
    ) {
        handle.gesture_swipe_end(data, event);
    }

    fn gesture_pinch_begin(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GesturePinchBeginEvent,
    ) {
        handle.gesture_pinch_begin(data, event);
    }

    fn gesture_pinch_update(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GesturePinchUpdateEvent,
    ) {
        handle.gesture_pinch_update(data, event);
    }

    fn gesture_pinch_end(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GesturePinchEndEvent,
    ) {
        handle.gesture_pinch_end(data, event);
    }

    fn gesture_hold_begin(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GestureHoldBeginEvent,
    ) {
        handle.gesture_hold_begin(data, event);
    }

    fn gesture_hold_end(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GestureHoldEndEvent,
    ) {
        handle.gesture_hold_end(data, event);
    }

    fn start_data(&self) -> &GrabStartData<State> {
        &self.start_data
    }

    fn unset(&mut self, data: &mut State) {
        data.camino.policy.end_grab(SEAT);
        data.camino.queue_redraw_all();
    }
}
