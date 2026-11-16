use super::*;

/// A curve of evenly spaced values over x0..x1 (0x5764E0): clamped at the
/// ends, linear between.
pub(super) fn table(values: &[f32], x0: f32, x1: f32, x: f32) -> f32 {
    let n = values.len();
    let f = (x - x0) * ((n - 1) as f32 / (x1 - x0));
    let i = f as i32;
    if i < 0 || f < 0.0 {
        return values[0];
    }
    if i as usize >= n - 1 {
        return values[n - 1];
    }
    let t = f - i as f32;
    values[i as usize + 1] * t + values[i as usize] * (1.0 - t)
}

/// A curve through points (0x5767A0): clamped at the ends, linear between,
/// the mean of a vertical step.
pub(super) fn pairs(points: &[(f32, f32)], x: f32) -> f32 {
    let n = points.len();
    if x <= points[0].0 {
        return points[0].1;
    }
    if x >= points[n - 1].0 {
        return points[n - 1].1;
    }
    for w in points.windows(2) {
        let ((x0, y0), (x1, y1)) = (w[0], w[1]);
        if x0 <= x && x < x1 {
            if (x1 - x0).abs() <= 1e-6 {
                return (y1 - y0) * 0.5 + y0;
            }
            return (x - x0) / (x1 - x0) * (y1 - y0) + y0;
        }
    }
    points[0].1
}

/// The game's 16-bit angle of a direction (0x43E6D0), counter-clockwise
/// from +x: the arcsine table (0x43C490) of the smaller component over
/// the length — |dy|/r when |dy| < |dx|, else 0x4000 minus that of |dx|/r
/// (nothing under r² ≤ 1e-4, 0x7843EC) — then put in its quadrant (dx < 0:
/// 0x8000 − a; dy < 0: 0x10000 − a; both: a + 0x8000).
pub fn angle16(dx: f32, dy: f32) -> u16 {
    let r2 = dx * dx + dy * dy;
    let ratio = |c: f32| if r2 > 1e-4 && c != 0.0 { c.abs() / r2.sqrt() } else { 0.0 };
    let a = if dy.abs() < dx.abs() { asin16(ratio(dy)) } else { 0x4000u16.wrapping_sub(asin16(ratio(dx))) };
    match (dx < 0.0, dy < 0.0) {
        (false, false) => a,
        (true, false) => 0x8000u16.wrapping_sub(a),
        (false, true) => 0u16.wrapping_sub(a),
        (true, true) => a.wrapping_add(0x8000),
    }
}

/// 0x43C490: the 16-bit arcsine by the table at 0x7EA628 (an angle and a
/// slope per entry): x × 65536 truncated finds a segment by halving
/// (0x8000, 0xC000, … at most 11 steps), 16 entries a segment; the entry's
/// angle less (x − the entry's x) × slope × −65536, truncated. 1 or more
/// is 0x4000; negative x mirrors (0x10000 − a).
pub fn asin16(x: f32) -> u16 {
    let neg = x < 0.0;
    let x = x.abs();
    if x >= 1.0 {
        return if neg { 0xC000 } else { 0x4000 };
    }
    let i = (x * 65536.0) as i32;
    let (mut step, mut end, mut seg) = (0x8000i32, 0x8000i32, 0usize);
    while i >= end && seg < 11 {
        step >>= 1;
        end += step;
        seg += 1;
    }
    let start = end - step;
    let local = (i - start) >> (11 - seg);
    let at = (step >> 4) * local + start;
    let (angle, slope) = ASIN_TABLE[seg * 16 + local as usize];
    let a = angle.wrapping_sub(((x - at as f32 * 1.525_878_9e-5) * slope * -65536.0) as i32 as u16);
    if neg { 0u16.wrapping_sub(a) } else { a }
}

#[allow(clippy::excessive_precision, clippy::approx_constant)]
pub(super) static ASIN_TABLE: crate::exe_tables::Table<[(u16, f32); 208]> = crate::exe_tables::Table::new(|t| &t.asin);

/// A 16-bit angle difference in radians, −π..π.
pub(super) fn radians16(a: u16) -> f32 {
    let f = a as f32 * 9.58738e-05;
    if f > 3.14159 { f - 6.28318 } else { f }
}

/// A unit vector in the plane (0x43CE50: x and y times 1 / length); a
/// zero vector gives (1, 0) (0x43CE99), so a standing car's "travel" in
/// 0x404EF0 is the world's +x axis, not nothing.
pub(super) fn flat_unit(v: Vec3) -> Vec2 {
    let p = v.truncate();
    let len = p.length();
    if len == 0.0 { Vec2::X } else { p * (1.0 / len) }
}
