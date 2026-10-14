//! Where the Overview's bar of workspaces puts each workspace's cell (244). Pure geometry,
//! of the output's size and the workspace count alone, as `layout` is.
//!
//! The bar is one row along the top of the output, in workspace order, whatever the
//! Workspace grid's shape. Its height is a part of the output's, and each cell has the
//! output's aspect. Where the cells at that height are wider than the output together,
//! they're all made smaller by the same amount, to fit. The windows' layout takes the area
//! beneath the bar.

use smithay::utils::{Logical, Point, Rectangle, Size};

/// The bar's height, as a part of the output's (the developer's start value of
/// 2026-10-13, which 247 checks in a Session).
const HEIGHT: f64 = 0.15;
/// The room between the bar's edges and its cells, and between two cells.
const PADDING: i32 = 12;

/// The bar on one output.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Bar {
    /// Each workspace's cell, from the first, from the output's top left corner.
    pub cells: Vec<Rectangle<i32, Logical>>,
    /// The bar's height: the windows' layout is beneath it.
    pub height: i32,
}

impl Bar {
    /// The bar of `count` workspaces on an output of `output`.
    pub(crate) fn new(output: Size<i32, Logical>, count: usize) -> Self {
        let height = (f64::from(output.h) * HEIGHT).round() as i32;
        let count_i32 = i32::try_from(count).expect("a workspace count fits an i32");
        if count == 0 || output.w <= 0 || output.h <= 0 {
            return Self {
                cells: Vec::new(),
                height,
            };
        }
        let tall = f64::from((height - 2 * PADDING).max(0));
        let aspect = f64::from(output.w) / f64::from(output.h);
        let room = f64::from((output.w - 2 * PADDING - (count_i32 - 1) * PADDING).max(0));
        // One factor for all, so that they fit across.
        let factor = (room / (tall * aspect * f64::from(count_i32))).min(1.0);
        // The height from the width as it's rounded, to keep the aspect.
        let width = (tall * aspect * factor).floor();
        let cell = Size::<i32, Logical>::from((width as i32, (width / aspect).round() as i32));
        let width = count_i32 * cell.w + (count_i32 - 1) * PADDING;
        let left = (output.w - width) / 2;
        let top = (height - cell.h) / 2;
        let cells = (0..count_i32)
            .map(|n| Rectangle::new(Point::from((left + n * (cell.w + PADDING), top)), cell))
            .collect();
        Self { cells, height }
    }

    /// The area the windows' layout takes on an output of `output`: all of it beneath the
    /// bar.
    pub(crate) fn area_beneath(&self, output: Size<i32, Logical>) -> Rectangle<i32, Logical> {
        Rectangle::new(
            Point::from((0, self.height)),
            Size::from((output.w, (output.h - self.height).max(0))),
        )
    }

    /// The workspace whose cell `point` is in.
    pub(crate) fn cell_at(&self, point: Point<f64, Logical>) -> Option<usize> {
        self.cells
            .iter()
            .position(|cell| cell.to_f64().contains(point))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUTPUT: (i32, i32) = (1920, 1080);

    fn bar(count: usize) -> Bar {
        Bar::new(OUTPUT.into(), count)
    }

    /// Whether `cell` has the output's aspect, to a pixel.
    fn has_the_outputs_aspect(cell: Rectangle<i32, Logical>) -> bool {
        let width = f64::from(cell.size.h) * f64::from(OUTPUT.0) / f64::from(OUTPUT.1);
        (f64::from(cell.size.w) - width).abs() <= 1.0
    }

    #[test]
    fn the_bar_is_a_part_of_the_outputs_height_and_the_layout_is_beneath_it() {
        let bar = bar(1);

        assert_eq!(bar.height, 162);
        assert_eq!(
            bar.area_beneath(OUTPUT.into()),
            Rectangle::new((0, 162).into(), (1920, 918).into())
        );
    }

    #[test]
    fn one_workspace_is_one_cell_in_the_middle_of_the_bar() {
        let bar = bar(1);

        let [cell] = bar.cells[..] else {
            panic!("one cell: {:?}", bar.cells);
        };
        assert_eq!(cell.size.h, 162 - 2 * PADDING);
        assert!(has_the_outputs_aspect(cell));
        assert!((cell.loc.x - (1920 - cell.loc.x - cell.size.w)).abs() <= 1);
        assert_eq!(cell.loc.y, PADDING);
    }

    #[test]
    fn four_workspaces_are_one_row_in_their_order() {
        let bar = bar(4);

        assert_eq!(bar.cells.len(), 4);
        for pair in bar.cells.windows(2) {
            assert_eq!(pair[1].loc.y, pair[0].loc.y);
            assert_eq!(pair[1].size, pair[0].size);
            assert_eq!(pair[1].loc.x, pair[0].loc.x + pair[0].size.w + PADDING);
        }
        // They fit at the bar's height.
        assert_eq!(bar.cells[0].size.h, 162 - 2 * PADDING);
        assert!(has_the_outputs_aspect(bar.cells[0]));
        let (first, last) = (bar.cells[0], bar.cells[3]);
        assert!((first.loc.x - (1920 - last.loc.x - last.size.w)).abs() <= 1);
    }

    #[test]
    fn so_many_workspaces_that_they_dont_fit_are_all_made_smaller_alike() {
        let bar = bar(12);

        let cell = bar.cells[0].size;
        assert!(cell.h < 162 - 2 * PADDING, "{cell:?}");
        assert!(bar.cells.iter().all(|other| other.size == cell));
        assert!(has_the_outputs_aspect(bar.cells[0]));
        assert!(bar.cells[0].loc.x >= PADDING);
        let last = bar.cells[11];
        assert!(last.loc.x + last.size.w <= 1920 - PADDING);
        // They're in the middle of the bar's height.
        assert!((bar.cells[0].loc.y - (162 - cell.h - bar.cells[0].loc.y)).abs() <= 1);
    }

    #[test]
    fn a_point_in_a_cell_is_its_workspace() {
        let bar = bar(4);
        let middle = |cell: Rectangle<i32, Logical>| {
            Point::<i32, Logical>::from((
                cell.loc.x + cell.size.w / 2,
                cell.loc.y + cell.size.h / 2,
            ))
            .to_f64()
        };

        assert_eq!(bar.cell_at(middle(bar.cells[2])), Some(2));
        assert_eq!(bar.cell_at((1.0, 1.0).into()), None);
        assert_eq!(bar.cell_at((960.0, 500.0).into()), None);
    }
}
