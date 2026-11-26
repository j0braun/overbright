use super::*;

/// The streak length of 0x490920's mode 2 (0x7F39A0, written at
/// 0x490977): 1.2 (0x7F39A4) + (10 (0x7F39A8) − 1.2) × the view's wetness
/// (weather +0x509C; view 4's weather object is view 1's, 0x5C310F).
pub fn streak_factor(wetness: f32) -> f32 {
    1.2 + (10.0 - 1.2) * wetness
}

/// A flare's mirror height (+0x1C): 999 (0x4479C000) as loaded, not yet
/// probed; 9999 (0x461C3C00) when it has no reflection.
pub const MIRROR_UNSET: f32 = 999.0;
pub const MIRROR_NONE: f32 = 9999.0;

/// 0x490920's probe of the mirror height (0x4909E4…0x490AE9, run while
/// +0x1C is 999 or param 5 is 1, with param 7 = 0): the ground under P
/// (0x486ED0 / 0x5E7E80 → 0x5E72F0 on a fresh query, 0x5E0B50); none →
/// 9999; within 0.05 m (0x7844F4) of P's z → 9999; else P's z mirrored
/// about the ground, `h − (z − h)`.
pub fn mirror_height(p: Vec3, ground: Option<f32>) -> f32 {
    match ground {
        None => MIRROR_NONE,
        Some(h) if (h - p.z).abs() < 0.05 => MIRROR_NONE,
        Some(h) => h - (p.z - h),
    }
}

impl View {
    /// 0x5BC310 (with no matrix): the box against the view's six planes,
    /// false when it lies wholly outside one.
    fn box_visible(&self, min: Vec3, max: Vec3) -> bool {
        let m = self.view_projection;
        let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));
        let centre = (min + max) * 0.5;
        let half = (max - min) * 0.5;
        [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2].iter().all(|pl| {
            let n = pl.truncate();
            n.dot(centre) + pl.w + n.abs().dot(half) >= 0.0
        })
    }
}

/// 0x490920 in its mode 2 (the road reflection, view 4) for one flare at
/// `mirror` (+0x1C, already probed): skipped while 999 or 9999; P = the
/// position under the transform with its z replaced by the mirror height
/// (0x487080 at 0x490BE9), whose box of half size `streak` (0x43D540 /
/// 0x43D610) must pass 0x5BC310 — no point test (0x5BC460 is mode 0's);
/// the direction's z negated (0x48B7F0 / 0x487320 / 0x487080 at
/// 0x490C79…0x490CAC) for the facing term; cut past d² 40000 like mode 0,
/// but the alpha is 255 (0x784544, 0x4915CF / 0x4915E7) instead of the
/// faded one; no screen-angle turn (mode 0 only); every layer takes
/// HEADLIGHTGLOW (0x4901D0(0x3394FE62)); the quad's down axis (camera
/// column 1) is scaled again by `streak`, or by half of it when the flare
/// stands less than 4 m (0x78D7B0) above its mirror height.
pub fn reflection_sprites(view: &View, flare: &Flare, transform: Mat4, mirror: f32, streak: f32, out: &mut Vec<Quad>) {
    if mirror == MIRROR_UNSET || mirror == MIRROR_NONE {
        return;
    }
    let Some(layers) = TYPES.get(flare.kind as usize) else { return };
    let placed = transform.transform_point3(flare.position);
    let p = Vec3::new(placed.x, placed.y, mirror);
    if !view.box_visible(p - Vec3::splat(streak), p + Vec3::splat(streak)) {
        return;
    }
    let to_eye = view.eye - p;
    if to_eye.length_squared() > FADE_FAR {
        return;
    }
    let n = to_eye.normalize_or_zero();
    let direction = Vec3::new(flare.direction.x, flare.direction.y, -flare.direction.z);
    let mut facing = transform.transform_vector3(direction).dot(n);
    if flare.flags & 1 != 0 {
        facing = facing.abs();
    } else if facing < 0.0 {
        facing = 0.0;
    }
    let depth_scale = (p - view.eye).dot(view.forward) / view.scale;
    let stretch = if flare.position.z - mirror < 4.0 { streak * 0.5 } else { streak };
    for layer in layers.iter().flatten() {
        let mut i = 1.0f32;
        if flare.flags & 4 != 0 {
            i = facing.powf(layer.exponent).min(1.0);
        }
        let mut size = (layer.size[1] - layer.size[0]) * i + layer.size[0];
        if size <= 0.0 {
            continue;
        }
        size *= depth_scale.max(layer.scale[0]).min(layer.scale[1]);
        let q = p + n * layer.toward_eye;
        let rgb = layer.rgb.map(|c| (i * c) as i32 as u8);
        let (r, u) = (view.right * size, view.down * (size * stretch));
        out.push(Quad {
            texture: bin_hash("HEADLIGHTGLOW"),
            corners: [q - r - u, q + r - u, q + r + u, q - r + u],
            uv: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            rgba: [rgb[0], rgb[1], rgb[2], 255],
        });
    }
}

/// The section flares' probed mirror heights (+0x1C of each flare
/// record), per pack, 999 until a reflection draw first reaches them.
#[derive(Default)]
pub struct MirrorHeights(Vec<Vec<f32>>);

/// 0x5CAC9C: 0x491860(view 4, mode 2) — the flares of the packs of the
/// sections visible from the eye whose box passes 0x5BC310, each through
/// 0x490920(view 4, flare, identity, 1.0, 0, 2, 0, 0). A flare's mirror
/// height is probed the first time (param 5 = 0: only while it is 999) and
/// kept, as the record keeps it.
pub fn world_reflection_sprites(
    view: &View,
    packs: &[FlarePack],
    visible: Option<&[u32]>,
    wetness: f32,
    mirrors: &mut MirrorHeights,
    ground: &mut dyn FnMut(Vec3) -> Option<f32>,
    out: &mut Vec<Quad>,
) {
    if view.front_end {
        return;
    }
    if mirrors.0.len() != packs.len() || mirrors.0.iter().zip(packs).any(|(m, p)| m.len() != p.flares.len()) {
        mirrors.0 = packs.iter().map(|p| vec![MIRROR_UNSET; p.flares.len()]).collect();
    }
    let streak = streak_factor(wetness);
    for (pack, heights) in packs.iter().zip(&mut mirrors.0) {
        if let Some(v) = visible
            && !v.contains(&pack.section)
        {
            continue;
        }
        if !view.box_visible(pack.min, pack.max) {
            continue;
        }
        for (flare, h) in pack.flares.iter().zip(heights.iter_mut()) {
            if *h == MIRROR_UNSET {
                *h = mirror_height(flare.position, ground(flare.position));
            }
            reflection_sprites(view, flare, Mat4::IDENTITY, *h, streak, out);
        }
    }
}
