//! The collision layer: the menu's words, the drawing (a reference image of
//! a small made-up region on the Map and in the 3D view, drawn through
//! egui's tessellator and filled on the CPU, so the picture is the same on
//! every machine) and, with the game files, the original's invisible walls.
//!
//! `OB_BLESS=1` writes the reference image again (look at it before
//! committing: tests/reference/collision-layer.png).

use egui::{Color32, Pos2, Rect};
use glam::{Vec2, Vec3};

use super::*;
use crate::draw::camera::Camera;
use crate::formats::collision::{BarrierGroup, CollisionFace, FLAG_QUAD, FLAG_TALL, FLAG_WALL};

#[test]
fn the_toggle_has_three_states_named_as_in_the_docs() {
    assert_eq!(Show::ALL.map(Show::label), ["Off", "Invisible barriers", "All collision"]);
    assert_eq!(Show::parse("invisible"), Some(Show::Invisible));
    assert_eq!(Show::parse("all"), Some(Show::All));
    assert_eq!(Show::parse("walls"), None);
    assert_eq!(Show::default(), Show::Off);
}

#[test]
fn the_command_line_picks_the_layer_and_the_camera() {
    let c =
        crate::editor::app::Cli::parse(["--collision", "all", "--camera", "1,2,3,90,-30"].map(String::from)).unwrap();
    assert_eq!(c.collision, Some(Show::All));
    assert_eq!(c.camera, Some([1.0, 2.0, 3.0, 90.0, -30.0]));
    assert!(crate::editor::app::Cli::parse(["--collision", "x"].map(String::from)).is_err());
    assert!(crate::editor::app::Cli::parse(["--camera", "1,2"].map(String::from)).is_err());
}

fn face(flags: u8, group: u8, v: [Vec3; 4]) -> CollisionFace {
    let q = |f: f32| (f * 8.0) as i16;
    CollisionFace {
        vertices: v,
        corners: 4,
        surface: 1,
        flags,
        group,
        bounds: 0,
        x: v.map(|p| q(p.x)),
        y: v.map(|p| q(p.y)),
    }
}

/// A wall along `a` → `b`, 0..`h` (tall: as the game replaces its corners).
fn wall(flags: u8, group: u8, a: Vec2, b: Vec2, h: f32) -> CollisionFace {
    let flags = flags | FLAG_WALL | FLAG_QUAD;
    let (lo, hi) = if flags & FLAG_TALL != 0 { (-2048.0, 2048.0) } else { (0.0, h) };
    face(flags, group, [a.extend(lo), a.extend(hi), b.extend(hi), b.extend(lo)])
}

/// The made-up region: a 60 m square street block on a floor, its north
/// and east sides behind a building's skin, the south side bare (an
/// invisible wall), a tall invisible wall across the road west of it, and a
/// career barrier across the road north.
fn region() -> Layer {
    let p = |x, y| Vec2::new(x, y);
    let faces = vec![
        wall(0, 0, p(0.0, 0.0), p(60.0, 0.0), 6.0),
        wall(0, 0, p(60.0, 0.0), p(60.0, 60.0), 6.0),
        wall(0, 0, p(60.0, 60.0), p(0.0, 60.0), 6.0),
        wall(FLAG_TALL, 0, p(-30.0, -20.0), p(-30.0, 80.0), 0.0),
        wall(0, 1, p(-20.0, 90.0), p(80.0, 90.0), 8.0),
        face(
            FLAG_QUAD,
            0,
            [
                Vec3::new(-60.0, -40.0, 0.0),
                Vec3::new(120.0, -40.0, 0.0),
                Vec3::new(120.0, 120.0, 0.0),
                Vec3::new(-60.0, 120.0, 0.0),
            ],
        ),
    ];
    // The building's skin 0.5 m inside the north and east walls.
    let skin = |a: Vec3, b: Vec3| {
        let (c, d) = (b + Vec3::Z * 8.0, a + Vec3::Z * 8.0);
        [[a, b, c], [a, c, d]]
    };
    let mut scenery = Vec::new();
    scenery.extend(skin(Vec3::new(59.5, 0.0, 0.0), Vec3::new(59.5, 60.0, 0.0)));
    scenery.extend(skin(Vec3::new(60.0, 59.5, 0.0), Vec3::new(0.0, 59.5, 0.0)));
    let groups = ["NONE", "BARRIERS_CAREER1"].map(|n| BarrierGroup { hash: 0, name: n.to_string() });
    collision::build(&faces, &groups, &scenery)
}

#[test]
fn the_made_up_region_sorts_its_walls() {
    let l = region();
    assert_eq!(l.counts(), collision::Counts { invisible: 2, barriers: 1, visible: 2, floors: 1 });
}

const W: u32 = 640;
const H: u32 = 360;

/// The layer drawn: the Map on the left half (all collision), the 3D view
/// on the right (invisible barriers), each with its legend.
fn draw() -> image::RgbaImage {
    let ctx = egui::Context::default();
    crate::editor::theme::apply(&ctx);
    let layer = region();
    let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(W as f32, H as f32));
    let raw = || egui::RawInput { screen_rect: Some(screen), ..Default::default() };
    // Once for the fonts, then the frame drawn; the font atlas comes in
    // pieces over both.
    let mut deltas = Vec::new();
    let mut shapes = Vec::new();
    for _ in 0..2 {
        let frame = ctx.run_ui(raw(), |ui| {
            let left = Rect::from_min_max(Pos2::ZERO, Pos2::new(W as f32 / 2.0, H as f32));
            let right = Rect::from_min_max(Pos2::new(W as f32 / 2.0, 0.0), Pos2::new(W as f32, H as f32));
            let painter = ui.painter_at(left);
            painter.rect_filled(left, 0.0, colour::PAGE);
            paint_map(&layer, Show::All, &painter, Vec2::new(30.0, 40.0), 1.2, left);
            legend(&layer, Show::All, false, &painter, left);
            let painter = ui.painter_at(right);
            painter.rect_filled(right, 0.0, Color32::from_rgb(40, 44, 48));
            // From the south-west, 25 m up, looking north-east and down.
            let cam = Camera::looking_at(Vec3::new(-70.0, -60.0, 25.0), Vec3::new(30.0, 40.0, 0.0));
            paint_3d(&layer, Show::Invisible, &Overlay::new(&painter, &cam, right));
            legend(&layer, Show::Invisible, true, &painter, right);
        });
        deltas.push(frame.textures_delta);
        shapes = frame.shapes;
    }
    let image = rasterize(&ctx, &deltas, shapes);
    // Applied by the rasterizer above.
    deltas.iter_mut().for_each(egui::TexturesDelta::clear);
    image
}

/// egui's output filled on the CPU: each mesh's triangles, vertex colours
/// times the font texture, premultiplied "over" blending, clipped.
fn rasterize(
    ctx: &egui::Context,
    deltas: &[egui::TexturesDelta],
    shapes: Vec<egui::epaint::ClippedShape>,
) -> image::RgbaImage {
    let mut tex: std::collections::HashMap<egui::TextureId, (usize, usize, Vec<Color32>)> = Default::default();
    for (id, delta) in deltas.iter().flat_map(|d| d.set.iter()).flat_map(|(id, ds)| ds.iter().map(move |d| (id, d))) {
        let egui::ImageData::Color(img) = &delta.image;
        let (w, h) = (img.size[0], img.size[1]);
        let e = tex.entry(*id).or_insert_with(|| (w, h, vec![Color32::TRANSPARENT; w * h]));
        let [x0, y0] = delta.pos.unwrap_or([0, 0]);
        if delta.pos.is_none() {
            *e = (w, h, vec![Color32::TRANSPARENT; w * h]);
        }
        for y in 0..h {
            for x in 0..w {
                e.2[(y0 + y) * e.0 + x0 + x] = img.pixels[y * w + x];
            }
        }
    }
    let mut px = vec![[0f32; 4]; (W * H) as usize];
    for prim in ctx.tessellate(shapes, 1.0) {
        let egui::epaint::Primitive::Mesh(m) = prim.primitive else { continue };
        let t = tex.get(&m.texture_id);
        let clip = prim.clip_rect.intersect(Rect::from_min_size(Pos2::ZERO, egui::vec2(W as f32, H as f32)));
        for tri in m.indices.chunks_exact(3) {
            let v = [m.vertices[tri[0] as usize], m.vertices[tri[1] as usize], m.vertices[tri[2] as usize]];
            let (p0, p1, p2) = (v[0].pos, v[1].pos, v[2].pos);
            let area = (p1.x - p0.x) * (p2.y - p0.y) - (p2.x - p0.x) * (p1.y - p0.y);
            if area.abs() < 1e-9 {
                continue;
            }
            let lo = p0.min(p1).min(p2).max(clip.min);
            let hi = p0.max(p1).max(p2).min(clip.max);
            for y in lo.y.floor() as i32..hi.y.ceil() as i32 {
                for x in lo.x.floor() as i32..hi.x.ceil() as i32 {
                    let p = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
                    if !clip.contains(p) {
                        continue;
                    }
                    let w0 = ((p1.x - p.x) * (p2.y - p.y) - (p2.x - p.x) * (p1.y - p.y)) / area;
                    let w1 = ((p2.x - p.x) * (p0.y - p.y) - (p0.x - p.x) * (p2.y - p.y)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let mix = |f: fn(&egui::epaint::Vertex) -> f32| w0 * f(&v[0]) + w1 * f(&v[1]) + w2 * f(&v[2]);
                    let mut c = [
                        mix(|v| v.color.r() as f32),
                        mix(|v| v.color.g() as f32),
                        mix(|v| v.color.b() as f32),
                        mix(|v| v.color.a() as f32),
                    ];
                    if let Some((tw, th, texels)) = t {
                        let (u, vv) = (mix(|v| v.uv.x), mix(|v| v.uv.y));
                        let tx = ((u * *tw as f32) as usize).min(tw - 1);
                        let ty = ((vv * *th as f32) as usize).min(th - 1);
                        let s = texels[ty * tw + tx];
                        let k = [s.r(), s.g(), s.b(), s.a()];
                        for i in 0..4 {
                            c[i] *= k[i] as f32 / 255.0;
                        }
                    }
                    let d = &mut px[(y as u32 * W + x as u32) as usize];
                    let a = c[3] / 255.0;
                    for i in 0..4 {
                        d[i] = c[i] + d[i] * (1.0 - a);
                    }
                }
            }
        }
    }
    image::RgbaImage::from_fn(W, H, |x, y| {
        let c = px[(y * W + x) as usize];
        image::Rgba([c[0].round().min(255.0) as u8, c[1].round().min(255.0) as u8, c[2].round().min(255.0) as u8, 255])
    })
}

#[test]
fn the_layer_draws_as_the_reference_image() {
    let got = draw();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference/collision-layer.png");
    if std::env::var_os("OB_BLESS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        got.save(&path).unwrap();
        return;
    }
    let want =
        image::open(&path).unwrap_or_else(|e| panic!("{}: {e} (OB_BLESS=1 writes it)", path.display())).to_rgba8();
    assert_eq!(want.dimensions(), got.dimensions());
    // Glyph edges may round differently: at most 0.2 % of the pixels off by
    // more than 8 levels.
    let off = want.pixels().zip(got.pixels()).filter(|(a, b)| (0..3).any(|i| a.0[i].abs_diff(b.0[i]) > 8)).count();
    if off * 500 > (W * H) as usize {
        let fail = path.with_file_name("collision-layer.got.png");
        let _ = got.save(&fail);
        panic!("{off} pixels differ from {} (this run's picture: {})", path.display(), fail.display());
    }
    // The picture holds each kind's colour.
    for c in [key::INVISIBLE, key::BARRIER, key::VISIBLE] {
        assert!(got.pixels().any(|p| p.0[..3] == [c.r(), c.g(), c.b()]), "{c:?} drawn");
    }
}

/// With the game's files: the airport's invisible wall a player meets at the
/// intro's start (docs/formats/career-3.md "The airport's invisible wall":
/// pack 240, from (1716.6, −892.3) to (1724.1, −850.1), z −2048..2048, the
/// original stops there) shows as invisible.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_airports_invisible_wall_shows() {
    let root = crate::testkit::game_root();
    let world = crate::world::World::load(&root, "L4RA").unwrap();
    let camera = Camera::new(Vec3::ZERO, 0.0, 0.0);
    let dir = std::env::temp_dir();
    let s = EditorState::new(root, "L4RA", dir, "fixture", Default::default(), world, camera);
    let layer = layer_of(&s);
    let c = layer.counts();
    eprintln!("L4RA: {c:?}, {} faces read", layer.read);
    let (a, b) = (Vec2::new(1716.6, -892.3), Vec2::new(1724.1, -850.1));
    let at =
        |w: &Wall| (w.a.distance(a) < 0.5 && w.b.distance(b) < 0.5) || (w.a.distance(b) < 0.5 && w.b.distance(a) < 0.5);
    let wall = layer.walls.iter().find(|w| at(w)).expect("the airport wall in the layer");
    assert!(wall.tall && wall.group == 0, "{wall:?}");
    assert_eq!(wall.kind(), collision::WallKind::Invisible, "{wall:?}");
    let bare: f32 = wall.bare.iter().map(|(t0, t1)| t1 - t0).sum();
    assert!(bare > 0.5, "mostly bare: {wall:?}");
    // Every barrier group's wall is a barrier, and most of the region's
    // walls stand behind scenery.
    assert!(c.barriers > 10_000 && c.visible > c.invisible, "{c:?}");
}

/// The collision in play, face by face (pack order).
fn listed_of(s: &EditorState) -> Vec<String> {
    s.session.world.ground.listed_faces().map(|f| format!("{f:?}")).collect()
}

/// One frame of the layer's overlay in a 1600 x 960 view with `events`.
fn frame(o: &mut CollisionOverlay, s: &mut EditorState, ctx: &egui::Context, events: Vec<egui::Event>) -> bool {
    use crate::editor::app::Overlay as _;
    let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(1600.0, 960.0));
    let input = egui::RawInput { screen_rect: Some(rect), events, ..Default::default() };
    let mut taken = false;
    let _ = crate::editor::run_headless(ctx, input, |ui| taken = o.overlay(s, ui.ctx(), ui.painter(), rect));
    taken
}

/// A left click at `at` (pointer moved there first, pressed, released).
fn click(o: &mut CollisionOverlay, s: &mut EditorState, ctx: &egui::Context, at: Pos2) -> bool {
    let b = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    frame(o, s, ctx, vec![egui::Event::PointerMoved(at)]);
    let took = frame(o, s, ctx, vec![b(true)]);
    frame(o, s, ctx, vec![b(false)]);
    took
}

/// With the game's files, in the 3D view: Edit ▸ Draw wall and two clicks
/// on the plaza make one undo step whose wall the layer shows (built again)
/// and the world collides with; a click selects it and Del removes it;
/// undo and redo give the same collision. A click on the airport's
/// invisible wall selects that original face, Remove wall takes it out of
/// the layer and the game's collision, Undo brings it back.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn walls_are_drawn_picked_and_removed_with_the_mouse() {
    let root = crate::testkit::game_root();
    let world = crate::world::World::load(&root, "L4RA").unwrap();
    let spot = Vec3::new(-327.0, -535.0, 21.269472);
    let spot = spot.truncate().extend(world.collision().ground_at(spot).unwrap().0);
    let camera = Camera::looking_at(spot + Vec3::new(0.0, 12.0, 5.0), spot);
    let dir = std::env::temp_dir();
    let mut s = EditorState::new(root, "L4RA", dir, "fixture", Default::default(), world, camera);
    s.collision = Show::Invisible;
    let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(1600.0, 960.0));
    let ctx = egui::Context::default();
    let mut o = CollisionOverlay::default();
    frame(&mut o, &mut s, &ctx, vec![]);
    let before = o.layer(&s).counts();
    let original = listed_of(&s);

    s.draw = Some(Draw::Wall { first: None });
    let cam = s.camera;
    let on = |p: Vec3| super::super::camera::project(&cam, rect, p).unwrap();
    let (a, b) = (on(spot - Vec3::X * 6.0), on(spot + Vec3::X * 6.0));
    assert!(click(&mut o, &mut s, &ctx, a), "the first end");
    assert!(click(&mut o, &mut s, &ctx, b), "the second end");
    assert_eq!(s.session.undo.undo_label(), Some("Draw wall"));
    let [CollisionEdit::Wall { from, to, height, .. }] = s.session.map.collision.as_slice() else {
        panic!("{:?}", s.session.map.collision)
    };
    assert!(
        from.truncate().distance((spot - Vec3::X * 6.0).truncate()) < 1.5
            && to.truncate().distance((spot + Vec3::X * 6.0).truncate()) < 1.5,
        "{from} {to}"
    );
    assert_eq!(*height, 2.0);
    let drawn = o.layer(&s).counts();
    // The box's four sides (invisible where no scenery stands at them).
    assert_eq!(drawn.invisible + drawn.visible, before.invisible + before.visible + 4, "{before:?} → {drawn:?}");
    assert!(drawn.invisible > before.invisible);
    let with_wall = listed_of(&s);

    // Select it and remove it (Del), then undo and redo.
    s.draw = None;
    assert!(click(&mut o, &mut s, &ctx, on(spot + Vec3::Z)), "a click on the drawn wall");
    let p = s.wall.clone().expect("the drawn wall selected");
    assert_eq!(p.key, Target::Drawn("wall-1".into()));
    crate::editor::props::delete_selection(&mut s);
    assert!(s.session.map.collision.is_empty());
    assert_eq!(s.session.undo.undo_label(), Some("Remove wall-1"));
    assert_eq!(o.layer(&s).counts(), before);
    assert!(listed_of(&s) == original, "the original's collision");
    s.session.undo();
    assert!(listed_of(&s) == with_wall, "undo: the wall again");
    s.session.undo();
    assert!(listed_of(&s) == original, "undo: no wall");
    s.session.redo();
    assert!(listed_of(&s) == with_wall, "redo: the wall");
    s.session.undo();

    // The airport's invisible wall.
    let (wa, wb) = (Vec2::new(1716.6, -892.3), Vec2::new(1724.1, -850.1));
    let mid = wa.lerp(wb, 0.5);
    let floor = s.session.world.collision().ground_at(mid.extend(500.0)).map_or(0.0, |(z, _)| z);
    let normal = (wb - wa).perp().normalize();
    s.camera = Camera::looking_at((mid + normal * 25.0).extend(floor + 3.0), mid.extend(floor + 1.5));
    let at = super::super::camera::project(&s.camera, rect, mid.extend(floor + 1.5)).unwrap();
    assert!(click(&mut o, &mut s, &ctx, at), "a click on the airport wall");
    let p = s.wall.clone().expect("a wall selected");
    let Target::Original(corners) = &p.key else { panic!("{p:?}") };
    assert_eq!(p.what, "Invisible wall");
    let wall_at = |l: &Layer| l.walls.iter().any(|w| w.corners == *corners);
    assert!(wall_at(o.layer(&s)));
    crate::editor::props::delete_selection(&mut s);
    assert_eq!(s.session.undo.undo_label(), Some("Remove wall"));
    assert!(!wall_at(o.layer(&s)), "removed from the layer");
    let key = nfsu2::studio_api::world::face_key(corners);
    let listed = |s: &EditorState| {
        s.session
            .world
            .ground
            .listed_faces()
            .any(|f| nfsu2::studio_api::world::face_key(&f.vertices[..f.corners.clamp(3, 4)]) == key)
    };
    assert!(!listed(&s), "removed from the game's collision");
    s.session.undo();
    assert!(listed(&s) && wall_at(o.layer(&s)), "undo brings it back");
    assert!(listed_of(&s) == original);
}

/// Walls drawn and picked on the Map and in Top (the overview), with the
/// game's files: x/y from the mouse and the floor's height, the same
/// collision as the wall drawn in the 3D view, a click picks it, Del and
/// undo/redo are exact, and a point drawn within a few points of the first
/// wall's end snaps to it (Alt: not).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn walls_are_drawn_and_picked_on_the_map_and_in_top() {
    use crate::editor::state::View;
    let root = crate::testkit::game_root();
    let spot = Vec3::new(-327.0, -535.0, 21.269472);
    let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(1600.0, 960.0));
    let (a, b) = (Vec3::new(-333.0, -535.0, 0.0), Vec3::new(-321.0, -535.0, 0.0));
    let new_state = || {
        let world = crate::world::World::load(&root, "L4RA").unwrap();
        let z = world.collision().ground_at(spot).unwrap().0;
        let camera =
            Camera::looking_at(spot.truncate().extend(z) + Vec3::new(0.0, 12.0, 5.0), spot.truncate().extend(z));
        let mut s =
            EditorState::new(root.clone(), "L4RA", std::env::temp_dir(), "fixture", Default::default(), world, camera);
        s.collision = Show::Invisible;
        (s, z)
    };
    let (r, _) = new_state();
    let original = listed_of(&r);

    for view in ["map", "top"] {
        let (mut s, z) = new_state();
        let ctx = egui::Context::default();
        // 4 points per metre: 10 points are 2.5 m.
        let top = super::super::camera::TopView { centre: spot.truncate(), half_height: 960.0 / 2.0 / 4.0 };
        let publish = |ctx: &egui::Context| {
            if view == "map" {
                crate::editor::sim::panel::publish_map(ctx, spot.truncate(), 4.0, rect);
            } else {
                super::super::viewport::publish_top(ctx, Some((top, rect)));
            }
        };
        s.view = if view == "map" { View::Map } else { View::ThreeD };
        let at = |p: Vec3| Pos2::new(800.0 + (p.x - spot.x) * 4.0, 480.0 - (p.y - spot.y) * 4.0);
        let mut o = CollisionOverlay::default();
        let click_at = |o: &mut CollisionOverlay, s: &mut EditorState, p: Pos2, alt: bool| {
            let m = if alt { egui::Modifiers::ALT } else { egui::Modifiers::NONE };
            let b = |pressed| egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: m,
            };
            publish(&ctx);
            frame(o, s, &ctx, vec![egui::Event::ModifiersChanged(m), egui::Event::PointerMoved(p)]);
            let took = frame(o, s, &ctx, vec![b(true)]);
            frame(o, s, &ctx, vec![b(false)]);
            frame(o, s, &ctx, vec![egui::Event::ModifiersChanged(Default::default())]);
            took
        };
        publish(&ctx);
        frame(&mut o, &mut s, &ctx, vec![]);
        s.draw = Some(Draw::Wall { first: None });
        assert!(click_at(&mut o, &mut s, at(a), false), "{view}: first end");
        assert!(click_at(&mut o, &mut s, at(b), false), "{view}: second end");
        let [CollisionEdit::Wall { from, to, .. }] = s.session.map.collision.as_slice() else {
            panic!("{view}: {:?}", s.session.map.collision)
        };
        assert!(
            from.truncate().distance(a.truncate()) < 0.5 && to.truncate().distance(b.truncate()) < 0.5,
            "{view}: {from} {to}"
        );
        assert!((from.z - z).abs() < 0.2, "{view}: height {} vs the floor's {z}", from.z);
        let (from, to) = (*from, *to);
        let with_wall = listed_of(&s);
        // The same wall through the same op: the same collision, face for face.
        let (mut r2, _) = new_state();
        let surface = surface_under(&r2, from);
        super::super::undo::run(&mut r2, Op::DrawWall { from, to, height: 2.0, surface, id: None }).unwrap();
        assert_eq!(listed_of(&r2), with_wall, "{view}: as drawn in 3D");

        // Snap: a start 3 points from the wall's end takes its place; Alt keeps the mouse's.
        let near = to + Vec3::new(0.0, 0.75, 0.0);
        click_at(&mut o, &mut s, at(near), false);
        click_at(&mut o, &mut s, at(near + Vec3::new(0.0, 10.0, 0.0)), false);
        let walls: Vec<(Vec3, Vec3)> = s
            .session
            .map
            .collision
            .iter()
            .filter_map(|e| match e {
                CollisionEdit::Wall { from, to, .. } => Some((*from, *to)),
                _ => None,
            })
            .collect();
        assert_eq!(walls.len(), 2, "{view}");
        assert!(walls[1].0.truncate().distance(to.truncate()) < 1e-3, "{view}: snapped {}", walls[1].0);
        click_at(&mut o, &mut s, at(near), true);
        click_at(&mut o, &mut s, at(near + Vec3::new(10.0, 10.0, 0.0)), true);
        let c = s.session.map.collision.last().cloned();
        let Some(CollisionEdit::Wall { from: f3, .. }) = c else { panic!("{view}: {c:?}") };
        assert!(f3.truncate().distance(near.truncate()) < 0.3, "{view}: Alt: {f3}");

        // Pick the first wall (on its foot), Del, undo, redo.
        s.draw = None;
        let ground_mid = (from + to) * 0.5;
        assert!(click_at(&mut o, &mut s, at(ground_mid), false), "{view}: a click on the wall");
        assert_eq!(s.wall.clone().map(|p| p.key), Some(Target::Drawn("wall-1".into())), "{view}");
        crate::editor::props::delete_selection(&mut s);
        assert!(
            s.session.map.collision.len() == 2
                && matches!(&s.session.map.collision[0], CollisionEdit::Wall { id, .. } if id == "wall-2"),
            "{view}: Del"
        );
        let removed = listed_of(&s);
        s.session.undo();
        assert_eq!(s.session.map.collision.len(), 3, "{view}");
        s.session.redo();
        assert_eq!(listed_of(&s), removed, "{view}: redo");
        s.session.undo();
        for _ in 0..3 {
            s.session.undo();
        }
        assert_eq!(listed_of(&s), original, "{view}: undo all");
    }
}

#[test]
fn the_key_steps_round_and_floors_are_coloured_by_kind() {
    assert_eq!(Show::Off.next(), Show::Invisible);
    assert_eq!(Show::Invisible.next(), Show::All);
    assert_eq!(Show::All.next(), Show::Off);
    let f = |not_ground| collision::Floor { corners: [Vec3::ZERO; 4], n: 3, not_ground };
    assert_eq!(floor_colour(&f(false)), key::FLOOR);
    assert_eq!(floor_colour(&f(true)), key::FLOOR_OFF);
    assert_ne!(key::FLOOR, key::FLOOR_OFF);
}
