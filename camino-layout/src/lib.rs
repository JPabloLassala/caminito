//! Keeping a layout contiguous (82, ADR 0014): docking an output against the others, and
//! healing a layout after a change that broke it.
//!
//! Camino and camino-displays both use it, so it names no protocol and no toolkit, and has
//! no dependencies. Everything is in logical pixels. An output here is only its rectangle,
//! and a layout is a slice of them, in the caller's order.
//!
//! Ported from the `Layout` module of the camino-displays prototype
//! (`.scratch/camino-displays-prototype/index.html` at 8a4981d).

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests;

/// How much edge two touching outputs must share, in logical pixels. Outputs shorter than
/// this along the edge must share the whole of the shorter one.
pub const MIN_SHARED_EDGE: i32 = 100;

/// An output's place and size in a layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    fn right(&self) -> i32 {
        self.x + self.w
    }

    fn bottom(&self) -> i32 {
        self.y + self.h
    }

    fn overlaps(&self, other: &Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    /// Mirrored in the diagonal, so that what is above is to the left.
    fn transposed(self) -> Rect {
        Rect {
            x: self.y,
            y: self.x,
            w: self.h,
            h: self.w,
        }
    }

    /// Whether the two lie side by side, or one above the other, along enough edge.
    fn joins(&self, other: &Rect) -> bool {
        let (shared, shorter) = if self.right() == other.x || other.right() == self.x {
            let shared = self.bottom().min(other.bottom()) - self.y.max(other.y);
            (shared, self.h.min(other.h))
        } else if self.bottom() == other.y || other.bottom() == self.y {
            let shared = self.right().min(other.right()) - self.x.max(other.x);
            (shared, self.w.min(other.w))
        } else {
            return false;
        };
        shared > 0 && shared >= MIN_SHARED_EDGE.min(shorter)
    }

    /// The square of the distance between the two's top-left corners.
    fn distance_squared(&self, other: &Rect) -> i64 {
        let dx = i64::from(self.x) - i64::from(other.x);
        let dy = i64::from(self.y) - i64::from(other.y);
        dx * dx + dy * dy
    }

    /// The square of the gap between the two, 0 if they touch or overlap.
    fn gap_squared(&self, other: &Rect) -> i64 {
        let dx = i64::from((self.x - other.right()).max(other.x - self.right()).max(0));
        let dy = i64::from(
            (self.y - other.bottom())
                .max(other.y - self.bottom())
                .max(0),
        );
        dx * dx + dy * dy
    }
}

/// Whether a layout is contiguous: no two outputs overlap, and each can be reached from
/// every other through outputs that share enough edge.
pub fn is_contiguous(outputs: &[Rect]) -> bool {
    let overlap = outputs
        .iter()
        .enumerate()
        .any(|(i, a)| outputs[i + 1..].iter().any(|b| a.overlaps(b)));
    if overlap {
        return false;
    }
    let mut reached: Vec<&Rect> = outputs.iter().take(1).collect();
    let mut rest: Vec<&Rect> = outputs.iter().skip(1).collect();
    while let Some(next) = rest
        .iter()
        .position(|output| reached.iter().any(|r| r.joins(output)))
    {
        reached.push(rest.swap_remove(next));
    }
    rest.is_empty()
}

/// The place nearest to `output`'s where it lies against the outline of `others`: sharing
/// enough edge with one of them, and on top of none. With no others, it stays where it is.
///
/// Within `snap` logical pixels of where its edge lines up with the matching edge of an
/// output it would lie against (top with top, bottom with bottom, left with left, right
/// with right), it takes that place. Outputs it wouldn't touch don't pull, and centres
/// don't. A `snap` of 0 turns that off.
pub fn dock(output: Rect, others: &[Rect], snap: i32) -> Rect {
    // Above and below are left and right of the layout mirrored in its diagonal.
    let mirrored: Vec<Rect> = others.iter().map(|other| other.transposed()).collect();
    let beside = places_beside(output, others, snap);
    let above_or_below = places_beside(output.transposed(), &mirrored, snap)
        .into_iter()
        .map(Rect::transposed);
    beside
        .into_iter()
        .chain(above_or_below)
        .min_by_key(|place| place.distance_squared(&output))
        .unwrap_or(output)
}

/// For each side, left and right, of each of `others`, the place nearest to `output`'s
/// where it lies against that side, if it has room there.
fn places_beside(output: Rect, others: &[Rect], snap: i32) -> Vec<Rect> {
    let mut places = Vec::new();
    for against in others {
        for x in [against.x - output.w, against.right()] {
            // Whether an output stands in the column `output` would take at `x`.
            let in_column = |other: &&Rect| x < other.right() && other.x < x + output.w;
            let shared = MIN_SHARED_EDGE.min(output.h).min(against.h);
            // Where its top may be: sharing enough edge with `against`, and clear of the
            // outputs in its column.
            let mut free = vec![(against.y - output.h + shared, against.bottom() - shared)];
            for other in others.iter().filter(in_column) {
                free = without(&free, other.y - output.h, other.bottom());
            }
            let nearest_to_wanted = |&y: &i32| y.abs_diff(output.y);
            let clamped = free.iter().map(|&(from, to)| output.y.clamp(from, to));
            let Some(mut y) = clamped.min_by_key(nearest_to_wanted) else {
                continue;
            };

            // Only the outputs it would lie against, on either side of its column, pull.
            let touched = |other: &&Rect| other.right() == x || other.x == x + output.w;
            let snapped = others
                .iter()
                .filter(touched)
                .flat_map(|other| [other.y, other.bottom() - output.h])
                .filter(|&to| to.abs_diff(output.y) <= snap.unsigned_abs())
                .filter(|&to| free.iter().any(|&(from, until)| from <= to && to <= until))
                .min_by_key(nearest_to_wanted);
            if snap > 0 {
                y = snapped.unwrap_or(y);
            }
            places.push(Rect { x, y, ..output });
        }
    }
    places
}

/// The closed `intervals` without what lies strictly between `from` and `to`.
fn without(intervals: &[(i32, i32)], from: i32, to: i32) -> Vec<(i32, i32)> {
    let mut left = Vec::new();
    for &(start, end) in intervals {
        if end <= from || start >= to {
            left.push((start, end));
            continue;
        }
        if start <= from {
            left.push((start, from));
        }
        if end >= to {
            left.push((to, end));
        }
    }
    left
}

/// Makes a layout contiguous, moving each output as little as it can. The output at
/// `anchor` stays where it is. A contiguous layout comes back unchanged.
///
/// # Panics
///
/// If `anchor` isn't one of a layout that has outputs.
pub fn heal(outputs: &[Rect], anchor: usize) -> Vec<Rect> {
    if outputs.is_empty() {
        return Vec::new();
    }
    let mut placed = vec![outputs[anchor]];
    let mut rest: Vec<usize> = (0..outputs.len()).filter(|&i| i != anchor).collect();
    let mut healed = outputs.to_vec();
    while !rest.is_empty() {
        // An output that already lies against the placed ones stays. Else the nearest to
        // them is docked, which leaves the fewest to move after it.
        let fits = |&i: &usize| {
            placed.iter().any(|p| p.joins(&healed[i]))
                && !placed.iter().any(|p| p.overlaps(&healed[i]))
        };
        let gap = |&i: &usize| placed.iter().map(|p| p.gap_squared(&healed[i])).min();
        let next = match rest.iter().position(fits) {
            Some(next) => next,
            None => {
                let nearest = (0..rest.len()).min_by_key(|&k| gap(&rest[k]));
                let nearest = nearest.expect("there is an output left to place");
                healed[rest[nearest]] = dock(healed[rest[nearest]], &placed, 0);
                nearest
            }
        };
        placed.push(healed[rest.remove(next)]);
    }
    healed
}

/// The layout after the output at `index` changes to `w` by `h`: those to its right and
/// below keep their side of it. It may need healing.
pub fn resized(outputs: &[Rect], index: usize, w: i32, h: i32) -> Vec<Rect> {
    let old = outputs[index];
    let mut resized = outputs.to_vec();
    for output in &mut resized {
        if output.x >= old.right() {
            output.x += w - old.w;
        }
        if output.y >= old.bottom() {
            output.y += h - old.h;
        }
    }
    resized[index] = Rect { w, h, ..old };
    resized
}

/// The layout moved so that its top-left corner is at 0,0.
pub fn at_origin(outputs: &[Rect]) -> Vec<Rect> {
    let left = outputs.iter().map(|o| o.x).min().unwrap_or(0);
    let top = outputs.iter().map(|o| o.y).min().unwrap_or(0);
    let moved = |o: &Rect| Rect {
        x: o.x - left,
        y: o.y - top,
        ..*o
    };
    outputs.iter().map(moved).collect()
}

/// Where a new output of `w` by `h` goes: to the right of the rightmost, with its top at
/// the layout's top, or at 0,0 as the first. It may need healing.
pub fn at_the_right(w: i32, h: i32, others: &[Rect]) -> Rect {
    Rect {
        x: others.iter().map(Rect::right).max().unwrap_or(0),
        y: others.iter().map(|o| o.y).min().unwrap_or(0),
        w,
        h,
    }
}
