//! Drawing the NIS scenes' characters ([`crate::character`]): the posed
//! meshes skinned on the CPU into world-space triangles each frame and drawn
//! through the world pipelines (world.efx, the texture's blend and alpha
//! test), depth-tested and written. **Not native** shading: the character
//! effect (render info +0xE8 palette draw) is not traced, so each corner's
//! vertex colour is the clone's half-Lambert under a fixed sun
//! ([`crate::character::shade`]), halved for world.efx's ×2.

use std::collections::HashMap;
use std::sync::Arc;

use glam::{Mat4, Vec3};

use crate::character::{SceneCharacters, SkinBounds, shade};
use crate::formats::solid::Vertex;
use crate::render::{Effect, Gpu, Pending, PipelineKey, WorldRenderer};

/// Corners a frame may hold (21 crowd members of ~1–2.5k corners and the
/// starter's ~8k).
const MAX_CORNERS: usize = 160_000;

/// The characters' instance mark (m0.w; world.wgsl `ML_CHARACTER_MARK`,
/// see vertex.wgsl): Modern Lighting lights them without the face normal. The native
/// shaders read m0.w only above 0.5 (a draw with a matrix), so this is the
/// particles' instance (0) to them.
pub const CHARACTER_MARK: f32 = 0.25;

/// The scene's character textures and this frame's skinned corners.
pub struct CharacterRenderer {
    /// Each texture's world slot and pipeline (shared with the skinning
    /// job, which only asks which textures are loaded).
    textures: Arc<HashMap<u32, (usize, PipelineKey)>>,
    vertices: wgpu::Buffer,
    /// The characters' instance: the identity, marked for Modern Lighting
    /// ([`CHARACTER_MARK`]).
    instance: wgpu::Buffer,
    /// This frame's corners and draw runs, and the skinning's scratch (kept
    /// between frames; out on a render thread while a job skins).
    skin: Skin,
    // Bound to the same immutable SceneCharacters models as `textures`.
    // Game's SceneCrowd constructors replace set and renderer together.
    bounds: Arc<Vec<Option<SkinBounds>>>,
    can_cull: bool,
    /// The frame skinned on a render thread, with the inputs it was
    /// started on: used only if [`CharacterRenderer::render`] asks for the
    /// very same ones (else it is skinned again in place).
    pending: Option<(SkinInputs, Pending<Skin>)>,
}

/// One frame's skinned corners and draw runs (texture hash, first corner,
/// count), with the skinning's scratch.
#[derive(Default)]
struct Skin {
    cpu: Vec<Vertex>,
    runs: Vec<(u32, u32, u32)>,
    done: Vec<u32>,
    posed: Vec<(Vec3, u8)>,
}

/// What a frame's skinning depends on besides the scene (compared by bits).
#[derive(Clone, PartialEq)]
struct SkinInputs {
    view_proj: [u32; 16],
    t: u32,
    origin: [u32; 16],
    lifts: Vec<Option<u32>>,
}

impl SkinInputs {
    fn new(view_proj: Mat4, t: f32, origin: Mat4, lifts: &[Option<f32>]) -> SkinInputs {
        SkinInputs {
            view_proj: view_proj.to_cols_array().map(f32::to_bits),
            t: t.to_bits(),
            origin: origin.to_cols_array().map(f32::to_bits),
            lifts: lifts.iter().map(|l| l.map(f32::to_bits)).collect(),
        }
    }
}

impl CharacterRenderer {
    /// Upload a scene's character textures.
    pub fn new(gpu: &Gpu, world: &mut WorldRenderer, set: &SceneCharacters) -> CharacterRenderer {
        // Through the world's own texture slots, so the draws bind them as
        // the world's pipelines expect (texture arrays or not).
        let textures = set
            .textures
            .iter()
            .map(|t| {
                (
                    t.hash,
                    (
                        world.add_texture_slot(gpu, t),
                        PipelineKey { effect: Effect::World, blend: t.blend, alpha_test: t.alpha_test },
                    ),
                )
            })
            .collect();
        let vertices = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("characters"),
            size: (MAX_CORNERS * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bounds = set.models.iter().map(SkinBounds::build).collect();
        // Skipping an offscreen model must not free the old corner budget
        // for later models: that would change which visible corners survive.
        // Oversized scenes keep the original unculled/truncated behavior.
        let can_cull = within_corner_budget(
            set.instances
                .iter()
                .filter_map(|i| set.models.iter().find(|m| i.solids.contains(&m.hash)))
                .flat_map(|m| m.ranges.iter().map(|r| r.index_count as usize)),
        );
        let mut m = crate::render::instance_matrix(Mat4::IDENTITY, false, 0.0, false);
        m[0][3] = CHARACTER_MARK;
        let instance = wgpu::util::DeviceExt::create_buffer_init(
            &gpu.device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("character instance"),
                contents: bytemuck::cast_slice(&m),
                usage: wgpu::BufferUsages::VERTEX,
            },
        );
        CharacterRenderer {
            textures: Arc::new(textures),
            vertices,
            instance,
            skin: Skin::default(),
            bounds: Arc::new(bounds),
            can_cull,
            pending: None,
        }
    }

    /// Start skinning the scene's characters `t` seconds in, placed by
    /// `origin` with the grounds `lifts` (`SceneCharacters::posed`), seen
    /// through `view_proj`, on a render thread (off the
    /// main thread, while it records the world and the cars). The same
    /// function on the same inputs as [`CharacterRenderer::render`] runs
    /// in place, so the same corners in the same order.
    pub fn start(&mut self, set: &Arc<SceneCharacters>, view_proj: Mat4, t: f32, origin: Mat4, lifts: &[Option<f32>]) {
        if let Some((_, p)) = self.pending.take() {
            self.skin = p.wait().unwrap_or_default();
        }
        let mut skin = std::mem::take(&mut self.skin);
        let (set, textures, bounds, can_cull) =
            (set.clone(), self.textures.clone(), self.bounds.clone(), self.can_cull);
        let inputs = SkinInputs::new(view_proj, t, origin, lifts);
        let lifts = lifts.to_vec();
        let job = Pending::spawn(move || {
            skin_frame(&mut skin, &set, &textures, &bounds, can_cull, view_proj, t, origin, &lifts);
            skin
        });
        self.pending = Some((inputs, job));
    }

    /// Skin the scene's characters `t` seconds in, placed by `origin`, and
    /// draw them over what is in `color` / `depth` (the corners [`start`]ed
    /// on a render thread if that was on these inputs).
    ///
    /// [`start`]: CharacterRenderer::start
    pub fn render(
        &mut self,
        gpu: &Gpu,
        world: &WorldRenderer,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        view_proj: Mat4,
        set: &SceneCharacters,
        t: f32,
        origin: Mat4,
        lifts: &[Option<f32>],
    ) {
        let inputs = SkinInputs::new(view_proj, t, origin, lifts);
        let started = match self.pending.take() {
            Some((at, p)) => {
                let skin = p.wait();
                let hit = at == inputs && skin.is_some();
                self.skin = skin.unwrap_or_default();
                hit
            }
            None => false,
        };
        if !started {
            skin_frame(&mut self.skin, set, &self.textures, &self.bounds, self.can_cull, view_proj, t, origin, lifts);
        }
        let Skin { cpu: vertices, runs, .. } = &self.skin;
        if vertices.is_empty() {
            return;
        }
        gpu.queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(vertices));
        let scene = world.scene_pipelines();
        // Modern + RT: the characters into the depth first and the traced
        // shadows' blocks again over their box, so their colour pass reads
        // their lights' rays as the world's pixels do (their own pipelines
        // traced them in the colour shader, every pixel at the scene's size).
        // `NFSU2_RT_CHAR_RAYS=1` keeps the in-shader rays (a check aid).
        let plan = world.rt_shadow_plan.lock().unwrap().clone();
        let blocks = match (&plan, &scene.depth) {
            (Some(plan), Some(prepass))
                if !scene.characters.is_empty() && !crate::render::debug_env::nfsu2_rt_char_rays() =>
            {
                self.depth_pass(world, encoder, depth, &prepass.depth, runs);
                let (lo, hi) = screen_box(vertices, view_proj, plan.blocks());
                plan.record_region(encoder, depth, lo, hi);
                crate::prof::gpu::mark(encoder, "characters: rt shadows");
                true
            }
            _ => false,
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("characters"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &world.scene_group(), &[]);
        pass.set_bind_group(2, &world.no_reflection, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_vertex_buffer(1, self.instance.slice(..));
        for &(tex, first, count) in runs {
            let (slot, key) = self.textures[&tex];
            // Modern + RT's character pipelines (their own rays) unless the
            // blocks hold them.
            let own = if blocks { None } else { scene.characters.get(&key) };
            pass.set_pipeline(own.unwrap_or(&scene.pipelines[&key]));
            // A pipeline set clears the immediates: bind the slot afresh.
            let mut bound = usize::MAX;
            world.set_texture(&mut pass, slot, &mut bound);
            pass.draw(first..first + count, 0..1);
        }
    }
}

impl CharacterRenderer {
    /// The characters' opaque runs into `depth` with the main view's
    /// pre-pass pipelines (blended runs write no depth there either).
    fn depth_pass(
        &self,
        world: &WorldRenderer,
        encoder: &mut wgpu::CommandEncoder,
        depth: &wgpu::TextureView,
        pipelines: &HashMap<crate::render::DepthKey, wgpu::RenderPipeline>,
        runs: &[(u32, u32, u32)],
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("characters depth"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &world.scene_group(), &[]);
        pass.set_bind_group(2, &world.no_reflection, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_vertex_buffer(1, self.instance.slice(..));
        for &(tex, first, count) in runs {
            let (slot, key) = self.textures[&tex];
            if key.blend != crate::formats::texture::Blend::None {
                continue;
            }
            let k = crate::render::DepthKey { reflect: false, alpha_test: key.alpha_test, fade: 0 };
            let Some(p) = pipelines.get(&k) else { continue };
            pass.set_pipeline(p);
            // A pipeline set clears the immediates: bind the slot afresh.
            let mut bound = usize::MAX;
            world.set_texture(&mut pass, slot, &mut bound);
            pass.draw(first..first + count, 0..1);
        }
    }
}

/// The blocks (of a `blocks` grid over the screen) the skinned `vertices`
/// cover under `view_proj`, one block of margin (the blocks' normals read
/// their neighbours); the whole grid when a corner is behind the eye.
fn screen_box(vertices: &[Vertex], view_proj: Mat4, blocks: (u32, u32)) -> ((u32, u32), (u32, u32)) {
    let (mut lo, mut hi) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
    for v in vertices {
        let c = view_proj * Vec3::from(v.position).extend(1.0);
        if c.w <= 1e-3 {
            return ((0, 0), blocks);
        }
        let n = glam::Vec2::new(c.x, c.y) / c.w;
        lo = lo.min(n);
        hi = hi.max(n);
    }
    let b = glam::Vec2::new(blocks.0 as f32, blocks.1 as f32);
    // NDC y up, blocks y down.
    let a = (glam::Vec2::new(lo.x, -hi.y) * 0.5 + 0.5) * b;
    let z = (glam::Vec2::new(hi.x, -lo.y) * 0.5 + 0.5) * b;
    let a = (a - 1.0).max(glam::Vec2::ZERO).min(b);
    let z = (z + 1.0).max(glam::Vec2::ZERO).min(b).ceil();
    ((a.x as u32, a.y as u32), (z.x as u32, z.y as u32))
}

/// One frame's skinning (no GPU access; on the main thread or a render
/// thread): the scene's characters posed `t` seconds in under `origin`,
/// the provably offscreen ones left out when `can_cull`, each corner's
/// world position, half-Lambert grey and uv into `skin.cpu` (at most
/// [`MAX_CORNERS`]) and the draw runs into `skin.runs`, in instance and
/// range order. `textures` says which textures are loaded.
#[allow(clippy::too_many_arguments)]
fn skin_frame<V>(
    skin: &mut Skin,
    set: &SceneCharacters,
    textures: &HashMap<u32, V>,
    bounds: &[Option<SkinBounds>],
    can_cull: bool,
    view_proj: Mat4,
    t: f32,
    origin: Mat4,
    lifts: &[Option<f32>],
) {
    let light = Vec3::new(0.3, -0.4, 0.85).normalize();
    let Skin { cpu: vertices, runs, done, posed } = skin;
    vertices.clear();
    runs.clear();
    for (model, palette, at) in set.posed(t, origin, lifts) {
        // Sample every animation as before. Only skinning/emission for a
        // provably offscreen whole instance is omitted; uncertainty draws.
        if can_cull
            && bounds
                .get(model)
                .and_then(Option::as_ref)
                .and_then(|b| b.posed(&palette, at))
                .is_some_and(|b| !b.visible(view_proj))
        {
            continue;
        }
        let m = &set.models[model];
        // A slot's texture if it is loaded, looked up once a range (a
        // range's corners share their slot); a range without one is
        // left out unposed.
        let tex_of = |slot: u8| m.textures.get(slot as usize).copied().filter(|t| textures.contains_key(t));
        let tex = std::cell::Cell::new(None);
        m.skin_ranges(
            &palette,
            at,
            done,
            posed,
            // The corner's vertex colour from its normal, once a vertex.
            |p, n| (p, (shade(n, light) * 127.0) as u8),
            |slot| {
                tex.set(tex_of(slot));
                tex.get().is_some()
            },
            |(p, g), uv, _| {
                if vertices.len() >= MAX_CORNERS {
                    return;
                }
                let Some(tex) = tex.get() else { return };
                let k = vertices.len() as u32;
                match runs.last_mut() {
                    Some(r) if r.0 == tex && r.1 + r.2 == k => r.2 += 1,
                    _ => runs.push((tex, k, 1)),
                }
                vertices.push(Vertex { position: p.to_array(), color: [g, g, g, 0xFF], uv });
            },
        );
    }
}

fn within_corner_budget(mut counts: impl Iterator<Item = usize>) -> bool {
    counts.try_fold(0usize, |sum, count| sum.checked_add(count)).is_some_and(|total| total <= MAX_CORNERS)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visibility_never_changes_overflow_truncation() {
        assert!(within_corner_budget([80_000, 80_000].into_iter()));
        assert!(!within_corner_budget([80_000, 80_001].into_iter()));
        assert!(!within_corner_budget([usize::MAX, 1].into_iter()));
    }

    /// The characters' box in blocks: a corner's projection with a block's
    /// margin, clamped to the grid; the whole grid when one is behind the eye.
    #[test]
    fn screen_box_covers_the_corners_with_a_margin() {
        let v = |x: f32, y: f32| Vertex { position: [x, y, 0.5], color: [0; 4], uv: [0.0; 2] };
        let (lo, hi) = screen_box(&[v(-0.5, 0.5), v(0.0, 0.0)], Mat4::IDENTITY, (100, 50));
        assert_eq!((lo, hi), ((24, 11), (51, 26)));
        let behind = Mat4::from_cols_array(&[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, -1.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(screen_box(&[v(0.0, 0.0)], behind, (100, 50)), ((0, 0), (100, 50)));
    }

    /// The crowd skinned on a render thread ([`CharacterRenderer::start`]'s
    /// job) gives the very corners and runs (by their bits) of the same
    /// function run in place, frame after frame with the scratch reused,
    /// for every IntroNis scene's characters, culled and not.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn skinning_on_a_render_thread_matches_in_place() {
        let root = crate::testkit::game_root();
        let origin = Mat4::from_translation(Vec3::new(1000.0, -2000.0, 30.0));
        let view_proj = Mat4::perspective_rh(1.0, 16.0 / 9.0, 0.5, 2000.0)
            * Mat4::look_at_rh(Vec3::new(1004.0, -2012.0, 32.0), Vec3::new(1000.0, -2000.0, 31.0), Vec3::Z);
        let mut scenes = 0;
        for k in 1..=18 {
            let Ok(b) =
                std::fs::read(std::path::Path::new(&root).join(format!("NIS/Scene_IntroNis{k:02}_BundleB.bun")))
            else {
                continue;
            };
            let Ok(set) = SceneCharacters::parse(&b) else { continue };
            let set = Arc::new(set);
            scenes += 1;
            let textures: Arc<HashMap<u32, ()>> = Arc::new(set.textures.iter().map(|t| (t.hash, ())).collect());
            let bounds: Arc<Vec<Option<SkinBounds>>> = Arc::new(set.models.iter().map(SkinBounds::build).collect());
            let (mut here, mut there) = (Skin::default(), Skin::default());
            for f in 0..40 {
                let t = f as f32 * 0.37;
                for can_cull in [false, true] {
                    skin_frame(&mut here, &set, &textures, &bounds, can_cull, view_proj, t, origin, &[]);
                    let (s, tx, bo) = (set.clone(), textures.clone(), bounds.clone());
                    let mut moved = std::mem::take(&mut there);
                    there = Pending::spawn(move || {
                        skin_frame(&mut moved, &s, &tx, &bo, can_cull, view_proj, t, origin, &[]);
                        moved
                    })
                    .wait()
                    .unwrap();
                    assert_eq!(here.runs, there.runs);
                    assert_eq!(
                        bytemuck::cast_slice::<Vertex, u8>(&here.cpu),
                        bytemuck::cast_slice::<Vertex, u8>(&there.cpu),
                        "scene {k} t {t}"
                    );
                }
            }
        }
        assert!(scenes > 0);
    }
}
