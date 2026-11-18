use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

/// A scratch folder, removed at the end.
struct Dir(PathBuf);
impl Dir {
    fn new() -> Dir {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "ob-obpack-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Dir(p)
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn png(w: u32, h: u32, c: [u8; 4]) -> Vec<u8> {
    let mut out = Vec::new();
    image::RgbaImage::from_pixel(w, h, image::Rgba(c))
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

/// The top level of `e` decoded.
pub(crate) fn rgba_of(e: &crate::upscale::Encoded) -> Vec<u8> {
    crate::formats::texture::Texture {
        name: String::new(),
        hash: 0,
        width: e.width,
        height: e.height,
        format: e.format,
        mip_count: e.mips,
        data: e.data.clone().into(),
        palette: Vec::new(),
        alpha_test: false,
        blend: crate::formats::texture::Blend::None,
        rendering_order: 0,
        late_buckets: false,
        scroll: Default::default(),
    }
    .decode_rgba()
}

fn put(dir: &Path, rel: &str, bytes: &[u8]) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

fn manifest(name: &str, priority: i32) -> String {
    format!("format = 1\nname = \"{name}\"\nauthor = \"A\"\nlicence = \"CC0-1.0\"\npriority = {priority}\n")
}

#[test]
fn manifest_round_trip() {
    let m = Manifest {
        format: 1,
        name: "Roads \"HD\" \\ v2".into(),
        author: "Someone, Other".into(),
        licence: "CC-BY-4.0".into(),
        priority: -7,
        version: "1.2.0".into(),
        description: "Re-painted roads.".into(),
        compress: Compress::None,
        source: "editor".into(),
    };
    assert_eq!(Manifest::parse(&m.to_toml()).unwrap(), m);
    // `license`, `authors` and unknown fields are read too.
    let m = Manifest::parse(
        "format = 1\nname = \"x\"\nlicense = \"MIT\"\nauthors = [\"a\", \"b\"]\nhomepage = \"h\"\n[group]\nid = 1\n",
    )
    .unwrap();
    assert_eq!((m.licence.as_str(), m.author.as_str(), m.priority, m.compress), ("MIT", "a, b", 0, Compress::Bc7));
}

#[test]
fn bad_manifests_are_refused() {
    for (text, why) in [
        ("name = \"x\"", "no `format`"),
        ("format = 0\nname = \"x\"", "not a pack format"),
        ("format = 2\nname = \"x\"", "newer"),
        ("format = 1", "no `name`"),
        ("format = 1\nname = \"  \"", "no `name`"),
        ("format = 1\nname = \"x\"\npriority = 5000000", "priority"),
        ("format = 1\nname = \"x\"\ncompress = \"bc1\"", "compress"),
        ("format = 1\nname = ", "pack.toml"),
        ("format = \"one\"\nname = \"x\"", "pack.toml"),
    ] {
        let e = Manifest::parse(text).unwrap_err().to_string();
        assert!(e.contains(why), "{text:?}: {e}");
    }
    assert!(Manifest::parse(&"#".repeat(manifest::MAX_BYTES + 1)).is_err());
}

#[test]
fn keys_by_hash_or_name() {
    use index::key_of_stem;
    assert_eq!(key_of_stem("ABCDEF12"), Some((0xABCD_EF12, None)));
    assert_eq!(key_of_stem("abcdef12"), Some((0xABCD_EF12, None)));
    assert_eq!(key_of_stem("0x5FB8BCD1"), Some((0x5FB8_BCD1, None)));
    assert_eq!(key_of_stem("0xBEEF"), Some((0xBEEF, None)));
    let road = crate::formats::hash::bin_hash("ROAD_MAIN");
    assert_eq!(key_of_stem("ROAD_MAIN"), Some((road, Some("ROAD_MAIN".into()))));
    // Not 8 digits, not 0x: a name.
    assert_eq!(key_of_stem("ABCDEF1").unwrap().1.as_deref(), Some("ABCDEF1"));
    assert_eq!(key_of_stem("0x123456789").unwrap().1.as_deref(), Some("0x123456789"));
    assert_eq!(key_of_stem(""), None);
    assert_eq!(stem_for(road, "ROAD_MAIN"), "ROAD_MAIN");
    assert_eq!(stem_for(road, "OTHER"), format!("0x{road:08X}"));
    assert_eq!(stem_for(7, "a/b"), "0x00000007");
    assert_eq!(stem_for(0xABCD_EF12, "ABCDEF12"), "0xABCDEF12");
}

#[test]
fn index_domains_any_and_subfolders() {
    let d = Dir::new();
    let p = d.0.join("roads");
    put(&p, "pack.toml", manifest("Roads", 0).as_bytes());
    put(&p, "world/ROAD_MAIN.png", &png(4, 4, [1, 2, 3, 255]));
    put(&p, "world/signs/0x00001234.png", &png(4, 4, [9; 4]));
    put(&p, "any/0000BEEF.png", &png(4, 4, [5; 4]));
    put(&p, "any/ROAD_MAIN.png", &png(4, 4, [6; 4]));
    put(&p, "car/README.txt", b"notes");
    let pack = Pack::open(&p).unwrap();
    let road = crate::formats::hash::bin_hash("ROAD_MAIN");
    assert_eq!(pack.index.world.len(), 3);
    assert_eq!(pack.index.car.len(), 2);
    assert_eq!(pack.index.world[&road].file, Path::new("world/ROAD_MAIN.png"), "world/ wins over any/");
    assert_eq!(pack.index.car[&road].file, Path::new("any/ROAD_MAIN.png"));
    assert_eq!(pack.index.world[&0x1234].file, Path::new("world/signs/0x00001234.png"));
    assert!(pack.index.car.contains_key(&0xBEEF) && !pack.index.car.contains_key(&0x1234));
    assert_eq!(pack.index.ignored, vec![PathBuf::from("car/README.txt")]);
    assert_eq!(pack.file_for(Domain::Car, 0xBEEF), Some(p.join("any/0000BEEF.png")));
}

#[test]
fn bad_packs_are_refused() {
    let d = Dir::new();
    // Two files for one texture in one folder.
    let p = d.0.join("dup");
    put(&p, "pack.toml", manifest("Dup", 0).as_bytes());
    put(&p, "world/ROAD.png", &png(4, 4, [1; 4]));
    let road = crate::formats::hash::bin_hash("ROAD");
    put(&p, &format!("world/x/{road:08X}.dds"), b"DDS ");
    assert!(Pack::open(&p).unwrap_err().to_string().contains("same texture"));
    // A bad id, a bad manifest, a newer format.
    let p = d.0.join("Bad Id");
    put(&p, "world/A.png", &png(4, 4, [1; 4]));
    assert!(Pack::open(&p).is_err());
    let p = d.0.join("newer");
    put(&p, "pack.toml", b"format = 9\nname = \"n\"\n");
    assert!(Pack::open(&p).unwrap_err().to_string().contains("newer"));
    // Folders too deep.
    let p = d.0.join("deep");
    put(&p, &format!("world/{}A.png", "x/".repeat(index::MAX_DEPTH + 1)), &png(4, 4, [1; 4]));
    assert!(Pack::open(&p).unwrap_err().to_string().contains("deeper"));
    // Listed with their reason, never used.
    let list = list_in(&d.0);
    assert_eq!(list.len(), 4);
    assert!(list.iter().all(|l| l.pack.is_err()));
    assert!(enabled_in(&d.0).is_empty());
}

#[test]
fn order_priority_and_winner() {
    let d = Dir::new();
    let packs = d.0.join("packs");
    for (id, prio, c) in [("low", -5, 10u8), ("high", 50, 20), ("mid", 0, 30), ("mid-b", 0, 40)] {
        put(&packs.join(id), "pack.toml", manifest(id, prio).as_bytes());
        put(&packs.join(id), "world/0x00000001.png", &png(4, 4, [c; 4]));
    }
    put(&packs.join("mid"), "car/0x00000002.png", &png(4, 4, [1; 4]));
    let ids = |l: &[Listed]| l.iter().map(|l| (l.id.clone(), l.on)).collect::<Vec<_>>();
    // No order file: by priority, then id; all on.
    let list = list_in(&packs);
    assert_eq!(ids(&list), [("high".into(), true), ("mid".into(), true), ("mid-b".into(), true), ("low".into(), true)]);
    let on = enabled_in(&packs);
    assert_eq!(winner(&on, Domain::World, 1).unwrap().id, "high");
    assert_eq!(winner(&on, Domain::Car, 2).unwrap().id, "mid");
    assert!(winner(&on, Domain::Car, 1).is_none());
    // The player's order first (with an unknown id and junk lines), the rest after.
    std::fs::write(packs.join(ORDER_FILE), "# Overbright packs v1\non low\noff high\nmaybe mid\non gone\n").unwrap();
    let list = list_in(&packs);
    assert_eq!(
        ids(&list),
        [("low".into(), true), ("high".into(), false), ("mid".into(), true), ("mid-b".into(), true)]
    );
    let on = enabled_in(&packs);
    assert_eq!(winner(&on, Domain::World, 1).unwrap().id, "low", "first wins");
    assert!(on.iter().all(|p| p.id != "high"), "off is not used");
    // The order file round-trips.
    let order = load_order(&packs);
    save_order(&packs, &order).unwrap();
    assert_eq!(load_order(&packs), order);
    assert_eq!(parse_order(&format_order(&order)), order);
}

#[test]
fn compile_bc7_png_dds_and_stamps() {
    let d = Dir::new();
    let (packs, compiled) = (d.0.join("packs"), d.0.join("compiled"));
    let p = packs.join("hd");
    put(&p, "pack.toml", manifest("HD", 0).as_bytes());
    // An 8 × 8 gradient (BC7), a 6 × 6 (stays PNG), a DDS (copied), one in any/.
    let grad = image::RgbaImage::from_fn(8, 8, |x, y| image::Rgba([(x * 30) as u8, (y * 30) as u8, 128, 255]));
    let mut grad_png = Vec::new();
    grad.write_to(&mut std::io::Cursor::new(&mut grad_png), image::ImageFormat::Png).unwrap();
    put(&p, "world/0x00000010.png", &grad_png);
    put(&p, "world/0x00000011.png", &png(6, 6, [1, 2, 3, 255]));
    let up = crate::upscale::Upscaled { width: 8, height: 4, rgba: [90u8, 80, 70, 255].repeat(32), scale: 1 };
    let dxt = crate::upscale::dds_bytes(&crate::upscale::encode(&up));
    put(&p, "world/0x00000012.dds", &dxt);
    put(&p, "any/0x00000013.png", &png(4, 4, [7, 7, 7, 128]));
    put(&p, "world/0x00000014.png", b"not a png");
    let pack = Pack::open(&p).unwrap();
    let mut seen = 0;
    let r = sync(&pack, &compiled.join("hd"), &mut |n, t| {
        seen = n;
        assert!(n <= t);
    })
    .unwrap();
    assert_eq!((r.compiled, r.kept, r.failed.len(), seen), (5, 0, 1, 6));
    assert!(r.failed[0].0.ends_with("0x00000014.png"));
    let w = compiled.join("hd/world");
    let bc7 = parse_any_dds(&std::fs::read(w.join("00000010.dds")).unwrap()).unwrap();
    assert_eq!((bc7.format, bc7.width, bc7.height, bc7.mips), (crate::formats::texture::PixelFormat::Bc7, 8, 8, 4));
    // BC7 of the gradient (a plane in colour space: no single BC line holds
    // it exactly) is close to it.
    let back = rgba_of(&bc7);
    let errs: Vec<i32> = back.iter().zip(grad.as_raw()).map(|(a, b)| (*a as i32 - *b as i32).abs()).collect();
    let (max, mean) = (*errs.iter().max().unwrap(), errs.iter().sum::<i32>() as f32 / errs.len() as f32);
    assert!(max <= 24 && mean <= 4.0, "BC7 error max {max}, mean {mean}");
    assert_eq!(std::fs::read(w.join("00000011.png")).unwrap(), png(6, 6, [1, 2, 3, 255]), "6 x 6 stays PNG");
    assert_eq!(std::fs::read(w.join("00000012.dds")).unwrap(), dxt, "a DDS as it is");
    assert!(w.join("00000013.dds").is_file() && compiled.join("hd/car/00000013.dds").is_file());
    // Nothing changed: all kept.
    let r = sync(&pack, &compiled.join("hd"), &mut |_, _| {}).unwrap();
    assert_eq!((r.compiled, r.kept, r.removed), (0, 5, 0));
    // A texture removed from the pack goes from the compiled folder; one
    // rewritten is built again; compress = "none" keeps PNG.
    std::fs::remove_file(p.join("any/0x00000013.png")).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    put(&p, "world/0x00000010.png", &png(8, 8, [200, 0, 0, 255]));
    put(&p, "pack.toml", format!("{}compress = \"none\"\n", manifest("HD", 0)).as_bytes());
    let pack = Pack::open(&p).unwrap();
    let r = sync(&pack, &compiled.join("hd"), &mut |_, _| {}).unwrap();
    assert_eq!((r.compiled, r.removed), (1, 2), "{r:?}");
    assert!(!w.join("00000013.dds").exists() && !compiled.join("hd/car/00000013.dds").exists());
    assert!(w.join("00000010.png").is_file() && !w.join("00000010.dds").exists(), "PNG replaces the BC7");
}

#[test]
fn dx10_round_trip_and_refusals() {
    let rgba: Vec<u8> = (0..16 * 8).flat_map(|i| [i as u8, 255 - i as u8, 60, 255]).collect();
    let e = dds::encode_bc7(&rgba, 16, 8);
    assert_eq!(e.mips, 5);
    // Levels 16×8, 8×4, 4×2, 2×1, 1×1: 8 + 2 + 1 + 1 + 1 blocks.
    assert_eq!(e.data.len(), 13 * 16);
    let bytes = dds::dx10_bytes(&e);
    let back = dds::parse_dx10(&bytes).unwrap();
    assert_eq!((back.format, back.width, back.height, back.mips, &back.data), (e.format, 16, 8, 5, &e.data));
    // Cut short, a cube / array, an unknown DXGI format, a DX9 file.
    assert!(dds::parse_dx10(&bytes[..bytes.len() - 1]).is_none());
    let mut b = bytes.clone();
    b[140..144].copy_from_slice(&6u32.to_le_bytes());
    assert!(dds::parse_dx10(&b).is_none());
    let mut b = bytes.clone();
    b[128..132].copy_from_slice(&28u32.to_le_bytes());
    assert!(dds::parse_dx10(&b).is_none());
    let up = crate::upscale::Upscaled { width: 8, height: 4, rgba: [1u8; 128].to_vec(), scale: 1 };
    let dx9 = crate::upscale::dds_bytes(&crate::upscale::encode(&up));
    assert!(dds::parse_dx10(&dx9).is_none() && parse_any_dds(&dx9).is_some());
    // BC1 as DX10 round-trips too.
    let e1 = crate::upscale::encode(&up);
    assert_eq!(parse_any_dds(&dds::dx10_bytes(&e1)).unwrap().data, e1.data);
}

#[test]
fn roots_follow_the_order_and_compile_once() {
    let d = Dir::new();
    let (packs, compiled) = (d.0.join("packs"), d.0.join("compiled"));
    // Nothing installed: no roots (the native path).
    assert_eq!(roots_in(&packs, &compiled), [Vec::<PathBuf>::new(), Vec::new()]);
    for id in ["a", "b"] {
        put(&packs.join(id), "pack.toml", manifest(id, 0).as_bytes());
        put(&packs.join(id), "world/0x00000001.png", &png(4, 4, [1; 4]));
    }
    put(&packs.join("b"), "car/0x00000002.png", &png(4, 4, [1; 4]));
    let [world, car] = roots_in(&packs, &compiled);
    assert_eq!(world, vec![compiled.join("a/world"), compiled.join("b/world")]);
    assert_eq!(car, vec![compiled.join("b/car")]);
    save_order(&packs, &[OrderEntry { id: "b".into(), on: true }, OrderEntry { id: "a".into(), on: false }]).unwrap();
    let [world, _] = roots_in(&packs, &compiled);
    assert_eq!(world, vec![compiled.join("b/world")]);
    // The loader reads the compiled BC7 as a pack folder.
    assert!(parse_any_dds(&std::fs::read(compiled.join("b/world/00000001.dds")).unwrap()).is_some());
}

#[test]
fn create_put_install_and_remove() {
    let d = Dir::new();
    let (packs, compiled) = (d.0.join("packs"), d.0.join("compiled"));
    let m = Manifest {
        name: "My Edits".into(),
        author: "Me".into(),
        licence: "CC0-1.0".into(),
        priority: 10,
        ..Default::default()
    };
    let id = new_id(&packs, &m.name);
    assert_eq!(id, "my-edits");
    let pack = create(&packs, &id, &m).unwrap();
    assert!(create(&packs, &id, &m).is_err(), "exists");
    assert_eq!(new_id(&packs, "My Edits"), "my-edits-2");
    let road = crate::formats::hash::bin_hash("ROAD_MAIN");
    let f = put_png(&pack, Domain::World, road, "ROAD_MAIN", &png(4, 4, [3; 4])).unwrap();
    assert_eq!(f, packs.join("my-edits/world/ROAD_MAIN.png"));
    assert!(put_png(&pack, Domain::World, road, "ROAD_MAIN", b"junk").is_err());
    let pack = Pack::open(&pack.dir).unwrap();
    assert_eq!(pack.manifest.priority, 10);
    assert_eq!(pack.index.world[&road].name.as_deref(), Some("ROAD_MAIN"));
    // Replacing it under its hash name removes the name file.
    put_png(&pack, Domain::World, road, "", &png(4, 4, [4; 4])).unwrap();
    let pack = Pack::open(&pack.dir).unwrap();
    assert_eq!(pack.index.world[&road].file, PathBuf::from(format!("world/0x{road:08X}.png")));
    // Install a downloaded folder with a name that is not an id.
    let src = d.0.join("Download Folder");
    put(&src, "pack.toml", manifest("Night Signs", 0).as_bytes());
    put(&src, "any/0x00000005.png", &png(4, 4, [5; 4]));
    put(&src, "junk.exe", b"MZ");
    let id = install_folder(&src, &packs).unwrap();
    assert_eq!(id, "night-signs");
    assert!(!packs.join(&id).join("junk.exe").exists() && packs.join(&id).join("any/0x00000005.png").is_file());
    assert!(install_folder(&d.0.join("missing"), &packs).is_err());
    let empty = d.0.join("empty");
    put(&empty, "pack.toml", manifest("E", 0).as_bytes());
    assert!(install_folder(&empty, &packs).is_err(), "no images");
    roots_in(&packs, &compiled);
    save_order(&packs, &[OrderEntry { id: id.clone(), on: true }]).unwrap();
    remove(&packs, &compiled, &id).unwrap();
    assert!(!packs.join(&id).exists() && !compiled.join(&id).exists() && load_order(&packs).is_empty());
    assert!(remove(&packs, &compiled, "../x").is_err());
}
