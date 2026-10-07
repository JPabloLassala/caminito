//! The workspace grid (96): where each workspace is for a count and the settings' rows and
//! columns, and the workspace beside one.

use crate::grid::Direction;
use crate::{Grid, Orientation, WorkspaceLayout};

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
    assert_eq!(grid.neighbour(0, Direction::Right), Some(1));
    assert_eq!(grid.neighbour(1, Direction::Left), Some(0));
    assert_eq!(grid.neighbour(3, Direction::Right), Some(4));
}

#[test]
fn there_s_no_neighbour_past_an_edge_or_in_an_empty_cell() {
    // 0 1 2
    // 3 4
    let grid = Grid::new(5, layout(Orientation::Horizontal, 2, 0));
    assert_eq!(grid.neighbour(0, Direction::Left), None);
    // The end of a row isn't the start of the next.
    assert_eq!(grid.neighbour(2, Direction::Right), None);
    assert_eq!(grid.neighbour(3, Direction::Left), None);
    assert_eq!(grid.neighbour(4, Direction::Right), None);
}
