//! The keyboard move and resize (156) that `begin-move`, `begin-resize` and X11's
//! `_NET_WM_MOVERESIZE` directions 9 and 10 start, as Marco's `process_keyboard_move_grab`
//! and `process_keyboard_resize_grab` (keybindings.c:1480-1627, 1631-2015) run it. The keys
//! step the window, the pointer moves or resizes it too, and policy says where the
//! compositor warps the pointer to, as Marco's `warp_grab_pointer` (window.c:8506-8615).
//! A manual placement (154) is a keyboard move too.

use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::edges::By;
use crate::{PolicyWindow, ResizeEdges, SeatId, WindowId};

use super::Policy;

/// A key pressed during a keyboard move or resize. Marco lets modifiers by, so the
/// compositor sends none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrabKey {
    /// An arrow, the keypad's or vi's `hjkl`, or a keypad diagonal: the way it points, `x`
    /// and `y` each -1, 0 or 1.
    Arrow { x: i32, y: i32, step: GrabStep },
    /// Escape, which puts the window back where it was.
    Escape,
    /// Any other key, Return among them, which puts the window down where it is.
    Other,
}

/// How far an arrow steps, by the modifiers held with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrabStep {
    /// 10 pixels.
    Normal,
    /// 1 pixel, with Ctrl.
    Fine,
    /// With Shift, to the next edge the window meets its way.
    Snap,
}

impl GrabStep {
    /// Marco's `NORMAL_INCREMENT` and `SMALL_INCREMENT`. A snap proposes the small one and
    /// goes on from there to the next edge.
    fn pixels(self) -> i32 {
        match self {
            GrabStep::Normal => 10,
            GrabStep::Fine | GrabStep::Snap => 1,
        }
    }
}

impl<W: PolicyWindow> Policy<W> {
    /// Starts moving a window with `seat`'s keys. Returns false if the window can't be
    /// moved, or the seat has an Interaction.
    pub fn begin_keyboard_move(&mut self, seat: SeatId, id: WindowId) -> bool {
        self.begin_keyboard_grab(seat, id, false)
    }

    /// Starts resizing a window with `seat`'s keys, from no edge until the first arrow picks
    /// one. Returns false if the window can't be resized, or the seat has an Interaction.
    pub fn begin_keyboard_resize(&mut self, seat: SeatId, id: WindowId) -> bool {
        self.begin_keyboard_grab(seat, id, true)
    }

    /// Like Marco's `meta_window_begin_grab_op`, a keyboard grab doesn't start while the
    /// seat has an Interaction, and a resize needs a window Marco gives `has_resize_func`.
    pub(super) fn begin_keyboard_grab(&mut self, seat: SeatId, id: WindowId, resize: bool) -> bool {
        if self.interaction(seat).is_some()
            || (resize
                && !self
                    .windows
                    .get(&id)
                    .is_some_and(|managed| managed.functions().resize()))
        {
            return false;
        }
        let edges = resize.then(ResizeEdges::default);
        if !self.begin_grab(seat, id, edges, Point::default(), false) {
            return false;
        }
        let grab = self.grabs.get_mut(&seat).unwrap();
        grab.keyboard = true;
        // Not a click, which with `raise-on-click` off would raise the window (153).
        grab.moved = true;
        self.warp_pointer(seat);
        true
    }

    /// A key pressed during the seat's keyboard move or resize.
    pub fn grab_key(&mut self, seat: SeatId, key: GrabKey) {
        let Some(grab) = self.grabs.get(&seat).filter(|grab| grab.keyboard) else {
            return;
        };
        let resize = grab.resize;
        match key {
            GrabKey::Escape => self.cancel_grab(seat),
            GrabKey::Other => self.end_grab(seat),
            GrabKey::Arrow { x, y, step } => match resize {
                None => self.step_move(seat, Point::new(x, y), step),
                // Marco's resize takes no keypad diagonal, so it puts the window down.
                Some(_) if x != 0 && y != 0 => self.end_grab(seat),
                Some(edges) => self.step_resize(seat, edges, Point::new(x, y), step),
            },
        }
        self.apply();
    }

    /// Where the compositor is to warp the seat's pointer to, once.
    pub fn take_pointer_warp(&mut self, seat: SeatId) -> Option<Point<f64, Logical>> {
        let warp = self.grabs.get_mut(&seat)?.warp.take()?;
        Some(warp.to_f64())
    }

    /// The window the seat is moving or resizing with its keys, or placing (154).
    pub fn keyboard_grab(&self, seat: SeatId) -> Option<WindowId> {
        self.grabs
            .get(&seat)
            .filter(|grab| grab.keyboard)
            .map(|grab| grab.window)
    }

    /// The edges the seat's grab is resizing, or `None` for a move. A keyboard resize's are
    /// none until the first arrow.
    pub fn grab_edges(&self, seat: SeatId) -> Option<ResizeEdges> {
        self.grabs.get(&seat)?.resize
    }

    /// Steps a keyboard move `direction`'s way, stopping at the first edge it would cross,
    /// or snapping to the next one.
    fn step_move(&mut self, seat: SeatId, direction: Point<i32, Logical>, step: GrabStep) {
        let grab = &self.grabs[&seat];
        let (window, placing, pointer) = (grab.window, grab.placing, grab.start_pointer);
        // A tiled window is untiled round the pointer first, as a pointer move does once it
        // shakes it loose (158).
        if self.tile(window).is_some() {
            self.unmaximize_for_drag(window, pointer, pointer);
            let loc = self.windows[&window].geometry.loc;
            let grab = self.grabs.get_mut(&seat).unwrap();
            grab.start.loc = loc;
            grab.initial.loc = loc;
        }
        let edges = self.edges_for(window);
        let managed = self.windows.get_mut(&window).unwrap();
        let before = managed.geometry.loc;
        let n = step.pixels();
        let to = before + Point::new(direction.x * n, direction.y * n);
        managed.move_within(to, &edges, step == GrabStep::Snap, By::Keys);
        let moved = managed.geometry.loc - before;
        if placing {
            // The pointer goes on moving the window from where the key left it.
            self.grabs.get_mut(&seat).unwrap().start.loc += moved;
        } else {
            self.warp_pointer(seat);
        }
    }

    /// Steps a keyboard resize. An arrow across the edges being resized picks the edge it
    /// points at instead, as Marco's `process_keyboard_resize_grab_op_change` does: the
    /// first arrow picks the first edge, and once the pointer has picked a corner, every
    /// arrow moves one of its edges.
    fn step_resize(
        &mut self,
        seat: SeatId,
        moving: ResizeEdges,
        direction: Point<i32, Logical>,
        step: GrabStep,
    ) {
        let across = if direction.x != 0 {
            !(moving.left || moving.right)
        } else {
            !(moving.top || moving.bottom)
        };
        if across {
            let picked = ResizeEdges {
                left: direction.x < 0,
                right: direction.x > 0,
                top: direction.y < 0,
                bottom: direction.y > 0,
            };
            self.set_resize_edges(seat, picked);
            return;
        }
        let window = self.grabs[&seat].window;
        let edges = self.edges_for(window);
        let managed = self.windows.get_mut(&window).unwrap();
        let start = managed.geometry;
        let n = step.pixels();
        let grows = |far: bool, way: i32| if far { way * n } else { -way * n };
        let size = Size::new(
            start.size.w + grows(moving.right, direction.x),
            start.size.h + grows(moving.bottom, direction.y),
        );
        managed.resize_within(
            start,
            moving,
            size,
            &edges,
            step == GrabStep::Snap,
            By::Keys,
        );
        self.warp_pointer(seat);
    }

    /// A keyboard resize with no edge yet takes the ones the pointer went towards from
    /// where it was warped, as Marco's `update_resize` does (window.c:7672-7720). Unlike
    /// Marco's, that first motion doesn't resize the window too, since the pointer is
    /// warped to the edge it picked and Marco loses that change at the next motion anyway.
    pub(super) fn pick_resize_edges(&mut self, seat: SeatId, delta: Point<i32, Logical>) {
        if delta == Point::default() {
            return;
        }
        let picked = ResizeEdges {
            left: delta.x < 0,
            right: delta.x > 0,
            top: delta.y < 0,
            bottom: delta.y > 0,
        };
        self.set_resize_edges(seat, picked);
    }

    fn set_resize_edges(&mut self, seat: SeatId, edges: ResizeEdges) {
        self.grabs.get_mut(&seat).unwrap().resize = Some(edges);
        self.warp_pointer(seat);
    }

    /// Warps the pointer to where Marco's `warp_grab_pointer` puts it for the grab: the
    /// middle of the window's frame, or the middle of the edge or the corner being resized,
    /// within the screen. The grab goes on from there, the pointer moving the window from
    /// its geometry now.
    fn warp_pointer(&mut self, seat: SeatId) {
        let screen = self.screen();
        let grab = self.grabs.get_mut(&seat).unwrap();
        let managed = self.windows.get_mut(&grab.window).unwrap();
        let frame = managed.frame_of(managed.geometry);
        let edges = grab.resize.unwrap_or_default();
        let along = |low: bool, high: bool, length: i32| {
            if low {
                0
            } else if high {
                length - 1
            } else {
                length / 2
            }
        };
        let x = frame.loc.x + along(edges.left, edges.right, frame.size.w);
        let y = frame.loc.y + along(edges.top, edges.bottom, frame.size.h);
        let warp = clamp_to(screen, Point::new(x, y));
        grab.warp = Some(warp);
        grab.start_pointer = warp.to_f64();
        grab.start = managed.geometry;
        if let Some(edges) = grab.resize {
            managed.resize = Some((edges, managed.geometry));
        }
    }
}

/// `point`, moved onto `screen` if it's off it, as Marco keeps a warp off the screen's
/// edge (window.c:8584-8586).
fn clamp_to(screen: Rectangle<i32, Logical>, point: Point<i32, Logical>) -> Point<i32, Logical> {
    let right = screen.loc.x + (screen.size.w - 1).max(0);
    let bottom = screen.loc.y + (screen.size.h - 1).max(0);
    Point::new(
        point.x.clamp(screen.loc.x, right),
        point.y.clamp(screen.loc.y, bottom),
    )
}
