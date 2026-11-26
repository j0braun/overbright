use crate::draw::quad::Quad;

/// Scale the HUD's quads by `scale` (1: untouched). The quads are grouped
/// into HUD parts (quads whose rectangles touch or overlap, within 4
/// canvas pixels, belong to one part: a gauge with its needle and digits, a
/// panel with its text). The canvas is cut in thirds each way; each part
/// keeps the corner, edge midpoint or centre of the third its bounding
/// box's centre is in, and scales toward it, so the gauges stay in their
/// corners and the race info at the top. Quads spanning most of the canvas
/// (flashes, fades) stay as they are. **Open**: parts that do not touch but
/// belong together on screen scale apart a little.
pub fn scale_hud(quads: &mut [Quad], scale: f32) {
    if scale == 1.0 || quads.is_empty() {
        return;
    }
    scale_hud_by(quads, |_, _| scale);
}

/// [`scale_hud`] with each part's scale from its bounding box (canvas
/// pixels, low and high corner): the per-element HUD scale
/// (`extras::hud_part_scale`).
pub fn scale_hud_by(quads: &mut [Quad], scale_of: impl Fn(glam::Vec2, glam::Vec2) -> f32) {
    const W: f32 = 640.0;
    const H: f32 = 480.0;
    const MARGIN: f32 = 4.0;
    let bounds = |q: &Quad| (q.rect[0].min(q.rect[1]), q.rect[0].max(q.rect[1]));
    let full = |q: &Quad| {
        let (lo, hi) = bounds(q);
        hi.x - lo.x >= W * 0.9 || hi.y - lo.y >= H * 0.9
    };
    // Union-find over the quads that are not full-canvas.
    let n = quads.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    let boxes: Vec<_> = quads.iter().map(bounds).collect();
    let skip: Vec<bool> = quads.iter().map(full).collect();
    for i in 0..n {
        if skip[i] {
            continue;
        }
        for j in i + 1..n {
            if skip[j] {
                continue;
            }
            let (a, b) = (boxes[i], boxes[j]);
            if quads[i].place == quads[j].place
                && a.0.x <= b.1.x + MARGIN
                && b.0.x <= a.1.x + MARGIN
                && a.0.y <= b.1.y + MARGIN
                && b.0.y <= a.1.y + MARGIN
            {
                let (ri, rj) = (root(&mut parent, i), root(&mut parent, j));
                if ri != rj {
                    parent[rj] = ri;
                }
            }
        }
    }
    // Each part's bounding box.
    let mut part: std::collections::HashMap<usize, (glam::Vec2, glam::Vec2)> = std::collections::HashMap::new();
    for i in (0..n).filter(|&i| !skip[i]) {
        let r = root(&mut parent, i);
        let e = part.entry(r).or_insert(boxes[i]);
        *e = (e.0.min(boxes[i].0), e.1.max(boxes[i].1));
    }
    // Each part scales about the point its anchor keeps (`display::Place`,
    // the same model that moves it to its screen edge).
    for i in (0..n).filter(|&i| !skip[i]) {
        let (lo, hi) = part[&root(&mut parent, i)];
        let scale = scale_of(lo, hi);
        if scale == 1.0 {
            continue;
        }
        let a = quads[i].place.origin(lo, hi);
        for p in &mut quads[i].rect {
            *p = a + (*p - a) * scale;
        }
    }
}
