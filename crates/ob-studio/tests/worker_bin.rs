//! The isolated file reader as the real program: `overbright-studio-cli --worker-read-gltf` reads a good file, says
//! why it refuses a bad one, and a file that is not a glTF cannot hurt the caller.

use std::path::PathBuf;

use ob_studio::browser::worker;

fn worker_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_overbright-studio-cli"))
}

#[test]
fn the_worker_program_reads_and_refuses() {
    use ob_formats::formats::solid::{Material, Solid, Vertex};
    let d = std::env::temp_dir().join(format!("ob-workerbin-{}", ob_core::unique_tag()));
    std::fs::create_dir_all(&d).unwrap();
    let v = |p: [f32; 3]| Vertex { position: p, color: [255; 4], uv: [0.0, 0.0] };
    let s = Solid {
        name: "A".into(),
        hash: 1,
        bbox_min: [0.0; 3],
        bbox_max: [1.0; 3],
        textures: vec![],
        vertices: vec![v([0.0; 3]), v([1.0, 0.0, 0.0]), v([0.0, 1.0, 0.0])],
        normals: vec![],
        light_materials: vec![],
        indices: vec![0, 1, 2],
        materials: vec![Material { texture_slot: 0, light_material: 0xFF, first_index: 0, index_count: 3, flags: 0 }],
        mesh_flags: 0,
        markers: vec![],
        light_markers: vec![],
    };
    let r = ob_studio::browser::gltf::write_reference(&d, None, &s, &Default::default(), "L4RA").unwrap();
    let f = worker::read_with(&worker_exe(), &r.gltf).unwrap();
    assert_eq!(f.model.as_deref(), Some("A"));
    assert_eq!(f.prims.len(), 1);
    assert_eq!(f.sets[0].positions.len(), 3);
    // Not a model: the reader's own refusal comes through.
    let bad = d.join("bad.gltf");
    std::fs::write(&bad, "{ nonsense").unwrap();
    let e = worker::read_with(&worker_exe(), &bad).unwrap_err().to_string();
    assert!(e.contains("not a glTF"), "{e}");
    let e = worker::read_with(&worker_exe(), &d.join("missing.gltf")).unwrap_err().to_string();
    assert!(e.contains("cannot read"), "{e}");
}
