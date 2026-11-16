use glam::{Mat4, Vec2, Vec3};

/// A car's underglow colours (render info +0x34C / +0x350, 0x61FC30 on the
/// slot 0x9A part): R, G, B, A each.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Neon {
    /// RED, GREEN, BLUE, ALPHA (the attributes' low bytes).
    pub base: [u8; 4],
    /// RED2 − RED … ALPHA2 − ALPHA, wrapping bytes (read back unsigned).
    pub delta: [u8; 4],
    /// A part above level 0 in slot 0x9A, 0x9B or 0x9D: only then does
    /// 0x6227DE update the colours (else +0x354 keeps its constructor 0).
    pub lit: bool,
}

/// 0x75BED4 (_ftol): truncation towards zero.
fn ftol(x: f32) -> i32 {
    x as i32
}

/// 0x43C350: sine through the 16-bit binary angle (× 65536/2π, 0x78423C).
fn bsin(x: f32) -> f32 {
    let a = (x * (65536.0 / std::f32::consts::TAU)) as i32 as u16;
    (a as f32 * (std::f32::consts::TAU / 65536.0)).sin()
}

/// x − trunc(x / 2π)·2π (0x622836..0x6228BD; 2π as π + π, 0x7A65C0).
fn wrap(x: f32) -> f32 {
    let two_pi = std::f32::consts::PI + std::f32::consts::PI;
    x - ftol(x * (1.0 / two_pi)) as f32 * two_pi
}

/// The underglow's vertex colour this frame (0x6227DE..0x622A93), as
/// [`Shadow::color`] bytes (B, G, R, A), or `None` when it is not drawn
/// (alpha 0, or no lit neon part).
///
/// * `seconds`: the tick × 0.00025 (0x784268): 0x865188 in game, 0x8651A8
///   in the front end (0x8654A4 = 3); 1.0 on the reward screen (0x510C30).
/// * `fade`: [`Shadow::fade`].
///
/// a, b, c = 6.1, 7.2, 4.7 (0x7A6674..7C) × seconds, each wrapped to one
/// turn; p = trunc(1.2 (0x7A6680) · |sin a| · |sin b|); per channel
/// clamp(trunc(delta · |sin c|) + base, 0, 0xFE), alpha + p (render info
/// +0x358); the drawn alpha min(trunc(A · fade) · 4, 0xFF) (+0x354).
pub fn neon_color(neon: &Neon, seconds: f32, fade: f32) -> Option<[u8; 4]> {
    if !neon.lit {
        return None;
    }
    let (a, b, c) = (wrap(6.1 * seconds), wrap(7.2 * seconds), wrap(4.7 * seconds));
    let p = ftol(bsin(a).abs() * bsin(b).abs() * 1.2);
    let s = bsin(c).abs();
    let ch = |i: usize, extra: i32| (ftol(neon.delta[i] as f32 * s) + neon.base[i] as i32 + extra).clamp(0, 0xFE);
    let (r, g, bl, al) = (ch(0, 0), ch(1, 0), ch(2, 0), ch(3, p));
    let alpha = (ftol(al as f32 * fade) * 4).min(0xFF);
    (alpha > 0).then_some([bl as u8, g as u8, r as u8, alpha as u8])
}

/// The underglow's light colour as linear R, G, B 0..1 × its alpha
/// (0..1) for lights that take RGB (the volumetric smoke). [`neon_color`]
/// gives the bytes in the D3D colour's B, G, R, A order (world.efx's
/// diffuse); read as R, G, B they turned a yellow neon (the finished-career save's
/// Skyline) cyan-blue in the volumetric smoke.
pub fn neon_rgb(neon: &Neon, seconds: f32) -> Option<glam::Vec3> {
    neon_color(neon, seconds, 1.0)
        .map(|[b, g, r, a]| glam::Vec3::new(f32::from(r), f32::from(g), f32::from(b)) / 255.0 * (f32::from(a) / 255.0))
}

/// The light an underglow drawn with `color` ([`neon_color`]'s B, G, R, A
/// bytes) throws on the ground, 0..1: its brightest channel × its alpha
/// (world.efx adds `2·tex·colour·alpha` with SRCALPHA / ONE). A clone
/// measure for the contact shadows' underglow share (docs/glow-darkening.md), not native.
pub fn neon_light(color: [u8; 4]) -> f32 {
    let peak = color[..3].iter().copied().max().unwrap_or(0);
    f32::from(peak) / 255.0 * f32::from(color[3]) / 255.0
}

/// The neon colour render info +0x358 holds this frame (0x6227DE..0x622A93,
/// the same pulse as [`neon_color`] before the underglow's ×4 alpha step),
/// as R, G, B, A (the dword's bytes 0..3): 0 while no neon part is lit
/// (the constructor's 0, 0x6387BB).
pub fn neon_tube_color(neon: &Neon, seconds: f32) -> [u8; 4] {
    if !neon.lit {
        return [0; 4];
    }
    let (a, b, c) = (wrap(6.1 * seconds), wrap(7.2 * seconds), wrap(4.7 * seconds));
    let p = ftol(bsin(a).abs() * bsin(b).abs() * 1.2);
    let s = bsin(c).abs();
    let ch = |i: usize, extra: i32| (ftol(neon.delta[i] as f32 * s) + neon.base[i] as i32 + extra).clamp(0, 0xFE) as u8;
    [ch(0, 0), ch(1, 0), ch(2, 0), ch(3, p)]
}

/// The underglow tubes' marker names (0x6357C4..0x6357D4): pair `i` (0..3)
/// of each is `<prefix><i>_START` / `<prefix><i>_END` (0x7A14F4 / 0x7A51C4).
pub const NEON_TUBE_PREFIXES: [&str; 3] = ["FRONT_NEON_", "SIDE_NEON_", "REAR_NEON_"];
/// The models they are looked for in (0x6357AC..0x6357F5): front bumper
/// (slot 1), skirts (0xB), rear bumper (2); all three the wide body (slot
/// 6, ride +0x5A8) when it holds a part above level 0.
pub const NEON_TUBE_SLOTS: [usize; 3] = [1, 0xB, 2];
pub const NEON_TUBE_WIDEBODY_SLOT: usize = 6;
/// The underglow tubes' half width (0x7A673C, 0x615AB0).
pub const NEON_TUBE_WIDTH: f32 = 0.045;
/// The tubes' texture (render info +0x348, 0x7A8D38; GLOBALB, Additive).
pub const NEON_TUBE_TEXTURE: &str = "NEONBLUR";

/// The view as 0x60D800 reads it.
#[derive(Clone, Copy, Debug)]
pub struct TubeView {
    /// Camera +0x40.
    pub eye: Vec3,
    /// The camera matrix's columns 0 and 1 (camera +0x00 / +0x10 / +0x20,
    /// +0x04 / +0x14 / +0x24): right and down in world space.
    pub right: Vec3,
    pub down: Vec3,
    /// View +0x80 (0x5BC4B0's matrix).
    pub view_projection: Mat4,
    /// Screen width over height (0x870980 / 0x870984 in 0x5BC4B0).
    pub aspect: f32,
    /// Camera +0xC4: the field of view as a 16-bit binary angle.
    pub fov: u16,
}

/// 0x5BC4B0: a world point to screen pixels (x right, y down), here in
/// units of half the screen height.
fn to_screen(view: &TubeView, p: Vec3) -> Vec2 {
    let c = view.view_projection * p.extend(1.0);
    let w = 1.0 / c.w;
    Vec2::new((c.x * w + 1.0) * view.aspect, (c.y * w - 1.0) * -1.0)
}

/// 0x60D800: a car's neon tubes as camera-facing quads (four per marker
/// pair, NEONBLUR, drawn by 0x5C7E90 in the light-glow pass).
///
/// * `tubes`: the marker pairs (car space: the START / END markers'
///   translations, 0x441000 on the zero point).
/// * `position`, `rotation`: the car (node +0x910 / +0x920 of the list
// prov: inferred shadow.to_screen
///   0x8A30F0, **inferred** to be the drawn car's).
/// * `color`: R, G, B, A ([`neon_tube_color`]).
/// * `width`, `shorten`, `fade`: the call's arguments 7–9 (underglow
///   0.045, 1, 1.0).
///
/// Per call: d = |position − eye|, f = (13018 / fov)² (0x7A6738); within
/// 10f m (0x7A674C) the width scale k and fade L are 1; up to 75f m
/// (0x7A6750) k = 20·r + (1 − r) (0x7A6754), L = 1 − r, r = (d − 10f) /
/// (65f); beyond, k = 1 and L = 0 (0x7D7D3C), so nothing. Alpha =
/// min(trunc(A·L·fade)·2, 0xFE), nothing at 0. Per pair i (both ends
/// pulled in by 6 % of the pair's length when `shorten` and i = 1): S = the
/// screen direction from start to end (0x43CE50; (1, 0) when 0); with r =
/// right·w, u = down·w (w = k·width), n = r·S.y − u·S.x and t = r·S.x +
/// u·S.y: quads [A, B, B − n, A − n] and [A, B, B + n, A + n] (UVs (½, ½),
/// (½, ½), (½, 0), (½, 0)), the caps [A − n, A − n − t, A + n − t, A + n]
/// (UVs (½, 0), (0, 0), (0, 1), (½, 1)) and [B − n, B − n + t, B + n + t,
/// B + n] (UVs (½, 0), (1, 0), (1, 1), (½, 1)), every vertex in the
/// colour (0x5C5590 swaps bytes 0 and 2, so R is red).
#[allow(clippy::too_many_arguments)]
pub fn neon_tube_quads(
    view: &TubeView,
    tubes: &[[Vec3; 2]],
    position: Vec3,
    rotation: Mat4,
    color: [u8; 4],
    width: f32,
    shorten: bool,
    fade: f32,
    texture: u32,
    out: &mut Vec<crate::particles::Quad>,
) {
    if tubes.is_empty() || view.fov == 0 {
        return;
    }
    let d = (position - view.eye).length();
    let f = (13018.0 / view.fov as f32) * (13018.0 / view.fov as f32);
    let (near, far) = (f * 10.0, f * 75.0);
    let (k, l) = if d <= near {
        (1.0, 1.0)
    } else if d < far {
        let r = (d - near) / (far - near);
        (r * 20.0 + (1.0 - r), 1.0 - r)
    } else {
        (1.0, 0.0)
    };
    let w = k * width;
    let a = ftol(color[3] as f32 * l * fade);
    if a == 0 {
        return;
    }
    let alpha = (a * 2).min(0xFE) as u8;
    let rgba = [color[0], color[1], color[2], alpha];
    let (r, u) = (view.right * w, view.down * w);
    for (i, &[s, e]) in tubes.iter().enumerate() {
        let mut a = rotation.transform_vector3(s) + position;
        let mut b = rotation.transform_vector3(e) + position;
        if shorten && i == 1 {
            let step = (b - a) * 0.06;
            a += step;
            b -= step;
        }
        let dir = to_screen(view, b) - to_screen(view, a);
        let len = dir.length();
        let sd = if len != 0.0 { dir / len } else { Vec2::X };
        let n = r * sd.y - u * sd.x;
        let t = r * sd.x + u * sd.y;
        let mid = [[0.5, 0.5], [0.5, 0.5], [0.5, 0.0], [0.5, 0.0]];
        for corners in [[a, b, b - n, a - n], [a, b, b + n, a + n]] {
            out.push(crate::particles::Quad { texture, corners, uv: mid, rgba });
        }
        out.push(crate::particles::Quad {
            texture,
            corners: [a - n, a - n - t, a + n - t, a + n],
            uv: [[0.5, 0.0], [0.0, 0.0], [0.0, 1.0], [0.5, 1.0]],
            rgba,
        });
        out.push(crate::particles::Quad {
            texture,
            corners: [b - n, b - n + t, b + n + t, b + n],
            uv: [[0.5, 0.0], [1.0, 0.0], [1.0, 1.0], [0.5, 1.0]],
            rgba,
        });
    }
}
