//! Enhanced Glass's geometry: glass kinds, draw order, boxes and rects (moved from car_glass.rs).

use super::*;

/// Glass kind by light material name: 1 window, 2 lamp cover, 0 not glass.
pub(crate) fn kind(material: &str) -> u8 {
    match material {
        "WINDSHIELD" | "WINDOWMASK" => 1,
        m if m.starts_with("WINDSHIELD_TINT") => 1,
        "CLEARPLASTIC" | "HEADLIGHTGLASS" => 2,
        _ => 0,
    }
}

/// Marks the blended draws car.efx draws over the glass (`draws` in the
/// car pass's order): the blended draws after the car's first glass draw (a
/// decal on a window: the NFSPEED rear sticker, the lamps' sprites) and a
/// glass draw laid on an earlier glass draw of the same part (a second layer
/// of one pane: WINDOWMASK's band over the WINDSHIELD_TINT, centres within
/// [`LAYER_GAP`]). With Enhanced Glass they are drawn after the glass
/// composite with the car's own pipelines, in their native order, so they
/// stay over the pane as car.efx blends them (the composite writes the pane
/// opaque: a layer composited over another would erase it).
pub(crate) fn mark_after_glass(draws: &mut [Draw]) {
    let Some(first) = draws.iter().position(|d| d.glass != 0) else { return };
    for i in first + 1..draws.len() {
        let d = &draws[i];
        let layer = d.glass != 0
            && draws[first..i]
                .iter()
                .any(|e| e.glass != 0 && !e.glass_after && e.part == d.part && e.centre.distance(d.centre) < LAYER_GAP);
        draws[i].glass_after = layer || (draws[i].glass == 0 && draws[i].blend != Blend::None);
    }
}

/// Two glass draws of one part whose centres are this close (m, model
/// space) are layers of one pane (the Lancer's rear tint and mask: 0.011 m).
const LAYER_GAP: f32 = 0.05;

/// Grows `b` (min, max) by a draw's indexed vertices (model space).
pub(crate) fn extend_box(
    b: &mut (Vec3, Vec3),
    vertices: &[crate::formats::solid::Vertex],
    indices: &[u16],
    first: u32,
    count: u32,
) {
    let range = indices.get(first as usize..(first + count) as usize).unwrap_or(&[]);
    for v in range.iter().filter_map(|&i| vertices.get(i as usize)) {
        let p = Vec3::from(v.position);
        b.0 = b.0.min(p);
        b.1 = b.1.max(p);
    }
}

/// The screen rect (NDC x0, y0, x1, y1) of the glass box (min, max),
/// grown by `pad` (m) and placed by `clip` (view_proj · world); None when a
/// corner is at or behind the eye (the whole view is copied) or there is no
/// glass; an empty rect when it is all off screen.
pub fn glass_rect(b: (Vec3, Vec3), pad: f32, clip: Mat4) -> Option<[f32; 4]> {
    if b.0.x > b.1.x {
        return None;
    }
    let (lo, hi) = (b.0 - Vec3::splat(pad), b.1 + Vec3::splat(pad));
    let mut r = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for k in 0..8 {
        let p = Vec3::new(
            if k & 1 == 0 { lo.x } else { hi.x },
            if k & 2 == 0 { lo.y } else { hi.y },
            if k & 4 == 0 { lo.z } else { hi.z },
        );
        let h = clip * p.extend(1.0);
        if h.w <= 1e-3 {
            return None;
        }
        let (x, y) = (h.x / h.w, h.y / h.w);
        r = [r[0].min(x), r[1].min(y), r[2].max(x), r[3].max(y)];
    }
    Some(r)
}

/// The scene copy's pixels (x, y, w, h) on a `size` target for the
/// cars' glass rects (NDC; None = the whole view): their union grown by the
/// refraction's reach (`car_glass.wgsl`: a lookup moves by at most
/// 0.035 of the view each way; 0.05 kept) and a pixel; the whole target
/// when any car needs it. car_glass.wgsl reads the copy only there.
pub fn copy_rect(rects: &[Option<[f32; 4]>], size: (u32, u32)) -> Option<[u32; 4]> {
    const REACH: f32 = 0.05;
    let full = [0, 0, size.0, size.1];
    let mut u = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for r in rects {
        let Some(r) = r else { return Some(full) };
        u = [u[0].min(r[0]), u[1].min(r[1]), u[2].max(r[2]), u[3].max(r[3])];
    }
    if u[0] > u[2] {
        return None;
    }
    // NDC → pixels (y down), grown by the reach (uv 0..1: 2·REACH in NDC).
    let px = |x: f32| ((x - 2.0 * REACH) * 0.5 + 0.5) * size.0 as f32;
    let px1 = |x: f32| ((x + 2.0 * REACH) * 0.5 + 0.5) * size.0 as f32;
    let py = |y: f32| (0.5 - (y + 2.0 * REACH) * 0.5) * size.1 as f32;
    let py1 = |y: f32| (0.5 - (y - 2.0 * REACH) * 0.5) * size.1 as f32;
    let x0 = (px(u[0]).floor() - 1.0).clamp(0.0, size.0 as f32) as u32;
    let x1 = (px1(u[2]).ceil() + 1.0).clamp(0.0, size.0 as f32) as u32;
    let y0 = (py(u[3]).floor() - 1.0).clamp(0.0, size.1 as f32) as u32;
    let y1 = (py1(u[1]).ceil() + 1.0).clamp(0.0, size.1 as f32) as u32;
    (x1 > x0 && y1 > y0).then_some([x0, y0, x1 - x0, y1 - y0])
}

/// The mean position of a draw's indexed vertices (model space).
pub(crate) fn centre(vertices: &[crate::formats::solid::Vertex], indices: &[u16], first: u32, count: u32) -> Vec3 {
    let range = indices.get(first as usize..(first + count) as usize).unwrap_or(&[]);
    let sum =
        range.iter().filter_map(|&i| vertices.get(i as usize)).fold(Vec3::ZERO, |a, v| a + Vec3::from(v.position));
    sum / range.len().max(1) as f32
}
