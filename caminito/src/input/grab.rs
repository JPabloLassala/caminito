//! The pointer grabs Caminito holds: an interactive move or resize, where policy does the
//! geometry and the grab feeds it the pointer until the buttons are released, and a press
//! on a frame, held until it's released (31).

use caminito_policy::{ResizeEdges, WindowId};
use caminito_theme::ButtonFunction;
use smithay::backend::input::ButtonState;
use smithay::input::pointer::{
    AxisFrame, ButtonEvent, Focus, GestureHoldBeginEvent, GestureHoldEndEvent,
    GesturePinchBeginEvent, GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent,
    GestureSwipeEndEvent, GestureSwipeUpdateEvent, GrabStartData, MotionEvent, PointerGrab,
    PointerInnerHandle, RelativeMotionEvent,
};
use smithay::utils::{Logical, Point, Serial};

use super::PointerFocus;
use crate::state::{Caminito, SEAT, State};

/// What an interactive grab does to its window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrabKind {
    Move,
    /// Resizes by moving these edges.
    Resize(ResizeEdges),
}

/// What a press on a frame holds the pointer for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameHold {
    /// A button being clicked, which acts if the grab's button is released over it.
    Button {
        window: WindowId,
        function: ButtonFunction,
    },
    /// The titlebar, pressed: dragging it past `drag-threshold` moves the window, if it
    /// moves.
    Title { window: WindowId, can_move: bool },
    /// A press that has done all it does, held until the buttons are released.
    Held,
}

/// Why Caminito holds the pointer.
#[derive(Debug, Clone, Copy)]
enum Purpose {
    /// Policy is moving or resizing a window.
    Window,
    Frame(FrameHold),
}

/// A pointer grab of Caminito's. No client has the pointer while it lasts.
pub struct Grab {
    start_data: GrabStartData<State>,
    purpose: Purpose,
}

/// Where a grab starts: the pointer at `location`, with `button` pressed, and no client
/// focus.
pub(super) fn start_data(button: u32, location: Point<f64, Logical>) -> GrabStartData<State> {
    GrabStartData {
        focus: None,
        button,
        location,
    }
}

impl Caminito {
    /// Starts policy's move or resize of a window from the pointer at `location`. Returns
    /// false if policy won't move or resize it.
    fn start_window_grab(
        &mut self,
        id: WindowId,
        kind: GrabKind,
        location: Point<f64, Logical>,
    ) -> bool {
        let policy = &mut self.policy;
        let started = match kind {
            GrabKind::Move => policy.begin_move(SEAT, id, location),
            GrabKind::Resize(edges) => policy.begin_resize(SEAT, id, edges, location),
        };
        if started {
            self.window_grab = Some(kind);
        }
        started
    }

    /// Starts a move grab from a titlebar drag that's crossed the drag threshold, with the
    /// pointer as it was `at_press` and now at `pointer`. Unmaximises the window first if
    /// it's maximised either way, keeping `pointer`'s position across the titlebar the same
    /// fraction it was across the maximised one (144).
    fn start_titlebar_move_grab(
        &mut self,
        id: WindowId,
        at_press: Point<f64, Logical>,
        pointer: Point<f64, Logical>,
    ) -> bool {
        let started = self.policy.begin_titlebar_move(SEAT, id, at_press, pointer);
        if started {
            self.window_grab = Some(GrabKind::Move);
        }
        started
    }
}

impl State {
    /// Starts moving or resizing a window from the pointer as `start_data` found it. Does
    /// nothing if policy won't move or resize the window, or while the session is locked,
    /// when no client has the pointer (12).
    pub fn begin_window_grab(
        &mut self,
        id: WindowId,
        kind: GrabKind,
        start_data: GrabStartData<State>,
        serial: Serial,
    ) {
        if self.caminito.lock.is_locked() {
            return;
        }
        if self
            .caminito
            .start_window_grab(id, kind, start_data.location)
        {
            let grab = Grab {
                start_data,
                purpose: Purpose::Window,
            };
            let pointer = self.caminito.seat.get_pointer().unwrap();
            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }

    /// Holds the pointer for a press on a frame, from where `start_data` found it.
    pub fn begin_frame_grab(
        &mut self,
        hold: FrameHold,
        start_data: GrabStartData<State>,
        serial: Serial,
    ) {
        if let FrameHold::Button { window, function } = hold {
            self.caminito.frame_pointer.pressed = Some((window, function));
        }
        let grab = Grab {
            start_data,
            purpose: Purpose::Frame(hold),
        };
        let pointer = self.caminito.seat.get_pointer().unwrap();
        pointer.set_grab(self, grab, serial, Focus::Clear);
    }
}

impl PointerGrab<State> for Grab {
    fn motion(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        _focus: Option<(PointerFocus, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        handle.motion(data, None, event);
        let caminito = &mut data.caminito;
        if let Purpose::Frame(FrameHold::Title { window, can_move }) = self.purpose {
            let start = self.start_data.location;
            let threshold = f64::from(caminito.settings.mouse.drag_threshold);
            let distance = event.location - start;
            if distance.x.abs() > threshold || distance.y.abs() > threshold {
                self.purpose = if can_move
                    && caminito.start_titlebar_move_grab(window, start, event.location)
                {
                    Purpose::Window
                } else {
                    Purpose::Frame(FrameHold::Held)
                };
            }
        }
        if let Purpose::Window = self.purpose {
            caminito.policy.grab_motion(SEAT, event.location);
            caminito.queue_redraw_all();
            // Policy ends the grab itself if the window goes away or is maximised.
            if caminito.policy.grab(SEAT).is_none() {
                handle.unset_grab(self, data, event.serial, event.time, false);
            }
        }
    }

    fn relative_motion(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        _focus: Option<(PointerFocus, Point<f64, Logical>)>,
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
        // Focus isn't restored here: the window may have moved since the pointer last
        // found its surface, and the refresh pass finds it again where it is now.
        if let Purpose::Frame(FrameHold::Button { window, function }) = self.purpose {
            // A click ends with its own button, over the button it began on or not.
            if event.button == self.start_data.button && event.state == ButtonState::Released {
                handle.unset_grab(self, data, event.serial, event.time, false);
                data.frame_button_released(window, function, event.button);
            }
        } else if handle.current_pressed().is_empty() {
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
        let caminito = &mut data.caminito;
        match self.purpose {
            Purpose::Window => {
                caminito.policy.end_grab(SEAT);
                caminito.window_grab = None;
            }
            // A titlebar press that never became a drag is a click on the titlebar (153).
            Purpose::Frame(FrameHold::Title { window, .. }) => {
                caminito.frame_pointer.pressed = None;
                caminito.policy.titlebar_released(window);
            }
            Purpose::Frame(_) => caminito.frame_pointer.pressed = None,
        }
        caminito.queue_redraw_all();
    }
}
