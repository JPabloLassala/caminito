//! Where the Window switcher's popup and its cells are, which cell a point is in and where
//! an arrow key goes (108). Pure geometry: no Wayland and no drawing.

use smithay::utils::{Logical, Point, Rectangle, Size};

/// What a cell shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "Task 4 chooses the style from the settings")
)]
pub(crate) enum CellStyle {
    /// A thumbnail of the window, with its icon in a corner.
    Thumbnails,
    /// The window's icon alone, `icon_size` logical pixels square.
    Icons { icon_size: i32 },
}

/// A way an arrow key moves the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "Task 4 maps the arrow keys to them")
)]
pub(crate) enum Direction {
    Left,
    Right,
    Up,
    Down,
}

/// The Window switcher's popup, laid out for one output. Every rectangle is in logical
/// pixels from the output's top left corner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Grid {
    /// The whole popup, centred on the output.
    pub popup: Rectangle<i32, Logical>,
    /// Each entry's cell, in the list's order, row by row.
    pub cells: Vec<Rectangle<i32, Logical>>,
    /// Where the selected window's title is drawn, under the cells.
    pub title: Rectangle<i32, Logical>,
    pub columns: usize,
    pub style: CellStyle,
}

/// The padding between the popup's edge and what it holds.
const PADDING: i32 = 12;
/// The gap between two cells, and between the cells and the title band.
const GAP: i32 = 8;
/// The height of the title band under the cells.
const TITLE_HEIGHT: i32 = 32;
/// The padding between a cell's edge and its content.
const CELL_PADDING: i32 = 8;
/// The widest a thumbnail's content box is.
const MAX_THUMBNAIL_WIDTH: i32 = 256;
/// The highest a thumbnail's content box is.
const MAX_THUMBNAIL_HEIGHT: i32 = 160;
/// The smallest a content box is while the popup can keep it so.
const MIN_CONTENT: i32 = 16;
/// The most of the output's width and of its height the popup takes, as a fraction.
const OUTPUT_FRACTION: (i32, i32) = (9, 10);

impl Grid {
    /// Lays `count` entries out on an output of `output` logical pixels, in rows of at
    /// most `max_columns`. `count` is at least 1.
    ///
    /// The content boxes shrink by one factor until the popup is within
    /// [`OUTPUT_FRACTION`] of the output, but not under [`MIN_CONTENT`]. At that minimum the
    /// popup may grow to the output. Where even that is too small for the rows or the
    /// columns, the minimum goes and the cells tile the output's room, down to one pixel:
    /// the popup is never larger than the output, and every cell has a positive size.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Task 4 opens the popup with it")
    )]
    pub(crate) fn new(
        count: usize,
        max_columns: usize,
        output: Size<i32, Logical>,
        style: CellStyle,
    ) -> Grid {
        let columns = count.min(max_columns).max(1);
        let rows = count.div_ceil(columns);
        let (cols_i, rows_i) = (to_i32(columns), to_i32(rows));

        let base: Size<i32, Logical> = match style {
            CellStyle::Thumbnails => Size::from((MAX_THUMBNAIL_WIDTH, MAX_THUMBNAIL_HEIGHT)),
            CellStyle::Icons { icon_size } => Size::from((icon_size.max(1), icon_size.max(1))),
        };
        // What the popup has besides the cells' own size: padding, gaps, cells' padding
        // and the title band.
        let extra_w = 2 * PADDING + (cols_i - 1) * GAP + cols_i * 2 * CELL_PADDING;
        let extra_h =
            2 * PADDING + (rows_i - 1) * GAP + rows_i * 2 * CELL_PADDING + GAP + TITLE_HEIGHT;
        let room_w = f64::from(output.w * OUTPUT_FRACTION.0 / OUTPUT_FRACTION.1 - extra_w);
        let room_h = f64::from(output.h * OUTPUT_FRACTION.0 / OUTPUT_FRACTION.1 - extra_h);

        let (base_w, base_h) = (f64::from(base.w), f64::from(base.h));
        let fits = (room_w / (cols_i as f64 * base_w)).min(room_h / (rows_i as f64 * base_h));
        let smallest = (f64::from(MIN_CONTENT) / base_w).max(f64::from(MIN_CONTENT) / base_h);
        let factor = fits.min(1.0).max(smallest);
        let content_w = ((base_w * factor).floor() as i32).max(MIN_CONTENT);
        let content_h = ((base_h * factor).floor() as i32).max(MIN_CONTENT);
        let cell = Size::<i32, Logical>::from((
            content_w + 2 * CELL_PADDING,
            content_h + 2 * CELL_PADDING,
        ));

        // The cells at their size; where the output has no room for them they tile it.
        let across = fit_axis(cols_i, cell.w, 2 * PADDING, output.w);
        let down = fit_axis(rows_i, cell.h, 2 * PADDING + GAP + TITLE_HEIGHT, output.h);

        let popup = Rectangle::new(
            Point::from(((output.w - across.extent) / 2, (output.h - down.extent) / 2)),
            Size::from((across.extent, down.extent)),
        );
        let cells = (0..count)
            .map(|i| {
                let (row, column) = (to_i32(i / columns), to_i32(i % columns));
                Rectangle::new(
                    Point::from((
                        popup.loc.x + PADDING + column * (across.cell + across.gap),
                        popup.loc.y + PADDING + row * (down.cell + down.gap),
                    )),
                    Size::from((across.cell, down.cell)),
                )
            })
            .collect();
        let title = Rectangle::new(
            Point::from((
                popup.loc.x + PADDING,
                popup.loc.y + PADDING + rows_i * down.cell + (rows_i - 1) * down.gap + GAP,
            )),
            Size::from((popup.size.w - 2 * PADDING, TITLE_HEIGHT)),
        );
        Grid {
            popup,
            cells,
            title,
            columns,
            style,
        }
    }

    /// The cell at `point`, if it's in one.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Task 5 hit-tests the pointer with it")
    )]
    pub(crate) fn cell_at(&self, point: Point<f64, Logical>) -> Option<usize> {
        self.cells.iter().position(|cell| {
            let (left, top) = (f64::from(cell.loc.x), f64::from(cell.loc.y));
            point.x >= left
                && point.x < left + f64::from(cell.size.w)
                && point.y >= top
                && point.y < top + f64::from(cell.size.h)
        })
    }

    /// The entry an arrow key moves the selection to from `from`.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Task 4 moves the selection with it")
    )]
    pub(crate) fn neighbour(&self, from: usize, direction: Direction) -> usize {
        let count = self.cells.len();
        let rows = count.div_ceil(self.columns);
        let (row, column) = (from / self.columns, from % self.columns);
        let in_row = |row: usize| (row * self.columns + column).min(count - 1);
        match direction {
            Direction::Left => (from + count - 1) % count,
            Direction::Right => (from + 1) % count,
            Direction::Up => in_row((row + rows - 1) % rows),
            Direction::Down => in_row((row + 1) % rows),
        }
    }

    /// The box inside cell `index` that its thumbnail or icon is fitted in.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Task 5 draws the cells with it")
    )]
    pub(crate) fn content(&self, index: usize) -> Rectangle<i32, Logical> {
        let cell = self.cells[index];
        // The cell's padding, less in the smallest cells so that some content is left.
        let inset = CELL_PADDING.min((cell.size.w.min(cell.size.h) - 1) / 2);
        Rectangle::new(
            cell.loc + Point::from((inset, inset)),
            Size::from((cell.size.w - 2 * inset, cell.size.h - 2 * inset)),
        )
    }

    /// Where a window of `source` logical pixels is drawn in cell `index`: scaled to fit
    /// `content(index)` with its aspect kept, never enlarged, and centred.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Task 6 places the thumbnails with it")
    )]
    pub(crate) fn thumbnail(
        &self,
        index: usize,
        source: Size<i32, Logical>,
    ) -> Rectangle<i32, Logical> {
        let content = self.content(index);
        let (source_w, source_h) = (f64::from(source.w.max(1)), f64::from(source.h.max(1)));
        let scale = (f64::from(content.size.w) / source_w)
            .min(f64::from(content.size.h) / source_h)
            .min(1.0);
        let size = Size::<i32, Logical>::from((
            ((source_w * scale).round() as i32).clamp(1, content.size.w),
            ((source_h * scale).round() as i32).clamp(1, content.size.h),
        ));
        Rectangle::new(
            content.loc
                + Point::from(((content.size.w - size.w) / 2, (content.size.h - size.h) / 2)),
            size,
        )
    }
}

/// One axis of the grid after fitting: the size of a cell, the gap between cells and the
/// popup's extent.
struct Axis {
    cell: i32,
    gap: i32,
    extent: i32,
}

/// Fits `count` cells of `cell` pixels along an axis of `limit` pixels, where `extra`
/// pixels go to what is not a cell or a gap. The cells keep their size if the popup is then
/// within `limit` (which is the output, whatever [`OUTPUT_FRACTION`] said). If it is not,
/// they tile what is left, with their gap, or with none if that leaves under a pixel each.
fn fit_axis(count: i32, cell: i32, extra: i32, limit: i32) -> Axis {
    let needed = extra + count * cell + (count - 1) * GAP;
    if needed <= limit {
        return Axis {
            cell,
            gap: GAP,
            extent: needed,
        };
    }
    let room = limit - extra;
    let (gap, cell) = match (room - (count - 1) * GAP) / count {
        cell if cell >= 1 => (GAP, cell),
        _ => (0, (room / count).max(1)),
    };
    Axis {
        cell,
        gap,
        extent: (extra + count * cell + (count - 1) * gap).min(limit),
    }
}

fn to_i32(n: usize) -> i32 {
    i32::try_from(n).expect("a window count fits an i32")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_hd() -> Size<i32, Logical> {
        Size::from((1920, 1080))
    }

    fn thumbnails(count: usize, max_columns: usize, output: Size<i32, Logical>) -> Grid {
        Grid::new(count, max_columns, output, CellStyle::Thumbnails)
    }

    fn right(r: Rectangle<i32, Logical>) -> i32 {
        r.loc.x + r.size.w
    }

    fn bottom(r: Rectangle<i32, Logical>) -> i32 {
        r.loc.y + r.size.h
    }

    fn contains(outer: Rectangle<i32, Logical>, inner: Rectangle<i32, Logical>) -> bool {
        inner.loc.x >= outer.loc.x
            && inner.loc.y >= outer.loc.y
            && right(inner) <= right(outer)
            && bottom(inner) <= bottom(outer)
    }

    fn overlap(a: Rectangle<i32, Logical>, b: Rectangle<i32, Logical>) -> bool {
        a.loc.x < right(b) && b.loc.x < right(a) && a.loc.y < bottom(b) && b.loc.y < bottom(a)
    }

    fn centre_of(r: Rectangle<i32, Logical>) -> Point<f64, Logical> {
        Point::from((
            f64::from(r.loc.x) + f64::from(r.size.w) / 2.0,
            f64::from(r.loc.y) + f64::from(r.size.h) / 2.0,
        ))
    }

    #[test]
    fn a_grid_wraps_at_the_maximum_columns() {
        let grid = thumbnails(7, 5, full_hd());
        assert_eq!(grid.columns, 5);
        assert_eq!(grid.cells.len(), 7);
        // Two rows: the sixth cell is under the first, the seventh under the second.
        assert_eq!(grid.cells[5].loc.x, grid.cells[0].loc.x);
        assert_eq!(grid.cells[6].loc.x, grid.cells[1].loc.x);
        assert!(grid.cells[5].loc.y > grid.cells[0].loc.y);
        assert_eq!(grid.cells[4].loc.y, grid.cells[0].loc.y);
        assert_eq!(grid.cells[6].loc.y, grid.cells[5].loc.y);
    }

    #[test]
    fn fewer_entries_than_the_maximum_make_one_row() {
        let grid = thumbnails(3, 5, full_hd());
        assert_eq!(grid.columns, 3);
        assert!(grid.cells.iter().all(|c| c.loc.y == grid.cells[0].loc.y));
    }

    #[test]
    fn the_popup_is_centred_on_the_output() {
        for count in [1, 3, 7, 30] {
            let grid = thumbnails(count, 5, full_hd());
            let p = grid.popup;
            // An odd margin leaves one pixel more on the right and at the bottom.
            let (left, below) = (
                full_hd().w - right(p) - p.loc.x,
                full_hd().h - bottom(p) - p.loc.y,
            );
            assert!((0..=1).contains(&left), "{count} entries, horizontally");
            assert!((0..=1).contains(&below), "{count} entries, vertically");
        }
    }

    #[test]
    fn cells_do_not_overlap_and_lie_inside_the_popup() {
        for count in [1, 2, 7, 12, 30] {
            let grid = thumbnails(count, 5, full_hd());
            for (i, a) in grid.cells.iter().enumerate() {
                assert!(contains(grid.popup, *a), "cell {i} of {count}");
                assert!(
                    !overlap(*a, grid.title),
                    "cell {i} of {count} over the title"
                );
                for b in &grid.cells[i + 1..] {
                    assert!(!overlap(*a, *b), "{count} entries: {a:?} and {b:?}");
                }
            }
            assert!(contains(grid.popup, grid.title), "{count} entries: title");
        }
    }

    #[test]
    fn thumbnails_shrink_when_there_are_many_windows() {
        let few = thumbnails(3, 5, full_hd());
        let many = thumbnails(30, 5, full_hd());
        assert!(many.cells[0].size.w < few.cells[0].size.w);
        assert!(many.cells[0].size.h < few.cells[0].size.h);
        assert!(many.popup.size.w <= full_hd().w * 9 / 10);
        assert!(many.popup.size.h <= full_hd().h * 9 / 10);
        // One factor both ways: the content box keeps its 256 : 160 shape.
        let content = many.content(0);
        let (w, h) = (f64::from(content.size.w), f64::from(content.size.h));
        assert!((w / h - 1.6).abs() < 0.05, "{content:?}");
    }

    #[test]
    fn sixty_windows_on_a_small_output_keep_positive_cells_inside_it() {
        let output = Size::from((800, 600));
        let screen = Rectangle::from_size(output);
        let grid = thumbnails(60, 5, output);
        assert_eq!(grid.cells.len(), 60);
        assert!(contains(screen, grid.popup));
        for (i, cell) in grid.cells.iter().enumerate() {
            assert!(cell.size.w > 0 && cell.size.h > 0, "cell {i}: {cell:?}");
            assert!(contains(screen, *cell), "cell {i}: {cell:?}");
            let content = grid.content(i);
            assert!(
                content.size.w >= 16 && content.size.h >= 16,
                "content {i}: {content:?}"
            );
        }
    }

    #[test]
    fn more_windows_than_fit_at_the_minimum_let_the_minimum_go_not_the_output() {
        // One column of 200 entries cannot hold 16 pixel boxes on 600 pixels.
        let output = Size::from((800, 600));
        let screen = Rectangle::from_size(output);
        let grid = thumbnails(200, 1, output);
        assert!(contains(screen, grid.popup));
        for (i, cell) in grid.cells.iter().enumerate() {
            assert!(cell.size.w > 0 && cell.size.h > 0, "cell {i}: {cell:?}");
            assert!(contains(screen, *cell), "cell {i}: {cell:?}");
            let content = grid.content(i);
            assert!(
                content.size.w > 0 && content.size.h > 0,
                "content {i}: {content:?}"
            );
        }
        assert!(grid.content(0).size.h < 16);
    }

    #[test]
    fn an_icons_grid_has_cells_for_the_icon_size() {
        let grid = Grid::new(4, 5, full_hd(), CellStyle::Icons { icon_size: 48 });
        for i in 0..4 {
            assert_eq!(grid.content(i).size, Size::from((48, 48)));
            assert!(contains(grid.cells[i], grid.content(i)));
        }
        assert!(grid.cells[0].size.w > 48);
    }

    #[test]
    fn an_icons_grid_shrinks_like_a_thumbnails_grid() {
        let grid = Grid::new(
            40,
            5,
            Size::from((800, 600)),
            CellStyle::Icons { icon_size: 256 },
        );
        assert!(grid.content(0).size.w < 256);
        assert_eq!(grid.content(0).size.w, grid.content(0).size.h);
        assert!(grid.popup.size.w <= 720 && grid.popup.size.h <= 540);
    }

    #[test]
    fn left_and_right_wrap_at_the_ends_of_the_list() {
        let grid = thumbnails(7, 5, full_hd());
        assert_eq!(grid.neighbour(0, Direction::Right), 1);
        assert_eq!(grid.neighbour(3, Direction::Left), 2);
        assert_eq!(grid.neighbour(6, Direction::Right), 0);
        assert_eq!(grid.neighbour(0, Direction::Left), 6);
        let one = thumbnails(1, 5, full_hd());
        assert_eq!(one.neighbour(0, Direction::Right), 0);
        assert_eq!(one.neighbour(0, Direction::Left), 0);
    }

    #[test]
    fn up_and_down_keep_the_column_and_wrap() {
        let grid = thumbnails(10, 5, full_hd());
        assert_eq!(grid.neighbour(1, Direction::Down), 6);
        assert_eq!(grid.neighbour(6, Direction::Up), 1);
        assert_eq!(grid.neighbour(6, Direction::Down), 1);
        assert_eq!(grid.neighbour(1, Direction::Up), 6);
        let one_row = thumbnails(3, 5, full_hd());
        assert_eq!(one_row.neighbour(2, Direction::Down), 2);
        assert_eq!(one_row.neighbour(2, Direction::Up), 2);
    }

    #[test]
    fn down_into_a_short_last_row_goes_to_its_last_entry() {
        let grid = thumbnails(7, 5, full_hd());
        assert_eq!(grid.neighbour(4, Direction::Down), 6);
        assert_eq!(grid.neighbour(3, Direction::Down), 6);
        assert_eq!(grid.neighbour(2, Direction::Down), 6);
        assert_eq!(grid.neighbour(1, Direction::Down), 6);
        assert_eq!(grid.neighbour(0, Direction::Down), 5);
        // Up from the first row wraps to the short last row the same way.
        assert_eq!(grid.neighbour(4, Direction::Up), 6);
        assert_eq!(grid.neighbour(0, Direction::Up), 5);
    }

    #[test]
    fn a_point_is_in_the_cell_it_is_over_and_in_none_between_cells() {
        let grid = thumbnails(7, 5, full_hd());
        for (i, cell) in grid.cells.iter().enumerate() {
            assert_eq!(grid.cell_at(centre_of(*cell)), Some(i));
            let corner = Point::from((f64::from(cell.loc.x), f64::from(cell.loc.y)));
            assert_eq!(grid.cell_at(corner), Some(i), "top left corner of {i}");
        }
        let (a, b) = (grid.cells[0], grid.cells[1]);
        let gap = Point::from((f64::from(right(a)) + 1.0, centre_of(a).y));
        assert!(gap.x < f64::from(b.loc.x));
        assert_eq!(grid.cell_at(gap), None);
        // The empty place after the last entry, and the title and the padding.
        assert_eq!(
            grid.cell_at(centre_of(Rectangle::new(
                grid.cells[6].loc + Point::from((a.size.w + 8, 0)),
                a.size
            ))),
            None
        );
        assert_eq!(grid.cell_at(centre_of(grid.title)), None);
        assert_eq!(grid.cell_at(Point::from((0.0, 0.0))), None);
        assert_eq!(grid.cell_at(Point::from((-5.0, -5.0))), None);
    }

    #[test]
    fn a_thumbnail_keeps_its_aspect_and_is_never_enlarged() {
        let grid = thumbnails(3, 5, full_hd());
        let content = grid.content(1);

        let small = grid.thumbnail(1, Size::from((100, 50)));
        assert_eq!(small.size, Size::from((100, 50)));
        let centre = centre_of(small);
        let expected = centre_of(content);
        assert!((centre.x - expected.x).abs() <= 1.0 && (centre.y - expected.y).abs() <= 1.0);

        let big = grid.thumbnail(1, Size::from((1920, 1080)));
        assert!(contains(content, big));
        let ratio = f64::from(big.size.w) / f64::from(big.size.h);
        assert!(
            (f64::from(big.size.h) * 1920.0 / 1080.0 - f64::from(big.size.w)).abs() <= 1.0,
            "{ratio}"
        );
        assert!(big.size.w == content.size.w || big.size.h == content.size.h);
    }

    /// A rectangle as `x,y WxH`.
    fn show(r: Rectangle<i32, Logical>) -> String {
        format!("{},{} {}x{}", r.loc.x, r.loc.y, r.size.w, r.size.h)
    }

    #[test]
    fn a_seven_entry_grid_in_rectangles() {
        let grid = thumbnails(7, 5, full_hd());
        let mut text = format!("popup {}\ntitle {}\n", show(grid.popup), show(grid.title));
        for (i, cell) in grid.cells.iter().enumerate() {
            text.push_str(&format!(
                "cell {i} {} content {}\n",
                show(*cell),
                show(grid.content(i))
            ));
        }
        insta::assert_snapshot!(text, @"
        popup 252,328 1416x424
        title 264,708 1392x32
        cell 0 264,340 272x176 content 272,348 256x160
        cell 1 544,340 272x176 content 552,348 256x160
        cell 2 824,340 272x176 content 832,348 256x160
        cell 3 1104,340 272x176 content 1112,348 256x160
        cell 4 1384,340 272x176 content 1392,348 256x160
        cell 5 264,524 272x176 content 272,532 256x160
        cell 6 544,524 272x176 content 552,532 256x160
        ");
    }
}
