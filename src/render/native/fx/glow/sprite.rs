use super::*;

/// One layer of a flare type (0x2C bytes at 0x7F3A30…0x7F3CC4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    /// +0x00 / +0x04: the size at intensity 0 and 1 (lerped).
    pub size: [f32; 2],
    /// +0x08 / +0x0C / +0x10: red, green, blue (0–255) at intensity 1.
    /// 0x490920 packs them into the low three bytes of the D3DCOLOR in
    /// this order; 0x5C5590 swaps bytes 0 and 2 when it copies the quad, so
    /// +0x08 ends up the red.
    pub rgb: [f32; 3],
    /// +0x14: alpha at intensity 1, doubled (and clamped to 255).
    pub alpha: f32,
    /// +0x18: the exponent of the facing term (flag 4).
    pub exponent: f32,
    /// +0x1C: how far towards the eye the quad is drawn.
    pub toward_eye: f32,
    /// +0x20 / +0x24: the clamp of depth / view +0x0C, the world size per
    /// unit of size.
    pub scale: [f32; 2],
    /// +0x28: the texture, index into [`TEXTURES`] (0x82CC10).
    pub texture: usize,
}

const fn layer(
    size: [f32; 2],
    rgb: [f32; 3],
    alpha: f32,
    exponent: f32,
    toward_eye: f32,
    scale: [f32; 2],
    texture: usize,
) -> Option<Layer> {
    Some(Layer { size, rgb, alpha, exponent, toward_eye, scale, texture })
}

/// The flare textures (hashes 0x7F39CC, looked up into 0x82CC10 by
/// 0x4908F0): HEADLIGHTFLAREINNER, HEADLIGHTFLAREOUTER, HEADLIGHTGLOW, all
/// GLOBALB, verified, `+0x56 = 2` (SRCALPHA / ONE).
pub const TEXTURES: [&str; 3] = ["HEADLIGHTFLAREINNER", "HEADLIGHTFLAREOUTER", "HEADLIGHTGLOW"];

pub fn texture_hashes() -> [u32; 3] {
    TEXTURES.map(bin_hash)
}

const HEAD_INNER: Option<Layer> = layer([0.0, 64.0], [100.0, 120.0, 170.0], 80.0, 1.45, 0.8, [0.01, 0.048], 0);
const HEAD_OUTER: Option<Layer> = layer([28.0, 28.0], [100.0, 120.0, 140.0], 20.0, 3.0, 0.8, [0.01, 0.015], 1);

/// The type table 0x7F3CC8: two layer pointers per type, 15 types. Type 0
/// is the cars' headlights (0x620530), 1 their brake lights; the street
/// lamps of Bayview's flare packs are all type 9.
pub const TYPES: [[Option<Layer>; 2]; 15] = [
    [HEAD_INNER, HEAD_OUTER],
    // +0x1C is rewritten each frame by 0x61AEA0: 0.2 (0x7F3A20), or 0.34
    // in views 4 and 5.
    [layer([20.0, 30.0], [128.0, 25.0, 25.0], 120.0, 0.45, 0.2, [0.01, 0.03], 2), None],
    [layer([20.0, 20.0], [255.0, 255.0, 255.0], 128.0, 1.2, 0.2, [0.005, 0.01], 2), None],
    [layer([20.0, 20.0], [128.0, 128.0, 128.0], 128.0, 1.0, 0.2, [0.005, 0.01], 2), None],
    [layer([30.0, 40.0], [128.0, 64.0, 64.0], 128.0, 1.0, 1.0, [0.01, 0.02], 0), None],
    [layer([30.0, 40.0], [64.0, 64.0, 128.0], 128.0, 1.0, 1.0, [0.01, 0.02], 0), None],
    [layer([30.0, 40.0], [128.0, 128.0, 128.0], 128.0, 1.0, 1.0, [0.01, 0.02], 0), None],
    [HEAD_INNER, HEAD_OUTER],
    [HEAD_INNER, HEAD_OUTER],
    [layer([30.0, 30.0], [110.0, 100.0, 75.0], 110.0, 1.0, 4.0, [0.001, 0.1], 2), None],
    [layer([10.0, 10.0], [110.0, 75.0, 0.0], 150.0, 1.0, 2.7, [0.001, 0.1], 2), None],
    [layer([10.0, 10.0], [110.0, 0.0, 0.0], 150.0, 1.0, 4.0, [0.001, 0.1], 2), None],
    [layer([10.0, 10.0], [0.0, 0.0, 120.0], 150.0, 1.0, 4.0, [0.001, 0.1], 2), None],
    [layer([30.0, 30.0], [110.0, 75.0, 0.0], 150.0, 1.0, 4.0, [0.001, 0.1], 2), None],
    [layer([1.0, 10.0], [105.0, 26.5, 0.0], 70.0, 1.0, 1.0, [0.07, 0.07], 2), None],
];

/// The squared distances of 0x490920's fade in its mode 0 (0x78D798 /
/// 0x78D79C): full up to 32000, gone past 40000.
const FADE_NEAR: f32 = 32000.0;
pub(super) const FADE_FAR: f32 = 40000.0;
/// The rotation by screen position (0x7F3D50): degrees across the screen.
const SPIN_DEGREES: f32 = 240.0;

/// What 0x490920 reads of the view.
#[derive(Clone, Copy, Debug)]
pub struct View {
    /// The camera position (camera +0x40).
    pub eye: Vec3,
    /// The camera matrix's columns 0, 1 and 2 in world space: right, down
    /// (the game's camera space is right, down, forward — see the
    /// environment map) and forward.
    pub right: Vec3,
    pub down: Vec3,
    pub forward: Vec3,
    /// The projection, for the frustum test (0x5BC460) and the screen x
    /// (0x5BC4B0).
    pub view_projection: Mat4,
    /// View +0x0C, `256 / tan(fov / 2)` (0x5C7ED0).
    pub scale: f32,
    /// The front end (0x8654A4 ≠ 6).
    pub front_end: bool,
}

impl View {
    /// 0x5BC460: inside all six planes (strictly).
    fn contains(&self, p: Vec3) -> bool {
        let c = self.view_projection * Vec4::new(p.x, p.y, p.z, 1.0);
        c.w > 0.0 && c.x > -c.w && c.x < c.w && c.y > -c.w && c.y < c.w && c.z > 0.0 && c.z < c.w
    }

    /// 0x5BC4B0's x over the screen width (0x5BC200): 0 at the left edge.
    fn screen_x(&self, p: Vec3) -> f32 {
        let c = self.view_projection * Vec4::new(p.x, p.y, p.z, 1.0);
        (c.x / c.w + 1.0) * 0.5
    }
}

/// 0x5BA8C0: the axis-angle matrix (row-major, as the game stores it).
fn axis_angle(axis: Vec3, sin: f32, cos: f32) -> [[f32; 3]; 3] {
    let (x, y, z) = (axis.x, axis.y, axis.z);
    let t = 1.0 - cos;
    [
        [t * x * x + cos, t * x * y - z * sin, y * sin + t * x * z],
        [z * sin + t * x * y, t * y * y + cos, t * y * z - x * sin],
        [t * x * z - y * sin, x * sin + t * y * z, t * z * z + cos],
    ]
}

/// A row vector times the matrix (the game's `v · M`).
fn row_times(v: Vec3, m: &[[f32; 3]; 3]) -> Vec3 {
    Vec3::new(
        v.x * m[0][0] + v.y * m[1][0] + v.z * m[2][0],
        v.x * m[0][1] + v.y * m[1][1] + v.z * m[2][1],
        v.x * m[0][2] + v.y * m[1][2] + v.z * m[2][2],
    )
}

/// 0x490920 in its mode 0 (the one the glow pass uses): the quads of one
/// flare under `transform`, at `intensity`, with `colour` (R, G, B) in
/// place of the layers' when given (the cars' headlights pass render info
/// +0x380).
pub fn sprites(
    view: &View,
    flare: &Flare,
    transform: Mat4,
    intensity: f32,
    colour: Option<[u8; 3]>,
    out: &mut Vec<Quad>,
) {
    sprites_in(view, flare, transform, intensity, colour, true, out);
}

/// 0x490920 in its modes 0 (`spin`) and 3 / 4: the same quads, but the
/// screen-angle turn is mode 0's alone (0x49139E: `param_6 == 0`).
pub(super) fn sprites_in(
    view: &View,
    flare: &Flare,
    transform: Mat4,
    intensity: f32,
    colour: Option<[u8; 3]>,
    spin: bool,
    out: &mut Vec<Quad>,
) {
    let Some(layers) = TYPES.get(flare.kind as usize) else { return };
    let p = transform.transform_point3(flare.position);
    if !view.contains(p) {
        return;
    }
    let to_eye = view.eye - p;
    let d2 = to_eye.length_squared();
    if d2 > FADE_FAR {
        return;
    }
    let fade = if d2 > FADE_NEAR { 1.0 - (d2 - FADE_NEAR) / (FADE_FAR - FADE_NEAR) } else { 1.0 };
    let n = to_eye.normalize_or_zero();
    // The facing term: the direction (w = 0) under the transform against
    // the way to the eye; its magnitude when two-sided (flag 1), else 0
    // when it faces away.
    let mut facing = transform.transform_vector3(flare.direction).dot(n);
    if flare.flags & 1 != 0 {
        facing = facing.abs();
    } else if facing < 0.0 {
        facing = 0.0;
    }
    // World size per unit: the depth over view +0x0C (the camera matrix's
    // z of the point), clamped per layer.
    let depth_scale = (p - view.eye).dot(view.forward) / view.scale;
    // The spin: the screen x of the point × 240°, as a binary angle
    // (0x401020: trunc(deg · 65536) / 360, the division truncating).
    let degrees = view.screen_x(p) * SPIN_DEGREES;
    let angle = (((degrees * 65536.0) as i32) / 360) as u16;
    let (sin, cos) = (angle as f32 * (std::f32::consts::TAU / 65536.0)).sin_cos();
    let (right, down) = if spin {
        let m = axis_angle(view.forward.normalize_or_zero(), sin, cos);
        (row_times(view.right, &m), row_times(view.down, &m))
    } else {
        (view.right, view.down)
    };
    for layer in layers.iter().flatten() {
        let mut i = intensity;
        if flare.flags & 4 != 0 {
            i = (facing.powf(layer.exponent) * intensity).min(1.0);
        }
        let mut size = (layer.size[1] - layer.size[0]) * i + layer.size[0];
        if size <= 0.0 {
            continue;
        }
        size *= depth_scale.max(layer.scale[0]).min(layer.scale[1]);
        let q = p + n * layer.toward_eye;
        let alpha = (i * layer.alpha * fade * 2.0).min(255.0);
        let rgb = match colour {
            Some(c) => c,
            None => layer.rgb.map(|c| (i * c) as i32 as u8),
        };
        let (r, u) = (right * size, down * size);
        out.push(Quad {
            texture: texture_hashes()[layer.texture],
            corners: [q - r - u, q + r - u, q + r + u, q - r + u],
            uv: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            rgba: [rgb[0], rgb[1], rgb[2], alpha as i32 as u8],
        });
    }
}

/// 0x491860: in game only, the flares of the flare packs of the sections
/// visible from the eye (0x5E00E0), each at intensity 1 under the identity
/// (0x872E90). The pack's box test (0x5BC310) is left to the flare's own
/// frustum test (a flare inside the frustum puts its pack's box there).
pub fn world_sprites(view: &View, packs: &[FlarePack], visible: Option<&[u32]>, out: &mut Vec<Quad>) {
    if view.front_end {
        return;
    }
    for pack in packs {
        if let Some(v) = visible
            && !v.contains(&pack.section)
        {
            continue;
        }
        for flare in &pack.flares {
            sprites(view, flare, Mat4::IDENTITY, 1.0, None, out);
        }
    }
}

/// The car light markers 0x6204E0 turns into flares (type +0x2C).
pub const LEFT_HEADLIGHT: u32 = 0x9DB9_0133;
pub const RIGHT_HEADLIGHT: u32 = 0xD090_91C6;
pub const CENTRE_HEADLIGHT: u32 = 0x7A5B_CF69;
pub const LEFT_BRAKELIGHT: u32 = 0x31A6_6786;
pub const RIGHT_BRAKELIGHT: u32 = 0xBF70_0A79;
pub const CENTRE_BRAKELIGHT: u32 = 0xA2A2_FC7C;

/// 0x620530: the type of a light marker's flare, 0 for the headlights, 1
/// for the brake lights.
pub fn car_flare_kind(hash: u32) -> Option<u8> {
    match hash {
        LEFT_HEADLIGHT | RIGHT_HEADLIGHT | CENTRE_HEADLIGHT => Some(0),
        LEFT_BRAKELIGHT | RIGHT_BRAKELIGHT | CENTRE_BRAKELIGHT => Some(1),
        _ => None,
    }
}

/// One car light flare (render info list +0x3F0): the marker's hash, its
/// translation less render info +0x30 and its z row, flags 4.
#[derive(Clone, Copy, Debug)]
pub struct CarFlare {
    pub hash: u32,
    pub flare: Flare,
}

/// 0x615640 for one car with its lights on (+0x93D, set by the
/// constructor) and not braking: the left and right headlights at
/// intensity 1 (1 + 1, clamped) in the headlight colour (render info
/// +0x380); the centre headlight never (its hash is not on the list); the
/// left and right brake lights at 0.85 (16.85 braking), the centre one at
/// 0 (8.425 braking), clamped to 1.
pub fn car_sprites(
    view: &View,
    flares: &[CarFlare],
    transform: Mat4,
    headlight: [u8; 3],
    braking: bool,
    out: &mut Vec<Quad>,
) {
    car_sprites_faded(view, flares, transform, headlight, braking, 1.0, out);
}

/// [`car_sprites`] at `share` of their intensity (pop-in: the traffic's
/// ring fade at World Detail Ultra / Extreme; 1 = native).
pub fn car_sprites_faded(
    view: &View,
    flares: &[CarFlare],
    transform: Mat4,
    headlight: [u8; 3],
    braking: bool,
    share: f32,
    out: &mut Vec<Quad>,
) {
    if share <= 0.0 {
        return;
    }
    for f in flares {
        let (intensity, colour) = match f.hash {
            LEFT_HEADLIGHT | RIGHT_HEADLIGHT => (2.0f32, Some(headlight)),
            LEFT_BRAKELIGHT | RIGHT_BRAKELIGHT => (if braking { 16.85 } else { 0.85 }, None),
            CENTRE_BRAKELIGHT => (if braking { 8.425 } else { 0.0 }, None),
            _ => (0.0, None),
        };
        if intensity <= 0.0 {
            continue;
        }
        let i = intensity.min(1.0);
        sprites(view, &f.flare, transform, if share < 1.0 { i * share } else { i }, colour, out);
    }
}
