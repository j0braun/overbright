//! The optional upscaled world texture pack (clone deviation, opt-in;
//! docs/modding/texture-pack-generator.md). Nothing is shipped: `overbright-upscale`
//! reads the player's own world textures and writes `<HASH>.dds` files for
//! the existing world pack path (`NFSU2_WORLD_TEXTURE_PACK`, `render::
//! texture_pack`).
//!
//! The method is deterministic and non-generative (no model, nothing is
//! invented): Lanczos-3 (wrap-around, as the game's WRAP sampler) → an
//! optional unsharp mask → an anti-ringing clamp to the source texels
//! under each output texel → iterative back-projection so that the s×s box
//! average of the result equals the source texel (the original's colours
//! stay exact at low frequencies; checked as ΔE in CIELAB) → BC1/BC3 with
//! a full alpha-weighted mip chain. Per category: roads sharper and
//! deblocked (DXT1 4×4 seams), signs text-preserving (full clamp, no
//! sharpening, no deblock), cut-outs and decals with their alpha.
use crate::formats::texture::{ALPHA_TEST_REF, Blend, PixelFormat, Texture};
use crate::surface_maps::{Class, TextureInfo, classify};

/// Bumped when the output of [`upscale`] changes.
pub const GENERATOR_VERSION: u32 = 1;

/// What a world texture is for the generator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    /// Road surfaces (worldreflect draws, road names).
    Road,
    /// Painted signs, adverts and billboards: text must stay crisp.
    Sign,
    /// Façades, walls, metal, glass, plain props.
    Building,
    /// Ground, plants, terrain (opaque).
    Natural,
    /// Alpha-tested cut-outs (fences, foliage, railings).
    Cutout,
    /// Alpha-blended decals (grime, road paint overlays).
    Decal,
    /// Not upscaled: glows, sky, effects, animated, water, emissive.
    Skip,
    /// G7b2, the car domain ([`car_category`]): traffic cars' shipped
    /// skins (opaque; the alpha is car.efx's gloss weight, data).
    CarSkin,
    /// Car parts drawn as they are: engine, misc, badges' backing.
    CarPart,
    /// Lamp lenses and light bars (opaque head/brake/centre lights): rejected
    /// by eye ([`NO_GAIN`]), their dithered art turns grainier.
    CarLamp,
    /// Rim faces and spinners (alpha-tested or blended discs).
    CarRim,
    /// The rims' paint masks (`_WHEEL_INNER_MASK`, spinner `_MASK`): data
    /// for the wheel compositor, upscaled with the rim so both stay aligned.
    CarMask,
    /// Sponsor decals and badging: lettering, kept like signs.
    CarDecal,
    /// Window glass (GLOBALB WINDOW*, REAR_DEFROSTER).
    CarGlass,
    /// docs/original/loading-image.md, the UI domain: the loading screens'
    /// pictures and bars the HD UI pack leaves out, at its 4× (up to 4096,
    /// the UI loader's limit); lettering kept like signs (no sharpening,
    /// no halos).
    UiPicture,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// Largest output side; the scale is 4 or 2, whichever fits.
    pub max_side: u32,
    /// Smooth DXT 4×4 block seams whose step is under this (0..1), 0 = off.
    pub deblock: f32,
    /// Unsharp-mask amount at the output scale.
    pub sharpen: f32,
    /// 0..1: how far an output texel is pulled into the min/max of the
    /// source texels around it (1 = no halos at all).
    pub antiring: f32,
}

impl Category {
    pub fn params(self) -> Params {
        match self {
            Category::Road => Params { max_side: 2048, deblock: 0.06, sharpen: 0.4, antiring: 0.85 },
            Category::Sign => Params { max_side: 2048, deblock: 0.0, sharpen: 0.0, antiring: 1.0 },
            Category::Building => Params { max_side: 1024, deblock: 0.04, sharpen: 0.35, antiring: 0.8 },
            Category::Natural => Params { max_side: 1024, deblock: 0.06, sharpen: 0.0, antiring: 0.7 },
            Category::Cutout => Params { max_side: 1024, deblock: 0.04, sharpen: 0.0, antiring: 0.9 },
            Category::Decal => Params { max_side: 1024, deblock: 0.06, sharpen: 0.0, antiring: 0.9 },
            Category::Skip => Params { max_side: 0, deblock: 0.0, sharpen: 0.0, antiring: 1.0 },
            Category::CarSkin => Params { max_side: 512, deblock: 0.04, sharpen: 0.25, antiring: 0.85 },
            Category::CarPart => Params { max_side: 1024, deblock: 0.04, sharpen: 0.0, antiring: 0.9 },
            Category::CarLamp => Params { max_side: 1024, deblock: 0.04, sharpen: 0.0, antiring: 0.9 },
            Category::CarRim => Params { max_side: 512, deblock: 0.0, sharpen: 0.0, antiring: 0.9 },
            Category::CarMask => Params { max_side: 512, deblock: 0.0, sharpen: 0.0, antiring: 1.0 },
            Category::CarDecal => Params { max_side: 512, deblock: 0.0, sharpen: 0.0, antiring: 1.0 },
            Category::CarGlass => Params { max_side: 512, deblock: 0.06, sharpen: 0.0, antiring: 0.9 },
            Category::UiPicture => Params { max_side: 4096, deblock: 0.04, sharpen: 0.35, antiring: 1.0 },
        }
    }
}

/// The category of a world texture (`reflect`: most of its draws are
/// worldreflect, a road surface). Uses Surface Detail's name/draw classes
/// (`surface_maps::classify`), except that alpha-blended decals are kept.
pub fn category(t: &Texture, reflect: bool) -> Category {
    let info = TextureInfo::of(t, reflect);
    let decal = info.blend == Blend::Alpha && !info.alpha_test;
    // Name-based skips (sky, glows, effects, animated) still apply to decals.
    let name = info.name.clone();
    let class = classify(&TextureInfo { blend: if decal { Blend::None } else { info.blend }, ..info });
    if class == Class::Skip {
        return Category::Skip;
    }
    if decal {
        return Category::Decal;
    }
    match class {
        Class::Asphalt => Category::Road,
        Class::Sign => Category::Sign,
        // Foliage and terrain, cut out or not.
        Class::Rough if t.alpha_test && !has_word(&name, &["ORG", "TRN", "TREE", "BUSH", "HEDGE", "LEAF", "LEAVES", "IVY", "PALM", "PINE", "GRASS", "HILLS", "FOLIAGE"]) => Category::Cutout,
        Class::Rough => Category::Natural,
        _ if t.alpha_test => Category::Cutout,
        _ => Category::Building,
    }
}

/// The category of a car texture (docs/modding/texture-pack-generator.md
/// "Cars"). `traffic_skin`: a traffic type's shipped `<CAR>_SKIN<n>`
/// (drawn as is). Left out (`Skip`): glows and lamp glass (additive, as
/// the world's glows), shadows, the neon, the compositor's dummies and the
/// skin layers (DOORLINE*, never drawn: composited into the 512² racer
/// skin, which keeps the original's size), carbon fibre, chrome and every
/// other skin (racers' skins are composited at load and are not in the
/// pack, so their look is untouched).
pub fn car_category(t: &Texture, traffic_skin: bool) -> Category {
    let n = t.name.to_ascii_uppercase();
    let has = |w: &str| n.contains(w);
    if t.blend == Blend::Additive || has("DUMMY_") || has("SHADOW") || has("NEON") || has("DOORLINE") || has("CARBON") || has("CHROME") || has("ENVMAP") || has("DAMAGE") {
        return Category::Skip;
    }
    if has("WINDOW") || has("DEFROST") {
        return Category::CarGlass;
    }
    if has("_SKIN") || n.starts_with("SKIN") {
        return if traffic_skin { Category::CarSkin } else { Category::Skip };
    }
    // Wheel masks before the wheels (`_WHEEL_INNER_MASK`, truncated names
    // keep `_WHEEL_IN…`; the spinner masks `_MASK` / `_OUTER_M…`).
    if (has("WHEEL_IN") && !n.ends_with("_WHEEL")) || (has("SPINNER") && (has("_MASK") || n.ends_with("_OUTER_M") || has("_OUTER_MA"))) {
        return Category::CarMask;
    }
    if has("WHEEL") || has("SPINNER") || has("_TIRE") || has("_OUTER") {
        return Category::CarRim;
    }
    if t.blend == Blend::Alpha && !t.alpha_test {
        return Category::CarDecal;
    }
    if has("LIGHT") || has("HEADL") || has("BRAKEL") || has("_HEAD") || has("_BRAK") {
        return Category::CarLamp;
    }
    Category::CarPart
}

fn has_word(name: &str, words: &[&str]) -> bool {
    name.split(['_', '.']).any(|t| words.iter().any(|w| t.starts_with(w)))
}

/// Categories left out of the pack after the review (docs/texture-pack-
/// generator.md "Review"): foliage, grass and terrain are noise-like
/// photo art; a non-generative upscale only turns each texel into a
/// sharper blob, which looked grainier and more pixelated than the GPU's
/// bilinear magnification of the original.
/// The car lamps likewise (review): their lenses are dithered
/// patterns; the upscale kept the rings' edges cleaner but sharpened the
/// dither into a coarser, grainier pattern.
pub const NO_GAIN: &[Category] = &[Category::Natural, Category::CarLamp];

/// Textures reviewed by eye and rejected (they looked worse than the
/// original in the review sheets): the loader falls back to the original.
pub const REJECTED: &[&str] = &[
    // City-light panoramas: blocky source art, the upscale only sharpens the blocks.
    "TRN_CITYGRIDB_DM_",
    "TRN_CITYGRIDC",
    // Noise-like grain (sand-blasted concrete, cobbles): grainier, not more detailed.
    "ARC_CONCFOUNDATION1",
    "RDP_ROADSTRIP_COBBLESTO",
];

/// Why a texture is not in the pack, or that it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Upscaled,
    /// Its category is not upscaled.
    Category,
    /// Under 16 texels a side (ramps, lookups).
    Small,
    /// The largest scale would exceed the category's size cap.
    Large,
    /// Nearly flat colour: nothing to gain, VRAM to lose.
    Flat,
    /// Low-frequency colour moved too much (ΔE check failed).
    Colour,
    /// Rejected by eye (`REJECTED`).
    Reviewed,
    /// Its category was rejected by eye (`NO_GAIN`).
    NoGain,
    /// The UI folder: the HD UI pack has its own file.
    Covered,
}

/// The scale (4 or 2) for a texture, or None when even 2× exceeds the cap.
pub fn scale_for(w: u32, h: u32, p: &Params) -> Option<u32> {
    [4, 2].into_iter().find(|s| w.max(h) * s <= p.max_side)
}

/// One channel plane, row-major, 0..1.
type Plane = Vec<f32>;

fn lanczos3(x: f32) -> f32 {
    let x = x.abs();
    if x < 1e-6 {
        1.0
    } else if x >= 3.0 {
        0.0
    } else {
        let px = std::f32::consts::PI * x;
        3.0 * px.sin() * (px / 3.0).sin() / (px * px)
    }
}

/// Taps (source offsets from the output texel's base, weights) for each
/// output phase of an s× upscale; `kernel` and its radius.
fn phase_taps(s: u32, kernel: fn(f32) -> f32, radius: i32) -> Vec<(i32, Vec<f32>)> {
    (0..s)
        .map(|p| {
            let u = (p as f32 + 0.5) / s as f32 - 0.5;
            let base = u.floor() as i32;
            let f = u - base as f32;
            let w: Vec<f32> = (1 - radius..=radius).map(|j| kernel(j as f32 - f)).collect();
            let sum: f32 = w.iter().sum();
            (base + 1 - radius, w.into_iter().map(|v| v / sum).collect())
        })
        .collect()
}

/// Separable s× upscale with wrap-around.
fn upsample(src: &[f32], w: usize, h: usize, s: usize, kernel: fn(f32) -> f32, radius: i32) -> Plane {
    let taps = phase_taps(s as u32, kernel, radius);
    let (ow, oh) = (w * s, h * s);
    let wrap = |i: i32, n: usize| i.rem_euclid(n as i32) as usize;
    let mut rows = vec![0.0f32; ow * h];
    for y in 0..h {
        let row = &src[y * w..][..w];
        for x in 0..ow {
            let (off, ws) = &taps[x % s];
            let base = (x / s) as i32 + off;
            rows[y * ow + x] = ws.iter().enumerate().map(|(k, wt)| wt * row[wrap(base + k as i32, w)]).sum();
        }
    }
    let mut out = vec![0.0f32; ow * oh];
    for y in 0..oh {
        let (off, ws) = &taps[y % s];
        let base = (y / s) as i32 + off;
        for (k, wt) in ws.iter().enumerate() {
            let r = &rows[wrap(base + k as i32, h) * ow..][..ow];
            for (o, v) in out[y * ow..][..ow].iter_mut().zip(r) {
                *o += wt * v;
            }
        }
    }
    out
}

fn tent(x: f32) -> f32 {
    (1.0 - x.abs()).max(0.0)
}

/// The s×s box average (what a mip level of the result shows at the
/// original's size).
pub fn box_down(src: &[f32], ow: usize, oh: usize, s: usize) -> Plane {
    let (w, h) = (ow / s, oh / s);
    let mut out = vec![0.0f32; w * h];
    let k = 1.0 / (s * s) as f32;
    for y in 0..oh {
        for x in 0..ow {
            out[(y / s) * w + x / s] += src[y * ow + x] * k;
        }
    }
    out
}

/// Separable Gaussian blur with wrap-around.
fn blur(src: &[f32], w: usize, h: usize, sigma: f32) -> Plane {
    let r = (sigma * 3.0).ceil() as i32;
    let k: Vec<f32> = (-r..=r).map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp()).collect();
    let sum: f32 = k.iter().sum();
    let k: Vec<f32> = k.into_iter().map(|v| v / sum).collect();
    let wrap = |i: i32, n: usize| i.rem_euclid(n as i32) as usize;
    let mut tmp = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            tmp[y * w + x] = k.iter().enumerate().map(|(j, kv)| kv * src[y * w + wrap(x as i32 + j as i32 - r, w)]).sum();
        }
    }
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            out[y * w + x] = k.iter().enumerate().map(|(j, kv)| kv * tmp[wrap(y as i32 + j as i32 - r, h) * w + x]).sum();
        }
    }
    out
}

/// Soften DXT 4×4 block seams: across each block boundary, a step under
/// `threshold` (in every channel) is split over the two texels (a quarter
/// each way); larger steps are real edges and stay.
fn deblock(planes: &mut [Plane], w: usize, h: usize, threshold: f32) {
    let n = planes.len();
    let step = |planes: &[Plane], a: usize, b: usize| (0..n).all(|c| (planes[c][b] - planes[c][a]).abs() < threshold);
    for x in (4..w).step_by(4) {
        for y in 0..h {
            let (a, b) = (y * w + x - 1, y * w + x);
            if step(planes, a, b) {
                for p in planes.iter_mut() {
                    let d = (p[b] - p[a]) * 0.25;
                    p[a] += d;
                    p[b] -= d;
                }
            }
        }
    }
    for y in (4..h).step_by(4) {
        for x in 0..w {
            let (a, b) = ((y - 1) * w + x, y * w + x);
            if step(planes, a, b) {
                for p in planes.iter_mut() {
                    let d = (p[b] - p[a]) * 0.25;
                    p[a] += d;
                    p[b] -= d;
                }
            }
        }
    }
}

/// What a texture's alpha means to the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaUse {
    /// Not coverage: opaque draws, where worldreflect reads it as the
    /// planar reflection's amount (roads). Every texel's colour shows.
    Data,
    /// Alpha-tested: texels at or under `ALPHA_TEST_REF` are discarded.
    Test,
    /// Alpha-blended: a texel's colour shows by its alpha.
    Blend,
}

impl AlphaUse {
    pub fn of(t: &Texture) -> AlphaUse {
        if t.alpha_test {
            AlphaUse::Test
        } else if t.blend == Blend::None {
            AlphaUse::Data
        } else {
            AlphaUse::Blend
        }
    }

    /// Whether a texel of alpha `a` (0..255) is seen at all.
    pub fn shows(self, a: u8) -> bool {
        match self {
            AlphaUse::Data => true,
            AlphaUse::Test => a > ALPHA_TEST_REF,
            AlphaUse::Blend => a > 0,
        }
    }
}

/// Fill the colour of texels that are never seen (`AlphaUse::shows`) from
/// their seen neighbours, so filtering never pulls their stray colours
/// (often black) into the kept edge.
fn dilate(rgb: &mut [Plane], alpha: &[f32], w: usize, h: usize, use_: AlphaUse) {
    if use_ == AlphaUse::Data {
        return;
    }
    let mut known: Vec<bool> = alpha.iter().map(|&a| use_.shows((a * 255.0).round() as u8)).collect();
    if known.iter().all(|&k| k) || !known.iter().any(|&k| k) {
        return;
    }
    for _ in 0..(w.max(h)) {
        let mut next = known.clone();
        let mut changed = false;
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if known[i] {
                    continue;
                }
                let mut acc = [0.0f32; 3];
                let mut n = 0;
                for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
                    let (nx, ny) = ((x as i32 + dx).rem_euclid(w as i32) as usize, (y as i32 + dy).rem_euclid(h as i32) as usize);
                    let j = ny * w + nx;
                    if known[j] {
                        for c in 0..3 {
                            acc[c] += rgb[c][j];
                        }
                        n += 1;
                    }
                }
                if n > 0 {
                    for c in 0..3 {
                        rgb[c][i] = acc[c] / n as f32;
                    }
                    next[i] = true;
                    changed = true;
                }
            }
        }
        known = next;
        if !changed {
            break;
        }
    }
}

/// Pull each output texel toward the min/max of the 2×2 source texels
/// around its centre (the ones bilinear filtering would use).
fn antiring(up: &mut [f32], src: &[f32], w: usize, h: usize, s: usize, k: f32) {
    if k <= 0.0 {
        return;
    }
    let ow = w * s;
    let lo_hi = |o: usize, n: usize| {
        let u = (o as f32 + 0.5) / s as f32 - 0.5;
        let i = u.floor() as i32;
        (i.rem_euclid(n as i32) as usize, (i + 1).rem_euclid(n as i32) as usize)
    };
    for oy in 0..h * s {
        let (y0, y1) = lo_hi(oy, h);
        for ox in 0..ow {
            let (x0, x1) = lo_hi(ox, w);
            let v = [src[y0 * w + x0], src[y0 * w + x1], src[y1 * w + x0], src[y1 * w + x1]];
            let lo = v.iter().copied().fold(f32::MAX, f32::min);
            let hi = v.iter().copied().fold(f32::MIN, f32::max);
            let o = &mut up[oy * ow + ox];
            *o += (o.clamp(lo, hi) - *o) * k;
        }
    }
}

/// Back-project: correct `up` until its s×s box average equals `target`
/// (tent-upsampled corrections, so no per-texel block steps appear),
/// keeping it in 0..1 and, for a full anti-ring clamp (signs, alpha), in
/// the source's local range (both sets hold the source itself).
fn back_project(up: &mut [f32], target: &[f32], w: usize, h: usize, s: usize, iterations: usize, ring: f32) {
    let (ow, oh) = (w * s, h * s);
    for _ in 0..iterations {
        let d = box_down(up, ow, oh, s);
        let e: Plane = target.iter().zip(&d).map(|(t, d)| t - d).collect();
        let c = upsample(&e, w, h, s, tent, 1);
        for (u, c) in up.iter_mut().zip(&c) {
            *u = (*u + c).clamp(0.0, 1.0);
        }
        if ring >= 1.0 {
            antiring(up, target, w, h, s, 1.0);
        }
    }
}

/// An upscaled texture: RGBA8, `width`×`height`.
pub struct Upscaled {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub scale: u32,
}

/// Whether the alpha channel is only 0 and 255 (DXT1 punch-through masks).
pub fn binary_alpha(rgba: &[u8]) -> bool {
    rgba.chunks_exact(4).all(|p| p[3] == 0 || p[3] == 255)
}

/// Upscale RGBA8 `w`×`h` by `s` (2 or 4) with `p`. `dxt`: the source was
/// block-compressed (deblock applies).
pub fn upscale(rgba: &[u8], w: u32, h: u32, s: u32, p: &Params, dxt: bool, alpha: AlphaUse) -> Upscaled {
    let (w, h, su) = (w as usize, h as usize, s as usize);
    let mut planes: Vec<Plane> = (0..4).map(|c| rgba.chunks_exact(4).map(|px| px[c] as f32 / 255.0).collect()).collect();
    let opaque = rgba.chunks_exact(4).all(|px| px[3] == 255);
    let binary = !opaque && binary_alpha(rgba);
    {
        let (rgb, a) = planes.split_at_mut(3);
        dilate(rgb, &a[0], w, h, alpha);
    }
    if dxt && p.deblock > 0.0 {
        deblock(&mut planes[..3], w, h, p.deblock);
    }
    let (ow, oh) = (w * su, h * su);
    let out: Vec<Plane> = {
        use rayon::prelude::*;
        planes
            .par_iter()
            .enumerate()
            .map(|(c, src)| {
                if c == 3 && opaque {
                    return vec![1.0; ow * oh];
                }
                let mut up = upsample(src, w, h, su, lanczos3, 3);
                if c < 3 && p.sharpen > 0.0 {
                    let b = blur(&up, ow, oh, 0.5 * su as f32);
                    for (u, b) in up.iter_mut().zip(&b) {
                        *u += p.sharpen * (*u - b);
                    }
                }
                let ring = if c == 3 { 1.0 } else { p.antiring };
                antiring(&mut up, src, w, h, su, ring);
                back_project(&mut up, src, w, h, su, 16, ring);
                if c == 3 && binary {
                    // Smooth contours at the new size, the mask's 0/255 kept.
                    for v in &mut up {
                        *v = if *v >= 0.5 { 1.0 } else { 0.0 };
                    }
                }
                up
            })
            .collect()
    };
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let mut out_rgba = vec![0u8; ow * oh * 4];
    for (i, px) in out_rgba.chunks_exact_mut(4).enumerate() {
        for c in 0..4 {
            px[c] = q(out[c][i]);
        }
    }
    Upscaled { width: ow as u32, height: oh as u32, rgba: out_rgba, scale: s }
}

fn srgb_to_lab(p: [f32; 3]) -> [f32; 3] {
    let lin = |c: f32| if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
    let (r, g, b) = (lin(p[0]), lin(p[1]), lin(p[2]));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f32| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// ΔE (CIE76) between the original and the s×s box average of the
/// upscale, over the texels the game shows ([`AlphaUse::shows`]): (mean, 99th percentile, max), and
/// the largest alpha difference (0..255).
pub fn low_frequency_delta_e(orig: &[u8], w: u32, h: u32, up: &Upscaled, alpha_use: AlphaUse) -> (f32, f32, f32, u8) {
    let s = up.scale as usize;
    let (ow, oh) = (up.width as usize, up.height as usize);
    let planes: Vec<Plane> = (0..4).map(|c| box_down(&up.rgba.chunks_exact(4).map(|p| p[c] as f32 / 255.0).collect::<Vec<_>>(), ow, oh, s)).collect();
    let mut de = Vec::with_capacity((w * h) as usize);
    let mut alpha = 0u8;
    for (i, o) in orig.chunks_exact(4).enumerate() {
        let a = (planes[3][i] * 255.0).round() as i32;
        alpha = alpha.max((a - o[3] as i32).unsigned_abs().min(255) as u8);
        if !alpha_use.shows(o[3]) {
            continue;
        }
        let l0 = srgb_to_lab([o[0] as f32 / 255.0, o[1] as f32 / 255.0, o[2] as f32 / 255.0]);
        let l1 = srgb_to_lab([planes[0][i], planes[1][i], planes[2][i]]);
        de.push(((l0[0] - l1[0]).powi(2) + (l0[1] - l1[1]).powi(2) + (l0[2] - l1[2]).powi(2)).sqrt());
    }
    if de.is_empty() {
        return (0.0, 0.0, 0.0, alpha);
    }
    let mean = de.iter().sum::<f32>() / de.len() as f32;
    de.sort_by(f32::total_cmp);
    let p99 = de[((de.len() - 1) as f32 * 0.99) as usize];
    (mean, p99, *de.last().unwrap(), alpha)
}

/// Limits of [`low_frequency_delta_e`] a pack texture must meet (the
/// deblocked roads move a little at block seams; under 1 is invisible).
pub const MAX_MEAN_DELTA_E: f32 = 1.0;
pub const MAX_P99_DELTA_E: f32 = 4.0;

/// Spread of luma (standard deviation, 0..1): under this the texture is
/// flat colour and gains nothing.
pub const FLAT_LUMA_STD: f32 = 0.006;

pub fn luma_std(rgba: &[u8]) -> f32 {
    let l: Vec<f32> = rgba.chunks_exact(4).map(|p| (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0).collect();
    let m = l.iter().sum::<f32>() / l.len().max(1) as f32;
    (l.iter().map(|v| (v - m) * (v - m)).sum::<f32>() / l.len().max(1) as f32).sqrt()
}

/// Up to which size a texture must be to be worth upscaling.
pub const MIN_SIDE: u32 = 16;

/// The generator's decision for one texture before upscaling.
pub fn pre_verdict(t: &Texture, cat: Category, rgba: &[u8]) -> Result<u32, Verdict> {
    if cat == Category::Skip {
        return Err(Verdict::Category);
    }
    if NO_GAIN.contains(&cat) {
        return Err(Verdict::NoGain);
    }
    if REJECTED.iter().any(|n| n.eq_ignore_ascii_case(&t.name)) {
        return Err(Verdict::Reviewed);
    }
    if t.width.min(t.height) < MIN_SIDE {
        return Err(Verdict::Small);
    }
    let Some(s) = scale_for(t.width, t.height, &cat.params()) else { return Err(Verdict::Large) };
    if luma_std(rgba) < FLAT_LUMA_STD {
        return Err(Verdict::Flat);
    }
    Ok(s)
}

/// Whether the source is block-compressed (DXT seams to soften).
pub fn is_dxt(t: &Texture) -> bool {
    matches!(t.format, PixelFormat::Dxt1 | PixelFormat::Dxt3 | PixelFormat::Dxt5)
}

// ---------------------------------------------------------------- mips + BC

/// The full mip chain (down to 1×1) of RGBA8 `w`×`h`: 2×2 boxes, colour
/// weighted by alpha (no colour from discarded texels), straight alpha out.
pub fn mip_chain(rgba: &[u8], w: u32, h: u32) -> Vec<(u32, u32, Vec<u8>)> {
    let mut levels = vec![(w, h, rgba.to_vec())];
    while let Some((lw, lh, l)) = levels.last().filter(|(lw, lh, _)| *lw > 1 || *lh > 1) {
        let (nw, nh) = ((lw / 2).max(1), (lh / 2).max(1));
        let (lw, lh) = (*lw as usize, *lh as usize);
        let mut next = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh as usize {
            for x in 0..nw as usize {
                let mut acc = [0.0f32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = ((2 * x + dx).min(lw - 1), (2 * y + dy).min(lh - 1));
                    let p = &l[(sy * lw + sx) * 4..][..4];
                    let a = p[3] as f32;
                    for c in 0..3 {
                        acc[c] += p[c] as f32 * a;
                    }
                    acc[3] += a;
                }
                let o = &mut next[(y * nw as usize + x) * 4..][..4];
                if acc[3] > 0.0 {
                    for c in 0..3 {
                        o[c] = (acc[c] / acc[3]).round() as u8;
                    }
                } else {
                    // Fully transparent: keep the plain colour average.
                    for c in 0..3 {
                        let mut v = 0u32;
                        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                            let (sx, sy) = ((2 * x + dx).min(lw - 1), (2 * y + dy).min(lh - 1));
                            v += l[(sy * lw + sx) * 4 + c] as u32;
                        }
                        o[c] = ((v + 2) / 4) as u8;
                    }
                }
                o[3] = (acc[3] / 4.0).round() as u8;
            }
        }
        levels.push((nw, nh, next));
    }
    levels
}

fn to565(c: [f32; 3]) -> u16 {
    let r = (c[0].clamp(0.0, 255.0) * 31.0 / 255.0).round() as u16;
    let g = (c[1].clamp(0.0, 255.0) * 63.0 / 255.0).round() as u16;
    let b = (c[2].clamp(0.0, 255.0) * 31.0 / 255.0).round() as u16;
    (r << 11) | (g << 5) | b
}

fn from565(c: u16) -> [f32; 3] {
    let (r, g, b) = ((c >> 11) & 31, (c >> 5) & 63, c & 31);
    [((r << 3) | (r >> 2)) as f32, ((g << 2) | (g >> 4)) as f32, ((b << 3) | (b >> 2)) as f32]
}

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// One BC1-style colour block (8 bytes) for `px` (`use_px[i]` false =
/// transparent in punch-through mode). `three`: 3-colour + transparent mode.
fn encode_colour_block(px: &[[u8; 4]; 16], use_px: &[bool; 16], three: bool) -> [u8; 8] {
    let pts: Vec<[f32; 3]> = (0..16).filter(|&i| use_px[i]).map(|i| [px[i][0] as f32, px[i][1] as f32, px[i][2] as f32]).collect();
    let mut out = [0u8; 8];
    if pts.is_empty() {
        // All transparent (3-colour mode, index 3).
        out[0..2].copy_from_slice(&0u16.to_le_bytes());
        out[2..4].copy_from_slice(&0u16.to_le_bytes());
        out[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        return out;
    }
    let n = pts.len() as f32;
    let mean = pts.iter().fold([0.0f32; 3], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n, a[2] + p[2] / n]);
    let mut cov = [[0.0f32; 3]; 3];
    for p in &pts {
        let d = [p[0] - mean[0], p[1] - mean[1], p[2] - mean[2]];
        for i in 0..3 {
            for j in 0..3 {
                cov[i][j] += d[i] * d[j];
            }
        }
    }
    let mut axis = [0.577f32, 0.577, 0.577];
    for _ in 0..8 {
        let v = [0, 1, 2].map(|i| cov[i][0] * axis[0] + cov[i][1] * axis[1] + cov[i][2] * axis[2]);
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if l < 1e-6 {
            break;
        }
        axis = [v[0] / l, v[1] / l, v[2] / l];
    }
    let proj = |p: &[f32; 3]| (p[0] - mean[0]) * axis[0] + (p[1] - mean[1]) * axis[1] + (p[2] - mean[2]) * axis[2];
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for p in &pts {
        let t = proj(p);
        lo = lo.min(t);
        hi = hi.max(t);
    }
    let at = |t: f32| [mean[0] + axis[0] * t, mean[1] + axis[1] * t, mean[2] + axis[2] * t];
    // Weights of endpoint a per index (b gets 1 − w).
    let weights: [f32; 4] = if three { [1.0, 0.0, 0.5, 0.0] } else { [1.0, 0.0, 2.0 / 3.0, 1.0 / 3.0] };
    let levels = if three { 3 } else { 4 };
    let evaluate = |a: [f32; 3], b: [f32; 3]| {
        let (mut ca, mut cb) = (to565(a), to565(b));
        if three && ca > cb || !three && ca < cb {
            std::mem::swap(&mut ca, &mut cb);
        }
        if !three && ca == cb {
            // Equal endpoints would select 3-colour mode: nudge.
            if cb > 0 { cb -= 1 } else { ca += 1 }
        }
        let (fa, fb) = (from565(ca), from565(cb));
        let pal: Vec<[f32; 3]> = (0..levels).map(|k| { let w = weights[k]; [fa[0] * w + fb[0] * (1.0 - w), fa[1] * w + fb[1] * (1.0 - w), fa[2] * w + fb[2] * (1.0 - w)] }).collect();
        let mut idx = [3u32; 16];
        let mut err = 0.0;
        for i in 0..16 {
            if !use_px[i] {
                continue;
            }
            let p = [px[i][0] as f32, px[i][1] as f32, px[i][2] as f32];
            let (k, e) = pal.iter().enumerate().map(|(k, q)| (k, dist2(p, *q))).min_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
            idx[i] = k as u32;
            err += e;
        }
        (err, ca, cb, idx)
    };
    let mut best = evaluate(at(hi), at(lo));
    // Least-squares refinement of the endpoints for the chosen indices.
    for _ in 0..2 {
        let (_, ca, cb, idx) = best;
        let (mut aa, mut ab, mut bb) = (0.0f32, 0.0f32, 0.0f32);
        let (mut ax, mut bx) = ([0.0f32; 3], [0.0f32; 3]);
        for i in 0..16 {
            if !use_px[i] {
                continue;
            }
            let w = weights[idx[i] as usize];
            let v = 1.0 - w;
            aa += w * w;
            ab += w * v;
            bb += v * v;
            for c in 0..3 {
                ax[c] += w * px[i][c] as f32;
                bx[c] += v * px[i][c] as f32;
            }
        }
        let det = aa * bb - ab * ab;
        if det.abs() < 1e-6 {
            break;
        }
        let a = [0, 1, 2].map(|c| (ax[c] * bb - bx[c] * ab) / det);
        let b = [0, 1, 2].map(|c| (bx[c] * aa - ax[c] * ab) / det);
        let trial = evaluate(a, b);
        if trial.0 < best.0 {
            best = trial;
        } else {
            let _ = (ca, cb);
            break;
        }
    }
    let (_, ca, cb, idx) = best;
    let bits = idx.iter().enumerate().fold(0u32, |acc, (i, &k)| acc | (k << (2 * i)));
    out[0..2].copy_from_slice(&ca.to_le_bytes());
    out[2..4].copy_from_slice(&cb.to_le_bytes());
    out[4..8].copy_from_slice(&bits.to_le_bytes());
    out
}

/// One BC3 alpha block (8 bytes): the better of the 8-level and the
/// 6-level (+0/255) modes.
fn encode_alpha_block(a: &[u8; 16]) -> [u8; 8] {
    let try_mode = |a0: u8, a1: u8| {
        let (x, y) = (a0 as u32, a1 as u32);
        let table: [u8; 8] = std::array::from_fn(|i| match i {
            0 => a0,
            1 => a1,
            _ if a0 > a1 => (((8 - i as u32) * x + (i as u32 - 1) * y + 3) / 7) as u8,
            6 => 0,
            7 => 255,
            _ => (((6 - i as u32) * x + (i as u32 - 1) * y + 2) / 5) as u8,
        });
        let mut bits = 0u64;
        let mut err = 0u32;
        for (i, &v) in a.iter().enumerate() {
            let (k, e) = table.iter().enumerate().map(|(k, &t)| (k, (t as i32 - v as i32).unsigned_abs())).min_by_key(|e| e.1).unwrap();
            bits |= (k as u64) << (3 * i);
            err += e * e;
        }
        let mut out = [0u8; 8];
        out[0] = a0;
        out[1] = a1;
        out[2..8].copy_from_slice(&bits.to_le_bytes()[..6]);
        (err, out)
    };
    let (lo, hi) = (*a.iter().min().unwrap(), *a.iter().max().unwrap());
    let eight = if hi > lo { try_mode(hi, lo) } else { try_mode(hi, lo) };
    let inner: Vec<u8> = a.iter().copied().filter(|&v| v != 0 && v != 255).collect();
    let six = match (inner.iter().min(), inner.iter().max()) {
        (Some(&l), Some(&h)) => try_mode(l, h.max(l)),
        _ => try_mode(0, 255),
    };
    if six.0 < eight.0 { six.1 } else { eight.1 }
}

fn blocks(rgba: &[u8], w: u32, h: u32) -> impl Iterator<Item = [[u8; 4]; 16]> + '_ {
    let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
    (0..bh).flat_map(move |by| {
        (0..bw).map(move |bx| {
            std::array::from_fn(|i| {
                let (x, y) = ((bx * 4 + i as u32 % 4).min(w - 1), (by * 4 + i as u32 / 4).min(h - 1));
                let p = &rgba[((y * w + x) * 4) as usize..][..4];
                [p[0], p[1], p[2], p[3]]
            })
        })
    })
}

/// BC1 (opaque, or punch-through when some alpha is under 128).
pub fn encode_bc1(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((w.div_ceil(4) * h.div_ceil(4) * 8) as usize);
    for b in blocks(rgba, w, h) {
        let keep: [bool; 16] = std::array::from_fn(|i| b[i][3] >= 128);
        let three = keep.iter().any(|k| !k);
        out.extend_from_slice(&encode_colour_block(&b, &keep, three));
    }
    out
}

/// BC3 (interpolated alpha).
pub fn encode_bc3(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((w.div_ceil(4) * h.div_ceil(4) * 16) as usize);
    for b in blocks(rgba, w, h) {
        let a: [u8; 16] = std::array::from_fn(|i| b[i][3]);
        out.extend_from_slice(&encode_alpha_block(&a));
        out.extend_from_slice(&encode_colour_block(&b, &[true; 16], false));
    }
    out
}

/// BC2 / DXT3 (explicit 4-bit alpha), for TexMod imports (
/// `tpf_import`) whose DXT3 art lacks some mip levels.
pub fn encode_bc2(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((w.div_ceil(4) * h.div_ceil(4) * 16) as usize);
    for b in blocks(rgba, w, h) {
        let mut a = [0u8; 8];
        for (i, p) in b.iter().enumerate() {
            // Round to 4 bits (x * 15 / 255).
            let q = ((p[3] as u32 * 15 + 127) / 255) as u8;
            a[i / 2] |= q << ((i % 2) * 4);
        }
        out.extend_from_slice(&a);
        out.extend_from_slice(&encode_colour_block(&b, &[true; 16], false));
    }
    out
}

/// docs/modding/texture-pack-cache.md: a DXT3 / DXT5 image (`data`: every
/// level's blocks back to back) whose every texel decodes to alpha 255, as
/// BC1 without loss: its colour blocks are BC1 blocks the 4-colour way
/// (D3D decodes a DXT3/5 colour block in 4-colour mode whatever its end
/// points' order), so each is kept when colour0 > colour1, has its end points
/// swapped and every index's low bit flipped when colour0 < colour1 (the
/// same four colours, listed the other way), and all indices set to 0 when
/// they are equal (all four colours are that colour; BC1's 3-colour mode
/// then never reaches its transparent index). Half the bytes; the same
/// texels. None if a texel is not opaque (or the data is not whole blocks).
pub fn opaque_to_bc1(format: PixelFormat, data: &[u8]) -> Option<Vec<u8>> {
    if !matches!(format, PixelFormat::Dxt3 | PixelFormat::Dxt5) || data.is_empty() || data.len() % 16 != 0 {
        return None;
    }
    let opaque = |b: &[u8]| match format {
        PixelFormat::Dxt3 => b[..8].iter().all(|&a| a == 0xFF),
        _ => {
            let (a0, a1) = (b[0], b[1]);
            let bits = u64::from_le_bytes([b[2], b[3], b[4], b[5], b[6], b[7], 0, 0]);
            // Index 0 is a0, 1 is a1; 7 is 255 in the 6-value mode (a0 <= a1),
            // 6 is 0 there; the interpolated ones reach 255 only between two
            // end points of 255.
            (0..16).all(|i| match (bits >> (3 * i)) & 7 {
                0 => a0 == 255,
                1 => a1 == 255,
                6 if a0 <= a1 => false,
                7 if a0 <= a1 => true,
                _ => a0 == 255 && a1 == 255,
            })
        }
    };
    if !data.chunks_exact(16).all(opaque) {
        return None;
    }
    let mut out = Vec::with_capacity(data.len() / 2);
    for b in data.chunks_exact(16) {
        let c = &b[8..16];
        let (c0, c1) = (u16::from_le_bytes([c[0], c[1]]), u16::from_le_bytes([c[2], c[3]]));
        let idx = u32::from_le_bytes([c[4], c[5], c[6], c[7]]);
        let (c0, c1, idx) = match c0.cmp(&c1) {
            std::cmp::Ordering::Greater => (c0, c1, idx),
            std::cmp::Ordering::Less => (c1, c0, idx ^ 0x5555_5555),
            std::cmp::Ordering::Equal => (c0, c1, 0),
        };
        out.extend_from_slice(&c0.to_le_bytes());
        out.extend_from_slice(&c1.to_le_bytes());
        out.extend_from_slice(&idx.to_le_bytes());
    }
    Some(out)
}

/// A pack texture: its format (Dxt1, Dxt3 or Dxt5), size and every mip level
/// back to back (the layout `Texture::data` and DDS files use).
pub struct Encoded {
    pub format: PixelFormat,
    pub width: u32,
    pub height: u32,
    pub mips: u32,
    pub data: Vec<u8>,
}

/// BC1 when the alpha is opaque or a 0/255 mask, else BC3; full mip chain.
pub fn encode(up: &Upscaled) -> Encoded {
    let bc1 = binary_alpha(&up.rgba);
    let levels = mip_chain(&up.rgba, up.width, up.height);
    let mut data = Vec::new();
    for (w, h, l) in &levels {
        // A mask's lower levels stay a mask for BC1 (alpha ≥ 128 kept).
        data.extend(if bc1 { encode_bc1(l, *w, *h) } else { encode_bc3(l, *w, *h) });
    }
    Encoded { format: if bc1 { PixelFormat::Dxt1 } else { PixelFormat::Dxt5 }, width: up.width, height: up.height, mips: levels.len() as u32, data }
}

/// A DDS file (DX9 header, FourCC DXT1/DXT3/DXT5, all mips).
pub fn dds_bytes(e: &Encoded) -> Vec<u8> {
    let mut h = [0u32; 32];
    h[0] = u32::from_le_bytes(*b"DDS ");
    h[1] = 124;
    h[2] = 0x1 | 0x2 | 0x4 | 0x1000 | 0x20000 | 0x80000;
    h[3] = e.height;
    h[4] = e.width;
    h[5] = e.format.level_size(e.width, e.height) as u32;
    h[7] = e.mips;
    h[19] = 32;
    h[20] = 0x4;
    h[21] = u32::from_le_bytes(match e.format {
        PixelFormat::Dxt1 => *b"DXT1",
        PixelFormat::Dxt3 => *b"DXT3",
        _ => *b"DXT5",
    });
    h[27] = 0x1000 | 0x400000 | 0x8;
    let mut out: Vec<u8> = h.iter().flat_map(|v| v.to_le_bytes()).collect();
    out.extend_from_slice(&e.data);
    out
}

/// Parse a DDS written by [`dds_bytes`] (or any DX9 DXT1/DXT3/DXT5 DDS with
/// sizes that are multiples of 4): format, size, mips and the levels.
pub fn parse_dds(bytes: &[u8]) -> Option<Encoded> {
    let u = |i: usize| bytes.get(i * 4..i * 4 + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    if bytes.len() < 128 || &bytes[..4] != b"DDS " || u(1)? != 124 || u(20)? & 0x4 == 0 {
        return None;
    }
    let (height, width) = (u(3)?, u(4)?);
    let format = match &bytes[84..88] {
        b"DXT1" => PixelFormat::Dxt1,
        b"DXT3" => PixelFormat::Dxt3,
        b"DXT5" => PixelFormat::Dxt5,
        _ => return None,
    };
    if width == 0 || height == 0 || width % 4 != 0 || height % 4 != 0 {
        return None;
    }
    let full = 32 - width.max(height).leading_zeros();
    let mips = if u(2)? & 0x20000 != 0 { u(7)?.clamp(1, full) } else { 1 };
    let size: usize = (0..mips).map(|m| format.level_size((width >> m).max(1), (height >> m).max(1))).sum();
    let data = bytes.get(128..128 + size)?.to_vec();
    Some(Encoded { format, width, height, mips, data })
}

impl Encoded {
    /// As a [`Texture`] (for decoding with the game's own BC decoder).
    pub fn as_texture(&self, like: &Texture) -> Texture {
        Texture { width: self.width, height: self.height, format: self.format, mip_count: self.mips, data: self.data.clone().into(), palette: Vec::new(), ..like.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opaque DXT3 / DXT5 blocks become BC1 blocks that decode to the
    /// same texels (all three end-point orders), and non-opaque ones stay.
    #[test]
    fn opaque_dxt_to_bc1_is_lossless() {
        let mut rgba: Vec<u8> = (0..32 * 16).flat_map(|i: u32| [(i * 7) as u8, (i * 3 + 40) as u8, (255 - i) as u8, 255]).collect();
        // A flat block (equal end points) and a two-colour one.
        for y in 0..4 {
            for x in 0..4 {
                rgba[((y * 32 + x) * 4) as usize..][..3].copy_from_slice(&[10, 20, 30]);
                rgba[((y * 32 + 4 + x) * 4) as usize..][..3].copy_from_slice(if x < 2 { &[0, 0, 0] } else { &[250, 250, 250] });
            }
        }
        let like = Texture { name: "t".into(), hash: 1, width: 32, height: 16, format: PixelFormat::Rgba8, mip_count: 1, data: Vec::new().into(), palette: vec![], alpha_test: false, blend: Blend::None, rendering_order: 0, late_buckets: false, scroll: Default::default() };
        for format in [PixelFormat::Dxt3, PixelFormat::Dxt5] {
            let mut data = if format == PixelFormat::Dxt3 { encode_bc2(&rgba, 32, 16) } else { encode_bc3(&rgba, 32, 16) };
            // Every end-point order: swap some blocks' colours by hand (the
            // 4-colour palette listed the other way), make one flat.
            for (k, b) in data.chunks_exact_mut(16).enumerate() {
                if k % 3 == 1 {
                    let c = b[8..16].to_vec();
                    b[8..10].copy_from_slice(&c[2..4]);
                    b[10..12].copy_from_slice(&c[0..2]);
                    let idx = u32::from_le_bytes([c[4], c[5], c[6], c[7]]) ^ 0x5555_5555;
                    b[12..16].copy_from_slice(&idx.to_le_bytes());
                }
                if k == 5 {
                    let c0 = [b[8], b[9]];
                    b[10..12].copy_from_slice(&c0);
                }
            }
            if format == PixelFormat::Dxt5 {
                // 6-value mode with index 7 (255) everywhere: still opaque.
                let b = &mut data[16..32];
                b[0] = 0;
                b[1] = 255;
                b[2..8].copy_from_slice(&[0xFF; 6]);
            }
            let src = Encoded { format, width: 32, height: 16, mips: 1, data: data.clone() }.as_texture(&like).decode_rgba();
            let bc1 = opaque_to_bc1(format, &data).expect("opaque");
            assert_eq!(bc1.len(), data.len() / 2);
            let out = Encoded { format: PixelFormat::Dxt1, width: 32, height: 16, mips: 1, data: bc1 }.as_texture(&like).decode_rgba();
            assert_eq!(out, src, "{format:?}");
            // One texel's alpha under 255: kept as it is.
            let mut a = data.clone();
            if format == PixelFormat::Dxt3 { a[0] = 0xEF } else { a[0] = 254; a[1] = 254 }
            assert!(opaque_to_bc1(format, &a).is_none());
        }
        assert!(opaque_to_bc1(PixelFormat::Dxt1, &[0; 16]).is_none());
    }

    fn tex(name: &str, w: u32, h: u32, rgba: Vec<u8>, blend: Blend, alpha_test: bool) -> Texture {
        Texture {
            name: name.into(), hash: 1, width: w, height: h, format: PixelFormat::Rgba8, mip_count: 1, data: rgba.into(),
            palette: vec![], alpha_test, blend, rendering_order: 0, late_buckets: false, scroll: Default::default(),
        }
    }

    fn noise(w: u32, h: u32, seed: u32) -> Vec<u8> {
        let mut s = seed;
        (0..w * h * 4).map(|i| if i % 4 == 3 { 255 } else { s = s.wrapping_mul(1664525).wrapping_add(1013904223); (s >> 24) as u8 }).collect()
    }

    /// Texture-like art: smooth shapes plus mid-range grain (40..215).
    fn art(w: u32, h: u32, seed: u32) -> Vec<u8> {
        let n = noise(w, h, seed);
        (0..(w * h) as usize).flat_map(|i| {
            let (x, y) = ((i as u32 % w) as f32, (i as u32 / w) as f32);
            let base = |c: f32| 127.0 + 60.0 * ((x * 0.4 + c).sin() * (y * 0.3 - c).cos());
            let g = |c: usize| (n[i * 4 + c] as f32 - 128.0) * 0.12;
            [(base(0.0) + g(0)) as u8, (base(1.0) + g(1)) as u8, (base(2.0) + g(2)) as u8, 255]
        }).collect()
    }

    #[test]
    fn categories_follow_names_blends_and_draws() {
        let px = vec![128u8; 64 * 4];
        assert_eq!(category(&tex("RDP_ROAD_A", 8, 8, px.clone(), Blend::None, false), true), Category::Road);
        assert_eq!(category(&tex("SGN_DINER", 8, 8, px.clone(), Blend::None, false), false), Category::Sign);
        assert_eq!(category(&tex("ARC_BRICKWALL", 8, 8, px.clone(), Blend::None, false), false), Category::Building);
        assert_eq!(category(&tex("OBJ_FENCE", 8, 8, px.clone(), Blend::None, true), false), Category::Cutout);
        assert_eq!(category(&tex("ORG_MAPLEA01", 8, 8, px.clone(), Blend::None, true), false), Category::Natural);
        assert_eq!(category(&tex("ORG_GRASS", 8, 8, px.clone(), Blend::None, false), false), Category::Natural);
        assert_eq!(category(&tex("OBJ_GRIME", 8, 8, px.clone(), Blend::Alpha, false), false), Category::Decal);
        assert_eq!(category(&tex("SFX_GLOW", 8, 8, px.clone(), Blend::Additive, false), false), Category::Skip);
        assert_eq!(category(&tex("SKYDOME", 8, 8, px.clone(), Blend::Alpha, false), false), Category::Skip);
        assert_eq!(category(&tex("ANM_SIGN", 8, 8, px, Blend::None, false), false), Category::Skip);
    }

    #[test]
    fn low_frequencies_are_kept_exactly() {
        for (s, cat) in [(2, Category::Road), (4, Category::Sign), (4, Category::Building)] {
            let src = art(16, 16, 7);
            let up = upscale(&src, 16, 16, s, &cat.params(), false, AlphaUse::Data);
            assert_eq!((up.width, up.height), (16 * s, 16 * s));
            let (mean, p99, _, alpha) = low_frequency_delta_e(&src, 16, 16, &up, AlphaUse::Data);
            assert!(mean < 0.5 && p99 < 2.0, "{cat:?} {s}: mean {mean} p99 {p99}");
            assert_eq!(alpha, 0);
            // Deblocked DXT sources move only at block seams, within the pack's limits.
            let up = upscale(&src, 16, 16, s, &cat.params(), true, AlphaUse::Data);
            let (mean, p99, _, _) = low_frequency_delta_e(&src, 16, 16, &up, AlphaUse::Data);
            assert!(mean < MAX_MEAN_DELTA_E && p99 < MAX_P99_DELTA_E, "{cat:?} {s} deblocked: mean {mean} p99 {p99}");
        }
    }

    #[test]
    fn road_alpha_is_data_not_coverage() {
        // Roads carry the reflection amount in alpha (often ~0): every
        // texel's colour is drawn, so nothing is filled from neighbours.
        let mut src = art(16, 16, 9);
        for (i, p) in src.chunks_exact_mut(4).enumerate() {
            p[3] = if i % 37 == 0 { 200 } else { 20 };
        }
        let up = upscale(&src, 16, 16, 4, &Category::Road.params(), false, AlphaUse::Data);
        let (mean, p99, _, alpha) = low_frequency_delta_e(&src, 16, 16, &up, AlphaUse::Data);
        assert!(mean < 0.5 && p99 < 2.0, "mean {mean} p99 {p99}");
        assert!(alpha <= 16, "an isolated alpha texel converges slowest: {alpha}");
    }

    #[test]
    fn signs_get_no_halos_and_stay_sharp() {
        // A black bar on white (text stroke): no texel darker than black or
        // brighter than white, and the edge stays within one source texel.
        let (w, h) = (32u32, 8u32);
        let src: Vec<u8> = (0..w * h).flat_map(|i| { let x = i % w; let v = if (12..18).contains(&x) { 0 } else { 255 }; [v, v, v, 255] }).collect();
        let up = upscale(&src, w, h, 4, &Category::Sign.params(), false, AlphaUse::Data);
        let row: Vec<u8> = (0..up.width).map(|x| up.rgba[((16 * up.width + x) * 4) as usize]).collect();
        assert!(row[..40].iter().all(|&v| v == 255), "halo before the stroke: {:?}", &row[..48]);
        assert!(row[52..68].iter().all(|&v| v == 0), "stroke core: {:?}", &row[44..76]);
        let ramp = row[40..80].iter().filter(|&&v| v > 0 && v < 255).count();
        assert!(ramp <= 8, "edges stay within a source texel each: {ramp}");
    }

    #[test]
    fn masks_stay_masks_and_cutout_colour_does_not_bleed() {
        // A 0/255 mask whose discarded texels are black: the result stays
        // 0/255, the kept texels keep their colour (no dark fringe).
        let (w, h) = (16u32, 16u32);
        let src: Vec<u8> = (0..w * h).flat_map(|i| { let (x, y) = (i % w, i / w); if (x as i32 - 8).pow(2) + (y as i32 - 8).pow(2) < 30 { [200, 180, 40, 255] } else { [0, 0, 0, 0] } }).collect();
        let up = upscale(&src, w, h, 4, &Category::Cutout.params(), true, AlphaUse::Test);
        assert!(binary_alpha(&up.rgba));
        let kept: Vec<&[u8]> = up.rgba.chunks_exact(4).filter(|p| p[3] == 255).collect();
        assert!(kept.iter().all(|p| p[0] >= 190 && p[2] <= 50), "fringe");
        // Coverage within 5 % of the source.
        let c0 = src.chunks_exact(4).filter(|p| p[3] == 255).count() as f32 * 16.0;
        let c1 = kept.len() as f32;
        assert!((c1 / c0 - 1.0).abs() < 0.05, "{c0} {c1}");
        let e = encode(&up);
        assert_eq!(e.format, PixelFormat::Dxt1);
    }

    #[test]
    fn bc_round_trip_is_close_and_dds_parses() {
        let src = art(8, 8, 3);
        let up = upscale(&src, 8, 8, 4, &Category::Building.params(), false, AlphaUse::Data);
        let e = encode(&up);
        assert_eq!((e.format, e.width, e.height, e.mips), (PixelFormat::Dxt1, 32, 32, 6));
        let back = parse_dds(&dds_bytes(&e)).unwrap();
        assert_eq!((back.format, back.width, back.height, back.mips), (e.format, e.width, e.height, e.mips));
        assert_eq!(back.data, e.data);
        let like = tex("X", 1, 1, vec![0; 4], Blend::None, false);
        let rgba = back.as_texture(&like).decode_rgba();
        let rmse = (rgba.iter().zip(&up.rgba).map(|(a, b)| (*a as f32 - *b as f32).powi(2)).sum::<f32>() / rgba.len() as f32).sqrt();
        assert!(rmse < 6.0, "BC1 rmse {rmse}");
        // Smooth alpha goes to BC3; its alpha survives within a few levels.
        let mut soft = up.rgba.clone();
        for (i, p) in soft.chunks_exact_mut(4).enumerate() {
            p[3] = (i % 32 * 8) as u8;
        }
        let e3 = encode(&Upscaled { rgba: soft.clone(), ..up });
        assert_eq!(e3.format, PixelFormat::Dxt5);
        let rgba3 = e3.as_texture(&like).decode_rgba();
        assert!(rgba3.chunks_exact(4).zip(soft.chunks_exact(4)).all(|(a, b)| (a[3] as i32 - b[3] as i32).abs() <= 6));
    }

    #[test]
    fn flat_small_and_skipped_textures_are_left_out() {
        let flat = tex("ARC_WALL", 64, 64, vec![100; 64 * 64 * 4], Blend::None, false);
        assert_eq!(pre_verdict(&flat, Category::Building, &flat.data), Err(Verdict::Flat));
        let small = tex("ARC_WALL", 8, 64, noise(8, 64, 1), Blend::None, false);
        assert_eq!(pre_verdict(&small, Category::Building, &small.data), Err(Verdict::Small));
        let big = tex("ARC_WALL", 1024, 64, noise(1024, 64, 1), Blend::None, false);
        assert_eq!(pre_verdict(&big, Category::Building, &big.data), Err(Verdict::Large));
        let ok = tex("RDP_ROAD", 128, 128, noise(128, 128, 1), Blend::None, false);
        assert_eq!(pre_verdict(&ok, Category::Road, &ok.data), Ok(4));
        assert_eq!(pre_verdict(&ok, Category::Building, &ok.data), Ok(4));
        let mid = tex("RDP_ROAD", 512, 256, noise(512, 256, 1), Blend::None, false);
        assert_eq!(pre_verdict(&mid, Category::Building, &mid.data), Ok(2));
        assert_eq!(pre_verdict(&ok, Category::Skip, &ok.data), Err(Verdict::Category));
        assert_eq!(pre_verdict(&ok, Category::Natural, &ok.data), Err(Verdict::NoGain));
    }
}
