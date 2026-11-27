//! Exact-path tests on the device: the streak buffer's tick form
//! ([`StreakFrame`]) against the per-frame path ([`PostFx::apply`]), bit for
//! bit. They need a GPU (`#[ignore]`); run them explicitly:
//! `cargo test --release --features dev-harness --lib streak_ticks -- --ignored`.

use super::*;

const SIZE: (u32, u32) = (64, 48);

/// The settings in force (the tests apply them first), as a frame takes them.
fn opts() -> crate::render::RenderOptions {
    crate::render::RenderOptions::in_force()
}

/// A glow list: additive rectangles (rect, colour), as the glow sprites add.
type Glows = Vec<([u32; 4], [f64; 3])>;

/// Tick `i`'s glows: a head that moves 3 px a tick (a trail behind it in
/// the buffer), a still lamp, and one that blinks. `i` may be fractional
/// (an interpolated state).
fn glows(i: f32) -> Glows {
    let x = (4.0 + 3.0 * i) as u32 % 56;
    let mut g = vec![([x, 10, 6, 5], [0.9, 0.5, 0.2]), ([40, 30, 8, 8], [0.25, 0.3, 0.6])];
    if (i as u32 / 4) % 2 == 0 {
        g.push(([10, 36, 4, 4], [0.2, 0.8, 0.1]));
    }
    g
}

struct Kit {
    gpu: Gpu,
    pipelines: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    out: wgpu::Texture,
}

impl Kit {
    fn new() -> Kit {
        let gpu = pollster::block_on(Gpu::new(None, wgpu::Instance::default())).unwrap();
        let out = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("out"),
            size: wgpu::Extent3d { width: SIZE.0, height: SIZE.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        Kit { gpu, pipelines: Vec::new(), out }
    }

    /// ONE · constant + ONE · dest over a rectangle: a glow sprite's add.
    fn pipeline(&mut self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        if !self.pipelines.iter().any(|p| p.0 == format) {
            let shader = self.gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("test glow"),
                source: wgpu::ShaderSource::Wgsl(
                    "@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
                        let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
                        return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
                    }
                    @fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }"
                        .into(),
                ),
            });
            let add = wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Constant,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            };
            let p = self.gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("test glow"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState { color: add, alpha: add }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
            self.pipelines.push((format, p));
        }
        &self.pipelines.iter().find(|p| p.0 == format).unwrap().1
    }

    fn draw_glows(
        pipeline: &wgpu::RenderPipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        list: &Glows,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("test glows"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            ..Default::default()
        });
        pass.set_pipeline(pipeline);
        for ([x, y, w, h], [r, g, b]) in list {
            pass.set_scissor_rect(*x, *y, (*w).min(SIZE.0 - x), (*h).min(SIZE.1 - y));
            pass.set_blend_constant(wgpu::Color { r: *r, g: *g, b: *b, a: 1.0 });
            pass.draw(0..3, 0..1);
        }
    }

    fn read(&self, t: &wgpu::Texture) -> Vec<u8> {
        let (w, h) = (t.size().width, t.size().height);
        let bpp = t.format().block_copy_size(None).unwrap();
        let row = (w * bpp).div_ceil(256) * 256;
        let buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("read"),
            size: u64::from(row * h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut e = self.gpu.device.create_command_encoder(&Default::default());
        e.copy_texture_to_buffer(
            t.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) },
            },
            t.size(),
        );
        self.gpu.queue.submit([e.finish()]);
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        let data = buf.slice(..).get_mapped_range().expect("mapped readback");
        (0..h).flat_map(|y| data[(y * row) as usize..][..(w * bpp) as usize].to_vec()).collect()
    }

    /// The scene: a fixed gradient with a bright patch (so the overbright
    /// pass's A is not empty).
    /// The scene of tick `i` (the bright patches moved `i` px, so
    /// the overbright pass's A differs from tick to tick; 0 the old one).
    fn scene_at(&mut self, post: &mut PostFx, i: u32) {
        let view = post.scene_view(&self.gpu, SIZE, &opts());
        let format = post.scene_format();
        let p = self.pipeline(format).clone();
        let mut e = self.gpu.device.create_command_encoder(&Default::default());
        clear(&mut e, &view, wgpu::Color { r: 0.1, g: 0.12, b: 0.2, a: 1.0 });
        let patches: Glows =
            (0..8u32).map(|k| ([(k * 8 + i) % 54, k * 5, 10, 9], [0.1 * f64::from(k), 0.6, 0.9])).collect();
        Self::draw_glows(&p, &mut e, &view, &patches);
        self.gpu.queue.submit([e.finish()]);
    }

    /// One drawn frame: the scene, then the post passes over it with the
    /// glow lists `pick` names. -> (output, streak buffer).
    fn frame(
        &mut self,
        post: &mut PostFx,
        switches: Switches,
        streak: Option<StreakFrame>,
        frames: u32,
        pick: &dyn Fn(StreakGlow) -> Glows,
    ) -> (Vec<u8>, Vec<u8>) {
        self.frame_at(post, 0, switches, streak, frames, pick)
    }

    /// [`Kit::frame`] over tick `scene`'s scene ([`Kit::scene_at`]).
    fn frame_at(
        &mut self,
        post: &mut PostFx,
        scene: u32,
        switches: Switches,
        streak: Option<StreakFrame>,
        frames: u32,
        pick: &dyn Fn(StreakGlow) -> Glows,
    ) -> (Vec<u8>, Vec<u8>) {
        self.scene_at(post, scene);
        post.multisampled(&self.gpu, SIZE, 1, &opts());
        let format = post.inter();
        let p = self.pipeline(format).clone();
        let out = self.out.create_view(&Default::default());
        let mut e = self.gpu.device.create_command_encoder(&Default::default());
        match streak {
            None => post.apply(&self.gpu, &opts(), &mut e, &out, switches, Frame::GAME, frames, false, &mut |e, v, _| {
                Self::draw_glows(&p, e, v, &pick(StreakGlow::Commit(0)))
            }),
            Some(s) => post.apply_ticks(&self.gpu, &opts(), &mut e, &out, switches, Frame::GAME, s, false, &mut |w, e, v, _| {
                Self::draw_glows(&p, e, v, &pick(w))
            }),
        }
        self.gpu.queue.submit([e.finish()]);
        (self.read(&self.out), self.read(&post.targets.as_ref().unwrap().streak))
    }

    /// Tick `i` committed alone ([`PostFx::commit_tick`]) over its
    /// own scene with its own glows. -> the streak buffer.
    fn commit(&mut self, post: &mut PostFx, i: u32, switches: Switches) -> Vec<u8> {
        self.scene_at(post, i);
        post.multisampled(&self.gpu, SIZE, 1, &opts());
        let format = post.inter();
        let p = self.pipeline(format).clone();
        let mut e = self.gpu.device.create_command_encoder(&Default::default());
        post.commit_tick(&self.gpu, &opts(), &mut e, switches, Frame::GAME, &mut |w, e, v, _| {
            assert_eq!(w, StreakGlow::Commit(0));
            Self::draw_glows(&p, e, v, &glows(i as f32))
        });
        self.gpu.queue.submit([e.finish()]);
        self.read(&post.targets.as_ref().unwrap().streak)
    }
}

fn switch_sets() -> [(&'static str, Switches); 2] {
    let all = Switches::from_detail(5);
    [("overbright", all), ("streaks only", Switches { overbright: false, ..all })]
}

/// A still's warm-up (16 commits) so the overbright level is settled and the
/// buffer holds a trail; the same on both paths.
fn warm(kit: &mut Kit, post: &mut PostFx, s: Switches) {
    kit.frame(post, s, None, 16, &|_| glows(0.0));
}

/// 60 Hz: one tick a frame, α = 0, through `apply_ticks` = the per-frame
/// `apply`, output and buffer, bit for bit.
#[test]
#[ignore = "requires a GPU; run explicitly"]
fn streak_ticks_at_60_are_the_per_frame_path() {
    let _g = crate::qol::test_globals();
    crate::qol::Enhancements::default().apply();
    let mut kit = Kit::new();
    for (name, s) in switch_sets() {
        let (mut a, mut b) = (
            PostFx::new(&kit.gpu, wgpu::TextureFormat::Rgba8Unorm),
            PostFx::new(&kit.gpu, wgpu::TextureFormat::Rgba8Unorm),
        );
        warm(&mut kit, &mut a, s);
        warm(&mut kit, &mut b, s);
        for i in 0..60 {
            let old = kit.frame(&mut a, s, None, 1, &|_| glows(i as f32));
            let new = kit.frame(&mut b, s, Some(StreakFrame::tick(1, 0.0)), 1, &|w| {
                assert_eq!(w == StreakGlow::Commit(0) || w == StreakGlow::Frame, true);
                glows(i as f32)
            });
            assert!(old == new, "{name}: frame {i}");
            assert!(old.1.iter().any(|&v| v != 0), "{name}: the buffer holds the glows");
        }
    }
}

/// 30 Hz: two commits a frame (ticks 2k and 2k + 1, each its own glows):
/// the buffer and the image of frame k = the 60 Hz path's after frame 2k + 1.
#[test]
#[ignore = "requires a GPU; run explicitly"]
fn streak_ticks_at_30_commit_twice() {
    let _g = crate::qol::test_globals();
    crate::qol::Enhancements::default().apply();
    let mut kit = Kit::new();
    for (name, s) in switch_sets() {
        let (mut a, mut b) = (
            PostFx::new(&kit.gpu, wgpu::TextureFormat::Rgba8Unorm),
            PostFx::new(&kit.gpu, wgpu::TextureFormat::Rgba8Unorm),
        );
        warm(&mut kit, &mut a, s);
        warm(&mut kit, &mut b, s);
        for k in 0..30 {
            kit.frame(&mut a, s, None, 1, &|_| glows((2 * k) as f32));
            let old = kit.frame(&mut a, s, None, 1, &|_| glows((2 * k + 1) as f32));
            let new = kit.frame(&mut b, s, Some(StreakFrame::tick(2, 0.0)), 1, &|w| match w {
                StreakGlow::Commit(0) => glows((2 * k) as f32),
                StreakGlow::Commit(1) => glows((2 * k + 1) as f32),
                // The reflections' copy: the drawn state's.
                StreakGlow::Frame => glows((2 * k + 1) as f32),
                other => panic!("{other:?}"),
            });
            assert!(old.1 == new.1, "{name}: buffer after frame {k}");
            assert!(old.0 == new.0, "{name}: image of frame {k}");
        }
    }
}

/// 120 Hz: a tick frame (α = 0) then a frame between ticks (α = ½). The
/// α = 0 frames are the 60 Hz frames bit for bit; a frame between ticks
/// leaves the committed buffer as it was; B(α) with the next tick's glows
/// (α → 1) is the next 60 Hz frame's image (B(1⁻) = C_n).
#[test]
#[ignore = "requires a GPU; run explicitly"]
fn streak_ticks_at_120_show_the_scratch_between() {
    let _g = crate::qol::test_globals();
    crate::qol::Enhancements::default().apply();
    let mut kit = Kit::new();
    for (name, s) in switch_sets() {
        let (mut a, mut b) = (
            PostFx::new(&kit.gpu, wgpu::TextureFormat::Rgba8Unorm),
            PostFx::new(&kit.gpu, wgpu::TextureFormat::Rgba8Unorm),
        );
        warm(&mut kit, &mut a, s);
        warm(&mut kit, &mut b, s);
        let mut old = kit.frame(&mut a, s, None, 1, &|_| glows(0.0));
        let tick0 = kit.frame(&mut b, s, Some(StreakFrame::tick(1, 0.0)), 1, &|_| glows(0.0));
        assert!(old == tick0, "{name}: tick 0");
        for i in 0..40 {
            // Half way: the interpolated state's glows; the buffer is kept.
            let half = kit.frame(&mut b, s, Some(StreakFrame::tick(0, 0.5)), 1, &|w| {
                assert_eq!(w, StreakGlow::Frame);
                glows(i as f32 + 0.5)
            });
            assert!(half.1 == old.1, "{name}: C kept after the scratch frame {i}");
            // α → 1 with tick i + 1's glows: tick i + 1's image.
            let next = kit.frame(&mut a, s, None, 1, &|_| glows((i + 1) as f32));
            let late = kit.frame(&mut b, s, Some(StreakFrame::tick(0, 0.999)), 1, &|_| glows((i + 1) as f32));
            assert!(late.0 == next.0, "{name}: B(1⁻) = C_(n) at {i}");
            assert!(late.1 == old.1, "{name}: C kept");
            // The tick: committed, as the 60 Hz frame.
            let tick = kit.frame(&mut b, s, Some(StreakFrame::tick(1, 0.0)), 1, &|_| glows((i + 1) as f32));
            assert!(tick == next, "{name}: tick {}", i + 1);
            old = next;
        }
    }
}

/// 30 Hz with the scene moving: tick 2k committed alone over ITS
/// scene (the game's probe draw), then the frame of tick 2k + 1 = the
/// 60 Hz path's two frames over their two scenes, buffer and image bit
/// for bit (the library's two commits in one frame reuse the drawn
/// scene's A for both, which this does not).
#[test]
#[ignore = "requires a GPU; run explicitly"]
fn streak_ticks_commit_alone_over_their_own_scene() {
    let _g = crate::qol::test_globals();
    crate::qol::Enhancements::default().apply();
    let mut kit = Kit::new();
    for (name, s) in switch_sets() {
        let (mut a, mut b) = (
            PostFx::new(&kit.gpu, wgpu::TextureFormat::Rgba8Unorm),
            PostFx::new(&kit.gpu, wgpu::TextureFormat::Rgba8Unorm),
        );
        warm(&mut kit, &mut a, s);
        warm(&mut kit, &mut b, s);
        for k in 0..30u32 {
            let first = kit.frame_at(&mut a, 2 * k, s, None, 1, &|_| glows((2 * k) as f32));
            let old = kit.frame_at(&mut a, 2 * k + 1, s, None, 1, &|_| glows((2 * k + 1) as f32));
            let alone = kit.commit(&mut b, 2 * k, s);
            assert!(alone == first.1, "{name}: the commit alone = the 60 Hz frame's buffer, tick {}", 2 * k);
            let new = kit.frame_at(&mut b, 2 * k + 1, s, Some(StreakFrame::tick(1, 0.0)), 1, &|_| glows((2 * k + 1) as f32));
            assert!(old == new, "{name}: frame {k}");
        }
        // A frame between ticks after a commit alone (200 Hz): the
        // committed buffer is kept.
        let c = kit.commit(&mut b, 60, s);
        let half = kit.frame_at(&mut b, 61, s, Some(StreakFrame::tick(0, 0.5)), 1, &|_| glows(60.5));
        assert!(half.1 == c, "{name}: C kept after the scratch frame");
    }
}

/// The timestamp the game's streak cost timer ends on
/// ([`PostFx::set_commit_stamp`]) is written once per chain, at the commit:
/// by the commit alone ([`PostFx::commit_tick`], the probe's chain) and by
/// a drawn frame's full chain before its output passes, with and without
/// the overbright pass; never twice, and not without a stamp asked for.
#[test]
#[ignore = "requires a GPU; run explicitly"]
fn streak_ticks_cost_timer_ends_at_the_commit() {
    let _g = crate::qol::test_globals();
    crate::qol::Enhancements::default().apply();
    let mut kit = Kit::new();
    assert!(
        kit.gpu.device.features().contains(crate::prof::gpu::FEATURES),
        "the device has no timestamp queries inside encoders"
    );
    let set = kit.gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
        label: Some("test stamps"),
        ty: wgpu::QueryType::Timestamp,
        count: 3,
    });
    let resolve = kit.gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("test stamps"),
        size: 24,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let read = kit.gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("test stamps"),
        size: 24,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    for (name, s) in switch_sets() {
        let mut post = PostFx::new(&kit.gpu, wgpu::TextureFormat::Rgba8Unorm);
        warm(&mut kit, &mut post, s);
        for full in [false, true] {
            kit.scene_at(&mut post, 1);
            post.multisampled(&kit.gpu, SIZE, 1, &opts());
            let p = kit.pipeline(post.inter()).clone();
            let out = kit.out.create_view(&Default::default());
            let mut e = kit.gpu.device.create_command_encoder(&Default::default());
            e.write_timestamp(&set, 0);
            post.set_commit_stamp(Some((set.clone(), 1)));
            let mut glow = |_: StreakGlow, e: &mut wgpu::CommandEncoder, v: &wgpu::TextureView, _: Option<&wgpu::TextureView>| {
                Kit::draw_glows(&p, e, v, &glows(1.0))
            };
            if full {
                post.apply_ticks(&kit.gpu, &opts(), &mut e, &out, s, Frame::GAME, StreakFrame::tick(1, 0.0), false, &mut glow);
            } else {
                post.commit_tick(&kit.gpu, &opts(), &mut e, s, Frame::GAME, &mut glow);
            }
            assert!(!post.take_commit_stamp(), "{name} (full chain {full}): the stamp was written");
            e.write_timestamp(&set, 2);
            e.resolve_query_set(&set, 0..3, &resolve, 0);
            e.copy_buffer_to_buffer(&resolve, 0, &read, 0, 24);
            kit.gpu.queue.submit([e.finish()]);
            read.slice(..).map_async(wgpu::MapMode::Read, |r| r.unwrap());
            let _ = kit.gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            let t: Vec<u64> =
                read.slice(..).get_mapped_range().unwrap().chunks_exact(8).map(|b| u64::from_le_bytes(b.try_into().unwrap())).collect();
            read.unmap();
            assert!(t[0] <= t[1] && t[1] <= t[2], "{name} (full chain {full}): stamps in order {t:?}");
            // Without a stamp asked for, the chain writes none.
            let mut e = kit.gpu.device.create_command_encoder(&Default::default());
            post.commit_tick(&kit.gpu, &opts(), &mut e, s, Frame::GAME, &mut glow);
            assert!(!post.take_commit_stamp());
            kit.gpu.queue.submit([e.finish()]);
        }
    }
}
