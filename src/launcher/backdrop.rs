//! The launcher's backdrop: a dark page with Overbright's own amber glow
//! (the icon's ring colour) when no game is known, and, once a usable game
//! folder is known, the title screen's picture read from the player's own
//! installed game (`FRONTEND/FrontB.lzc`, texture LS_SPLASH, with the
//! game's own readers) — darkened and blurred so every text stays at
//! least 4.5:1 over it (test `backdrop_keeps_text_readable`). Nothing of the
//! game is in the program or its package, and nothing is written: the
//! picture is read at each start on a worker thread and faded in when it
//! is ready (the first frame shows the fallback at once).

use std::path::Path;

use super::draw::{Rgb, theme};

/// Whether the launcher shows the game's title picture behind its pages at
/// all (on by default). `false` here is the one line that switches it off:
/// the launcher then always shows Overbright's own glow and never reads
/// the picture (the window's `poll_backdrop` asks this first).
pub const FROM_THE_GAME: bool = true;

/// The title screen's picture in the game's front end.
pub const PICTURE: &str = "LS_SPLASH";

/// How much of the (blurred) picture shows over the page, at most: the
/// brightest texel adds at most this share of white. Every text on the
/// page itself (not on a panel) keeps 4.5:1 over it.
pub const STRENGTH: f32 = 0.24;

/// The picture, small and blurred (straight RGBA).
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

/// Reads the title picture from the player's game folder (None when it
/// is not there or cannot be read: the fallback stays).
pub fn load(root: &Path) -> Option<Picture> {
    let path = crate::game_fs::path(root, "FRONTEND/FrontB.lzc");
    let raw = std::fs::read(path).ok()?;
    let data = if raw.starts_with(b"JDLZ") { crate::formats::jdlz::decompress(&raw).ok()? } else { raw };
    let hash = crate::formats::hash::bin_hash(PICTURE);
    let mut found = None;
    crate::formats::bundle::walk(crate::formats::bundle::chunks(&data, 0), &mut |c| {
        if found.is_none()
            && c.id == 0xB330_0000
            && let Ok(ts) = crate::formats::texture::parse_texture_pack_where(c, &|h| h == hash)
        {
            found = ts.into_iter().find(|t| t.hash == hash);
        }
    });
    let t = found?;
    let rgba = t.decode_rgba();
    if t.width == 0 || t.height == 0 || rgba.len() != (t.width * t.height * 4) as usize {
        return None;
    }
    Some(soften(&rgba, t.width, t.height))
}

/// Shrinks to at most 160 texels wide and blurs (three box passes).
pub fn soften(rgba: &[u8], w: u32, h: u32) -> Picture {
    let f = (w as f32 / 160.0).max(1.0);
    let (nw, nh) = (((w as f32 / f).round() as u32).max(1), ((h as f32 / f).round() as u32).max(1));
    let small = crate::tpf_import::resample(rgba, w, h, nw, nh, false);
    let mut px: Vec<[f32; 3]> =
        small.chunks_exact(4).map(|p| [f32::from(p[0]), f32::from(p[1]), f32::from(p[2])]).collect();
    for _ in 0..3 {
        px = box_blur(&px, nw as usize, nh as usize, 3);
    }
    let rgba = px.iter().flat_map(|p| [p[0] as u8, p[1] as u8, p[2] as u8, 255]).collect();
    Picture { w: nw, h: nh, rgba }
}

fn box_blur(px: &[[f32; 3]], w: usize, h: usize, r: usize) -> Vec<[f32; 3]> {
    let pass = |src: &[[f32; 3]], horizontal: bool| -> Vec<[f32; 3]> {
        let mut out = vec![[0.0; 3]; src.len()];
        for y in 0..h {
            for x in 0..w {
                let mut acc = [0.0f32; 3];
                let mut n = 0.0;
                for d in -(r as i32)..=(r as i32) {
                    let (sx, sy) = if horizontal { (x as i32 + d, y as i32) } else { (x as i32, y as i32 + d) };
                    if sx < 0 || sy < 0 || sx >= w as i32 || sy >= h as i32 {
                        continue;
                    }
                    let p = src[sy as usize * w + sx as usize];
                    for k in 0..3 {
                        acc[k] += p[k];
                    }
                    n += 1.0;
                }
                out[y * w + x] = [acc[0] / n, acc[1] / n, acc[2] / n];
            }
        }
        out
    };
    pass(&pass(px, true), false)
}

/// The page: `w` × `h` RGBA, the fallback (Overbright's glow) with the
/// picture faded in by `fade` (0..1).
pub fn compose(w: u32, h: u32, picture: Option<&Picture>, fade: f32) -> Vec<u8> {
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    let bg = theme::BG;
    let base = [f32::from(bg.0), f32::from(bg.1), f32::from(bg.2)];
    let accent = theme::ACCENT;
    let glow = [f32::from(accent.0), f32::from(accent.1), f32::from(accent.2)];
    let fade = fade.clamp(0.0, 1.0);
    let (wf, hf) = (w.max(1) as f32, h.max(1) as f32);
    // The picture covers the window (cropped to its aspect).
    let cover = picture.map(|p| {
        let s = (wf / p.w as f32).max(hf / p.h as f32);
        (s, (p.w as f32 * s - wf) / 2.0, (p.h as f32 * s - hf) / 2.0)
    });
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / wf, y as f32 / hf);
            // Overbright's own: an amber glow from the top left, behind
            // the wordmark, fading out over the page.
            let d = ((u * 1.6).powi(2) + (v * 2.4).powi(2)).sqrt();
            let g = (1.0 - d).max(0.0).powi(2) * 0.06;
            let mut c = [0.0f32; 3];
            for k in 0..3 {
                c[k] = base[k] + glow[k] * g;
            }
            if let (Some(p), Some((s, ox, oy)), true) = (picture, cover, fade > 0.0) {
                let sx = ((x as f32 + ox) / s).clamp(0.0, p.w as f32 - 1.0);
                let sy = ((y as f32 + oy) / s).clamp(0.0, p.h as f32 - 1.0);
                let t = sample(p, sx, sy);
                // Darker towards the bottom, where the panels sit.
                let k = STRENGTH * (1.0 - 0.45 * v) * fade;
                for i in 0..3 {
                    c[i] = c[i] * (1.0 - g * 0.5 * fade) + t[i] * k;
                }
            }
            out.extend([
                c[0].round().min(255.0) as u8,
                c[1].round().min(255.0) as u8,
                c[2].round().min(255.0) as u8,
                255,
            ]);
        }
    }
    out
}

/// Bilinear sample of the small picture.
fn sample(p: &Picture, x: f32, y: f32) -> [f32; 3] {
    let (x0, y0) = (x.floor() as u32, y.floor() as u32);
    let (x1, y1) = ((x0 + 1).min(p.w - 1), (y0 + 1).min(p.h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let at = |x: u32, y: u32, k: usize| f32::from(p.rgba[((y * p.w + x) * 4) as usize + k]);
    let mut c = [0.0; 3];
    for (k, ck) in c.iter_mut().enumerate() {
        let top = at(x0, y0, k) * (1.0 - fx) + at(x1, y0, k) * fx;
        let bot = at(x0, y1, k) * (1.0 - fx) + at(x1, y1, k) * fx;
        *ck = top * (1.0 - fy) + bot * fy;
    }
    c
}

/// The brightest the page can get under text: the fallback's glow peak
/// plus a white picture at full strength.
pub fn brightest() -> Rgb {
    let white = Picture { w: 1, h: 1, rgba: vec![255; 4] };
    let px = compose(8, 8, Some(&white), 1.0);
    let mut best = (0u32, Rgb(0, 0, 0));
    for p in px.chunks_exact(4) {
        let s = u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2]);
        if s > best.0 {
            best = (s, Rgb(p[0], p[1], p[2]));
        }
    }
    best.1
}
