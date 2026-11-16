//! A car renderer's per-car state: LOD, poses, environment, uniforms (moved from car.rs).

use super::*;

pub struct CarRenderer {
    /// Pop meter: where the car was drawn last (a jump: placed again).
    pub meter_at: Option<glam::Vec3>,
    /// Pop meter: the share drawn last frame (the traffic ring fade).
    pub meter_share: f32,
    /// The car's underside was visible on its last draw (render info
    /// +0x2EB8, written by 0x60C330 after the LOD pick; 0 from 0x638380):
    /// what the next pick's airborne rule reads (`car::airborne_lod`).
    pub underside_seen: bool,
    pub(super) shared: std::sync::Arc<CarShared>,
    pub(crate) pipelines: std::sync::Arc<Pipelines>,
    /// The same pipelines for the world renderer's reflection image
    /// ([`CarRenderer::prepare_reflection`]).
    pub(super) reflection_pipelines: Option<std::sync::Arc<Pipelines>>,
    /// The scene's format (for [`CarRenderer::sampled`]).
    pub(super) color_format: wgpu::TextureFormat,
    pub(super) frame_buffer: wgpu::Buffer,
    pub(super) env_sampler: wgpu::Sampler,
    pub(crate) frame_group: wgpu::BindGroup,
    /// The cube bound now (to rebind only on change).
    pub(super) environment: Option<wgpu::TextureView>,
    pub(super) world_environment: bool,
    pub(super) draw_buffer: wgpu::Buffer,
    pub(crate) draw_group: wgpu::BindGroup,
    pub(super) textures: Vec<wgpu::BindGroup>,
    /// Surface Detail (docs/surface-maps.md "Cars"): the textures'
    /// views and sampler (to bind a map with them), the groups with a map
    /// (bound instead of `textures` once made; the native shaders never
    /// read the map), the textures that may get one, and their builder.
    pub(super) texture_views: Vec<wgpu::TextureView>,
    pub(super) texture_sampler: wgpu::Sampler,
    /// docs/pack-hot-reload.md: the pack changes seen, and per
    /// texture (`textures`' index) its source, to upload it again.
    pub(super) pack_seen: u64,
    pub(super) pack_textures: Vec<Option<crate::formats::texture::Texture>>,
    pub(super) surface_textures: Vec<Option<wgpu::BindGroup>>,
    pub(super) surface_jobs: Vec<SurfaceJob>,
    pub(super) surface_builder: Option<crate::surface_maps::Builder<(u64, Option<crate::surface_maps::CarMap>)>>,
    pub(crate) vertices: wgpu::Buffer,
    pub(crate) indices: wgpu::Buffer,
    pub(crate) draws: Vec<Draw>,
    pub(super) part_transforms: Vec<Mat4>,
    /// Per part: the wheel it follows (table index, rolls with it).
    pub(super) part_wheels: Vec<Option<(usize, bool)>>,
    pub(super) wheel_centres: [Vec3; 4],
    /// Per part: [`crate::car::CarPart::pivot_y`].
    pub(super) part_pivots: Vec<Option<f32>>,
    /// Per part: its light context's factor on the harmonic.
    pub(super) part_harmonic: Vec<f32>,
    pub(super) part_scene: Vec<crate::car::Scene>,
    pub(super) part_spinner: Vec<bool>,
    pub(super) part_audio: Vec<crate::car::AudioPaint>,
    pub(super) part_anim: Vec<Option<(u8, u8, glam::Vec3)>>,
    pub(super) part_hood_gate: Vec<bool>,
    pub(super) part_harmonic_anim: Vec<Option<(u8, f32)>>,
    pub(super) part_harmonic_glow: Vec<Option<f32>>,
    /// The engine and trunk neon colours this frame (render info +0x364 /
    /// +0x37C: R, G, B, intensity bytes), [`CarRenderer::set_part_glow`].
    pub(super) glow: [[u8; 4]; 2],
    /// Per part: [`crate::car::CarPart::lods`].
    pub(super) part_lods: Vec<Option<u8>>,
    /// The LOD the next draws use ([`CarRenderer::set_lod`]); None: the
    /// car is too small to draw.
    pub(crate) lod: Option<u8>,
    /// The part animation channels' poses for the next draws
    /// ([`CarRenderer::set_part_poses`]).
    pub(super) poses: [[Option<crate::part_anim::ChannelPose>; 2]; 4],
    /// Queued for the shared pass: the front end's lighting or not, and
    /// where the draws' values start in the [`CarBatch`] (None: in the
    /// car's own buffers).
    pub(super) queued: std::cell::Cell<Option<(bool, Option<u32>)>>,
    /// LOD Fade: the queued draws cross-fade from this LOD, whose values
    /// start at this entry.
    pub(super) queued_fade: std::cell::Cell<Option<(Option<u8>, u32)>>,
    /// What the last [`CarRenderer::record_queued`] drew, for the SSR
    /// mask ([`record_ssr_masks`]).
    pub(super) mask_queued: std::cell::Cell<Option<(bool, Option<u32>)>>,
    /// Enhanced Glass (car_glass.rs): the glass draws are left out of
    /// the car pass (drawn after it over the scene's copy); what the glass
    /// pass draws (as `mask_queued`); the eye in the car's space.
    pub(crate) glass_split: std::cell::Cell<bool>,
    pub(crate) glass_queued: std::cell::Cell<Option<(bool, Option<u32>)>>,
    pub(crate) glass_eye: std::cell::Cell<Vec3>,
    /// Which pipelines the car pass drew with (the glass pass draws the
    /// draws after the glass with them): 0 `pipelines`, 1 `sampled`, 2 LOD Fade's.
    pub(crate) glass_with: std::cell::Cell<u8>,
    /// The glass draws' box (model space, min / max) and this frame's
    /// screen rect of it (NDC x0, y0, x1, y1; None: the whole view).
    pub(crate) glass_box: (Vec3, Vec3),
    pub(crate) glass_rect: std::cell::Cell<Option<[f32; 4]>>,
    /// The frame group reading the [`CarBatch`]'s frame values, with this
    /// car's cube and sampler (made when first queued, again when the cube
    /// changes).
    pub(crate) batch_frame_group: Option<wgpu::BindGroup>,
    /// The pipelines for the scene's mode (the batch's sample count, the
    /// Graphics page's Reversed-Z and MSAA), when not the native one.
    pub(crate) sampled: Option<(CarMode, std::sync::Arc<Pipelines>)>,
    /// LOD Fade (clone deviation): the main view's LOD as last picked
    /// ([`CarRenderer::set_lod_faded`]; None before the first), the switch
    /// being faded (the LOD shown before and when it began), and this
    /// frame's fade (the LOD faded from, the new LOD's share), which the
    /// next queue draws; `set_lod` (other views, shots) draws none.
    pub(super) main_lod: Option<Option<u8>>,
    pub(super) switch: Option<(Option<u8>, f32)>,
    pub(super) drawing_fade: Option<(Option<u8>, f32)>,
    /// LOD Fade's pipelines for the batch's scene mode.
    pub(crate) fade_pipelines: Option<(CarMode, std::sync::Arc<Pipelines>)>,
    /// Wet Car (clone deviation): the car's state and this frame's values
    /// (None: dry, the native pipelines).
    /// Wet Car's per-car state (its velocity and spray film), stepped on
    /// the tick ([`CarRenderer::tick_wet`]), never by a draw.
    pub(super) wet: crate::render::wet_car::WetState,
    /// The car's wet values of the last two ticks (S_{n−1}, S_n): a draw
    /// blends them ([`crate::render::wet_car::WetParams::blend`]).
    pub(super) wet_ticks: [Option<crate::render::wet_car::WetParams>; 2],
    /// This frame's wet values (set by the draw from `wet_ticks`; read by
    /// the frame's own uniforms only).
    pub(crate) wet_now: Option<crate::render::wet_car::WetParams>,
}

impl CarRenderer {
    /// The car LOD the following draws use ([`crate::car::pick_lod`]; A = 0,
    /// the default); None draws nothing.
    pub fn set_lod(&mut self, lod: Option<u8>) {
        self.lod = lod;
        self.drawing_fade = None;
    }

    /// The main view's LOD as last picked by [`CarRenderer::set_lod_faded`]
    /// (None before the first; the inner None: not drawn).
    pub fn main_lod(&self) -> Option<Option<u8>> {
        self.main_lod
    }

    /// The main view's LOD at frame time `now` (seconds). With LOD Fade
    /// (clone deviation) a switch is drawn as a cross-fade over
    /// [`crate::render::RenderOptions::LOD_FADE_SECONDS`]: the old LOD with one screen-door
    /// pattern, the new one with its complement, so every pixel shows one
    /// of them; without it, as [`CarRenderer::set_lod`] (native).
    /// `lod_fade`: LOD Fade in force (the frame's).
    pub fn set_lod_faded(&mut self, lod: Option<u8>, now: f32, lod_fade: bool) {
        let before = self.main_lod.replace(lod);
        self.lod = lod;
        self.drawing_fade = None;
        let switched = before.is_some_and(|b| b != lod);
        if !lod_fade {
            self.switch = None;
            if switched {
                crate::render::count_car_pop(false);
            }
            return;
        }
        if switched {
            // From the LOD that was in front (mid-fade: the newer one).
            self.switch = Some((before.flatten_lod(), now));
            crate::render::count_car_pop(true);
        }
        if let Some((from, start)) = self.switch {
            let t = (now - start) / crate::render::RenderOptions::LOD_FADE_SECONDS;
            if !(0.0..1.0).contains(&t) || from == lod {
                self.switch = None;
            } else {
                self.drawing_fade = Some((from, t.max(1.0 / 64.0)));
            }
        }
    }

    /// World Detail Ultra / Extreme with LOD Fade: the car drawn at
    /// `share` (1 whole; 0 not drawn; between, the LOD Fade screen-door
    /// at that share, any LOD switch in progress folded in), after
    /// [`CarRenderer::set_lod_faded`].
    pub fn set_presence(&mut self, share: f32) {
        if share >= 1.0 {
            return;
        }
        if share <= 0.0 {
            self.lod = None;
            self.drawing_fade = None;
            return;
        }
        let t = self.drawing_fade.map_or(1.0, |(_, t)| t);
        if self.lod.is_some() {
            self.drawing_fade = Some((None, (t * share).max(1.0 / 64.0)));
        }
    }

    /// The hood, trunk and door poses the following draws use
    /// ([`crate::part_anim::Channels::poses`]; all None: everything shut).
    pub fn set_part_poses(&mut self, poses: [[Option<crate::part_anim::ChannelPose>; 2]; 4]) {
        self.poses = poses;
    }

    /// The engine (channel 0) and trunk (channel 1) neon colours the
    /// level-3 engine and trunk audio contexts add to c0
    /// ([`crate::car::CarPart::harmonic_glow`]): render info +0x364 /
    /// +0x37C as the neon pulse (0x622AA8 / 0x622BF8,
    /// `shadow::neon_tube_color`) leaves them, 0 without the neon part.
    pub fn set_part_glow(&mut self, glow: [[u8; 4]; 2]) {
        self.glow = glow;
    }

    /// Bind a dynamic world cube. Enhanced captures use world XYZ; static
    /// frontend cubes keep their authored camera-relative convention.
    pub fn set_world_environment(&mut self, gpu: &Gpu, cube: &wgpu::TextureView) {
        self.set_environment(gpu, cube);
        self.world_environment = true;
    }

    /// Bind an authored, camera-relative cube (for example frontend platforms).
    /// Use `set_world_environment` for the world renderer's dynamic cube.
    pub fn set_environment(&mut self, gpu: &Gpu, cube: &wgpu::TextureView) {
        self.world_environment = false;
        if self.environment.as_ref() == Some(cube) {
            return;
        }
        self.frame_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("car frame"),
            layout: &self.shared.frame_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.frame_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(cube) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.env_sampler) },
            ],
        });
        self.environment = Some(cube.clone());
        self.batch_frame_group = None;
    }

    /// The tick: step the car's wet state with the tick's
    /// weather (`scene`: None out of game or with every weather row off)
    /// and the car's matrix at the tick, keeping the tick's wet values
    /// beside the last tick's. Returns the car as the spray and the wet
    /// road see it this tick (None without a scene).
    pub fn tick_wet(
        &mut self,
        scene: Option<&crate::render::wet_car::Scene>,
        world: Mat4,
    ) -> Option<crate::render::weather_fx::CarSample> {
        let wet = self.wet.step(scene, world);
        self.wet_ticks = [self.wet_ticks[1], wet];
        scene.map(|_| crate::render::weather_fx::CarSample::of(world, &self.wheel_centres, self.wet.velocity))
    }

    /// A digest of the car's wet state at the last tick (the rate tests).
    pub fn wet_digest(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        if let Some(p) = self.wet_ticks[1] {
            for v in [p.film, p.rain, p.spray, p.velocity.x, p.velocity.y, p.velocity.z, p.clock] {
                h = (h ^ u64::from(v.to_bits())).wrapping_mul(0x100_0000_01b3);
            }
        }
        h
    }

    /// Wet Car: this frame's wet values (in game, the option on, the car
    /// wet), blended between the last two ticks' by the frame's α
    /// (`wet_car::frame_alpha`); front-end scenes and a dry car get none.
    /// Reads the tick state only (the step is the tick's).
    pub(super) fn frame_wet(&mut self, lighting: &CarLighting, opts: &crate::render::RenderOptions) {
        let scene = if lighting.is_frontend() { None } else { crate::render::wet_car::scene() };
        let wet = scene.and_then(|_| {
            crate::render::wet_car::WetParams::blend(
                self.wet_ticks[0],
                self.wet_ticks[1],
                crate::render::wet_car::frame_alpha(),
            )
        });
        // Wet Road Interaction alone: the road's water on the lower
        // body and the tyres (the spray film), not the rain's.
        self.wet_now = if opts.wet_car {
            wet
        } else if opts.wet_interaction != 0 {
            wet.and_then(crate::render::wet_car::spray_only)
        } else {
            None
        };
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn write_uniforms(
        &self,
        gpu: &Gpu,
        view_proj: Mat4,
        view: Mat4,
        lighting: &CarLighting,
        world: Mat4,
        wheels: &[WheelPose; 4],
        opts: &crate::render::RenderOptions,
    ) {
        let _t = crate::prof::acc::span("cars: uniforms");
        gpu.queue.write_buffer(&self.frame_buffer, 0, bytemuck::bytes_of(&frame_uniform(view_proj, view)));
        let mut uniforms = Vec::with_capacity(self.draws.len());
        if self.draw_uniforms(lighting, world, wheels, &mut uniforms, opts) {
            gpu.queue.write_buffer(&self.draw_buffer, 0, bytemuck::cast_slice(&uniforms));
        }
    }

    /// Every draw's values, appended to `uniforms` (in draw order; zeros
    /// for the draws the LOD leaves out). False (nothing appended) with no
    /// LOD drawn.
    pub(super) fn draw_uniforms(
        &self,
        lighting: &CarLighting,
        world: Mat4,
        wheels: &[WheelPose; 4],
        uniforms: &mut Vec<DrawUniform>,
        opts: &crate::render::RenderOptions,
    ) -> bool {
        // No draw at all at this size: no draw reads its values.
        let Some(lod) = self.lod else { return false };
        self.draw_uniforms_at(lod, [0.0, 0.0], lighting, world, wheels, uniforms, opts);
        true
    }

    /// [`CarRenderer::draw_uniforms`] at `lod`, with LOD Fade's fade and
    /// sample count in `powers.zw` (0, 0: none, native).
    pub(super) fn draw_uniforms_at(
        &self,
        lod: u8,
        fade: [f32; 2],
        lighting: &CarLighting,
        world: Mat4,
        wheels: &[WheelPose; 4],
        uniforms: &mut Vec<DrawUniform>,
        opts: &crate::render::RenderOptions,
    ) {
        // Native 0x62132D / traffic 0x61684C: copied mesh matrix only.
        // Ordinary parts/wheels use it directly; animated parts carry it
        // through the base axes below, as the native open-part path does.
        let base_world = world;
        let world = mesh_world(base_world);
        // Per part, once (its draws share them): its matrix and light
        // context.
        let mut parts: Vec<Option<(Mat4, [glam::Vec4; 10], Vec3)>> = vec![None; self.part_transforms.len()];
        for d in &self.draws {
            // Not drawn at this LOD (as `record_with` skips it): its
            // values are not read.
            if self.part_lods[d.part].is_some_and(|m| m >> lod & 1 == 0) {
                uniforms.push(DrawUniform::zeroed());
                continue;
            }
            let (part_world, harmonic, local_eye) = *parts[d.part].get_or_insert_with(|| {
                let pivot = |i: usize| {
                    let c = self.wheel_centres[i];
                    self.part_pivots[d.part].map_or(c, |y| Vec3::new(c.x, y, c.z))
                };
                // The front wheels' x-turn (camber), at LOD A and B only
                // (0x6270A0): wheel 0 by +angle, wheel 1 by −angle; the
                // brakes (no roll) by their own, accumulated turn.
                let tilt = |i: usize, rolls: bool| match i {
                    0 | 1 if lod <= 1 => {
                        let t = if rolls { wheels[i].tilt } else { wheels[i].brake_tilt };
                        if i == 0 { t } else { -t }
                    }
                    _ => 0.0,
                };
                let pose = match self.part_wheels[d.part] {
                    // A spinner turns by its own angle (0x5EC800).
                    Some((i, rolls)) if self.part_spinner[d.part] => {
                        crate::car::WheelPose { roll: wheels[i].spinner, ..wheels[i] }.matrix_tilted(
                            pivot(i),
                            rolls,
                            tilt(i, true),
                        )
                    }
                    Some((i, rolls)) => wheels[i].matrix_tilted(pivot(i), rolls, tilt(i, rolls)),
                    None => Mat4::IDENTITY,
                };
                // An animated model (0x6235A1…): its channel's pose in car
                // space, when the channel is not shut.
                let animated_world = self.part_anim[d.part]
                    .and_then(|(c, g, origin)| {
                        self.poses[c as usize][(g as usize).min(1)]
                            .map(|p| animated_mesh_world(base_world, p.part_matrix(origin)))
                    })
                    .unwrap_or(world);
                let part_world = animated_world * pose * self.part_transforms[d.part];
                // The car's light context (LocalEyePos and HarmonicCoeff in
                // the car's axes) for the body; wheels and brakes get their
                // own, in their model's axes (0x48F660 per wheel in
                // 0x620AF0).
                let (harmonic, local_eye) = match self.part_wheels[d.part] {
                    Some(_) => lighting.in_axes(part_world),
                    None => (lighting.harmonic, lighting.local_eye),
                };
                (part_world, harmonic, local_eye)
            });
            let c = d.constants;
            // An animated context (0x6304E3 / 0x6306F6): the harmonic ×
            // the channel's value × k, then for a level-3 part c0.rgb +=
            // colour · intensity · k' · value (bytes ÷ 256 each).
            let (harmonic, harmonic_x) = match self.part_harmonic_anim[d.part] {
                Some((ch, k)) => {
                    let v = self.poses[ch as usize][0].map_or(0.0, |p| p.value);
                    let mut h = harmonic;
                    // The add is ∝ value too, so with a shut channel both
                    // are 0; the shader multiplies by harmonic_scale.x,
                    // folded in here.
                    if let Some(k2) = self.part_harmonic_glow[d.part]
                        && v * k != 0.0
                    {
                        let g = self.glow[usize::from(ch).min(1)].map(|b| f32::from(b) / 256.0);
                        h[0] += glam::Vec4::new(g[0], g[1], g[2], 0.0) * (g[3] * k2 / k);
                    }
                    (h, v * k)
                }
                None => (harmonic, self.part_harmonic[d.part]),
            };
            // docs/car-scene-specular.md "Higher settings": with the scene
            // specular the environment reflection carries the body (1, the
            // native material, otherwise).
            let env = |v: [f32; 4]| {
                let k = lighting.env_gain;
                [v[0] * k, v[1] * k, v[2] * k, v[3]]
            };
            uniforms.push(DrawUniform {
                world: part_world.to_cols_array_2d(),
                local_eye: local_eye.extend(1.0).to_array(),
                harmonic: harmonic.map(|v| v.to_array()),
                material: [
                    c.diffuse_min,
                    c.diffuse_range,
                    c.specular_min,
                    c.specular_range,
                    env(c.envmap_min),
                    env(c.envmap_range),
                ],
                powers: [c.specular_power, c.envmap_power, fade[0], fade[1]],
                harmonic_scale: [
                    harmonic_x,
                    if opts.enhanced_reflections { 1.0 } else { 0.0 },
                    if self.world_environment { 1.0 } else { 0.0 },
                    // w: unused (0).
                    0.0,
                ],
                paint: if opts.enhanced_paint { d.paint } else { [0.0; 4] },
                lamps: lighting.lamps,
                wet: match self.wet_now {
                    Some(w) => w.uniform(d.wet_class, self.wheel_centres[0].x, self.wheel_centres[0].z, opts.rain_next_gen()),
                    None => [[0.0; 4]; 3],
                },
            });
        }
    }

    /// Texture `i`'s group: with its Surface Detail map once made.
    pub(crate) fn texture_group(&self, i: usize) -> &wgpu::BindGroup {
        self.surface_textures.get(i).and_then(|g| g.as_ref()).unwrap_or(&self.textures[i])
    }

    /// Surface Detail (G7c2): with Materials + Normals on, start making the
    /// car's maps (on first use, in the background; shared by key with the
    /// other cars) and bind those that arrived (all of them at once in the
    /// headless modes, `surface_maps::blocking`).
    /// docs/pack-hot-reload.md, Reload Packs When Changed: upload
    /// again the textures whose car pack file changed (any size: each is a
    /// texture of its own). Their Surface Detail map is left off until the
    /// car is built again.
    pub(super) fn sync_texture_pack(&mut self, gpu: &Gpu) {
        let (now, pending) =
            crate::render::pack_reload::since(self.pack_seen, crate::render::texture_pack::Domain::Car);
        self.pack_seen = now;
        if !pending.any() {
            return;
        }
        let bc = gpu.device.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        for (i, t) in self.pack_textures.iter().enumerate() {
            let Some(t) = t.as_ref().filter(|t| pending.hits(t.hash, &t.name)) else { continue };
            let view = upload_texture_for(gpu, t, bc, crate::render::texture_pack::Domain::Car);
            let flat = self.shared.surface_defaults(&gpu.device);
            self.textures[i] =
                bind_car_texture(&gpu.device, &self.shared.texture_layout, &view, &self.texture_sampler, flat);
            self.texture_views[i] = view;
            if let Some(g) = self.surface_textures.get_mut(i) {
                *g = None;
            }
        }
    }

    pub(super) fn pump_surface(&mut self, gpu: &Gpu, opts: &crate::render::RenderOptions) {
        if opts.surface_detail < 2 || (self.surface_jobs.is_empty() && self.surface_builder.is_none()) {
            return;
        }
        let device = &gpu.device;
        if self.surface_builder.is_none() {
            let _t = crate::prof::acc::span("car surface maps");
            let pack = crate::render::texture_pack::enabled(crate::render::texture_pack::Domain::Car);
            let mut jobs = Vec::new();
            for job in std::mem::take(&mut self.surface_jobs) {
                // In memory: the texture, its regions and whether a pack is on.
                let key = crate::surface_maps::car_cache_key(
                    crate::surface_maps::texture_key(&job.t) ^ u64::from(pack),
                    &job.regions,
                );
                let known = self.shared.surface_maps.lock().unwrap().get(&key).cloned();
                match known {
                    Some(map) => self.bind_surface(device, job.texture, map.as_ref()),
                    None => jobs.push((
                        job.texture,
                        Box::new(move || {
                            let source =
                                crate::render::surface::pack_source(crate::render::texture_pack::Domain::Car, &job.t);
                            crate::surface_maps::car_map_for(&job.t, &job.regions, source)
                                .map(|m| (key, Some(m)))
                                .or(Some((key, None)))
                        })
                            as crate::surface_maps::Job<(u64, Option<crate::surface_maps::CarMap>)>,
                    )),
                }
            }
            self.surface_builder = Some(crate::surface_maps::Builder::spawn(jobs));
        }
        let Some(builder) = self.surface_builder.as_mut() else { return };
        let results = if crate::surface_maps::blocking() { builder.wait() } else { builder.poll() };
        for (texture, made) in results {
            let Some((key, map)) = made else { continue };
            let view = map.as_ref().map(|m| crate::render::surface::car_map_view(gpu, m));
            self.shared.surface_maps.lock().unwrap().insert(key, view.clone());
            self.bind_surface(device, texture, view.as_ref());
        }
    }

    fn bind_surface(&mut self, device: &wgpu::Device, texture: usize, map: Option<&wgpu::TextureView>) {
        let Some(map) = map else { return };
        let sampler = self.shared.surface_defaults(device).1.clone();
        let group = bind_car_texture(
            device,
            &self.shared.texture_layout,
            &self.texture_views[texture],
            &self.texture_sampler,
            &(map.clone(), sampler),
        );
        if let Some(slot) = self.surface_textures.get_mut(texture) {
            *slot = Some(group);
        }
    }
}

/// The animated path's matrix (0x62356A–0x6243E5).
pub(super) fn animated_mesh_world(base: Mat4, animation: Mat4) -> Mat4 {
    // Native first removes the unmodified base from the copied mesh,
    // applies the part pose, then restores the base. The 1 cm copied
    // translation therefore turns with the open part, also in reflection.
    base * animation * base.inverse() * mesh_world(base)
}

/// Copied mesh matrix for native Car_Draw and traffic draw; base stays intact.
pub(super) fn mesh_world(mut world: Mat4) -> Mat4 {
    world.w_axis.z += 0.01;
    world
}
