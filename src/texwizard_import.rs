//! docs/texture-packs.md, "TexWizard packs": a TexWizard pack
//! (`formats::texwizard`) converted into the texture-pack folders the same
//! way as a TexMod package (`tpf_import::import_images`). TexWizard names
//! each original by its bin hash, which is what our packs are keyed by, so
//! no key has to be computed: a pair `[ORIGINAL, NEW]` puts the pixels of
//! the pack's texture NEW under ORIGINAL's hash, in the world and/or car
//! folder by the file ORIGINAL comes from. A NEW the pack does not hold is
//! looked up in the game's own files (TexWizard asks the game for any
//! loaded texture). Nothing here runs unless the launcher or a tool asks.
use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::formats::texture::{PixelFormat, Texture};
use crate::formats::texwizard::Pack;
use crate::tpf_import::{Image, ImportOptions, ImportSummary, Source};

/// A texture of a chunk file as an image to convert: DXT levels as stored,
/// anything else decoded to RGBA8.
pub fn texture_image(t: &Texture) -> Image {
    match t.format {
        PixelFormat::Dxt1 | PixelFormat::Dxt3 | PixelFormat::Dxt5 => Image::Bc {
            format: t.format,
            width: t.width,
            height: t.height,
            mips: t.mip_count.max(1),
            data: t.data.to_vec(),
        },
        _ => Image::Rgba { width: t.width, height: t.height, rgba: t.decode_rgba() },
    }
}

/// The game's textures by bin hash.
pub fn bin_index(sources: &[Source]) -> HashMap<u32, Vec<&Source>> {
    let mut m: HashMap<u32, Vec<&Source>> = HashMap::new();
    for s in sources {
        m.entry(s.tex.hash).or_default().push(s);
    }
    m
}

/// Converts `pack` for the game's textures `sources` into `out/world`,
/// `out/car` (report `import-texwizard.tsv`). In the summary, `images` is
/// the pack's readable pairs, `missing` those whose NEW texture is neither
/// in the pack nor in the game's files.
pub fn import_pack(
    pack: &Pack,
    sources: &[Source],
    out: &Path,
    opts: &ImportOptions,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<ImportSummary> {
    let index = bin_index(sources);
    let mut items: Vec<(u32, String)> = Vec::new();
    let mut images: Vec<&Texture> = Vec::new();
    let mut missing_lines = Vec::new();
    for p in &pack.pairs {
        let label = format!("{} -> {}", p.original, p.new);
        let tex = pack.texture(p.new_hash).or_else(|| index.get(&p.new_hash).and_then(|v| v.first()).map(|s| &s.tex));
        match tex {
            Some(t) => {
                items.push((p.original_hash, label));
                images.push(t);
            }
            None => missing_lines.push(format!(
                "{:08X}\t{label}\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\tmissing: {} ({:08X}) is not in textures.bin nor in the game's files",
                p.original_hash, p.new, p.new_hash
            )),
        }
    }
    let read = |i: usize| -> Result<Image> { Ok(texture_image(images[i])) };
    let report = "import-texwizard.tsv";
    let mut sum = crate::tpf_import::import_images(
        &items,
        &read,
        &index,
        &crate::tpf_import::variants(sources),
        out,
        opts,
        progress,
        "original",
        report,
    )?;
    let used: std::collections::HashSet<u32> = pack.pairs.iter().map(|p| p.new_hash).collect();
    let unused = pack.textures.iter().filter(|t| !used.contains(&t.hash)).count();
    if !missing_lines.is_empty() || unused > 0 || pack.bad_pairs > 0 {
        sum.missing = missing_lines.len();
        sum.images += missing_lines.len();
        sum.lines.extend(missing_lines);
        if unused > 0 {
            sum.lines.push(format!(
                "# {unused} texture(s) of textures.bin are named in no pair (TexWizard does not use them either)"
            ));
        }
        if pack.bad_pairs > 0 {
            sum.lines.push(format!("# {} pair(s) of meta.json could not be read (left out)", pack.bad_pairs));
        }
        if !opts.dry_run {
            std::fs::create_dir_all(out)?;
            std::fs::write(out.join(report), sum.lines.join("\n") + "\n")?;
        }
    }
    Ok(sum)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::hash::bin_hash;
    use crate::formats::texwizard::{read_pack, tpk_bytes};
    use crate::tpf_import::Domain;

    fn bgra(name: &str, w: u32, h: u32, px: [u8; 4]) -> Texture {
        Texture { format: PixelFormat::Bgra8, ..crate::texture_mods::tests::texture(name, w, h, px) }
    }

    #[test]
    fn pairs_land_under_the_originals_hash() {
        let game = vec![
            Source {
                file: "TRACKS/L4RA.BUN".into(),
                domain: Domain::World,
                tex: bgra("ROAD", 4, 4, [90, 90, 90, 255]),
            },
            Source {
                file: "CARS/X/TEXTURES.BIN".into(),
                domain: Domain::Car,
                tex: bgra("BADGE", 4, 4, [9, 9, 9, 255]),
            },
            Source {
                file: "TRACKS/L4RA.BUN".into(),
                domain: Domain::World,
                tex: bgra("WALL", 4, 4, [200, 10, 10, 255]),
            },
        ];
        // The pack's own texture for ROAD, the game's own WALL for BADGE, a
        // name nothing holds, and one name of another game.
        let bin = tpk_bytes(&[bgra("_ROAD", 8, 8, [20, 140, 60, 255])]);
        let meta =
            br#"{ "textures": [ ["ROAD", "_ROAD"], ["BADGE", "WALL"], ["ROAD2", "_GONE"], ["NOT_IN_UG2", "_ROAD"] ] }"#;
        let pack = read_pack("Test", meta, &bin).unwrap();
        let out = crate::texture_mods::tests::scratch("texwizard-pairs");
        let opts = ImportOptions { tone: false, ..Default::default() };
        let s = import_pack(&pack, &game, &out, &opts, &|_, _| {}).unwrap();
        assert_eq!((s.images, s.matched, s.unmatched, s.missing, s.unsupported), (4, 2, 1, 1, 0));
        let road = out.join(format!("world/{:08X}.png", bin_hash("ROAD")));
        let img = image::open(&road).unwrap().into_rgba8();
        assert_eq!((img.width(), img.get_pixel(3, 3).0), (8, [60, 140, 20, 255]), "the pack's pixels (BGRA read)");
        let badge = image::open(out.join(format!("car/{:08X}.png", bin_hash("BADGE")))).unwrap().into_rgba8();
        assert_eq!(badge.get_pixel(0, 0).0, [10, 10, 200, 255], "the game's own WALL");
        let report = std::fs::read_to_string(out.join("import-texwizard.tsv")).unwrap();
        assert!(report.contains("missing: _GONE") && report.contains("ROAD -> _ROAD"), "{report}");
        let _ = std::fs::remove_dir_all(out);
    }
}
