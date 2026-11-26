use std::collections::{HashMap, HashSet};

use ob_formats::formats::solid::{Material, Solid};
use ob_formats::formats::texture::{Blend, PixelFormat, Scroll, Texture};

use super::*;

fn solid(name: &str, hash: u32, textures: &[u32], materials: &[u8]) -> Solid {
    Solid {
        name: name.into(),
        hash,
        bbox_min: [-0.5; 3],
        bbox_max: [0.5; 3],
        textures: textures.to_vec(),
        vertices: vec![],
        normals: vec![],
        light_materials: vec![],
        indices: vec![0; 6],
        materials: materials
            .iter()
            .map(|&s| Material { texture_slot: s, light_material: 0xFF, first_index: 0, index_count: 3, flags: 0 })
            .collect(),
        mesh_flags: 0,
        markers: vec![],
        light_markers: vec![],
    }
}

fn texture(name: &str, hash: u32, side: u32) -> Texture {
    Texture {
        name: name.into(),
        hash,
        width: side,
        height: side,
        format: PixelFormat::Rgba8,
        mip_count: 1,
        data: vec![0u8; (side * side * 4) as usize].into(),
        palette: vec![],
        alpha_test: false,
        blend: Blend::None,
        rendering_order: 0,
        late_buckets: false,
        scroll: Scroll::default(),
    }
}

fn place(solid: usize, section: u32, flags: u16) -> Placement {
    Placement {
        solid,
        low_solid: None,
        mid_solid: None,
        transform: glam::Mat4::IDENTITY,
        section,
        flags,
        cull_radius: 1.0,
    }
}

/// Three models (a cone placed twice in two sections, a hidden bench, a sign used only as a lower detail level)
/// and four textures.
fn index() -> Index {
    let solids = vec![
        solid("XC_CONE_01", 1, &[10, 11], &[0, 1, 1]),
        solid("XB_BENCH_01", 2, &[11], &[0]),
        solid("XS_SIGN_BIG", 3, &[12], &[0]),
    ];
    let mut placements = vec![place(0, 1407, 0), place(0, 1408, 0), place(1, 1407, HIDDEN_FLAG)];
    placements[1].low_solid = Some(2);
    let textures: HashMap<u32, Texture> = [
        texture("ROAD_ASPHALT", 10, 64),
        texture("PAINT_YELLOW", 11, 1024),
        texture("SIGN_FACE", 12, 32),
        texture("LEFTOVER", 13, 16),
    ]
    .into_iter()
    .map(|t| (t.hash, t))
    .collect();
    let knock = HashSet::from([0]);
    let animated = HashSet::from([12]);
    Index::from_parts("L4RA", &solids, &placements, &textures, &knock, &animated)
}

fn names(i: &Index, hits: &[Hit]) -> Vec<String> {
    hits.iter().map(|h| i.name(*h).to_string()).collect()
}

#[test]
fn models_and_textures_are_found_by_name_across_kinds() {
    let i = index();
    assert_eq!(i.search(&Query::default()).len(), 7);
    let q = Query { text: "sign".into(), ..Default::default() };
    assert_eq!(names(&i, &i.search(&q)), ["XS_SIGN_BIG", "SIGN_FACE"]);
    let q = Query { text: "paint yellow".into(), ..Default::default() };
    assert_eq!(names(&i, &i.search(&q)), ["PAINT_YELLOW"]);
    assert_eq!(i.counts("sign"), (1, 1));
    let q = Query { text: "sign".into(), kind: Kind::Models, ..Default::default() };
    assert_eq!(names(&i, &i.search(&q)), ["XS_SIGN_BIG"]);
    assert!(i.search(&Query { text: "zebra".into(), ..Default::default() }).is_empty());
}

#[test]
fn a_hash_finds_only_that_texture_or_model() {
    let i = index();
    let q = Query { text: "0xB".into(), ..Default::default() };
    assert_eq!(names(&i, &i.search(&q)), ["PAINT_YELLOW"]);
    let q = Query { text: "0x2".into(), ..Default::default() };
    assert_eq!(names(&i, &i.search(&q)), ["XB_BENCH_01"]);
}

#[test]
fn the_filters_of_each_kind_narrow_their_own_results() {
    let i = index();
    let q = Query { kind: Kind::Models, models: Filter::KnockOver, ..Default::default() };
    assert_eq!(names(&i, &i.search(&q)), ["XC_CONE_01"]);
    let q = Query { kind: Kind::Textures, textures: TexFilter::Unused, ..Default::default() };
    // The sign is placed only as a lower level of detail: its texture is drawn by no placed model.
    assert_eq!(names(&i, &i.search(&q)), ["LEFTOVER", "SIGN_FACE"]);
    let q = Query { kind: Kind::Textures, textures: TexFilter::Large, ..Default::default() };
    assert_eq!(names(&i, &i.search(&q)), ["PAINT_YELLOW"]);
    let q = Query { kind: Kind::Textures, textures: TexFilter::Animated, ..Default::default() };
    assert_eq!(names(&i, &i.search(&q)), ["SIGN_FACE"]);
    let q = Query { kind: Kind::Textures, textures: TexFilter::InUse, ..Default::default() };
    assert_eq!(names(&i, &i.search(&q)), ["PAINT_YELLOW", "ROAD_ASPHALT"]);
}

#[test]
fn a_section_restricts_both_kinds() {
    let i = index();
    let q = Query { text: "section:1408".into(), ..Default::default() };
    assert_eq!(names(&i, &i.search(&q)), ["XC_CONE_01", "PAINT_YELLOW", "ROAD_ASPHALT"]);
    let q = Query { text: "sec:1407 bench".into(), ..Default::default() };
    assert_eq!(names(&i, &i.search(&q)), ["XB_BENCH_01"]);
    // Half typed: no restriction yet.
    assert_eq!(i.search(&Query { text: "section:".into(), ..Default::default() }).len(), 7);
}

#[test]
fn a_model_says_where_it_is_placed_and_what_it_draws() {
    let i = index();
    let cone = i.model_of_solid(0).unwrap();
    assert_eq!(i.models[cone].placed(), 2);
    assert_eq!(i.models[cone].sections, [(1407, 1), (1408, 1)]);
    assert_eq!(i.models[cone].textures, [(10, 1), (11, 2)]);
    assert_eq!(i.models[cone].triangles, 2);
    assert_eq!(i.model_used(cone), "2 placed in 2 sections");
    let bench = i.model_of_solid(1).unwrap();
    assert_eq!(i.model_used(bench), "1 placed in 1 section (1 hidden)");
    let sign = i.model_of_solid(2).unwrap();
    assert_eq!(i.models[sign].as_lod, 1);
    assert_eq!(i.model_used(sign), "Only as a lower level of detail, by 1 placements");
}

#[test]
fn a_texture_says_which_models_and_materials_draw_it() {
    let i = index();
    let paint = i.texture_of_hash(11).unwrap();
    let users: Vec<(&str, usize)> =
        i.textures[paint].used_by.iter().map(|&(m, n)| (i.models[m].name.as_str(), n)).collect();
    assert_eq!(users, [("XB_BENCH_01", 1), ("XC_CONE_01", 2)]);
    assert_eq!(i.texture_used(paint), "3 materials in 2 models");
    assert_eq!(i.texture_sections(paint), [(1407, 2), (1408, 1)]);
    let left = i.texture_of_hash(13).unwrap();
    assert_eq!(i.texture_used(left), "No model of this region draws it");
    assert!(i.textures[paint].id().to_string().starts_with("world:0x"));
}

#[test]
fn an_empty_region_finds_nothing() {
    let i = Index::default();
    assert!(i.search(&Query::default()).is_empty());
    assert_eq!(i.counts("x"), (0, 0));
}
