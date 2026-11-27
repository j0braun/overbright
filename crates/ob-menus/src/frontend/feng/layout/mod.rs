//! [`layout`]: a package's objects at a moment of their scripts as canvas quads.

pub mod transform;
pub mod tree;

use glam::{Mat4, Quat, Vec2};

use crate::frontend::assets::Assets;
use crate::frontend::enh::widescreen::{element_place, side_strip};
use crate::frontend::feng::package_run::run_package;
use crate::frontend::feng::script::FE_RATE;
use crate::frontend::feng::state::{CANVAS, Quad, ScreenState};
use crate::frontend::feng::text::{line_count, line_count_wrapped, push_box, string_quads, wrap_feng};
use crate::frontend::feng::{Kind, Package, ResourceKind};
use crate::frontend::mouse;
use crate::render::display::Place;
use transform::{Attrs, Placed, hidden, key_matrix, local_matrix};
use tree::tree_index;

#[cfg(any(test, feature = "dev-harness"))]
thread_local! {
    static DRAWN_TEXT: std::cell::RefCell<Option<Vec<(u32, String)>>> = const { std::cell::RefCell::new(None) };
}

/// The strings `f`'s layouts drew (with at least one glyph): per string
/// object, its hash and text. For checks that a screen's texts show.
#[cfg(any(test, feature = "dev-harness"))]
pub fn drawn_texts(f: impl FnOnce()) -> Vec<(u32, String)> {
    DRAWN_TEXT.with(|d| *d.borrow_mut() = Some(Vec::new()));
    f();
    DRAWN_TEXT.with(|d| d.borrow_mut().take()).unwrap_or_default()
}

/// FENG_DEBUG: print every queued object.
#[cfg(any(test, feature = "dev-harness"))]
fn feng_debug() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("FENG_DEBUG").is_some())
}

/// Without the developer harness: no FENG_DEBUG.
#[cfg(not(any(test, feature = "dev-harness")))]
fn feng_debug() -> bool {
    false
}

/// The package's quads at `now` ms, back to front.
///
/// The FEng renderer (0x52C5E0 → 0x51B7F0 / 0x51B850 → 0x5370B0): each
/// group's world transform and colour are its own composed with its
/// parent's (0x51B0B0: matrix local · parent, colour (own × parent +
/// 0x80) >> 8 per channel); a group with alpha 0 skips its children.
/// Every drawn object goes to the draw bucket of its own z plus its
/// parent's world z, truncated and clamped to 0..255; the buckets draw
/// from 255 down to 0 (0x537249), each in the order it was filled. The
/// canvas is the world moved by (320, 240) (0x536F70).
pub fn layout(assets: &Assets, package: &Package, state: &ScreenState, now: f32) -> Vec<Quad> {
    let index = tree_index(package);
    // The package's run, then each object's attributes when its place is
    // worked out (a hidden one's never are, see `full`): the same values
    // as [`attributes`] gives.
    let run = run_package(package, state, now);
    let attrs = Attrs {
        package,
        state,
        run: &run,
        now: now * FE_RATE,
        cache: std::cell::RefCell::new(vec![None; package.objects.len()]),
    };
    let mut placed: Vec<Option<Placed>> = vec![None; package.objects.len()];
    // A hidden object's transform, colour and keys are read only by the
    // mouse's hit tests (`mouse::record_hidden`): without them (no frame
    // recording) only its `hidden` and `root` are worked out, for its
    // children (which a hidden group hides too). The quads are the same.
    let full = mouse::recording();
    #[allow(clippy::too_many_arguments)]
    fn place(
        i: usize,
        depth: usize,
        package: &Package,
        state: &ScreenState,
        attrs: &Attrs,
        parents: &[Option<usize>],
        placed: &mut Vec<Option<Placed>>,
        full: bool,
    ) -> Placed {
        if let Some(p) = placed[i] {
            return p;
        }
        let o = &package.objects[i];
        if !full {
            let parent = parents[i].filter(|_| depth < 16);
            let shut = match parent {
                Some(g) => {
                    let up = place(g, depth + 1, package, state, attrs, parents, placed, full);
                    (up.hidden || up.color[3] == 0 || hidden(o, state)).then_some(up.root)
                }
                None => hidden(o, state).then_some(i),
            };
            if let Some(root) = shut {
                let p = Placed {
                    world: Mat4::ZERO,
                    color: [0; 4],
                    hidden: true,
                    z: 0.0,
                    key: Mat4::ZERO,
                    queue_z: 0.0,
                    root,
                };
                placed[i] = Some(p);
                return p;
            }
        }
        let a = &attrs.get(i);
        let mut position = a.position();
        if let Some(p) = state.position.get(&o.hash) {
            // 0x51D2E0 / 0x51D4E0 → 0x5178D0 write (x, y, the object's
            // current z) into the position track of every script the
            // object has (its list +0x4C, not only the running one) and
            // into the object: set at the screen's construction, the z
            // is the INIT one, which a later FADEIN keeps (UI_PostRaceStats'
            // option_data: FADEIN's z 100 never shows).
            position.x = p.x;
            position.y = p.y;
            position.z = o.attributes.position().z;
        }
        let mut size = a.size();
        if let Some(s) = state.size.get(&o.hash) {
            size.x = s.x;
            size.y = s.y;
        }
        let rotation = state.angle.get(&o.hash).map_or_else(|| a.rotation(), |&r| Quat::from_rotation_z(r));
        let own = state.color.get(&o.hash).copied().unwrap_or_else(|| a.color()).map(|c| (c * 255.0).round() as i32);
        let local = local_matrix(size, a.pivot(), rotation, position);
        let key = key_matrix(a.pivot(), rotation, position);
        let parent = parents[i].filter(|_| depth < 16);
        let p = match parent {
            Some(g) => {
                let up = place(g, depth + 1, package, state, attrs, parents, placed, full);
                let color = std::array::from_fn(|k| (own[k] * up.color[k] + 0x80) >> 8);
                // 0x51B850: own z + the parent's world z.
                let z = position.z + up.world.w_axis.z;
                let queue_z = (up.key * (a.pivot() + position).extend(1.0)).z;
                Placed {
                    world: up.world * local,
                    color,
                    hidden: up.hidden || up.color[3] == 0 || hidden(o, state),
                    z,
                    key: up.key * key,
                    queue_z,
                    root: up.root,
                }
            }
            None => Placed {
                world: local,
                color: own,
                hidden: hidden(o, state),
                z: position.z,
                key,
                queue_z: a.pivot().z + position.z,
                root: i,
            },
        };
        placed[i] = Some(p);
        p
    }
    let order = &index.order;
    // Each element's place on a screen that is not 4:3 (clone deviation).
    let mut places: Vec<Option<Place>> = vec![None; package.objects.len()];
    // Per queued object: (bucket, queue z, queue position, its quads'
    // range in `all`).
    let mut queued: Vec<(f32, f32, usize, std::ops::Range<usize>)> = Vec::new();
    let mut all: Vec<Quad> = Vec::new();
    for (i, o) in package.objects.iter().enumerate() {
        let p = place(i, 0, package, state, &attrs, &index.parents, &mut placed, full);
        // 0x51A970: nothing with alpha 0 is queued, nor what its view
        // transform (0x504B50) puts at z <= 0 (0x51A9D0 against 0.0 at
        // 0x7D7D3C; a NaN passes). The view matrix is the identity
        // (0x536530 passes the matrix the manager's slot +0x3C, 0x50C490
        // → 0x43C780, fills), so the test is on pivot + position's z
        // through the groups' size-free key matrices (`key_matrix`).
        if p.hidden || p.color[3] <= 0 || p.queue_z <= 0.0 {
            // The mouse's hit tests (0x52D030 → 0x51DA00) read an image's
            // rectangle without its hidden flag: a hidden arrow or frame
            // still takes clicks (`mouse::rect_any`).
            if mouse::recording()
                && matches!(
                    o.kind,
                    Kind::Image | Kind::SimpleImage | Kind::ColoredImage | Kind::MultiImage | Kind::AnimImage
                )
            {
                let mut quads = Vec::new();
                push_box(
                    &mut quads,
                    None,
                    &p.world,
                    Vec2::splat(-0.5),
                    Vec2::splat(0.5),
                    [Vec2::ZERO, Vec2::ONE],
                    [1.0; 4],
                    0.0,
                );
                let root = p.root;
                let now_at = placed[root].map_or(Vec2::ZERO, |r| r.world.w_axis.truncate().truncate());
                let place =
                    places[root].unwrap_or_else(|| element_place(package, &package.objects[root], state, now_at));
                for q in &mut quads {
                    q.place = side_strip(package, place, q);
                }
                mouse::record_hidden(&package.name, o.hash, &quads);
            }
            continue;
        }
        let first = all.len();
        let quads = &mut all;
        // Every FE draw path halves the vertex colour's red, green and
        // blue with an integer division before 0x51A1C0 (images 0x536A60,
        // movies 0x536860, coloured images 0x536C90, strings 0x51C110:
        // `c / 2`, the alpha whole) and fe.efx's pixel shader doubles the
        // product again (`add r0.xyz, r0, r0`). **Observed** (Wine,
        // 1280 × 960, the strips' U2_MENU_BOX_WHITE): the frames match the
        // full colour (92.4, 92.1, 91.8 against 92.4, 92.1, 91.7), not the
        // halved one with its last bit dropped (91.8, 91.8, 91.1), so the
        // clone keeps the full colour; why the odd bit survives is **open**.
        let color = p.color.map(|c| c.clamp(0, 255) as f32 / 255.0);
        // A coloured image (0x536C90) colours its vertices with its own
        // corner colours (SA dwords 21..36), not the object's colour; the
        // four are equal in every shipped one (the loading screens' tip
        // box, 0, 0, 0, 235), so the first stands for all.
        let a = attrs.get(i);
        let color: [f32; 4] = if o.kind == Kind::ColoredImage && a.len >= 25 {
            let w = &a.words;
            let c = |j: usize| (w[j] as i32).clamp(0, 255);
            [c(23), c(22), c(21), c(24)].map(|c| c as f32 / 255.0)
        } else {
            color
        };
        let z = p.z.trunc().clamp(0.0, 255.0);
        if feng_debug() {
            let at = p.world.w_axis.truncate().truncate() + CANVAS * 0.5;
            eprintln!("feng {:08X} {:?} at {at:?} alpha {} z {z} queue {}", o.hash, o.kind, color[3], p.queue_z);
        }
        let a = &a;
        let resource = package.resources.get(o.resource as usize);
        match o.kind {
            Kind::Image | Kind::SimpleImage | Kind::ColoredImage | Kind::MultiImage | Kind::AnimImage => {
                let texture = state.image.get(&o.hash).copied().or_else(|| match resource {
                    Some(r) if r.kind == ResourceKind::Texture => assets.texture_for(&r.name),
                    _ => None,
                });
                if resource.is_some_and(|r| r.kind == ResourceKind::Texture) && texture.is_none() {
                    continue;
                }
                let uv = state.uv.get(&o.hash).copied().unwrap_or_else(|| a.uv());
                // The unit quad ±0.5 (0x536A60 → 0x51A1C0).
                push_box(
                    quads,
                    texture,
                    &p.world,
                    Vec2::splat(-0.5),
                    Vec2::splat(0.5),
                    [Vec2::new(uv[0], uv[1]), Vec2::new(uv[2], uv[3])],
                    color,
                    z,
                );
            }
            // 0x536860: a movie object draws its movie's texture (set by
            // the screen) on the unit quad, at the corners the screen gives.
            Kind::Movie => {
                let Some(&texture) = state.image.get(&o.hash) else { continue };
                let uv = state.uv.get(&o.hash).copied().unwrap_or([0.0, 0.0, 1.0, 1.0]);
                push_box(
                    quads,
                    Some(texture),
                    &p.world,
                    Vec2::splat(-0.5),
                    Vec2::splat(0.5),
                    [Vec2::new(uv[0], uv[1]), Vec2::new(uv[2], uv[3])],
                    color,
                    z,
                );
            }
            Kind::String => {
                let Some(font) = resource.and_then(|r| assets.font_for(&r.name)) else { continue };
                let text = state.text.get(&o.hash).cloned().unwrap_or_else(|| {
                    o.label.and_then(|h| assets.strings.get(h)).map_or_else(|| o.text.clone(), str::to_string)
                });
                // The block's line count (0x50C080) on the string as set,
                // with its wrap width when it has one.
                let lines = Some(if o.max_width > 0 {
                    line_count_wrapped(font, &text, o.max_width as f32)
                } else {
                    line_count(&text)
                });
                let text = if o.max_width > 0 { wrap_feng(font, &text, o.max_width as f32) } else { text };
                string_quads(assets, font, &text, o.justify, o.leading, lines, &p.world, color, z, quads);
                #[cfg(any(test, feature = "dev-harness"))]
                if quads.len() > first {
                    DRAWN_TEXT.with(|d| {
                        if let Some(v) = d.borrow_mut().as_mut() {
                            v.push((o.hash, text.clone()));
                        }
                    });
                }
            }
            _ => {}
        }
        let root = p.root;
        let mut place = *places[root].get_or_insert_with(|| {
            let now_at = placed[root].map_or(Vec2::ZERO, |r| r.world.w_axis.truncate().truncate());
            element_place(package, &package.objects[root], state, now_at)
        });
        place.fill = matches!(o.kind, Kind::Image | Kind::SimpleImage | Kind::ColoredImage)
            && resource.is_some_and(|r| {
                r.kind == ResourceKind::Texture && r.name.to_ascii_lowercase().starts_with("white16x16")
            });
        let quads = &mut all[first..];
        for q in quads.iter_mut() {
            q.place = side_strip(package, place, q);
        }
        if mouse::recording() {
            // The object and its groups (a group's rectangle is its
            // children's, for the mouse).
            mouse::record(&package.name, o.hash, quads);
            let mut g = index.parents[i];
            for _ in 0..16 {
                let Some(pi) = g else { break };
                mouse::record(&package.name, package.objects[pi].hash, quads);
                g = index.parents[pi];
            }
        }
        queued.push((z, p.queue_z, order[i], first..all.len()));
    }
    // Back to front: the larger bucket first (0x537249), each in fill
    // order. The fill order is the queue sorted by its z (0x5114F0: a
    // stable LSD radix sort over the float's four bytes, ascending) and
    // walked from its end (0x50C2C0): larger queue z first, and among
    // equal ones the later queued first.
    queued.sort_by(|a, b| b.0.total_cmp(&a.0).then(b.1.total_cmp(&a.1)).then(b.2.cmp(&a.2)));
    let mut out = Vec::with_capacity(all.len());
    for q in queued {
        out.extend_from_slice(&all[q.3]);
    }
    out
}
