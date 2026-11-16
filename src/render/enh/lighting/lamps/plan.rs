use super::*;
impl WorldRenderer {
    /// The lamps' part of a frame's plan: the lights and clusters, the car
    /// occluders, the shadow layers to draw. Sets `main`'s lamp values and
    /// returns the main view's group 0 (None: all three options off, the
    /// native frame). `view_proj`: the main view's (forward depth).
    pub(in crate::render) fn plan_lamps(
        &mut self,
        gpu: &Gpu,
        params: &FrameParams,
        visible: &[bool],
        view_proj: Mat4,
        headlight: &[[f32; 4]; 5],
        main: &mut FrameUniform,
    ) -> (Option<wgpu::BindGroup>, Vec<ShadowPass>) {
        let opts = params.opts;
        let user_on = opts.dynamic_lamps;
        let user_quality = opts.lamp_shadows;
        // Modern Lighting (hidden switch): the clusters and the lamp
        // layers at High whatever the rows say; lamps.x = 2 below.
        let modern = crate::render::modern_light::district(opts);
        let on = user_on || modern.is_some();
        // The tier's layers (Low as Lamp Shadows Low, Modern / + RT 24 at 1024).
        let bold = modern.map(|_| {
            crate::render::modern_light_bold::Params::new(
                crate::render::modern_light_bold::tier(opts),
                crate::render::modern_light_bold::strength(opts),
            )
        });
        if bold.is_none() {
            crate::render::modern_light_bold::publish(&[], None);
        }
        let quality = match bold.map(|p| p.tier) {
            Some(1) => user_quality.max(1),
            Some(_) => 3,
            None => user_quality,
        };
        let contact = opts.contact_shadows;
        let beams = opts.headlight_beams;
        let surface = opts.surface_detail;
        if !on && quality == 0 && !contact && !beams && surface == 0 {
            *self.core.lamp_main_rt.write().unwrap() = None;
            return (None, Vec::new());
        }
        let _span = crate::prof::acc::span("world: lamps");
        let device = &gpu.device;
        let arrays = self.arrays;
        self.ensure_state(device);
        self.bind_surface_maps(gpu, surface);
        self.ensure_shadow_maps(gpu, quality, arrays);

        // The cars (the player's first: the headlight's own car).
        let occluders: Vec<GpuOccluder> = params.occluders.iter().take(MAX_OCCLUDERS).map(Occluder::gpu).collect();
        if crate::env_set!("NFSU2_LAMPS_LOG") {
            for (o, g) in params.occluders.iter().zip(&occluders) {
                log::warn!("occluder box {:?}..{:?} gpu {:?}", o.min, o.max, g);
            }
            log::warn!("headlight projector {:?}", headlight);
        }
        let eye = params.camera.position;
        let forward = params.camera.forward();
        let lamps = &self.lamps;
        let state = self.lamp.as_mut().unwrap();
        if !occluders.is_empty() {
            gpu.queue.write_buffer(&state.bindings.occluders, 0, bytemuck::cast_slice(&occluders));
        }
        // The lamps' fades (World Detail Ultra / Extreme with LOD Fade)
        // and the shadow layers handed out, once per world tick.
        let (g14, pop_guard, fresh) = step_lamp_ticks(params, state, lamps, visible, on, bold, eye, forward);
        let fades_ref: Option<&PopFades> = pop_guard.as_ref();
        let visible_section = |s: u32| match fades_ref {
            Some(f) => f.share(s) > 0.0,
            None => visible.get(s as usize).copied().unwrap_or(false),
        };

        let (passes, layer_of, headlight_layer) =
            plan_shadow_layers(gpu, params, state, lamps, fresh, g14, user_quality, headlight, eye, forward);

        // The clusters' bounds (docs/lamps-shadows.md, "Per-pixel work"):
        // the cars and lamps a pixel tests, culled per cluster.
        let view = ClusterView { view_proj, eye, forward };
        let spheres = cluster_spheres(view_proj, eye, forward);
        // The cars that can shade a cluster from the player's headlight
        // (Lamp Shadows: `headlight_visibility`, the player's beam).
        if user_quality > 0 {
            let at = Vec3::new(headlight[0][0], headlight[0][1], headlight[0][2]);
            let masks = headlight_masks(at, &occluders, &spheres, &view);
            gpu.queue.write_buffer(&state.bindings.clusters, (HEADLIGHT_CARS * 4) as u64, bytemuck::cast_slice(&masks));
        }
        // Contact Shadows' cars per cluster (`contact_visibility`; every
        // word 0 without cars).
        if contact {
            let masks = contact_masks(&occluders, &spheres, &view);
            gpu.queue.write_buffer(&state.bindings.clusters, (CONTACT_CARS * 4) as u64, bytemuck::cast_slice(&masks));
        }
        let first_beam = build_light_list(
            gpu,
            params,
            state,
            lamps,
            &occluders,
            &spheres,
            &view,
            &visible_section,
            &layer_of,
            fades_ref,
            modern,
            bold,
            beams,
            on,
            surface,
            view_proj,
            eye,
            forward,
        );
        let beam_list = upload_beams(gpu, params, state, beams, on, surface, bold, first_beam, eye, forward);
        write_frame_lamps(
            main,
            params,
            modern,
            user_on,
            contact,
            user_quality,
            &occluders,
            eye,
            headlight_layer,
            &beam_list,
            first_beam,
            surface,
        );
        // Modern + RT: group 0 with the traced scene (the scene's
        // pipelines are made with that layout).
        // (Kept beside the plain group: only the scene's pipelines take it,
        // `WorldCore::scene_group`; the SSR mask, the cover shadow and the
        // native pipelines bind the plain one.)
        let bindings = state.bindings.clone();
        let rt = (self.core.scene_pipelines().mode.modern == 2)
            .then(|| self.frame_group_rt(gpu, &self.core.main_frame.0, &bindings));
        let plain = self.lamp.as_ref().unwrap().main_group.clone();
        *self.core.lamp_main_rt.write().unwrap() = rt;
        (Some(plain), passes)
    }

    /// The lamps' state made on first use (buffers, placeholders, the main
    /// view's group).
    fn ensure_state(&mut self, device: &wgpu::Device) {
        if self.lamp.is_none() {
            let placeholder = LampBindings::placeholder(device);
            let bindings = LampBindings {
                // + 1: Modern Lighting's params record.
                lights: storage_buffer(
                    device,
                    "lamps",
                    ((MAX_LIGHTS + MAX_BEAMS + 1) * std::mem::size_of::<GpuLamp>()) as u64,
                ),
                clusters: storage_buffer(device, "lamp clusters", (CLUSTER_WORDS * 4) as u64),
                views: storage_buffer(
                    device,
                    "lamp shadow views",
                    (MAX_LAYERS * std::mem::size_of::<GpuShadowView>()) as u64,
                ),
                occluders: storage_buffer(
                    device,
                    "car occluders",
                    (MAX_OCCLUDERS * std::mem::size_of::<GpuOccluder>()) as u64,
                ),
                maps: placeholder.maps.clone(),
                sampler: placeholder.sampler.clone(),
                surface: placeholder.surface.clone(),
                surface_sampler: placeholder.surface_sampler.clone(),
                wet: self.wet_bindings(),
            };
            let main_group = self.frame_group(device, &self.core.main_frame.0, &bindings);
            self.lamp = Some(LampState {
                bindings,
                placeholder_maps: placeholder.maps,
                maps: None,
                main_group,
                surface_bound: false,
                gate: Default::default(),
            });
        }
    }

        // Surface Detail's normal maps (Materials + Normals): made on first
        // use in the background, uploaded as they arrive (all before the
        // first frame in shots and benches), bound once the array exists.
    fn bind_surface_maps(&mut self, gpu: &Gpu, surface: u8) {
        let device = &gpu.device;
        if surface >= 2 && self.surface_maps.is_none() && !self.surface_jobs.is_empty() {
            self.surface_maps = Some(crate::render::surface::SurfaceMaps::new(
                device,
                &gpu.queue,
                std::mem::take(&mut self.surface_jobs),
            ));
        }
        if surface >= 2
            && let Some(m) = self.surface_maps.as_mut()
        {
            m.pump(&gpu.queue, &self.core.surface_slots, crate::surface_maps::blocking());
            let state = self.lamp.as_mut().unwrap();
            if !state.surface_bound {
                state.bindings.surface = m.view.clone();
                state.surface_bound = true;
                let bindings = state.bindings.clone();
                let group = self.frame_group(device, &self.core.main_frame.0, &bindings);
                self.lamp.as_mut().unwrap().main_group = group;
            }
        }
    }

        // The shadow layers at this quality (remade when it changed).
    fn ensure_shadow_maps(&mut self, gpu: &Gpu, quality: u8, arrays: bool) {
        let device = &gpu.device;
        let wanted = crate::render::lamps::shadow_layout(quality);
        let have = self.lamp.as_ref().unwrap().maps.as_ref().map(|m| m.quality);
        if have != wanted.map(|_| quality) {
            let maps = wanted.map(|(size, lamp_layers)| {
                let layers = lamp_layers + 1;
                let texture = map_array(device, size, layers);
                let state = self.lamp.as_ref().unwrap();
                let placeholder = LampBindings { maps: state.placeholder_maps.clone(), ..state.bindings.clone() };
                let frames = (0..layers)
                    .map(|_| {
                        let buffer = crate::render::zeroed_buffer(
                            device,
                            &wgpu::BufferDescriptor {
                                label: Some("lamp shadow frame"),
                                size: std::mem::size_of::<FrameUniform>() as u64,
                                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                                mapped_at_creation: false,
                            },
                        );
                        let group = self.frame_group(device, &buffer, &placeholder);
                        (buffer, group)
                    })
                    .collect();
                let views = (0..layers)
                    .map(|i| {
                        texture.create_view(&wgpu::TextureViewDescriptor {
                            dimension: Some(wgpu::TextureViewDimension::D2),
                            base_array_layer: i,
                            array_layer_count: Some(1),
                            ..Default::default()
                        })
                    })
                    .collect();
                (
                    array_view(&texture),
                    ShadowMaps {
                        quality,
                        layers: views,
                        frames,
                        holders: vec![None; lamp_layers as usize],
                        views: vec![GpuShadowView { view_proj: [[0.0; 4]; 4], eye: [0.0; 4] }; layers as usize],
                        pipelines: shadow_pipelines(gpu, arrays),
                        drawn: vec![0; layers as usize],
                        draws: 0,
                    },
                )
            });
            let state = self.lamp.as_mut().unwrap();
            state.bindings.maps = maps.as_ref().map_or_else(|| state.placeholder_maps.clone(), |m| m.0.clone());
            state.maps = maps.map(|m| m.1);
            let bindings = state.bindings.clone();
            let group = self.frame_group(device, &self.core.main_frame.0, &bindings);
            self.lamp.as_mut().unwrap().main_group = group;
        }
    }
}

/// docs/uncapped-fps.md, "Enhancement effects on ticks": the
/// lamps' section fades (World Detail Ultra / Extreme with LOD Fade) and
/// the shadow layers' hand-out (at most [`NEW_LAYERS_PER_FRAME`] new
/// layers a step), stepped once per world tick with that tick's eye,
/// sections and time (`render::enh::tick_views`); without published ticks
/// once per draw with the draw's, as before. -> (the fades on, the fades
/// held locked, the layers handed out this frame).
#[allow(clippy::too_many_arguments)]
fn step_lamp_ticks(
    params: &FrameParams,
    state: &mut LampState,
    lamps: &[Lamp],
    visible: &[bool],
    on: bool,
    bold: Option<crate::render::modern_light_bold::Params>,
    eye: Vec3,
    forward: Vec3,
) -> (bool, std::sync::MutexGuard<'static, Option<PopFades>>, Vec<usize>) {
    use crate::render::enh::tick_views::Due;
    let g14 = params.opts.pop_free && params.opts.lod_fade;
    let mut pop_guard = POP_FADES.lock().unwrap();
    if !g14 {
        *pop_guard = None;
    }
    let frame = crate::render::enh::tick_views::frame();
    // Each step with its tick's pop-in fixes and LOD Fade (a step per
    // draw: the frame's).
    let steps: Vec<(std::borrow::Cow<[bool]>, f32, Vec3, Vec3, bool)> = match state.gate.due(frame.as_deref()) {
        Due::Draw => vec![(std::borrow::Cow::Borrowed(visible), params.time, eye, forward, g14)],
        Due::Ticks(views) => views
            .iter()
            .map(|v| {
                let own = v.camera.position.to_array().map(f32::to_bits) == eye.to_array().map(f32::to_bits)
                    && v.sections.as_deref() == params.visible_sections;
                (
                    if own {
                        std::borrow::Cow::Borrowed(visible)
                    } else {
                        std::borrow::Cow::Owned(crate::render::native::world::plan::visible_of(v.sections.as_deref()))
                    },
                    v.time,
                    v.camera.position,
                    v.camera.forward(),
                    v.opts.pop_free && v.opts.lod_fade,
                )
            })
            .collect(),
    };
    let mut fresh: Vec<usize> = Vec::new();
    for (vis, time, e, f, step_g14) in &steps {
        if *step_g14 {
            pop_guard.get_or_insert_with(PopFades::default).step(vis, *time, *e);
        }
        let fades_ref = pop_guard.as_ref();
        let visible_section = |s: u32| match fades_ref {
            Some(f) => f.share(s) > 0.0,
            None => vis.get(s as usize).copied().unwrap_or(false),
        };
        for l in assign_shadow_layers(state, lamps, &visible_section, on, bold, *step_g14, *time, *e, *f) {
            if !fresh.contains(&l) {
                fresh.push(l);
            }
        }
    }
    (g14, pop_guard, fresh)
}

/// One step of the shadow layers' hand-out: the nearest downward lamps
/// ahead of (or around) `eye` keep or take a layer (at most the tier's
/// budget new ones); a layer handed out fades in from `time`. -> the
/// layers handed out.
#[allow(clippy::too_many_arguments)]
fn assign_shadow_layers(
    state: &mut LampState,
    lamps: &[Lamp],
    visible_section: &impl Fn(u32) -> bool,
    on: bool,
    bold: Option<crate::render::modern_light_bold::Params>,
    g14: bool,
    time: f32,
    eye: Vec3,
    forward: Vec3,
) -> Vec<usize> {
    let Some(m) = state.maps.as_mut() else { return Vec::new() };
    let lamp_layers = m.holders.len();
    // The nearest downward lamps ahead of (or around) the eye.
    let mut near: Vec<(f32, usize)> = if on {
        lamps
            .iter()
            .enumerate()
            .filter(|(_, l)| visible_section(l.section) && casts_down(&l.light))
            .filter_map(|(i, l)| {
                let d = l.light.position - eye;
                let dist = d.length() - l.light.outer * 0.5;
                let reach = if bold.is_some() { crate::render::modern_light_bold::SHADOW_REACH } else { SHADOW_REACH };
                (dist < reach && d.dot(forward) > -l.light.outer).then_some((dist, i))
            })
            .collect()
    } else {
        Vec::new()
    };
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    let wanted: Vec<usize> = near.iter().take(lamp_layers).map(|n| n.1).collect();
    let fresh = assign_layers(
        &mut m.holders,
        &wanted,
        if bold.is_some_and(|p| p.tier >= 2) { 2 * NEW_LAYERS_PER_FRAME } else { NEW_LAYERS_PER_FRAME },
    );
    // A shadow handed out now fades in.
    let mut sh = POP_SHADOWS.lock().unwrap();
    if g14 {
        let map = sh.get_or_insert_with(Default::default);
        map.retain(|l, t| m.holders.contains(&Some(*l)) && time - *t < SHADOW_FADE_SECONDS && time >= *t);
        for &layer in &fresh {
            if let Some(l) = m.holders[layer] {
                map.insert(l, time);
            }
        }
    } else {
        *sh = None;
    }
    fresh
}

/// The shadow layers handed out (`fresh`) drawn and every layer's frame
/// written. -> (the passes to draw, each lamp's layer, the headlight's
/// layer or −1).
#[allow(clippy::too_many_arguments)]
fn plan_shadow_layers(
    gpu: &Gpu,
    params: &FrameParams,
    state: &mut LampState,
    lamps: &[Lamp],
    fresh: Vec<usize>,
    g14: bool,
    user_quality: u8,
    headlight: &[[f32; 4]; 5],
    eye: Vec3,
    forward: Vec3,
) -> (Vec<ShadowPass>, std::collections::HashMap<usize, i32>, f32) {
    let mut passes = Vec::new();
    let mut headlight_layer = -1.0;
    let mut layer_of = std::collections::HashMap::new();
    if let Some(m) = state.maps.as_mut() {
        let lamp_layers = m.holders.len();
        // A layer handed out at one tick and again at a later one of the
        // same frame holds the later lamp: drawn once.
        let fresh: Vec<usize> = fresh.into_iter().filter(|&l| m.holders.get(l).is_some_and(|h| h.is_some())).collect();
        // Each lamp's layer (the holders are final here; read before
        // `frame_of` borrows the maps, so no per-frame copy of them).
        for (layer, h) in m.holders.iter().enumerate() {
            if let Some(h) = h {
                // Fresh layers wait until drawn (this frame's passes
                // run before the main view, so they count already).
                layer_of.insert(*h, layer as i32);
            }
        }
        let mut frame_of = |layer: usize, vp: Mat4, at: Vec3, dir: Vec3, fov: f32| {
            let uniform = FrameUniform {
                view_proj: vp.to_cols_array_2d(),
                camera: at.extend(1.0).to_array(),
                forward: dir.extend(0.0).to_array(),
                fog_value: [1.0e9, 0.0, 1.0, 0.0],
                fog_color: [0.0; 4],
                clip: [f32::MIN, params.time, 0.0, 0.0],
                cull: [crate::render::cull_scale(fov), crate::render::LOW_LOD_MIN_SIZE, 0.0, 0.0],
                headlight: [[0.0; 4]; 5],
                wet: [0.0; 4],
                lamps: [0.0; 4],
                cluster: [0.0; 4],
                cluster_near: [0.0; 4],
            };
            gpu.queue.write_buffer(&m.frames[layer].0, 0, bytemuck::bytes_of(&uniform));
            m.draws += 1;
            m.drawn[layer] = m.draws;
            m.views[layer] =
                GpuShadowView { view_proj: vp.to_cols_array_2d(), eye: at.extend(SHADOW_BIAS).to_array() };
            passes.push(ShadowPass {
                target: m.layers[layer].clone(),
                frame: m.frames[layer].1.clone(),
                cull: PassCull {
                    planes: crate::render::frustum_planes(vp),
                    mirrored: false,
                    eye: at,
                    forward: dir,
                    scale: crate::render::cull_scale(fov),
                    min_size: crate::render::LOW_LOD_MIN_SIZE,
                    fades: None,
                },
                pipelines: m.pipelines.clone(),
            });
        };
        for layer in fresh {
            let l = &lamps[m.holders[layer].unwrap()].light;
            // Pop meter: the lamp's shadow appears at once.
            if crate::render::pop_meter::enabled() && (l.position - eye).dot(forward) > 0.0 {
                let gap = ((l.position - eye).length() - l.outer).max(l.outer);
                crate::render::pop_meter::event(
                    crate::render::pop_meter::Source::LampShadow,
                    crate::render::pop_meter::px(
                        crate::render::cull_scale(70f32.to_radians()) * l.outer * 0.5 / gap,
                        params.opts,
                    ),
                    if g14 { SHADOW_FADE_SECONDS } else { 0.0 },
                );
            }
            let dir = if l.kind == 3 && l.direction.z < -0.5 { l.direction.normalize() } else { Vec3::NEG_Z };
            frame_of(layer, lamp_view(l), l.position, dir, 140f32.to_radians());
        }
        // The headlight's layer, every frame (the car moves).
        // (Not while only Modern Lighting holds the layers: nothing
        // reads the headlight's layer with the row off.)
        if user_quality > 0
            && let Some(vp) = headlight_view(headlight)
        {
            let at = Vec3::new(headlight[0][0], headlight[0][1], headlight[0][2]);
            let dir = Vec3::new(headlight[3][0], headlight[3][1], headlight[3][2]);
            frame_of(lamp_layers, vp, at, dir, 2.0 * (1.0 / headlight[2][3]).atan());
            headlight_layer = lamp_layers as f32;
        }
        gpu.queue.write_buffer(&state.bindings.views, 0, bytemuck::cast_slice(&m.views));
    }
    (passes, layer_of, headlight_layer)
}

/// The lights and their clusters (Dynamic Lamps, Surface Detail, Modern
/// Lighting) written. -> the first beam's index.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::ptr_arg)]
fn build_light_list(
    gpu: &Gpu,
    params: &FrameParams,
    state: &LampState,
    lamps: &Vec<Lamp>,
    occluders: &[GpuOccluder],
    spheres: &[(Vec3, f32)],
    view: &ClusterView,
    visible_section: &impl Fn(u32) -> bool,
    layer_of: &std::collections::HashMap<usize, i32>,
    fades_ref: Option<&PopFades>,
    modern: Option<([f32; 2], [f32; 2])>,
    bold: Option<crate::render::modern_light_bold::Params>,
    beams: bool,
    on: bool,
    surface: u8,
    view_proj: Mat4,
    eye: Vec3,
    forward: Vec3,
) -> usize {
    let mut first_beam = 0;
    // Surface Detail reads the clusters too (the lamps' specular
    // and their direction for the map's relief), lamps on or off.
    if on || surface > 0 {
        // With the Lighting row (or a strength over 0) the lamps,
        // the neon / signs and the cars' lights (index ≥ the lamps' count:
        // no layer; the cars' lights in every section).
        let all: Option<Vec<Lamp>> =
            bold.filter(|p| p.tier > 0 && (params.opts.modern_lighting != 0 || p.strength > 0.0)).map(|_| {
                let mut v = lamps.clone();
                v.extend(crate::render::modern_light_bold::loaded_emitters().iter().cloned());
                v.extend(crate::render::modern_light_bold::car_lights(
                    params.occluders,
                    params.headlight_pools,
                    beams,
                ));
                v
            });
        let list: &[Lamp] = all.as_deref().unwrap_or(lamps);
        let visible_or_car = |s: u32| s == u32::MAX || visible_section(s);
        // The clusters' bounds: a lamp listed only where its sphere reaches
        // (not with Modern Lighting Low, whose pixels take the first few
        // of a list, reached or not), and Lamp Shadows' cars per cluster.
        let tight = bold.is_none_or(|p| p.lights as usize >= MAX_LIGHTS);
        let mut clusters = build_in(
            list,
            &visible_or_car,
            view_proj,
            eye,
            forward,
            &|i| layer_of.get(&i).copied().unwrap_or(-1),
            Some(spheres).filter(|_| tight),
        );
        if let Some(d) = modern {
            if all.is_some() {
                crate::render::modern_light_bold::tag(&mut clusters.lights, &clusters.kept, list, d);
            } else {
                crate::render::modern_light::tag(&mut clusters.lights, &clusters.kept, lamps, d);
            }
        }
        apply_pop_fades(fades_ref, &mut clusters, list, lamps, params);
        if crate::render::pop_meter::enabled() {
            let kept: std::collections::HashMap<usize, f32> =
                clusters.kept.iter().map(|&k| (k, fades_ref.map_or(1.0, |f| f.share(list[k].section)))).collect();
            meter_lights(list, &|i| kept.get(&i).copied().unwrap_or(0.0), view_proj, eye, forward, params.opts);
        }
        if !clusters.lights.is_empty() {
            gpu.queue.write_buffer(&state.bindings.lights, 0, bytemuck::cast_slice(&clusters.lights));
        }
        crate::render::modern_light_bold::publish(&clusters.lights, bold);
        if let Some(p) = bold {
            let at = (crate::render::modern_light_bold::PARAMS_INDEX * std::mem::size_of::<GpuLamp>()) as u64;
            gpu.queue.write_buffer(&state.bindings.lights, at, bytemuck::bytes_of(&p.gpu()));
        }
        gpu.queue.write_buffer(&state.bindings.clusters, 0, bytemuck::cast_slice(&clusters.words));
        // The cars per cluster entry (Lamp Shadows: `lamp_light`'s
        // car_visibility; Modern Lighting: `ml_cars`, which always shades
        // with the cars). (Without cars every word is 0.)
        if params.opts.lamp_shadows > 0 || modern.is_some() {
            let masks = car_masks(&clusters.words, &clusters.lights, occluders, spheres, view);
            if !masks.is_empty() {
                gpu.queue.write_buffer(
                    &state.bindings.clusters,
                    ((CLUSTERS * 2 + LAMP_CAR_MASKS) * 4) as u64,
                    bytemuck::cast_slice(&masks),
                );
            }
        }
        first_beam = clusters.lights.len();
    }
    first_beam
}

/// The lamps' light at their section's share, a new shadow at its
/// strength.
fn apply_pop_fades(fades_ref: Option<&PopFades>, clusters: &mut Clusters, list: &[Lamp], lamps: &[Lamp], _params: &FrameParams) {
// The lamps' light at their section's share, a new shadow
// at its strength (the layer's fraction: world.wgsl reads
// 1 − fract(shadow.x) as the shadow's depth scale).
if let Some(f) = fades_ref {
    let sh = POP_SHADOWS.lock().unwrap();
    for (g, &k) in clusters.lights.iter_mut().zip(&clusters.kept) {
        let sec = list[k].section;
        if sec == u32::MAX {
            continue;
        }
        let share = f.share(sec);
        if share < 1.0 {
            for c in 0..3 {
                g.colour[c] *= share;
                g.shadow[c + 1] *= share;
            }
        }
        if g.shadow[0] >= 0.0
            && k < lamps.len()
            && let Some(&t) = sh.as_ref().and_then(|m| m.get(&k))
        {
            let strength = ((f.time() - t) / SHADOW_FADE_SECONDS).clamp(0.0, 1.0);
            if strength < 1.0 {
                g.shadow[0] = g.shadow[0].floor() + 0.999 * (1.0 - strength);
            }
        }
    }
}
}

/// Headlight Beams and the Next-Gen car lights written after the lamps.
/// -> them.
#[allow(clippy::too_many_arguments)]
fn upload_beams(
    gpu: &Gpu,
    params: &FrameParams,
    state: &LampState,
    beams: bool,
    on: bool,
    surface: u8,
    bold: Option<crate::render::modern_light_bold::Params>,
    first_beam: usize,
    eye: Vec3,
    forward: Vec3,
) -> Vec<GpuLamp> {
    // Headlight Beams: after the lamps.
    let mut beam_list = if beams { self::beams(params.headlight_pools, eye) } else { Vec::new() };
    // One volume: with the froxel medium on (any Volumetric Fog tier) the light it scatters out of the
    // beams into the air does not reach the road.
    if params.opts.volumetric_fog >= 1 {
        let t = crate::render::enh::volumes::medium::medium(crate::render::sky_ng::rain_amount()).beam_transmittance();
        for b in &mut beam_list {
            for c in &mut b.colour[..3] {
                *c *= t;
            }
        }
    }
    // Next-Gen car lights (docs/car-lights.md): the tail / brake
    // lights and the underglows after the beams (world.wgsl
    // `beam_light`, kinds 2 and 3), within the buffer before the
    // Modern Lighting params record.
    if params.opts.car_lights_next_gen {
        let modern_tails = (on || surface > 0)
            && bold.is_some_and(|p| p.tier > 0 && (params.opts.modern_lighting != 0 || p.strength > 0.0));
        let room = crate::render::modern_light_bold::PARAMS_INDEX.saturating_sub(first_beam + beam_list.len());
        let cars = crate::render::car_lights::cars(params.opts.car_lights_next_gen);
        beam_list.extend(
            crate::render::car_lights::world_glows(
                &cars,
                eye,
                forward,
                modern_tails,
                &crate::render::car_lights::look(),
            )
            .into_iter()
            .take(room),
        );
    }
    if !beam_list.is_empty() {
        gpu.queue.write_buffer(
            &state.bindings.lights,
            (first_beam * std::mem::size_of::<GpuLamp>()) as u64,
            bytemuck::cast_slice(&beam_list),
        );
    }
    beam_list
}

/// The main view's lamp values (`lamps`, `cluster`, `cluster_near`,
/// `cull.z`).
#[allow(clippy::too_many_arguments)]
fn write_frame_lamps(
    main: &mut FrameUniform,
    params: &FrameParams,
    modern: Option<([f32; 2], [f32; 2])>,
    user_on: bool,
    contact: bool,
    user_quality: u8,
    occluders: &[GpuOccluder],
    eye: Vec3,
    headlight_layer: f32,
    beam_list: &[GpuLamp],
    first_beam: usize,
    surface: u8,
) {
    // Modern Lighting: lamps.x = 2 + its car count (its relight in
    // place of Dynamic Lamps' add); lamps.z stays the row's (the headlight projector's
    // shadow and the native lamps' PCF follow Lamp Shadows as before).
    let x = if modern.is_some() {
        crate::render::modern_light::frame_x(&params.occluders[..occluders.len()], eye)
    } else if user_on {
        1.0
    } else {
        0.0
    };
    main.lamps = [x, if contact { CONTACT_DEPTH } else { 0.0 }, f32::from(user_quality), occluders.len() as f32];
    main.cluster = cluster_constants();
    main.cluster_near = [NEAR, headlight_layer, beam_list.len() as f32, first_beam as f32];
    // Surface Detail: the main view's cull.z (unused by the
    // native shaders): 1 Materials, 2 Materials + Normals.
    main.cull[2] = f32::from(surface.min(2));
    // NFSU2_SURFACE_DEBUG=1 (with Materials + Normals): the world shows
    // its shading normals (a review aid; docs/surface-maps.md).
    static DEBUG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if surface >= 2 && *DEBUG.get_or_init(|| crate::render::debug_env::nfsu2_surface_debug()) {
        main.cull[2] = 3.0;
    }
}
