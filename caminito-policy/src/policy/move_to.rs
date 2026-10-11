//! The bindings that move the focused window without a grab (160): to a corner, a side or
//! the centre of its output's work area, and to the output beside its own, each as Marco's
//! handler in `core/keybindings.c` (1.29.0) does it.

use smithay::utils::{FrameExtents, Logical, Point, Rectangle, Size};

use crate::grid::Direction;
use crate::{Anchor, Output, PolicyWindow, WindowId, placement};

use super::{Policy, frame_around};

/// Where along one axis of a work area a Frame goes.
#[derive(Clone, Copy)]
enum Along {
    Start,
    Middle,
    End,
}

impl Anchor {
    /// Where the Frame goes along x and along y, with none for an axis a side's binding
    /// leaves alone (`handle_move_to_corner_backend`'s `xchange` and `ychange`).
    fn along(self) -> (Option<Along>, Option<Along>) {
        use Along::{End, Middle, Start};
        match self {
            Anchor::NorthWest => (Some(Start), Some(Start)),
            Anchor::North => (None, Some(Start)),
            Anchor::NorthEast => (Some(End), Some(Start)),
            Anchor::West => (Some(Start), None),
            Anchor::Centre => (Some(Middle), Some(Middle)),
            Anchor::East => (Some(End), None),
            Anchor::SouthWest => (Some(Start), Some(End)),
            Anchor::South => (None, Some(End)),
            Anchor::SouthEast => (Some(End), Some(End)),
        }
    }
}

impl<W: PolicyWindow> Policy<W> {
    /// `move-to-corner-*`, `move-to-side-*` and `move-to-center`
    /// (`handle_move_to_corner_backend` and `handle_move_to_center`,
    /// keybindings.c:2567-2736): the Frame, not the client's area, goes against the corner
    /// or side, or to the middle (`center_rect_in_area`, place.c:531-537), and the window
    /// keeps its size. The work area is that of the output the window is most on. Marco
    /// measures a corner or a side against the work area of the whole screen, so with two
    /// monitors a window can leave its own; the ticket asks for the window's output.
    ///
    /// Marco's constraints then put a maximised, tiled or fullscreen window back
    /// (`constrain_maximization`, `constrain_tiling` and `constrain_fullscreen`,
    /// constraints.c:816-1095), so those don't move, and one maximised one way moves the
    /// other way alone. They keep the titlebar in the work area too
    /// (`constrain_titlebar_visible`, constraints.c:1482-1564), so a window taller than it
    /// keeps its top there.
    pub(super) fn move_to(&mut self, id: WindowId, anchor: Anchor) {
        if !self.moves_by_binding(id) {
            return;
        }
        let managed = &self.windows[&id];
        if managed.maximized() || managed.tile.is_some() || managed.fullscreen {
            return;
        }
        let frame = managed.frame();
        let Some(output) = self.output_of(frame).or(self.primary_output()) else {
            return;
        };
        let area = Self::work_area(output);
        let place = |along, loc: i32, len: i32, start: i32, area_len: i32| match along {
            None => loc,
            Some(Along::Start) => start,
            Some(Along::Middle) => start + (area_len - len) / 2,
            Some(Along::End) => start + area_len - len,
        };
        let (x, y) = anchor.along();
        let to = Point::new(
            place(x, frame.loc.x, frame.size.w, area.loc.x, area.size.w),
            place(y, frame.loc.y, frame.size.h, area.loc.y, area.size.h),
        );
        // Only a binding that moves the window up or down brings its top into the work area.
        let to = if y.is_some() {
            Point::new(to.x, to.y.max(area.loc.y))
        } else {
            to
        };
        let managed = self.windows.get_mut(&id).unwrap();
        let before = managed.geometry;
        managed.geometry.loc += to - frame.loc;
        managed.pin_maximized_axes(before);
    }

    /// `move-to-monitor-*` (`handle_move_to_monitor`, keybindings.c:2738-2759, and
    /// `meta_window_move_to_monitor`, window.c:2891-2926): the window goes from the output
    /// it's most on to the one beside it that way, and with none there it stays. It keeps
    /// its share of the output: Marco scales its place in the monitor and its size by the
    /// two monitors' sizes, so between outputs of one size it only moves. The size is then
    /// kept within the hints, and the Frame moved inside the work area, or to its top-left
    /// along an axis it doesn't fit. Marco lets all of a moved window but a strip leave the
    /// work area (`constrain_partially_onscreen`, constraints.c:1566-1632); the ticket asks for it to be clamped.
    ///
    /// A maximised or fullscreen window is maximised or fullscreen on the new output, as
    /// Marco's constraints make it from the monitor the asked-for rectangle is on
    /// (`setup_constraint_info`, constraints.c:417-427), and a tiled one keeps its tile
    /// there. The geometry such a window goes back to is moved the same way. Marco leaves a
    /// tile resized by its inner sides on the monitor it was on, with its tile's monitor
    /// changed; here it's tiled afresh on the new one.
    pub(super) fn move_to_monitor(&mut self, id: WindowId, direction: Direction) {
        if !self.moves_by_binding(id) {
            return;
        }
        let managed = &self.windows[&id];
        let Some(from) = self
            .output_of(managed.frame())
            .or(self.primary_output())
            .copied()
        else {
            return;
        };
        let Some(to) = self.output_beside(&from, direction).copied() else {
            return;
        };
        let area = Self::work_area(&to);
        let hints = managed.hints;
        // `geometry` as it is on `to`, with a frame of extents `e` whose height is `shaded`
        // while the window is.
        let moved =
            |geometry: Rectangle<i32, Logical>, e: FrameExtents<i32, Logical>, shaded: bool| {
                let scaled = transform_to_output(geometry, from.geometry, to.geometry);
                let size = hints.clamp(scaled.size);
                let mut frame = frame_around(Rectangle::new(scaled.loc, size), e);
                if shaded {
                    frame.size.h = e.top;
                }
                let frame_loc = placement::constrain(frame, area);
                Rectangle::new((frame_loc.x + e.left, frame_loc.y + e.top).into(), size)
            };
        if !managed.constrained() {
            let geometry = moved(managed.geometry, managed.visible_extents(), managed.shaded);
            self.windows.get_mut(&id).unwrap().geometry = geometry;
            return;
        }
        let floating = managed
            .floating
            .map(|floating| moved(floating, managed.floating_extents(), false));
        let managed = self.windows.get_mut(&id).unwrap();
        managed.floating = floating;
        if let Some(tiled) = &mut managed.tile {
            tiled.output = to.id;
            tiled.resized = false;
        }
        if managed.fullscreen_output.is_some() {
            managed.fullscreen_output = Some(to.id);
        }
        self.relayout_on(id, Some(to.id));
    }

    /// The output that shares the side of `output` that way, the first policy was told of,
    /// as `meta_screen_get_xinerama_neighbor` picks it (screen.c:1973-2003): its facing
    /// side is on the same line, and the two overlap along it.
    fn output_beside(&self, output: &Output, direction: Direction) -> Option<&Output> {
        let from = output.geometry;
        let (right, bottom) = (from.loc.x + from.size.w, from.loc.y + from.size.h);
        self.outputs.iter().find(|other| {
            let g = other.geometry;
            let (g_right, g_bottom) = (g.loc.x + g.size.w, g.loc.y + g.size.h);
            let beside_in_y = g.loc.y < bottom && from.loc.y < g_bottom;
            let beside_in_x = g.loc.x < right && from.loc.x < g_right;
            match direction {
                Direction::Right => g.loc.x == right && beside_in_y,
                Direction::Left => g_right == from.loc.x && beside_in_y,
                Direction::Up => g_bottom == from.loc.y && beside_in_x,
                Direction::Down => g.loc.y == bottom && beside_in_x,
            }
        })
    }

    /// Whether a binding of this module moves `id`: not the desktop or a dock, which Marco
    /// gives no move function (`recalc_window_features`, window.c:6805-6814), and not a
    /// window a grab is moving or resizing.
    fn moves_by_binding(&self, id: WindowId) -> bool {
        !self.is_desktop_or_dock(id) && !self.grabs.values().any(|grab| grab.window == id)
    }
}

/// `rect` on output `from`, as it is on output `to`: its place in the output and its size
/// scaled by the outputs' sizes, each rounded towards zero, as
/// `meta_window_transform_to_monitor` has it (window.c:2928-2948).
fn transform_to_output(
    rect: Rectangle<i32, Logical>,
    from: Rectangle<i32, Logical>,
    to: Rectangle<i32, Logical>,
) -> Rectangle<i32, Logical> {
    let horizontal = f64::from(to.size.w) / f64::from(from.size.w);
    let vertical = f64::from(to.size.h) / f64::from(from.size.h);
    let scale = |v: i32, ratio: f64| (ratio * f64::from(v)) as i32;
    Rectangle::new(
        Point::new(
            to.loc.x + scale(rect.loc.x - from.loc.x, horizontal),
            to.loc.y + scale(rect.loc.y - from.loc.y, vertical),
        ),
        Size::new(scale(rect.size.w, horizontal), scale(rect.size.h, vertical)),
    )
}
