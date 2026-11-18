//! The full-screen passes at the end of the frame (0x5CAAE0, from
//! 0x5CB778): bleach bypass (screenfilter.efx), overbright (overbright.efx
//! and filter.efx) and the light-streak buffer they share. See
//! docs/formats.md "Screen filters" for the trace.
//!
//! The scene is drawn into [`PostFx::scene_view`] (the game's back buffer),
//! then [`PostFx::apply`] writes the filtered image to the output.
//!
//! Render targets (0x5BB850, all A8R8G8B8, kept 8-bit here): two
//! screen-size copies (0x86EB0C / 0x86EB88), the screen-size streak buffer
//! (0x86FF70 / 0x86F2E4) and three half-size ones (0x86FFA8.. / 0x86FF74..,
//! called A, B, C below).

use crate::render::Gpu;

mod chain;
mod fused;
mod glow_skip;
mod horizon_fog_pass;
pub mod lens_drops;
mod motion_blur_pass;
mod streaks;
mod targets;
use crate::render::enh::post::hdr::grade_lut::*;
pub use chain::*;
use fused::*;
pub use streaks::*;
use targets::*;

pub struct PostFx {
    /// 0x876848 (see [`OverbrightLevel`]).
    level: OverbrightLevel,
    /// Tick form: the level the next [`PostFx::apply`] draws with, from
    /// the game's tick-stepped level ([`PostFx::set_tick_level`]); None:
    /// the per-frame step of `level`.
    tick_level: Option<OverbrightLevel>,
    /// Tick form: the committed streak buffer C_{n−1} kept while a
    /// frame between ticks shows the scratch B(α) ([`StreakFrame`]).
    streak_keep: Option<wgpu::Texture>,
    pub(crate) format: wgpu::TextureFormat,
    layout: wgpu::BindGroupLayout,
    pub(crate) pipeline_layout: wgpu::PipelineLayout,
    pub(crate) shader: wgpu::ShaderModule,
    sampler: wgpu::Sampler,
    uniforms: wgpu::Buffer,
    gain: wgpu::Texture,
    gain_amount: Option<f32>,
    pub(crate) ramp: wgpu::Texture,
    /// By (entry, format, blend, fused variant: 0, or 1 / 2 for `q`'s
    /// ties up / to even).
    pub(crate) pipelines: Vec<((&'static str, wgpu::TextureFormat, u8, u8, u32), wgpu::RenderPipeline)>,
    /// `unorm_lut`'s texture (binding 5).
    lut: wgpu::Texture,
    /// `unorm_bounds` (binding 6): per k, the bisection's low and high
    /// float bits, the high ones the step to k + 1 once measured.
    bounds: wgpu::Buffer,
    /// The fused passes in use (their `q` variant), once the self-check
    /// passed ([`PostFx::fused_status`]).
    fused: Option<u8>,
    check: Option<Check>,
    report: Option<String>,
    /// Where the self-check's verdict is kept for this device and driver
    /// ([`check_cache_file`]).
    cache_file: Option<std::path::PathBuf>,
    pub(crate) targets: Option<Targets>,
    /// The horizon fog grid's pipeline, vertex and index buffers.
    fog: Option<(wgpu::RenderPipeline, wgpu::Buffer, wgpu::Buffer)>,
    /// Its pipeline into the multisampled scene.
    fog_ms: Option<(crate::render::SceneMode, wgpu::RenderPipeline)>,
    /// This frame's scene sample count ([`PostFx::multisampled`]) and the
    /// multisampled targets.
    pub(crate) samples: u32,
    pub(crate) msaa: Option<Msaa>,
    /// The rain's passes (made on first use) and this frame's rain.
    lens: Option<crate::render::rain::LensPass>,
    rain: RainFrame,
    /// The Graphics page's screen passes (clone deviations; made on first
    /// use), the screen's size when it differs from the scene's (Render
    /// Scale, [`PostFx::set_output_size`]), the last pass's image at the
    /// scene's size for them, and a depth buffer at the scene's size.
    enhance: Option<crate::render::enhance::Enhance>,
    pub(crate) output_size: Option<(u32, u32)>,
    /// The output size the last [`PostFx::apply`] wrote (the glow
    /// targets' size follows it, [`glow_size`]).
    last_output: Option<(u32, u32)>,
    enhanced: Option<((u32, u32), wgpu::TextureView)>,
    scaled_depth: Option<((u32, u32), wgpu::TextureView)>,
    /// The Graphics page's ambient occlusion, and this frame's values for
    /// it ([`PostFx::set_ssao`]).
    pub(crate) ssao: Option<crate::render::ssao::Ssao>,
    pub(crate) ssao_frame: Option<crate::render::ssao::Frame>,
    /// The Graphics page's screen-space reflections, and this frame's
    /// values for them ([`PostFx::set_ssr`]).
    pub(crate) ssr: Option<crate::render::ssr::Ssr>,
    pub(crate) ssr_frame: Option<crate::render::ssr::Frame>,
    /// The frame's glow sprites alone, for the reflections to see (scene
    /// size, the streak buffer's format), when they go to the streaks.
    pub(crate) ssr_glow: Option<((u32, u32), wgpu::TextureFormat, std::sync::Arc<crate::render::pool::Target>)>,
    /// The temporal pass (docs/temporal.md; clone deviation): its
    /// history, and with Upscaling the stage the scene is drawn into at
    /// the lower size (its own targets, motion blur, horizon fog, SSAO,
    /// SSR), resolved into this one's scene at the screen's size.
    pub temporal: Option<crate::render::temporal::Temporal>,
    pub(crate) stage: Option<Box<PostFx>>,
    /// Volumetric Fog (clone deviation) and this frame's values.
    pub(crate) volumetric_frame: Option<crate::render::volumetric::Frame>,
    /// Next-Gen car lights (docs/car-lights.md): the pass and this frame's.
    pub(crate) car_lights: Option<crate::render::car_lights::CarLightsPass>,
    pub(crate) car_lights_frame: Option<crate::render::car_lights::Frame>,
    /// The rain's tint for the Volumetric Fog's apply to fold in this frame ([`PostFx::fold_tint`]), and
    /// whether it did (the tint pass then does not run).
    pub(crate) fog_tint: Option<[f32; 3]>,
    pub(crate) tint_folded: bool,
    /// Volumetric Fog Next-Gen (docs/atmosphere.md).
    pub(crate) atmosphere: Option<crate::render::atmosphere::Atmosphere>,
    /// HDR Output (docs/hdr.md; clone deviation): the grade LUT (made on
    /// first use), the post passes' unclipped image for the display
    /// transform, and whether this frame's apply wrote it.
    pub(crate) grade: Option<GradeLut>,
    pub(crate) unclipped: Option<((u32, u32), wgpu::TextureView)>,
    pub(crate) unclipped_fresh: bool,
    /// docs/motionblur-dof.md; clone deviation: the motion blur and
    /// depth of field passes, made on first use.
    pub blur_dof: Option<crate::render::blur_dof::BlurDof>,
    /// A timestamp to write where the streak buffer's commit is made
    /// ([`PostFx::set_commit_stamp`]); taken when written.
    commit_stamp: Option<(wgpu::QuerySet, u32)>,
    /// Under SSAA, keep the passes after the rain at the scene's size and
    /// resample at the end, as before [`resolves_first`]
    /// (`NFSU2_SSAA_SCENE_SIZE=post`, for comparisons).
    pub(crate) scene_size_post: bool,
    /// High Precision Post's blur where it can be lit ([`glow_skip`]; made
    /// with its set, where the device can take it).
    glow_skip: Option<glow_skip::GlowSkip>,
}

/// The rain's part of a frame ([`PostFx::set_rain`]).
#[derive(Default)]
pub struct RainFrame {
    /// The tint pass's multiplier (0x5C7870 with slot 2), see
    /// [`crate::weather::tint_multiplier`].
    pub tint: Option<[f32; 3]>,
    /// The lens drops (0x5C7AC0).
    pub drops: Vec<crate::weather::LensDrop>,
}

impl PostFx {
    /// Dev harness (rate gate, `state_hash` `vis`): the overbright level 0x876848 as bits.
    #[cfg(feature = "dev-harness")]
    pub fn dev_level_bits(&self) -> u32 {
        self.level.0.to_bits()
    }

    /// `format` is the output's, and the scene's unless High Precision
    /// Post is on ([`PostFx::scene_format`]).
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> PostFx {
        let device = &gpu.device;
        let tex_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("postfx"),
            entries: &[
                tex_entry(0),
                tex_entry(1),
                tex_entry(2),
                tex_entry(5),
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(BOUNDS_SIZE),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<PassUniform>() as u64),
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("postfx"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("postfx.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                crate::render::core::shader::registry::text(crate::render::core::shader::registry::ShaderId::PostFx)
                    .into(),
            ),
        });
        // filter.efx's 4-tap box at ±0.5 texel only averages with bilinear
        // reads; the screen filter's samplers are taken to be the same.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        // Created initialised (zeros, mapped at creation), as the gain map
        // below is written at once (docs/perf-main-thread.md): the
        // scene's command buffers bind these uniforms (the motion blur) and
        // are submitted from a render thread while the main thread writes
        // the post passes' slots; wgpu-core 30 zero-fills a buffer's or
        // texture's uninitialised ranges at submission *after* the pending
        // writes it takes then, so a write landing in that window was
        // wiped (the first frame's post passes with zeroed slots: dimmed,
        // ~1 run in 40), and `write_texture` / `submit` take two locks in
        // opposite orders (a deadlock, ~1 run in 40). Initialised resources
        // take no part in either. The values the passes read are the same.
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("postfx"),
            size: SLOT * ALL_SLOTS,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        uniforms.unmap();
        let gain = texture(gpu, "gain map", (GAIN_SIZE as u32, GAIN_SIZE as u32), INTERMEDIATE);
        // Written once here, at creation (see the uniforms above): the
        // frame's own map is written over it by `apply` before the bleach
        // pass reads it.
        write_texture(gpu, &gain, &gain_map(0.0), GAIN_SIZE as u32, GAIN_SIZE as u32);
        let ramp = texture(gpu, "ramp map", (1, RAMP_SIZE as u32), INTERMEDIATE);
        write_texture(gpu, &ramp, &ramp_map(), 1, RAMP_SIZE as u32);
        let lut = texture(gpu, "unorm lut", (256, 1), INTERMEDIATE);
        write_texture(gpu, &lut, &(0..=255u8).flat_map(|k| [k; 4]).collect::<Vec<u8>>(), 256, 1);
        let bounds = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("unorm bounds"),
            size: BOUNDS_SIZE,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        bounds.unmap();
        let mut post = PostFx {
            format,
            layout,
            pipeline_layout,
            shader,
            sampler,
            uniforms,
            gain,
            gain_amount: None,
            ramp,
            level: OverbrightLevel::default(),
            tick_level: None,
            streak_keep: None,
            pipelines: Vec::new(),
            targets: None,
            fog: None,
            fog_ms: None,
            samples: 1,
            msaa: None,
            lut,
            bounds,
            fused: None,
            check: None,
            report: None,
            cache_file: check_cache_file(gpu),
            lens: None,
            rain: RainFrame::default(),
            enhance: None,
            output_size: None,
            last_output: None,
            enhanced: None,
            scaled_depth: None,
            ssao: None,
            ssao_frame: None,
            ssr: None,
            ssr_frame: None,
            temporal: None,
            stage: None,
            volumetric_frame: None,
            atmosphere: None,
            car_lights: None,
            car_lights_frame: None,
            fog_tint: None,
            tint_folded: false,
            ssr_glow: None,
            grade: None,
            unclipped: None,
            unclipped_fresh: false,
            blur_dof: None,
            commit_stamp: None,
            scene_size_post: crate::render::debug_env::ssaa_scene_size("post"),
            glow_skip: None,
        };
        post.warm(gpu);
        // The fused passes (R3): on once the self-check, run on the GPU now
        // and read a few frames later, finds them equal to the passes they
        // replace on this device; `NFSU2_NO_FUSED_POST=1` keeps them off,
        // `NFSU2_FUSED_POST=measured|even` forces a variant (for comparisons).
        match crate::render::debug_env::nfsu2_fused_post().as_deref() {
            Some("measured") => {
                post.fused = Some(1);
                post.report = Some("fused (forced, measured steps)".into());
            }
            Some("even") => {
                post.fused = Some(2);
                post.report = Some("fused (forced, to nearest)".into());
            }
            _ if crate::render::debug_env::nfsu2_no_fused_post() => {
                post.report = Some("separate passes (NFSU2_NO_FUSED_POST)".into())
            }
            // A verdict kept from an earlier run on this device, driver and
            // build (`NFSU2_POST_CHECK=1` runs the check again).
            _ if let Some(kept) = post
                .cache_file
                .as_ref()
                .filter(|_| !crate::render::debug_env::nfsu2_post_check_set())
                .and_then(|p| std::fs::read_to_string(p).ok()) =>
            {
                let (verdict, report) = kept.split_once('\n').unwrap_or((&kept, ""));
                post.fused = (verdict == "fused").then_some(1);
                post.report = Some(format!("{} (kept from an earlier run's self-check)", report.trim()));
                if post.fused.is_some() {
                    post.measure_bounds(gpu);
                }
            }
            _ => post.check = Some(post.start_check(gpu)),
        }
        if post.check.is_none() && post.fused == Some(1) {
            post.measure_bounds(gpu);
        }
        post
    }

    /// Runs the end-of-frame passes over the scene into `output` (in the
    /// scene's format). `frames` > 1 runs the streak buffer's per-frame
    /// decay and add that many times over the same image, as a still of a
    /// game that has been running shows. `glow` draws the frame's glow
    /// sprites into the streak buffer it is given, right after each decay
    /// (0x5CBB10); with the streaks off the caller draws them into the
    /// scene instead (0x5CB5BC). With `alpha_zero` the output's alpha is
    /// written 0 (its colour unchanged), for the UI's destination-alpha
    /// textures ([`crate::render::ui::Alpha::zeroed`]).
    #[allow(clippy::too_many_arguments)]
    /// The rain for the next [`PostFx::apply`] (taken by it); RAINDROPPC
    /// for the lens drops.
    pub fn set_rain(&mut self, gpu: &Gpu, rain: RainFrame, raindrop: Option<&crate::formats::texture::Texture>) {
        if !rain.drops.is_empty()
            && let Some(t) = raindrop
        {
            let format = self.format;
            // The 8-bit targets, and High Precision Post's half-float ones.
            self.lens
                .get_or_insert_with(|| {
                    crate::render::rain::LensPass::new(gpu, &[INTERMEDIATE, format, crate::render::HDR_FORMAT])
                })
                .set_texture(gpu, t);
        }
        self.rain = rain;
    }

    pub fn apply(
        &mut self,
        gpu: &Gpu,
        opts: &crate::render::RenderOptions,
        encoder: &mut wgpu::CommandEncoder,
        real_output: &wgpu::TextureView,
        switches: Switches,
        frame: Frame,
        frames: u32,
        alpha_zero: bool,
        glow: &mut dyn FnMut(&mut wgpu::CommandEncoder, &wgpu::TextureView, Option<&wgpu::TextureView>),
    ) {
        // The per-frame path over the tick form: `frames` commits of this
        // frame's glows, shown as committed.
        self.apply_ticks(
            gpu,
            opts,
            encoder,
            real_output,
            switches,
            frame,
            StreakFrame::per_frame(frames),
            alpha_zero,
            &mut |_, encoder, view, ms| glow(encoder, view, ms),
        );
    }

    /// [`PostFx::apply`] in the tick form (frame-rate independence): `streak` says how
    /// many ticks this frame commits into the streak buffer (each a decay
    /// and that tick's glows, [`StreakGlow::Commit`]) and whether the frame
    /// lies between ticks (the scratch B(α), [`StreakGlow::Frame`], shown
    /// and dropped). `glow` draws the named glow list.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_ticks(
        &mut self,
        gpu: &Gpu,
        opts: &crate::render::RenderOptions,
        encoder: &mut wgpu::CommandEncoder,
        real_output: &wgpu::TextureView,
        switches: Switches,
        frame: Frame,
        streak: StreakFrame,
        alpha_zero: bool,
        glow: &mut dyn FnMut(StreakGlow, &mut wgpu::CommandEncoder, &wgpu::TextureView, Option<&wgpu::TextureView>),
    ) {
        self.chain(gpu, opts, encoder, Some(real_output), switches, frame, streak, alpha_zero, glow);
    }

    /// One tick's commit into the streak buffer and nothing else
    /// (no output): the chain over this scene up to the streak pass, with
    /// the overbright pass's A from this scene and the level
    /// [`PostFx::set_tick_level`] gave, and the glows `glow` draws
    /// ([`StreakGlow::Commit`] 0). The game draws the scene of a tick no
    /// drawn frame stands on (a frame spanning several ticks, or one
    /// between ticks) and commits it here, so the buffer holds what the
    /// 60 Hz frames left (docs/uncapped-fps.md, "Streak buffer on ticks").
    #[allow(clippy::too_many_arguments)]
    pub fn commit_tick(
        &mut self,
        gpu: &Gpu,
        opts: &crate::render::RenderOptions,
        encoder: &mut wgpu::CommandEncoder,
        switches: Switches,
        frame: Frame,
        glow: &mut dyn FnMut(StreakGlow, &mut wgpu::CommandEncoder, &wgpu::TextureView, Option<&wgpu::TextureView>),
    ) {
        self.chain(gpu, opts, encoder, None, switches, frame, StreakFrame::tick(1, 0.0), false, glow);
    }

    /// The next chain writes timestamp `index` of `set` right after the
    /// streak buffer's commit — the end of what [`PostFx::commit_tick`]
    /// draws, also inside a full chain (the game's measure of what an
    /// offscreen commit costs: the passes after it, the output and the
    /// screen passes, are not in it). None: no timestamp.
    pub fn set_commit_stamp(&mut self, stamp: Option<(wgpu::QuerySet, u32)>) {
        self.commit_stamp = stamp;
    }

    /// Whether the timestamp given with [`PostFx::set_commit_stamp`] is
    /// still unwritten (the chain did not reach the commit); clears it.
    pub fn take_commit_stamp(&mut self) -> bool {
        self.commit_stamp.take().is_some()
    }

    fn write_commit_stamp(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let Some((set, index)) = self.commit_stamp.take() {
            encoder.write_timestamp(&set, index);
        }
    }

    /// [`PostFx::apply_ticks`]; without `real_output` it stops after the
    /// streak buffer's commits ([`PostFx::commit_tick`]).
    #[allow(clippy::too_many_arguments)]
    fn chain(
        &mut self,
        gpu: &Gpu,
        opts: &crate::render::RenderOptions,
        encoder: &mut wgpu::CommandEncoder,
        real_output: Option<&wgpu::TextureView>,
        switches: Switches,
        frame: Frame,
        streak: StreakFrame,
        alpha_zero: bool,
        glow: &mut dyn FnMut(StreakGlow, &mut wgpu::CommandEncoder, &wgpu::TextureView, Option<&wgpu::TextureView>),
    ) {
        self.poll_check(gpu);
        self.tint_folded = false;
        self.fog_tint = if real_output.is_some() { self.fold_tint(switches) } else { None };
        self.pre_passes(gpu, opts, encoder, switches.streaks(), &mut |e, v, m| glow(StreakGlow::Frame, e, v, m));
        self.fog_tint = None;
        let output_size = self.output_size.take();
        // The glows into the streak buffer: multisampled, through a
        // multisampled copy of it, resolved back (0x5CBBB2..0x5CBCE5).
        let ms_streak = self.msaa.as_ref().filter(|_| self.samples > 1).map(|m| m.streak.clone());
        let copy_streak = match &ms_streak {
            Some(_) => {
                let format = self.inter();
                Some(self.pipeline_ms(gpu, "fs_filter", format, Blend::Replace, self.samples))
            }
            None => None,
        };
        let Some((size, hdr)) = self.targets.as_ref().map(|t| (t.size, t.hdr)) else { return };
        // The screen's size (the Graphics page's SSAA / Render Scale draw
        // the scene at another): the glow is made at the size native makes
        // it for the screen (docs/post-scale.md). At 1× nothing changes.
        let out_size = output_size.unwrap_or(size);
        // HDR Output (docs/hdr.md): the float chain is kept unclipped to
        // the end, the grade through the captured LUT.
        let hdr_output = hdr.is_some() && opts.hdr_output_active;
        let resolved = !self.scene_size_post && resolves_first(size, out_size, hdr, hdr_output);
        let streaks = switches.streaks();
        let overbright = streaks && switches.overbright_pass();
        let mut glow = |this: &Self,
                        which: StreakGlow,
                        encoder: &mut wgpu::CommandEncoder,
                        streak: &wgpu::TextureView| match (&ms_streak, copy_streak) {
            (Some(ms), Some(p)) => {
                let t = this.targets.as_ref().unwrap();
                full_pass(
                    encoder,
                    "streaks into the multisampled copy",
                    &this.pipelines[p].1,
                    &[ms],
                    &t.groups[Src::Streak as usize],
                    COPY,
                    None,
                    None,
                );
                glow(which, encoder, ms, Some(streak));
            }
            _ => glow(which, encoder, streak, None),
        };
        let (scaled, inter, level) =
            self.prepare_targets(gpu, switches, &frame, streak.still, size, hdr, out_size, resolved);

        let (cv, out) = self.begin_chain(encoder);
        let cur = self.streaks(gpu, encoder, &cv, inter, hdr, streak, streaks, overbright, &mut glow);
        let rain = std::mem::take(&mut self.rain);
        // A commit alone (`commit_tick`): without the overbright pass the
        // commit is made; with it, its A comes from the image below.
        if !overbright {
            self.write_commit_stamp(encoder);
        }
        if real_output.is_none() && !overbright {
            return;
        }
        let cur = self.lens_drops(gpu, encoder, switches, &rain, &cv, inter, out, cur);
        self.rain_tint(gpu, encoder, switches, &rain, &cv, inter, out, cur);
        if resolved {
            // SSAA with High Precision Post: the image resolved to the
            // screen's size here; the bleach bypass, the glow's source and
            // the output at the screen's size (the streak buffer stays at
            // the scene's).
            let cur = self.resolve_bleach(gpu, encoder, switches, out_size, cur);
            self.overbright(gpu, encoder, &frame, level, hdr, false, streak, overbright, &cv, inter, cur, &mut glow);
            self.write_commit_stamp(encoder);
            let Some(real_output) = real_output else { return };
            let (plan, enhancing, final_view) = self.graphics_target(gpu, opts, out_size, out_size, real_output);
            let out = self.write_output_resolved(gpu, encoder, &final_view, overbright, alpha_zero, cur);
            if streaks && streak.scratch {
                self.streak_restore(encoder);
            }
            if enhancing {
                let e = self.enhance.get_or_insert_with(|| crate::render::enhance::Enhance::new(gpu, out));
                e.run(gpu, encoder, &final_view, out_size, real_output, out_size, plan);
            }
            return;
        }
        let cur = self.bleach_bypass(gpu, encoder, switches, &frame, hdr, hdr_output, &cv, inter, cur);
        self.overbright(gpu, encoder, &frame, level, hdr, scaled, streak, overbright, &cv, inter, cur, &mut glow);
        self.write_commit_stamp(encoder);
        let Some(real_output) = real_output else { return };
        let (plan, enhancing, final_view) = self.graphics_target(gpu, opts, size, out_size, real_output);
        let output = &final_view;
        let out = self.write_output(gpu, encoder, output, hdr_output, hdr, overbright, alpha_zero, size, cur);
        // A frame between ticks showed B(α): C_{n−1} back.
        if streaks && streak.scratch {
            self.streak_restore(encoder);
        }
        if enhancing {
            let format = out;
            let e = self.enhance.get_or_insert_with(|| crate::render::enhance::Enhance::new(gpu, format));
            e.run(gpu, encoder, &final_view, size, real_output, out_size, plan);
        }
    }

    /// [`PostFx::apply`]'s first stage: the glow targets at the output's size, the
    /// gain map, the overbright level and the passes' uniforms. -> (scaled, the
    /// intermediate format, this frame's level).
    #[allow(clippy::too_many_arguments)]
    fn prepare_targets(
        &mut self,
        gpu: &Gpu,
        switches: Switches,
        frame: &Frame,
        still: bool,
        size: (u32, u32),
        hdr: Option<u8>,
        out_size: (u32, u32),
        resolved: bool,
    ) -> (bool, wgpu::TextureFormat, OverbrightLevel) {
        self.last_output = Some(out_size);
        let glow_dims = glow_size(size, out_size, hdr);
        self.ensure_glow(gpu, glow_dims);
        let scaled = size != out_size;
        // Resolved first: the glow's source is the screen-size image, its
        // prefilter native's at 1× on it.
        let (prefilter_size, box_scaled) = if resolved { (out_size, false) } else { (size, scaled) };
        let half = if scaled { glow_dims } else { ((size.0 / 2).max(1), (size.1 / 2).max(1)) };
        let inter = self.inter();
        // The gain map follows the amount (0x5C68E0 rebuilds it when the
        // curves change, 0x5BBE90).
        if switches.bleach_bypass && self.gain_amount != Some(frame.bleach) {
            write_texture(gpu, &self.gain, &gain_map(frame.bleach), GAIN_SIZE as u32, GAIN_SIZE as u32);
            self.gain_amount = Some(frame.bleach);
        }
        // A still of a running game (`frames` > 1) has a settled level;
        // otherwise this frame uses the level the frames before it left.
        if still {
            self.level = OverbrightLevel(OverbrightLevel::target(frame.front_end));
        }
        let level = match self.tick_level.take() {
            // Tick form: the game's tick-stepped level, as drawn.
            Some(level) => level,
            None => self.level.frame(frame.front_end),
        };
        let offset = f32::from(level.offset()) / 255.0 * 0.5;
        let tint = {
            let [r, g, b] = frame.tint_color();
            [r, g, b, SATURATION_SCALE]
        };
        let taps = |taps: &[(f32, f32, f32)], (w, h): (u32, u32)| {
            let mut u = PassUniform {
                weights: [[0.0; 4]; 4],
                offsets: [[0.0; 4]; 4],
                params: [taps.len() as f32, frame.desaturate, offset, 0.0],
                grey: OVERBRIGHT_GREY,
                tint,
            };
            for (i, &(wt, du, dv)) in taps.iter().enumerate() {
                u.weights[i] = [wt; 4];
                u.offsets[i] = [du / w as f32, dv / h as f32, 0.0, 0.0];
            }
            u
        };
        let mut data = vec![0u8; (SLOT * SLOTS) as usize];
        let mut put = |slot: u32, u: PassUniform| {
            let o = slot as usize * SLOT as usize;
            data[o..o + std::mem::size_of::<PassUniform>()].copy_from_slice(bytemuck::bytes_of(&u));
        };
        put(COPY, taps(&[(1.0, 0.0, 0.0)], size));
        put(OVERBRIGHT, taps(&[(1.0, 0.0, 0.0)], half));
        put(ACROSS, taps(&BLUR_ACROSS, half));
        put(DOWN, taps(&BLUR_DOWN, half));
        put(BLEACH, taps(&[(1.0, 0.0, 0.0)], size));
        // High Precision Post: the glow at `glow_size`, a 3×3 tent at the
        // screen's size (one tap at half size, as native), blurred once
        // across and once down (σ in its texels, the subtraction's factor
        // in tint.x).
        if let Some(scale) = hdr {
            let sub = if frame.front_end { SUBTRACT_FRONT_END } else { SUBTRACT_GAME } as f32;
            let sigma = if scale == 2 { HDR_SIGMA_FULL } else { HDR_SIGMA_HALF };
            let blur = |dx: f32, dy: f32| PassUniform {
                weights: [[0.0; 4]; 4],
                offsets: [[dx / glow_dims.0 as f32, dy / glow_dims.1 as f32, 0.0, 0.0], [0.0; 4], [0.0; 4], [0.0; 4]],
                params: [0.0, frame.desaturate, offset, sigma],
                grey: OVERBRIGHT_GREY,
                tint: [sub, 0.0, 0.0, 0.0],
            };
            put(ACROSS, blur(1.0, 0.0));
            put(DOWN, blur(0.0, 1.0));
            let prefilter = if scale == 2 {
                taps(&[(0.25, -0.5, -0.5), (0.25, 0.5, -0.5), (0.25, -0.5, 0.5), (0.25, 0.5, 0.5)], prefilter_size)
            } else {
                taps(&[(1.0, 0.0, 0.0)], glow_dims)
            };
            if !box_scaled {
                gpu.queue.write_buffer(&self.uniforms, u64::from(PREFILTER) * SLOT, bytemuck::bytes_of(&prefilter));
            }
        }
        // The resolve: the bleach bypass's values and the scene's texels
        // per screen pixel.
        if resolved {
            let mut u = taps(&[(1.0, 0.0, 0.0)], size);
            u.offsets[1] = [
                size.0 as f32 / out_size.0.max(1) as f32,
                size.1 as f32 / out_size.1.max(1) as f32,
                size.0 as f32,
                size.1 as f32,
            ];
            gpu.queue.write_buffer(&self.uniforms, u64::from(RESOLVE) * SLOT, bytemuck::bytes_of(&u));
        }
        // Scaled: the glow's C is the box of the scene under each glow
        // texel (`fs_box`: params.x taps a side, offsets[0] apart).
        if box_scaled {
            let width = if hdr == Some(2) { HDR_FULL_BOX } else { 1.0 };
            let (n, step) = box_prefilter(size, glow_dims, width);
            let mut u = taps(&[(1.0, 0.0, 0.0)], glow_dims);
            u.params[0] = n as f32;
            u.offsets[0] = [step[0], step[1], 0.0, 0.0];
            gpu.queue.write_buffer(&self.uniforms, u64::from(PREFILTER) * SLOT, bytemuck::bytes_of(&u));
        }
        gpu.queue.write_buffer(&self.uniforms, 0, &data);
        (box_scaled, inter, level)
    }

    /// The image so far (`cur`, the scene's size) resolved to the screen's
    /// (`out_size`): each scene texel graded (the bleach bypass when on, as
    /// `fs_bleach_hdr`), each screen pixel the box of those under it
    /// (`fs_resolve`). -> the resolved image.
    fn resolve_bleach(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        switches: Switches,
        out_size: (u32, u32),
        cur: Src,
    ) -> Src {
        let view = self.ensure_resolved(gpu, out_size);
        let variant = resolve_variant(switches.bleach_bypass);
        let p = self.pipeline_v(gpu, "fs_resolve", crate::render::HDR_FORMAT, 1, Blend::Replace, variant);
        let t = self.targets.as_ref().unwrap();
        let (pipeline, group) = (&self.pipelines[p].1, &t.groups[cur as usize]);
        full_pass_op(encoder, "fs_resolve", pipeline, &[&view], group, RESOLVE, None, replace_load());
        crate::prof::gpu::mark(encoder, "post: resolve and bleach bypass");
        Src::Resolved
    }

    /// The last pass of a resolved chain (`cur` at the screen's size) to
    /// `output`: the image, plus the streak buffer's box under each pixel
    /// when the overbright pass ran, back to 8 bits as High Precision
    /// Post's `fs_out` / `fs_out_add`. -> the output's format.
    fn write_output_resolved(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        overbright: bool,
        alpha_zero: bool,
        cur: Src,
    ) -> wgpu::TextureFormat {
        let out = self.format;
        let (entry, src, slot) = match (overbright, alpha_zero) {
            (true, false) => ("fs_out_add_resolved", cur.with_streak(), RESOLVE),
            (true, true) => ("fs_out_add_resolved_a0", cur.with_streak(), RESOLVE),
            (false, false) => ("fs_out", cur, COPY),
            (false, true) => ("fs_out_a0", cur, COPY),
        };
        self.draw(gpu, encoder, output, out, entry, Blend::Replace, src, slot, None, None);
        crate::prof::gpu::mark(encoder, "post: to the back buffer");
        out
    }

    /// The chain's views ([`ChainViews`]) and the scene's format; clears the
    /// streak buffer the first time after the targets were made.
    fn begin_chain(&mut self, encoder: &mut wgpu::CommandEncoder) -> (ChainViews, wgpu::TextureFormat) {
        let t = self.targets.as_mut().unwrap();
        let v = &t.views;
        let (full0, full1) = (v.full[0].clone(), v.full[1].clone());
        let (a, b, c) = (v.half[0].clone(), v.half[1].clone(), v.half[2].clone());
        let streak = v.streak.clone();
        let clear_streak = !t.fresh;
        t.fresh = true;
        let out = self.scene_format();
        if clear_streak {
            clear(encoder, &streak, wgpu::Color::TRANSPARENT);
        }
        (ChainViews { full0, full1, a, b, c, streak }, out)
    }

    /// The streak buffer without the overbright pass: decay, glows, added onto a
    /// copy of the scene. -> the image so far.
    #[allow(clippy::too_many_arguments)]
    fn streaks(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        cv: &ChainViews,
        inter: wgpu::TextureFormat,
        hdr: Option<u8>,
        streak: StreakFrame,
        streaks: bool,
        overbright: bool,
        glow: &mut impl FnMut(&Self, StreakGlow, &mut wgpu::CommandEncoder, &wgpu::TextureView),
    ) -> Src {
        // 0x5CBB10 in the view loop: the decay, then (without overbright)
        // the buffer added straight onto the back buffer.
        let mut cur = Src::Scene;
        if streaks && !overbright {
            // Saturated as the 8-bit buffer's writes are (High Precision).
            let add = StreakAdd { overbright: None, saturate: hdr.is_some() };
            self.streak_frame(gpu, encoder, &cv.streak, inter, add, streak, glow);
            // A copy of the scene with the buffer added ONE / ONE, in one
            // pass.
            self.draw(gpu, encoder, &cv.full1, inter, "fs_add", Blend::Replace, Src::SceneStreak, COPY, None, None);
            cur = Src::Full1;
            crate::prof::gpu::mark(encoder, "post: streaks");
        }
        cur
    }

    /// The rain's lens drops. -> the image so far.
    #[allow(clippy::too_many_arguments)]
    fn lens_drops(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        switches: Switches,
        rain: &RainFrame,
        cv: &ChainViews,
        inter: wgpu::TextureFormat,
        out: wgpu::TextureFormat,
        mut cur: Src,
    ) -> Src {
        // 0x5C7AC0 then 0x5C7870: the lens drops over a copy of the frame
        // so far (into the screen copy the bleach does not read from as it
        // writes), then the rain's tint (dest × 2·colour).
        if switches.rain_splatter && !rain.drops.is_empty() && self.lens.is_some() {
            let scene = self.targets.as_ref().unwrap().views.scene.clone();
            // High Precision Post: the scene and the screen copy share the half-float format, so the
            // copy is the frame texel for texel. Only the texels the drops sample are copied (into the
            // screen copy, each at its place) and the drops are drawn onto the scene itself, sampling
            // them: the same drops over the same frame, without copying the rest of it.
            if matches!(cur, Src::Scene)
                && inter == out
                && self.hdr()
                && lens_drops::in_place_allowed()
                && self.lens.as_ref().unwrap().copy_sources(gpu, encoder, &cv.full1, inter, &scene, &rain.drops)
            {
                self.lens.as_ref().unwrap().draw(gpu, encoder, &scene, out, &cv.full1, &rain.drops);
                lens_drops::count_in_place();
                crate::prof::gpu::mark(encoder, "post: lens drops");
                return cur;
            }
            let (target, format, next) =
                if matches!(cur, Src::Scene) { (&cv.full1, inter, Src::Full1) } else { (&scene, out, Src::Scene) };
            let source = if matches!(cur, Src::Scene) { scene.clone() } else { cv.full1.clone() };
            self.draw(gpu, encoder, target, format, "fs_filter", Blend::Replace, cur, COPY, None, None);
            self.lens.as_ref().unwrap().draw(gpu, encoder, target, format, &source, &rain.drops);
            cur = next;
            crate::prof::gpu::mark(encoder, "post: lens drops");
        }
        cur
    }

    /// The rain's tint pass kept (true) where the Volumetric Fog's apply could fold it in
    /// ([`PostFx::fold_tint`]): for the test that compares the two.
    pub fn set_plain_tint(on: bool) {
        PLAIN_TINT.store(on, std::sync::atomic::Ordering::Relaxed);
    }

    /// The frames whose rain tint the Volumetric Fog's apply folded in so far (the comparison test's check).
    pub fn tint_folds() -> u64 {
        TINT_FOLDS.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// The rain's tint for the Volumetric Fog's apply to fold in (its pass then multiplies the scene by it
    /// through the blend, and the light it adds): where the tint is the next thing to touch the scene after
    /// the medium (no lens drops, no car lights' lens streaks between, the image still the scene, i.e. no
    /// streak copy before it), in High Precision Post's float scene, every channel at most 1 (one blend). The
    /// same pixels up to the half float's rounding of the one product instead of the two steps'.
    fn fold_tint(&self, switches: Switches) -> Option<[f32; 3]> {
        let tint = self.rain.tint.filter(|t| t.iter().all(|&m| (0.0..=1.0).contains(&m)))?;
        let drops = switches.rain_splatter && !self.rain.drops.is_empty() && self.lens.is_some();
        let streak_copy = switches.streaks() && !switches.overbright_pass();
        (!PLAIN_TINT.load(std::sync::atomic::Ordering::Relaxed)
            && switches.tinting
            && !drops
            && !streak_copy
            && self.car_lights_frame.is_none()
            && self.volumetric_frame.is_some()
            && self.hdr())
        .then_some(tint)
    }

    /// The rain's tint, in place.
    #[allow(clippy::too_many_arguments)]
    fn rain_tint(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        switches: Switches,
        rain: &RainFrame,
        cv: &ChainViews,
        inter: wgpu::TextureFormat,
        out: wgpu::TextureFormat,
        cur: Src,
    ) {
        if self.tint_folded {
            self.tint_folded = false;
            TINT_FOLDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return;
        }
        if switches.tinting
            && let Some([r, g, b]) = rain.tint
        {
            let scene = self.targets.as_ref().unwrap().views.scene.clone();
            let (target, format) = match cur {
                Src::Scene => (&scene, out),
                Src::Full0 => (&cv.full0, inter),
                _ => (&cv.full1, inter),
            };
            // 0x5C7870's DESTCOLOR / SRCCOLOR quad: dest × m, m = 2·c/255 up
            // to 2 (the NIS SCREEN_FLASH: 1.99). A blend constant is clamped
            // to 1, so the part above 1 is a second pass, dest × (1 + m − 1).
            let low = [r, g, b].map(|m| f64::from(m.min(1.0)));
            self.draw(
                gpu,
                encoder,
                target,
                format,
                "fs_zero",
                Blend::ScaleDest,
                Src::A,
                COPY,
                Some(wgpu::Color { r: low[0], g: low[1], b: low[2], a: 1.0 }),
                None,
            );
            if r > 1.0 || g > 1.0 || b > 1.0 {
                let high = [r, g, b].map(|m| f64::from((m - 1.0).max(0.0)));
                self.draw(
                    gpu,
                    encoder,
                    target,
                    format,
                    "fs_one",
                    Blend::AddScaledDest,
                    Src::A,
                    COPY,
                    Some(wgpu::Color { r: high[0], g: high[1], b: high[2], a: 0.0 }),
                    None,
                );
            }
            crate::prof::gpu::mark(encoder, "post: rain tint");
        }
    }

    /// The bleach bypass (or HDR Output's grade LUT). -> the image so far.
    #[allow(clippy::too_many_arguments)]
    fn bleach_bypass(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        switches: Switches,
        frame: &Frame,
        hdr: Option<u8>,
        hdr_output: bool,
        cv: &ChainViews,
        inter: wgpu::TextureFormat,
        mut cur: Src,
    ) -> Src {
        // 0x5C68E0: screenfilter.efx technique 0's three passes, ping-pong
        // between the two screen copies.
        if switches.bleach_bypass && hdr_output {
            let gain_view = self.gain.create_view(&Default::default());
            let strip = self.record_lut(gpu, encoder, frame, &gain_view);
            let p = self.pipeline(gpu, "fs_grade_lut", inter, Blend::Replace);
            let group = self.group_of(gpu, &self.src_view(cur), &gain_view, &strip, &self.uniforms);
            full_pass_op(
                encoder,
                "fs_grade_lut",
                &self.pipelines[p].1,
                &[&cv.full0],
                &group,
                BLEACH,
                None,
                replace_load(),
            );
            cur = Src::Full0;
            crate::prof::gpu::mark(encoder, "post: grade lut (hdr output)");
        } else if switches.bleach_bypass && hdr.is_some() {
            // High Precision Post: the three in one pass in float, each
            // 8-bit hand-off's saturation kept, its rounding not.
            self.draw(gpu, encoder, &cv.full0, inter, "fs_bleach_hdr", Blend::Replace, cur, BLEACH, None, None);
            cur = Src::Full0;
            crate::prof::gpu::mark(encoder, "post: bleach bypass");
        } else if switches.bleach_bypass
            && let Some(v) = self.fused
        {
            // The three in one pass (the self-check found them equal).
            self.draw_fused(gpu, encoder, &[&cv.full0], "fs_bleach_fused", v, cur, BLEACH);
            cur = Src::Full0;
            crate::prof::gpu::mark(encoder, "post: bleach bypass");
        } else if switches.bleach_bypass {
            self.draw(gpu, encoder, &cv.full0, INTERMEDIATE, "fs_bleach0", Blend::Replace, cur, BLEACH, None, None);
            self.draw(gpu, encoder, &cv.full1, INTERMEDIATE, "fs_bleach1", Blend::Replace, Src::Full0, BLEACH, None, None);
            self.draw(gpu, encoder, &cv.full0, INTERMEDIATE, "fs_bleach2", Blend::Replace, Src::Full1, BLEACH, None, None);
            cur = Src::Full0;
            crate::prof::gpu::mark(encoder, "post: bleach bypass");
        }
        cur
    }

    /// The overbright pass and the streak buffer it feeds.
    #[allow(clippy::too_many_arguments)]
    fn overbright(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        frame: &Frame,
        level: OverbrightLevel,
        hdr: Option<u8>,
        scaled: bool,
        streak: StreakFrame,
        overbright: bool,
        cv: &ChainViews,
        inter: wgpu::TextureFormat,
        cur: Src,
        glow: &mut impl FnMut(&Self, StreakGlow, &mut wgpu::CommandEncoder, &wgpu::TextureView),
    ) {
        if overbright && hdr.is_some() {
            self.overbright_high_precision(gpu, encoder, frame, level, scaled, streak, cv, inter, cur, glow);
        } else if overbright {
            self.overbright_half_size(gpu, encoder, frame, level, scaled, streak, cv, cur, glow);
        }
    }

    /// High Precision Post's overbright pass and streak buffer.
    #[allow(clippy::too_many_arguments)]
    fn overbright_high_precision(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        frame: &Frame,
        level: OverbrightLevel,
        scaled: bool,
        streak: StreakFrame,
        cv: &ChainViews,
        inter: wgpu::TextureFormat,
        cur: Src,
        glow: &mut impl FnMut(&Self, StreakGlow, &mut wgpu::CommandEncoder, &wgpu::TextureView),
    ) {
        // High Precision Post: C and A at the glow's size (the
        // prefilter), A blurred across, then down with A − C · factor
        // (held at 0, as the 8-bit target is) in the same pass; where the
        // device can, the blur only where it can be lit ([`glow_skip`]).
        if let Some(mut skip) = self.glow_skip.take() {
            if scaled {
                self.draw(gpu, encoder, &cv.c, inter, "fs_box_hdr", Blend::Replace, cur, PREFILTER, None, None);
            }
            let hdr = self.targets.as_ref().and_then(|t| t.hdr).unwrap_or(1);
            skip.run(gpu, encoder, self, cv, cur, scaled, hdr);
            self.glow_skip = Some(skip);
        } else {
            self.overbright_blur(gpu, encoder, scaled, cv, inter, cur);
        }
        crate::prof::gpu::mark(encoder, "post: overbright (high precision)");
        let add = if frame.front_end { ADD_FRONT_END } else { f64::from(level.add()) / 255.0 };
        let add = StreakAdd { overbright: Some(add), saturate: true };
        self.streak_frame(gpu, encoder, &cv.streak, inter, add, streak, glow);
        crate::prof::gpu::mark(encoder, "post: streaks");
    }

    /// High Precision Post's prefilter and blur over every texel.
    fn overbright_blur(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        scaled: bool,
        cv: &ChainViews,
        inter: wgpu::TextureFormat,
        cur: Src,
    ) {
        if scaled {
            self.draw(gpu, encoder, &cv.c, inter, "fs_box_hdr", Blend::Replace, cur, PREFILTER, None, None);
            self.draw(gpu, encoder, &cv.a, inter, "fs_overbright_sat", Blend::Replace, Src::C, OVERBRIGHT, None, None);
        } else {
            let p = self.pipeline_v(gpu, "fs_overbright_mrt_hdr", inter, 2, Blend::Replace, 0);
            let t = self.targets.as_ref().unwrap();
            full_pass_op(
                encoder,
                "fs_overbright_mrt_hdr",
                &self.pipelines[p].1,
                &[&cv.c, &cv.a],
                &t.groups[cur as usize],
                PREFILTER,
                None,
                replace_load(),
            );
        }
        self.draw(gpu, encoder, &cv.b, inter, "fs_blur_hdr", Blend::Replace, Src::A, ACROSS, None, None);
        self.draw(gpu, encoder, &cv.a, inter, "fs_blur_sub_hdr", Blend::Replace, Src::BWithC, DOWN, None, None);
    }

    /// The native overbright pass (0x5CCA10) and streak buffer.
    #[allow(clippy::too_many_arguments)]
    fn overbright_half_size(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        frame: &Frame,
        level: OverbrightLevel,
        scaled: bool,
        streak: StreakFrame,
        cv: &ChainViews,
        cur: Src,
        glow: &mut impl FnMut(&Self, StreakGlow, &mut wgpu::CommandEncoder, &wgpu::TextureView),
    ) {
        // 0x5CCA10: C = the image at half size (one bilinear tap),
        // A = overbright(C), blurred twice across and down.
        match self.fused {
            // Scaled: C the box under each glow texel, then A.
            _ if scaled => {
                self.draw(gpu, encoder, &cv.c, INTERMEDIATE, "fs_box", Blend::Replace, cur, PREFILTER, None, None);
                self.draw(
                    gpu,
                    encoder,
                    &cv.a,
                    INTERMEDIATE,
                    "fs_overbright",
                    Blend::Replace,
                    Src::C,
                    OVERBRIGHT,
                    None,
                    None,
                );
            }
            // C and A in one pass (MRT).
            Some(v) => self.draw_fused(gpu, encoder, &[&cv.c, &cv.a], "fs_overbright_mrt", v, cur, COPY),
            None => {
                self.draw(gpu, encoder, &cv.c, INTERMEDIATE, "fs_filter", Blend::Replace, cur, COPY, None, None);
                self.draw(
                    gpu,
                    encoder,
                    &cv.a,
                    INTERMEDIATE,
                    "fs_overbright",
                    Blend::Replace,
                    Src::C,
                    OVERBRIGHT,
                    None,
                    None,
                );
            }
        }
        for _ in 0..BLUR_ROUNDS {
            self.draw(gpu, encoder, &cv.b, INTERMEDIATE, "fs_filter", Blend::Replace, Src::A, ACROSS, None, None);
            self.draw(gpu, encoder, &cv.a, INTERMEDIATE, "fs_filter", Blend::Replace, Src::B, DOWN, None, None);
        }
        // A −= C · factor (BLENDOP_REVSUBTRACT).
        let sub = if frame.front_end { SUBTRACT_FRONT_END } else { SUBTRACT_GAME };
        self.draw(
            gpu,
            encoder,
            &cv.a,
            INTERMEDIATE,
            "fs_filter",
            Blend::SubtractConstant,
            Src::C,
            COPY,
            Some(wgpu::Color { r: sub, g: sub, b: sub, a: sub }),
            None,
        );
        crate::prof::gpu::mark(encoder, "post: overbright (half size)");
        // The streak buffer: decayed once per frame (0x5CBB10), then
        // A · factor added (0x5CCB9A).
        let add = if frame.front_end { ADD_FRONT_END } else { f64::from(level.add()) / 255.0 };
        let add = StreakAdd { overbright: Some(add), saturate: false };
        self.streak_frame(gpu, encoder, &cv.streak, INTERMEDIATE, add, streak, glow);
        crate::prof::gpu::mark(encoder, "post: streaks");
    }

    /// The image the last pass writes: the output, or one at the scene's size
    /// for the Graphics page's passes. -> (their plan, whether they run, it).
    fn graphics_target(
        &mut self,
        gpu: &Gpu,
        opts: &crate::render::RenderOptions,
        size: (u32, u32),
        out_size: (u32, u32),
        real_output: &wgpu::TextureView,
    ) -> (crate::render::enhance::Plan, bool, wgpu::TextureView) {
        // The Graphics page's passes (clone deviations): the last pass
        // goes to an image of the scene's size, which they take to the
        // output. Native: straight to the output, as before.
        let plan = crate::render::enhance::Plan::of(opts);
        let enhancing = plan.any(size, out_size);
        let final_view = if enhancing {
            if self.enhanced.as_ref().is_none_or(|(s, _)| *s != size) {
                let t = gpu.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("before the graphics passes"),
                    size: wgpu::Extent3d { width: size.0.max(1), height: size.1.max(1), depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: self.format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                self.enhanced = Some((size, t.create_view(&Default::default())));
            }
            self.enhanced.as_ref().unwrap().1.clone()
        } else {
            // No Graphics page pass, no intermediate kept.
            self.enhanced = None;
            real_output.clone()
        };
        (plan, enhancing, final_view)
    }

    /// The last pass, to `output` (plus the streak buffer when the overbright
    /// pass ran). -> the output's format.
    #[allow(clippy::too_many_arguments)]
    fn write_output(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        hdr_output: bool,
        hdr: Option<u8>,
        overbright: bool,
        alpha_zero: bool,
        size: (u32, u32),
        cur: Src,
    ) -> wgpu::TextureFormat {
        // To the back buffer, plus the streak buffer (ONE / ONE) when the
        // overbright pass ran: the two in one pass.
        let out = self.format;
        if hdr_output {
            // HDR Output: the SDR image as below and the unclipped one.
            let entry = match (overbright, alpha_zero) {
                (true, false) => "fs_out_add_hdr",
                (true, true) => "fs_out_add_hdr_a0",
                (false, false) => "fs_out_hdr",
                (false, true) => "fs_out_hdr_a0",
            };
            if self.unclipped.as_ref().is_none_or(|(s, _)| *s != size) {
                let t = texture(gpu, "unclipped (hdr output)", size, crate::render::HDR_FORMAT);
                self.unclipped = Some((size, t.create_view(&Default::default())));
            }
            let unclipped = self.unclipped.as_ref().unwrap().1.clone();
            let p = self.pipeline_hdr_out(gpu, entry);
            let src = if overbright { cur.with_streak() } else { cur };
            let t = self.targets.as_ref().unwrap();
            full_pass_op(
                encoder,
                entry,
                &self.pipelines[p].1,
                &[output, &unclipped],
                &t.groups[src as usize],
                COPY,
                None,
                replace_load(),
            );
            self.unclipped_fresh = true;
        } else if hdr.is_some() {
            // High Precision Post: back to 8 bits, saturated as the native
            // image is, with a ±1-step dither in place of the banding.
            let entry = match (overbright, alpha_zero) {
                (true, false) => "fs_out_add",
                (true, true) => "fs_out_add_a0",
                (false, false) => "fs_out",
                (false, true) => "fs_out_a0",
            };
            self.draw(
                gpu,
                encoder,
                output,
                out,
                entry,
                Blend::Replace,
                if overbright { cur.with_streak() } else { cur },
                COPY,
                None,
                None,
            );
        } else if overbright {
            self.draw(
                gpu,
                encoder,
                output,
                out,
                if alpha_zero { "fs_add_a0" } else { "fs_add" },
                Blend::Replace,
                cur.with_streak(),
                COPY,
                None,
                None,
            );
        } else {
            self.draw(
                gpu,
                encoder,
                output,
                out,
                if alpha_zero { "fs_filter_a0" } else { "fs_filter" },
                Blend::Replace,
                cur,
                COPY,
                None,
                None,
            );
        }
        crate::prof::gpu::mark(encoder, "post: to the back buffer");
        out
    }
}

/// Whether a chain resolves the scene (`size`) to the screen's size
/// (`out_size`) before the bleach bypass (docs/ssaa-screen-passes.md): SSAA
/// (a scene larger than the screen both ways) with High Precision Post
/// (`hdr`), not HDR Output. Everything else, and every frame without SSAA,
/// runs as before.
pub(super) fn resolves_first(size: (u32, u32), out_size: (u32, u32), hdr: Option<u8>, hdr_output: bool) -> bool {
    hdr.is_some() && !hdr_output && size.0 > out_size.0 && size.1 > out_size.1
}

/// `fs_resolve`'s pipeline variant: bit 0 the bleach bypass (never 0, the
/// variant without constants).
pub(super) fn resolve_variant(bleach: bool) -> u8 {
    2 | u8::from(bleach)
}

static PLAIN_TINT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static TINT_FOLDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// [`PostFx::apply`]'s screen copies, half-size targets and streak buffer.
struct ChainViews {
    full0: wgpu::TextureView,
    full1: wgpu::TextureView,
    a: wgpu::TextureView,
    b: wgpu::TextureView,
    c: wgpu::TextureView,
    streak: wgpu::TextureView,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The grade LUT's lattice (HDR Output, docs/hdr.md): 32 points on
    /// 8-bit codes from 0 to 255, so the LUT is the native grade there.
    #[test]
    fn grade_lattice_sits_on_8bit_codes() {
        let codes: Vec<u8> = (0..LUT_N).map(lattice_code).collect();
        assert_eq!((codes[0], codes[1], codes[2], codes[31]), (0, 8, 16, 255));
        assert!(codes.windows(2).all(|w| w[1] > w[0] && w[1] - w[0] <= 9));
        let strip = identity_strip();
        assert_eq!(strip.len(), (LUT_N * LUT_N * LUT_N * 4) as usize);
        // Texel (r + 32·b, g).
        let at = |r: u32, g: u32, b: u32| &strip[((g * LUT_N * LUT_N + b * LUT_N + r) * 4) as usize..][..4];
        assert_eq!(at(31, 1, 2), [255, 8, 16, 255]);
    }

    /// The LUT against the ported passes on the device: equal on the
    /// lattice, within ±3 steps (98 % within ±1) on every 24-bit colour
    /// of a sample (the private developer probe `grade_lut verify` does all 2^24).
    #[test]
    #[ignore = "requires a GPU; run explicitly"]
    fn grade_lut_matches_the_native_passes() {
        let gpu = pollster::block_on(Gpu::new(None, wgpu::Instance::default())).unwrap();
        let mut post = PostFx::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
        for frame in [
            Frame::FRONT_END,
            Frame::GAME,
            Frame { front_end: false, bleach: 0.76, desaturate: 0.12, tint: [0.873, 0.729, 0.435] },
        ] {
            let lut = post.grade_lut_texels(&gpu, frame);
            let lattice =
                post.grade_image(&gpu, frame, None, &identity_strip(), (LUT_N * LUT_N, LUT_N), GradeMode::Native);
            assert_eq!(lut, lattice, "{frame:?}");
            let (w, h) = (1024u32, 256u32);
            let src: Vec<u8> = (0..w * h)
                .flat_map(|i| {
                    let v = i.wrapping_mul(2_654_435_761);
                    [(v & 255) as u8, ((v >> 8) & 255) as u8, ((v >> 16) & 255) as u8, 255]
                })
                .collect();
            let native = post.grade_image(&gpu, frame, None, &src, (w, h), GradeMode::Native);
            let graded = post.grade_image(&gpu, frame, None, &src, (w, h), GradeMode::LutTrilinear);
            let diffs: Vec<u8> = native
                .chunks_exact(4)
                .zip(graded.chunks_exact(4))
                .map(|(a, b)| (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap())
                .collect();
            let max = *diffs.iter().max().unwrap();
            let within1 = diffs.iter().filter(|&&d| d <= 1).count() as f64 / diffs.len() as f64;
            assert!(max <= 3 && within1 > 0.98, "{frame:?}: max {max}, within ±1 {within1}");
        }
    }

    #[test]
    #[ignore = "requires a GPU; run explicitly for AA target lifecycle"]
    fn aa_targets_release_on_disable_and_resize() {
        let gpu = pollster::block_on(Gpu::new(None, wgpu::Instance::default())).unwrap();
        let mut post = PostFx::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
        let opts = crate::render::RenderOptions::in_force();
        assert!(post.multisampled(&gpu, (64, 64), 4, &opts).is_some());
        post.multisampled(&gpu, (64, 64), 1, &opts);
        assert!(post.msaa.is_none(), "disabled MSAA must release its attachments");
        post.multisampled(&gpu, (64, 64), 4, &opts);
        post.scaled_depth(&gpu, (64, 64));
        post.scene_view(&gpu, (32, 32), &opts);
        assert!(post.msaa.is_none(), "resize must release old MSAA before new scene allocation");
        assert!(post.scaled_depth.is_none(), "resize must release old scaled depth");
        assert!(post.multisampled(&gpu, (32, 32), 4, &opts).is_some());
    }

    /// The multisampled targets are made again when what they are made
    /// from changes (the sample count, the size) and kept otherwise: a
    /// Particles tier change (the Cinematic smoke reads the scene) keeps
    /// the same textures.
    #[test]
    #[ignore = "requires a GPU; run explicitly for AA target lifecycle"]
    fn msaa_targets_rebuild_only_for_their_inputs() {
        let gpu = pollster::block_on(Gpu::new(None, wgpu::Instance::default())).unwrap();
        let mut post = PostFx::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
        // The native options (no global read: only the Particles tier moves).
        let mut opts = crate::render::RenderOptions::default();
        let first = post.multisampled(&gpu, (64, 64), 4, &opts).unwrap().0;
        for tier in [crate::render::smoke_cinematic::TIER, 1, 0] {
            opts.particles = tier;
            let now = post.multisampled(&gpu, (64, 64), 4, &opts).unwrap().0;
            assert!(now == first, "particles tier {tier}: the targets were made again");
        }
        let more = post.multisampled(&gpu, (64, 64), 2, &opts).unwrap().0;
        assert!(more != first, "a new sample count keeps the old targets");
        let resized = post.multisampled(&gpu, (64, 48), 4, &opts).unwrap().0;
        assert!(resized != first && resized != more, "a new size keeps the old targets");
        let again = post.multisampled(&gpu, (64, 48), 4, &opts).unwrap().0;
        assert!(again == resized, "unchanged inputs: the targets were made again");
    }

    /// High Precision Post's targets and passes on the device: every pass
    /// of `apply` (streaks, bleach, overbright, both glow sizes, alpha 0)
    /// without a validation error, the half-float formats while on, and
    /// the native 8-bit ones back once off.
    #[test]
    #[ignore = "requires a GPU; run explicitly for High Precision Post's pipelines"]
    fn hdr_passes_build_and_run() {
        let _globals = crate::qol::test_globals();
        let gpu = pollster::block_on(Gpu::new(None, wgpu::Instance::default())).unwrap();
        let mut post = PostFx::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
        let out = texture(&gpu, "out", (64, 48), wgpu::TextureFormat::Rgba8Unorm).create_view(&Default::default());
        for (hdr, samples, alpha_zero) in [(2u8, 1u32, false), (1, 4, true), (2, 4, false), (0, 1, false)] {
            crate::qol::Enhancements { hdr_post: hdr, ..Default::default() }.apply();
            let opts = crate::render::RenderOptions::in_force();
            let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
            post.scene_view(&gpu, (64, 48), &opts);
            post.multisampled(&gpu, (64, 48), samples, &opts);
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            post.motion_blur(
                &gpu,
                &mut encoder,
                crate::motion_blur::Blur { shift: [0.01, 0.0, 0.01, 0.0], factor: 128 },
            );
            post.apply(
                &gpu,
                &opts,
                &mut encoder,
                &out,
                Switches::from_detail(5),
                Frame::GAME,
                3,
                alpha_zero,
                &mut |_, _, _| {},
            );
            gpu.queue.submit([encoder.finish()]);
            let error = pollster::block_on(scope.pop());
            assert!(error.is_none(), "hdr {hdr} samples {samples}: {error:?}");
            let want = if hdr != 0 && crate::qol::hdr_active() {
                crate::render::HDR_FORMAT
            } else {
                wgpu::TextureFormat::Rgba8Unorm
            };
            assert_eq!(post.scene_format(), want);
            assert_eq!(
                post.targets.as_ref().unwrap().streak.format(),
                if hdr != 0 && crate::qol::hdr_active() { want } else { INTERMEDIATE }
            );
        }
        crate::qol::Enhancements::default().apply();
    }

    /// The blur where it can be lit draws what the full passes draw, byte
    /// for byte (a dark scene with bright texels in the middle and at the
    /// edges, both glow sizes, a still of three frames; at 1× and under SSAA
    /// 2x, where the glow's source is the image resolved to the screen's
    /// size first), and runs on the device.
    #[test]
    #[ignore = "requires a GPU; run explicitly for High Precision Post's pipelines"]
    fn glow_blur_where_lit_is_the_full_blur() {
        let _globals = crate::qol::test_globals();
        let gpu = pollster::block_on(Gpu::new(None, wgpu::Instance::default())).unwrap();
        // The screen.
        let (w, h) = (96u32, 64u32);
        let half = |v: f32| -> [u8; 2] {
            let b = v.to_bits();
            let e = ((b >> 23) & 0xff) as i32 - 127 + 15;
            let m = ((b >> 13) & 0x3ff) as u16;
            if v == 0.0 { [0, 0] } else { (((e as u16) << 10) | m).to_le_bytes() }
        };
        // 1×, and SSAA 2x (resolved first).
        for (sw, sh) in [(w, h), (2 * w, 2 * h)] {
            let mut data = Vec::new();
            for sy in 0..sh {
                for sx in 0..sw {
                    // single pixels (at the edges too) and a block big
                    // enough to glow past the subtraction, on black (the
                    // screen's pixels, each the scene's texels under it)
                    let (x, y) = (sx * w / sw, sy * h / sh);
                    let lit = [(5, 7), (48, 30), (95, 63), (0, 40), (70, 0)].contains(&(x, y))
                        || ((60..84).contains(&x) && (20..44).contains(&y));
                    let v = if lit { 1.0 } else { 0.0 };
                    for c in [v, v * 0.8, v * 0.6, 1.0] {
                        data.extend(half(c));
                    }
                }
            }
            for hdr in [2u8, 1] {
                crate::qol::Enhancements { hdr_post: hdr, ..Default::default() }.apply();
                let opts = crate::render::RenderOptions::in_force();
                if !crate::qol::hdr_active() {
                    continue;
                }
                let mut frames = Vec::new();
                for skip in [true, false] {
                    let mut post = PostFx::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
                    post.scene_view(&gpu, (sw, sh), &opts);
                    if skip {
                        assert!(post.glow_skip.is_some() || !glow_skip::supported(&gpu));
                    } else {
                        post.glow_skip = None;
                    }
                    let scene = &post.targets.as_ref().unwrap().scene;
                    gpu.queue.write_texture(
                        scene.as_image_copy(),
                        &data,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(sw * 8),
                            rows_per_image: Some(sh),
                        },
                        scene.size(),
                    );
                    let out = gpu.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("out"),
                        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                        view_formats: &[],
                    });
                    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
                    post.set_output_size((w, h));
                    let mut encoder = gpu.device.create_command_encoder(&Default::default());
                    post.apply(
                        &gpu,
                        &opts,
                        &mut encoder,
                        &out.create_view(&Default::default()),
                        Switches::from_detail(5),
                        Frame::GAME,
                        3,
                        false,
                        &mut |_, _, _| {},
                    );
                    gpu.queue.submit([encoder.finish()]);
                    assert!(pollster::block_on(scope.pop()).is_none(), "scene {sw}x{sh} hdr {hdr} skip {skip}");
                    frames.push(crate::render::read_back(&gpu, &out, w, h));
                }
                assert!(frames[0] == frames[1], "scene {sw}x{sh} hdr {hdr}: the blur where lit differs");
                // The glow reached the dark pixels (the test sees a glow at all).
                let px = |f: &Vec<u8>, x: u32, y: u32| f[((y * w + x) * 4) as usize];
                assert!(px(&frames[0], 57, 32) > px(&frames[0], 30, 50), "scene {sw}x{sh} hdr {hdr}: no glow");
            }
        }
        crate::qol::Enhancements::default().apply();
    }

    #[test]
    fn only_ssaa_with_high_precision_post_resolves_first() {
        let (scene, screen) = ((5431, 3055), (3840, 2160));
        assert!(resolves_first(scene, screen, Some(2), false));
        assert!(resolves_first(scene, screen, Some(1), false));
        // The native 8-bit chain, HDR Output, 1×, render scale below 100 %
        // (FSR 1 / the resample upscale) and a scene larger one way only:
        // as before.
        assert!(!resolves_first(scene, screen, None, false));
        assert!(!resolves_first(scene, screen, Some(2), true));
        assert!(!resolves_first(screen, screen, Some(2), false));
        assert!(!resolves_first((1920, 1080), screen, Some(2), false));
        assert!(!resolves_first((3840, 3055), screen, Some(2), false));
    }

    /// An f32 as the half float a `Rgba16Float` texel holds (normal values
    /// and 0, mantissa truncated; the test's input only).
    fn f16_bits(x: f32) -> u16 {
        if x == 0.0 {
            return 0;
        }
        let b = x.to_bits();
        let e = ((b >> 23) & 0xff) as i32 - 127 + 15;
        ((e as u32) << 10 | ((b >> 13) & 0x3ff)) as u16
    }

    /// SSAA with High Precision Post (docs/ssaa-screen-passes.md) on the
    /// device: the chain that resolves to the screen's size before the
    /// bleach bypass against the one at the scene's size. The scene is
    /// exactly twice the screen, in 2 × 2 blocks of one colour each (some
    /// above 1): every screen pixel's box is one colour, where the bleach
    /// bypass and the streak buffer's add are the same before or after the
    /// average. What is left: the 8-bit rounding and the dither (once at
    /// the screen's size instead of per scene texel, averaged) and the
    /// glow's prefilter (the screen's tent on the resolved image instead
    /// of the box of the scene). Then every texel its own colour: the
    /// bleach bypass is still graded per scene texel before the average,
    /// so only the streak buffer's saturation (after the average now) and
    /// the dither differ. Every variant of the last pass (alpha 0, without
    /// the overbright pass, the streaks or the bleach bypass) runs without
    /// a validation error.
    #[test]
    #[ignore = "requires a GPU; run explicitly for SSAA's resolved post chain"]
    fn resolved_chain_matches_the_scene_size_chain() {
        let _globals = crate::qol::test_globals();
        let gpu = pollster::block_on(Gpu::new(None, wgpu::Instance::default())).unwrap();
        crate::qol::Enhancements { hdr_post: 2, ..Default::default() }.apply();
        let opts = crate::render::RenderOptions::in_force();
        let (scene, screen) = ((128u32, 96u32), (64u32, 48u32));
        // `block` × `block` scene texels of one colour each.
        let pattern = |block: u32| -> Vec<u16> {
            (0..scene.1)
                .flat_map(|y| {
                    (0..scene.0).flat_map(move |x| {
                        let i = ((y / block) * 128 + x / block).wrapping_mul(2_654_435_761);
                        let c = |s: u32| f16_bits(((i >> s) & 255) as f32 / 200.0);
                        [c(0), c(8), c(16), f16_bits(1.0)]
                    })
                })
                .collect()
        };
        let blocks = pattern(2);
        let run_on = |texels: &[u16], scene_size_post: bool, switches: Switches, alpha_zero: bool| {
            let mut post = PostFx::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
            post.scene_size_post = scene_size_post;
            post.scene_view(&gpu, scene, &opts);
            gpu.queue.write_texture(
                post.scene_texture().as_image_copy(),
                bytemuck::cast_slice(texels),
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(scene.0 * 8), rows_per_image: None },
                wgpu::Extent3d { width: scene.0, height: scene.1, depth_or_array_layers: 1 },
            );
            let out = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("out"),
                size: wgpu::Extent3d { width: screen.0, height: screen.1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
            post.set_output_size(screen);
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            let view = out.create_view(&Default::default());
            post.apply(&gpu, &opts, &mut encoder, &view, switches, Frame::GAME, 1, alpha_zero, &mut |_, _, _| {});
            gpu.queue.submit([encoder.finish()]);
            let error = pollster::block_on(scope.pop());
            assert!(error.is_none(), "scene size {scene_size_post}, alpha 0 {alpha_zero}: {error:?}");
            crate::render::read_back(&gpu, &out, screen.0, screen.1)
        };
        let run = |scene_size_post: bool, switches: Switches, alpha_zero: bool| {
            run_on(&blocks, scene_size_post, switches, alpha_zero)
        };
        let diffs = |a: &[u8], b: &[u8]| -> Vec<u8> {
            a.chunks_exact(4)
                .zip(b.chunks_exact(4))
                .map(|(a, b)| (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap())
                .collect()
        };
        let all = Switches::from_detail(5);
        let d = diffs(&run(true, all, false), &run(false, all, false));
        let max = *d.iter().max().unwrap();
        let mean = d.iter().map(|&v| f64::from(v)).sum::<f64>() / d.len() as f64;
        let counts: Vec<usize> = (0..=max).map(|k| d.iter().filter(|&&v| v == k).count()).collect();
        // ±1 of each dither, rounded, and the glow's prefilter: at most 3
        // steps, about one on average.
        assert!(max <= 3 && mean < 1.0, "resolved vs scene-size chain: max {max}, mean {mean:.3}, counts {counts:?}");
        // Every texel its own colour: the bleach bypass is graded per scene
        // texel before the box as before, so only the streak buffer's
        // saturation (after the box now) and the dither differ.
        let texels = pattern(1);
        let d = diffs(&run_on(&texels, true, all, false), &run_on(&texels, false, all, false));
        let mut sorted = d.clone();
        sorted.sort_unstable();
        let (p99, max) = (sorted[sorted.len() * 99 / 100], *sorted.last().unwrap());
        let mean = d.iter().map(|&v| f64::from(v)).sum::<f64>() / d.len() as f64;
        assert!(p99 <= 2 && max <= 12 && mean < 1.25, "per-texel colours: p99 {p99}, max {max}, mean {mean:.3}");
        // The other last passes.
        run(false, all, true);
        run(false, Switches { overbright: false, ..all }, false);
        run(false, Switches { light_streaks: false, ..all }, true);
        run(false, Switches { bleach_bypass: false, ..all }, false);
        crate::qol::Enhancements::default().apply();
    }

    #[test]
    fn glow_size_is_native_at_1x_and_follows_the_screen_when_scaled() {
        // 1×: the scene's half (native 0x5BB850), or its own size (HDR 2).
        assert_eq!(glow_size((1280, 960), (1280, 960), None), (640, 480));
        assert_eq!(glow_size((1281, 961), (1281, 961), Some(1)), (640, 480));
        assert_eq!(glow_size((1280, 960), (1280, 960), Some(2)), (1280, 960));
        // SSAA 4x / 16x, render scale 50 %: the screen's half whatever the
        // scene's size.
        for scene in [(2560, 1920), (5120, 3840), (640, 480), (1810, 1358)] {
            assert_eq!(glow_size(scene, (1280, 960), None), (640, 480));
            assert_eq!(glow_size(scene, (1280, 960), Some(2)), (1280, 960));
        }
    }

    /// The box `box_prefilter` makes, per axis, as weights over the
    /// scene's texels (each bilinear tap spreads over its two texels).
    fn box_weights(scene: u32, glow: u32, width: f32, glow_texel: u32) -> Vec<f32> {
        let (n, step) = box_prefilter((scene, scene), (glow, glow), width);
        let mut w = vec![0.0f32; scene as usize];
        let centre = (glow_texel as f32 + 0.5) / glow as f32;
        for i in 0..n {
            let uv = centre + step[0] * (i as f32 - (n as f32 - 1.0) * 0.5);
            let x = uv * scene as f32 - 0.5;
            let (x0, f) = (x.floor(), x - x.floor());
            w[x0 as usize] += (1.0 - f) / n as f32;
            w[x0 as usize + 1] += f / n as f32;
        }
        w
    }

    #[test]
    fn box_prefilter_is_the_screens_2x2_box_at_whole_scales() {
        // Native: C's texel is the 2×2 box of the screen's texels; at an
        // SSAA scale s the same box is 2s scene texels, flat.
        for s in [2u32, 3, 4] {
            let (glow, scene) = (8u32, 16 * s);
            let w = box_weights(scene, glow, 1.0, 3);
            let lit: Vec<usize> = (0..w.len()).filter(|&i| w[i] > 1e-6).collect();
            assert_eq!(lit, ((6 * s) as usize..(8 * s) as usize).collect::<Vec<_>>(), "s {s}");
            for &i in &lit {
                assert!((w[i] - 1.0 / (2 * s) as f32).abs() < 1e-5, "s {s} texel {i}: {}", w[i]);
            }
        }
        // Render scale 50 %: the glow texel is the scene's: one tap at its
        // centre, the texel itself.
        let w = box_weights(8, 8, 1.0, 5);
        assert!((w[5] - 1.0).abs() < 1e-6 && w.iter().sum::<f32>() - 1.0 < 1e-6);
        // The taps a side stay within the shader's loop.
        assert_eq!(box_prefilter((15360, 8640), (1920, 1080), HDR_FULL_BOX).0, BOX_MAX.min(11));
        assert!(box_prefilter((7680, 4320), (960, 540), 1.0).0 <= BOX_MAX);
    }

    #[test]
    fn hdr_full_box_has_the_tents_variance() {
        // The ¼ ½ ¼ tent (½) and the screen pixel's own box (1/12).
        assert!((HDR_FULL_BOX * HDR_FULL_BOX / 12.0 - (0.5 + 1.0 / 12.0)).abs() < 1e-5);
    }

    #[test]
    fn scaled_blur_spans_the_same_part_of_the_screen() {
        // filter.efx's taps (±1, ±3 glow texels) in uv: with the glow at
        // the screen's half they are the same at 1× and every scale, so
        // the glow's reach on screen does not shrink with SSAA.
        let uv = |g: (u32, u32)| BLUR_ACROSS.map(|(_, du, _)| du / g.0 as f32);
        let native = uv(glow_size((1920, 1080), (1920, 1080), None));
        for scene in [(3840, 2160), (7680, 4320), (960, 540)] {
            assert_eq!(uv(glow_size(scene, (1920, 1080), None)), native);
        }
    }

    #[test]
    fn hdr_sigma_matches_the_native_blur_variance() {
        // One round of ⅓ at ±1 and ⅙ at ±3 texels: variance 11/3; two rounds.
        let round: f32 = BLUR_ACROSS.iter().map(|&(w, du, _)| w * du * du).sum();
        assert!((round - 11.0 / 3.0).abs() < 1e-5);
        assert!((HDR_SIGMA_HALF * HDR_SIGMA_HALF - 2.0 * round).abs() < 1e-3);
        // At the screen's size: ×4, plus the 2×2 box (¼) and the bilinear
        // upsample (⅔), less the 3×3 tent (½).
        assert!((HDR_SIGMA_FULL * HDR_SIGMA_FULL - (4.0 * 2.0 * round + 0.25 + 2.0 / 3.0 - 0.5)).abs() < 1e-3);
    }

    #[test]
    fn half_gain_is_the_identity() {
        for i in 0..=20 {
            let x = i as f32 / 20.0;
            assert!((gain(x, 0.5) - x).abs() < 1e-6);
        }
    }

    #[test]
    fn gain_keeps_the_pivot_and_ends() {
        for g in [0.26, 0.2, 0.4] {
            assert!((gain(GAIN_PIVOT, g) - GAIN_PIVOT).abs() < 1e-6);
            assert_eq!(gain(0.0, g), 0.0);
            assert!((gain(1.0, g) - 1.0).abs() < 1e-6);
        }
        // Below 0.5 the curve steepens about the pivot.
        assert!(gain(0.2, 0.26) < 0.2);
        assert!(gain(0.7, 0.26) > 0.7);
    }

    #[test]
    fn settled_overbright_levels_match_the_traced_constants() {
        // apitrace of SPEED2.EXE: in game OVERBRIGHTOFFSET 0.3627451 and
        // BLENDFACTOR 0x16161616; in the front end 0.2529412.
        let game = OverbrightLevel(OverbrightLevel::target(false));
        assert_eq!((game.offset(), game.add()), (185, 22));
        let front = OverbrightLevel(OverbrightLevel::target(true));
        assert_eq!(front.offset(), 129);
        // The level settles from .data's start value.
        let mut l = OverbrightLevel::default();
        for _ in 0..100 {
            l.step(false);
        }
        assert!((l.0 - 0.2).abs() < 1e-5);
    }

    #[test]
    fn tint_color_follows_the_amount() {
        // Front end: TINTCOLOR (0.105, 0.105, 0.105, 0.2) in the trace.
        let c = Frame::FRONT_END.tint_color();
        assert!((c[0] - 0.105).abs() < 1e-5);
    }

    #[test]
    fn maps_have_their_sizes() {
        assert_eq!(gain_map(0.8).len(), 32 * 32 * 4);
        let ramp = ramp_map();
        assert_eq!(ramp.len(), 256 * 4);
        assert_eq!(&ramp[255 * 4..], &[255, 255, 255, 255]);
        // Blue is biased below the grey.
        assert!(ramp[128 * 4 + 2] < ramp[128 * 4]);
    }
}
