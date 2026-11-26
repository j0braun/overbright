use std::collections::HashMap;

use ob_formats::formats::solid::{Material, Solid, Vertex};
use ob_formats::formats::texture::{Blend, PixelFormat, Scroll, Texture};

use super::*;

fn v(p: [f32; 3], uv: [f32; 2]) -> Vertex {
    Vertex { position: p, color: [7, 8, 9, 255], uv }
}

fn orig(lit: bool) -> Solid {
    Solid {
        name: "XC_CONE".into(),
        hash: 0xAB,
        bbox_min: [0.0; 3],
        bbox_max: [1.0, 1.0, 2.0],
        textures: vec![10, 11],
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
            Material { texture_slot: 0, light_material: 0xFF, first_index: 0, index_count: 3, flags: 5 },
            Material { texture_slot: 1, light_material: 0xFF, first_index: 3, index_count: 3, flags: 6 },
        ],
        mesh_flags: 0,
        markers: vec![],
        light_markers: vec![],
    }
}

fn tex(hash: u32, name: &str) -> Texture {
    Texture {
        name: name.into(),
        hash,
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
    }
}

fn textures() -> HashMap<u32, Texture> {
    HashMap::from([(10, tex(10, "ASPHALT")), (11, tex(11, "PAINT"))])
}

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ob-import-{tag}-{}", ob_core::unique_tag()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn exported(tag: &str, s: &Solid) -> PathBuf {
    let d = dir(tag);
    gltf::write_reference(&d, None, s, &textures(), "L4RA").unwrap().gltf
}

#[test]
fn an_export_comes_back_the_same_shape() {
    let o = orig(true);
    let f = read_gltf(&exported("rt", &o)).unwrap();
    assert_eq!(f.model.as_deref(), Some("XC_CONE"));
    assert_eq!(f.origin.as_deref(), Some("from-game-files"));
    let imp = map_onto(&f, &o, &textures()).unwrap();
    assert_eq!(imp.parts, vec![("ASPHALT".to_string(), 1), ("PAINT".to_string(), 1)]);
    assert!(same_shape(&imp.solid, &o));
    // Same vertices: the original's baked colours stay, and each part keeps its draw flags.
    assert_eq!(imp.solid.vertices[2].color, [7, 8, 9, 255]);
    assert_eq!(imp.solid.materials[1].flags, 6);
    assert_eq!(imp.solid.materials[1].texture_slot, 1);
    assert_eq!(imp.solid.normals.len(), 4);
}

#[test]
fn an_edit_in_the_file_moves_a_vertex() {
    let o = orig(false);
    let p = exported("edit", &o);
    let bin = p.with_extension("bin");
    let mut b = std::fs::read(&bin).unwrap();
    // The first position's x (game x stays glTF x): 0.0 -> 3.0.
    b[0..4].copy_from_slice(&3.0f32.to_le_bytes());
    std::fs::write(&bin, b).unwrap();
    let imp = map_onto(&read_gltf(&p).unwrap(), &o, &textures()).unwrap();
    assert!((imp.solid.vertices[0].position[0] - 3.0).abs() < 1e-4);
    assert!(!same_shape(&imp.solid, &o));
    assert_eq!(imp.solid.bbox_min[0], 0.0);
    assert!((imp.solid.bbox_max[0] - 3.0).abs() < 1e-4);
}

#[test]
fn what_cannot_be_mapped_is_refused_with_the_reason() {
    let o = orig(false);
    let f = read_gltf(&exported("refuse", &o)).unwrap();
    // A material that is not a part, and a part with no triangles.
    let mut g = f.clone();
    g.prims[1].material = "CHROME".into();
    let e = map_onto(&g, &o, &textures()).unwrap_err().to_string();
    assert!(e.contains("`CHROME` is not a part of XC_CONE"), "{e}");
    assert!(e.contains("`PAINT` has no triangles"), "{e}");
    // Another model's file.
    let mut g = f.clone();
    g.model = Some("XC_OTHER".into());
    assert!(map_onto(&g, &o, &textures()).unwrap_err().to_string().contains("exported from XC_OTHER"));
    let mut g = f.clone();
    g.hash = Some(1);
    assert!(map_onto(&g, &o, &textures()).unwrap_err().to_string().contains("hash"));
    // No UVs; an index past the vertices; too many vertices; a lit model without normals.
    let mut g = f.clone();
    g.sets[0].uvs.clear();
    assert!(map_onto(&g, &o, &textures()).unwrap_err().to_string().contains("texture coordinates"));
    let mut g = f.clone();
    g.prims[0].indices[0] = 99;
    assert!(map_onto(&g, &o, &textures()).unwrap_err().to_string().contains("past its last vertex"));
    let mut g = f.clone();
    g.sets[0].positions = vec![[0.0; 3]; 70_000];
    g.sets[0].uvs = vec![[0.0; 2]; 70_000];
    assert!(map_onto(&g, &o, &textures()).unwrap_err().to_string().contains("at most 65535"));
    let lit = orig(true);
    let mut g = read_gltf(&exported("lit", &lit)).unwrap();
    g.sets[0].normals.clear();
    assert!(map_onto(&g, &lit, &textures()).unwrap_err().to_string().contains("normals"));
}

#[test]
fn foreign_files_that_are_not_models_are_refused() {
    let d = dir("bad");
    let w = |name: &str, s: &str| {
        let p = d.join(name);
        std::fs::write(&p, s).unwrap();
        p
    };
    let e = |p: PathBuf| read_gltf(&p).unwrap_err().to_string();
    assert!(e(w("a.gltf", "not json")).contains("not a glTF"));
    assert!(e(w("b.gltf", r#"{"asset":{"version":"1.0"}}"#)).contains("glTF 2.0"));
    assert!(e(w("c.gltf", r#"{"asset":{"version":"2.0"},"skins":[{}]}"#)).contains("skins"));
    assert!(
        e(w("d.gltf", r#"{"asset":{"version":"2.0"},"buffers":[{"uri":"data:x","byteLength":1}]}"#))
            .contains("data URI")
    );
    assert!(
        e(w("e.gltf", r#"{"asset":{"version":"2.0"},"buffers":[{"uri":"../x.bin","byteLength":1}]}"#))
            .contains("beside it")
    );
    assert!(
        e(w("f.gltf", r#"{"asset":{"version":"2.0"},"buffers":[{"uri":"f.bin","byteLength":1}]}"#))
            .contains("not beside it")
    );
    std::fs::write(d.join("g.bin"), [0u8; 4]).unwrap();
    assert!(
        e(w("g.gltf", r#"{"asset":{"version":"2.0"},"buffers":[{"uri":"g.bin","byteLength":4}]}"#))
            .contains("no model")
    );
    let mut glb = b"glTF".to_vec();
    glb.extend([0u8; 8]);
    std::fs::write(d.join("h.glb"), glb).unwrap();
    assert!(e(d.join("h.glb")).contains(".glb"));
    // Accessors reading past the data are caught, not trusted.
    let mut doc = serde_json::json!({
        "asset": {"version": "2.0"}, "scene": 0, "scenes": [{"nodes": [0]}], "nodes": [{"mesh": 0}],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}}]}],
        "buffers": [{"uri": "g.bin", "byteLength": 4}],
        "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": 4}],
        "accessors": [{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"}],
    });
    assert!(e(w("i.gltf", &doc.to_string())).contains("past the end"));
    doc["meshes"][0]["primitives"][0]["mode"] = serde_json::json!(1);
    assert!(e(w("j.gltf", &doc.to_string())).contains("not a list of triangles"));
}

#[test]
fn a_derived_model_stays_local_and_an_own_one_is_shared() {
    let o = orig(true);
    let f = read_gltf(&exported("keep", &o)).unwrap();
    let imp = map_onto(&f, &o, &textures()).unwrap();
    let m = dir("mod");
    let k = write_model(&m, &imp, &textures(), "L4RA", false).unwrap();
    assert!(!k.shared);
    assert!(k.dir.ends_with(".editor/models"));
    assert!(std::fs::read_to_string(&k.record).unwrap().contains("share = \"local-only\""));
    assert!(k.dir.join("XC_CONE-ASPHALT.png").is_file() && k.dir.join(gltf::NOTE_FILE).is_file());
    // Nothing of it is in a shared copy.
    let shared = ob_map::tools::map::share::files(&m).unwrap();
    assert!(shared.is_empty(), "{shared:?}");

    // An unchanged shape is not "own"; a file still marked from the game is not either.
    assert!(check_own(&f, &imp, &o).unwrap_err().to_string().contains("from your game files"));
    let mut unmarked = f.clone();
    unmarked.origin = None;
    assert!(check_own(&unmarked, &imp, &o).unwrap_err().to_string().contains("unchanged"));
    let mut edited = imp.clone();
    edited.solid.vertices[0].position[0] = 9.0;
    check_own(&unmarked, &edited, &o).unwrap();
    let m2 = dir("own");
    let k = write_model(&m2, &edited, &textures(), "L4RA", true).unwrap();
    assert!(k.shared && k.dir.ends_with("models"));
    let text = std::fs::read_to_string(&k.gltf).unwrap();
    assert!(text.contains("\"origin\": \"own\"") && !text.contains(".png"));
    let shared: Vec<String> = ob_map::tools::map::share::files(&m2).unwrap().into_iter().map(|f| f.path).collect();
    assert_eq!(shared, vec!["models/XC_CONE.bin", "models/XC_CONE.gltf", "models/XC_CONE.model.toml"]);
    // And it reads back as a model of the same parts.
    let back = map_onto(&read_gltf(&k.gltf).unwrap(), &o, &textures()).unwrap();
    assert!((back.solid.vertices[0].position[0] - 9.0).abs() < 1e-4);
}
