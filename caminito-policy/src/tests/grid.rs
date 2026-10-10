//! The workspace grid (96): where each workspace is for a count and the settings' rows and
//! columns, and the workspace beside one, by `wrap-style` (150).

use crate::grid::Direction::{self, Down, Left, Right, Up};
use crate::{Grid, Orientation, WorkspaceLayout, WrapStyle};

fn layout(orientation: Orientation, rows: usize, columns: usize) -> WorkspaceLayout {
    WorkspaceLayout {
        orientation,
        rows,
        columns,
    }
}

/// Each workspace's column and row, from the first.
fn cells(count: usize, layout: WorkspaceLayout) -> Vec<(usize, usize)> {
    let grid = Grid::new(count, layout);
    (0..count).map(|workspace| grid.cell(workspace)).collect()
}

#[test]
fn the_default_grid_is_one_row() {
    let grid = Grid::new(4, WorkspaceLayout::default());
    assert_eq!((grid.columns(), grid.rows()), (4, 1));
    assert_eq!(
        cells(4, WorkspaceLayout::default()),
        [(0, 0), (1, 0), (2, 0), (3, 0)]
    );
}

#[test]
fn neither_rows_nor_columns_is_one_row() {
    let neither = layout(Orientation::Vertical, 0, 0);
    assert_eq!(cells(3, neither), [(0, 0), (1, 0), (2, 0)]);
}

#[test]
fn two_rows_of_an_odd_count_leave_the_last_cell_empty() {
    let two_rows = layout(Orientation::Horizontal, 2, 0);
    let grid = Grid::new(5, two_rows);
    assert_eq!((grid.columns(), grid.rows()), (3, 2));
    assert_eq!(cells(5, two_rows), [(0, 0), (1, 0), (2, 0), (0, 1), (1, 1)]);
    assert_eq!(grid.workspace_at(2, 1), None);
}

#[test]
fn rows_are_derived_from_the_columns() {
    let three_columns = layout(Orientation::Horizontal, 0, 3);
    let grid = Grid::new(7, three_columns);
    assert_eq!((grid.columns(), grid.rows()), (3, 3));
    assert_eq!(grid.cell(3), (0, 1));
    assert_eq!(grid.cell(6), (0, 2));
}

#[test]
fn a_vertical_grid_fills_down_each_column() {
    // As the pager on a vertical panel writes it: its "rows" preference is columns there.
    let two_columns = layout(Orientation::Vertical, 0, 2);
    let grid = Grid::new(6, two_columns);
    assert_eq!((grid.columns(), grid.rows()), (2, 3));
    assert_eq!(
        cells(6, two_columns),
        [(0, 0), (0, 1), (0, 2), (1, 0), (1, 1), (1, 2)]
    );
}

#[test]
fn a_grid_with_more_cells_than_workspaces_is_kept_as_it_is() {
    let three_by_three = layout(Orientation::Horizontal, 3, 3);
    let grid = Grid::new(4, three_by_three);
    assert_eq!((grid.columns(), grid.rows()), (3, 3));
    assert_eq!(cells(4, three_by_three), [(0, 0), (1, 0), (2, 0), (0, 1)]);
    assert_eq!(grid.workspace_at(1, 1), None);
    assert_eq!(grid.workspace_at(0, 2), None);
}

#[test]
fn a_grid_too_small_for_the_workspaces_grows_along_its_fill_order() {
    let horizontal = Grid::new(6, layout(Orientation::Horizontal, 2, 2));
    assert_eq!((horizontal.columns(), horizontal.rows()), (3, 2));
    assert_eq!(horizontal.cell(5), (2, 1));

    let vertical = Grid::new(6, layout(Orientation::Vertical, 2, 2));
    assert_eq!((vertical.columns(), vertical.rows()), (2, 3));
    assert_eq!(vertical.cell(5), (1, 2));
}

#[test]
fn no_two_workspaces_share_a_cell_whatever_the_settings() {
    for orientation in [Orientation::Horizontal, Orientation::Vertical] {
        for rows in 0..=5 {
            for columns in 0..=5 {
                for count in 1..=12 {
                    let grid = Grid::new(count, layout(orientation, rows, columns));
                    for workspace in 0..count {
                        let (column, row) = grid.cell(workspace);
                        assert!(column < grid.columns() && row < grid.rows());
                        assert_eq!(grid.workspace_at(column, row), Some(workspace));
                    }
                }
            }
        }
    }
}

#[test]
fn a_neighbour_is_the_workspace_in_the_next_cell() {
    // 0 1 2
    // 3 4
    let grid = Grid::new(5, layout(Orientation::Horizontal, 2, 0));
    assert_eq!(
        grid.neighbour(0, Direction::Right, WrapStyle::NoWrap),
        Some(1)
    );
    assert_eq!(
        grid.neighbour(1, Direction::Left, WrapStyle::NoWrap),
        Some(0)
    );
    assert_eq!(
        grid.neighbour(3, Direction::Right, WrapStyle::NoWrap),
        Some(4)
    );
}

#[test]
fn there_s_no_neighbour_past_an_edge_or_in_an_empty_cell() {
    // 0 1 2
    // 3 4
    let grid = Grid::new(5, layout(Orientation::Horizontal, 2, 0));
    assert_eq!(grid.neighbour(0, Direction::Left, WrapStyle::NoWrap), None);
    // The end of a row isn't the start of the next.
    assert_eq!(grid.neighbour(2, Direction::Right, WrapStyle::NoWrap), None);
    assert_eq!(grid.neighbour(3, Direction::Left, WrapStyle::NoWrap), None);
    assert_eq!(grid.neighbour(4, Direction::Right, WrapStyle::NoWrap), None);
}

/// Where each move goes from each workspace of `grid`, as `[left, right, up, down]`, with
/// `None` for no move.
fn moves(grid: Grid, wrap: WrapStyle, count: usize) -> Vec<[Option<usize>; 4]> {
    (0..count)
        .map(|workspace| [Left, Right, Up, Down].map(|d| grid.neighbour(workspace, d, wrap)))
        .collect()
}

#[test]
fn up_and_down_in_one_row_go_nowhere_but_with_classic() {
    let grid = Grid::new(4, WorkspaceLayout::default());
    for wrap in [WrapStyle::NoWrap, WrapStyle::Toroidal] {
        for workspace in 0..4 {
            assert_eq!(grid.neighbour(workspace, Up, wrap), None, "{wrap:?}");
            assert_eq!(grid.neighbour(workspace, Down, wrap), None, "{wrap:?}");
        }
    }
    // Each column is one workspace, so classic goes on to the column before or after, as
    // Marco's does.
    assert_eq!(grid.neighbour(1, Up, WrapStyle::Classic), Some(0));
    assert_eq!(grid.neighbour(0, Up, WrapStyle::Classic), Some(3));
    assert_eq!(grid.neighbour(3, Down, WrapStyle::Classic), Some(0));
}

#[test]
fn classic_in_one_row_runs_on_from_each_end_to_the_other() {
    let grid = Grid::new(4, WorkspaceLayout::default());
    assert_eq!(grid.neighbour(3, Right, WrapStyle::Classic), Some(0));
    assert_eq!(grid.neighbour(0, Left, WrapStyle::Classic), Some(3));
    assert_eq!(grid.neighbour(3, Right, WrapStyle::Toroidal), Some(0));
    assert_eq!(grid.neighbour(0, Left, WrapStyle::Toroidal), Some(3));
}

#[test]
fn a_full_grid_stops_at_its_edges_with_no_wrap() {
    // 0 1
    // 2 3
    let grid = Grid::new(4, layout(Orientation::Horizontal, 2, 0));
    assert_eq!(
        moves(grid, WrapStyle::NoWrap, 4),
        [
            [None, Some(1), None, Some(2)],
            [Some(0), None, None, Some(3)],
            [None, Some(3), Some(0), None],
            [Some(2), None, Some(1), None],
        ]
    );
}

#[test]
fn toroidal_takes_each_edge_of_a_full_grid_round_to_its_own_row_or_column() {
    // 0 1 2
    // 3 4 5
    let grid = Grid::new(6, layout(Orientation::Horizontal, 2, 0));
    assert_eq!(
        moves(grid, WrapStyle::Toroidal, 6),
        [
            [Some(2), Some(1), Some(3), Some(3)],
            [Some(0), Some(2), Some(4), Some(4)],
            [Some(1), Some(0), Some(5), Some(5)],
            [Some(5), Some(4), Some(0), Some(0)],
            [Some(3), Some(5), Some(1), Some(1)],
            [Some(4), Some(3), Some(2), Some(2)],
        ]
    );
}

#[test]
fn classic_takes_each_edge_of_a_full_grid_on_to_the_next_row_or_column() {
    // 0 1 2
    // 3 4 5
    // The end of a row leads to the start of the next, and the end of a column to the top
    // of the next, the last to the first, as in Marco (`meta_workspace_get_neighbor`).
    let grid = Grid::new(6, layout(Orientation::Horizontal, 2, 0));
    assert_eq!(
        moves(grid, WrapStyle::Classic, 6),
        [
            [Some(5), Some(1), Some(5), Some(3)],
            [Some(0), Some(2), Some(3), Some(4)],
            [Some(1), Some(3), Some(4), Some(5)],
            [Some(2), Some(4), Some(0), Some(1)],
            [Some(3), Some(5), Some(1), Some(2)],
            [Some(4), Some(0), Some(2), Some(0)],
        ]
    );
}

#[test]
fn a_part_filled_grid_s_empty_cells_are_past_its_edges() {
    // 0 1 2
    // 3 4 5
    // 6
    let grid = Grid::new(7, layout(Orientation::Horizontal, 0, 3));
    assert_eq!(
        moves(grid, WrapStyle::NoWrap, 7)[4..],
        [
            [Some(3), Some(5), Some(1), None],
            [Some(4), None, Some(2), None],
            [None, None, Some(3), None],
        ]
    );
    // Toroidal goes round the filled part of the row or column, as Marco does.
    assert_eq!(
        moves(grid, WrapStyle::Toroidal, 7),
        [
            [Some(2), Some(1), Some(6), Some(3)],
            [Some(0), Some(2), Some(4), Some(4)],
            [Some(1), Some(0), Some(5), Some(5)],
            [Some(5), Some(4), Some(0), Some(6)],
            [Some(3), Some(5), Some(1), Some(1)],
            [Some(4), Some(3), Some(2), Some(2)],
            [None, None, Some(3), Some(0)],
        ]
    );
    // Classic goes on to the start or the end of the filled part of the next or previous.
    assert_eq!(
        moves(grid, WrapStyle::Classic, 7),
        [
            [Some(6), Some(1), Some(5), Some(3)],
            [Some(0), Some(2), Some(6), Some(4)],
            [Some(1), Some(3), Some(4), Some(5)],
            [Some(2), Some(4), Some(0), Some(6)],
            [Some(3), Some(5), Some(1), Some(2)],
            [Some(4), Some(6), Some(2), Some(0)],
            [Some(5), Some(0), Some(3), Some(1)],
        ]
    );
}

#[test]
fn classic_passes_over_a_row_with_no_workspaces() {
    // 0 1 2
    // 3
    // (an empty row)
    let grid = Grid::new(4, layout(Orientation::Horizontal, 3, 3));
    assert_eq!(grid.neighbour(3, Right, WrapStyle::Classic), Some(0));
    assert_eq!(grid.neighbour(0, Left, WrapStyle::Classic), Some(3));
    // Column 2 has only workspace 2, so up from 0 is the bottom of that column.
    assert_eq!(grid.neighbour(0, Up, WrapStyle::Classic), Some(2));
}

#[test]
fn a_vertical_grid_wraps_as_a_horizontal_one_turned() {
    // 0 2
    // 1
    let grid = Grid::new(3, layout(Orientation::Vertical, 2, 0));
    assert_eq!(grid.neighbour(1, Down, WrapStyle::Classic), Some(2));
    assert_eq!(grid.neighbour(2, Down, WrapStyle::Classic), Some(0));
    assert_eq!(grid.neighbour(1, Right, WrapStyle::Classic), Some(0));
    assert_eq!(grid.neighbour(2, Down, WrapStyle::Toroidal), None);
    assert_eq!(grid.neighbour(1, Right, WrapStyle::Toroidal), None);
    assert_eq!(grid.neighbour(0, Left, WrapStyle::Toroidal), Some(2));
}
