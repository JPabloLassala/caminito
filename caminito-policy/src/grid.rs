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

/// Marco's `wrap-style`: where a move past an edge of the grid goes (150).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WrapStyle {
    /// Nowhere: the move does nothing, as by default.
    #[default]
    NoWrap,
    /// On to the next row or column: the end of a row leads to the start of the next, and
    /// the bottom of a column to the top of the next, the last to the first.
    Classic,
    /// Round: the end of a row leads to its own start, and of a column to its own top.
    Toroidal,
}

/// A way to go from one workspace to the one beside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    Left,
    Right,
    Up,
    Down,
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

    /// The workspace one cell from `workspace` in `direction`, or, past an edge of the grid,
    /// where `wrap` takes the move, as Marco's `meta_workspace_get_neighbor` does
    /// (workspace.c:830-960). `None` when the move goes nowhere, or back to `workspace`.
    ///
    /// The move runs along a line, the workspace's row for left and right and its column
    /// for up and down. An empty cell is past the line's edge: the filled cells of a line
    /// are always at its start, since the grid is filled in order from the top left.
    pub(crate) fn neighbour(
        &self,
        workspace: usize,
        direction: Direction,
        wrap: WrapStyle,
    ) -> Option<usize> {
        let (column, row) = self.cell(workspace);
        let across = matches!(direction, Direction::Left | Direction::Right);
        let forward = matches!(direction, Direction::Right | Direction::Down);
        let (line, along, lines, length) = if across {
            (row, column, self.rows, self.columns)
        } else {
            (column, row, self.columns, self.rows)
        };
        let at = |line: usize, along: usize| {
            if across {
                self.workspace_at(along, line)
            } else {
                self.workspace_at(line, along)
            }
        };
        // The first or the last workspace of a line, the end a move into it comes in at.
        let end = |line: usize| {
            if forward {
                at(line, 0)
            } else {
                (0..length).rev().find_map(|along| at(line, along))
            }
        };
        let next = if forward {
            Some(along + 1)
        } else {
            along.checked_sub(1)
        };
        let neighbour = match next.and_then(|along| at(line, along)) {
            Some(neighbour) => Some(neighbour),
            None => match wrap {
                WrapStyle::NoWrap => None,
                WrapStyle::Toroidal => end(line),
                // The next line, or the previous one, that has a workspace. The
                // workspace's own line has, so the search ends.
                WrapStyle::Classic => (1..=lines).find_map(|step| {
                    let line = if forward {
                        (line + step) % lines
                    } else {
                        (line + lines - step) % lines
                    };
                    end(line)
                }),
            },
        };
        neighbour.filter(|neighbour| *neighbour != workspace)
    }
}
