//! overbright-upscale: make the optional upscaled texture pack from the
//! player's own game files (docs/texture-pack-generator.md). Nothing is
//! downloaded and no model is used (`nfsu2::upscale`: Lanczos-3, anti-ring
//! clamp, back-projection, BC1/BC3 with mips; `nfsu2::upscale_pack`: the
//! sources, the run and the manifest — the same code the Graphics page's
//! "Generate Texture Pack" row runs).
//!
//!   overbright-upscale                       # world + cars into the game's data dir
//!   overbright-upscale --out DIR [--car-out DIR] [--tracks L4RA,..] [--force] [--cars-only]
//!   overbright-upscale --ui | --ui-out DIR [--hd HDPACK]  # the loading and title screens' textures the HD UI pack lacks (docs/loading-image.md)
//!   overbright-upscale --sheet PREFIX [--out DIR | --car-out DIR --cars] [--top 60] [--per 12] [--category Sign,..]
//!
//! With no folder the pack goes where the World Texture Pack row reads it
//! (`<data>/overbright/texture-pack/{world,car}`). `--out` alone (or
//! $NFSU2_WORLD_TEXTURE_PACK) makes the world pack only, as before; add
//! `--car-out` for the cars. Files: `<HASH>.dds` per texture and
//! `overbright-upscale.tsv` (what was done to every texture and why).
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use nfsu2::formats::texture::Texture;
use nfsu2::upscale as up;
use nfsu2::upscale_pack::{self as pack, ALL_TRACKS, MANIFEST, Progress};

struct Args {
    out: Option<PathBuf>,
    car_out: Option<PathBuf>,
    tracks: Vec<String>,
    force: bool,
    cars_only: bool,
    sheet: Option<PathBuf>,
    /// Sheets of the car pack.
    cars: bool,
    top: usize,
    per: usize,
    /// Sheets: only these categories (comma list), e.g. Sign.
    only: Option<String>,
    /// The loading screens' UI textures into DIR (`upscale_pack::ui_dir`).
    ui: Option<PathBuf>,
    /// The HD UI pack whose files are left alone (default: the one the
    /// game looks for, `texture-pack` beside it or NFSU2_TEXTURE_PACK).
    hd: Option<PathBuf>,
}

fn args() -> Result<Args> {
    let mut a = Args { out: std::env::var_os("NFSU2_WORLD_TEXTURE_PACK").filter(|v| !v.is_empty()).map(PathBuf::from), car_out: None, tracks: ALL_TRACKS.iter().map(|s| s.to_string()).collect(), force: false, cars_only: false, sheet: None, cars: false, top: 60, per: 12, only: None, ui: None, hd: nfsu2::hd_ui_pack::candidate_root().filter(|p| p.is_dir()).map(PathBuf::from) };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        match k.as_str() {
            "--out" => a.out = Some(it.next().context("--out DIR")?.into()),
            "--car-out" => a.car_out = Some(it.next().context("--car-out DIR")?.into()),
            "--tracks" => a.tracks = it.next().context("--tracks LIST")?.split(',').map(str::to_string).collect(),
            "--force" => a.force = true,
            "--cars-only" => a.cars_only = true,
            "--cars" => a.cars = true,
            "--sheet" => a.sheet = Some(it.next().context("--sheet PREFIX")?.into()),
            "--top" => a.top = it.next().context("--top N")?.parse()?,
            "--per" => a.per = it.next().context("--per N")?.parse()?,
            "--category" => a.only = Some(it.next().context("--category LIST")?),
            "--ui" => a.ui = Some(pack::ui_dir()),
            "--ui-out" => a.ui = Some(it.next().context("--ui-out DIR")?.into()),
            "--hd" => a.hd = Some(it.next().context("--hd DIR")?.into()),
            "-h" | "--help" => {
                println!("overbright-upscale [--out DIR] [--car-out DIR] [--tracks L4RA,..] [--force] [--cars-only] [--ui | --ui-out DIR] [--hd HDPACK] | --sheet PREFIX [--cars] [--top 60] [--per 12] [--category Sign,Road]\nNo folder: world + cars into the game's data dir. See docs/texture-pack-generator.md.");
                std::process::exit(0);
            }
            _ => bail!("unknown argument {k} (--help)"),
        }
    }
    if a.out.is_none() && a.car_out.is_none() && a.ui.is_none() {
        a.out = Some(pack::world_dir());
        a.car_out = Some(pack::car_dir());
        a.ui = Some(pack::ui_dir());
    }
    Ok(a)
}

fn threads() -> usize {
    std::thread::available_parallelism().map_or(4, |n| n.get()).min(12)
}

fn generate(a: &Args) -> Result<()> {
    let root = nfsu2::game_root();
    let progress = Progress::default();
    if let Some(out) = a.out.as_ref().filter(|_| !a.cars_only) {
        let t0 = std::time::Instant::now();
        let sources = pack::collect_world(&root, &a.tracks, &progress)?;
        eprintln!("world: {} textures read in {:.1} s", sources.len(), t0.elapsed().as_secs_f32());
        pack::generate(&sources, out, &format!("tracks {}", a.tracks.join(",")), a.force, threads(), false, &progress)?.print(out);
    }
    if let Some(out) = &a.car_out {
        let sources = pack::collect_cars(&root)?;
        pack::generate(&sources, out, "cars", a.force, threads(), false, &progress)?.print(out);
    }
    if let Some(out) = &a.ui {
        // docs/loading-image.md: the loading screens' textures and the title screen's.
        let assets = nfsu2::frontend::Assets::load(&root)?;
        let textures = pack::collect_loading(&assets);
        eprintln!("loading and title screens: {} textures; HD UI pack {}", textures.len(), a.hd.as_ref().map_or("none".into(), |d| d.display().to_string()));
        let s = pack::generate_ui(&textures, a.hd.as_deref(), out, a.force)?;
        for r in &s.rows {
            eprintln!("  {:08X} {:<24} {:>9} {:?} {}", r.hash, r.name, r.size, r.verdict, r.out);
        }
        s.print(out);
    }
    Ok(())
}

// ------------------------------------------------------------------ sheets

/// Bilinear sample (wrap) of RGBA8 at texel coordinates.
fn sample(rgba: &[u8], w: u32, h: u32, x: f32, y: f32) -> [u8; 4] {
    let (x0, y0) = ((x - 0.5).floor(), (y - 0.5).floor());
    let (fx, fy) = (x - 0.5 - x0, y - 0.5 - y0);
    let at = |xi: i64, yi: i64| {
        let (xi, yi) = (xi.rem_euclid(w as i64) as u32, yi.rem_euclid(h as i64) as u32);
        &rgba[((yi * w + xi) * 4) as usize..][..4]
    };
    let (x0, y0) = (x0 as i64, y0 as i64);
    std::array::from_fn(|c| {
        let v = |xi, yi| at(xi, yi)[c] as f32;
        let top = v(x0, y0) * (1.0 - fx) + v(x0 + 1, y0) * fx;
        let bot = v(x0, y0 + 1) * (1.0 - fx) + v(x0 + 1, y0 + 1) * fx;
        (top * (1.0 - fy) + bot * fy).round() as u8
    })
}

/// The `cw`×`ch` source window with the most luma variance (the detail a
/// reviewer has to see: text, edges, grain).
fn busiest(rgba: &[u8], w: u32, h: u32, cw: u32, ch: u32) -> (u32, u32) {
    let l: Vec<f32> = rgba.chunks_exact(4).map(|p| 0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32).collect();
    let mut best = (f32::MIN, (0, 0));
    for y in (0..=h - ch).step_by((ch / 4).max(1) as usize) {
        for x in (0..=w - cw).step_by((cw / 4).max(1) as usize) {
            let (mut s, mut s2) = (0.0f32, 0.0f32);
            for yy in y..y + ch {
                for xx in x..x + cw {
                    let v = l[(yy * w + xx) as usize];
                    s += v;
                    s2 += v * v;
                }
            }
            let n = (cw * ch) as f32;
            let var = s2 / n - (s / n).powi(2);
            if var > best.0 {
                best = (var, (x, y));
            }
        }
    }
    best.1
}

fn sheet(a: &Args, prefix: &Path) -> Result<()> {
    let root = nfsu2::game_root();
    let out: &Path = if a.cars { a.car_out.as_deref().context("--cars sheets need --car-out")? } else { a.out.as_deref().context("--out DIR")? };
    let manifest = std::fs::read_to_string(out.join(MANIFEST)).context("no manifest: generate the pack first")?;
    let mut wanted: Vec<(u32, String)> = Vec::new();
    for l in manifest.lines().filter(|l| !l.starts_with('#') && !l.starts_with("hash")) {
        let f: Vec<&str> = l.split('\t').collect();
        if f[4] == "Upscaled" && a.only.as_ref().is_none_or(|o| o.split(',').any(|c| c == f[3])) {
            wanted.push((u32::from_str_radix(f[0], 16)?, f[3].to_string()));
        }
    }
    // The manifest is sorted by near-road area already (collect's order).
    wanted.truncate(a.top);
    let sources = if a.cars { pack::collect_cars(&root)? } else { pack::collect_world(&root, &a.tracks, &Progress::default())? };
    let by: HashMap<u32, &Texture> = sources.iter().map(|s| (s.tex.hash, &s.tex)).collect();
    const P: u32 = 256; // panel side
    let cols = 3u32;
    let mut index = String::new();
    for (sheet_no, chunk) in wanted.chunks(a.per).enumerate() {
        let rows = (chunk.len() as u32).div_ceil(cols);
        let mut img = image::RgbaImage::from_pixel(cols * (2 * P + 8), rows * (P + 8), image::Rgba([40, 40, 40, 255]));
        for (i, (hash, cat)) in chunk.iter().enumerate() {
            let Some(t) = by.get(hash) else { continue };
            let bytes = std::fs::read(out.join(format!("{hash:08X}.dds")))?;
            let e = up::parse_dds(&bytes).context("dds")?;
            let packed = e.as_texture(t).decode_rgba();
            let orig = t.decode_rgba();
            let (cw, ch) = (t.width.min(48), t.height.min(48));
            // Over the shown colour (premultiplied: decals keep their lettering in the alpha).
            let shown: Vec<u8> = orig.chunks_exact(4).flat_map(|p| { let a = if t.blend == nfsu2::formats::texture::Blend::Alpha || t.alpha_test { p[3] as u32 } else { 255 }; [(p[0] as u32 * a / 255) as u8, (p[1] as u32 * a / 255) as u8, (p[2] as u32 * a / 255) as u8, 255] }).collect();
            let (cx, cy) = busiest(&shown, t.width, t.height, cw, ch);
            let k = e.width / t.width;
            let (ox, oy) = ((i as u32 % cols) * (2 * P + 8), (i as u32 / cols) * (P + 8));
            let side = cw.max(ch) as f32;
            for y in 0..P {
                for x in 0..P {
                    let (u, v) = (cx as f32 + (x as f32 + 0.5) * side / P as f32, cy as f32 + (y as f32 + 0.5) * side / P as f32);
                    let mut p0 = sample(&orig, t.width, t.height, u, v);
                    let mut p1 = sample(&packed, e.width, e.height, u * k as f32, v * k as f32);
                    // As the game draws it: cut-outs alpha-tested, decals
                    // blended (over a checker); otherwise the alpha is not
                    // coverage (roads: the reflection amount) and is ignored.
                    let bg = if ((x / 16) + (y / 16)) % 2 == 0 { 90.0 } else { 160.0 };
                    for p in [&mut p0, &mut p1] {
                        let al = if t.alpha_test {
                            if p[3] > nfsu2::formats::texture::ALPHA_TEST_REF { 1.0 } else { 0.0 }
                        } else if t.blend == nfsu2::formats::texture::Blend::Alpha {
                            p[3] as f32 / 255.0
                        } else {
                            1.0
                        };
                        for c in 0..3 {
                            p[c] = (p[c] as f32 * al + bg * (1.0 - al)).round() as u8;
                        }
                        p[3] = 255;
                    }
                    img.put_pixel(ox + x, oy + y, image::Rgba(p0));
                    img.put_pixel(ox + P + x, oy + y, image::Rgba(p1));
                }
            }
            index += &format!("sheet {:02} #{:2} {:08X} {} {}x{} {} -> {}x{} crop {}x{}@{},{}\n", sheet_no + 1, i + 1, hash, t.name, t.width, t.height, cat, e.width, e.height, cw, ch, cx, cy);
        }
        let path = PathBuf::from(format!("{}-{:02}.png", prefix.display(), sheet_no + 1));
        img.save(&path)?;
        eprintln!("{}", path.display());
    }
    std::fs::write(format!("{}-index.txt", prefix.display()), &index)?;
    print!("{index}");
    Ok(())
}

fn main() -> Result<()> {
    // A developer tool: the developer cache, never the player's (docs/asset-cache.md).
    nfsu2::platform::dirs::developer_program();
    let a = args()?;
    match a.sheet.clone() {
        Some(prefix) => sheet(&a, &prefix),
        None => generate(&a),
    }
}
