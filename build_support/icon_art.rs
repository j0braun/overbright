// The game's icon, drawn in code (docs/platform-windows.md): an original placeholder (a
// glowing amber ring with a light streak on a dark tile), nothing taken from
// the game. Shared by build.rs (the .ico resource, build_support/windows_res.rs)
// and the library (the window icon, `platform::icon`, through include!), so it
// has no inner attributes and uses std only.

/// Draws the icon at `n`×`n` pixels: straight (not premultiplied) RGBA,
/// rows top to bottom, 4×4 supersampled.
pub fn draw(n: u32) -> Vec<[u8; 4]> {
    const SS: u32 = 4;
    let mut px = Vec::with_capacity((n * n) as usize);
    for y in 0..n {
        for x in 0..n {
            let mut acc = [0f32; 4]; // premultiplied sum
            for sy in 0..SS {
                for sx in 0..SS {
                    // Sample position in [-1, 1].
                    let u = ((x * SS + sx) as f32 + 0.5) / (n * SS) as f32 * 2.0 - 1.0;
                    let v = ((y * SS + sy) as f32 + 0.5) / (n * SS) as f32 * 2.0 - 1.0;
                    let [r, g, b, a] = sample(u, v);
                    acc[0] += r * a;
                    acc[1] += g * a;
                    acc[2] += b * a;
                    acc[3] += a;
                }
            }
            let a = acc[3] / (SS * SS) as f32;
            let un = |c: f32| if acc[3] > 0.0 { (c / acc[3]).clamp(0.0, 1.0) } else { 0.0 };
            let q = |c: f32| (c * 255.0 + 0.5) as u8;
            px.push([q(un(acc[0])), q(un(acc[1])), q(un(acc[2])), q(a)]);
        }
    }
    px
}

/// One sample: colour (0..1) and coverage.
fn sample(u: f32, v: f32) -> [f32; 4] {
    // The tile: a rounded square, corner radius 0.32.
    let (qx, qy) = ((u.abs() - 0.68).max(0.0), (v.abs() - 0.68).max(0.0));
    if (qx * qx + qy * qy).sqrt() > 0.32 {
        return [0.0; 4];
    }
    // Dark blue-grey, a little lighter at the top.
    let mut c = [0.07 + 0.03 * (1.0 - v) * 0.5, 0.08 + 0.03 * (1.0 - v) * 0.5, 0.11 + 0.04 * (1.0 - v) * 0.5];
    let mut add = |rgb: [f32; 3], k: f32| {
        for i in 0..3 {
            c[i] += rgb[i] * k;
        }
    };
    // The ring: radius 0.52, half width 0.11, with a soft glow around it.
    let r = (u * u + v * v).sqrt();
    let d = (r - 0.52).abs();
    let core = (1.0 - ((d - 0.11) / 0.03).clamp(0.0, 1.0)).max(0.0);
    let glow = (-(d / 0.16).powi(2)).exp() * 0.45;
    add([1.0, 0.62, 0.12], core);
    add([1.0, 0.45, 0.05], glow);
    // A hot inner edge on the ring.
    add([1.0, 0.9, 0.6], (1.0 - ((d - 0.02).abs() / 0.03).clamp(0.0, 1.0)) * 0.5 * core);
    // The streak: a thin cyan-white line through the middle, tilted, fading to the sides.
    let t = v + 0.18 * u;
    let line = (-(t / 0.035).powi(2)).exp() * (1.0 - (u.abs() / 0.95).powi(2)).max(0.0);
    add([0.55, 0.85, 1.0], line * 0.9);
    [c[0].min(1.0), c[1].min(1.0), c[2].min(1.0), 1.0]
}
