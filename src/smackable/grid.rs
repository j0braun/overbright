use super::*;

/// The sweep's broad phase (no native counterpart: the original
/// tests every prop): a uniform x/y grid of the props' rest boxes. A query
/// returns a superset of the props whose rest box meets the query box, so
/// the exact box test that follows sees the same props in the same
/// (ascending) order as a scan of all of them.
#[derive(Debug)]
pub(super) struct PropGrid {
    /// The list it was built from (address, length).
    pub(super) of: (usize, usize),
    pub(super) origin: Vec2,
    pub(super) w: i32,
    pub(super) h: i32,
    /// `None`: no grid (a box not finite, or too many cells); scan all.
    pub(super) cells: Option<Vec<Vec<u32>>>,
}

/// The grid's cell edge, metres.
pub(super) const GRID_CELL: f32 = 32.0;
/// Past this many cells the props scan without a grid.
pub(super) const GRID_MAX_CELLS: i64 = 1 << 20;

impl PropGrid {
    pub(super) fn key(props: &[Prop]) -> (usize, usize) {
        (props.as_ptr() as usize, props.len())
    }

    pub(super) fn new(props: &[Prop]) -> PropGrid {
        let boxes: Vec<(Vec2, Vec2)> = props.iter().map(|p| (p.world_min.truncate(), p.world_max.truncate())).collect();
        PropGrid::of_boxes(PropGrid::key(props), &boxes)
    }

    pub(super) fn of_boxes(of: (usize, usize), boxes: &[(Vec2, Vec2)]) -> PropGrid {
        let none = PropGrid { of, origin: Vec2::ZERO, w: 0, h: 0, cells: None };
        if boxes.is_empty() || boxes.iter().any(|(a, b)| !a.is_finite() || !b.is_finite()) {
            return none;
        }
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for &(a, b) in boxes {
            lo = lo.min(a);
            hi = hi.max(b);
        }
        let span = (hi - lo) / GRID_CELL;
        if !(span.x < 1e6 && span.y < 1e6) {
            return none;
        }
        let (w, h) = (span.x as i32 + 1, span.y as i32 + 1);
        if i64::from(w) * i64::from(h) > GRID_MAX_CELLS {
            return none;
        }
        let mut cells = vec![Vec::new(); (w * h) as usize];
        for (k, &(a, b)) in boxes.iter().enumerate() {
            let (x0, y0, x1, y1) = PropGrid::range(lo, w, h, a, b);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    cells[(y * w + x) as usize].push(k as u32);
                }
            }
        }
        PropGrid { of, origin: lo, w, h, cells: Some(cells) }
    }

    /// The cells lo..hi covers, clamped to the grid. The cell of a
    /// coordinate is monotonic in it (rounded subtraction and division
    /// are), so every point both of a prop's box and of a query box lies
    /// in a cell both ranges hold.
    fn range(origin: Vec2, w: i32, h: i32, lo: Vec2, hi: Vec2) -> (i32, i32, i32, i32) {
        let cell =
            |v: f32, o: f32, n: i32| (((v - o) / GRID_CELL).floor().clamp(-1.0, n as f32) as i32).clamp(0, n - 1);
        (cell(lo.x, origin.x, w), cell(lo.y, origin.y, h), cell(hi.x, origin.x, w), cell(hi.y, origin.y, h))
    }

    /// Appends the props whose rest box may meet lo..hi (a superset, with
    /// repeats); false when the caller must scan them all.
    pub(super) fn query(&self, lo: Vec2, hi: Vec2, out: &mut Vec<usize>) -> bool {
        let Some(cells) = &self.cells else { return false };
        if !(lo.is_finite() && hi.is_finite()) {
            return false;
        }
        let (x0, y0, x1, y1) = PropGrid::range(self.origin, self.w, self.h, lo, hi);
        for y in y0..=y1 {
            for x in x0..=x1 {
                out.extend(cells[(y * self.w + x) as usize].iter().map(|&k| k as usize));
            }
        }
        true
    }
}
