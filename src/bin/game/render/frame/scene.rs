//! The world's 3D scene: one [`draw_world_scene`] for the
//! window's frame and the shots' stills. The order of its passes is data
//! ([`WORLD_PASSES`]); the two callers' differences are explicit: the
//! inputs ([`FrameInputs`]: the detail source, the cars' sets, the glow
//! gate, the time) and how the scene reaches the GPU ([`ScenePath`]: the
//! window's render threads and car batch, or a still's one encoder
//! submitted in place). Each difference keeps today's value at its caller
//! (a later logic change removes the shots' ones).

use crate::*;

/// One pass of the world's scene, in [`WORLD_PASSES`] order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorldPass {
    /// The frame's inputs (flare pools, reflection flares, tint, fog,
    /// occluders), the world's views, the native speed blur under its
    /// switch ([`native_speed_blur`]), the cars' environment cube.
    World,
    /// The cars (the player's, the companion, the rivals, the encounter
    /// racers, the traffic).
    Cars,
    /// The cars in the rear-view mirror (view 3).
    RearView,
    /// The race start scene's characters, under the ambient occlusion
    /// when it is on (it multiplies the lit scene only).
    CharactersUnderAo,
    /// The Graphics page's ambient occlusion.
    Ssao,
    /// The cars' shadow texture and underglow (an additive layer).
    Shadows,
    /// The characters after the shadows when the occlusion is off.
    CharactersOverShadows,
    /// The clone's motion blur and depth of field on the lit scene.
    BlurDof,
    /// The particles and the rain (additive).
    Particles,
    /// The glow sprites (additive).
    Glows,
    /// The horizon fog (0x5CB81C).
    HorizonFog,
    /// The temporal pass's motion inputs (the window's only).
    MotionInputs,
}

/// The world's scene: the lit scene (world, cars, characters), then the
/// ambient occlusion, motion blur and depth of field on it, then the
/// additive layers (shadows, particles, glows, horizon fog).
pub(crate) const WORLD_PASSES: [WorldPass; 12] = [
    WorldPass::World,
    WorldPass::Cars,
    WorldPass::RearView,
    WorldPass::CharactersUnderAo,
    WorldPass::Ssao,
    WorldPass::Shadows,
    WorldPass::CharactersOverShadows,
    WorldPass::BlurDof,
    WorldPass::Particles,
    WorldPass::Glows,
    WorldPass::HorizonFog,
    WorldPass::MotionInputs,
];

/// The native speed blur (0x5CB31F: after the world, before the cars):
/// under Customize Display's Motion Blur switch, left out only for the
/// clone's Camera / Per-Object tiers (`tier` is read only with the
/// switch on).
pub(crate) fn native_speed_blur(switches: Switches, tier: impl FnOnce() -> u8) -> bool {
    switches.motion_blur && tier() == 0
}

/// The settings the drawn tick-stepped state was stepped with: the
/// native visuals' switches (the flare pools' blink, the speed blur, the
/// horizon fog) and Motion Blur's tier, the particles' and Sparks &
/// Fire's tiers (the births recorded), Wet Road Interaction and the
/// weather rows (the spray, the trail map, the wet cars). A setting changed
/// between a tick and a frame is drawn from the next tick on, with the
/// state stepped with it (docs/developing/frame-settings.md).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Stepped {
    pub(crate) switches: Switches,
    pub(crate) motion_blur: u8,
    pub(crate) particles: u8,
    pub(crate) sparks_fire: u8,
    pub(crate) wet_interaction: u8,
    pub(crate) weather_fx: bool,
    pub(crate) lens_drops: bool,
}

impl Stepped {
    /// The tick's.
    pub(crate) fn of_tick(t: &crate::sim::TickSettings) -> Stepped {
        Stepped {
            switches: t.switches,
            motion_blur: t.motion_blur,
            particles: t.particles,
            sparks_fire: t.sparks_fire,
            wet_interaction: t.wet_interaction,
            weather_fx: t.weather_fx,
            lens_drops: t.lens_drops,
        }
    }

    /// The frame's (no tick stepped the state: a still).
    pub(crate) fn of_frame(s: &nfsu2::render::FrameSettings) -> Stepped {
        Stepped {
            switches: s.switches,
            motion_blur: s.opts.motion_blur,
            particles: s.opts.particles,
            sparks_fire: s.opts.sparks_fire,
            wet_interaction: s.opts.wet_interaction,
            weather_fx: s.opts.weather_fx,
            lens_drops: s.opts.lens_drops,
        }
    }
}

/// The values a world scene is drawn with. The shots' differences from
/// the window's frame are fields with the shots' values.
pub(crate) struct FrameInputs {
    pub(crate) aspect: f32,
    /// The scene's size and samples.
    pub(crate) size: (u32, u32),
    pub(crate) samples: u32,
    /// The ambient occlusion's texel scale (the scene's height over the
    /// output's).
    pub(crate) ssao_texel_scale: f32,
    /// The output's size (Modern + RT's traced shadows follow it when the
    /// scene is supersampled, `WorldRenderer::set_screen_size`).
    pub(crate) out_size: (u32, u32),
    /// The scene's clocks, the only time the draw reads
    /// (`render::clocks`; the window: the frame's between the ticks; a
    /// shot: the shaders' clock at 0, the blur's at 1 s, the world clock
    /// as the drive stands).
    pub(crate) clocks: crate::render::clocks::DrawClocks,
    /// The frame's settings (taken once per frame). The detail
    /// source of its `display` and `switches`: the window's from the
    /// settings' custom values, a shot's from the `NFS_DETAIL` presets.
    pub(crate) settings: nfsu2::render::FrameSettings,
    /// The window gates the flare pools, the reflection flares and the
    /// glow sprites on the Light Glow switch (and Road Reflection
    /// Detail); a shot (false) always makes them and draws its glows only
    /// in the post passes.
    pub(crate) gate_glow: bool,
    /// The paused race's cars under a menu (the window); a shot: false.
    pub(crate) paused_fallback: bool,
    /// The traffic in the headlight pools and in the glows (a shot: none).
    pub(crate) pool_traffic: TrafficSet,
    pub(crate) glow_traffic: TrafficSet,
    /// The HUD asked for the rear-view mirror this frame.
    pub(crate) rear_view: bool,
    /// Photo mode is open (the chase camera's own car stays drawn); a
    /// shot: false.
    pub(crate) photo_open: bool,
    pub(crate) aid_target: Option<Vec3>,
    pub(crate) rollers: Option<[glam::Mat4; 4]>,
    /// The camera's mover (the temporal key).
    pub(crate) mover: u32,
    /// A still's shutter (the motion blur's time is `clocks.blur`).
    pub(crate) still: Option<f64>,
    /// FSR 2 takes the motion blur and depth of field after it.
    pub(crate) blur_after_upscale: bool,
    /// The cars' renderers' target format.
    pub(crate) format: wgpu::TextureFormat,
    /// The settings the drawn tick state was stepped with ([`Stepped`];
    /// the window: the newest tick's; a shot: None, its own).
    pub(crate) stepped: Option<Stepped>,
}

impl FrameInputs {
    /// [`FrameInputs::stepped`], or the frame's own settings.
    pub(crate) fn stepped(&self) -> Stepped {
        self.stepped.unwrap_or_else(|| Stepped::of_frame(&self.settings))
    }
}

/// How the scene reaches the GPU.
pub(crate) enum ScenePath<'a> {
    /// The window's frame: the world's views and cube encoded on render
    /// threads, the cars queued into one batch pass, the scene submitted
    /// on a render thread.
    Window {
        cars: &'a mut nfsu2::render::car::CarBatch,
        belt: &'a mut Option<wgpu::util::StagingBelt>,
        encoded: Vec<nfsu2::render::Finishing>,
        temporal_on: bool,
        late_on: bool,
        jitter: glam::Vec2,
        smoke_format: wgpu::TextureFormat,
        late_format: wgpu::TextureFormat,
    },
    /// A shot's still: one encoder, the cars drawn one by one, the
    /// mirror's cars submitted in place, every cube face.
    #[cfg_attr(not(feature = "dev-harness"), allow(dead_code))]
    Still {
        /// After the mirror's cars are submitted (the shot's image dump).
        on_rear_view: &'a mut dyn FnMut(&Gpu, &WorldRenderer) -> Result<()>,
    },
}

/// What [`draw_world_scene`] draws with.
pub(crate) struct SceneCtx<'a> {
    pub(crate) gpu: &'a Gpu,
    pub(crate) post: &'a mut PostFx,
    pub(crate) color: &'a wgpu::TextureView,
    pub(crate) depth: &'a wgpu::TextureView,
    pub(crate) d: &'a mut Drive,
    /// The frame's camera (its own copy: the blend between the
    /// ticks, or the one written after the tick; the TAA jitter is set on
    /// it). The drive's own camera is not read or written by the scene.
    pub(crate) camera: &'a mut Camera,
    /// The drive's world renderer and the cars' (were in `Drive`
    /// and the sim's cars).
    pub(crate) render: &'a mut RenderState,
    pub(crate) game: SceneGame<'a>,
    pub(crate) encoder: wgpu::CommandEncoder,
    pub(crate) path: ScenePath<'a>,
}

/// What the scene leaves for the post passes.
pub(crate) struct SceneOut {
    pub(crate) game_frame: PostFrame,
    /// The world's views were drawn (a world renderer was up).
    pub(crate) drawn: bool,
    pub(crate) ssao: Option<nfsu2::render::ssao::Frame>,
    pub(crate) ssr: Option<nfsu2::render::ssr::Frame>,
    pub(crate) volumetric: Option<nfsu2::render::volumetric::Frame>,
    pub(crate) car_air: Option<nfsu2::render::car_lights::Frame>,
    pub(crate) temporal_frame: Option<nfsu2::render::temporal::FrameInput>,
    /// The window's scene, submitted on a render thread.
    pub(crate) submitted: Option<nfsu2::render::Submitted>,
    /// A still's mirror image.
    pub(crate) rear_image: Option<wgpu::TextureView>,
}

/// A command encoder of the window's frame (a GPU profiler gap first).
pub(crate) fn new_frame_encoder(gpu: &Gpu) -> wgpu::CommandEncoder {
    let mut e = gpu.device.create_command_encoder(&Default::default());
    nfsu2::prof::gpu::mark(&mut e, nfsu2::prof::gpu::GAP);
    e
}

/// The world's scene, pass by pass in [`WORLD_PASSES`] order, then its
/// submission.
pub(crate) fn draw_world_scene(ctx: SceneCtx<'_>, inp: &FrameInputs) -> Result<SceneOut> {
    let mut scene = WorldScene {
        c: ctx,
        inp,
        drawn: None,
        pools: Vec::new(),
        fog_drawn: None,
        rear_camera: None,
        ao_first: false,
        span: None,
        out: SceneOut {
            game_frame: PostFrame::GAME,
            drawn: false,
            ssao: None,
            ssr: None,
            volumetric: None,
            car_air: None,
            temporal_frame: None,
            submitted: None,
            rear_image: None,
        },
    };
    for pass in WORLD_PASSES {
        scene.run(pass)?;
    }
    Ok(scene.submit())
}

/// The world pass's lights: the drawn sections, the cars' headlight
/// pools, the flare pools and the reflection flares.
fn world_lights(
    d: &mut Drive,
    cam: &Camera,
    world_renderer: Option<&WorldRenderer>,
    game: &SceneGame<'_>,
    inp: &FrameInputs,
) -> (Option<Vec<u32>>, Vec<HeadlightPool>, Vec<nfsu2::particles::Quad>) {
    let (aspect, display, switches) = (inp.aspect, inp.settings.display, inp.settings.switches);
    // The drawn sections (Keep All Loaded widens them, a clone
    // deviation; else the native list), once for the frame: the
    // flare pools, the reflection flares, the views and the glows
    // all ask at the same eye.
    let drawn: Option<Vec<u32>> = d.world.drawn_sections_at(cam.position, inp.settings.opts.keep_loaded).map(|(_, l)| l.into_owned());
    let mut pools = {
        let cars = DrawnCars::of(game, CarSet { paused_race: inp.paused_fallback, traffic: inp.pool_traffic });
        headlight_pools(d, game.poses, cars.race, cars.traffic)
    };
    if let Some(c) = game.shop_scene.and_then(|run| run.companion.as_ref()) {
        pools.push(HeadlightPool { transform: game.poses.car(&c.id, &c.state).transform(), color: c.car.headlight_pool_color });
    }
    if !inp.gate_glow || inp.stepped().switches.light_glow {
        fill_flare_pools(d, game.poses.vis.blink, cam, world_renderer, display.world_lod_b, drawn.as_deref(), &inp.settings.opts);
        if let (Mode::Race(_) | Mode::Menu(_), Some(c)) = (&*game.mode, game.scene_crowd) {
            flag_girl_flares(d, c, game.poses.crowd_frame(c));
        }
    }
    let crowd = game.scene_crowd.filter(|_| matches!(*game.mode, Mode::Race(_) | Mode::Menu(_)));
    flag_girl_lights(d, crowd.map(|c| (c, game.poses.crowd_frame(c))));
    let reflection_quads = if !inp.gate_glow || (switches.light_glow && game.settings.custom[7] != 0) {
        reflection_glows(d, cam, aspect, drawn.as_deref())
    } else {
        Vec::new()
    };
    (drawn, pools, reflection_quads)
}

/// The world's moved placements, the GPS aid and the dyno's rollers.
#[allow(clippy::too_many_arguments)]
fn place_markers(
    r: &mut WorldRenderer,
    gpu: &Gpu,
    path: &ScenePath<'_>,
    posed: &std::collections::HashMap<usize, glam::Mat4>,
    world: &World,
    camera: &Camera,
    inp: &FrameInputs,
) {
    let span = nfsu2::prof::acc::span("world: moved placements");
    match path {
        ScenePath::Window { .. } => r.move_placements(gpu, posed.iter().map(|(&p, &t)| (p, t))),
        ScenePath::Still { .. } => {
            for (&placement, &transform) in posed {
                r.move_placement(gpu, placement, transform);
            }
        }
    }
    drop(span);
    // MARKER_GPS_AID (0x5F3CC0 → 0x5EACA0), as the shots place it.
    if let Some(p) = world.gps_aid {
        let (transform, flags) = gps_aid_transform(camera, inp.aid_target);
        r.update_placement(gpu, p, transform, flags);
    }
    // The dyno's rollers (0x56E900, drawn after the GPS aid).
    if let Some(first) = world.dyno_rollers {
        for (i, (transform, flags)) in roller_poses(inp.rollers.as_ref()).into_iter().enumerate() {
            r.update_placement(gpu, first + i, transform, flags);
        }
    }
}

/// The cars' environment cube (g_CarEnvironmentMapEnable).
///
/// docs/original/uncapped-fps.md, "Environment cube on ticks": the window's
/// faces follow the ticks, not the drawn frames. A frame renders, for
/// each tick since the cube's last one up to the tick it draws
/// (`env.drawn`), the faces due at that tick (0x5C397B, counter 0x870818 =
/// the tick's schedule index), each from that tick's eye (S_m: its car
/// and camera, `EnvEye`), oldest first; a frame with no new tick renders
/// none. The cube a frame samples is so a function of its tick alone, at
/// any display rate.
#[allow(clippy::too_many_arguments)]
fn environment_cube(
    r: &mut WorldRenderer,
    gpu: &Gpu,
    encoder: &mut wgpu::CommandEncoder,
    path: &mut ScenePath<'_>,
    params: &FrameParams<'_>,
    display: Display,
    state: &CarState,
    camera: &Camera,
    env: &render::snapshot::EnvFrame,
    photo_open: bool,
) {
    let axes = CameraAxes::look(camera.forward(), camera.up);
    // g_CarEnvironmentMapEnable: 0 leaves the cars a black
    // cube; else the ticks' faces (0x5C397B, counter 0x870818).
    if display.env_rate == 0 {
        r.clear_environment(encoder);
        r.env_tick = None;
    } else {
        match path {
            ScenePath::Window { encoded, .. } => {
                // The Graphics page's Reflection Update Rate "Every Frame"
                // (clone deviation): all six faces each tick.
                let due = nfsu2::render::display::env_faces_due(
                    display.env_rate,
                    params.opts.reflection_every_frame,
                    r.env_tick,
                    env.drawn,
                );
                r.env_tick = Some(r.env_tick.map_or(env.drawn, |t| t.max(env.drawn)));
                if due.is_empty() {
                    return;
                }
                let _span = nfsu2::prof::acc::span("world: environment cube");
                // The frame's own eye: the tick it draws when that is the
                // newest as the game holds it; also where a tick has no
                // captured eye (a harness frame stepped without a capture).
                let own = render::snapshot::EnvEye::of(env.drawn, state, camera, params.time);
                for (m, faces) in due {
                    let mut eye = if env.live && m == env.drawn { own } else { env.eye(m).copied().unwrap_or(own) };
                    // Photo mode's camera (written after the tick) orients
                    // the cube while it is open.
                    if photo_open {
                        eye.axes = (axes.right, axes.down, axes.forward);
                    }
                    let tick_params = FrameParams { time: eye.time, ..*params };
                    encoded.push(r.render_environment_async(
                        gpu,
                        eye.centre,
                        eye.axes,
                        &tick_params,
                        faces,
                        display.env_world,
                    ));
                }
            }
            // A shot is one frame: every face, as a first frame's cube.
            ScenePath::Still { .. } => r.render_environment(
                gpu,
                encoder,
                state.position + Vec3::Z * 0.95,
                (axes.right, axes.down, axes.forward),
                params,
                [true; 6],
                display.env_world,
            ),
        }
    }
}

/// The scene between its passes.
struct WorldScene<'a, 'i> {
    c: SceneCtx<'a>,
    inp: &'i FrameInputs,
    /// The drawn sections, once for the frame (the flare pools, the
    /// reflection flares, the views and the glows all ask at one eye).
    drawn: Option<Vec<u32>>,
    pools: Vec<HeadlightPool>,
    /// The fog the main view's world was drawn with (None with the fog
    /// off), for the effects drawn later (the volumetric smoke).
    fog_drawn: Option<nfsu2::fog::Fog>,
    rear_camera: Option<Camera>,
    ao_first: bool,
    /// The window's profiler span over several passes.
    span: Option<nfsu2::prof::acc::Acc>,
    out: SceneOut,
}

/// A GPU profiler mark, on the window's frame only.
fn mark(path: &ScenePath<'_>, encoder: &mut wgpu::CommandEncoder, label: &'static str) {
    if let ScenePath::Window { .. } = path {
        nfsu2::prof::gpu::mark(encoder, label);
    }
}

impl WorldScene<'_, '_> {
    fn run(&mut self, pass: WorldPass) -> Result<()> {
        match pass {
            WorldPass::World => self.world(),
            WorldPass::Cars => self.cars(),
            WorldPass::RearView => self.rear_view()?,
            WorldPass::CharactersUnderAo => {
                // docs/notes/graphics/car-look.md "Darkening settings against emissive light and the car's shadow": the Graphics page's ambient
                // occlusion multiplies the lit scene only — the world, the cars
                // and (drawn before the shadows then) the characters — before
                // the additive layers drawn from here on (the shadow texture and
                // underglow, particles, glow sprites, horizon fog).
                self.ao_first = self.out.ssao.is_some();
                if self.ao_first {
                    self.characters();
                }
            }
            WorldPass::Ssao => self.ssao(),
            WorldPass::Shadows => self.shadows(),
            WorldPass::CharactersOverShadows => {
                if !self.ao_first {
                    self.characters();
                }
            }
            WorldPass::BlurDof => self.blur_dof(),
            WorldPass::Particles => self.particles(),
            WorldPass::Glows => self.glows(),
            WorldPass::HorizonFog => self.horizon_fog(),
            WorldPass::MotionInputs => self.motion_inputs(),
        }
        Ok(())
    }

    fn world(&mut self) {
        let inp = self.inp;
        let SceneCtx { gpu, post, color, depth, d, camera, render, game, encoder, path } = &mut self.c;
        let (gpu, color, depth) = (*gpu, *color, *depth);
        let d: &mut Drive = d;
        let cam: &Camera = camera;
        let ps = game.poses.player();
        let (aspect, size, samples, display) = (inp.aspect, inp.size, inp.samples, inp.settings.display);
        let opts = &inp.settings.opts;
        let span = nfsu2::prof::acc::span("world: flare pools, reflection flares");
        let (drawn, pools, reflection_quads) = world_lights(d, cam, render.world.as_ref(), game, inp);
        drop(span);
        let span = nfsu2::prof::acc::span("world: tint and fog");
        // The tint, fog and blur as the ticks left them (`FrameVis`).
        let vis = game.poses.vis;
        self.out.game_frame = PostFrame::game(&vis.tint);
        // View 3, the rear-view mirror (0x44BA80), when the HUD asked
        // for it this frame.
        // The impact shake of the tick's views on the same slot (0x44BE7B).
        let rear_camera = inp.rear_view.then(|| {
            let mut c = nfsu2::rear_view::camera(ps.position, ps.orientation);
            nfsu2::chase::impact::shake_camera(&mut c, d.impact.applied);
            c
        });
        // World Detail Extreme (clone deviation): the far sections.
        let far = drawn.as_ref().and_then(|l| d.world.far_sections_at(cam.position, l, opts.far_draw));
        let cars = DrawnCars::of(game, CarSet { paused_race: inp.paused_fallback, traffic: TrafficSet::Live });
        let hidden = view_leaves_out_player(game.mode, d, game.shop_scene, inp.photo_open);
        let (occluders, traced_cars) = {
            let (companion, race, encounters, traffic) = cars.parts();
            publish_car_lights(d, cars.poses, companion, race, encounters, traffic, inp.clocks.seconds, opts);
            (
                car_occluders(d, cam, cars.poses, companion, race, encounters, traffic, inp.clocks.seconds, opts, hidden),
                rt_cars(cam.position, cars.poses, companion, race, encounters, traffic, opts),
            )
        };
        let params = FrameParams {
            camera: &cam,
            aspect,
            visible_sections: drawn.as_deref(),
            far_sections: far.as_deref(),
            fog: vis.fog,
            time: inp.clocks.seconds,
            // Road Reflection Detail (custom value 7, 0x870CD4: 0x5CAB25 skips
            // view 4 at 0).
            road_reflections: game.settings.custom[7] != 0,
            road_reflection_plane: (game.settings.custom[7] != 0 && opts.enhanced_reflections)
                .then(|| d.world.road_height_below(ps.position + Vec3::Z * 0.5))
                .flatten(),
            sky: d.time_of_day.sky(),
            reversed: matches!(&*game.mode, Mode::Race(r) if r.reversed),
            headlight_pools: &pools,
            reflection_glows: &reflection_quads,
            display,
            opts,
            rear_view: rear_camera.as_ref(),
            occluders: &occluders,
        };
        self.out.ssao = ssao_frame(&cam, aspect, depth, samples, params.fog.filter(|_| display.fog), &occluders, opts);
        self.fog_drawn = params.fog.filter(|_| display.fog);
        drop(span);
        if let Some(r) = render.world.as_mut() {
            place_markers(r, gpu, path, game.poses.posed(&d.posed), &d.world, &cam, inp);
            let span = nfsu2::prof::acc::span("world: road reflection, main view");
            // The Graphics page's Car Reflection Res and Road
            // Reflection Res (clone deviations) follow these.
            r.sync_environment_size(gpu, opts);
            r.set_screen_size(inp.out_size);
            if let ScenePath::Window { .. } = path {
                r.sync_texture_pack(gpu, &d.world);
                r.set_view_size(size);
                // (A shot sets its samples before its encoder.)
                r.set_samples(gpu, samples, opts);
            }
            // The wet roads' ripples on the world clock as drawn.
            r.set_weather(d.weather.amount, d.weather.road_wet, inp.clocks.world.unwrap_or(d.clock));
            // Ambient Detail: its props for this world, once the row is on.
            r.ensure_ambient(gpu, &d.world, opts);
            water_prepare(
                gpu,
                &mut d.effects.water,
                &d.world,
                r,
                &cam,
                aspect,
                params.visible_sections,
                params.fog.filter(|_| display.fog),
                display.env_rate != 0,
                params.reversed,
                // The tick's clock (the Next-Gen water's simulation steps
                // on it) and the drawn clock's lag behind it.
                d.clock,
                inp.clocks.water_lag(d.clock),
                d.weather.amount,
                opts,
            );
            nfsu2::render::wet_car::publish_weather(&d.weather, d.clock, inp.stepped().weather_fx);
            // The cars' wet values are blended by the frame's α.
            nfsu2::render::wet_car::publish_alpha(game.poses.alpha);
            prepare_modern_rt(gpu, &d.world, &d.region, (&d.car, ps.transform()), cam.position, &traced_cars, opts);
            match path {
                ScenePath::Window { encoded, .. } => encoded.push(r.render_async(gpu, color, depth, &params)),
                ScenePath::Still { .. } => r.render(gpu, encoder, color, depth, size, &params),
            }
            self.out.ssr = ssr_frame(&cam, aspect, depth, samples, r.wet_mask(opts), opts);
            attach_rt(
                &mut self.out.ssr,
                gpu,
                &d.world,
                &d.region,
                (&d.car, ps.transform()),
                cam.position,
                &traced_cars,
                opts,
            );
            self.out.volumetric = volumetric_frame(
                r,
                &cam,
                aspect,
                depth,
                samples,
                params.fog.filter(|_| display.fog),
                &pools,
                display.headlight,
                d.weather.amount,
                opts,
            );
            self.out.car_air = car_air_frame(
                r,
                &cam,
                aspect,
                depth,
                samples,
                params.fog.filter(|_| display.fog),
                d.weather.amount,
                inp.clocks.seconds,
                opts,
            );
            drop(span);
            self.out.drawn = true;
            // 0x5CB31F: motion blur after the world, before the cars.
            // The clone's Camera / Per-Object tiers replace it.
            let stepped = inp.stepped();
            if native_speed_blur(stepped.switches, || stepped.motion_blur)
                && let Some(blur) = motion_blur_frame(&vis.blur, &cam)
            {
                post.stage().motion_blur(gpu, encoder, blur);
                mark(path, encoder, "motion blur");
            }
            environment_cube(r, gpu, encoder, path, &params, display, ps, &cam, &game.poses.env, inp.photo_open);
            if let ScenePath::Window { .. } = path
                && let Some(cr) = render.cars.get_mut(&d.car_id)
            {
                cr.set_world_environment(gpu, r.environment());
            }
        }
        self.drawn = drawn;
        self.pools = pools;
        self.rear_camera = rear_camera;
    }

    fn cars(&mut self) {
        let inp = self.inp;
        let (aspect, samples, format, time) = (inp.aspect, inp.samples, inp.format, inp.clocks.seconds);
        let opts = &inp.settings.opts;
        let c = &mut self.c;
        let (gpu, color, depth) = (c.gpu, c.color, c.depth);
        let settings = c.game.settings;
        // The view's own car is left out (camera +0x280, 0x60D584).
        let hidden = view_leaves_out_player(c.game.mode, c.d, c.game.shop_scene, inp.photo_open);
        let d: &mut Drive = c.d;
        let cam: &Camera = c.camera;
        let ps = c.game.poses.player();
        let render: &mut RenderState = c.render;
        let encoder = &mut c.encoder;
        match &mut c.path {
            ScenePath::Window { cars: batch, belt, .. } => {
                self.span = nfsu2::prof::acc::span("cars");
                // The world's lights for the cars (the same camera for all).
                let lights = CarLighting::world_lights(&d.world, &cam, opts);
                if debug_env().car_draw_probe {
                    let cr = render.cars.get(&d.car_id);
                    eprintln!(
                        "car draw: hidden {hidden} kind {} back {} renderer {} lod {:?} car {:.2?} finite {} cam {:.2?} fwd {:.2?} fov {:.3?} nitrous {:?} ghost {:.2}",
                        d.chase.kind(),
                        d.chase.looking_back(),
                        cr.is_some(),
                        cr.map(|c| c.drawn_lod()),
                        ps.position,
                        ps.transform().is_finite(),
                        cam.position,
                        cam.forward(),
                        cam.game_fov,
                        ps.drivetrain.as_ref().map(|t| t.nitrous),
                        ps.ghost_timer,
                    );
                }
                if let Some(cr) = render.cars.get_mut(&d.car_id).filter(|_| !hidden) {
                    let vp = cam.view_projection_depth(aspect, opts.reversed_z);
                    car_lod_main(cr, &cam, settings, ps, &d.car, LodDraw::InGame, time, opts);
                    if debug_env().car_draw_probe {
                        eprintln!("car draw: picked {:?}", cr.drawn_lod());
                    }
                    if place_blink_drawn(cr, ps, &inp.clocks) {
                        cr.queue(
                            gpu,
                            batch,
                            vp,
                            cam.view(),
                            &lights.car(ps.transform()),
                            ps.transform(),
                            &ps.wheel_poses(&d.car),
                            opts,
                        );
                    }
                }
                // The companion, the opponents' cars (each with its own
                // renderer), career free roam's encounter racers on the
                // road, explore mode's or the race's traffic.
                let d: &Drive = d;
                let world_renderer = render.world.as_ref();
                let ahead = &mut render.cars_ahead;
                for_each_drawn_car(&c.game, &mut render.cars, inp.paused_fallback, |kind, id, renderer, car, state| {
                    let cr = renderer.get_or_insert_with(|| {
                        ahead.take(id, &crate::render::cars_ahead::BuildKey::now(gpu, format, opts)).unwrap_or_else(|| CarRenderer::new(gpu, car, format, opts))
                    });
                    if let Some(world) = world_renderer {
                        cr.set_world_environment(gpu, world.environment());
                    }
                    car_lod_main(cr, &cam, settings, state, car, kind.lod(), time, opts);
                    // World Detail Ultra / Extreme with LOD Fade — the
                    // traffic fades out toward the despawn ring, so a car
                    // placed again (or newly placed past it) never pops.
                    if kind == DrawnKind::Traffic && opts.pop_free && opts.lod_fade {
                        cr.set_presence(nfsu2::ai::traffic::ring_presence(state.position.distance(ps.position)));
                    }
                    if !place_blink_drawn(cr, state, &inp.clocks) {
                        return;
                    }
                    let vp = cam.view_projection_depth(aspect, opts.reversed_z);
                    cr.queue(
                        gpu,
                        batch,
                        vp,
                        cam.view(),
                        &lights.car(state.transform()),
                        state.transform(),
                        &state.wheel_poses(car),
                        opts,
                    );
                });
                // The queued cars in one pass, in the order queued above.
                let cars = car_pass_set(&render.cars, render.cars.get(&d.car_id), &c.game);
                batch.upload(gpu, belt.as_mut(), encoder);
                let mut pass = nfsu2::render::car::begin_pass(encoder, color, depth);
                for cr in &cars {
                    cr.record_queued(&mut pass, batch, opts);
                }
                drop(pass);
                // Enhanced Glass (clone deviation): the glass over the
                // scene's copy (nothing when off).
                nfsu2::render::car::record_glass_batch(gpu, encoder, batch, color, depth, &cars, opts);
                // Screen-Space Reflections (clone deviation): the cars into
                // the reflections' mask.
                nfsu2::render::car::record_ssr_masks(gpu, encoder, batch, &cars, opts);
                nfsu2::prof::gpu::mark(encoder, "cars");
            }
            ScenePath::Still { .. } => {
                let env = render.world.as_ref().unwrap().environment().clone();
                render.cars.get_mut(&d.car_id).unwrap().set_world_environment(gpu, &env);
                let vp = cam.view_projection_depth(aspect, opts.reversed_z);
                let cr = render.cars.get_mut(&d.car_id).unwrap();
                let lod = airborne_pick(car_lod(&cam, settings, ps.position, LodDraw::InGame, opts), cr, ps);
                cr.set_lod(lod);
                note_underside(cr, &cam, ps, &d.car, lod.filter(|_| !hidden));
                if !hidden && place_blink_drawn(cr, ps, &inp.clocks) {
                    render.cars.get_mut(&d.car_id).unwrap().render_sampled(
                        gpu,
                        samples,
                        encoder,
                        color,
                        depth,
                        vp,
                        cam.view(),
                        &CarLighting::in_world(&d.world, &cam, ps.transform(), opts),
                        ps.transform(),
                        &ps.wheel_poses(&d.car),
                        opts,
                    );
                }
                let d: &Drive = d;
                let ahead = &mut render.cars_ahead;
                for_each_drawn_car(&c.game, &mut render.cars, inp.paused_fallback, |kind, id, renderer, car, state| {
                    let cr = renderer.get_or_insert_with(|| {
                        ahead.take(id, &crate::render::cars_ahead::BuildKey::now(gpu, format, opts)).unwrap_or_else(|| CarRenderer::new(gpu, car, format, opts))
                    });
                    cr.set_world_environment(gpu, &env);
                    let lod = airborne_pick(car_lod(&cam, settings, state.position, kind.lod(), opts), cr, state);
                    cr.set_lod(lod);
                    note_underside(cr, &cam, state, car, lod);
                    if !place_blink_drawn(cr, state, &inp.clocks) {
                        return;
                    }
                    cr.render_sampled(
                        gpu,
                        samples,
                        encoder,
                        color,
                        depth,
                        vp,
                        cam.view(),
                        &CarLighting::in_world(&d.world, &cam, state.transform(), opts),
                        state.transform(),
                        &state.wheel_poses(car),
                        opts,
                    );
                });
            }
        }
    }

    fn rear_view(&mut self) -> Result<()> {
        let opts = &self.inp.settings.opts;
        let SceneCtx { gpu, d, render, game, encoder, path, .. } = &mut self.c;
        let gpu = *gpu;
        let d: &Drive = d;
        let render: &mut RenderState = render;
        match path {
            ScenePath::Window { encoded, .. } => {
                // The cars in the rear-view mirror (0x6316E0(view 3)), all but
                // the player's (camera +0x280).
                if let (Some(camera), Some(r)) = (self.rear_camera.as_ref(), render.world.as_ref())
                    && let (Some(color), Some(depth)) = (r.rear_view_image().cloned(), r.rear_view_depth().cloned())
                {
                    let env = r.environment().clone();
                    let cars = rear_view_list(game.poses, game.mode, game.encounters, game.traffic, &render.cars);
                    rear_view_cars(gpu, encoder, &color, &depth, camera, game.settings, &d.world, &env, &mut render.cars, cars, opts);
                    r.record_rear_view_frame(encoder);
                    nfsu2::prof::gpu::mark(encoder, "rear view cars");
                }
                // The cars (with the motion blur before them) are encoded on a
                // render thread from here.
                encoded.push(nfsu2::render::Finishing::start(std::mem::replace(encoder, new_frame_encoder(gpu))));
                drop(self.span.take());
                self.span = nfsu2::prof::acc::span("shadows, particles, glows, horizon fog");
            }
            ScenePath::Still { on_rear_view } => {
                // The cars in the rear-view mirror (view 3).
                if let Some(camera) = self.rear_camera.as_ref()
                    && let Some(r) = render.world.as_ref()
                    && let (Some(color), Some(depth)) = (r.rear_view_image().cloned(), r.rear_view_depth().cloned())
                {
                    // The main view's car uniforms go to the GPU first.
                    let done = std::mem::replace(encoder, gpu.device.create_command_encoder(&Default::default()));
                    gpu.queue.submit([done.finish()]);
                    let env = r.environment().clone();
                    let cars = rear_view_list(game.poses, game.mode, game.encounters, game.traffic, &render.cars);
                    rear_view_cars(gpu, encoder, &color, &depth, camera, game.settings, &d.world, &env, &mut render.cars, cars, opts);
                    r.record_rear_view_frame(encoder);
                    let done = std::mem::replace(encoder, gpu.device.create_command_encoder(&Default::default()));
                    gpu.queue.submit([done.finish()]);
                    on_rear_view(gpu, r)?;
                    self.out.rear_image = Some(color);
                }
            }
        }
        Ok(())
    }

    /// The race start scene's characters (`nfsu2::character`).
    fn characters(&mut self) {
        let aspect = self.inp.aspect;
        let SceneCtx { gpu, color, depth, camera, render, game, encoder, .. } = &mut self.c;
        let (gpu, color, depth) = (*gpu, *color, *depth);
        let cam: &Camera = camera;
        let sub = nfsu2::prof::acc::span("  characters");
        if let (Mode::Race(_) | Mode::Menu(_), Some(c), Some(r)) =
            (game.mode, game.scene_crowd, render.world.as_mut())
        {
            let SceneCrowd { set, origin, id, lifts, .. } = c;
            let frame = game.poses.crowd_frame(c);
            let renderer = render.crowds.slot(id);
            let cr = renderer.get_or_insert_with(|| nfsu2::render::characters::CharacterRenderer::new(gpu, r, set));
            cr.render(
                gpu,
                r,
                encoder,
                color,
                depth,
                cam.view_projection(aspect),
                set,
                frame / nfsu2::shop_scene::FPS,
                *origin,
                lifts,
            );
        }
        drop(sub);
    }

    fn ssao(&mut self) {
        let inp = self.inp;
        let SceneCtx { gpu, post, color, encoder, .. } = &mut self.c;
        if let Some(f) = self.out.ssao.take() {
            post.stage().ssao_into(gpu, encoder, color, inp.samples, inp.size, inp.ssao_texel_scale, &f);
        }
    }

    /// The cars' shadows (0x6162F0, queued in effect 13's lists), with
    /// g_CarShadowEnable (0x622113).
    fn shadows(&mut self) {
        let inp = self.inp;
        let SceneCtx { gpu, color, depth, d, camera, render, game, encoder, path, .. } = &mut self.c;
        let (gpu, color, depth) = (*gpu, *color, *depth);
        let (cam, ps): (&Camera, &CarState) = (camera, game.poses.player());
        let seconds = inp.clocks.seconds;
        let sub = nfsu2::prof::acc::span("  shadows");
        if let (true, Some(r)) = (inp.settings.display.shadows, render.world.as_mut()) {
            // The neon's clock (0x865188 × 0.00025): the clone's run time.
            let pull = nfsu2::shadow::shadow_pull(Some(native_mover_type(game.mode, Some(d))));
            // The view's own car gets no shadow or neon either: 0x6316E0's
            // per-car block skips it (`cmp [view+0x280], car; je`,
            // 0x631796 / 0x63179C) before both draws, 0x620AF0 / 0x6162F0
            // — the car list's test (0x60D584) that hides the body.
            let hidden = view_leaves_out_player(game.mode, d, game.shop_scene, inp.photo_open);
            let mut shadows: Vec<(u32, nfsu2::shadow::Shadow)> = if hidden {
                Vec::new()
            } else {
                car_shadow_in_game(gpu, r, &d.world, &d.car, ps, seconds, cam.position, pull, &inp.settings.opts)
            };
            let cars = DrawnCars::of(game, CarSet { paused_race: inp.paused_fallback, traffic: TrafficSet::Live });
            if let Some((car, state)) = cars.companion {
                shadows.extend(car_shadow_in_game(gpu, r, &d.world, car, state, seconds, cam.position, pull, &inp.settings.opts));
            }
            for (car, state) in cars.on_road() {
                shadows.extend(car_shadow_in_game(gpu, r, &d.world, car, state, seconds, cam.position, pull, &inp.settings.opts));
            }
            r.render_shadows(gpu, encoder, color, depth, &shadows_then_neons(shadows));
            mark(path, encoder, "shadows");
        }
        drop(sub);
    }

    /// The clone's motion blur and depth of field (docs/notes/graphics/motionblur-dof.md)
    /// on the lit scene, before the additive layers; a still's
    /// motion blur is the motion over the shutter taken from the
    /// velocities.
    fn blur_dof(&mut self) {
        let inp = self.inp;
        let SceneCtx { gpu, post, color, depth, d, camera, game, encoder, .. } = &mut self.c;
        let cars = DrawnCars::of(game, CarSet { paused_race: inp.paused_fallback, traffic: TrafficSet::Live });
        let key = temporal_key(game.mode, inp.mover, game.shop_scene.is_some());
        // Under FSR 2 (docs/notes/graphics/fsr2.md) after the upscaler, on the
        // display image (FSR 2's guide; below, before the late sprites).
        if !inp.blur_after_upscale {
            blur_and_dof(
                post.stage(),
                gpu,
                encoder,
                color,
                depth,
                inp.samples,
                inp.size,
                d,
                camera,
                cars.poses,
                inp.aspect,
                cars.parts(),
                key,
                inp.mover,
                inp.clocks.blur,
                inp.still,
                false,
                &inp.settings.opts,
            );
        }
    }

    /// The particles, over the world and the cars, and the rain.
    fn particles(&mut self) {
        let inp = self.inp;
        let (aspect, size, samples) = (inp.aspect, inp.size, inp.samples);
        let SceneCtx { gpu, post, color, depth, d, camera, render, game, encoder, path } = &mut self.c;
        let (gpu, color, depth) = (*gpu, *color, *depth);
        let d: &mut Drive = d;
        let (cam, poses): (&Camera, &render::snapshot::FramePoses) = (camera, game.poses);
        let render: &mut RenderState = render;
        let pools = &self.pools;
        match path {
            ScenePath::Window { late_on, smoke_format, late_format, .. } => {
                let (late_on, smoke_format, late_format) = (*late_on, *smoke_format, *late_format);
                let sub = nfsu2::prof::acc::span("  particles");
                let rain_quads = d.rain_quads_at(cam, poses.player(), poses.rain());
                if render.world.is_some() {
                    // The water's surfaces and sheets, into the scene (before
                    // the temporal pass, which they are part of).
                    if inp.settings.opts.animated_water {
                        // This frame's water and puff pipelines built
                        // before they are first drawn (no first-use hitch).
                        d.effects.water.warm(gpu, smoke_format, samples, samples, true);
                        d.effects.water.draw_surfaces(gpu, encoder, color, smoke_format, samples, depth, size);
                    }
                    let mut quads = particle_quads(gpu, encoder, d, cam, poses, render.world.as_ref(), pools, aspect, self.fog_drawn, &inp.settings.opts, inp.stepped());
                    // (After `particle_quads`, which makes the puffs' GPU half.)
                    if late_on {
                        d.effects.detail.warm(gpu, late_format, 1, 1);
                        d.effects.cinematic.warm(gpu, late_format, 1, 1);
                        d.effects.sparks_fire.warm(gpu, late_format, 1, 1);
                    } else {
                        d.effects.sparks_fire.warm(gpu, smoke_format, samples, samples);
                        d.effects.detail.warm(gpu, smoke_format, samples, samples);
                        d.effects.cinematic.warm(gpu, smoke_format, samples, samples);
                    }
                    quads.extend(rain_quads);
                    let r = render.world.as_mut().unwrap();
                    if late_on {
                        // Drawn after the temporal pass (no depth: they would ghost).
                        r.upload_particles(gpu, &quads, &inp.settings.opts);
                    } else {
                        r.render_particles(gpu, encoder, color, depth, &quads, &inp.settings.opts);
                        // The detailed smoke over them (soft against the depth).
                        d.effects.detail.draw(gpu, encoder, color, smoke_format, samples, depth, samples, size, None);
                        d.effects.cinematic.draw(gpu, encoder, color, smoke_format, samples, depth, samples, None);
                        d.effects.sparks_fire.draw(gpu, encoder, color, smoke_format, samples, depth, samples, None);
                        // Ambient Detail Next-Gen: tyre dust and steam.
                        render.world.as_ref().unwrap().draw_ambient(
                            gpu,
                            encoder,
                            color,
                            smoke_format,
                            samples,
                            depth,
                            samples,
                            None,
                            &inp.settings.opts,
                        );
                        if inp.settings.opts.animated_water {
                            d.effects.water.draw_spray(
                                gpu,
                                encoder,
                                color,
                                smoke_format,
                                samples,
                                depth,
                                samples,
                                None,
                            );
                        }
                        nfsu2::prof::gpu::mark(encoder, "particles");
                    }
                }
                drop(sub);
            }
            ScenePath::Still { .. } => {
                if inp.settings.opts.animated_water {
                    d.effects.water.draw_surfaces(gpu, encoder, color, post.scene_format(), samples, depth, size);
                }
                let mut quads = particle_quads(gpu, encoder, d, cam, poses, render.world.as_ref(), pools, aspect, self.fog_drawn, &inp.settings.opts, inp.stepped());
                quads.extend(d.rain_quads_at(cam, poses.player(), poses.rain()));
                render.world.as_mut().unwrap().render_particles(gpu, encoder, color, depth, &quads, &inp.settings.opts);
                d.effects.detail.draw(gpu, encoder, color, post.scene_format(), samples, depth, samples, size, None);
                d.effects.cinematic.draw(gpu, encoder, color, post.scene_format(), samples, depth, samples, None);
                d.effects.sparks_fire.draw(gpu, encoder, color, post.scene_format(), samples, depth, samples, None);
                atmosphere_drive(&mut self.out.volumetric, d, inp.clocks.world.unwrap_or(d.clock));
                render.world.as_ref().unwrap().draw_ambient(
                    gpu,
                    encoder,
                    color,
                    post.scene_format(),
                    samples,
                    depth,
                    samples,
                    None,
                    &inp.settings.opts,
                );
                if inp.settings.opts.animated_water {
                    d.effects.water.draw_spray(gpu, encoder, color, post.scene_format(), samples, depth, samples, None);
                }
            }
        }
    }

    /// The glow sprites: into the streak buffer inside the post passes,
    /// or (the window's frame) here into the scene when the streaks are off.
    fn glows(&mut self) {
        let inp = self.inp;
        let SceneCtx { gpu, color, depth, d, camera, render, game, encoder, path, .. } = &mut self.c;
        let (gpu, color, depth) = (*gpu, *color, *depth);
        let cam: &Camera = camera;
        let late_on = matches!(path, ScenePath::Window { late_on: true, .. });
        let hidden = view_leaves_out_player(game.mode, d, game.shop_scene, inp.photo_open);
        let sub = nfsu2::prof::acc::span("  glows");
        let switches = inp.settings.switches;
        let set = CarSet { paused_race: inp.paused_fallback, traffic: inp.glow_traffic };
        if !inp.gate_glow || (switches.light_glow && render.world.is_some()) {
            let cars = DrawnCars::of(game, set);
            let quads =
                glow_quads(d, cam, cars.poses, cars.race, cars.encounters, cars.traffic, inp.aspect, inp.clocks.seconds, self.drawn.as_deref(), &inp.settings.opts, inp.stepped().wet_interaction, hidden);
            let r = render.world.as_mut().unwrap();
            r.upload_glows(gpu, &quads);
            if inp.gate_glow && !switches.streaks() && !late_on {
                r.draw_glows(encoder, color, depth, false);
            }
        } else if inp.stepped().wet_interaction != 0 && render.world.is_some() {
            // Wet Road Interaction's lights and wheels without the
            // Light Glow switch (its hook rides `glow_quads`; the
            // sprites are dropped).
            let cars = DrawnCars::of(game, set);
            let _ =
                glow_quads(d, cam, cars.poses, cars.race, cars.encounters, cars.traffic, inp.aspect, inp.clocks.seconds, self.drawn.as_deref(), &inp.settings.opts, inp.stepped().wet_interaction, hidden);
        }
        drop(sub);
    }

    /// 0x5CB81C: the horizon fog, first in the end-of-frame chain.
    fn horizon_fog(&mut self) {
        let inp = self.inp;
        let sub = nfsu2::prof::acc::span("  horizon fog");
        // The visibility as the ticks left it (never sampled: the
        // first sample at the frame's camera, a pure read).
        let visibility = self.c.game.poses.vis.horizon.unwrap_or_else(|| {
            nfsu2::horizon_fog::HorizonFog::default().step(&self.c.d.world, self.c.camera.position.truncate())
        });
        let SceneCtx { gpu, post, depth, d, camera, encoder, path, .. } = &mut self.c;
        let cam: &Camera = camera;
        if inp.stepped().switches.horizon_fog
            && let Some(grid) = nfsu2::horizon_fog::grid(&d.world.horizon_fog, visibility, &fog_view(&cam, inp.size))
        {
            post.stage().horizon_fog(gpu, encoder, depth, &grid, &inp.settings.opts);
            mark(path, encoder, "horizon fog");
        }
        drop(sub);
        drop(self.span.take());
    }

    /// The frame's motion inputs, the jitter taken off the camera.
    fn motion_inputs(&mut self) {
        let inp = self.inp;
        let SceneCtx { d, camera, game, path, .. } = &mut self.c;
        if let ScenePath::Window { temporal_on: true, jitter, .. } = path {
            let cam: &mut Camera = camera;
            cam.jitter = glam::Vec2::ZERO;
            let cars = DrawnCars::of(game, CarSet { paused_race: inp.paused_fallback, traffic: TrafficSet::Live });
            let (companion, race, encounters, traffic) = cars.parts();
            self.out.temporal_frame = Some(nfsu2::render::temporal::FrameInput {
                camera: *cam,
                aspect: inp.aspect,
                jitter: *jitter,
                reversed: inp.settings.opts.reversed_z,
                cars: temporal_cars(d, cam, cars.poses, companion, race, encounters, traffic),
                cut: false,
                key: temporal_key(game.mode, inp.mover, game.shop_scene.is_some()),
            });
        }
    }

    /// The scene to the GPU: the window's on a render thread (the belt's
    /// copies are recorded: its chunks close before the submission), a
    /// still's in place.
    fn submit(self) -> SceneOut {
        let WorldScene { c, mut out, .. } = self;
        match c.path {
            ScenePath::Window { belt, encoded, .. } => {
                if let Some(b) = belt.as_mut() {
                    b.finish();
                }
                out.submitted = Some(nfsu2::render::Submitted::start(c.gpu.queue.clone(), encoded, c.encoder));
            }
            ScenePath::Still { .. } => {
                c.gpu.queue.submit([c.encoder.finish()]);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(pass: WorldPass) -> usize {
        WORLD_PASSES.iter().position(|&p| p == pass).unwrap()
    }

    /// The scene's order (the window's frame and the shots alike, one
    /// sequence): the lit scene, then the ambient occlusion and the
    /// clone's motion blur and depth of field, then the additive
    /// layers. Replaces the source-text checks that were in
    /// `render::ssao` and `render::blur_dof`.
    #[test]
    fn occlusion_blur_and_dof_run_before_the_additive_layers() {
        for p in WORLD_PASSES {
            assert_eq!(WORLD_PASSES.iter().filter(|&&q| q == p).count(), 1, "{p:?} once");
        }
        let additive = [WorldPass::Shadows, WorldPass::Particles, WorldPass::Glows, WorldPass::HorizonFog];
        // SSAO multiplies the lit scene: the world, the cars and the
        // characters drawn under it, before every additive layer.
        for lit in [WorldPass::World, WorldPass::Cars, WorldPass::CharactersUnderAo] {
            assert!(at(lit) < at(WorldPass::Ssao), "{lit:?} before the occlusion");
        }
        for a in additive {
            assert!(at(WorldPass::Ssao) < at(a), "the occlusion before {a:?}");
        }
        // Without the occlusion the characters come after the shadows.
        assert!(at(WorldPass::Shadows) < at(WorldPass::CharactersOverShadows));
        // Motion blur and depth of field: after the shadows and the
        // characters, before the particles, glows and horizon fog.
        assert!(at(WorldPass::Shadows) < at(WorldPass::BlurDof));
        assert!(
            at(WorldPass::CharactersUnderAo) < at(WorldPass::BlurDof)
                && at(WorldPass::CharactersOverShadows) < at(WorldPass::BlurDof)
        );
        for a in [WorldPass::Particles, WorldPass::Glows, WorldPass::HorizonFog] {
            assert!(at(WorldPass::BlurDof) < at(a), "motion blur / DOF before {a:?}");
        }
        // The native speed blur (in the world pass) before the cars.
        assert!(at(WorldPass::World) < at(WorldPass::Cars));
    }

    /// The native speed blur: under Customize Display's switch, left out
    /// only for the clone's motion blur tiers; the tier is not read with
    /// the switch off.
    #[test]
    fn native_speed_blur_is_under_its_switch() {
        let on = Switches::from_detail(5);
        let off = Switches { motion_blur: false, ..on };
        assert!(on.motion_blur);
        assert!(native_speed_blur(on, || 0));
        assert!(!native_speed_blur(on, || 1));
        assert!(!native_speed_blur(on, || 2));
        assert!(!native_speed_blur(off, || unreachable!("the tier is read only with the switch on")));
    }

    /// The Original tier without depth of field is the native path:
    /// `blur_and_dof` returns before it makes or draws anything.
    #[test]
    fn original_tier_without_dof_draws_nothing() {
        use nfsu2::render::blur_dof::MotionMode;
        assert!(blur_dof_idle(MotionMode::Original, false));
        assert!(!blur_dof_idle(MotionMode::Original, true));
        assert!(!blur_dof_idle(MotionMode::Camera, false));
        assert!(!blur_dof_idle(MotionMode::PerObject, false));
    }
}

/// The blink of a car within the hold of its last stamped placement
/// (`nfsu2::racer_reset::PlaceBlink`, 0x60D500): whether its body and
/// wheels are drawn this frame. Its lights, shadow and headlight pool are
/// drawn by their own passes and stay steady, as in the original.
fn place_blink_drawn(cr: &mut CarRenderer, state: &CarState, clocks: &crate::render::clocks::DrawClocks) -> bool {
    let now = clocks.world.map_or(clocks.blur, f64::from);
    cr.place_blink.drawn(state.ghost_timer > 0.0, now)
}
