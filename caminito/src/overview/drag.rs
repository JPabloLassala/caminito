//! A dragged window's shrink and its drop (249): pure time and geometry, as `transition`
//! is. What's drawn from it is the scene's.
//!
//! Over a cell of the bar, the dragged window eases from its layout size to its real size
//! times the cell's scale, the cell's width over its output's: since a cell has its
//! output's aspect, the window covers the same share of the cell as it would of the
//! output. It shrinks about the point it was grabbed by, which stays under the pointer.
//! Off the bar it eases back. Dropped, it eases from where it's drawn under the pointer to
//! where it's drawn next: its place in a cell's picture, or its slot in a layout. Each
//! takes 250 ms on the Workspace slide's ease-out, and a change of aim part-way goes on
//! from where it's drawn, as a reversal of the transition does.

use std::time::Duration;

use smithay::utils::{Logical, Point, Rectangle, Size};

use caminito_policy::{OutputId, WindowId};

use super::transition::between;
use crate::workspace_slide::{LENGTH, way_left};

/// The scale of `cell`, a cell of the bar of an output of `output`: its width over the
/// output's.
pub(crate) fn cell_scale(cell: Rectangle<i32, Logical>, output: Size<i32, Logical>) -> f64 {
    f64::from(cell.size.w) / f64::from(output.w.max(1))
}

/// Where `rect`, from an output's top left corner, is in `cell` of that output's bar: the
/// output scaled into the cell by its scale, on both axes. None of it is less than a
/// pixel.
pub(crate) fn scaled_into(
    rect: Rectangle<i32, Logical>,
    cell: Rectangle<i32, Logical>,
    output: Size<i32, Logical>,
) -> Rectangle<i32, Logical> {
    let factor = cell_scale(cell, output);
    let at = |n: i32| (f64::from(n) * factor).round() as i32;
    Rectangle::new(
        cell.loc + Point::from((at(rect.loc.x), at(rect.loc.y))),
        Size::from((at(rect.size.w).max(1), at(rect.size.h).max(1))),
    )
}

/// Where a window of `size` is drawn with `pointer` on the point it was grabbed by:
/// `grab`, as parts of its width and its height from its top left corner.
pub(crate) fn about_grab(
    pointer: Point<f64, Logical>,
    grab: Point<f64, Logical>,
    size: Size<f64, Logical>,
) -> Rectangle<i32, Logical> {
    let corner = pointer - Point::from((grab.x * size.w, grab.y * size.h));
    Rectangle::new(corner.to_i32_round(), size.to_i32_round())
}

/// The point `at` grabs `rect` by, as parts of its width and its height from its top left
/// corner.
pub(crate) fn grab_point(
    rect: Rectangle<i32, Logical>,
    at: Point<f64, Logical>,
) -> Point<f64, Logical> {
    let part = |from: i32, length: i32, at: f64| (at - f64::from(from)) / f64::from(length.max(1));
    Point::from((
        part(rect.loc.x, rect.size.w, at.x),
        part(rect.loc.y, rect.size.h, at.y),
    ))
}

/// What size a dragged window eases to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Aim {
    /// Its size in the layout.
    Layout,
    /// Its real size times the scale of a bar's cells.
    Scale(f64),
}

/// A dragged window's size on its way to its aim.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Shrink {
    pub(crate) window: WindowId,
    /// The point it was grabbed by, as parts of its width and its height, which stays
    /// under the pointer whatever the layout does after.
    pub(crate) grab: Point<f64, Logical>,
    pub(crate) aim: Aim,
    /// The size it was drawn at when it was given its aim.
    from: Size<f64, Logical>,
    /// When it was given its aim, on Caminito's clock.
    start: Duration,
    /// Whether it eases, or takes its aim's size at once.
    animated: bool,
}

impl Shrink {
    /// `window`, which a drag takes at `now` by `grab`, at its layout size, `layout`.
    pub(crate) fn new(
        window: WindowId,
        grab: Point<f64, Logical>,
        layout: Size<f64, Logical>,
        now: Duration,
    ) -> Self {
        Self {
            window,
            grab,
            aim: Aim::Layout,
            from: layout,
            start: now,
            animated: false,
        }
    }

    /// The same, aimed at `aim` from `now`: from the size `size` has it at now, if it's to
    /// ease. One aimed there already goes on.
    pub(crate) fn aimed(
        self,
        aim: Aim,
        size: impl Fn(Aim) -> Size<f64, Logical>,
        now: Duration,
        animated: bool,
    ) -> Self {
        if aim == self.aim {
            return self;
        }
        Self {
            aim,
            from: self.size(size(self.aim), now),
            start: now,
            animated,
            ..self
        }
    }

    /// The size it's drawn at `now`, on its way to `to`, its aim's size.
    pub(crate) fn size(&self, to: Size<f64, Logical>, now: Duration) -> Size<f64, Logical> {
        if !self.animated {
            return to;
        }
        let left = way_left(now.saturating_sub(self.start));
        Size::from((
            to.w + (self.from.w - to.w) * left,
            to.h + (self.from.h - to.h) * left,
        ))
    }

    /// When it's at its aim's size.
    pub(crate) fn end(&self) -> Duration {
        self.start + LENGTH
    }

    /// Whether it's on its way at `now`.
    pub(crate) fn runs(&self, now: Duration) -> bool {
        self.animated && now < self.end()
    }

    /// The same, at its aim's size from now on if it's there at `now`, and whether it
    /// arrived just now.
    pub(crate) fn settled(self, now: Duration) -> (Self, bool) {
        match self.animated && now >= self.end() {
            true => (
                Self {
                    animated: false,
                    ..self
                },
                true,
            ),
            false => (self, false),
        }
    }
}

/// A dropped window on its way from under the pointer to where it's drawn next.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Dropping {
    pub(crate) window: WindowId,
    /// Where it was drawn under the pointer, in global logical coordinates.
    from: Rectangle<i32, Logical>,
    /// When it was dropped, on Caminito's clock.
    start: Duration,
    /// The output and the workspace of the cell it was dropped on, whose picture it's
    /// drawn in next: none for its slot in a layout. A switch to that workspace ends it.
    pub(crate) cell: Option<(OutputId, usize)>,
}

impl Dropping {
    /// `window`, dropped at `now` where it was drawn at `from`, on its way to the picture of
    /// `cell`, or to its slot with none.
    pub(crate) fn new(
        window: WindowId,
        from: Rectangle<i32, Logical>,
        cell: Option<(OutputId, usize)>,
        now: Duration,
    ) -> Self {
        Self {
            window,
            from,
            start: now,
            cell,
        }
    }

    /// Where it's drawn at `now`, on its way to `to`.
    pub(crate) fn rect(
        &self,
        to: Rectangle<i32, Logical>,
        now: Duration,
    ) -> Rectangle<i32, Logical> {
        between(
            self.from,
            to,
            1.0 - way_left(now.saturating_sub(self.start)),
        )
    }

    /// When it's at its end.
    pub(crate) fn end(&self) -> Duration {
        self.start + LENGTH
    }

    /// Whether it's at its end at `now`.
    pub(crate) fn is_over(&self, now: Duration) -> bool {
        now >= self.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUTPUT: (i32, i32) = (1920, 1080);

    fn after(millis: u64) -> Duration {
        Duration::from_secs(10) + Duration::from_millis(millis)
    }

    /// A cell of the output's aspect, a tenth of its width.
    fn cell() -> Rectangle<i32, Logical> {
        Rectangle::new((300, 12).into(), (192, 108).into())
    }

    #[test]
    fn a_cells_scale_is_its_width_over_its_outputs() {
        assert_eq!(cell_scale(cell(), OUTPUT.into()), 0.1);
    }

    #[test]
    fn a_rectangle_scaled_into_a_cell_covers_the_same_share_of_it_with_its_aspect() {
        // 90% of the output's width and height, from its middle.
        let rect = Rectangle::new((96, 54).into(), (1728, 972).into());

        let scaled = scaled_into(rect, cell(), OUTPUT.into());

        assert_eq!(scaled, Rectangle::new((310, 17).into(), (173, 97).into()));
        let share = |part: i32, whole: i32| f64::from(part) / f64::from(whole);
        assert!((share(scaled.size.w, 192) - 0.9).abs() < 0.01);
        assert!((share(scaled.size.h, 108) - 0.9).abs() < 0.01);
        let aspect = |size: Size<i32, Logical>| f64::from(size.w) / f64::from(size.h);
        assert!((aspect(scaled.size) - aspect(rect.size)).abs() < 0.02);
    }

    #[test]
    fn a_rectangle_on_another_output_is_scaled_from_that_outputs_corner() {
        // The window's new geometry on the other output, from its corner: a cell of that
        // output's bar has that output's scale.
        let output = Size::from((1280, 1024));
        let cell = Rectangle::new((40, 12).into(), (160, 128).into());
        let rect = Rectangle::new((128, 256).into(), (640, 512).into());

        assert_eq!(
            scaled_into(rect, cell, output),
            Rectangle::new((56, 44).into(), (80, 64).into())
        );
    }

    #[test]
    fn a_tiny_rectangle_is_a_pixel_at_least() {
        let rect = Rectangle::new((0, 0).into(), (4, 4).into());

        assert_eq!(scaled_into(rect, cell(), OUTPUT.into()).size, (1, 1).into());
    }

    #[test]
    fn the_shrink_keeps_the_grabbed_point_under_the_pointer() {
        let slot = Rectangle::new((400, 300).into(), (800, 600).into());
        // A quarter of the way across and half of the way down.
        let pressed = Point::from((600.0, 600.0));
        let grab = grab_point(slot, pressed);
        assert_eq!(grab, Point::from((0.25, 0.5)));

        // At its size in the layout it's where the pointer moved it.
        let pointer = Point::from((700.0, 100.0));
        assert_eq!(
            about_grab(pointer, grab, slot.size.to_f64()),
            Rectangle::new((500, -200).into(), (800, 600).into())
        );
        // Shrunk, the same point of it is under the pointer.
        let small = about_grab(pointer, grab, Size::from((80.0, 60.0)));
        assert_eq!(small, Rectangle::new((680, 70).into(), (80, 60).into()));
        assert_eq!(grab_point(small, pointer), grab);
    }

    fn sizes(aim: Aim) -> Size<f64, Logical> {
        match aim {
            Aim::Layout => Size::from((800.0, 600.0)),
            Aim::Scale(scale) => Size::from((1000.0 * scale, 500.0 * scale)),
        }
    }

    #[test]
    fn a_dragged_window_eases_to_a_cells_scale_and_back() {
        let shrink = Shrink::new(WindowId(1), Point::default(), sizes(Aim::Layout), after(0));
        assert_eq!(
            shrink.size(sizes(Aim::Layout), after(0)),
            sizes(Aim::Layout)
        );

        let over = shrink.aimed(Aim::Scale(0.1), sizes, after(0), true);
        let to = sizes(Aim::Scale(0.1));
        assert_eq!(over.size(to, after(0)), sizes(Aim::Layout));
        // Half the time leaves an eighth of the way.
        assert_eq!(
            over.size(to, after(125)),
            Size::from((100.0 + 700.0 / 8.0, 50.0 + 550.0 / 8.0))
        );
        assert_eq!(over.size(to, after(250)), to);
        assert!(over.runs(after(249)) && !over.runs(after(250)));
    }

    #[test]
    fn a_change_of_aim_part_way_goes_on_from_where_it_is() {
        let shrink = Shrink::new(WindowId(1), Point::default(), sizes(Aim::Layout), after(0));
        let over = shrink.aimed(Aim::Scale(0.1), sizes, after(0), true);
        let there = over.size(sizes(Aim::Scale(0.1)), after(125));

        let back = over.aimed(Aim::Layout, sizes, after(125), true);

        assert_eq!(back.size(sizes(Aim::Layout), after(125)), there);
        assert_eq!(
            back.size(sizes(Aim::Layout), after(375)),
            sizes(Aim::Layout)
        );
        // The same aim again goes on as it was.
        assert_eq!(back.aimed(Aim::Layout, sizes, after(200), true), back);
    }

    #[test]
    fn without_the_animation_it_takes_each_size_at_once() {
        let shrink = Shrink::new(WindowId(1), Point::default(), sizes(Aim::Layout), after(0));

        let over = shrink.aimed(Aim::Scale(0.1), sizes, after(0), false);

        assert_eq!(
            over.size(sizes(Aim::Scale(0.1)), after(0)),
            sizes(Aim::Scale(0.1))
        );
        assert!(!over.runs(after(0)));
    }

    #[test]
    fn a_drop_eases_from_under_the_pointer_to_where_it_goes() {
        let from = Rectangle::new((600, 40).into(), (100, 80).into());
        let to = Rectangle::new((300, 20).into(), (20, 16).into());
        let drop = Dropping::new(WindowId(1), from, None, after(0));

        assert_eq!(drop.rect(to, after(0)), from);
        assert_eq!(
            drop.rect(to, after(125)),
            Rectangle::new((338, 23).into(), (30, 24).into())
        );
        assert_eq!(drop.rect(to, after(250)), to);
        assert!(!drop.is_over(after(249)) && drop.is_over(after(250)));
    }
}
