//! The workspace grid (09, 96): each workspace's column and row, for the pager and for
//! moving to the workspace beside one.
//!
//! Its shape is `_NET_DESKTOP_LAYOUT`'s, which a pager sets on X11 and Marco reads
//! (`meta_screen_calc_workspace_layout`, screen.c): an orientation, rows and columns, with
//! 0 for "derive it from the other". The starting corner is always the top left.

/// The order the grid is filled in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Along each row, from the top one.
    Horizontal,
    /// Down each column, from the left one.
    Vertical,
}

/// The grid as the settings ask for it (09's `workspace-layout-*` keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceLayout {
    /// The order the cells are filled in.
    pub orientation: Orientation,
    /// 0 to derive them from the columns.
    pub rows: usize,
    /// 0 to derive them from the rows.
    pub columns: usize,
}

impl Default for WorkspaceLayout {
    /// One row, as the schema's defaults have it.
    fn default() -> Self {
        Self {
            orientation: Orientation::Horizontal,
            rows: 1,
            columns: 0,
        }
    }
}

/// A way to go from one workspace to the one beside it. Up and down come with their
/// bindings, in M8 (17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    Left,
    Right,
}

/// The grid for a number of workspaces: enough cells for all of them, filled from the top
/// left in the orientation's order. The last cells are empty when there are more than workspaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grid {
    count: usize,
    orientation: Orientation,
    columns: usize,
    rows: usize,
}

impl Grid {
    /// The grid these settings give `count` workspaces.
    ///
    /// With neither rows nor columns it's one row, as in Marco. With both, and fewer cells
    /// than workspaces, the dimension the fill order runs along grows (09): the columns of
    /// a horizontal grid, the rows of a vertical one. Marco leaves such workspaces out of
    /// the grid instead.
    pub fn new(count: usize, layout: WorkspaceLayout) -> Self {
        let count = count.max(1);
        let WorkspaceLayout {
            orientation,
            mut rows,
            mut columns,
        } = layout;
        if rows == 0 && columns == 0 {
            rows = 1;
        }
        if rows > 0 && columns > 0 && rows * columns < count {
            match orientation {
                Orientation::Horizontal => columns = 0,
                Orientation::Vertical => rows = 0,
            }
        }
        if rows == 0 {
            rows = count.div_ceil(columns);
        }
        if columns == 0 {
            columns = count.div_ceil(rows);
        }
        Self {
            count,
            orientation,
            columns,
            rows,
        }
    }

    /// How many columns the grid has.
    pub fn columns(&self) -> usize {
        self.columns
    }

    /// How many rows the grid has.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// A workspace's column and row, both from 0.
    pub fn cell(&self, workspace: usize) -> (usize, usize) {
        match self.orientation {
            Orientation::Horizontal => (workspace % self.columns, workspace / self.columns),
            Orientation::Vertical => (workspace / self.rows, workspace % self.rows),
        }
    }

    /// The workspace in a cell, if the cell is in the grid and isn't empty.
    pub(crate) fn workspace_at(&self, column: usize, row: usize) -> Option<usize> {
        if column >= self.columns || row >= self.rows {
            return None;
        }
        let workspace = match self.orientation {
            Orientation::Horizontal => row * self.columns + column,
            Orientation::Vertical => column * self.rows + row,
        };
        (workspace < self.count).then_some(workspace)
    }

    /// The workspace one cell from `workspace`, if there is one. Nothing wraps: the end of
    /// a row isn't the start of the next.
    pub(crate) fn neighbour(&self, workspace: usize, direction: Direction) -> Option<usize> {
        let (column, row) = self.cell(workspace);
        let (column, row) = match direction {
            Direction::Left => (column.checked_sub(1)?, row),
            Direction::Right => (column + 1, row),
        };
        self.workspace_at(column, row)
    }
}
