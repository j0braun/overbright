//! The underglow's "cutoff line" (docs/neon-quality.md): the native
//! `%s_NEON` masks are still lit on their border, so the glow stops in a
//! straight line at the shadow grid's edge; Car Shadow / Neon Ultra's
//! rebuild fades them to 0 there. Ignored without NFSU2_ROOT.
mod common;

use nfsu2::formats::bundle::{chunks, walk};

fn neon(root: &std::path::Path, car: &str) -> Option<(Vec<u8>, u32, u32)> {
    let raw = std::fs::read(root.join("CARS").join(car).join("TEXTURES.BIN")).expect("reading the car's TEXTURES.BIN");
    let data = if raw.starts_with(b"JDLZ") { nfsu2::formats::jdlz::decompress(&raw).expect("decompressing TEXTURES.BIN") } else { raw };
    let want = nfsu2::formats::hash::bin_hash(&format!("{car}_NEON"));
    let mut out = None;
    walk(chunks(&data, 0), &mut |c| {
        if out.is_some() || c.id != 0xB330_0000 {
            return;
        }
        if let Ok(ts) = nfsu2::formats::texture::parse_texture_pack(c)
            && let Some(t) = ts.into_iter().find(|t| t.hash == want)
        {
            out = Some((t.decode_rgba(), t.width, t.height));
        }
    });
    out
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn native_neon_border_is_lit_and_ultra_fades_it() {
    let root = common::game_root();
    for car in ["LANCEREVO8", "COROLLA", "PEUGOT", "350Z"] {
        let Some((rgba, w, h)) = neon(&root, car) else { panic!("{car}_NEON") };
        let (wu, hu) = (w as usize, h as usize);
        let at = |x: usize, y: usize| rgba[(y * wu + x) * 4..][..3].iter().copied().max().unwrap();
        // Native: the rear / front columns' middle still lit (the line).
        let native_edge = (0..hu).map(|y| at(0, y).max(at(wu - 1, y))).max().unwrap();
        assert!(native_edge >= 8, "{car}: native border {native_edge}");
        let e = nfsu2::neon_quality::enhance(&rgba, w, h).unwrap();
        let (lw, lh) = (e.width as usize, e.height as usize);
        let l0 = &e.levels[0];
        let border = (0..lw).flat_map(|x| [l0[x], l0[(lh - 1) * lw + x]]).chain((0..lh).flat_map(|y| [l0[y * lw], l0[y * lw + lw - 1]])).map(|p| p[0].max(p[1]).max(p[2])).fold(0.0f32, f32::max);
        assert_eq!(border, 0.0, "{car}: rebuilt border");
        // The glow's peaks are kept (within 3 %).
        let peak_native = rgba.chunks_exact(4).map(|p| p[0].max(p[1]).max(p[2])).max().unwrap() as f32 / 255.0;
        let peak = l0.iter().map(|p| p[0].max(p[1]).max(p[2])).fold(0.0f32, f32::max);
        assert!((peak - peak_native).abs() < 0.03 * peak_native + 0.02, "{car}: peak {peak} vs {peak_native}");
    }
}
