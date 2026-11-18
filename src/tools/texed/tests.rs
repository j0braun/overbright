use super::*;
use crate::formats::texture::{Blend, Scroll};

fn tex(name: &str, w: u32, h: u32, format: PixelFormat, data: Vec<u8>, mips: u32) -> Texture {
    Texture {
        name: name.into(),
        hash: crate::formats::hash::bin_hash(name),
        width: w,
        height: h,
        format,
        mip_count: mips,
        data: data.into(),
        palette: Vec::new(),
        alpha_test: false,
        blend: Blend::None,
        rendering_order: 0,
        late_buckets: false,
        scroll: Scroll::default(),
    }
}

struct Tmp(PathBuf);
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn tmp(tag: &str) -> Tmp {
    let p = std::env::temp_dir().join(format!("ob-texed-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    Tmp(p)
}

fn dirs(t: &Tmp) -> Dirs {
    Dirs { packs: t.0.join("packs"), compiled: t.0.join("compiled"), exports: t.0.join("exports") }
}

fn wait(e: &mut Editor) {
    let start = Instant::now();
    while !e.jobs.is_empty() {
        assert!(start.elapsed() < Duration::from_secs(30), "job hangs");
        std::thread::sleep(Duration::from_millis(5));
        e.poll(Instant::now());
    }
}

#[test]
fn checker_tints_cells_and_keeps_cutouts_visible() {
    let rgba = [100u8, 100, 100, 0].repeat(16 * 16);
    let out = checker(&rgba, 16, 16);
    // Cell 2 × 2 texels: (0,0) magenta-ish, (2,0) cyan-ish.
    assert_eq!(&out[..4], &[216, 25, 216, 160]);
    assert_eq!(&out[2 * 4..2 * 4 + 4], &[25, 216, 216, 160]);
    assert_eq!(checker(&[1, 2, 3, 255], 1, 1)[3], 255);
}

#[test]
fn exports_round_trip() {
    // A DXT1 texture: its DDS holds its own blocks; its PNG its decoded pixels.
    let up = crate::upscale::Upscaled { width: 8, height: 8, rgba: [10u8, 200, 30, 255].repeat(64), scale: 1 };
    let e = crate::upscale::encode(&up);
    let t = tex("SIGN_A", 8, 8, e.format, e.data.clone(), e.mips);
    let dds = crate::upscale::parse_dds(&dds_of(&t)).unwrap();
    assert_eq!((dds.format, dds.mips, &dds.data), (e.format, e.mips, &e.data));
    let png = obpack::decode_png(&png_of(&t).unwrap()).unwrap();
    assert_eq!(png.as_raw(), &t.decode_rgba());
    // An RGBA texture's DDS is BC7.
    let t = tex("GLOW", 4, 4, PixelFormat::Rgba8, [9u8, 8, 7, 255].repeat(16), 1);
    assert_eq!(obpack::parse_any_dds(&dds_of(&t)).unwrap().format, PixelFormat::Bc7);
    assert_eq!(export_stem(&t), "GLOW");
    let odd = Texture { name: "a b".into(), ..t };
    assert_eq!(export_stem(&odd), format!("0x{:08X}", odd.hash));
}

#[test]
fn browse_export_edit_import_save_revert() {
    let t = tmp("flow");
    let mut ed = Editor::new(dirs(&t));
    assert!(!recording());
    ed.toggle();
    assert!(ed.open && recording());
    // The renderers' notes: one world slot, one car texture.
    let road = tex("ROAD_MAIN", 8, 8, PixelFormat::Rgba8, [50u8, 60, 70, 255].repeat(64), 1);
    let body = tex("CARSKIN", 4, 4, PixelFormat::Rgba8, [1u8, 2, 3, 255].repeat(16), 1);
    note_world_slots([3usize].into_iter());
    note_textures(true, [&body].into_iter());
    let r = road.clone();
    ed.take_frame(&|slot| (slot == 3).then(|| r.clone()), Instant::now());
    assert_eq!(ed.rows.len(), 2);
    // The panel lists them; a click on the first row selects it.
    let font = font();
    let built = ed.panel(&font, &Input::default()).unwrap();
    let texts = built.texts();
    assert!(texts.iter().any(|s| s.starts_with("ROAD_MAIN 8x8 RGBA8")), "{texts:?}");
    assert!(texts.iter().any(|s| s.starts_with("car CARSKIN 4x4")), "{texts:?}");
    ed.list.selected = Some(0);
    assert_eq!(ed.selected().unwrap().texture.name, "ROAD_MAIN");
    // Nothing exported: import says what to do.
    assert!(ed.import_selected().unwrap_err().to_string().contains("export it first"));
    let png = ed.export(false).unwrap();
    assert_eq!(png, t.0.join("exports/ROAD_MAIN.png"));
    // "Edit" it: red.
    let mut red = Vec::new();
    image::RgbaImage::from_pixel(16, 16, image::Rgba([255, 0, 0, 255]))
        .write_to(&mut std::io::Cursor::new(&mut red), image::ImageFormat::Png)
        .unwrap();
    std::fs::write(&png, &red).unwrap();
    ed.author.text = "Tester".into();
    ed.licence.text = "CC0-1.0".into();
    ed.import_selected().unwrap();
    wait(&mut ed);
    assert!(ed.status.1, "{:?}", ed.status);
    let id = ed.workspace.clone().unwrap();
    assert_eq!(id, "my-textures");
    let pack = obpack::Pack::open(&t.0.join("packs").join(&id)).unwrap();
    assert_eq!(pack.index.world[&road.hash].file, PathBuf::from("world/ROAD_MAIN.png"));
    assert_eq!(pack.manifest.author, "Tester");
    let compiled = t.0.join("compiled").join(&id).join("world").join(format!("{:08X}.dds", road.hash));
    let first = std::fs::read(&compiled).unwrap();
    assert_eq!(obpack::load_order(&t.0.join("packs"))[0].id, id, "the workspace pack first");
    // Saved again: re-imported by the watch (hot reload).
    std::thread::sleep(Duration::from_millis(20));
    let mut blue = Vec::new();
    image::RgbaImage::from_pixel(16, 16, image::Rgba([0, 0, 255, 255]))
        .write_to(&mut std::io::Cursor::new(&mut blue), image::ImageFormat::Png)
        .unwrap();
    std::fs::write(&png, &blue).unwrap();
    ed.poll(Instant::now() + Duration::from_secs(10));
    wait(&mut ed);
    assert_ne!(std::fs::read(&compiled).unwrap(), first, "rebuilt from the new save");
    // Save pack: the manifest from the fields.
    ed.name.text = "Night Roads".into();
    ed.save_pack().unwrap();
    wait(&mut ed);
    let pack = obpack::Pack::open(&t.0.join("packs").join(&id)).unwrap();
    assert_eq!((pack.manifest.name.as_str(), pack.manifest.licence.as_str()), ("Night Roads", "CC0-1.0"));
    // Highlight on and off (the loader's stand-in).
    ed.act("highlight");
    assert_eq!(ed.highlight, Some((Domain::World, road.hash)));
    ed.act("highlight");
    assert_eq!(ed.highlight, None);
    // Revert: out of the pack and the compiled folder.
    ed.revert_selected().unwrap();
    assert!(!t.0.join("packs").join(&id).join("world/ROAD_MAIN.png").exists());
    assert!(!compiled.exists());
    // Another pack loaded as the workspace.
    let m = Manifest { name: "Other".into(), author: "B".into(), ..Default::default() };
    obpack::create(&t.0.join("packs"), "other", &m).unwrap();
    ed.load_pack("other").unwrap();
    wait(&mut ed);
    assert_eq!((ed.workspace.as_deref(), ed.author.text.as_str()), (Some("other"), "B"));
    assert_eq!(obpack::load_order(&t.0.join("packs"))[0].id, "other");
    ed.toggle();
    assert!(!recording());
}

fn font() -> crate::frontend::font::Font {
    use crate::frontend::font::{Font, Glyph};
    let glyph = Glyph {
        width: 5.0,
        height: 10.0,
        x: 0.0,
        y: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
        advance: 5.0,
        kerning: Vec::new(),
    };
    let glyphs = (32u8..127).map(|c| (u32::from(c), glyph.clone())).collect();
    Font { name: "t".into(), texture: "t".into(), line_height: 30.0, texture_size: [64.0, 64.0], glyphs }
}
