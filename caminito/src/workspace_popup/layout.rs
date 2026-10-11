//! The Workspace popup's geometry (152): the popup in the middle of its output, a cell for
//! each workspace in the Workspace grid's rows and columns, and in each cell a preview of
//! the workspace over its name. It's pure, so the popup, the drawing and the debug dump all
//! read the same rectangles.

use caminito_policy::Grid;
use smithay::utils::{Logical, Rectangle, Size};

/// The space between the popup's edge and the cells.
const PADDING: i32 = 12;
/// The space between two cells.
const GAP: i32 = 4;
/// The space between a cell's edge and what it shows, where the selected cell's fill shows.
const CELL_PADDING: i32 = 6;
/// The height of the name under a preview, and the space between the two.
const NAME_HEIGHT: i32 = 20;
const NAME_GAP: i32 = 4;
/// The share of the output's width the previews of one row of every cell take together, as
/// Marco's `MINI_WORKSPACE_SCREEN_FRACTION` of 0.33 (tabpopup.c:1009, 1032).
const SCREEN_FRACTION: i32 = 3;
/// The narrowest preview that rule gives, so that a name has room in a grid of many cells.
/// Marco's popup has no floor: its names aren't in the cells.
const MIN_PREVIEW_WIDTH: i32 = 96;
/// The popup is within this many tenths of the output, as the Window switcher's is (108).
const FIT_TENTHS: i32 = 9;

/// One workspace's cell, from the output's corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Cell {
    /// The whole cell, which the selection fills.
    pub(crate) rect: Rectangle<i32, Logical>,
    /// The workspace in small: the output's shape, with an outline of each window.
    pub(crate) preview: Rectangle<i32, Logical>,
    /// The band the workspace's name is drawn in.
    pub(crate) name: Rectangle<i32, Logical>,
}

/// The popup's rectangles on one output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Layout {
    pub(crate) columns: usize,
    pub(crate) rows: usize,
    /// The popup, from the output's corner.
    pub(crate) popup: Rectangle<i32, Logical>,
    /// Each workspace's cell, from the first. An empty cell of the grid has none.
    pub(crate) cells: Vec<Cell>,
}

impl Layout {
    /// The popup for `count` workspaces in `grid`, on an output of `output` logical
    /// pixels. A preview has the output's shape, and shrinks until the popup fits.
    pub(crate) fn new(grid: &Grid, count: usize, output: Size<i32, Logical>) -> Self {
        let (columns, rows) = (grid.columns() as i32, grid.rows() as i32);
        let output = Size::<i32, Logical>::from((output.w.max(1), output.h.max(1)));
        // What a preview can be at most, for the popup to fit.
        let around = (2 * PADDING, 2 * CELL_PADDING);
        let fit_w =
            (output.w * FIT_TENTHS / 10 - around.0 - (columns - 1) * GAP) / columns - around.1;
        let fit_h = (output.h * FIT_TENTHS / 10 - around.0 - (rows - 1) * GAP) / rows
            - around.1
            - NAME_GAP
            - NAME_HEIGHT;
        let mut width = (output.w / SCREEN_FRACTION / (columns * rows))
            .max(MIN_PREVIEW_WIDTH)
            .min(fit_w)
            .max(1);
        let mut height = (width * output.h / output.w).max(1);
        if height > fit_h {
            height = fit_h.max(1);
            width = (height * output.w / output.h).max(1);
        }

        let cell = Size::<i32, Logical>::from((
            width + around.1,
            height + around.1 + NAME_GAP + NAME_HEIGHT,
        ));
        let size = Size::<i32, Logical>::from((
            around.0 + columns * cell.w + (columns - 1) * GAP,
            around.0 + rows * cell.h + (rows - 1) * GAP,
        ));
        let popup = Rectangle::new(
            ((output.w - size.w) / 2, (output.h - size.h) / 2).into(),
            size,
        );
        let cells = (0..count)
            .map(|workspace| {
                let (column, row) = grid.cell(workspace);
                let loc = (
                    popup.loc.x + PADDING + column as i32 * (cell.w + GAP),
                    popup.loc.y + PADDING + row as i32 * (cell.h + GAP),
                );
                let inside = (loc.0 + CELL_PADDING, loc.1 + CELL_PADDING);
                Cell {
                    rect: Rectangle::new(loc.into(), cell),
                    preview: Rectangle::new(inside.into(), (width, height).into()),
                    name: Rectangle::new(
                        (inside.0, inside.1 + height + NAME_GAP).into(),
                        (width, NAME_HEIGHT).into(),
                    ),
                }
            })
            .collect();
        Self {
            columns: grid.columns(),
            rows: grid.rows(),
            popup,
            cells,
        }
    }
}
