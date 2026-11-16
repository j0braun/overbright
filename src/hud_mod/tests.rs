//! The HUD mod's conversion with tiny stand-ins made here: two "downloads"
//! laid out as the real ones are (an extracted HUD folder with its
//! installer script, a map ZIP or folder with its script), with images of
//! our own (4 × 4 DXT1 blocks of one colour each) and as many of them as
//! the game's completeness check wants. Nothing of the mods is used.

use std::path::{Path, PathBuf};

use super::*;

/// A fresh folder for one test.
fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ob-hud-mod-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A 4 × 4 DXT1 DDS of one colour (`c` as RGB565).
fn dds(c: u16) -> Vec<u8> {
    let mut v = b"DDS ".to_vec();
    let mut h = [0u8; 124];
    h[0..4].copy_from_slice(&124u32.to_le_bytes());
    h[4..8].copy_from_slice(&0x1007u32.to_le_bytes());
    h[8..12].copy_from_slice(&4u32.to_le_bytes());
    h[12..16].copy_from_slice(&4u32.to_le_bytes());
    h[72..76].copy_from_slice(&32u32.to_le_bytes());
    h[76..80].copy_from_slice(&4u32.to_le_bytes());
    h[80..84].copy_from_slice(b"DXT1");
    v.extend_from_slice(&h);
    // c0 > c1: four colours; every texel index 0.
    v.extend_from_slice(&c.max(1).to_le_bytes());
    v.extend_from_slice(&0u16.to_le_bytes());
    v.extend_from_slice(&[0; 4]);
    v
}

const FOLDERS: [&str; 3] = ["FRONTB\\FRONTENDTEXTURES", "INGAMECOMMON\\HUDTEXTURESGLOBAL", "HUD_CUSTOMTEXTURES_ALL"];

/// An extracted HUD part stand-in with `n` images (the known count
/// by default), plus an UnlimiterTextures image that is left out.
fn hud_folder(root: &Path, n: usize) -> PathBuf {
    let d = root.join("hud-part-example");
    let assets = d.join("assets");
    let mut script = String::from("[VERSN2]\n\nunlock_memory all\n");
    for (i, f) in FOLDERS.iter().enumerate() {
        script += &format!("bind_textures synchronize GLOBAL\\EXAMPLE{i}.BIN TPKBlocks COLL{i} \"{f}\"\n");
        std::fs::create_dir_all(assets.join(f.replace('\\', "/"))).unwrap();
    }
    script += "bind_textures synchronize FRONTEND\\FRONTB.LZC TPKBlocks FRONTENDTEXTURES \"UnlimiterTextures\"\n";
    std::fs::create_dir_all(assets.join("UnlimiterTextures")).unwrap();
    std::fs::write(assets.join("UnlimiterTextures/EXTRA_ICON.dds"), dds(5)).unwrap();
    std::fs::write(assets.join("userstart.end"), script).unwrap();
    for i in 0..n {
        let f = FOLDERS[i % 3].replace('\\', "/");
        std::fs::write(assets.join(f).join(format!("EXAMPLE_HUD_{i:03}.dds")), dds(i as u16 * 7 + 3)).unwrap();
    }
    // One name in two folders with the same pixels: one PNG.
    std::fs::write(assets.join("HUD_CUSTOMTEXTURES_ALL/EXAMPLE_HUD_000.dds"), dds(3)).unwrap();
    std::fs::write(d.join("ReadMe.txt"), "example readme\n").unwrap();
    d
}

/// The map stand-in's script and files: `KNOWN_MAP_IMAGES` maps, 27
/// calibrated tracks, 88 rows (the game's check), one Beta map left out.
fn map_files() -> Vec<(String, Vec<u8>)> {
    let mut script = String::from("[VERSN2]\n\n//Common\n");
    let mut files = Vec::new();
    for t in 0..KNOWN_MAP_IMAGES as u32 {
        let n = 4000 + t;
        if t < 100 {
            script += &format!(
                "replace_texture TRACKS\\TRACKMAP{n}.BIN TPKBlocks TRACKMAP{n} TRACKMAP{n} \"Map\\TRACKMAP{n}.dds\"\n"
            );
            files.push((format!("Binary/Map/TRACKMAP{n}.dds"), dds(1000 + t as u16)));
        }
    }
    for t in 0..27u32 {
        let fields = if t < 7 { 4 } else { 3 };
        for f in &MAP_FIELDS[..fields] {
            script += &format!("update_collection GLOBAL\\GLOBALB.LZC Tracks {} {f} {}\n", 4000 + t, 100 + t);
        }
    }
    script += "combobox \"Default\" \"Beta\" \"Select map version\"\n\n\"Default\"\n";
    for t in 100..KNOWN_MAP_IMAGES as u32 {
        let n = 4000 + t;
        script += &format!(
            "if collection_exists TRACKS\\TRACKMAP{n}.BIN TPKBlocks TRACKMAP{n}\ndo\nreplace_texture TRACKS\\TRACKMAP{n}.BIN TPKBlocks TRACKMAP{n} TRACKMAP{n} \"Map\\Default\\TRACKMAP{n}.dds\"\nelse\nreplace_texture TRACKS\\TRACKMAP{n}.BIN TPKBlocks DEFAULT TRACKMAP{n} \"Map\\Default\\TRACKMAP{n}.dds\"\nend\n"
        );
        files.push((format!("Binary/Map/Default/TRACKMAP{n}.dds"), dds(2000 + t as u16)));
    }
    script += "\n\"Beta\"\nreplace_texture TRACKS\\TRACKMAP4000.BIN TPKBlocks DEFAULT TRACKMAP4000 \"Map\\Beta\\TRACKMAP4000.dds\"\n";
    files.push(("Binary/Map/Beta/TRACKMAP4000.dds".into(), dds(9)));
    files.push(("Binary/script.end".into(), script.into_bytes()));
    files.push(("readme.txt".into(), b"example map readme\n".to_vec()));
    files
}

fn map_folder(root: &Path) -> PathBuf {
    let d = root.join("map-part-example");
    for (name, data) in map_files() {
        let p = d.join(&name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, data).unwrap();
    }
    d
}

fn map_zip(root: &Path) -> PathBuf {
    let files = map_files();
    let list: Vec<(&str, &[u8], u16)> = files.iter().map(|(n, d)| (n.as_str(), d.as_slice(), 8)).collect();
    let p = root.join("map-part-example.zip");
    std::fs::write(&p, crate::formats::zip::build(&list)).unwrap();
    p
}

fn parts_of(root: &Path, chosen: &[&Path]) -> Parts {
    let _ = root;
    let mut p = Parts::default();
    for c in chosen {
        add(&mut p, c);
    }
    p
}

#[test]
fn the_downloads_become_a_complete_pack() {
    let root = scratch("complete");
    let downloads = root.join("Downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    let hud = hud_folder(&downloads, KNOWN_HUD_IMAGES);
    let _map = map_zip(&downloads);
    // Choosing the HUD folder finds the map ZIP beside it.
    let parts = parts_of(&root, &[&hud]);
    assert!(parts.complete(), "{parts:?}");
    assert!(matches!(parts.map, Some(Source::Zip { .. })));
    let packs = root.join("packs");
    let seen = std::sync::Mutex::new(0);
    let r = convert(parts.hud.as_ref().unwrap(), parts.map.as_ref().unwrap(), &packs, &|d, t| {
        assert!(d <= t);
        *seen.lock().unwrap() = t;
    });
    let s = r.as_ref().unwrap();
    assert_eq!(*seen.lock().unwrap(), KNOWN_HUD_IMAGES + KNOWN_MAP_IMAGES);
    assert_eq!(
        (s.hud_sources, s.hud_images, s.map_images, s.rows, s.tracks),
        (KNOWN_HUD_IMAGES + 1, KNOWN_HUD_IMAGES, KNOWN_MAP_IMAGES, 88, 27)
    );
    assert!(s.check.complete, "{:?}", s.check);
    assert!(crate::hd_ui_pack::is_complete(&packs.join(PACK_DIR)));
    let lines = result_lines(&r, &parts);
    assert_eq!(lines[0].0, Some(true));
    assert!(lines[0].1.contains("HUD mod set up: 595 HUD images and 115 maps"), "{}", lines[0].1);
    // Pixels kept: a HUD image and a map read back.
    let dir = packs.join(PACK_DIR);
    let png = |name: &str| image::open(dir.join(format!("{:08X}.png", bin_hash(name)))).unwrap().into_rgba8();
    let want =
        decode(&Source::Dir(hud.clone()), "assets/INGAMECOMMON/HUDTEXTURESGLOBAL/EXAMPLE_HUD_001.dds").unwrap().2;
    assert_eq!(png("EXAMPLE_HUD_001").as_raw(), &want);
    let want = decode(&parts.map.clone().unwrap(), "Binary/Map/Default/TRACKMAP4110.dds").unwrap().2;
    assert_eq!(png("TRACKMAP4110").as_raw(), &want);
    // The UnlimiterTextures and Beta images are not in it; the notes are.
    assert!(!dir.join(format!("{:08X}.png", bin_hash("EXTRA_ICON"))).exists());
    assert!(
        dir.join("ReadMe.txt").is_file() && dir.join("detailed-map-readme.txt").is_file() && dir.join(MARKER).is_file()
    );
    let tsv = std::fs::read_to_string(dir.join("map-overrides.tsv")).unwrap();
    assert!(tsv.starts_with("# track\tfield\tvalue\n4000\tMenuMapZoomOffsetX\t100\n"), "{tsv}");
    // Again: the older pack is replaced, no leftovers.
    convert(parts.hud.as_ref().unwrap(), parts.map.as_ref().unwrap(), &packs, &|_, _| {}).unwrap();
    let names: Vec<String> =
        std::fs::read_dir(&packs).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, [PACK_DIR]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn missing_files_are_named_and_nothing_changes() {
    let root = scratch("missing");
    let hud = hud_folder(&root, KNOWN_HUD_IMAGES);
    let map = map_folder(&root);
    std::fs::remove_file(map.join("Binary/Map/Default/TRACKMAP4105.dds")).unwrap();
    let packs = root.join("packs");
    std::fs::create_dir_all(packs.join(PACK_DIR)).unwrap();
    std::fs::write(packs.join(PACK_DIR).join(MARKER), "older").unwrap();
    let parts = parts_of(&root, &[&map, &hud]);
    assert!(matches!(parts.map, Some(Source::Dir(_))));
    let r = convert(parts.hud.as_ref().unwrap(), parts.map.as_ref().unwrap(), &packs, &|_, _| {});
    match &r {
        Err(Problem::Missing(f)) => assert_eq!(f, &["Binary/Map/Default/TRACKMAP4105.dds".to_string()]),
        other => panic!("{other:?}"),
    }
    let line = &result_lines(&r, &parts)[0];
    assert_eq!(line.0, Some(false));
    assert!(line.1.contains("TRACKMAP4105.dds") && line.1.contains("Download the mod again"), "{}", line.1);
    // The older pack is untouched, the temporary folder gone.
    assert_eq!(std::fs::read_to_string(packs.join(PACK_DIR).join(MARKER)).unwrap(), "older");
    assert!(!packs.join(".hud-mod-import").exists());
    // A damaged image (not a DDS) is named too.
    std::fs::write(map.join("Binary/Map/Default/TRACKMAP4105.dds"), b"not a dds").unwrap();
    let r = convert(parts.hud.as_ref().unwrap(), parts.map.as_ref().unwrap(), &packs, &|_, _| {});
    assert!(matches!(&r, Err(Problem::Missing(f)) if f[0].contains("TRACKMAP4105")), "{r:?}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn another_version_is_converted_but_not_complete() {
    let root = scratch("version");
    let hud = hud_folder(&root, 30);
    let map = map_folder(&root);
    let parts = parts_of(&root, &[&root]);
    assert!(parts.complete(), "the folder holding both: {parts:?}");
    let packs = root.join("packs");
    let r = convert(parts.hud.as_ref().unwrap(), parts.map.as_ref().unwrap(), &packs, &|_, _| {});
    let s = r.as_ref().unwrap();
    assert!(!s.check.complete && s.check.is_pack && s.check.present == 30 + KNOWN_MAP_IMAGES);
    let line = &result_lines(&r, &parts)[0];
    assert_eq!(line.0, None);
    assert!(line.1.contains("not the version Overbright knows"), "{}", line.1);
    let _ = (hud, map);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn what_is_chosen_is_named() {
    let root = scratch("identify");
    // A 7-Zip file alone: extract it first.
    let seven = root.join("only/hud-part-example.7z");
    std::fs::create_dir_all(seven.parent().unwrap()).unwrap();
    std::fs::write(&seven, b"7z\xBC\xAF\x27\x1C").unwrap();
    let p = parts_of(&root, &[&seven]);
    assert!(
        p.hud.is_none() && p.notes[0].contains("7-Zip archive") && p.notes[0].contains("extracted folder"),
        "{p:?}"
    );
    // The 7-Zip file beside its extracted folder: the folder is used.
    let hud = hud_folder(seven.parent().unwrap(), 3);
    let p = parts_of(&root, &[&seven]);
    assert_eq!(p.hud, Some(Source::Dir(hud.clone())));
    assert!(p.notes.is_empty(), "{p:?}");
    assert!(waiting_line(&p).unwrap().contains("Now add the map part"));
    // The `assets` folder itself.
    assert_eq!(identify(&hud.join("assets")), Found::Hud(Source::Dir(hud.clone())));
    // Something else.
    let other = root.join("Holiday photos");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("beach.jpg"), b"x").unwrap();
    let p = parts_of(&root, &[&other]);
    assert!(p.notes[0].contains("is not the HUD mod"), "{p:?}");
    let z = root.join("other.zip");
    std::fs::write(&z, crate::formats::zip::build(&[("readme.txt", b"x", 0)])).unwrap();
    assert_eq!(identify(&z), Found::Other);
    // A map ZIP with a folder inside.
    let files = map_files();
    let list: Vec<(String, &[u8], u16)> =
        files.iter().map(|(n, d)| (format!("Map download/{n}"), d.as_slice(), 0)).collect();
    let list: Vec<(&str, &[u8], u16)> = list.iter().map(|(n, d, m)| (n.as_str(), *d, *m)).collect();
    let z = root.join("nested.zip");
    std::fs::write(&z, crate::formats::zip::build(&list)).unwrap();
    match identify(&z) {
        Found::Map(s) => {
            assert!(s.is_file("Binary/Map/TRACKMAP4001.dds"));
            assert_eq!(s.files_in("binary/map/default").unwrap().len(), 15);
        }
        f => panic!("{f:?}"),
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// The DXT decoding's rounding: RGB565 widened by repeating its top bits,
/// the palette in integer thirds, DXT1's transparent fourth colour, the
/// DXT5 ramp.
#[test]
fn dxt_decoding_rounds_as_the_reference() {
    use crate::formats::texture::PixelFormat;
    // c0 = (1, 1, 1) in 565, c1 = 0: index 0, 1, 2, 3 in the first four texels.
    let c0: u16 = (1 << 11) | (1 << 5) | 1;
    let mut b = c0.to_le_bytes().to_vec();
    b.extend_from_slice(&[0, 0, 0b1110_0100, 0, 0, 0]);
    let px = decode_bc(PixelFormat::Dxt1, 4, 4, &b);
    // 1 -> 8 (5 bits: 00001 -> 00001000 | 000), 1 -> 4 (6 bits).
    assert_eq!(&px[..16], &[8, 4, 8, 255, 0, 0, 0, 255, 5, 2, 5, 255, 2, 1, 2, 255]);
    // 31 -> 255, 30 -> 247 (the game's own decoder gives 246).
    let c: u16 = 30 << 11;
    let mut b = c.to_le_bytes().to_vec();
    b.extend_from_slice(&[0xFF, 0xFF, 0, 0, 0, 0]);
    assert_eq!(decode_bc(PixelFormat::Dxt1, 4, 4, &b)[0], 247);
    // DXT1 with c0 <= c1: index 3 is transparent black.
    let mut b = vec![0, 0, 0xFF, 0xFF, 0xFF, 0, 0, 0];
    b[4] = 3;
    assert_eq!(&decode_bc(PixelFormat::Dxt1, 4, 4, &b)[..4], &[0, 0, 0, 0]);
    // DXT5: a0 = 200 > a1 = 100, index 2 = (6*200 + 100) / 7 = 185.
    let mut b = vec![200, 100, 2, 0, 0, 0, 0, 0];
    b.extend_from_slice(&[0; 8]);
    assert_eq!(decode_bc(PixelFormat::Dxt5, 4, 4, &b)[3], 185);
}

#[test]
fn scripts_are_read_as_data() {
    let c = parse_hud_script(
        "\u{feff}[VERSN2]\nbind_textures synchronize GLOBAL\\A.BIN TPKBlocks A \"A\\B\"\nbind_textures synchronize X TPKBlocks Y \"UnlimiterTextures\\Z\"\n",
    )
    .unwrap();
    assert_eq!(c, [Category { folder: "A/B".into(), game_file: "GLOBAL\\A.BIN".into(), collection: "A".into() }]);
    assert!(parse_hud_script("bind_textures synchronize X TPKBlocks Y \"..\\..\\etc\"\n").is_err());
    assert!(parse_hud_script("bind_textures synchronize X TPKBlocks Y \"C:\\Windows\"\n").is_err());
    assert!(parse_hud_script("[VERSN2]\n").is_err());
    let ok = "combobox \"Default\" \"Beta\" \"Select map version\"\n\"Default\"\nreplace_texture TRACKS\\TRACKMAP4001.BIN TPKBlocks DEFAULT TRACKMAP4001 \"Map\\Default\\TRACKMAP4001.dds\"\n\"Beta\"\nanything at all\n";
    let m = parse_map_script(ok).unwrap();
    assert_eq!(m.textures["TRACKMAP4001"], "Map/Default/TRACKMAP4001.dds");
    for bad in [
        "move_file a b\n",
        "replace_texture TRACKS\\TRACKMAP4001.BIN TPKBlocks DEFAULT TRACKMAP4001 \"..\\TRACKMAP4001.dds\"\n",
        "update_collection GLOBAL\\GLOBALB.LZC Tracks 4001 CarPhysics 1\n",
        "update_collection GLOBAL\\GLOBALB.LZC Tracks 4001 MenuMapZoomWidth nan\n",
    ] {
        assert!(parse_map_script(&format!("{bad}{ok}")).is_err(), "{bad}");
    }
}

#[test]
fn a_clashing_image_is_refused() {
    let root = scratch("clash");
    let hud = hud_folder(&root, KNOWN_HUD_IMAGES);
    let map = map_folder(&root);
    // The same name in another folder with other pixels.
    std::fs::write(hud.join("assets/HUD_CUSTOMTEXTURES_ALL/EXAMPLE_HUD_000.dds"), dds(999)).unwrap();
    let packs = root.join("packs");
    let r = convert(&Source::Dir(hud), &Source::Dir(map), &packs, &|_, _| {});
    assert!(matches!(&r, Err(Problem::Missing(f)) if f[0].contains("a different image")), "{r:?}");
    assert!(!packs.join(PACK_DIR).exists());
    let _ = std::fs::remove_dir_all(&root);
}
