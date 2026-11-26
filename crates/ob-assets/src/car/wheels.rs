//! Wheel pose and wheel-composite textures.

use super::*;

/// How a wheel sits at the moment: suspension lift (m, up), steer (radians
/// around +z) and roll (radians around the axle, +y).
#[derive(Clone, Copy, Debug, Default)]
pub struct WheelPose {
    pub lift: f32,
    pub steer: f32,
    pub roll: f32,
    /// The spinner's own angle about the axle (radians; car object +0x4E8
    /// + 4i, which 0x5EC800 turns the spinner frames by).
    pub spinner: f32,
    /// The x-turn of a front wheel's rim and spinner frames ([`Car::camber`],
    /// 0x5BAD00 at 0x62794C / 0x6287D3 / 0x62964F / 0x62A138 on a copy of
    /// the frame): about the car's x axis after the steer, at the part's
    /// own origin; wheel 0 (front left) by +angle, wheel 1 by −angle. The
    /// draw takes it only at LOD A or B (0x6270A0).
    pub tilt: f32,
    /// The same turn for the front brakes (0x62E141 / 0x62EBB7): 0x5BAD00
    /// turns the caller's frame [ebp+0x1C] itself, in place, so each draw
    /// of the car in a frame adds one more turn (the showroom draws the
    /// floor reflection first: 1×, then the car: 2×; read from an apitrace
    /// of the original's main menu, 3.5° and 7.0° with v = 0.5).
    pub brake_tilt: f32,
}

impl WheelPose {
    /// Car-space transform that moves a part at rest at wheel centre `at`
    /// into this pose.
    pub fn matrix(&self, at: Vec3, roll: bool) -> Mat4 {
        self.matrix_tilted(at, roll, 0.0)
    }

    /// [`WheelPose::matrix`] with the front wheel's x-turn `tilt` (signed,
    /// see [`WheelPose::tilt`]), applied after the steer.
    pub fn matrix_tilted(&self, at: Vec3, roll: bool, tilt: f32) -> Mat4 {
        let spin = if roll { self.roll } else { 0.0 };
        Mat4::from_translation(at + Vec3::Z * self.lift)
            * Mat4::from_rotation_x(tilt)
            * Mat4::from_rotation_z(self.steer)
            * Mat4::from_rotation_y(spin)
            * Mat4::from_translation(-at)
    }
}

/// [`Car::camber`] from the fitted performance parts: 0x59A430 gives
/// v = 0.5·(part 0x24 fitted) + 0.5·(part 0x23 fitted) (bytes ride +0x364 /
/// +0x363 = the upgrade state's +0x50 + id; read from the running original:
/// Rachel's 350Z with 0x24 alone, v = 0.5), 0x6270DC… turns 7·v degrees
/// into binary degrees (×65536, ftol) and divides by 360 (integer).
pub fn camber_angle(fitted: &dyn Fn(u8) -> bool) -> f32 {
    let v = 0.5 * f32::from(u8::from(fitted(0x24))) + 0.5 * f32::from(u8::from(fitted(0x23)));
    let binary = (7.0 * v * 65536.0) as i32 / 360;
    binary as f32 * std::f32::consts::TAU / 65536.0
}

/// 0x61DEA0 → 0x612EF0: a wheel or spinner texture painted into its dummy
/// (DUMMY_WHEEL / DUMMY_SPINNER): only when the dummy, the source and the
/// mask are all 32-bit or all not (+0x4A) and of one size; per texel with
/// the mask's byte 0 `m`, the source texel where `m` = 0, else the source
/// weighed (0x611B10) with the source tinted by the paint (0x611A40: each
/// byte × the paint's / 255, truncated) by 1 − m/255 and m/255, the
/// source's alpha kept. The paint is the slot's RED, GREEN, BLUE, GLOSS, or
/// white without a part. The palettized path (0x612D20: sixteen levels,
/// the mask's palette byte 0 >> 4) is not taken on PC, where the dummies
/// are 32-bit (**inferred** that the clone's decoded byte order matches
/// the skin's: under Wine yellow powder coat painted the 350Z's rims
/// yellow). None when it does not apply: the game leaves the dummy as it
/// was ([`wheel_remap_target`]).
pub fn composite_wheel(
    textures: &HashMap<u32, Texture>,
    dummy: u32,
    source: u32,
    mask: u32,
    paint: Option<[u8; 4]>,
) -> Option<Texture> {
    use crate::formats::texture::PixelFormat;
    let (d, s, m) = (textures.get(&dummy)?, textures.get(&source)?, textures.get(&mask)?);
    let wide = |t: &Texture| matches!(t.format, PixelFormat::Bgra8 | PixelFormat::Rgba8);
    if !(wide(d) && wide(s) && wide(m))
        || (d.width, d.height) != (s.width, s.height)
        || (d.width, d.height) != (m.width, m.height)
    {
        return None;
    }
    let paint = paint.unwrap_or([0xFF; 4]);
    // The generated car texture pack (opt-in): the source and its
    // mask upscaled alike are composited the same way at their size, with
    // a mip chain. Without the pack (native) this is skipped.
    if let Some(t) = composite_wheel_pack(d, s, m, paint) {
        return Some(t);
    }
    let (src, mask) = (s.decode_rgba(), m.decode_rgba());
    let mut out = src.clone();
    for (i, texel) in src.chunks_exact(4).enumerate() {
        let k = mask[i * 4];
        if k == 0 {
            continue;
        }
        let f = f32::from(k) * 0.003_921_569;
        let tinted: [u8; 4] =
            std::array::from_fn(|c| (f32::from(paint[c]) * 0.003_921_569 * f32::from(texel[c])) as i32 as u8);
        let w = weigh(&[[texel[0], texel[1], texel[2], texel[3]], tinted], &[1.0 - f, f], false);
        out[i * 4..i * 4 + 3].copy_from_slice(&w[..3]);
    }
    Some(Texture { format: PixelFormat::Rgba8, mip_count: 1, data: out.into(), palette: Vec::new(), ..d.clone() })
}

/// What the rim's (or spinner's) source texture is drawn as (
/// **verified**): the remap table sends `<base>_WHEEL` to DUMMY_WHEEL%d /
/// DUMMY_SPINNER%d whenever the rim part exists and the type has a
/// DUMMY_SKIN slot (0x631EB7…0x631EDB: +0x23C = the source; 0x631F44…
/// 0x631F56: +0x29C / +0x2A0 = render info +0x588 / +0x58C), whatever
/// 0x61DEA0 did — its callers (0x6343F2, 0x634417, 0x61E0C8, 0x61E0E9)
/// ignore its result. So a composite that bails (sizes or bit depths
/// differ, the source's mask missing) leaves the rim drawn with the dummy
/// as loaded (the clone's composite is its own texture, `made`, which the
/// game writes into the dummy). **Open**: with the dummy itself missing
/// (0x61DEA0 returns 1) the remap names a texture that does not exist; the
/// clone keeps the source then.
pub fn wheel_remap_target(made: Option<u32>, dummy: u32, dummy_loaded: bool) -> Option<u32> {
    made.or_else(|| dummy_loaded.then_some(dummy))
}

/// One texel of [`composite_wheel`]: the source weighed with its paint tint by the mask byte.
pub fn wheel_texel(texel: &[u8], k: u8, paint: [u8; 4]) -> [u8; 4] {
    if k == 0 {
        return [texel[0], texel[1], texel[2], texel[3]];
    }
    let f = f32::from(k) * 0.003_921_569;
    let tinted: [u8; 4] =
        std::array::from_fn(|c| (f32::from(paint[c]) * 0.003_921_569 * f32::from(texel[c])) as i32 as u8);
    let w = weigh(&[[texel[0], texel[1], texel[2], texel[3]], tinted], &[1.0 - f, f], false);
    [w[0], w[1], w[2], texel[3]]
}

/// [`composite_wheel`] on the car pack's upscaled source and mask (both
/// present, one size): RGBA8 with an alpha-weighted mip chain
/// (`Texture::mip_count` > 1; the renderer uploads every level).
pub fn composite_wheel_pack(d: &Texture, s: &Texture, m: &Texture, paint: [u8; 4]) -> Option<Texture> {
    let (sw, sh, src) = crate::render::enh::assets::texture_pack::car_pack_rgba(s)?;
    let (mw, mh, mask) = crate::render::enh::assets::texture_pack::car_pack_rgba(m)?;
    if (sw, sh) != (mw, mh) {
        return None;
    }
    let mut out = src.clone();
    for (i, texel) in src.chunks_exact(4).enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&wheel_texel(texel, mask[i * 4], paint));
    }
    let levels = crate::upscale::mip_chain(&out, sw, sh);
    let mips = levels.len() as u32;
    let data: Vec<u8> = levels.into_iter().flat_map(|l| l.2).collect();
    Some(Texture {
        format: crate::formats::texture::PixelFormat::Rgba8,
        width: sw,
        height: sh,
        mip_count: mips,
        data: data.into(),
        palette: Vec::new(),
        ..d.clone()
    })
}

#[cfg(test)]
mod remap_tests {
    use super::*;

    /// 0x631EDB / 0x631F4A send the rim's source to the dummy
    /// whatever 0x61DEA0 did; a bailed composite draws the dummy, not the
    /// source.
    #[test]
    fn a_bailed_wheel_composite_still_draws_the_dummy() {
        let dummy = bin_hash("DUMMY_WHEEL1");
        assert_eq!(wheel_remap_target(None, dummy, true), Some(dummy));
        assert_eq!(wheel_remap_target(Some(7), dummy, true), Some(7));
        assert_eq!(wheel_remap_target(None, dummy, false), None);
    }
}
