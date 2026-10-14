//! Where the Overview puts each window and its label (241). Pure geometry, of the windows'
//! rectangles and the area alone: no Wayland and no drawing, as the Window switcher's is.

use smithay::utils::{Logical, Point, Rectangle, Size};

/// One window's place in the layout. Both rectangles are in the area's coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Slot {
    /// Where the window is drawn, at the layout's one factor of its size.
    pub window: Rectangle<i32, Logical>,
    /// Where its icon and its title are drawn, beneath it and as wide.
    pub label: Rectangle<i32, Logical>,
}

/// The Overview's windows laid out in an area: rows of windows, each with its aspect and
/// its size relative to the others, none enlarged and none over another.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Layout {
    /// Each window's slot, in the order the windows were given in.
    pub slots: Vec<Slot>,
}

/// The room left free along each edge of the area.
const PADDING: i32 = 48;
/// The gap between two windows of a row, and between a row's labels and the next row.
const GAP: i32 = 32;
/// The gap between a window and its label.
const LABEL_GAP: i32 = 8;
/// A label's height.
pub(crate) const LABEL_HEIGHT: i32 = 28;

/// The room a layout leaves around and between the windows.
#[derive(Clone, Copy)]
struct Spacing {
    padding: i32,
    gap: i32,
    /// What a row has beneath its windows: the gap and the label.
    beneath: i32,
}

impl Layout {
    /// Lays windows with these rectangles out in `area`.
    ///
    /// The rows follow the windows' real places: the windows are split into rows from the
    /// top one down, by their centres, and each row is in order from the left. Of every
    /// number of rows, it's the one that leaves the windows largest, and the fewest rows
    /// of those. Every window is drawn at one factor of its size, which is 1 at the most.
    /// A row's windows stand on one line, so that their labels are in one line too.
    ///
    /// Where the area has no room for the spacing, the windows are laid out with none, and
    /// with no labels.
    pub(crate) fn new(windows: &[Rectangle<i32, Logical>], area: Rectangle<i32, Logical>) -> Self {
        if windows.is_empty() {
            return Self::default();
        }
        let sizes: Vec<Size<i32, Logical>> = windows
            .iter()
            .map(|window| Size::from((window.size.w.max(1), window.size.h.max(1))))
            .collect();
        let roomy = Spacing {
            padding: PADDING,
            gap: GAP,
            beneath: LABEL_GAP + LABEL_HEIGHT,
        };
        let tight = Spacing {
            padding: 0,
            gap: 0,
            beneath: 0,
        };
        let (spacing, rows, factor) = [roomy, tight]
            .into_iter()
            .find_map(|spacing| {
                let (rows, factor) = best_rows(windows, &sizes, area.size, spacing);
                (factor > 0.0).then_some((spacing, rows, factor))
            })
            // An area with no room at all: each window is one pixel.
            .unwrap_or_else(|| (tight, rows_of(windows, windows.len()), 0.0));

        let scaled = |index: usize| {
            let size = sizes[index];
            Size::<i32, Logical>::from((
                ((f64::from(size.w) * factor).floor() as i32).max(1),
                ((f64::from(size.h) * factor).floor() as i32).max(1),
            ))
        };
        let row_height = |row: &Vec<usize>| row.iter().map(|i| scaled(*i).h).max().unwrap_or(0);
        let rows_count = to_i32(rows.len());
        let height: i32 = rows.iter().map(row_height).sum::<i32>()
            + rows_count * spacing.beneath
            + (rows_count - 1) * spacing.gap;
        let mut slots = vec![
            Slot {
                window: Rectangle::default(),
                label: Rectangle::default(),
            };
            windows.len()
        ];
        let mut top = area.loc.y + (area.size.h - height) / 2;
        for row in &rows {
            let band = row_height(row);
            let width: i32 = row.iter().map(|i| scaled(*i).w).sum::<i32>()
                + (to_i32(row.len()) - 1) * spacing.gap;
            let mut left = area.loc.x + (area.size.w - width) / 2;
            for &index in row {
                let size = scaled(index);
                let window = Rectangle::new(Point::from((left, top + band - size.h)), size);
                let label_height = (spacing.beneath - LABEL_GAP).max(0);
                let label = Rectangle::new(
                    Point::from((left, top + band + spacing.beneath - label_height)),
                    Size::from((size.w, label_height)),
                );
                slots[index] = Slot { window, label };
                left += size.w + spacing.gap;
            }
            top += band + spacing.beneath + spacing.gap;
        }
        Self { slots }
    }

    /// The window whose slot `point` is in, on the window or on its label.
    pub(crate) fn slot_at(&self, point: Point<f64, Logical>) -> Option<usize> {
        self.slots.iter().position(|slot| {
            slot.window.to_f64().contains(point) || slot.label.to_f64().contains(point)
        })
    }
}

/// The rows that leave the windows largest in an area of `area`, of every number of rows,
/// with the factor they're drawn at. The factor is 0 or less if no number of rows fits.
fn best_rows(
    windows: &[Rectangle<i32, Logical>],
    sizes: &[Size<i32, Logical>],
    area: Size<i32, Logical>,
    spacing: Spacing,
) -> (Vec<Vec<usize>>, f64) {
    let mut best: Option<(Vec<Vec<usize>>, f64)> = None;
    for count in 1..=windows.len() {
        let rows = rows_of(windows, count);
        let factor = factor_of(&rows, sizes, area, spacing);
        if best.as_ref().is_none_or(|(_, largest)| factor > *largest) {
            best = Some((rows, factor));
        }
    }
    best.expect("a layout has a window")
}

/// The windows in `count` rows, as their places in `windows`: split from the top one down
/// by their centres, as evenly as they go with the fuller rows first, and each row from
/// the left. Windows at one place keep the order they were given in.
fn rows_of(windows: &[Rectangle<i32, Logical>], count: usize) -> Vec<Vec<usize>> {
    // Twice the centre, which is whole.
    let centre = |index: usize| {
        let window = windows[index];
        (
            2 * window.loc.x + window.size.w,
            2 * window.loc.y + window.size.h,
        )
    };
    let mut order: Vec<usize> = (0..windows.len()).collect();
    order.sort_by_key(|&index| (centre(index).1, centre(index).0, index));
    let (each, more) = (order.len() / count, order.len() % count);
    let mut rows = Vec::with_capacity(count);
    let mut rest = order.as_slice();
    for row in 0..count {
        let (taken, left) = rest.split_at(each + usize::from(row < more));
        let mut taken = taken.to_vec();
        taken.sort_by_key(|&index| (centre(index).0, index));
        rows.push(taken);
        rest = left;
    }
    rows
}

/// The largest factor, 1 at the most, that `rows` fit an area of `area` at: each row
/// across it, and the rows down it.
fn factor_of(
    rows: &[Vec<usize>],
    sizes: &[Size<i32, Logical>],
    area: Size<i32, Logical>,
    spacing: Spacing,
) -> f64 {
    let count = to_i32(rows.len());
    let across = f64::from(area.w - 2 * spacing.padding);
    let down = f64::from(
        area.h - 2 * spacing.padding - count * spacing.beneath - (count - 1) * spacing.gap,
    );
    let mut factor: f64 = 1.0;
    let mut heights = 0.0;
    for row in rows {
        let width: f64 = row.iter().map(|i| f64::from(sizes[*i].w)).sum();
        let gaps = f64::from((to_i32(row.len()) - 1) * spacing.gap);
        factor = factor.min((across - gaps) / width);
        heights += f64::from(row.iter().map(|i| sizes[*i].h).max().unwrap_or(1));
    }
    factor.min(down / heights)
}

fn to_i32(n: usize) -> i32 {
    i32::try_from(n).expect("a window count fits an i32")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_hd() -> Rectangle<i32, Logical> {
        Rectangle::from_size(Size::from((1920, 1080)))
    }

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
        Rectangle::new(Point::from((x, y)), Size::from((w, h)))
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

    fn centre_of(r: Rectangle<i32, Logical>) -> Point<f64, Logical> {
        Point::from((
            f64::from(r.loc.x) + f64::from(r.size.w) / 2.0,
            f64::from(r.loc.y) + f64::from(r.size.h) / 2.0,
        ))
    }

    /// The top of each row of `layout`, from the top one down.
    fn row_lines(layout: &Layout) -> Vec<i32> {
        let mut lines: Vec<i32> = layout.slots.iter().map(|slot| slot.label.loc.y).collect();
        lines.sort_unstable();
        lines.dedup();
        lines
    }

    /// Every window and label of `layout` is in `area`, and none is over another.
    fn assert_apart_and_inside(layout: &Layout, area: Rectangle<i32, Logical>, what: &str) {
        let parts: Vec<Rectangle<i32, Logical>> = layout
            .slots
            .iter()
            .flat_map(|slot| [slot.window, slot.label])
            .filter(|part| !part.is_empty())
            .collect();
        for (i, a) in parts.iter().enumerate() {
            assert!(contains(area, *a), "{what}: {a:?} is outside {area:?}");
            for b in &parts[i + 1..] {
                assert!(!a.overlaps(*b), "{what}: {a:?} is over {b:?}");
            }
        }
    }

    #[test]
    fn no_windows_make_an_empty_layout() {
        assert_eq!(Layout::new(&[], full_hd()).slots, []);
    }

    #[test]
    fn one_window_that_fits_keeps_its_size_in_the_middle() {
        let layout = Layout::new(&[rect(100, 50, 800, 600)], full_hd());

        let slot = layout.slots[0];
        assert_eq!(slot.window.size, Size::from((800, 600)));
        assert_eq!(slot.window.loc.x, (1920 - 800) / 2);
        // The window and its label are in the middle together.
        let (above, below) = (slot.window.loc.y, 1080 - bottom(slot.label));
        assert!((above - below).abs() <= 1, "{above} above, {below} below");
        assert_eq!(slot.label.loc.x, slot.window.loc.x);
        assert_eq!(slot.label.size, Size::from((800, LABEL_HEIGHT)));
        assert_eq!(slot.label.loc.y, bottom(slot.window) + LABEL_GAP);
    }

    #[test]
    fn one_window_as_large_as_the_area_is_shrunk_with_its_aspect() {
        let layout = Layout::new(&[rect(0, 0, 1920, 1080)], full_hd());

        let window = layout.slots[0].window;
        assert!(window.size.w < 1920 && window.size.h < 1080);
        let aspect = f64::from(window.size.w) / f64::from(window.size.h);
        assert!((aspect - 16.0 / 9.0).abs() < 0.01, "{aspect}");
        assert_apart_and_inside(&layout, full_hd(), "one window");
    }

    #[test]
    fn windows_of_different_sizes_keep_their_sizes_relative_to_each_other() {
        let windows = [
            rect(0, 0, 1600, 900),
            rect(200, 100, 800, 600),
            rect(900, 300, 400, 300),
        ];
        let layout = Layout::new(&windows, full_hd());

        let factor = f64::from(layout.slots[0].window.size.w) / 1600.0;
        assert!(factor < 1.0);
        for (window, slot) in windows.iter().zip(&layout.slots) {
            let (w, h) = (f64::from(window.size.w), f64::from(window.size.h));
            let drawn = slot.window.size;
            assert!((f64::from(drawn.w) - w * factor).abs() <= 1.0, "{drawn:?}");
            assert!((f64::from(drawn.h) - h * factor).abs() <= 1.0, "{drawn:?}");
        }
        assert_apart_and_inside(&layout, full_hd(), "three sizes");
    }

    #[test]
    fn no_window_is_enlarged() {
        let windows = [rect(10, 10, 200, 100), rect(700, 500, 120, 300)];
        let layout = Layout::new(&windows, full_hd());

        for (window, slot) in windows.iter().zip(&layout.slots) {
            assert_eq!(slot.window.size, window.size);
        }
    }

    #[test]
    fn many_windows_take_more_than_one_row() {
        let windows = [rect(0, 0, 1920, 1080); 6];
        let layout = Layout::new(&windows, full_hd());

        assert_eq!(row_lines(&layout).len(), 2);
        // Two rows of three leave them larger than one of six or three of two.
        assert!(layout.slots[0].window.size.w > 500, "{:?}", layout.slots[0]);
        assert_apart_and_inside(&layout, full_hd(), "six maximised windows");
    }

    #[test]
    fn no_window_is_over_another_however_many_there_are() {
        for count in [1, 2, 3, 5, 8, 13, 30] {
            let windows: Vec<_> = (0..count)
                .map(|i| rect(37 * i, 23 * i, 300 + 90 * (i % 4), 200 + 70 * (i % 3)))
                .collect();
            let layout = Layout::new(&windows, full_hd());

            assert_eq!(layout.slots.len(), windows.len());
            assert_apart_and_inside(&layout, full_hd(), &format!("{count} windows"));
        }
    }

    #[test]
    fn a_window_on_the_left_of_the_output_is_on_the_left_of_the_layout() {
        // Given from the right to the left.
        let windows = [
            rect(1300, 200, 500, 400),
            rect(700, 220, 500, 400),
            rect(50, 180, 500, 400),
        ];
        let layout = Layout::new(&windows, full_hd());

        let x = |index: usize| layout.slots[index].window.loc.x;
        assert!(x(2) < x(1) && x(1) < x(0), "{:?}", layout.slots);
        assert_eq!(row_lines(&layout).len(), 1);
    }

    #[test]
    fn the_upper_windows_are_in_the_upper_row() {
        // A two by two tiling, given in no order of place.
        let windows = [
            rect(960, 540, 960, 540),
            rect(0, 0, 960, 540),
            rect(0, 540, 960, 540),
            rect(960, 0, 960, 540),
        ];
        let layout = Layout::new(&windows, full_hd());

        let at = |index: usize| layout.slots[index].window.loc;
        assert_eq!(row_lines(&layout).len(), 2);
        assert!(at(1).y < at(2).y && at(3).y < at(0).y);
        assert_eq!(at(1).y, at(3).y);
        assert!(at(1).x < at(3).x && at(2).x < at(0).x);
    }

    #[test]
    fn a_rows_windows_stand_on_one_line_with_their_labels_in_one_line() {
        let windows = [rect(0, 0, 400, 300), rect(500, 0, 400, 600)];
        let layout = Layout::new(&windows, full_hd());

        let (short, tall) = (layout.slots[0], layout.slots[1]);
        assert_eq!(bottom(short.window), bottom(tall.window));
        assert_eq!(short.label.loc.y, tall.label.loc.y);
    }

    #[test]
    fn the_layout_is_in_the_areas_coordinates() {
        let area = rect(1920, 100, 1280, 800);
        let windows = [rect(2000, 200, 600, 400), rect(2700, 300, 300, 300)];
        let layout = Layout::new(&windows, area);

        assert_apart_and_inside(&layout, area, "an area away from the origin");
    }

    #[test]
    fn an_area_with_no_room_for_the_spacing_lays_the_windows_out_with_none() {
        let area = rect(0, 0, 60, 40);
        let windows = [rect(0, 0, 800, 600), rect(900, 0, 800, 600)];
        let layout = Layout::new(&windows, area);

        for slot in &layout.slots {
            assert!(slot.window.size.w >= 1 && slot.window.size.h >= 1);
            assert!(contains(area, slot.window), "{slot:?}");
            assert!(slot.label.is_empty());
        }
        assert!(!layout.slots[0].window.overlaps(layout.slots[1].window));
    }

    #[test]
    fn a_point_is_in_the_slot_of_the_window_or_the_label_it_is_on() {
        let windows = [rect(0, 0, 500, 400), rect(600, 0, 500, 400)];
        let layout = Layout::new(&windows, full_hd());

        for (index, slot) in layout.slots.iter().enumerate() {
            assert_eq!(layout.slot_at(centre_of(slot.window)), Some(index));
            assert_eq!(layout.slot_at(centre_of(slot.label)), Some(index));
        }
        let between = Point::from((
            f64::from(right(layout.slots[0].window)) + 1.0,
            centre_of(layout.slots[0].window).y,
        ));
        assert_eq!(layout.slot_at(between), None);
        assert_eq!(layout.slot_at(Point::from((0.0, 0.0))), None);
    }

    /// A rectangle as `x,y WxH`.
    fn show(r: Rectangle<i32, Logical>) -> String {
        format!("{},{} {}x{}", r.loc.x, r.loc.y, r.size.w, r.size.h)
    }

    #[test]
    fn five_windows_in_rectangles() {
        let windows = [
            rect(0, 24, 960, 1000),
            rect(960, 24, 960, 500),
            rect(960, 524, 960, 500),
            rect(300, 200, 640, 480),
            rect(1200, 100, 400, 300),
        ];
        let layout = Layout::new(&windows, full_hd());
        let mut text = String::new();
        for (index, slot) in layout.slots.iter().enumerate() {
            text.push_str(&format!(
                "window {index} {} label {}\n",
                show(slot.window),
                show(slot.label)
            ));
        }
        insta::assert_snapshot!(text, @"
        window 0 381,409 563x586 label 381,1003 563x28
        window 1 1015,48 563x293 label 1015,349 563x28
        window 2 976,702 563x293 label 976,1003 563x28
        window 3 342,60 375x281 label 342,349 375x28
        window 4 749,165 234x176 label 749,349 234x28
        ");
    }
}
