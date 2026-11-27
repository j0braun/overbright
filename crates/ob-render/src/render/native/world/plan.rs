//! A frame's world views planned (ViewsPlan, plan_views) and the per-frame accessors next to it (moved from render/mod.rs).

use crate::render::*;

/// A frame's world views as [`WorldRenderer::plan_views`] set them up:
/// what recording them needs besides the core.
pub(in crate::render) struct ViewsPlan {
    /// By section number: drawn this frame.
    pub(in crate::render) visible: Vec<bool>,
    /// World Detail Extreme: the far sections the main view draws at their
    /// lowest LOD (with LOD Fade, also those still fading out).
    pub(in crate::render) far: Option<Vec<bool>>,
    /// The road reflection's culling and target (colour, depth), if drawn.
    pub(in crate::render) reflection: Option<(PassCull, wgpu::TextureView, wgpu::TextureView)>,
    /// The rear-view mirror's (view 3), if drawn.
    pub(in crate::render) rear: Option<(PassCull, wgpu::TextureView, wgpu::TextureView)>,
    pub(in crate::render) main: PassCull,
    pub(in crate::render) clear: wgpu::Color,
    /// Group 2 of the main view: the reflection image, or black.
    pub(in crate::render) reflection_group: wgpu::BindGroup,
    pub(in crate::render) time: f32,
    pub(in crate::render) sky: SkyFrame,
    pub(in crate::render) reversed: bool,
    pub(in crate::render) main_lod: u8,
    /// Snapshotted while planning; async recording must not re-read options.
    pub(in crate::render) reflection_lod: u8,
    pub(in crate::render) pool_count: u32,
    pub(in crate::render) reflection_glow_count: u32,
    /// LOD Fade: the per-instance fades the main view draws with, and
    /// its sections (`visible` and those still fading out).
    pub(in crate::render) fade: Option<(wgpu::Buffer, std::sync::Arc<Vec<bool>>)>,
    pub(in crate::render) main_visible: Option<Vec<bool>>,
    /// Screen-Space Reflections' mask, when drawn this frame.
    pub(in crate::render) wet_mask: Option<WetMaskPlan>,
    /// FSR 2's animation mask, when drawn this frame.
    pub(in crate::render) anim_mask: Option<AnimMaskPlan>,
    /// Dynamic Lamps / shadows (clone deviations): the main view's group 0
    /// with the lamps' buffers, and the shadow layers to draw first.
    pub(in crate::render) lamp_frame: Option<wgpu::BindGroup>,
    pub(in crate::render) lamp_shadows: Vec<lamps::ShadowPass>,
    /// Animated Water: the main view leaves the water draws out.
    pub(in crate::render) hide_water: bool,
    /// Modern + RT's traced shadows, between the pre-pass and the
    /// colour pass.
    pub(in crate::render) rt_shadow: Option<rt_shadow::Plan>,
    /// The main view's road draws this frame with a dry road (wet.x 0):
    /// their pipelines without the Wet Roads branch (`DepthPrepass::equal_dry`).
    pub(in crate::render) dry_road: bool,
}

/// By section number, whether a frame draws it: the visibility list's
/// sections and the global one (all, without a list). Section numbers stay
/// below 2700 (X/Y/Z shared sections are 24xx–26xx).
pub(in crate::render) fn visible_sections(params: &FrameParams) -> Vec<bool> {
    visible_of(params.visible_sections)
}

/// [`visible_sections`] of a section list (None: all); a world
/// tick's (`render::enh::tick_views::TickView::sections`).
pub(in crate::render) fn visible_of(list: Option<&[u32]>) -> Vec<bool> {
    let mut visible = vec![list.is_none(); 4096];
    visible[GLOBAL_SECTION as usize] = true;
    for &s in list.unwrap_or_default() {
        if let Some(v) = visible.get_mut(s as usize) {
            *v = true;
        }
    }
    visible
}

/// By section number, the far sections a frame draws with their lowest
/// LOD (World Detail Extreme): listed in `far_sections` and not in the
/// main list. None when there are none.
pub(in crate::render) fn far_sections(params: &FrameParams, visible: &[bool]) -> Option<Vec<bool>> {
    let list = params.far_sections.filter(|l| !l.is_empty() && params.visible_sections.is_some())?;
    let mut far = vec![false; visible.len()];
    for &s in list {
        if let Some(v) = far.get_mut(s as usize)
            && !visible[s as usize]
        {
            *v = true;
        }
    }
    Some(far)
}

impl WorldRenderer {
    /// A frame's world views set up: the uniforms written, the dome moved
    /// to the camera, the reflection target made, the pools uploaded.
    pub(in crate::render) fn plan_views(&mut self, gpu: &Gpu, params: &FrameParams) -> ViewsPlan {
        // Light Probes: bake near the camera before the cars are lit.
        self.bake_probes(gpu, params);
        let visible = visible_sections(params);
        let fog = params.fog.unwrap_or(NO_FOG);
        let view_proj = params.camera.view_projection(params.aspect);
        let morph_alpha = params.sky.morph.map_or(0.0, |(_, a)| a);
        let enhanced = params.opts.enhanced_reflections;
        let main_lod = if params.display.world_lod_b { MAIN_LOD_B } else { MAIN_LOD };
        let (reflection_lod, reflection_min_size) = reflection_capture_quality(enhanced, main_lod, detail_min_size(params.opts));
        // Views 1 and 4 share the camera, so its size scale; view 4 needs
        // the larger size natively (enhanced capture follows main detail).
        // Only the main view draws worldheadlight: every
        // other target turns all non-car effects into worldnofog (0x5C5940).
        let scale = cull_scale(params.camera.game_fov.unwrap_or(params.camera.fov_y));
        // View 9 only with g_CarHeadlightEnable (0x5CB200).
        let headlight =
            if params.display.headlight { headlight_uniform(params.headlight_pools.first()) } else { [[0.0; 4]; 5] };
        // FOGON (0x5CB0B1): g_FogEnable in the main view only; the road
        // reflection's targets use worldnofog regardless.
        let main_fog = if params.display.fog { fog } else { NO_FOG };
        let uniform = |view_proj: Mat4, clip: f32, flip: f32| FrameUniform {
            view_proj: view_proj.to_cols_array_2d(),
            camera: params.camera.position.extend(1.0).to_array(),
            forward: params.camera.forward().extend(0.0).to_array(),
            fog_value: if flip > 0.5 {
                [fog.value[0], fog.value[1], fog.value[2], 0.0]
            } else {
                [main_fog.value[0], main_fog.value[1], main_fog.value[2], 0.0]
            },
            fog_color: [fog.color[0], fog.color[1], fog.color[2], 1.0],
            clip: [clip, params.time, morph_alpha, flip],
            cull: [scale, if flip > 0.5 { reflection_min_size } else { main_min_size(params.opts) }, 0.0, 0.0],
            headlight: if flip > 0.5 { [[0.0; 4]; 5] } else { headlight },
            wet: [0.0; 4],
            lamps: [0.0; 4],
            cluster: [0.0; 4],
            cluster_near: [0.0; 4],
        };
        // 0x60F890: the dome sits at the camera, unrotated and unscaled in
        // views 1–3.
        let dome = Mat4::from_translation(params.camera.position).to_cols_array_2d();
        for &slot in &self.sky_instances {
            gpu.queue.write_buffer(&self.instances, slot as u64 * 64, bytemuck::cast_slice(&dome));
        }
        let clear = wgpu::Color { r: fog.color[0] as f64, g: fog.color[1] as f64, b: fog.color[2] as f64, a: 1.0 };

        let (reflecting, reflection) =
            self.plan_reflection(gpu, params, &visible, &uniform, view_proj, enhanced, scale, reflection_min_size);

        let rear = self.plan_rear(gpu, params, fog, morph_alpha);

        // Pass 2: the frame itself (its depth reversed with the Graphics
        // page's Reversed-Z, as its pipelines are). LOD Fade (clone
        // deviation): the CPU's per-instance fades decide the main view's
        // scenery (its frame's size need 0; `cull.w` the sample count the
        // dither covers); they are sized and culled with the native matrix.
        self.main_lod = main_lod;
        let far = far_sections(params, &visible);
        let (fades, main_visible, mid, far) =
            match self.update_fades(gpu, params, &visible, far.as_deref(), view_proj, scale) {
                Some((f, v, m, far)) => (Some(f), Some(v), Some(m), far),
                None => (None, None, None, far),
            };
        let scene_mode = self.scene_pipelines().mode;
        let main_proj = params.camera.view_projection_depth(params.aspect, scene_mode.reversed);
        self.core.sky_on.store(sky_ng::enabled(params.opts), std::sync::atomic::Ordering::Relaxed);
        self.prepare_sky_ng(gpu, params, scene_mode, main_fog);
        self.prepare_ambient(gpu, params, scene_mode, &visible, main_fog);
        let mut main_uniform = uniform(main_proj, if enhanced { 1.0 } else { f32::MIN }, 0.0);
        if fades.is_some() {
            main_uniform.cull = [scale, 0.0, 1.0, scene_mode.samples as f32];
        }
        // Wet Roads (clone deviation): the main view's worldreflect with
        // the weather's rain and wetness.
        // Only once rain has wetted the road (`Weather::road_wet`): a dry
        // road keeps the native path (docs/notes/graphics/reflections.md "Wet Roads and Screen-Space Reflections").
        // Wet Road Interaction works on Wet Roads' surface: its row
        // turns the surface on too.
        if let (true, Some(wet)) = (params.opts.wet_roads || params.opts.wet_interaction != 0, self.weather) {
            main_uniform.wet = wet_uniform(wet);
        }
        let (wet_mask, anim_mask) = self.plan_screen_masks(gpu, params, scene_mode, reflecting, &main_uniform, main_fog);
        // Dynamic Lamps / Lamp Shadows / Contact Shadows (clone deviations).
        // Headlight Beams: the player's beam is view 9's projector
        // whatever Customize Display's Car Headlight says; its shadow layer
        // follows it.
        let beam_headlight =
            if params.opts.headlight_beams { headlight_uniform(params.headlight_pools.first()) } else { headlight };
        let rt_tlas = self.prepare_rt_shadow(gpu, params, scene_mode, main_proj);
        let (lamp_frame, lamp_shadows) =
            self.plan_lamps(gpu, params, &visible, view_proj, &beam_headlight, &mut main_uniform);
        let rt_shadow = match (rt_tlas, self.lamp_rt_buffers(), self.rt_shadow.as_mut()) {
            (Some(tlas), Some((lights, clusters)), Some(r)) => {
                Some(r.plan(gpu, scene_mode.samples, &self.core.main_frame.0, &lights, &clusters, &tlas))
            }
            _ => None,
        };
        *self.rt_shadow_plan.lock().unwrap() = rt_shadow.clone();
        // Wet Road Interaction: its bindings in the main view's group.
        let lamp_frame = self.plan_wet_road(gpu, lamp_frame, params.opts);
        // The road's Wet Roads branch runs where wet.x > 0.5 (the frame's
        // value from here on): without it, the dry set.
        let dry_road = main_uniform.wet[0] <= 0.5;
        *self.core.lamp_main.write().unwrap() = lamp_frame.clone();
        // The main view's draw with the scene's pipelines.
        let lamp_frame = lamp_frame.map(|_| self.core.scene_group());
        gpu.queue.write_buffer(&self.main_frame.0, 0, bytemuck::bytes_of(&main_uniform));
        let reflection_group = match (&self.reflection, reflecting) {
            (Some(r), true) => r.bind_group.clone(),
            _ => self.no_reflection.clone(),
        };
        let fade_buffer = fades.as_ref().and(self.fade.as_ref()).zip(mid).map(|(f, m)| (f.buffer.clone(), m));
        let main = PassCull {
            planes: side_planes(view_proj).to_vec(),
            mirrored: false,
            eye: params.camera.position,
            forward: params.camera.forward(),
            scale,
            min_size: if fades.is_some() { 0.0 } else { main_min_size(params.opts) },
            fades,
        };
        ViewsPlan {
            visible,
            far,
            reflection,
            rear,
            main,
            clear,
            reflection_group,
            time: params.time,
            sky: params.sky,
            reversed: params.reversed,
            main_lod: self.main_lod,
            reflection_lod,
            pool_count: self.pool_count,
            reflection_glow_count: if reflecting { self.reflection_glow_count } else { 0 },
            fade: fade_buffer,
            main_visible,
            wet_mask,
            anim_mask,
            lamp_frame,
            lamp_shadows,
            hide_water: self.hide_water,
            rt_shadow,
            dry_road,
        }
    }

    /// [`WorldRenderer::plan_views`]' pass 1: the road reflection's frames and
    /// culling. -> (whether it is drawn, its culling and target).
    #[allow(clippy::too_many_arguments)]
    fn plan_reflection(
        &mut self,
        gpu: &Gpu,
        params: &FrameParams,
        visible: &[bool],
        uniform: &impl Fn(Mat4, f32, f32) -> FrameUniform,
        view_proj: Mat4,
        enhanced: bool,
        scale: f32,
        reflection_min_size: f32,
    ) -> (bool, Option<(PassCull, wgpu::TextureView, wgpu::TextureView)>) {
        // Pass 1: the reflection, if any road is on screen to receive it.
        let reflecting =
            params.road_reflections && self.sections.iter().any(|(s, _, reflect)| *reflect && visible[*s as usize]);
        let mut reflection = None;
        if reflecting {
            self.ensure_reflection_target(gpu, params.opts.road_reflection_size(REFLECTION_SIZE, self.view_size));
            self.upload_pools(gpu, params.headlight_pools);
            self.upload_reflection_glows(gpu, params.reflection_glows);
            // View 4 shares view 1's camera (0x5C30F6 points its +0x40 at
            // 0x8760E0, view 1's) and projection factors (0x5C7ED0), so it
            // is the main view's matrix unchanged. The mirroring is per
            // instance: 0x5D67B4 negates m22 of every matrix the view's
            // cull context has 0x800 for, flipping each prop about its own
            // origin; null-transform geometry is not flipped. Cleared to
            // opaque black (0x5BA3D0 with 1: 0xFF000000).
            let native_frame = uniform(view_proj, f32::MIN, 1.0);
            gpu.queue.write_buffer(&self.reflection_effect_frame.0, 0, bytemuck::bytes_of(&native_frame));
            let plane = params.road_reflection_plane.filter(|h| enhanced && h.is_finite());
            let (capture_proj, eye, forward) = if let Some(height) = plane {
                let mirror = road_plane_matrix(height);
                (
                    view_proj * mirror,
                    mirror.transform_point3(params.camera.position),
                    mirror.transform_vector3(params.camera.forward()),
                )
            } else {
                (view_proj, params.camera.position, params.camera.forward())
            };
            let capture_frame = if let Some(height) = plane {
                let mut frame = uniform(capture_proj, height, 2.0);
                frame.camera = eye.extend(1.0).to_array();
                frame.forward = forward.extend(0.0).to_array();
                frame
            } else {
                native_frame
            };
            gpu.queue.write_buffer(&self.reflection_frame.0, 0, bytemuck::bytes_of(&capture_frame));
            let target = self.reflection.as_ref().unwrap();
            // Native tests locally mirrored bounds against the main camera.
            // Planar capture instead tests original bounds against the reflected
            // camera/projection, matching the geometry submitted to its shader.
            let cull = PassCull {
                planes: side_planes(capture_proj).to_vec(),
                mirrored: plane.is_none(),
                eye,
                forward,
                scale,
                min_size: reflection_min_size,
                fades: None,
            };
            reflection = Some((cull, target.color.clone(), target.depth.clone()));
        }
        (reflecting, reflection)
    }

    /// [`WorldRenderer::plan_views`]' view 3 (the rear-view mirror): its frame
    /// and culling.
    fn plan_rear(&mut self, gpu: &Gpu, params: &FrameParams, fog: Fog, morph_alpha: f32) -> Option<(PassCull, wgpu::TextureView, wgpu::TextureView)> {
        // View 3, the rear-view mirror: its own camera (0x44BA80) and
        // projection (0x5C7ED0: factor 1 across, the target's h / w = 1
        // down), at the reflection's LOD and size threshold (cull base
        // 0x1000 has 0x1800: LOD C, or B for flag 0x0400; 32 pixels,
        // 0x5D672A..0x5D6747).
        let mut rear = None;
        if let Some(camera) = params.rear_view {
            self.ensure_rear_target(gpu);
            let view_proj = camera.view_projection(1.0);
            let scale = cull_scale(camera.fov_y);
            let frame = FrameUniform {
                view_proj: view_proj.to_cols_array_2d(),
                camera: camera.position.extend(1.0).to_array(),
                forward: camera.forward().extend(0.0).to_array(),
                fog_value: [fog.value[0], fog.value[1], fog.value[2], 0.0],
                fog_color: [fog.color[0], fog.color[1], fog.color[2], 1.0],
                clip: [f32::MIN, params.time, morph_alpha, 0.0],
                cull: [scale, LOW_LOD_MIN_SIZE, 0.0, 0.0],
                headlight: [[0.0; 4]; 5],
                wet: [0.0; 4],
                lamps: [0.0; 4],
                cluster: [0.0; 4],
                cluster_near: [0.0; 4],
            };
            gpu.queue.write_buffer(&self.rear_frame.0, 0, bytemuck::bytes_of(&frame));
            let target = self.rear.as_ref().unwrap();
            let cull = PassCull {
                planes: frustum_planes(view_proj),
                mirrored: false,
                eye: camera.position,
                forward: camera.forward(),
                scale,
                min_size: LOW_LOD_MIN_SIZE,
                fades: None,
            };
            rear = Some((cull, target.color.clone(), target.depth.clone()));
        }
        rear
    }

    /// Sky Next-Gen's frame (before the main view).
    fn prepare_sky_ng(&mut self, gpu: &Gpu, params: &FrameParams, scene_mode: SceneMode, main_fog: Fog) {
        // Sky Next-Gen (docs/notes/graphics/atmosphere.md): made the first frame the row is on, then prepared every frame.
        if sky_ng::enabled(params.opts) {
            let n = self
                .core
                .sky_ng
                .get_or_init(|| sky_ng::NightSky::new(gpu, &self.core.sky_rows, &self.frame_layout, REFLECTION_FORMAT));
            let rot_proj = params.camera.projection_depth(params.aspect, scene_mode.reversed) * {
                let mut v = params.camera.view();
                v.w_axis = glam::Vec4::W;
                v
            };
            n.prepare(
                gpu,
                &sky_ng::SkyParams {
                    view_proj: rot_proj,
                    eye: params.camera.position,
                    fog_value: main_fog.value,
                    fog_color: main_fog.color,
                    time: params.time,
                    set: params.sky.set,
                    morph: params.sky.morph,
                },
                scene_mode.target(self.scene_format),
                scene_mode.samples,
                scene_mode.reversed,
            );
            // The panorama's rows of the ticks since the last frame.
            n.plan_rows(gpu, &self.sky_ticks);
        }
        self.sky_ticks.clear();
    }

    /// Ambient Detail's draw for this frame.
    fn prepare_ambient(
        &mut self,
        gpu: &Gpu,
        params: &FrameParams,
        scene_mode: SceneMode,
        visible: &[bool],
        main_fog: Fog,
    ) {
        // Ambient Detail: in game (a weather given) with the row on.
        let ambient = match (self.ambient.as_mut(), self.weather) {
            (Some(a), Some((_, road_wet, _))) if params.opts.ambient_detail != 0 => a.prepare(
                gpu,
                &ambient_detail::Frame {
                    camera: params.camera,
                    aspect: params.aspect,
                    reversed: scene_mode.reversed,
                    format: scene_mode.target(self.scene_format),
                    samples: scene_mode.samples,
                    visible,
                    fog: params.fog.map(|f| (main_fog.value, f.color)),
                    time: params.time,
                    wet: road_wet,
                    lamps: &self.lamps,
                    modern: modern_light::enabled(params.opts),
                    tier: params.opts.ambient_detail,
                },
            ),
            _ => None,
        };
        *self.core.ambient_draw.lock().unwrap() = ambient;
    }

    /// The screen masks (the SSR mask, FSR 2's animation mask), the weather
    /// effects' view and the main fog. -> (the SSR mask, the animation mask).
    #[allow(clippy::too_many_arguments)]
    fn plan_screen_masks(
        &mut self,
        gpu: &Gpu,
        params: &FrameParams,
        scene_mode: SceneMode,
        reflecting: bool,
        main_uniform: &FrameUniform,
        main_fog: Fog,
    ) -> (Option<WetMaskPlan>, Option<AnimMaskPlan>) {
        // Screen-Space Reflections (clone deviation): the mask, the road's
        // when its reflection is drawn, the cars' over it (armed for the
        // car renderers: `car::CarShared::ssr_mask`).
        // SSR off, the mask's targets go too (only SSR reads them).
        if params.opts.ssr == 0 {
            self.wet_mask = None;
        }
        let wet_mask =
            (params.opts.ssr != 0 && self.weather.is_some() && self.view_size.0 > 0 && self.view_size.1 > 0)
                .then(|| {
                    let wet = main_uniform.wet[0] > 0.5;
                    self.plan_wet_mask(gpu, scene_mode.reversed, reflecting, wet, params.opts)
                });
        self.wet_mask_drawn = wet_mask.is_some();
        // FSR 2's animation mask (docs/notes/graphics/fsr2.md), the game's world
        // under an FSR 2 mode only; else nothing (and no targets).
        let fsr2_frame =
            params.opts.fsr2 && params.opts.temporal_frame && self.view_size.0 > 0 && self.view_size.1 > 0;
        if !fsr2_frame {
            self.anim_mask = None;
            self.anim_mask_time = None;
        }
        let anim_mask =
            fsr2_frame.then(|| self.plan_anim_mask(gpu, scene_mode.reversed, main_uniform.wet[0] > 0.5, params.time));
        self.anim_mask_drawn = anim_mask.is_some();
        self.main_fog = (main_fog.value, main_fog.color);
        // Only a view with a weather (the in-game main view, `set_weather`)
        // draws the weather effects: the original updates and draws the rain
        // in game mode 6 only (0x5CB40D, 0x5CB7EE), so the front end's garage
        // scenes have none, whatever weather a drive left published
        // (docs/formats/front-end.md, "Garage scenes: no weather").
        self.fx_view = (self.weather.is_some() && self.fx_sim.stepped_or(params.opts).weather_fx).then_some(weather_fx::View {
            camera: *params.camera,
            aspect: params.aspect,
            reversed: scene_mode.reversed,
            fog: main_fog.color,
        });
        gpu.car_shared().arm_ssr_mask(wet_mask.as_ref().map(|m| car::SsrMaskTargets {
            term: m.term.clone(),
            surface: m.surface.clone(),
            tint: m.tint.clone(),
            depth: m.depth.clone(),
            reversed: scene_mode.reversed,
        }));
        (wet_mask, anim_mask)
    }

    /// Modern + RT's traced shadows prepared. -> this frame's traced scene
    /// when they run.
    fn prepare_rt_shadow(
        &mut self,
        gpu: &Gpu,
        params: &FrameParams,
        scene_mode: SceneMode,
        main_proj: Mat4,
    ) -> Option<wgpu::Tlas> {
        // Modern + RT's traced shadows run after the pre-pass, from
        // this frame's traced scene; without them (no scene yet, no
        // pre-pass) the blocks are an empty 1 × 1 target: fallback shadows.
        let rt_on = scene_mode.modern == 2
            && self.view_size.0 > 0
            && self.view_size.1 > 0
            && depth_prepass(scene_mode)
            && self.scene_pipelines().depth.is_some();
        let rt_tlas = if rt_on { modern_light_bold::frame_rt().map(|f| f.tlas) } else { None };
        if scene_mode.modern == 2 {
            let size = if rt_tlas.is_some() { self.view_size } else { (0, 0) };
            self.rt_shadow.get_or_insert_with(|| rt_shadow::RtShadow::new(gpu)).prepare(
                gpu,
                size,
                self.screen_size,
                main_proj,
                params.camera.position,
                scene_mode.clear_depth(),
                params.time,
            );
        }
        rt_tlas
    }

    /// docs/notes/graphics/car-lights.md: the projector's cookie (HEADLIGHTS) and
    /// its sampler, for the car lights in the air.
    pub fn headlight_cookie(&self) -> (wgpu::TextureView, wgpu::Sampler) {
        self.headlight.clone()
    }

    /// Ambient Detail (docs/notes/graphics/ambient-detail.md): its props and state
    /// for this world, made the first time the row is on (a one-off
    /// placement, ~0.1 s for the city); nothing with the row off. Props
    /// made at the load or ahead of it, or kept from the last drive for this
    /// one, are kept only if the world still gives the same ones.
    pub fn ensure_ambient(&mut self, gpu: &Gpu, world: &World, opts: &RenderOptions) {
        if std::mem::take(&mut self.ambient_check) && self.ambient.as_ref().is_some_and(|a| !a.made_for(world)) {
            self.ambient = None;
        }
        if self.ambient.is_none() && opts.ambient_detail != 0 {
            self.ambient = Some(Box::new(ambient_detail::AmbientDetail::new(gpu, world)));
        }
    }

    /// A new drive takes this renderer with the props of the last drive (a
    /// world kept for the next drive: the drive's own, or one kept beside
    /// the menus after a car change), its barriers set up: those props are
    /// checked against the world as the drive has it, as props made ahead
    /// are (by the drive's warm-up, [`AmbientWarm::run`], and once more by
    /// its first frame, [`WorldRenderer::ensure_ambient`]), so a race whose
    /// barriers switch other road pieces on draws its own props, placed by
    /// the load. Where the props' surfaces are as before (the city: always)
    /// they are kept as they are.
    pub fn new_drive_ambient(&mut self) {
        self.ambient_check |= self.ambient.is_some();
    }

    /// Ambient Detail's state for this world, once made (the tests).
    pub fn ambient(&self) -> Option<&ambient_detail::AmbientDetail> {
        self.ambient.as_deref()
    }

    /// Ambient Detail's part of a drive's warm-up ([`AmbientWarm::run`]),
    /// taken out of the renderer so that a load can run it beside the
    /// rest ([`WorldRenderer::warm_drive_passes`]); put back with
    /// [`WorldRenderer::put_ambient`].
    pub fn take_ambient(&mut self, samples: u32, opts: &RenderOptions) -> AmbientWarm {
        let mode = SceneMode::driving(opts, samples);
        AmbientWarm {
            ambient: self.ambient.take(),
            check: std::mem::take(&mut self.ambient_check),
            target: (mode.target(self.scene_format), mode.samples, mode.reversed),
        }
    }

    /// [`WorldRenderer::take_ambient`]'s part, after its run.
    pub fn put_ambient(&mut self, warm: AmbientWarm) {
        self.ambient = warm.ambient;
        self.ambient_check = warm.check;
    }

    /// Wet Road Interaction's spray volume for `opts`, made ahead of the
    /// frame that first has a spray cloud (that frame made it: ~12 ms of
    /// its main thread, most of it the accumulators' zeros), on the load's
    /// warm-up beside the other jobs and beside a world loaded ahead. None
    /// when the row is off. A volume that has drawn nothing holds no
    /// state: one made here is the one the first cloud would make.
    pub fn spray_volume_ahead(gpu: &Gpu, opts: &RenderOptions) -> Option<wet_spray::SprayVolume> {
        (opts.wet_interaction != 0).then(|| wet_spray::SprayVolume::new(gpu, opts.wet_interaction))
    }

    /// Whether [`WorldRenderer::spray_volume_ahead`] has something to make
    /// for this renderer (the row on and no volume yet).
    pub fn wants_spray_volume(&self, opts: &RenderOptions) -> bool {
        opts.wet_interaction != 0 && self.spray_volume.is_none()
    }

    /// A volume made by [`WorldRenderer::spray_volume_ahead`], kept unless
    /// the renderer has one (a tier changed later is replaced by the frame
    /// as before, `prepare_weather_fx`).
    pub fn offer_spray_volume(&mut self, v: Option<wet_spray::SprayVolume>) {
        if self.spray_volume.is_none() {
            self.spray_volume = v;
        }
    }

    /// What a drive's first frame makes on first use, made now, at
    /// the drive's load (behind its loading screen): the main view's
    /// pipelines ([`WorldRenderer::warm`]), Light Probes' field and baker
    /// (the bake itself stays the frame's) and Ambient Detail's props
    /// ([`AmbientWarm::run`]). Each only with its row on; the objects are
    /// the ones the frame would make.
    pub fn warm_drive(&mut self, gpu: &Gpu, world: &World, samples: u32, opts: &RenderOptions) {
        self.warm_drive_passes(gpu, samples, opts);
        let mut ambient = self.take_ambient(samples, opts);
        ambient.run(gpu, world, opts);
        self.put_ambient(ambient);
    }

    /// [`WorldRenderer::warm_drive`] but for Ambient Detail's props (a
    /// load runs them beside it, [`WorldRenderer::take_ambient`]).
    pub fn warm_drive_passes(&mut self, gpu: &Gpu, samples: u32, opts: &RenderOptions) {
        self.warm(gpu, samples, opts);
        // The masks the main view draws for Screen-Space Reflections and
        // FSR 2, and the lamps' shadow layers' pipelines.
        let reversed = SceneMode::driving(opts, samples).reversed;
        if opts.ssr != 0 {
            self.wet_mask_pipelines_for(gpu, reversed, opts);
            if self.mask_from_main_device(gpu) {
                let _ = self.mask_mrt_for(gpu, reversed, opts);
            }
        }
        if opts.fsr2 && opts.temporal {
            self.anim_mask_pipelines_for(gpu, reversed);
        }
        // (Modern Lighting draws the lamps' shadow layers whatever the row.)
        if opts.lamp_shadows != 0 || modern_light::district(opts).is_some() {
            let _ = crate::render::lamps::shadow_pipelines(gpu, self.arrays);
        }
        // Wet Road Interaction's trail map (made with the first wet frame).
        if opts.wet_interaction != 0 {
            drop(crate::render::wet_road::WetRoad::new(gpu, opts.wet_interaction));
        }
        // Modern Lighting's traced shadows (Modern + RT): their pass, as
        // the first frame with the traced scene makes it.
        let mode = SceneMode::driving(opts, samples);
        if mode.modern == 2 {
            self.rt_shadow.get_or_insert_with(|| rt_shadow::RtShadow::new(gpu)).warm(gpu, mode.samples > 1);
        }
        // Sky Next-Gen: made the first frame the row is on, as there.
        if sky_ng::enabled(opts) {
            let mode = SceneMode::driving(opts, samples);
            self.core
                .sky_ng
                .get_or_init(|| sky_ng::NightSky::new(gpu, &self.core.sky_rows, &self.frame_layout, REFLECTION_FORMAT))
                .warm(gpu, mode.target(self.scene_format), mode.samples, mode.reversed);
        }
        // The rain's and spray's passes (made with the first rain).
        if opts.rain_detail != 0 || opts.spray || opts.wet_interaction != 0 {
            let mode = SceneMode::driving(opts, samples);
            self.weather_fx
                .get_or_insert_with(|| crate::render::weather_fx::WeatherFx::new(gpu))
                .warm(mode.target(self.scene_format), mode.samples);
        }
        if opts.light_probes {
            self.ensure_probe_baker(gpu);
        }
    }

    /// Ambient Detail Next-Gen's volume (tyre dust, steam) over `target`,
    /// with the other particles (as `smoke_cinematic::Cinematic::draw`).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_ambient(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        samples: u32,
        depth: &wgpu::TextureView,
        depth_samples: u32,
        viewport: Option<[f32; 4]>,
        opts: &RenderOptions,
    ) {
        if let Some(a) = self.ambient.as_ref().filter(|_| self.weather.is_some()) {
            a.draw_volume(gpu, encoder, target, format, samples, depth, depth_samples, viewport, opts.ambient_detail);
        }
    }

    /// The light packs' lamps (the dynamic lamps' list; Volumetric Fog).
    pub fn lamp_list(&self) -> &[lamps::Lamp] {
        &self.lamps
    }

    /// Wet Roads' weather for the frames that follow: the rain's amount
    /// (weather +0x1F0), the road's wetness from the rain
    /// (`Weather::road_wet`, 0..1) and the world clock (s, the ripples).
    pub fn set_weather(&mut self, rain: f32, road_wet: f32, clock: f32) {
        self.weather = Some((rain, road_wet, clock));
        // The Next-Gen sky's clouds follow the rain.
        sky_ng::set_rain(rain);
    }

    /// Animated Water (docs/notes/graphics/water.md): the main view leaves the water
    /// textures' draws to `render::water` while set (off: native).
    pub fn set_hide_water(&mut self, hide: bool) {
        self.hide_water = hide;
    }

    /// Whether the main view draws the placements' LOD B (World Detail
    /// 0–1, `Placement::mid_solid`) rather than their top LOD.
    pub fn main_lod_b(&self) -> bool {
        self.main_lod == MAIN_LOD_B
    }

    /// Screen-Space Reflections' mask as this frame's world drew it
    /// ([`WetMask`]; the cars add theirs after the car pass), or none.
    pub fn wet_mask(&self, opts: &RenderOptions) -> Option<ssr::Mask> {
        self.wet_mask.as_ref().filter(|_| self.wet_mask_drawn).map(|m| ssr::Mask {
            term: m.term.clone(),
            surface: m.surface.clone(),
            tint: m.tint.clone(),
            fog: Some(self.main_fog),
            enhanced: opts.enhanced_reflections,
            wet: (opts.wet_roads || opts.wet_interaction != 0)
                && self.weather.is_some_and(|w| wet_uniform(w)[0] > 0.5),
            depth: Some(m.depth.clone()),
        })
    }

    /// FSR 2's animation mask as this frame's world drew it
    /// ([`AnimMask`]: its colour, R animated / G ripples, and its depth),
    /// or none (not an FSR 2 frame).
    pub fn anim_mask(&self) -> Option<(wgpu::TextureView, wgpu::TextureView)> {
        self.anim_mask.as_ref().filter(|_| self.anim_mask_drawn).map(|m| (m.color.clone(), m.depth.clone()))
    }

    /// FSR 2's animation mask pipelines for `reversed`, made where
    /// missing (the frame's [`Self::plan_anim_mask`], and a drive's warm-up).
    fn anim_mask_pipelines_for(&mut self, gpu: &Gpu, reversed: bool) {
        let key = (reversed, self.arrays);
        if self.anim_mask_pipelines.as_ref().is_none_or(|(k, _)| *k != key) {
            let mode = gpu.world_shared().mode_with(&gpu.device, self.arrays, false, false, false);
            let make = |vertex, fragment| {
                make_anim_mask_pipeline(
                    &gpu.device,
                    &mode.layout,
                    &mode.shader,
                    vertex,
                    fragment,
                    reversed,
                    gpu.pipeline_cache(),
                )
            };
            self.anim_mask_pipelines = Some((
                key,
                [
                    make("vs_main", "fs_anim_mask"),
                    make("vs_main", "fs_anim_mask_alpha_test"),
                    make("vs_main", "fs_anim_mask_light"),
                    make("vs_reflect", "fs_anim_mask"),
                    make("vs_reflect", "fs_anim_mask_alpha_test"),
                    make("vs_reflect", "fs_anim_mask_light"),
                ],
            ));
        }
    }

    /// The animation mask's targets at the view's size and its
    /// pipelines (the native world module: vertex shaders as the main
    /// view's), made where missing.
    fn plan_anim_mask(&mut self, gpu: &Gpu, reversed: bool, ripples: bool, time: f32) -> AnimMaskPlan {
        let size = self.view_size;
        if self.anim_mask.as_ref().is_none_or(|m| m.size != size) {
            let make = |label: &str, format: wgpu::TextureFormat| {
                gpu.device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some(label),
                        size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            };
            self.anim_mask = Some(AnimMask {
                size,
                color: make("fsr2 animation mask", ANIM_MASK_FORMAT),
                depth: make("fsr2 animation mask depth", DEPTH_FORMAT),
            });
        }
        self.anim_mask_pipelines_for(gpu, reversed);
        // A gap of more than a second (a pause, a load): as no last frame.
        let last_time = self.anim_mask_time.filter(|&l| l <= time && time - l < 1.0);
        self.anim_mask_time = Some(time);
        let m = self.anim_mask.as_ref().unwrap();
        AnimMaskPlan {
            color: m.color.clone(),
            depth: m.depth.clone(),
            clear_depth: if reversed { 0.0 } else { 1.0 },
            ripples,
            last_time,
            pipelines: self.anim_mask_pipelines.as_ref().unwrap().1.clone(),
        }
    }

    /// The SSR mask's pipelines for `reversed` and `opts`, made where
    /// missing (the frame's [`Self::plan_wet_mask`], and a drive's warm-up).
    fn wet_mask_pipelines_for(&mut self, gpu: &Gpu, reversed: bool, opts: &RenderOptions) {
        // Surface Detail (docs/notes/graphics/surface-maps.md "SSR"): with the row on
        // (and the texture arrays its immediates need) the mask takes the
        // lamps' module, whose wet_mask reads the slot's material and map;
        // off, the native module as before.
        let surface = opts.surface_detail > 0 && self.arrays;
        if self.wet_mask_pipelines.as_ref().is_none_or(|(k, _)| *k != (reversed, surface)) {
            let mode = gpu.world_shared().mode_with(&gpu.device, self.arrays, false, surface, false);
            let make = |entry| {
                make_wet_mask_pipeline(&gpu.device, &mode.layout, &mode.shader, entry, reversed, gpu.pipeline_cache())
            };
            self.wet_mask_pipelines = Some((
                (reversed, surface),
                [make("fs_wet_mask"), make("fs_wet_mask_alpha_test"), make("fs_overlay_mask")],
            ));
        }
    }

    /// The mask's targets at the view's size and its pipelines, made
    /// where missing.
    fn plan_wet_mask(&mut self, gpu: &Gpu, reversed: bool, road: bool, wet: bool, opts: &RenderOptions) -> WetMaskPlan {
        let size = self.view_size;
        if self.wet_mask.as_ref().is_none_or(|m| m.size != size) {
            let make = |label: &str, format: wgpu::TextureFormat| {
                gpu.device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some(label),
                        size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            };
            let depth = make("ssr mask depth", DEPTH_FORMAT);
            let depth_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ssr mask road depth"),
                layout: &mask_road_depth_layout(&gpu.device),
                entries: &[wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&depth) }],
            });
            self.wet_mask = Some(WetMask {
                size,
                term: make("ssr mask term", WET_MASK_FORMAT),
                surface: make("ssr mask surface", WET_MASK_FORMAT),
                tint: make("ssr mask tint", WET_MASK_TINT_FORMAT),
                depth,
                depth_group,
            });
        }
        self.wet_mask_pipelines_for(gpu, reversed, opts);
        // The mask from the main view where it was measured to pay: the wet road (the dry one's term is a few
        // taps) on a discrete GPU (on the integrated GPU its main view's extra targets cost what the mask
        // pass saves, docs/notes/graphics/reflections.md "Wet Roads and Screen-Space Reflections").
        let mrt = if road && wet && self.mask_from_main_device(gpu) {
            self.mask_mrt_for(gpu, reversed, opts)
        } else {
            None
        };
        let m = self.wet_mask.as_ref().unwrap();
        WetMaskPlan {
            term: m.term.clone(),
            surface: m.surface.clone(),
            tint: m.tint.clone(),
            depth: m.depth.clone(),
            depth_group: m.depth_group.clone(),
            road,
            clear_depth: if reversed { 0.0 } else { 1.0 },
            pipelines: self.wet_mask_pipelines.as_ref().unwrap().1.clone(),
            mrt,
        }
    }

    /// Whether this renderer's device takes the mask from the main view: a discrete GPU (or a test's
    /// [`FORCE_MASK_FROM_MAIN`]); noted in [`MASK_FROM_MAIN_DEVICE`].
    fn mask_from_main_device(&mut self, gpu: &Gpu) -> bool {
        let discrete = *self
            .mask_from_main_discrete
            .get_or_insert_with(|| gpu.adapter.get_info().device_type == wgpu::DeviceType::DiscreteGpu);
        let on = discrete || FORCE_MASK_FROM_MAIN.load(std::sync::atomic::Ordering::Relaxed);
        MASK_FROM_MAIN_DEVICE.store(if on { 1 } else { 2 }, std::sync::atomic::Ordering::Relaxed);
        on
    }

    /// The mask from the main view's pipelines (docs/notes/graphics/reflections.md "The mask from the main view"), made
    /// where missing (the frame's [`Self::plan_wet_mask`], and a drive's warm-up): with the depth pre-pass at
    /// one sample (its EQUAL road pipelines are the ones that write the mask), else None (the mask pass
    /// works the road's term out itself, as before).
    fn mask_mrt_for(&mut self, gpu: &Gpu, reversed: bool, opts: &RenderOptions) -> Option<MaskMrt> {
        if PLAIN_SSR_MASK.load(std::sync::atomic::Ordering::Relaxed) {
            return None;
        }
        let scene = self.scene_pipelines();
        let mode = scene.mode;
        if scene.depth.is_none() || mode.samples != 1 || mode.coverage || depth_prepass_env() == Some(2) {
            return None;
        }
        let lod_fade = scene.fade.is_some();
        drop(scene);
        let surface = opts.surface_detail > 0 && self.arrays;
        let key = (mode, self.scene_format, lod_fade, reversed, surface);
        if self.mask_mrt_pipelines.as_ref().is_none_or(|(k, _)| *k != key) {
            let shared = gpu.world_shared();
            let road = [0u8, 1, 2].map(|f| {
                if f > 0 && !lod_fade {
                    HashMap::new()
                } else {
                    shared.mrt_pipelines(&gpu.device, self.arrays, mode, self.scene_format, f)
                }
            });
            let m = shared.mode_with(&gpu.device, self.arrays, false, surface, false);
            let make = |entry| {
                make_wet_mask_pipeline(&gpu.device, &m.layout, &m.shader, entry, reversed, gpu.pipeline_cache())
            };
            let kind = [make("fs_wet_mask_kind"), make("fs_wet_mask_kind_alpha_test")];
            let make_hidden = |entry| {
                make_wet_mask_hidden_pipeline(
                    &gpu.device,
                    &m.mask_hidden_layout,
                    &m.shader,
                    entry,
                    reversed,
                    gpu.pipeline_cache(),
                )
            };
            let hidden = [make_hidden("fs_wet_mask_hidden"), make_hidden("fs_wet_mask_hidden_alpha_test")];
            self.mask_mrt_pipelines = Some((key, MaskMrt { road, kind, hidden }));
        }
        self.mask_mrt_pipelines.as_ref().map(|(_, m)| m.clone())
    }
}

/// The mask pass working the road's term out itself, as before the main view wrote it (true): for the test
/// that compares the two (docs/notes/graphics/reflections.md "The mask from the main view").
pub static PLAIN_SSR_MASK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Frames whose main view wrote the SSR mask's term and surface (for the tests).
pub static MASK_FROM_MAIN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The mask from the main view on any device (true), not only a discrete GPU: for the tests and the
/// measurements.
pub static FORCE_MASK_FROM_MAIN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the last renderer to plan an SSR mask was on a device that takes the mask from the main view
/// (1), or not (2); 0 none yet. For the tests.
pub static MASK_FROM_MAIN_DEVICE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// [`PLAIN_SSR_MASK`] (renderers' frames from now on).
pub fn set_plain_ssr_mask(on: bool) {
    PLAIN_SSR_MASK.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// [`MASK_FROM_MAIN`].
pub fn ssr_mask_from_main() -> u64 {
    MASK_FROM_MAIN.load(std::sync::atomic::Ordering::Relaxed)
}

/// The mask from the main view's pipelines ([`WorldRenderer::mask_mrt_for`]).
#[derive(Clone)]
pub(in crate::render) struct MaskMrt {
    /// The main view's opaque road (EQUAL after the pre-pass) by LOD Fade's form: none, the fades read,
    /// dithered.
    pub(in crate::render) road: [HashMap<PipelineKey, wgpu::RenderPipeline>; 3],
    /// The mask pass's road then: its depth and kind only (plain, alpha-tested).
    pub(in crate::render) kind: [wgpu::RenderPipeline; 2],
    /// And the road the main view hides: its term and surface (plain, alpha-tested).
    pub(in crate::render) hidden: [wgpu::RenderPipeline; 2],
}

// Refactor: from render/mod.rs
/// Screen-Space Reflections' mask (clone deviation, docs/notes/graphics/reflections.md "Wet Roads and Screen-Space Reflections"):
/// the main view's worldreflect draws once more, after the main view, into
/// targets of the scene's size with a depth of their own (the road
/// against the road only; [`ssr`]'s composite drops pixels something else
/// covers): the reflection term and its weight × fog, and the surface
/// (normal x, y, roughness, view depth).
pub(in crate::render) struct WetMask {
    pub(in crate::render) size: (u32, u32),
    pub(in crate::render) term: wgpu::TextureView,
    pub(in crate::render) surface: wgpu::TextureView,
    /// The term's per-channel weight and the surface's kind (road 0; the
    /// cars', drawn over the road's after the car pass: car_ssr.wgsl).
    pub(in crate::render) tint: wgpu::TextureView,
    pub(in crate::render) depth: wgpu::TextureView,
    /// `depth` read by the hidden road's pass ([`mask_road_depth_layout`]).
    pub(in crate::render) depth_group: wgpu::BindGroup,
}

/// The mask's tint format.
pub(in crate::render) const WET_MASK_TINT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The mask's colour format.
pub(in crate::render) const WET_MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// docs/notes/graphics/fsr2.md "Animation mask" (clone deviation, FSR 2 only):
/// the main view's draws whose texture moves on a still surface — the
/// flip-books (`TextureRef::Animated`: blinking signs), the UV scrolls
/// (scroll kinds 1 and 2: the chevron barriers step every 0.2 s) and the
/// water, and the road while Wet Roads has wetted it (the ripples) — drawn
/// once more after the main view into a target of the scene's size with a
/// depth of its own: R the reactive value, G the transparency-and-
/// composition value ([`anim_values`]). FSR 2's motion pass drops a mark
/// that lies behind the final scene's surface (a car in front of a
/// barrier).
pub(in crate::render) struct AnimMask {
    pub(in crate::render) size: (u32, u32),
    pub(in crate::render) color: wgpu::TextureView,
    pub(in crate::render) depth: wgpu::TextureView,
}

/// The animation mask's format (R reactive, G composition).
pub(in crate::render) const ANIM_MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg8Unorm;

/// How a draw's texture moves this frame ([`WorldCore::anim_motion`]).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AnimMotion {
    /// Its texture stepped since the last drawn frame (a flip-book's next
    /// frame, a stepped scroll's next step; or no last frame).
    Stepped,
    /// It moves every frame (a continuous scroll, the water).
    Continuous,
    /// A stepped texture between its steps.
    Still,
    /// The wet road (its ripples).
    Ripples,
}

/// A mask draw's (reactive, composition) values (FSR 2's guide: an
/// animated texture is reactive where it changes and kept out of locks and
/// the wide box; the history of a stepped texture is replaced at the step
/// and accumulates between steps).
pub fn anim_values(motion: AnimMotion) -> [f64; 2] {
        match motion {
        AnimMotion::Stepped => [debug_env::nfsu2_fsr2_step_reactive().unwrap_or(1.0), debug_env::nfsu2_fsr2_anim_tc().unwrap_or(1.0)],
        AnimMotion::Continuous => [debug_env::nfsu2_fsr2_anim_reactive().unwrap_or(0.5), debug_env::nfsu2_fsr2_anim_tc().unwrap_or(1.0)],
        AnimMotion::Still => [debug_env::nfsu2_fsr2_still_reactive().unwrap_or(0.0), debug_env::nfsu2_fsr2_anim_tc().unwrap_or(1.0)],
        AnimMotion::Ripples => [0.0, debug_env::nfsu2_fsr2_ripple_tc().unwrap_or(0.5)],
    }
}

/// Whether a texture animated at `fps` frames a second shows
/// another frame at `time` than at `last` (s).
pub(in crate::render) fn flip_stepped(fps: f32, time: f32, last: f32) -> bool {
    (time * fps) as i64 != (last * fps) as i64
}

/// A scroll's motion between `last` and `time` (s) from its
/// parameters (`scroll_params`: kind, speed u v, step): None for none.
pub(in crate::render) fn scroll_motion(scroll: &[f32; 8], time: f32, last: Option<f32>) -> Option<AnimMotion> {
    let (kind, step) = (scroll[0], scroll[3]);
    if kind != 1.0 && kind != 2.0 {
        return None;
    }
    if kind == 1.0 || step == 0.0 {
        return Some(AnimMotion::Continuous);
    }
    Some(match last {
        Some(l) if (time / step).trunc() == (l / step).trunc() => AnimMotion::Still,
        _ => AnimMotion::Stepped,
    })
}

/// What recording the animation mask needs ([`ViewsPlan::anim_mask`]).
pub(in crate::render) struct AnimMaskPlan {
    pub(in crate::render) color: wgpu::TextureView,
    pub(in crate::render) depth: wgpu::TextureView,
    pub(in crate::render) clear_depth: f32,
    /// The road's draws as ripples (Wet Roads has wetted the road).
    pub(in crate::render) ripples: bool,
    /// The last drawn frame's time (s), None after a gap.
    pub(in crate::render) last_time: Option<f32>,
    /// [world, road] × [alpha coverage, alpha-tested, light (additive
    /// between steps)].
    pub(in crate::render) pipelines: [wgpu::RenderPipeline; 6],
}

/// What recording the mask needs ([`ViewsPlan::wet_mask`]).
pub(in crate::render) struct WetMaskPlan {
    pub(in crate::render) term: wgpu::TextureView,
    pub(in crate::render) surface: wgpu::TextureView,
    pub(in crate::render) tint: wgpu::TextureView,
    pub(in crate::render) depth: wgpu::TextureView,
    /// `depth` for the hidden road's pass ([`WetMask::depth_group`]).
    pub(in crate::render) depth_group: wgpu::BindGroup,
    /// The road reflection is drawn (else the mask is only cleared, for
    /// the cars).
    pub(in crate::render) road: bool,
    /// The scene's depth convention (Reversed-Z clears to 0).
    pub(in crate::render) clear_depth: f32,
    /// worldreflect's mask, plain and alpha-tested; the overlays' flag
    /// ([`WorldCore::overlay_draws`]).
    pub(in crate::render) pipelines: [wgpu::RenderPipeline; 3],
    /// The term and the surface from the main view's road instead ([`MaskMrt`]), where the frame can.
    pub(in crate::render) mrt: Option<MaskMrt>,
}

/// Ambient Detail's part of a drive's warm-up ([`WorldRenderer::take_ambient`]):
/// the renderer's props, whether they are to be checked against the world
/// (made by a warm-up, or kept from the last drive for a new one, and not
/// drawn since), and the drive's main view (its format, samples, reversed
/// depth).
pub struct AmbientWarm {
    ambient: Option<Box<ambient_detail::AmbientDetail>>,
    check: bool,
    target: (wgpu::TextureFormat, u32, bool),
}

impl AmbientWarm {
    /// The props for `world` as it is now: those made by a warm-up before
    /// (the menus' preload makes them for the world as loaded) or kept from
    /// the last drive on this world ([`WorldRenderer::new_drive_ambient`])
    /// kept if the world still gives the same ones
    /// (`AmbientDetail::made_for`: on some drag, drift and URL tracks a
    /// race's scenery groups switch road pieces on; in the city no group
    /// holds a surface), else made again for it; made if there are none;
    /// and their pipelines for the drive's main view. With the row off,
    /// none made. Props already drawn in this drive (a Graphics row changed
    /// mid-drive) are left as they are.
    pub fn run(&mut self, gpu: &Gpu, world: &World, opts: &RenderOptions) {
        if self.check && self.ambient.as_ref().is_some_and(|a| !a.made_for(world)) {
            self.ambient = None;
        }
        if self.ambient.is_none() && opts.ambient_detail != 0 {
            self.ambient = Some(Box::new(ambient_detail::AmbientDetail::new(gpu, world)));
            self.check = true;
        }
        if let Some(a) = self.ambient.as_mut().filter(|_| opts.ambient_detail != 0) {
            let (format, samples, reversed) = self.target;
            a.warm(gpu, format, samples, reversed);
        }
    }
}

#[cfg(test)]
mod anim_mask_tests {
    use crate::render::*;

    /// docs/notes/graphics/fsr2.md "Animation mask": the chevron barriers' scroll
    /// (kind 2, a step every 0.2 s) is reactive on the frame it steps
    /// (and with no last frame) and only composition between its steps; a
    /// continuous scroll moves every frame; kinds 0 and 3 (a fixed offset)
    /// are no animation; a flip-book steps when its frame index changes.
    #[test]
    fn stepped_textures_are_reactive_at_their_steps_only() {
        let chevrons = [2.0, 0.0, -0.78125, 0.2, 0.0, 0.0, 0.0, 0.0];
        assert_eq!(scroll_motion(&chevrons, 10.21, Some(10.19)), Some(AnimMotion::Stepped));
        assert_eq!(scroll_motion(&chevrons, 10.25, Some(10.2333)), Some(AnimMotion::Still));
        assert_eq!(scroll_motion(&chevrons, 10.25, None), Some(AnimMotion::Stepped));
        assert_eq!(scroll_motion(&[1.0, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], 3.0, Some(2.98)), Some(AnimMotion::Continuous));
        assert_eq!(scroll_motion(&[2.0, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], 3.0, Some(2.98)), Some(AnimMotion::Continuous));
        assert_eq!(scroll_motion(&[0.0; 8], 3.0, Some(2.98)), None);
        assert_eq!(scroll_motion(&[3.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.0, 0.0], 3.0, Some(2.98)), None);
        // 15 frames a second at 60 Hz: one step in four frames
        let steps = (1..=60).filter(|&i| flip_stepped(15.0, i as f32 / 60.0, (i - 1) as f32 / 60.0)).count();
        assert_eq!(steps, 15);
        // the values: history replaced at a step, kept (out of the locks
        // and the wide box) between steps; the wet road composition only
        assert_eq!(anim_values(AnimMotion::Stepped), [1.0, 1.0]);
        assert_eq!(anim_values(AnimMotion::Still), [0.0, 1.0]);
        assert_eq!(anim_values(AnimMotion::Continuous), [0.5, 1.0]);
        assert_eq!(anim_values(AnimMotion::Ripples), [0.0, 0.5]);
    }

    /// The mask's fragment entry points are in every native world module
    /// form (its pipelines take the native module).
    #[test]
    fn anim_mask_entries_in_the_native_modules() {
        for arrays in [false, true] {
            for bound in [false, true] {
                let source = world_module_source(false, arrays, false, false, bound && arrays);
                for entry in ["fn fs_anim_mask(", "fn fs_anim_mask_alpha_test(", "fn fs_anim_mask_light(", "fn vs_main(", "fn vs_reflect("] {
                    assert!(source.contains(entry), "{entry}");
                }
                let module = wgpu::naga::front::wgsl::parse_str(&source).expect("parse");
                wgpu::naga::valid::Validator::new(wgpu::naga::valid::ValidationFlags::all(), wgpu::naga::valid::Capabilities::all()).validate(&module).expect("valid");
            }
        }
    }
}
