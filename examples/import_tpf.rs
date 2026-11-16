//! docs/texmod-import.md: convert a TexMod package into a texture
//! pack the clone loads.
//!
//!   NFSU2_ROOT=<game> import_tpf <pack.tpf> <out-dir> [--tone match|keep] [--alpha match|keep] [--max 4096] [--dry-run]
//!
//! Writes `<out>/world/<HASH>.dds|png` (and `<out>/car/…`) keyed by the
//! game's bin hash, and `<out>/import-tpf.tsv`: one line per package
//! texture and target (TexMod key, the texture it matched, input and
//! output format/size, notes, luma/alpha against the original before and
//! after the tone match, flags), the unmatched ones too.
//! `--tone match` (default) gives each replacement the original's mean
//! colour and contrast and, where alpha is data (road reflection amount),
//! its mean alpha (the look rule: no brighter or flatter ground than the
//! original's); `--tone keep` keeps the pack's art as authored.
//! Use the folder with `NFSU2_WORLD_TEXTURE_PACK=<out>/world` (several
//! folders: `a:b`, first wins) or put `<out>` at
//! `<data>/overbright/texture-pack/imported` for the World Texture Pack
//! row. The package is read as data only; nothing in it is run.
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use nfsu2::formats::tpf;
use nfsu2::tpf_import as ti;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut pos = Vec::new();
    let mut max = ti::MAX_DIMENSION;
    let mut dry = false;
    let mut tone_match = true;
    let mut alpha_match = true;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--max" => max = it.next().context("--max N")?.parse()?,
            "--dry-run" => dry = true,
            "--tone" => tone_match = match it.next().map(String::as_str) {
                Some("match") => true,
                Some("keep") => false,
                _ => anyhow::bail!("--tone match|keep"),
            },
            "--alpha" => alpha_match = match it.next().map(String::as_str) {
                Some("match") => true,
                Some("keep") => false,
                _ => anyhow::bail!("--alpha match|keep"),
            },
            _ => pos.push(a.clone()),
        }
    }
    anyhow::ensure!(pos.len() == 2, "usage: import_tpf <pack.tpf> <out-dir> [--tone match|keep] [--alpha match|keep] [--max 4096] [--dry-run]");
    let (pack, out) = (PathBuf::from(&pos[0]), PathBuf::from(&pos[1]));
    let root = PathBuf::from(std::env::var_os("NFSU2_ROOT").context("set NFSU2_ROOT to the game folder")?);

    let t0 = Instant::now();
    let package = tpf::read(&std::fs::read(&pack).with_context(|| pack.display().to_string())?)?;
    let images = ti::package_images(&package);
    eprintln!("package: {} entries, {} images, comment {:?} ({:.1} s)", package.entries.len(), images.len(), package.comment.trim(), t0.elapsed().as_secs_f32());
    let t1 = Instant::now();
    let sources = ti::collect(&root)?;
    eprintln!("game: {} textures, {} keys ({:.1} s)", sources.len(), ti::index(&sources).len(), t1.elapsed().as_secs_f32());

    let t2 = Instant::now();
    let opts = ti::ImportOptions { max, tone: tone_match, alpha: alpha_match, dry_run: dry };
    let sum = ti::import_package(&package, &sources, &out, &opts, &|_, _| {})?;
    println!(
        "{} of {} package textures matched, {} unmatched, {} unsupported; {} files written {:?}, {} skipped (same as the original); tone {}; GPU memory {:.0} MB for the replacements; {:.1} s",
        sum.matched,
        sum.images,
        sum.unmatched,
        sum.unsupported,
        sum.files_written(),
        sum.files,
        sum.same,
        if tone_match { "matched to the original" } else { "as authored" },
        sum.vram as f64 / 1e6,
        t2.elapsed().as_secs_f32()
    );
    if let Some(p) = sum.other_game() {
        println!("made for {p}, not SPEED2.EXE: nothing of it matches this game");
    }
    if !dry {
        println!("report: {}", out.join("import-tpf.tsv").display());
    }
    Ok(())
}
