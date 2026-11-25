//! The launcher's backdrop: drawn by the program (a near-black gradient,
//! faint pink and lime glows, three faint bokeh dots and one pink light
//! streak on the main screen), cached per size, and, once a usable game
//! folder is known, the title screen's picture read from the player's own
//! installed game (`FRONTEND/FrontB.lzc`, texture LS_SPLASH, with the
//! game's own readers) on the right half only — darkened and blurred so
//! every text stays at least 4.5:1 over it (test
//! `backdrop_keeps_text_readable`). Nothing of the
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
pub const STRENGTH: f32 = 0.20;

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
    let raw = crate::game_fs::read_file(&path).ok()?;
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

/// The page: `w` × `h` RGBA, Overbright's own backdrop with the light
/// streak, the picture faded in by `fade` (0..1).
pub fn compose(w: u32, h: u32, picture: Option<&Picture>, fade: f32) -> Vec<u8> {
    compose_scene(w, h, picture, fade, true)
}

/// Overbright's own backdrop, `w` × `h`, as straight f32 RGB: a near-black
/// vertical gradient, a faint pink glow top right and a lime one bottom
/// right, three faint bokeh dots and (`streak`) one pink light streak
/// across the empty right half. Built once per size (cached).
fn base(w: u32, h: u32, streak: bool) -> std::sync::Arc<Vec<[f32; 3]>> {
    type Cache = Vec<((u32, u32, bool), std::sync::Arc<Vec<[f32; 3]>>)>;
    static CACHE: std::sync::Mutex<Cache> = std::sync::Mutex::new(Vec::new());
    let key = (w, h, streak);
    if let Some(b) = CACHE.lock().ok().and_then(|c| c.iter().find(|(k, _)| *k == key).map(|(_, b)| b.clone())) {
        return b;
    }
    let b = std::sync::Arc::new(build_base(w, h, streak));
    if let Ok(mut c) = CACHE.lock() {
        if c.len() >= 4 {
            c.remove(0);
        }
        c.push((key, b.clone()));
    }
    b
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn build_base(w: u32, h: u32, streak: bool) -> Vec<[f32; 3]> {
    let t = theme();
    let (wf, hf) = (w.max(1) as f32, h.max(1) as f32);
    let u = (wf / 960.0).min(hf / 720.0);
    let f = |c: Rgb| [f32::from(c.0), f32::from(c.1), f32::from(c.2)];
    let (top, bottom) = (f(t.page_top), f(t.page_bottom));
    // (centre u, v; radii as shares of w, h; colour; peak alpha; the share
    // of the radius where it has faded out)
    let glows: [((f32, f32), (f32, f32), [f32; 3], f32, f32); 2] = [
        ((0.88, 0.18), (0.60, 0.70), [255.0, 60.0, 110.0], 0.10, 0.60),
        ((0.70, 0.95), (0.50, 0.60), [198.0, 240.0, 135.0], 0.06, 0.60),
    ];
    // Bokeh: (centre u, v; core, edge as shares of the farthest corner's
    // distance; colour; alpha).
    let bokeh: [((f32, f32), f32, f32, [f32; 3], f32); 3] = [
        ((0.92, 0.30), 0.0035, 0.006, [255.0, 220.0, 170.0], 0.10),
        ((0.80, 0.22), 0.005, 0.008, [255.0, 210.0, 160.0], 0.08),
        ((0.96, 0.58), 0.0045, 0.007, [160.0, 200.0, 255.0], 0.07),
    ];
    let bokeh: Vec<((f32, f32), f32, f32, [f32; 3], f32)> = bokeh
        .iter()
        .map(|&((bu, bv), c0, c1, col, a)| {
            let (cx, cy) = (bu * wf, bv * hf);
            let far = cx.max(wf - cx).hypot(cy.max(hf - cy));
            ((cx, cy), c0 * far, c1 * far, col, a)
        })
        .collect();
    // The streak: a line from 58 % to 105 % of the width at 41 % of the
    // height, 3 u thick, turned 6° (its right end up); transparent at its
    // left end, pink at 70 %, near white at its right end.
    let (s0, s1) = (0.58 * wf, 1.05 * wf);
    let thick = 3.0 * u;
    let (scx, scy) = ((s0 + s1) / 2.0, 0.41 * hf + thick / 2.0);
    let len = s1 - s0;
    let (sin, cos) = (-6.0f32).to_radians().sin_cos();
    let (pink, hot) = (f(t.streak), f(t.streak_end));
    let halo = 9.0 * u;
    let mut out = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        let v = (y as f32 + 0.5) / hf;
        let mut row = [0.0; 3];
        for k in 0..3 {
            row[k] = top[k] + (bottom[k] - top[k]) * v;
        }
        for x in 0..w {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let mut c = row;
            for ((gu, gv), (rx, ry), col, a, end) in glows {
                let d = (((px / wf - gu) / rx).powi(2) + ((py / hf - gv) / ry).powi(2)).sqrt();
                let k = a * (1.0 - smoothstep(0.0, end, d));
                if k > 0.0 {
                    for i in 0..3 {
                        c[i] += (col[i] - c[i]) * k;
                    }
                }
            }
            for &((cx, cy), c0, c1, col, a) in &bokeh {
                let d = (px - cx).hypot(py - cy);
                if d < c1 {
                    let k = a * (1.0 - smoothstep(c0, c1, d));
                    for i in 0..3 {
                        c[i] += (col[i] - c[i]) * k;
                    }
                }
            }
            if streak {
                let (dx, dy) = (px - scx, py - scy);
                // Along the line (0 at its left end, 1 at its right) and
                // across it (px).
                let along = (dx * cos + dy * sin) / len + 0.5;
                let across = -dx * sin + dy * cos;
                if (0.0..=1.0).contains(&along) && across.abs() < halo * 3.0 {
                    let g = smoothstep(0.05, 0.70, along);
                    let core = 1.0 - smoothstep(thick * 0.5 - 0.5, thick * 0.5 + 0.7, across.abs());
                    let a = 0.75 * g * core;
                    let col: [f32; 3] = if along > 0.70 {
                        let m = (along - 0.70) / 0.30;
                        [0, 1, 2].map(|i| pink[i] + (hot[i] - pink[i]) * m)
                    } else {
                        pink
                    };
                    let glow = 0.45 * g * (-(across / halo).powi(2)).exp() * 0.55;
                    for i in 0..3 {
                        c[i] += ([255.0, 70.0, 120.0][i] - c[i]) * glow;
                        c[i] += (col[i] - c[i]) * a;
                    }
                }
            }
            out.push(c);
        }
    }
    out
}

/// The page: `w` × `h` RGBA, Overbright's own backdrop (the streak only
/// when `streak`: the main screen's empty right half) with the player's
/// title picture faded in by `fade` (0..1), on the right half only, graded
/// dark and kept off the header and footer rows.
pub fn compose_scene(w: u32, h: u32, picture: Option<&Picture>, fade: f32, streak: bool) -> Vec<u8> {
    let base = base(w, h, streak);
    let fade = fade.clamp(0.0, 1.0);
    let (wf, hf) = (w.max(1) as f32, h.max(1) as f32);
    // The picture covers the right half (cropped to its aspect).
    let cover = picture.map(|p| {
        let (rw, rh) = (wf * 0.5, hf);
        let s = (rw / p.w as f32).max(rh / p.h as f32);
        (s, (p.w as f32 * s - rw) / 2.0, (p.h as f32 * s - rh) / 2.0)
    });
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        let v = y as f32 / hf;
        // Off the header (page names) and the footer (legends).
        let wy = smoothstep(0.12, 0.22, v) * (1.0 - smoothstep(0.80, 0.88, v));
        for x in 0..w {
            let mut c = base[(y * w + x) as usize];
            if let (Some(p), Some((s, ox, oy)), true) = (picture, cover, fade > 0.0 && wy > 0.0) {
                let u = x as f32 / wf;
                let wx = smoothstep(0.50, 0.64, u);
                if wx > 0.0 {
                    let sx = ((x as f32 - wf * 0.5 + ox) / s).clamp(0.0, p.w as f32 - 1.0);
                    let sy = ((y as f32 + oy) / s).clamp(0.0, p.h as f32 - 1.0);
                    let t = sample(p, sx, sy);
                    // The dark grade: a little desaturated, at most
                    // STRENGTH of the texel over the page.
                    let l = 0.2126 * t[0] + 0.7152 * t[1] + 0.0722 * t[2];
                    let k = STRENGTH * wx * wy * fade;
                    for i in 0..3 {
                        let g = t[i] * 0.75 + l * 0.25;
                        c[i] = c[i] * (1.0 - k * 0.5) + g * k;
                    }
                }
            }
            out.extend([
                c[0].round().clamp(0.0, 255.0) as u8,
                c[1].round().clamp(0.0, 255.0) as u8,
                c[2].round().clamp(0.0, 255.0) as u8,
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

/// The brightest the page can get under text: the backdrop's glow peak
/// plus a white picture at full strength (the streak is never behind
/// text: it lies in the main screen's empty right half).
pub fn brightest() -> Rgb {
    let white = Picture { w: 1, h: 1, rgba: vec![255; 4] };
    let px = compose_scene(64, 48, Some(&white), 1.0, false);
    let mut best = (0u32, Rgb(0, 0, 0));
    for p in px.chunks_exact(4) {
        let s = u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2]);
        if s > best.0 {
            best = (s, Rgb(p[0], p[1], p[2]));
        }
    }
    best.1
}

/// Which backdrop a page has: `Main` with the light streak in its empty
/// right half, `Plain` without it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scene {
    Main,
    #[default]
    Plain,
}

/// The look's backdrop for a `w` × `h` window, RGBA, with the player's
/// picture (if any) on its right half. Cached per (w, h, scene, picture):
/// asking again for the same page costs nothing.
pub fn backdrop_a(w: u32, h: u32, scene: Scene, picture: Option<&Picture>) -> std::sync::Arc<Vec<u8>> {
    type Key = (u32, u32, Scene, Option<(u32, u32, u64)>);
    static LAST: std::sync::Mutex<Option<(Key, std::sync::Arc<Vec<u8>>)>> = std::sync::Mutex::new(None);
    let pic_key = picture.map(|p| {
        // A cheap fingerprint of the small picture.
        let sum = p.rgba.iter().step_by(7).fold(0u64, |a, &v| a.wrapping_mul(31).wrapping_add(u64::from(v)));
        (p.w, p.h, sum)
    });
    let key = (w, h, scene, pic_key);
    if let Ok(l) = LAST.lock()
        && let Some((k, px)) = l.as_ref()
        && *k == key
    {
        return px.clone();
    }
    let px = std::sync::Arc::new(compose_scene(w, h, picture, 1.0, scene == Scene::Main));
    if let Ok(mut l) = LAST.lock() {
        *l = Some((key, px.clone()));
    }
    px
}
