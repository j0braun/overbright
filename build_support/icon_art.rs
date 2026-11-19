// The app icon, drawn in code (docs/playing/platform-windows.md): Overbright's mark as
// the launcher draws it in its header (`Kind::Logo`, src/launcher/ui.rs): a
// lime ring (`Theme::lime`), its width 0.32 of its outer radius, here on a
// rounded tile in the launcher's page colours (`page_top` to `page_bottom`)
// with the slab's faint edge light. Our own art, nothing taken from the game.
// Shared by build.rs (the .ico resource, build_support/windows_res.rs), the
// library (the window icon, `platform::icon`, through include!) and
// scripts/release/icon_rgba.rs (the Linux PNG), so it has no inner attributes
// and uses std only.
//
// Crisp at every size: the ring's radii are whole pixels around the tile's
// centre (a pixel corner at even sizes), so its outermost and innermost rows
// and columns are solid, and the ring is never under 2 px wide (a little
// heavier than 0.32 at the small sizes, where rounding goes up). Edges are
// antialiased analytically (coverage from the distance to the edge, 1 px ramp).

/// The launcher's lime (`THEME_A.lime`, #C6F087).
const LIME: [f32; 3] = [0xc6 as f32 / 255.0, 0xf0 as f32 / 255.0, 0x87 as f32 / 255.0];
/// The launcher's page gradient, top and bottom (`page_top` #080A08, `page_bottom` #040504).
const PAGE_TOP: [f32; 3] = [0x08 as f32 / 255.0, 0x0a as f32 / 255.0, 0x08 as f32 / 255.0];
const PAGE_BOTTOM: [f32; 3] = [0x04 as f32 / 255.0, 0x05 as f32 / 255.0, 0x04 as f32 / 255.0];
/// The slab's edge light (`slab_edge`, white at 23 / 255), the tile's rim.
const RIM: f32 = 23.0 / 255.0;

/// The ring's geometry at `n` px: (outer radius, inner radius) in pixels.
pub fn ring_px(n: u32) -> (f32, f32) {
    // Outer radius 0.62 of the half tile (a little more at the smallest sizes, so
    // the mark keeps its weight in a 16 px taskbar slot).
    let k = if n <= 16 {
        0.72
    } else if n <= 24 {
        0.68
    } else {
        0.62
    };
    let outer = (k * n as f32 / 2.0).round().max(2.0);
    let width = (0.32 * outer).round().max(2.0);
    (outer, outer - width)
}

/// Draws the icon at `n`×`n` pixels: straight (not premultiplied) RGBA,
/// rows top to bottom.
pub fn draw(n: u32) -> Vec<[u8; 4]> {
    let nf = n as f32;
    let c = nf / 2.0;
    let (outer, inner) = ring_px(n);
    // The tile: the whole square with rounded corners (radius 0.2 of the side).
    let corner = (0.2 * nf).max(2.0);
    let half = nf / 2.0;
    // The rim: 1 px at the small sizes, up to 2 px at 256.
    let rim_w = (nf / 128.0).clamp(1.0, 2.0);
    let mut px = Vec::with_capacity((n * n) as usize);
    for y in 0..n {
        for x in 0..n {
            let (u, v) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            // Signed distance to the rounded square's edge (negative inside).
            let (qx, qy) = (u.abs() - (half - corner), v.abs() - (half - corner));
            let out_d = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - corner;
            let tile = (0.5 - out_d).clamp(0.0, 1.0);
            if tile <= 0.0 {
                px.push([0; 4]);
                continue;
            }
            // The page's vertical gradient.
            let t = (y as f32 + 0.5) / nf;
            let mut col = [0f32; 3];
            for ((ch, top), bottom) in col.iter_mut().zip(PAGE_TOP).zip(PAGE_BOTTOM) {
                *ch = top + (bottom - top) * t;
            }
            // The rim just inside the edge.
            let rim = ((rim_w + 0.5 + out_d).clamp(0.0, 1.0)) * RIM;
            for ch in &mut col {
                *ch += (1.0 - *ch) * rim;
            }
            // The ring.
            let r = (u * u + v * v).sqrt();
            let ring = (outer - r + 0.5).clamp(0.0, 1.0) * (r - inner + 0.5).clamp(0.0, 1.0);
            for (ch, lime) in col.iter_mut().zip(LIME) {
                *ch += (lime - *ch) * ring;
            }
            let q = |f: f32| (f.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            px.push([q(col[0]), q(col[1]), q(col[2]), q(tile)]);
        }
    }
    px
}
