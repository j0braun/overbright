//! The Browser page: its focus lines, chips, jumps and layout, on a small synthetic region (no game files).

use ob_formats::formats::solid::{Material, Solid};
use ob_formats::formats::texture::{Blend, PixelFormat, Scroll, Texture};
use ob_studio::browser::{Hit, Kind, TexFilter};

use super::Studio;
use super::browser::{Act, FIRST_RESULT, Page, Source};
use super::shot::{frames, raw_input};
use super::workspace::Workspace;
use crate::world::{Placement, World};

fn region() -> World {
    let mut w = World::flat(100.0);
    let names = ["XC_CONE_01", "XB_BENCH_01", "XS_SIGN_BIG"];
    for (k, n) in names.iter().enumerate() {
        w.solids.push(Solid {
            name: (*n).into(),
            hash: k as u32 + 1,
            bbox_min: [-0.5; 3],
            bbox_max: [0.5; 3],
            textures: vec![10 + k as u32],
            vertices: vec![],
            normals: vec![],
            light_materials: vec![],
            indices: vec![0; 3],
            materials: vec![Material {
                texture_slot: 0,
                light_material: 0xFF,
                first_index: 0,
                index_count: 3,
                flags: 0,
            }],
            mesh_flags: 0,
            markers: vec![],
            light_markers: vec![],
        });
        w.textures.insert(
            10 + k as u32,
            Texture {
                name: format!("TEX_{k}"),
                hash: 10 + k as u32,
                width: 4,
                height: 4,
                format: PixelFormat::Rgba8,
                mip_count: 1,
                data: vec![200u8; 64].into(),
                palette: vec![],
                alpha_test: false,
                blend: Blend::None,
                rendering_order: 0,
                late_buckets: false,
                scroll: Scroll::default(),
            },
        );
    }
    for (solid, section) in [(0, 5), (0, 6), (1, 5)] {
        w.placements.push(Placement {
            solid,
            low_solid: None,
            mid_solid: None,
            transform: glam::Mat4::IDENTITY,
            section,
            flags: 0,
            cull_radius: 1.0,
        });
    }
    w
}

fn page(w: &World) -> Page {
    let mut p = Page::new();
    p.ensure(&Source { region: "L4RA", world: w, mod_props: 0, thumbs: None });
    p
}

#[test]
fn the_focus_runs_over_the_chips_then_the_results() {
    let w = region();
    let mut p = page(&w);
    assert_eq!(p.hits().len(), 6);
    assert_eq!(p.focus, FIRST_RESULT);
    assert!(p.step_focus(-1) && p.step_focus(-1), "up to the kinds");
    assert!(!p.step_focus(-1));
    // ← → on the kinds line change the kind; Enter does too.
    p.step_chip(1);
    assert_eq!(p.query.kind, Kind::Models);
    assert!(p.activate().is_none());
    assert_eq!(p.query.kind, Kind::Textures);
    // On the filters line they change that kind's filter.
    p.step_focus(1);
    p.step_chip(1);
    assert_eq!(p.query.textures, TexFilter::InUse);
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None });
    assert_eq!(p.hits().len(), 2, "TEX_2 is drawn only by an unplaced model");
}

#[test]
fn a_result_takes_to_its_place() {
    let w = region();
    let mut p = page(&w);
    p.query.text = "bench".into();
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None });
    p.focus = FIRST_RESULT;
    assert_eq!(p.activate(), Some(Act::ShowModel { solid: 1, name: "XB_BENCH_01".into(), placement: Some(2) }));
    p.query.text = "tex_1".into();
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None });
    p.focus = FIRST_RESULT;
    assert_eq!(p.activate(), Some(Act::OpenTexture { hash: 11, name: "TEX_1".into() }));
    // A model nothing places says so: no placement to show.
    p.query.text = "sign".into();
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None });
    p.focus = FIRST_RESULT;
    assert!(matches!(p.activate(), Some(Act::ShowModel { placement: None, .. })));
}

#[test]
fn a_used_by_link_shows_that_result() {
    let w = region();
    let mut p = page(&w);
    let tex = p.index().unwrap().texture_of_hash(10).unwrap();
    p.select(Hit::Texture(tex));
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None });
    assert_eq!(p.focused_name().as_deref(), Some("TEX_0"));
    assert_eq!(p.query.text, "TEX_0");
}

#[test]
fn the_index_follows_the_world() {
    let mut w = region();
    let mut p = page(&w);
    assert_eq!(p.index().unwrap().models[0].placed() + p.index().unwrap().models[1].placed(), 3);
    w.placements.pop();
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None });
    let placed: usize = p.index().unwrap().models.iter().map(|m| m.placed()).sum();
    assert_eq!(placed, 2, "made again when the placements change");
}

#[test]
fn ctrl_4_opens_it_and_it_draws_without_a_mod() {
    let dir = std::env::temp_dir().join(format!("ob-studio-browser-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("m")).unwrap();
    std::fs::write(dir.join("m").join(crate::tools::map::store::FILE), "overbright-map 1\nname = M\n").unwrap();
    let mut s = Studio::new(dir.clone(), Some("m"));
    frames(&mut s, (1280, 800), 1.0, 1);
    let key = egui::Event::Key {
        key: egui::Key::Num4,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers { ctrl: true, command: true, ..Default::default() },
    };
    s.frame(raw_input((1280, 800), 1.0, 0, vec![key]));
    assert_eq!(s.nav.workspace, Workspace::Browser);
    assert_eq!(s.address().to_string(), "ob://m/@browser");
    let (_, l) = frames(&mut s, (960, 600), 1.0, 2);
    assert!(l.problems().is_empty(), "{:?}", l.problems());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn it_draws_inside_its_slab_at_each_size() {
    let w = region();
    for (width, height) in [(960.0, 600.0), (1280.0, 800.0), (1920.0, 1080.0)] {
        let ctx = egui::Context::default();
        crate::editor::theme::apply(&ctx);
        let mut p = Page::new();
        let inner = egui::Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(width - 100.0, height - 150.0));
        for _ in 0..2 {
            let mut out = ctx.run_ui(raw_input((width as u32, height as u32), 1.0, 1, vec![]), |ui| {
                p.draw(ui, inner, true, true, Some(Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None }));
            });
            out.textures_delta.clear();
        }
        let d = &p.last;
        let inside = inner.expand(1.0);
        for r in d.rows.iter().chain(&d.chips).chain(d.viewer.iter()).chain(&d.buttons).chain(d.search.iter()) {
            assert!(inside.contains_rect(*r), "{width}x{height}: {r:?} outside {inner:?}");
        }
        assert!(!d.rows.is_empty() && d.viewer.is_some());
        // The viewer and the list do not overlap.
        let v = d.viewer.unwrap();
        assert!(d.rows.iter().all(|r| !r.intersects(v)));
    }
}

#[test]
fn a_model_exports_and_a_texture_does_not() {
    let w = region();
    let mut p = page(&w);
    p.query.text = "bench".into();
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None });
    p.focus = FIRST_RESULT;
    assert_eq!(p.export_act(), Some(Act::ExportModel { solid: 1, name: "XB_BENCH_01".into() }));
    p.query.text = "tex_1".into();
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None });
    assert_eq!(p.export_act(), None, "a texture has no glTF");
    // On a chip line nothing is focused to export.
    p.focus = 0;
    assert_eq!(p.export_act(), None);
}
