use std::collections::HashMap;

use ob_formats::formats::solid::{Material, Solid, Vertex};
use ob_formats::formats::texture::{Blend, PixelFormat, Scroll, Texture};

use nfsu2::studio_api::world::{HIDDEN_FLAG, Placement};

use super::*;

fn v(p: [f32; 3], uv: [f32; 2]) -> Vertex {
    Vertex { position: p, color: [255; 4], uv }
}

fn quad(lit: bool) -> Solid {
    Solid {
        name: "XC_CONE/01".into(),
        hash: 0xAB,
        bbox_min: [0.0; 3],
        bbox_max: [1.0; 3],
        textures: vec![10, 99],
        vertices: vec![
            v([0.0, 0.0, 0.0], [0.0, 0.0]),
            v([1.0, 0.0, 0.0], [1.0, 0.0]),
            v([1.0, 1.0, 2.0], [1.0, 1.0]),
            v([0.0, 1.0, 2.0], [0.0, 1.0]),
        ],
        normals: if lit { vec![[0.0, 0.0, 1.0]; 4] } else { vec![] },
        light_materials: vec![],
        indices: vec![0, 1, 2, 0, 2, 3],
        materials: vec![
            Material { texture_slot: 0, light_material: 0xFF, first_index: 0, index_count: 3, flags: 0 },
            // Slot 1 names a texture the region does not have; slot 2 does not exist at all.
            Material { texture_slot: 1, light_material: 0xFF, first_index: 3, index_count: 3, flags: 0 },
            // A range past the indices is skipped, not trusted.
            Material { texture_slot: 0, light_material: 0xFF, first_index: 4, index_count: 9, flags: 0 },
        ],
        mesh_flags: 0,
        markers: vec![],
        light_markers: vec![],
    }
}

fn textures() -> HashMap<u32, Texture> {
    let t = Texture {
        name: "ROAD ASPHALT".into(),
        hash: 10,
        width: 2,
        height: 2,
        format: PixelFormat::Rgba8,
        mip_count: 1,
        data: vec![200u8; 16].into(),
        palette: vec![],
        alpha_test: false,
        blend: Blend::None,
        rendering_order: 0,
        late_buckets: false,
        scroll: Scroll::default(),
    };
    HashMap::from([(10, t)])
}

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ob-gltf-{tag}-{}", ob_core::unique_tag()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

#[test]
fn json_is_a_valid_marked_reference() {
    let tex = textures();
    let (doc, buf, wanted) = build(&quad(true), &tex, "cone", "L4RA").unwrap();
    assert_eq!(doc["asset"]["version"], "2.0");
    assert_eq!(doc["asset"]["extras"]["origin"], "from-game-files");
    assert_eq!(doc["asset"]["extras"]["share"], "local-only");
    assert_eq!(buf.len() % 4, 0);
    assert_eq!(doc["buffers"][0]["byteLength"], buf.len());
    // Two usable ranges: one primitive each, the out-of-range one dropped.
    let prims = doc["meshes"][0]["primitives"].as_array().unwrap();
    assert_eq!(prims.len(), 2);
    assert!(prims[0]["attributes"].get("NORMAL").is_some());
    assert_eq!(doc["accessors"][0]["min"], json!([0.0, 0.0, 0.0]));
    assert_eq!(doc["accessors"][0]["max"], json!([1.0, 1.0, 2.0]));
    // Every view and accessor stays inside the buffer.
    for bv in doc["bufferViews"].as_array().unwrap() {
        assert!(bv["byteOffset"].as_u64().unwrap() + bv["byteLength"].as_u64().unwrap() <= buf.len() as u64);
    }
    // Z up becomes Y up through the root node.
    assert_eq!(doc["nodes"][0]["name"], "OB_Z_UP");
    // The known texture is wanted as a PNG named from the model and the texture; the missing one is only a name.
    assert_eq!(wanted.len(), 1);
    assert_eq!(wanted[0].0, "cone-ROAD_ASPHALT.png");
    assert_eq!(doc["materials"][1]["name"], "slot1");
}

#[test]
fn unlit_models_have_no_normals() {
    let (doc, _, _) = build(&quad(false), &textures(), "cone", "L4RA").unwrap();
    assert!(doc["meshes"][0]["primitives"][0]["attributes"].get("NORMAL").is_none());
}

#[test]
fn bad_models_are_refused() {
    let mut s = quad(false);
    s.indices[0] = 9;
    assert!(build(&s, &textures(), "x", "R").unwrap_err().contains("past its last vertex"));
    let mut s = quad(false);
    s.vertices.clear();
    assert!(build(&s, &textures(), "x", "R").unwrap_err().contains("no geometry"));
    let mut s = quad(false);
    s.materials.clear();
    assert!(build(&s, &textures(), "x", "R").unwrap_err().contains("no drawable"));
}

#[test]
fn writes_the_files_and_the_local_only_note() {
    let d = dir("write");
    let r = write_reference(&d, None, &quad(true), &textures(), "L4RA").unwrap();
    assert!(r.gltf.ends_with("XC_CONE_01.gltf"), "{:?}", r.gltf);
    let json: Value = serde_json::from_slice(&std::fs::read(&r.gltf).unwrap()).unwrap();
    let len = std::fs::metadata(&r.bin).unwrap().len();
    assert_eq!(json["buffers"][0]["byteLength"].as_u64().unwrap(), len);
    assert_eq!(r.textures.len(), 1);
    let png = image::open(&r.textures[0]).unwrap();
    assert_eq!((png.width(), png.height()), (2, 2));
    let note = std::fs::read_to_string(&r.note).unwrap();
    assert!(note.contains("from-game-files") && note.contains("local-only"));
    // Again: replaced, nothing else appears.
    let n = std::fs::read_dir(&d).unwrap().count();
    write_reference(&d, None, &quad(true), &textures(), "L4RA").unwrap();
    assert_eq!(std::fs::read_dir(&d).unwrap().count(), n);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn never_into_the_game_folder() {
    let game = dir("game");
    std::fs::create_dir_all(&game).unwrap();
    for target in [game.clone(), game.join("TRACKS").join("new")] {
        let e = write_reference(&target, Some(&game), &quad(false), &textures(), "L4RA").unwrap_err();
        assert!(e.contains("game's own folder"), "{e}");
    }
    assert!(!game.join("TRACKS").exists(), "refused before anything was made");
    assert!(check_target(&dir("else"), Some(&game)).is_ok());
    let _ = std::fs::remove_dir_all(&game);
}

#[test]
fn names_cannot_leave_the_folder_and_the_mod_default_is_unshared() {
    assert_eq!(file_stem("../../etc/passwd"), "______etc_passwd");
    assert_eq!(file_stem(""), "model");
    let r = reference_dir(Path::new("/m/mymod"));
    assert!(r.starts_with("/m/mymod/.editor"));
}

fn placed(solid: usize, at: [f32; 3], flags: u16) -> Placement {
    Placement {
        solid,
        low_solid: None,
        mid_solid: None,
        transform: glam::Mat4::from_translation(glam::Vec3::from(at)),
        section: 1407,
        flags,
        cull_radius: 1.0,
    }
}

/// A cone placed twice near the origin (once hidden), once far away, and a second model nearby.
fn area_world() -> (Vec<Placement>, Vec<Solid>) {
    let mut other = quad(false);
    other.name = "XB_BENCH_01".into();
    other.hash = 0xCD;
    (
        vec![
            placed(0, [5.0, 0.0, 0.0], 0),
            placed(0, [-9.0, 3.0, 0.0], 0),
            placed(0, [1.0, 1.0, 0.0], HIDDEN_FLAG),
            placed(0, [500.0, 0.0, 0.0], 0),
            placed(1, [20.0, 0.0, 1.0], 0),
        ],
        vec![quad(true), other],
    )
}

#[test]
fn an_area_holds_each_near_shown_placement_once_per_model() {
    let (p, s) = area_world();
    let d = dir("area");
    let r = write_area(&d, None, &p, &s, &textures(), "L4RA", glam::Vec3::ZERO, 30.0).unwrap();
    assert_eq!((r.placements, r.models, r.skipped), (3, 2, 0));
    assert!(r.files.gltf.ends_with("area-L4RA.gltf"), "{:?}", r.files.gltf);
    let doc: Value = serde_json::from_slice(&std::fs::read(&r.files.gltf).unwrap()).unwrap();
    assert_eq!(doc["asset"]["extras"]["origin"], "from-game-files");
    assert_eq!(doc["asset"]["extras"]["share"], "local-only");
    assert_eq!(doc["asset"]["extras"]["area"]["radius"], 30.0);
    // The root, then the nearest first: the cone at 5 m carries its translation as a matrix.
    let nodes = doc["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 4);
    assert_eq!(nodes[1]["name"], "XC_CONE/01");
    assert_eq!(nodes[1]["matrix"][12], 5.0);
    assert_eq!(nodes[1]["mesh"], nodes[2]["mesh"], "the two cones share their mesh");
    assert_ne!(nodes[1]["mesh"], nodes[3]["mesh"]);
    // The texture both models draw is one material and one file.
    assert_eq!(r.files.textures.len(), 1);
    let len = std::fs::metadata(&r.files.bin).unwrap().len();
    assert_eq!(doc["buffers"][0]["byteLength"].as_u64().unwrap(), len);
    for bv in doc["bufferViews"].as_array().unwrap() {
        assert!(bv["byteOffset"].as_u64().unwrap() + bv["byteLength"].as_u64().unwrap() <= len);
    }
    assert!(std::fs::read_to_string(&r.files.note).unwrap().contains("local-only"));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn an_area_refuses_the_game_folder_an_empty_area_and_a_huge_one() {
    let (p, s) = area_world();
    let game = dir("area-game");
    std::fs::create_dir_all(&game).unwrap();
    let e = write_area(&game.join("x"), Some(&game), &p, &s, &textures(), "L4RA", glam::Vec3::ZERO, 30.0).unwrap_err();
    assert!(e.contains("game's own folder"), "{e}");
    assert!(!game.join("x").exists());
    let d = dir("area-empty");
    let e = write_area(&d, None, &p, &s, &textures(), "L4RA", glam::Vec3::new(9000.0, 0.0, 0.0), 30.0).unwrap_err();
    assert!(e.contains("nothing with geometry"), "{e}");
    assert!(!d.exists(), "nothing is made for an empty area");
    let e = write_area(&d, None, &p, &s, &textures(), "L4RA", glam::Vec3::ZERO, 5000.0).unwrap_err();
    assert!(e.contains("radius"), "{e}");
    let many: Vec<Placement> = (0..AREA_MAX_PLACEMENTS + 1).map(|_| placed(0, [1.0, 0.0, 0.0], 0)).collect();
    let e = write_area(&d, None, &many, &s, &textures(), "L4RA", glam::Vec3::ZERO, 30.0).unwrap_err();
    assert!(e.contains("too many"), "{e}");
    let _ = std::fs::remove_dir_all(&game);
}

#[test]
fn a_model_without_geometry_is_left_out_and_counted() {
    let (p, mut s) = area_world();
    s[1].vertices.clear();
    let d = dir("area-skip");
    let r = write_area(&d, None, &p, &s, &textures(), "L4RA", glam::Vec3::ZERO, 30.0).unwrap();
    assert_eq!((r.placements, r.models, r.skipped), (2, 1, 1));
    let _ = std::fs::remove_dir_all(&d);
}
