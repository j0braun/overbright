//! The Browser page: its focus lines, chips, jumps and layout, on a small synthetic region (no game files).

use ob_formats::formats::solid::{Material, Solid};
use ob_formats::formats::texture::{Blend, PixelFormat, Scroll, Texture};
use ob_studio::browser::media::{GivenMedia, Line, Sound};
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
    p.ensure(&Source { region: "L4RA", world: w, mod_props: 0, thumbs: None, game: None });
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
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None, game: None });
    assert_eq!(p.hits().len(), 2, "TEX_2 is drawn only by an unplaced model");
}

#[test]
fn a_result_takes_to_its_place() {
    let w = region();
    let mut p = page(&w);
    p.query.text = "bench".into();
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None, game: None });
    p.focus = FIRST_RESULT;
    assert_eq!(p.activate(), Some(Act::ShowModel { solid: 1, name: "XB_BENCH_01".into(), placement: Some(2) }));
    p.query.text = "tex_1".into();
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None, game: None });
    p.focus = FIRST_RESULT;
    assert_eq!(p.activate(), Some(Act::OpenTexture { hash: 11, name: "TEX_1".into() }));
    // A model nothing places says so: no placement to show.
    p.query.text = "sign".into();
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None, game: None });
    p.focus = FIRST_RESULT;
    assert!(matches!(p.activate(), Some(Act::ShowModel { placement: None, .. })));
}

#[test]
fn a_used_by_link_shows_that_result() {
    let w = region();
    let mut p = page(&w);
    let tex = p.index().unwrap().texture_of_hash(10).unwrap();
    p.select(Hit::Texture(tex));
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None, game: None });
    assert_eq!(p.focused_name().as_deref(), Some("TEX_0"));
    assert_eq!(p.query.text, "TEX_0");
}

#[test]
fn the_index_follows_the_world() {
    let mut w = region();
    let mut p = page(&w);
    assert_eq!(p.index().unwrap().models[0].placed() + p.index().unwrap().models[1].placed(), 3);
    w.placements.pop();
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None, game: None });
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

/// The browser page drawn into its slab at every standard size with the mouse, the keys and the pad (the kit);
/// `setup` makes the page, `check` returns what else is wrong with what was drawn.
fn slab_matrix(name: &str, mut setup: impl FnMut() -> Page, mut check: impl FnMut(&Page, egui::Rect) -> Vec<String>) -> usize {
    let w = region();
    let mut drawn = 0;
    let report = ob_testkit::matrix::run(name, &ob_testkit::matrix::STANDARD, &super::matrix::INPUTS, |size, input| {
        if super::matrix::too_small(size) {
            return vec![];
        }
        drawn += 1;
        let sc = super::matrix::scale_of(size);
        let (lw, lh) = (size.w as f32 / sc, size.h as f32 / sc);
        let ctx = egui::Context::default();
        crate::editor::theme::apply(&ctx);
        let mut p = setup();
        let inner = egui::Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(lw - 100.0, lh - 150.0));
        let keys = input != ob_look::legends::InputKind::Mouse;
        for _ in 0..3 {
            let mut out = ctx.run_ui(raw_input((size.w, size.h), sc, 1, vec![]), |ui| {
                p.draw(ui, inner, keys, true, Some(Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None, game: None }));
            });
            out.textures_delta.clear();
        }
        let d = &p.last;
        let inside = inner.expand(1.0);
        let mut bad = vec![];
        for r in d.rows.iter().chain(&d.chips).chain(d.viewer.iter()).chain(&d.buttons).chain(d.search.iter()).chain(d.area_button.iter()) {
            if !inside.contains_rect(*r) {
                bad.push(format!("{r:?} outside {inner:?}"));
            }
        }
        if d.rows.is_empty() || d.viewer.is_none() {
            bad.push("no rows or no viewer drawn".into());
        }
        if let Some(v) = d.viewer
            && d.rows.iter().any(|r| r.intersects(v))
        {
            bad.push("a row overlaps the viewer".into());
        }
        bad.extend(check(&p, inner));
        bad
    });
    report.finish(name);
    drawn
}

#[test]
fn it_draws_inside_its_slab_at_each_size() {
    let w = region();
    let _ = &w;
    let drawn = slab_matrix("browser models", Page::new, |_, _| vec![]);
    assert!(drawn >= 12, "{drawn}");
}

#[test]
fn a_model_exports_and_a_texture_does_not() {
    let w = region();
    let mut p = page(&w);
    p.query.text = "bench".into();
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None, game: None });
    p.focus = FIRST_RESULT;
    assert_eq!(p.export_act(), Some(Act::ExportModel { solid: 1, name: "XB_BENCH_01".into() }));
    // Bring back is for the same model; a texture has neither.
    assert_eq!(p.import_act(), Some(Act::ImportModel { solid: 1, name: "XB_BENCH_01".into() }));
    p.query.text = "tex_1".into();
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None, game: None });
    assert_eq!(p.export_act(), None, "a texture has no glTF");
    assert_eq!(p.import_act(), None);
    // On a chip line nothing is focused to export.
    p.focus = 0;
    assert_eq!(p.export_act(), None);
}

fn media() -> GivenMedia {
    let sound = |bank: &str, name: &str, samples: &[u32], playable: bool| Sound {
        bank: bank.into(),
        name: name.into(),
        samples: samples.to_vec(),
        rate: 22050,
        frames: 11025,
        playable,
    };
    let lines = |t: &[(u32, &str)]| t.iter().map(|&(hash, text)| Line { hash, text: text.into() }).collect::<Vec<_>>();
    GivenMedia {
        sounds: vec![
            sound("SOUND/FE/FE_MB.abk", "FE_MENU_CLICK", &[3], true),
            sound("SOUND/IG_GLOBAL/FX_MAIN_MEM_MB.abk", "FX_TURBO_01", &[7, 8], true),
            sound("SOUND/IG_GLOBAL/FX_MAIN_MEM_MB.abk", "FX_BLOWOFF", &[2], false),
        ],
        pcm: vec![],
        languages: vec![
            ("English".into(), lines(&[(0x10, "Press START"), (0x20, "Race Again")])),
            ("German".into(), lines(&[(0x10, "START dr\u{fc}cken"), (0x20, "Nochmal")])),
        ],
    }
}

fn media_page(w: &World) -> Page {
    let mut p = page(w);
    p.set_media(Box::new(media()));
    p
}

#[test]
fn the_sounds_chip_reads_the_sounds_once_and_the_results_are_searched() {
    let w = region();
    let mut p = media_page(&w);
    assert!(p.index().unwrap().sounds.is_none(), "nothing is read until the chip is chosen");
    p.query.kind = Kind::Sounds;
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None, game: None });
    assert_eq!(p.hits().len(), 3);
    p.query.text = "fx main".into();
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None, game: None });
    assert_eq!(p.hits().len(), 2);
    p.query.text = "turbo".into();
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None, game: None });
    p.focus = FIRST_RESULT;
    // Enter plays its first sample; Ctrl+E writes the WAV; neither is a model's.
    assert_eq!(
        p.activate(),
        Some(Act::PlaySound { bank: "SOUND/IG_GLOBAL/FX_MAIN_MEM_MB.abk".into(), sample: 7, name: "FX_TURBO_01".into() })
    );
    assert_eq!(
        p.export_act(),
        Some(Act::ExportSound { bank: "SOUND/IG_GLOBAL/FX_MAIN_MEM_MB.abk".into(), sample: 7, name: "FX_TURBO_01".into() })
    );
    assert_eq!(p.import_act(), None);
    assert_eq!(p.focused_name().as_deref(), Some("FX_TURBO_01"));
    // The world's index changing keeps what was read.
    let mut w2 = region();
    w2.placements.pop();
    p.ensure(&Source { region: "L4RA", world: &w2, mod_props: 0, thumbs: None, game: None });
    assert!(p.index().unwrap().sounds.is_some());
    assert_eq!(p.hits().len(), 1);
}

#[test]
fn the_strings_chip_shows_a_language_and_its_chips_change_it() {
    let w = region();
    let mut p = media_page(&w);
    p.query.kind = Kind::Strings;
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None, game: None });
    assert_eq!(p.index().unwrap().language, "English");
    assert_eq!(p.hits().len(), 2);
    p.query.text = "race".into();
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None, game: None });
    p.focus = FIRST_RESULT;
    assert_eq!(p.focused_name().as_deref(), Some("0x00000020"), "a line is named by its hash");
    // A line is read, not taken anywhere or exported.
    assert_eq!(p.activate(), None);
    assert_eq!(p.export_act(), None);
    // ← → on the filters line pick the language; the lines follow.
    p.query.text.clear();
    p.focus = 1;
    assert!(p.step_chip(1));
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None, game: None });
    assert_eq!(p.index().unwrap().language, "German");
    p.query.text = "nochmal".into();
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None, game: None });
    assert_eq!(p.hits().len(), 1);
}

#[test]
fn without_a_game_folder_the_kinds_say_so() {
    let w = region();
    let mut p = page(&w);
    p.query.kind = Kind::Sounds;
    p.ensure(&Source { region: "L4RA", world: &w, mod_props: 0, thumbs: None, game: None });
    assert!(p.hits().is_empty());
    assert!(p.index().unwrap().sounds.is_none());
    // The pad's X on a kind line exports the area; on a result it exports that result.
    p.focus = 0;
    assert_eq!(p.export_or_area_act(), Some(Act::ExportArea));
}

#[test]
fn the_media_kinds_draw_inside_their_slab_at_each_size() {
    for kind in [Kind::Sounds, Kind::Strings] {
        let w = region();
        let drawn = slab_matrix(
            &format!("browser {kind:?}"),
            || {
                let mut p = media_page(&w);
                p.query.kind = kind;
                p
            },
            |p, _| {
                let d = &p.last;
                let mut bad = vec![];
                if d.chips.len() != 5 + 2 * usize::from(kind == Kind::Strings) {
                    bad.push("the kinds, and the languages of Strings".to_string());
                }
                match (d.area_button, d.search) {
                    (Some(a), Some(s)) => {
                        if a.intersects(s) || d.chips.iter().any(|c| c.intersects(a)) {
                            bad.push("the area button meets the search field or a chip".into());
                        }
                    }
                    _ => bad.push("no area button or search field".into()),
                }
                bad
            },
        );
        assert!(drawn >= 12, "{drawn}");
    }
}

#[test]
fn a_models_viewer_has_the_mesh_numbers_the_command_prints() {
    let w = region();
    let m = ob_studio::browser::mesh::facts(&w.solids[0], &w.textures);
    assert_eq!(m.lines()[0], "0 vertices · 1 triangles · unlit");
}
