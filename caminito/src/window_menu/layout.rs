//! The window menu's geometry (161): a column of rows, each a mark, a label and a
//! keybinding, placed where it was asked for and kept on its output. It's pure, so the
//! menu, the drawing and the debug dump all read the same rectangles.

use smithay::utils::{Logical, Point, Rectangle, Size};

/// The space between the menu's edge and its rows, above and below.
const PADDING: i32 = 4;
/// The column before the labels, where a check or a radio mark is drawn.
pub(super) const MARK_COLUMN: i32 = 24;
/// The column after the labels and the keybindings, where the submenu's arrow is drawn.
pub(super) const ARROW_COLUMN: i32 = 20;
/// The space between the widest label and the keybindings.
const ACCELERATOR_GAP: i32 = 24;
/// The space above and below a row's text.
const ROW_PADDING: i32 = 5;
/// The height of a separator's row.
const SEPARATOR_HEIGHT: i32 = 7;
/// How far a submenu lies over its parent.
const SUBMENU_OVERLAP: i32 = 2;

/// What a row needs room for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Row {
    Separator,
    /// An item: the width of its label, and of its keybinding's text, or 0.
    Item {
        label: i32,
        accelerator: i32,
    },
}

/// A menu's rectangles, from its output's corner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Layout {
    /// The whole menu.
    pub(crate) rect: Rectangle<i32, Logical>,
    /// Each row, from the first, as wide as the menu.
    pub(crate) rows: Vec<Rectangle<i32, Logical>>,
}

impl Layout {
    /// The size of a menu of `rows`, whose text is `text_height` high.
    fn size(rows: &[Row], text_height: i32) -> Size<i32, Logical> {
        let widest = |width: fn(&Row) -> i32| rows.iter().map(width).max().unwrap_or(0);
        let label = widest(|row| match row {
            Row::Item { label, .. } => *label,
            Row::Separator => 0,
        });
        let accelerator = widest(|row| match row {
            Row::Item { accelerator, .. } => *accelerator,
            Row::Separator => 0,
        });
        let accelerators = if accelerator > 0 {
            ACCELERATOR_GAP + accelerator
        } else {
            0
        };
        let height: i32 = rows.iter().map(|row| row_height(row, text_height)).sum();
        Size::new(
            MARK_COLUMN + label + accelerators + ARROW_COLUMN,
            2 * PADDING + height,
        )
    }

    /// A menu of `rows` with its top left corner at `at`, from the output's corner, or as
    /// near as keeps it on an output of `output` logical pixels, as Marco keeps its menu
    /// on the screen (menu.c:491-493).
    pub(crate) fn at(
        rows: &[Row],
        text_height: i32,
        at: Point<i32, Logical>,
        output: Size<i32, Logical>,
    ) -> Self {
        let size = Self::size(rows, text_height);
        let loc = Point::new(
            at.x.clamp(0, (output.w - size.w).max(0)),
            at.y.clamp(0, (output.h - size.h).max(0)),
        );
        Self::placed(rows, text_height, Rectangle::new(loc, size))
    }

    /// The submenu of `rows` that opens from the row `parent` of the menu `menu`: to the
    /// right of the menu with its first row beside its parent, or to the left where the
    /// output has no room on the right, and kept on the output.
    pub(crate) fn beside(
        rows: &[Row],
        text_height: i32,
        menu: Rectangle<i32, Logical>,
        parent: Rectangle<i32, Logical>,
        output: Size<i32, Logical>,
    ) -> Self {
        let size = Self::size(rows, text_height);
        let right = menu.loc.x + menu.size.w - SUBMENU_OVERLAP;
        let left = menu.loc.x - size.w + SUBMENU_OVERLAP;
        let x = if right + size.w <= output.w || left < 0 {
            right
        } else {
            left
        };
        let loc = Point::new(
            x.clamp(0, (output.w - size.w).max(0)),
            (parent.loc.y - PADDING).clamp(0, (output.h - size.h).max(0)),
        );
        Self::placed(rows, text_height, Rectangle::new(loc, size))
    }

    fn placed(rows: &[Row], text_height: i32, rect: Rectangle<i32, Logical>) -> Self {
        let mut y = rect.loc.y + PADDING;
        let rows = rows
            .iter()
            .map(|row| {
                let height = row_height(row, text_height);
                let row = Rectangle::new((rect.loc.x, y).into(), (rect.size.w, height).into());
                y += height;
                row
            })
            .collect();
        Self { rect, rows }
    }

    /// The row at `point`, from the output's corner.
    pub(crate) fn row_at(&self, point: Point<f64, Logical>) -> Option<usize> {
        self.rows
            .iter()
            .position(|row| row.to_f64().contains(point))
    }

    /// Whether `point`, from the output's corner, is on the menu.
    pub(crate) fn contains(&self, point: Point<f64, Logical>) -> bool {
        self.rect.to_f64().contains(point)
    }
}

fn row_height(row: &Row, text_height: i32) -> i32 {
    match row {
        Row::Separator => SEPARATOR_HEIGHT,
        Row::Item { .. } => text_height + 2 * ROW_PADDING,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUTPUT: (i32, i32) = (1920, 1080);

    fn item(label: i32, accelerator: i32) -> Row {
        Row::Item { label, accelerator }
    }

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
        Rectangle::new((x, y).into(), (w, h).into())
    }

    #[test]
    fn a_menu_is_as_wide_as_its_widest_label_and_keybinding_and_as_high_as_its_rows() {
        let rows = [item(60, 0), item(100, 40), Row::Separator, item(30, 50)];

        let layout = Layout::at(&rows, 16, (200, 300).into(), OUTPUT.into());

        // 24 for the marks, 100, 24 and 50 for the keybindings, 20 for the arrow; three
        // rows of 26, a separator of 7, and 4 above and below.
        assert_eq!(layout.rect, rect(200, 300, 218, 93));
        assert_eq!(
            layout.rows,
            [
                rect(200, 304, 218, 26),
                rect(200, 330, 218, 26),
                rect(200, 356, 218, 7),
                rect(200, 363, 218, 26),
            ]
        );
    }

    #[test]
    fn a_menu_with_no_keybinding_leaves_no_room_for_one() {
        let layout = Layout::at(&[item(60, 0)], 16, (0, 0).into(), OUTPUT.into());
        assert_eq!(layout.rect.size, Size::new(104, 34));
    }

    #[test]
    fn a_menu_asked_for_near_an_edge_is_kept_on_the_output() {
        let rows = [item(100, 0), item(100, 0)];

        let layout = Layout::at(&rows, 16, (1900, 1070).into(), OUTPUT.into());
        assert_eq!(layout.rect, rect(1776, 1020, 144, 60));
        assert_eq!(layout.rows[0], rect(1776, 1024, 144, 26));

        let layout = Layout::at(&rows, 16, (-30, -5).into(), OUTPUT.into());
        assert_eq!(layout.rect.loc, Point::new(0, 0));
    }

    #[test]
    fn the_row_under_a_point_is_found_and_the_padding_is_no_row() {
        let rows = [item(100, 0), Row::Separator, item(100, 0)];
        let layout = Layout::at(&rows, 16, (200, 300).into(), OUTPUT.into());

        assert_eq!(layout.row_at((210., 304.).into()), Some(0));
        assert_eq!(layout.row_at((210., 331.).into()), Some(1));
        assert_eq!(layout.row_at((210., 340.).into()), Some(2));
        assert_eq!(layout.row_at((210., 301.).into()), None);
        assert!(layout.contains((210., 301.).into()));
        assert!(!layout.contains((199., 310.).into()));
        assert_eq!(layout.row_at((199., 310.).into()), None);
    }

    #[test]
    fn a_submenu_opens_to_the_right_with_its_first_row_beside_its_parent() {
        let menu = rect(200, 300, 218, 93);
        let parent = rect(200, 363, 218, 26);

        let layout = Layout::beside(&[item(80, 0), item(80, 0)], 16, menu, parent, OUTPUT.into());

        assert_eq!(layout.rect, rect(416, 359, 124, 60));
        assert_eq!(layout.rows[0].loc.y, parent.loc.y);
    }

    #[test]
    fn a_submenu_with_no_room_on_the_right_opens_to_the_left() {
        let menu = rect(1700, 300, 218, 93);
        let parent = rect(1700, 363, 218, 26);

        let layout = Layout::beside(&[item(80, 0)], 16, menu, parent, OUTPUT.into());

        assert_eq!(layout.rect.loc, Point::new(1578, 359));
    }

    #[test]
    fn a_submenu_near_the_bottom_is_kept_on_the_output() {
        let menu = rect(200, 980, 218, 93);
        let parent = rect(200, 1043, 218, 26);
        let rows = [item(80, 0), item(80, 0), item(80, 0)];

        let layout = Layout::beside(&rows, 16, menu, parent, OUTPUT.into());

        assert_eq!(layout.rect, rect(416, 994, 124, 86));
    }
}
