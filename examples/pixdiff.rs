//! Image parity gate (docs/developing/perf-gates.md): compares two PNGs, or two
//! directories of PNGs by file name, pixel by pixel (decoded RGBA8).
//!
//! `cargo run --release --example pixdiff -- A B [--max-pixels N] [--max-diff D]`
//!
//! Per file: the pixels that differ in any channel, the largest channel
//! difference, and the mean absolute difference over all channels. Exit
//! status 1 when a file is missing on one side, sizes differ, or a file
//! exceeds the thresholds (default 0 and 0: byte-identical pixels, the
//! Original gate). Last line: `pixdiff: N files, M differ, worst …`.
use std::path::{Path, PathBuf};

struct Diff {
    pixels: u64,
    max: u8,
    mean: f64,
    total: u64,
}

fn diff(a: &Path, b: &Path) -> Result<Diff, String> {
    let ia = image::open(a).map_err(|e| format!("{}: {e}", a.display()))?.to_rgba8();
    let ib = image::open(b).map_err(|e| format!("{}: {e}", b.display()))?.to_rgba8();
    if ia.dimensions() != ib.dimensions() {
        return Err(format!("size {:?} vs {:?}", ia.dimensions(), ib.dimensions()));
    }
    let (mut pixels, mut max, mut sum) = (0u64, 0u8, 0u64);
    for (pa, pb) in ia.as_raw().chunks_exact(4).zip(ib.as_raw().chunks_exact(4)) {
        let mut any = false;
        for k in 0..4 {
            let d = pa[k].abs_diff(pb[k]);
            if d > 0 {
                any = true;
                max = max.max(d);
                sum += u64::from(d);
            }
        }
        pixels += u64::from(any);
    }
    let total = u64::from(ia.width()) * u64::from(ia.height());
    Ok(Diff { pixels, max, mean: sum as f64 / (total * 4).max(1) as f64, total })
}

fn pngs(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).into_iter().flatten().flatten().filter_map(|e| e.file_name().into_string().ok()).filter(|n| n.ends_with(".png")).collect();
    v.sort();
    v
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut paths: Vec<PathBuf> = Vec::new();
    let (mut max_pixels, mut max_diff) = (0u64, 0u8);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--max-pixels" => {
                i += 1;
                max_pixels = args.get(i).and_then(|v| v.parse().ok()).expect("--max-pixels N");
            }
            "--max-diff" => {
                i += 1;
                max_diff = args.get(i).and_then(|v| v.parse().ok()).expect("--max-diff D");
            }
            p => paths.push(PathBuf::from(p)),
        }
        i += 1;
    }
    let [a, b] = paths.as_slice() else {
        eprintln!("usage: pixdiff A B [--max-pixels N] [--max-diff D]   (files or directories)");
        std::process::exit(2);
    };
    let pairs: Vec<(String, PathBuf, PathBuf)> = if a.is_dir() {
        let (na, nb) = (pngs(a), pngs(b));
        let mut names: Vec<String> = na.iter().chain(nb.iter()).cloned().collect();
        names.sort();
        names.dedup();
        names.into_iter().map(|n| (n.clone(), a.join(&n), b.join(&n))).collect()
    } else {
        vec![(a.display().to_string(), a.clone(), b.clone())]
    };
    let (mut bad, mut differ) = (0, 0);
    let (mut worst_pixels, mut worst_max) = (0u64, 0u8);
    for (name, pa, pb) in &pairs {
        if !pa.exists() || !pb.exists() {
            println!("{name}: MISSING on the {} side", if pa.exists() { "second" } else { "first" });
            bad += 1;
            continue;
        }
        match diff(pa, pb) {
            Ok(d) => {
                worst_pixels = worst_pixels.max(d.pixels);
                worst_max = worst_max.max(d.max);
                let over = d.pixels > max_pixels || d.max > max_diff;
                if d.pixels > 0 {
                    differ += 1;
                    println!("{name}: {} of {} pixels differ ({:.4} %), max channel diff {}, mean abs diff {:.5}{}", d.pixels, d.total, 100.0 * d.pixels as f64 / d.total as f64, d.max, d.mean, if over { "  OVER" } else { "" });
                }
                bad += usize::from(over);
            }
            Err(e) => {
                println!("{name}: ERROR {e}");
                bad += 1;
            }
        }
    }
    println!("pixdiff: {} files, {differ} differ, {bad} over the limits (pixels {max_pixels}, diff {max_diff}); worst {worst_pixels} pixels, max channel diff {worst_max}", pairs.len());
    std::process::exit(i32::from(bad > 0));
}
