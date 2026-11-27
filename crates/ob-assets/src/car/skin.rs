//! Skin textures: remaps, vinyl layers and composites.

use super::*;

/// The six window texture slots, in 0x615290's order (remap table entries
/// at render info +0x708 + 0x10·i; i = 1, the rear, also swaps
/// REAR_DEFROSTER when damaged).
pub const WINDOW_SLOTS: [&str; 6] =
    ["WINDOW_FRONT", "WINDOW_REAR", "WINDOW_LEFT_FRONT", "WINDOW_LEFT_REAR", "WINDOW_RIGHT_FRONT", "WINDOW_RIGHT_REAR"];

/// The rear window's defroster texture (GLOBALB), shown on an intact rear
/// window and swapped for WINDOW_DAMAGEn with it (0x615290).
pub const REAR_DEFROSTER: &str = "REAR_DEFROSTER";

/// What a window slot shows by damage state 0..3 (0x802740; GLOBALB).
pub const WINDOW_TEXTURES: [&str; 4] = ["WINDOW", "WINDOW_DAMAGE0", "WINDOW_DAMAGE1", "WINDOW_DAMAGE2"];

/// Hash of the generated skin texture; see [`stock_texture_remap`].
pub fn skin_hash(car: &str) -> u32 {
    bin_hash(&format!("{car}_SKIN1"))
}

/// The from→to texture table for a car with its lights off (0x631E30):
/// skin slots → `<CAR>_SKIN1`; `HEADLIGHT_*` → the car's headlight textures,
/// and likewise for brake lights. `light_bases` are the fitted headlight
/// (slot 0xE) and brakelight (0xF) parts' texture bases (attribute
/// 0x10C98090, e.g. 350Z VOLTAGE → `350Z_STYLE02_HEADLIGHT`).
pub fn stock_texture_remap(
    car: &str,
    textures: &HashMap<u32, Texture>,
    light_bases: [Option<u32>; 2],
) -> HashMap<u32, u32> {
    let mut remap = HashMap::new();
    let skin = skin_hash(car);
    for from in [bin_hash("GLOBAL_SKIN1"), 0xA736_6AE6, 0x3C84_D757] {
        remap.insert(from, skin);
    }
    // Intact windows (damage state 0; the clone has no window damage).
    for window in WINDOW_SLOTS {
        remap.insert(bin_hash(window), bin_hash(WINDOW_TEXTURES[0]));
    }
    // Lights: the part's own texture base (here the stock KIT00 one) plus
    // a state suffix. Use whichever variant the car actually ships.
    let first_existing =
        |candidates: &[String]| candidates.iter().map(|n| bin_hash(n)).find(|h| textures.contains_key(h));
    // 0x631E30 / 0x60C950: LEFT and RIGHT take `<base>_OFF`, their GLASS
    // `<base>_GLASS_OFF` (base: the part's attribute, else the bare
    // HEADLIGHT / BRAKELIGHT); BRAKELIGHT_CENTRE and _GLASS_CENTRE both
    // take `<CAR>_CENTRE_BRAKELIGHT_OFF` (render info +0x320).
    let exists = |h: u32| Some(h).filter(|h| textures.contains_key(h));
    for (i, light) in ["HEADLIGHT", "BRAKELIGHT"].into_iter().enumerate() {
        let base = light_bases[i].unwrap_or_else(|| bin_hash(light));
        for side in ["LEFT", "RIGHT"] {
            if let Some(to) = exists(bin_hash_from(base, "_OFF")) {
                remap.insert(bin_hash(&format!("{light}_{side}")), to);
            }
            if let Some(to) = exists(bin_hash_from(base, "_GLASS_OFF")) {
                remap.insert(bin_hash(&format!("{light}_GLASS_{side}")), to);
            }
        }
    }
    if let Some(to) = exists(bin_hash(&format!("{car}_CENTRE_BRAKELIGHT_OFF"))) {
        for from in ["BRAKELIGHT_CENTRE", "BRAKELIGHT_GLASS_CENTRE"] {
            remap.insert(bin_hash(from), to);
        }
    }
    // Fallback where the above found no texture: the stock KIT00 one,
    // whichever variant the car ships (not traced).
    for (light, sides) in [("HEADLIGHT", &["LEFT", "RIGHT"][..]), ("BRAKELIGHT", &["LEFT", "RIGHT", "CENTRE"][..])] {
        let base = format!("{car}_KIT00_{light}");
        for side in sides {
            if let Some(to) = first_existing(&[format!("{base}_OFF"), format!("{base}_ON"), base.clone()]) {
                remap.entry(bin_hash(&format!("{light}_{side}"))).or_insert(to);
            }
            if let Some(to) =
                first_existing(&[format!("{base}_GLASS_OFF"), format!("{base}_GLASS_ON"), format!("{base}_GLASS")])
            {
                remap.entry(bin_hash(&format!("{light}_GLASS_{side}"))).or_insert(to);
            }
        }
    }
    remap
}

/// A fitted vinyl as the skin takes it (0x61D7A0's layers 1–4): its
/// image `<CAR>_<TEXTURE>` and `<CAR>_<TEXTURE>_MASK` (0x61D630: the car
/// name, '_', the part's TEXTURE string; both in CARS/<car>/VINYLS.BIN),
/// and with REMAP the four colours (RGBA; the layer's colour slots
/// 0x4D + 4 × layer, RED / GREEN / BLUE / GLOSS) its red, green and blue
/// channels weigh. A colour slot the profile leaves unset has the stock
/// ride's colour (`VINYL_STOCK_COLOURS`); one truly empty keeps its
/// channel: colour k is 0xFF in byte k (0x61D7A0).
pub struct VinylLayer {
    pub image: Texture,
    pub mask: Texture,
    pub remap: Option<[[u8; 4]; 4]>,
}

/// The stock ride's vinyl colours (0x637050 at 0x637100: every layer's
/// colour slots 0x4D + 4 × layer + k take these parts by name, 0x61B990),
/// which a car keeps until a colour is bought: grey 128, white 250,
/// 129 / 130 / 120, black 5. Under Wine the 206's ride held them in both
/// its +0x940 and +0x50 (Customize, no vinyl fitted).
pub const VINYL_STOCK_COLOURS: [&str; 4] =
    ["VINYL_L1_COLOR09", "VINYL_L1_COLOR02", "VINYL_L2_COLOR11", "VINYL_L1_COLOR01"];

/// The fitted vinyls bottom to top (slots 0x40–0x43), a vinyl a lower
/// layer already has skipped (0x61D7A0); none when VINYLS.BIN is missing.
pub fn vinyl_layers(
    car_dir: &Path,
    car: &str,
    db: &PartDb,
    installed: &std::collections::BTreeMap<u8, u32>,
) -> Vec<VinylLayer> {
    let mut wanted: Vec<(u8, &crate::formats::parts::Part, String)> = Vec::new();
    for layer in 0x40u8..=0x43 {
        let Some(&hash) = installed.get(&layer) else { continue };
        if wanted.iter().any(|w| w.1.hash == hash) {
            continue;
        }
        let Some(part) = db.find(car, layer as usize, Some(hash), None) else { continue };
        let Some(texture) = db.attribute(part, bin_hash("TEXTURE")).and_then(|i| db.string(i)) else { continue };
        wanted.push((layer, part, texture));
    }
    if wanted.is_empty() {
        return Vec::new();
    }
    // Mapped (the whole 17 MB file was read for each part change;
    // the walk touches only the packs' headers and the vinyls asked for).
    let Ok(data) = crate::cache::SourceBytes::open(&crate::game_fs::path(car_dir, "VINYLS.BIN")) else { return Vec::new() };
    let hashes: Vec<u32> = wanted
        .iter()
        .flat_map(|w| [bin_hash(&format!("{car}_{}", w.2)), bin_hash(&format!("{car}_{}_MASK", w.2))])
        .collect();
    let mut found: HashMap<u32, Texture> = HashMap::new();
    bundle::walk(bundle::chunks(&data, 0), &mut |c| {
        if c.id == id::TEXTURE_PACK
            && let Ok(ts) = crate::formats::texture::parse_texture_pack_where(c, &|h| hashes.contains(&h))
        {
            found.extend(ts.into_iter().filter(|t| hashes.contains(&t.hash)).map(|t| (t.hash, t)));
        }
    });
    let colour = |slot: u8, k: usize| -> [u8; 4] {
        let part = installed
            .get(&slot)
            .and_then(|&h| db.find(car, slot as usize, Some(h), None))
            .or_else(|| db.find(car, slot as usize, Some(bin_hash(VINYL_STOCK_COLOURS[k])), None));
        match part {
            Some(p) => {
                let get = |n: &str| db.attribute(p, bin_hash(n)).unwrap_or(0).min(255) as u8;
                [get("RED"), get("GREEN"), get("BLUE"), get("GLOSS")]
            }
            None => {
                let mut c = [0; 4];
                c[k] = 0xFF;
                c
            }
        }
    };
    wanted
        .into_iter()
        .filter_map(|(layer, part, texture)| {
            let image = found.remove(&bin_hash(&format!("{car}_{texture}")))?;
            let mask = found.remove(&bin_hash(&format!("{car}_{texture}_MASK")))?;
            let remap = db
                .attribute(part, bin_hash("REMAP"))
                .filter(|&r| r != 0)
                .map(|_| std::array::from_fn(|k| colour(0x4D + (layer - 0x40) * 4 + k as u8, k)));
            Some(VinylLayer { image, mask, remap })
        })
        .collect()
}

/// Colours weighted (0x611B10): each with a weight above 1/255 adds its
/// channels × the weight (truncated), the sums clamped to 255; the alpha
/// is the largest weighted one with `max_alpha`, else summed too.
pub fn weigh(colours: &[[u8; 4]], weights: &[f32], max_alpha: bool) -> [u8; 4] {
    let mut sum = [0i32; 4];
    for (c, &w) in colours.iter().zip(weights) {
        if w <= 1.0 / 255.0 {
            continue;
        }
        for ch in 0..3 {
            sum[ch] += (c[ch] as f32 * w) as i32;
        }
        let a = (c[3] as f32 * w) as i32;
        sum[3] = if max_alpha { sum[3].max(a & 0xFF) } else { sum[3] + a };
    }
    sum.map(|v| v.clamp(0, 255) as u8)
}

/// Build the car's skin the way SPEED2.EXE does (0x61D7A0 → 0x611F20):
/// fill with the paint colour, its alpha the paint's GLOSS, then for each
/// layer take the mask's green channel `m`: `m >= 128` replaces the texel
/// with the layer's, alpha included; `0 < m < 128` blends by `m / 128`
/// (0x611B10: the layer texel × m/128 plus the skin's × (1 − m/128), each
/// channel truncated, alpha likewise summed); `0` keeps it. The vinyl
/// layers come first, bottom to top; a vinyl that takes colours has each
/// texel's red, green and blue (0–1) weigh its first three colours
/// (0x611B10, the alpha the largest weighted one). Last is the panel-line
/// layer `doorline` (see [`Car::doorline`]) with `doorline` + `_MASK`
/// (0x61D7A0 layer 5, never remapped). The skin's size is the DUMMY
/// skin's (512²); a layer of another size is skipped.
///
/// Then the part-paint markers (0x611F20): the texels of the shared
/// `DUMMY_SKIN` texture (CARS/TEXTURES.BIN; the game takes the positions
/// from the first skin it composites, 0x8A1D18) whose ARGB is one of
/// 0xBFFF0000, 0xBF00FF00, 0xBF0000FF, 0xBFFF00FF, 0xBF00FFFF,
/// 0xBFFFFF00, 0xBFFFFFFF are overwritten last with `part_colors[0..7]`
/// (RGBA; the paint slots 0x44, 0x45, 0x46, 0x47, 0x48, 0x4B, 0x4C; four
/// by four texels each along x 484–511, y 508–511 of the 512² skins).
///
/// The skin's alpha is the gloss car.efx weighs the envmap and specular
/// with (r0 = 2·(env + spec)·tex.a·… + diffuse); DUMMY_SKIN has no
/// blending, so it never shows as transparency.
pub fn composite_skin(
    textures: &HashMap<u32, Texture>,
    paint: [u8; 4],
    doorline: &str,
    vinyls: &[VinylLayer],
    part_colors: &[[u8; 4]; 7],
) -> Texture {
    let dummy = textures.get(&bin_hash("DUMMY_SKIN1"));
    let (width, height) = dummy.map_or((512, 512), |t| (t.width, t.height));
    let n = (width * height) as usize;
    let mut rgba: Vec<u8> = std::iter::repeat_n(paint, n).flatten().collect();
    let mut blend = |image: &Texture, mask: &Texture, remap: Option<&[[u8; 4]; 4]>| {
        if (image.width, image.height) != (width, height) || (mask.width, mask.height) != (width, height) {
            return;
        }
        let (image, mask) = (image.decode_rgba(), mask.decode_rgba());
        for i in 0..n {
            let m = mask[i * 4 + 1];
            if m == 0 {
                continue;
            }
            let texel = &image[i * 4..i * 4 + 4];
            let c = match remap {
                Some(colours) => {
                    let w = [texel[0] as f32 / 255.0, texel[1] as f32 / 255.0, texel[2] as f32 / 255.0];
                    weigh(&colours[..3], &w, true)
                }
                None => [texel[0], texel[1], texel[2], texel[3]],
            };
            let out = if m >= 128 {
                c
            } else {
                let t = m as f32 * 0.0078125;
                let old = [rgba[i * 4], rgba[i * 4 + 1], rgba[i * 4 + 2], rgba[i * 4 + 3]];
                weigh(&[c, old], &[t, (1.0 - t).max(0.0)], false)
            };
            rgba[i * 4..i * 4 + 4].copy_from_slice(&out);
        }
    };
    for v in vinyls {
        blend(&v.image, &v.mask, v.remap.as_ref());
    }
    if let (Some(layer), Some(mask)) =
        (textures.get(&bin_hash(doorline)), textures.get(&bin_hash(&format!("{doorline}_MASK"))))
    {
        blend(layer, mask, None);
    }
    if let Some(dummy) = dummy {
        const MARKERS: [[u8; 3]; 7] = [
            [0xFF, 0, 0],
            [0, 0xFF, 0],
            [0, 0, 0xFF],
            [0xFF, 0, 0xFF],
            [0, 0xFF, 0xFF],
            [0xFF, 0xFF, 0],
            [0xFF, 0xFF, 0xFF],
        ];
        for (i, texel) in dummy.decode_rgba().chunks_exact(4).enumerate() {
            if texel[3] != 0xBF {
                continue;
            }
            if let Some(k) = MARKERS.iter().position(|m| m[..] == texel[..3]) {
                rgba[i * 4..i * 4 + 4].copy_from_slice(&part_colors[k]);
            }
        }
    }
    Texture {
        name: "SKIN1".into(),
        hash: 0,
        width,
        height,
        format: crate::formats::texture::PixelFormat::Rgba8,
        mip_count: 1,
        data: rgba.into(),
        palette: Vec::new(),
        alpha_test: false,
        blend: crate::formats::texture::Blend::None,
        rendering_order: 0,
        late_buckets: false,
        scroll: Default::default(),
    }
}
