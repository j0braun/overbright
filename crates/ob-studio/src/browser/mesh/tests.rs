use std::collections::HashMap;

use ob_formats::formats::solid::{Material, Solid, Vertex};
use ob_formats::formats::texture::{Blend, PixelFormat, Scroll, Texture};

use super::*;

fn v(p: [f32; 3]) -> Vertex {
    Vertex { position: p, color: [255; 4], uv: [0.0; 2] }
}

fn solid() -> Solid {
    Solid {
        name: "XC_CONE_01".into(),
        hash: 0xAB,
        bbox_min: [0.0; 3],
        bbox_max: [1.0; 3],
        textures: vec![10, 99],
        vertices: vec![v([0.0, 0.0, 0.0]), v([1.0, 0.0, 0.0]), v([1.0, 2.0, 3.0]), v([0.0, 2.0, 3.0])],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        light_materials: vec![],
        indices: vec![0, 1, 2, 0, 2, 3, 0, 1, 3],
        materials: vec![
            Material { texture_slot: 0, light_material: 0xFF, first_index: 0, index_count: 3, flags: 0 },
            Material { texture_slot: 1, light_material: 0xFF, first_index: 3, index_count: 3, flags: 0 },
            // The same slot again joins its part; a range past the indices is skipped.
            Material { texture_slot: 0, light_material: 0xFF, first_index: 6, index_count: 3, flags: 0 },
            Material { texture_slot: 0, light_material: 0xFF, first_index: 8, index_count: 9, flags: 0 },
        ],
        mesh_flags: 0,
        markers: vec![],
        light_markers: vec![],
    }
}

fn textures() -> HashMap<u32, Texture> {
    let t = Texture {
        name: "ROAD_ASPHALT".into(),
        hash: 10,
        width: 2,
        height: 2,
        format: PixelFormat::Rgba8,
        mip_count: 1,
        data: vec![0u8; 16].into(),
        palette: vec![],
        alpha_test: false,
        blend: Blend::None,
        rendering_order: 0,
        late_buckets: false,
        scroll: Scroll::default(),
    };
    HashMap::from([(10, t)])
}

#[test]
fn the_facts_count_the_geometry_and_group_parts_by_slot() {
    let f = facts(&solid(), &textures());
    assert_eq!((f.vertices, f.triangles, f.lit), (4, 3, true));
    assert_eq!(f.size, [1.0, 2.0, 3.0]);
    // The glTF export's part names: the texture's name, else slot<N>.
    assert_eq!(
        f.parts,
        [
            Part { name: "ROAD_ASPHALT".into(), triangles: 2, texture: Some(10) },
            Part { name: "slot1".into(), triangles: 1, texture: Some(99) },
        ]
    );
    let l = f.lines();
    assert_eq!(l[0], "4 vertices · 3 triangles · lit (normals)");
    assert_eq!(l[1], "Size 1.00 × 2.00 × 3.00 m (Z up)");
    assert_eq!(l[2], "Part ROAD_ASPHALT: 2 triangles");
}

#[test]
fn a_model_without_geometry_has_zero_size() {
    let mut s = solid();
    s.vertices.clear();
    s.indices.clear();
    s.materials.clear();
    let f = facts(&s, &textures());
    assert_eq!((f.vertices, f.triangles, f.size), (0, 0, [0.0; 3]));
    assert!(f.parts.is_empty());
}
