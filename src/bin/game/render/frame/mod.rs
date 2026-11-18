use crate::*;

pub(crate) mod cars;
mod scene;
mod view;
pub(crate) use cars::*;
pub(crate) use scene::*;
pub(crate) use view::*;

/// The frame's layout and the world's per-frame values (before the GPU).
pub(crate) struct FramePrep {
    hud: Vec<Quad>,
    menu: Vec<Quad>,
    loading: Vec<Quad>,
    garage: Option<GarageRequest>,
    aid_target: Option<Vec3>,
    rollers: Option<[glam::Mat4; 4]>,
    tuning_world: bool,
    temporal_mover: u32,
}

/// The frame's scene targets and the switches that chose them.
struct FrameTargets {
    /// The frame's settings, taken once where they are final.
    settings: nfsu2::render::FrameSettings,
    out_size: (u32, u32),
    size: (u32, u32),
    samples: u32,
    scene_color: wgpu::TextureView,
    scene_depth: wgpu::TextureView,
    base_depth: wgpu::TextureView,
    smoke_format: wgpu::TextureFormat,
    late_format: wgpu::TextureFormat,
    driving: bool,
    temporal_on: bool,
    upscaling: bool,
    staged: bool,
    late_on: bool,
    blur_after_upscale: bool,
}

/// What the world's draw leaves for the post passes.
struct WorldFrame {
    in_game: bool,
    cleared: bool,
    game_frame: PostFrame,
    ssao: Option<nfsu2::render::ssao::Frame>,
    ssr: Option<nfsu2::render::ssr::Frame>,
    volumetric: Option<nfsu2::render::volumetric::Frame>,
    car_air: Option<nfsu2::render::car_lights::Frame>,
    temporal_frame: Option<nfsu2::render::temporal::FrameInput>,
    scene_submitted: Option<nfsu2::render::Submitted>,
}

/// The frame's display image and where the frame is drawn.
struct Acquired {
    frame: Option<wgpu::SurfaceTexture>,
    view: wgpu::TextureView,
    presented_view: wgpu::TextureView,
    hdr_target: Option<wgpu::TextureView>,
    capture_request: Option<(usize, bool)>,
    capture: Option<wgpu::Texture>,
}

/// The post passes' results for the UI and the submission.
struct PostDone {
    quads: Vec<Quad>,
    masked: bool,
    post_encoded: Option<nfsu2::render::Finishing>,
    garage_drawn: bool,
    in_game: bool,
    cleared: bool,
    scene_submitted: Option<nfsu2::render::Submitted>,
}

impl Game {
    /// `poses`, the frame's values between the tick snapshots
    /// ([`render::snapshot::FramePoses`]); `photo_camera`: photo mode's
    /// camera is in for the frame.
    /// `probe`: the streak buffer's commit alone (`render::streak`).
    pub(crate) fn draw_frame(&mut self, poses: render::snapshot::FramePoses, photo_camera: bool, probe: bool) -> Result<()> {
        profiling::scope!("draw");
        // The renderers of cars dropped outside an update (a menu
        // key ending a race, a load) go before the frame draws.
        self.render.sweep();
        let now = self.now();
        let span = nfsu2::prof::acc::span("hud and menu layout");
        profiling::scope!("hud and menu layout");
        // The per-frame updates the draw path still runs (unchanged,
        // in their order, before the draw): the HUD's layout with its
        // `*_update`s, the display mode. The menus' hit map, the garage's
        // request and scene, the dyno camera and photo mode's close step on
        // the tick (`Game::fe_tick`).
        let mut prep = if probe { self.prepare_probe(span) } else { self.prepare_frame(now, span) };
        // The dyno's rollers as the frame draws them between the
        // ticks.
        if let (Some(a), Some(r)) = (poses.rollers, prep.rollers.and(self.dyno.rollers.as_ref())) {
            prep.rollers = Some(r.transforms_with(&a));
        }
        // The frame's camera: the blend between the ticks, unless
        // photo mode's is in for the frame (the dyno camera is the tick's
        // too, so it is in the snapshots).
        let camera = self.sim.drive.as_ref().map(|d| poses.camera_for(&d.camera, photo_camera));
        // The ticks the enhancement effects step on this frame.
        // The frame's lag behind the newest tick, (1 − α)·dt (the snapshot
        // unit's former `water_lag`).
        self.publish_tick_views(poses.alpha, (1.0 - poses.alpha) * poses.dt);
        // The draw: the game read through a shared view, the render side
        // written.
        let (view, mut render) = self.draw_parts(&poses);
        let (out, result) = draw(&view, &mut render, prep, camera, probe);
        // The updates that take the draw's results.
        self.drawn_update(&out);
        result?;
        if out.presented && !probe {
            self.frame_drawn();
            profiling::finish_frame!();
        }
        Ok(())
    }

    /// [`Game::prepare_frame`] for a probe: no layout (its UI is
    /// not drawn), no display mode change.
    fn prepare_probe(&mut self, span: Option<nfsu2::prof::acc::Acc>) -> FramePrep {
        drop(span);
        FramePrep {
            hud: Vec::new(),
            menu: Vec::new(),
            loading: Vec::new(),
            garage: self.garage_fe.request.clone(),
            aid_target: self.gps_aid_target(),
            rollers: self.dyno.rollers.as_ref().filter(|_| self.on_dyno_track()).map(|r| r.transforms()),
            tuning_world: self.tuning_menu_over_world(),
            temporal_mover: self.view_history_key(),
        }
    }

    /// The HUD's and the menus' layout, the garage's request, the GPS
    /// aid's target, the dyno's rollers and camera, the display mode.
    fn prepare_frame(&mut self, now: f32, span: Option<nfsu2::prof::acc::Acc>) -> FramePrep {
        let photo_quads = self.photo_quads().or_else(|| self.free_camera_quads());
        // The HUD's quads from the last tick's update (`overlay_update`
        // in `step`): a pure read here.
        let hud = if photo_quads.is_some() { Vec::new() } else { self.overlay_layout(now) };
        let mut menu = match photo_quads {
            Some(q) => q,
            None => self.menu_quads(now),
        };
        let saving_showcase = matches!(&self.mode, Mode::Menu(Menu::Showcase(v)) if v.save_ticks.is_some());
        if !saving_showcase
            && self.platform.perf_overlay.shown.is_some()
            && let Some(font) = self.game_data.assets.fonts.values().find(|f| f.name.contains("tt21i"))
        {
            let (settings, window) = (&self.settings, self.platform.window.as_ref());
            self.platform.perf_overlay.quads(
                &self.game_data.assets,
                font,
                || settings.deviations(),
                || overlay_display(window, settings),
                &settings.overlay,
                &mut menu,
            );
        }
        // The texture editor's panel (`--editor`, F7; none while closed).
        if !saving_showcase {
            self.texture_editor_quads(&mut menu);
        }
        // The cursor over the front end (0x5378D0 → 0x50B460).
        // Native Yes clears 0x7FA29C, which gates the cursor in 0x53790D.
        if !saving_showcase
            && self.photo.is_none()
            && self.free_camera.is_none()
            && let Some(c) = self.cursor_quad(now)
        {
            menu.push(c);
        }
        // The mouse tests the tick's layout (`Game::fe_map_tick`):
        // this layout records nothing.
        let loading = self.loading_quads(now);
        drop(span);
        // The garage's request as the tick made it (`garage_tick`).
        let garage = self.garage_fe.request.clone();
        let aid_target = self.gps_aid_target();
        let rollers = self.dyno.rollers.as_ref().filter(|_| self.on_dyno_track()).map(|r| r.transforms());
        let tuning_world = self.tuning_menu_over_world();
        self.apply_display_mode();
        // The camera's mover, incl. each shot of a scene's camera track: a change
        // is a cut for the temporal pass.
        let temporal_mover = self.view_history_key();
        FramePrep { hud, menu, loading, garage, aid_target, rollers, tuning_world, temporal_mover }
    }
}

/// A frame's draw: the game's state read through `view`
/// (shared), the render side and the listed game writes through `render`;
/// the frame's place between the last two ticks is `view.poses`,
/// `camera` the frame's camera (the drive's, as drawn). What the updates
/// after it take is the outcome.
pub(crate) fn draw(
    view: &FrameView<'_>,
    render: &mut RenderMut<'_>,
    prep: FramePrep,
    camera: Option<Camera>,
    probe: bool,
) -> (DrawOutcome, Result<()>) {
    let mut f = FrameDraw { v: view, r: render, camera, out: DrawOutcome::default(), probe };
    let result = f.run(prep);
    (f.out, result)
}

/// The draw's inputs between its steps.
struct FrameDraw<'v, 'r, 'a, 'b> {
    v: &'v FrameView<'a>,
    r: &'r mut RenderMut<'b>,
    /// The frame's camera (its own copy; the TAA jitter is set on
    /// it, never on the drive's).
    camera: Option<Camera>,
    out: DrawOutcome,
    /// The streak buffer's probe (`render::streak`): the world and
    /// the post chain up to the commit, nothing presented.
    probe: bool,
}

impl FrameDraw<'_, '_, '_, '_> {
    fn run(&mut self, prep: FramePrep) -> Result<()> {
        let Some(targets) = self.frame_targets(&prep) else { return Ok(()) };
        let mut world = self.draw_world(&prep, &targets)?;
        if self.probe {
            let post = self.draw_post(prep, &targets, world, None);
            let w = self.r.window.as_mut().unwrap();
            w.frame_end = Some(nfsu2::render::FrameEnd::start_ui(
                w.gpu.queue.clone(),
                post.scene_submitted,
                post.post_encoded,
                None,
                Vec::new(),
                None,
            ));
            return Ok(());
        }
        let Some(acquired) = self.acquire_targets(targets.out_size, &mut world.scene_submitted) else { return Ok(()) };
        let post = self.draw_post(prep, &targets, world, Some(&acquired.view));
        self.draw_ui_submit(targets.out_size, &targets.settings, acquired, post);
        self.out.presented = true;
        Ok(())
    }

    /// The window's frame settings (the swap chain, HDR, packs, filters),
    /// the drive's renderers and the scene's targets. None: no window.
    fn frame_targets(&mut self, prep: &FramePrep) -> Option<FrameTargets> {
        let FramePrep { garage, tuning_world, .. } = prep;
        let tuning_world = *tuning_world;
        let w = self.r.window.as_mut()?;
        // The last frame's end, which went on while this frame's
        // simulation and layout ran, before anything is written to the GPU.
        w.settle();
        // The streak buffer's cost: the GPU timings read back by now.
        self.r.state.streak.gpu_harvest(&w.gpu);
        // HDR Output switched (the Display page): the swap chain again.
        // The Display page is the calibration screen: its test pattern
        // shows while it is open.
        let calibrating = matches!(self.v.mode, Mode::Menu(Menu::Options(o)) if matches!(o.category, options::Category::GraphicsGroup(options::GraphicsGroup::Display) | options::Category::Settings(nfsu2::frontend::settings_model::UxGroup::Display)));
        let pattern_env = {
            static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            *ON.get_or_init(|| debug_env().hdr_pattern.as_deref() == Some("1"))
        };
        nfsu2::qol::set_hdr_pattern(calibrating || pattern_env);
        // Here, before anything of this frame is written to the GPU (the
        // switch waits for the GPU and recalls the staging belt).
        w.sync_hdr();
        // Reload Packs When Changed (docs/pack-hot-reload.md): the pack
        // watcher, and the HD UI Art row live; the maps' calibration follows.
        // The calibration (game data) in `drawn_update`, after the draw.
        if nfsu2::render::pack_reload::frame(self.v.settings.enh.hd_ui) {
            self.out.packs_reloaded = true;
        }
        // g_TextureFiltering, for the renderers built from here on
        // prov: open game.draw_frame
        // (**open**: the game reloads its effects on a change, 0x8709D1).
        nfsu2::render::display::set_texture_filter(self.v.settings.custom[20]);
        // The clone's display rows (Aspect Ratio, Vsync Off Mode).
        nfsu2::render::display::set_aspect_mode(if self.v.settings.aspect == 1 {
            nfsu2::render::display::AspectMode::Stretched
        } else {
            nfsu2::render::display::AspectMode::Wide
        });
        nfsu2::render::display::set_vsync_off_mailbox(self.v.settings.vsync_off_mailbox);
        // g_VSyncOn: the swap chain's interval, reset when it changes.
        let present = nfsu2::render::display::present_mode_for(&self.v.settings.custom, &w.present_modes);
        // Low Latency: one frame in flight instead of two.
        let latency = if nfsu2::extras::low_latency() { 1 } else { 2 };
        if w.config.present_mode != present || w.config.desired_maximum_frame_latency != latency {
            w.config.present_mode = present;
            w.config.desired_maximum_frame_latency = latency;
            if let Some(surface) = &w.surface {
                configure_surface(surface, &w.gpu.device, &w.surface_config());
            }
        }
        // Renderers for a freshly loaded region.
        if let Some(d) = self.r.writes.drive.as_deref_mut() {
            if self.r.state.world.is_none() {
                self.r.state.world = Some(WorldRenderer::new(&w.gpu, &d.world, w.config.format, &nfsu2::render::RenderOptions::in_force()));
            }
            if self.r.state.cars.get(&d.car_id).is_none() {
                self.r.state.cars.set(&d.car_id, CarRenderer::new(&w.gpu, &d.car, w.config.format, &nfsu2::render::RenderOptions::in_force()));
            }
        }
        let out_size = (w.config.width, w.config.height);
        // The 3D scene goes to the post passes' back buffer first.
        let format = w.config.format;
        let post = w.post.get_or_insert_with(|| PostFx::new(&w.gpu, format));
        let driving = garage.is_none() && (tuning_world || world_under(self.v.mode, self.r.writes.drive.is_some()));
        // Full Screen Anti-Aliasing (custom 14, 0x870D08): the game's
        // back buffer is multisampled (0x5B9B3B), so the game's scene and
        // the front end's (the garage scenes) are, resolved before the
        // screen passes (`PostFx::multisampled`).
        // The temporal pass runs on the game's world only: the front
        // end's and the garage's scenes keep the native MSAA with TAA chosen.
        nfsu2::qol::set_temporal_frame(nfsu2::qol::temporal() && driving && garage.is_none());
        // The frame's settings, read once here (the display mode, the
        // HDR switch, the pack reload and the temporal decision made).
        let settings = nfsu2::render::FrameSettings::new(&self.v.settings.custom, nfsu2::render::RenderOptions::in_force());
        let opts = &settings.opts;
        let samples = if driving || garage.is_some() {
            nfsu2::render::display::msaa_samples(&self.v.settings.custom, &w.gpu, opts)
        } else {
            1
        };
        // The Graphics page's Render Scale (clone deviation): the 3D scene
        // at that size, taken to the screen's after the post passes; the UI
        // stays at the screen's. Native: the screen's size.
        // The temporal pass (docs/temporal.md): TAA / Upscaling on the
        // game's world only (the front end's scenes draw as before).
        let temporal_on = opts.temporal_frame;
        let upscaling = temporal_on && opts.upscale != 0;
        // FSR 1 (docs/modern-extras.md) on the game's world only: the
        // scene and the native post chain at the tier's share with the
        // native MSAA (the Render Scale path), EASU + RCAS taking it to the
        // screen's size in place of the resample (`enhance::Plan::fsr`),
        // the HUD after it at the screen's size.
        let fsr_on = opts.fsr && driving && garage.is_none();
        nfsu2::render::enhance::set_fsr_frame(fsr_on);
        let staged = upscaling;
        let late_on = temporal_on;
        // FSR 2 takes the clone's motion blur and depth of field
        // after it (at the display's size, unjittered).
        let blur_after_upscale = upscaling && opts.fsr2;
        let size = if opts.upscale != 0 && !(upscaling || fsr_on) {
            out_size
        } else {
            opts.scene_size(out_size, samples)
        };
        let scene = match post.set_upscale(&w.gpu, opts, staged.then_some(size), out_size, samples) {
            Some(stage_scene) => stage_scene,
            None => post.scene_view(&w.gpu, size, opts),
        };
        // FSR 2 Native AA (staged at the screen's size) writes the
        // display-size depth into w.depth from the scene's: its own then.
        let base_depth = if size == out_size && !staged { w.depth.clone() } else { post.scaled_depth(&w.gpu, size) };
        let (scene_color, scene_depth) =
            post.stage().multisampled(&w.gpu, size, samples, opts).unwrap_or_else(|| (scene.clone(), base_depth.clone()));
        // The scene's format for the detailed smoke's pipelines.
        let smoke_format = post.stage().scene_format();
        // The late (after the temporal pass) draws' format.
        let late_format = post.scene_format();
        w.cars.samples = samples;
        if !temporal_on && let Some(t) = post.temporal.as_mut() {
            t.reset();
        }
        Some(FrameTargets {
            settings,
            out_size,
            size,
            samples,
            scene_color,
            scene_depth,
            base_depth,
            smoke_format,
            late_format,
            driving,
            temporal_on,
            upscaling,
            staged,
            late_on,
            blur_after_upscale,
        })
    }

    /// The world's scene ([`draw_world_scene`]) under the game's world.
    fn draw_world(&mut self, prep: &FramePrep, t: &FrameTargets) -> Result<WorldFrame> {
        let FrameTargets {
            out_size,
            size,
            samples,
            ref scene_color,
            ref scene_depth,
            smoke_format,
            late_format,
            driving,
            temporal_on,
            late_on,
            blur_after_upscale,
            ..
        } = *t;
        let w = self.r.window.as_mut().unwrap();
        let mut temporal_frame: Option<nfsu2::render::temporal::FrameInput> = None;
        let mut in_game = false;
        let mut game_frame = PostFrame::GAME;
        // The world shows under the in-game menus (pause, post race) and
        // the dialogs raised while driving; the front end is drawn alone.
        // A dialog over the front end's scene (e.g. the D-Day lot's
        // "Rachel needs her car back.", raised with the city still loaded)
        // keeps that scene alone behind it: drawing the world too let the
        // two scenes, submitted from different threads, flicker.
        let mut cleared = false;
        let mut scene_submitted = None;
        let mut ssao = None;
        let mut ssr = None;
        let mut volumetric = None;
        let mut car_air = None;
        // No car lights outside the game's frames (the front end).
        nfsu2::render::car_lights::publish(Vec::new(), t.settings.opts.car_lights_next_gen);
        if let (true, Some(d)) = (driving, self.r.writes.drive.as_deref_mut()) {
            // The scene in four command buffers: the world's views and the
            // cube, each recorded and encoded on a render thread while the
            // cars are recorded here; the cars, encoded on a render thread
            // while the shadows, particles and fog are recorded here; and
            // those, encoded on a render thread too, which then submits the
            // four in that order while the end of the frame is recorded.
            let encoder = new_frame_encoder(&w.gpu);
            // The streak buffer's cost measurement: the frame's GPU work
            // from its first command buffer to the streak buffer's commit
            // in the post passes (`render::streak`).
            let mut encoded: Vec<nfsu2::render::Finishing> = Vec::new();
            if let Some(start) = self.r.state.streak.gpu_begin(&w.gpu) {
                encoded.push(nfsu2::render::Finishing::ready(start));
            }
            let aspect = out_size.0 as f32 / out_size.1 as f32;
            // The main view's sub-pixel jitter for this frame's draw
            // (taken off again after the scene).
            let opts = &t.settings.opts;
            let jitter = if temporal_on {
                let t = w
                    .post
                    .as_mut()
                    .unwrap()
                    .temporal
                    .get_or_insert_with(|| nfsu2::render::temporal::Temporal::new(&w.gpu));
                t.next_jitter(size, out_size, opts)
            } else {
                glam::Vec2::ZERO
            };
            let camera = self.camera.as_mut().expect("the frame's camera is built with its drive");
            if temporal_on {
                camera.jitter = nfsu2::render::temporal::jitter_ndc(jitter, size);
            }
            // The scene's characters skinned on a render thread from
            // here (their frame, place and the camera are final), used by
            // `draw_characters!` below; the first frame's renderer is made
            // there, as before (it adds the world's texture slots).
            if let (Mode::Race(_) | Mode::Menu(_), Some(c)) = (self.v.mode, self.v.scene_crowd)
                && let Some(cr) = self.r.state.crowds.get_mut(&c.id)
            {
                cr.start(&c.set, camera.view_projection(aspect), self.v.poses.crowd_frame(c) / nfsu2::shop_scene::FPS, c.origin);
            }
            let inputs = FrameInputs {
                aspect,
                size,
                samples,
                ssao_texel_scale: size.1 as f32 / out_size.1.max(1) as f32,
                out_size,
                // The frame's clocks between the ticks.
                clocks: self.v.poses.clocks,
                settings: t.settings,
                gate_glow: true,
                paused_fallback: true,
                pool_traffic: TrafficSet::ExploreOnly,
                glow_traffic: TrafficSet::Live,
                rear_view: self.v.rear_view_on,
                photo_open: self.v.photo.is_some(),
                aid_target: prep.aid_target,
                rollers: prep.rollers,
                mover: prep.temporal_mover,
                still: None,
                blur_after_upscale,
                format: w.config.format,
                stepped: self.v.poses.stepped.as_ref().map(Stepped::of_tick),
            };
            let ctx = SceneCtx {
                gpu: &w.gpu,
                post: w.post.as_mut().unwrap(),
                color: scene_color,
                depth: scene_depth,
                d,
                camera,
                render: &mut *self.r.state,
                game: SceneGame {
                    poses: self.v.poses,
                    mode: self.v.mode,
                    paused_race: self.v.paused_race,
                    traffic: self.v.traffic,
                    encounters: self.v.encounters,
                    shop_scene: self.v.shop_scene,
                    scene_crowd: self.v.scene_crowd,
                    settings: &self.v.settings,
                },
                encoder,
                path: ScenePath::Window {
                    cars: &mut w.cars,
                    belt: &mut w.belt,
                    encoded,
                    temporal_on,
                    late_on,
                    jitter,
                    smoke_format,
                    late_format,
                },
            };
            let out = draw_world_scene(ctx, &inputs)?;
            game_frame = out.game_frame;
            if out.drawn {
                cleared = true;
                in_game = true;
            }
            ssao = out.ssao;
            ssr = out.ssr;
            volumetric = out.volumetric;
            car_air = out.car_air;
            temporal_frame = out.temporal_frame;
            scene_submitted = out.submitted;
        }
        Ok(WorldFrame { in_game, cleared, game_frame, ssao, ssr, volumetric, car_air, temporal_frame, scene_submitted })
    }

    /// The swap chain's image and the frame's targets (the showcase
    /// capture, HDR Output). None: no image this frame (the scene waited
    /// for).
    fn acquire_targets(
        &mut self,
        out_size: (u32, u32),
        scene_submitted: &mut Option<nfsu2::render::Submitted>,
    ) -> Option<Acquired> {
        let w = self.r.window.as_mut().unwrap();
        // The swap chain's image, taken only now: the scene above needs
        // none, and with vsync the wait for a free one overlaps its
        // recording rather than holding it up.
        // With no image this frame, the scene still goes to the GPU before
        // the next frame writes its uniforms (in order, as on any frame).
        let acquire_start = Instant::now();
        let acquired = w.surface.as_ref().map(|s| s.get_current_texture());
        w.acquire = acquire_start.elapsed().as_secs_f32();
        w.acquired = Some((acquire_start, Instant::now()));
        // Single-frame stalls of about 1 s in free roam (1007 / 1052 ms, a player
        // report): wgpu-core 30 waits up to FRAME_TIMEOUT_MS = 1000 ms for a
        // swap-chain image and then gives Timeout. A wait that long is the
        // compositor holding every image (a screenshot UI, a mode switch,
        // the window hidden), not the game's own work: logged as such.
        if w.acquire >= ACQUIRE_STALL_S
            || matches!(acquired, Some(wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded))
        {
            let outcome = match &acquired {
                Some(wgpu::CurrentSurfaceTexture::Success(_)) => "an image",
                Some(wgpu::CurrentSurfaceTexture::Suboptimal(_)) => "a suboptimal image",
                Some(wgpu::CurrentSurfaceTexture::Timeout) => "timeout, frame skipped",
                Some(wgpu::CurrentSurfaceTexture::Occluded) => "occluded, frame skipped",
                Some(wgpu::CurrentSurfaceTexture::Outdated) => "outdated, reconfigured",
                Some(wgpu::CurrentSurfaceTexture::Lost) => "lost, reconfigured",
                _ => "no image",
            };
            log::warn!(
                "swap chain: waited {:.0} ms for a display image ({outcome}; focused {}, occluded {}): the compositor held them, not the game",
                w.acquire * 1000.0,
                self.v.focused,
                self.v.occluded
            );
        }
        // The diagnostics' events (docs/troubleshooting.md): an image
        // shown, or why none came.
        match &acquired {
            Some(wgpu::CurrentSurfaceTexture::Success(_)) => nfsu2::diag::events::image_shown(),
            Some(wgpu::CurrentSurfaceTexture::Suboptimal(_)) => {
                nfsu2::diag::events::image_shown();
                nfsu2::diag::events::record(nfsu2::diag::events::Kind::Surface, "image suboptimal");
            }
            Some(other) => nfsu2::diag::events::record(
                nfsu2::diag::events::Kind::Surface,
                match other {
                    wgpu::CurrentSurfaceTexture::Timeout => "no image: timeout",
                    wgpu::CurrentSurfaceTexture::Occluded => "no image: occluded",
                    wgpu::CurrentSurfaceTexture::Outdated => "no image: outdated, reconfiguring",
                    wgpu::CurrentSurfaceTexture::Lost => "no image: surface lost, reconfiguring",
                    _ => "no image: other",
                },
            ),
            None => {}
        }
        let frame = match acquired {
            None => None,
            Some(wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t)) => Some(t),
            Some(wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost) => {
                if let Some(s) = scene_submitted.take() {
                    s.wait();
                }
                configure_surface(w.surface.as_ref().unwrap(), &w.gpu.device, &w.surface_config());
                return None;
            }
            Some(_) => {
                if let Some(s) = scene_submitted.take() {
                    s.wait();
                }
                return None;
            }
        };
        let Some(view) =
            frame.as_ref().map(|f| &f.texture).or(w.offscreen.as_ref()).map(|t| t.create_view(&Default::default()))
        else {
            if let Some(s) = scene_submitted.take() {
                s.wait();
            }
            return None;
        };
        let capture_request = match self.v.mode {
            Mode::Menu(Menu::Showcase(v)) if v.save_ticks == Some(0) => Some((v.milestone, v.dvd)),
            _ => None,
        };
        let capture = capture_request.map(|_| {
            w.gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("showcase picture framebuffer"),
                size: wgpu::Extent3d { width: out_size.0, height: out_size.1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: w.config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        });
        // HDR Output: windowed, the frame is drawn into an SDR texture the
        // display transform reads into the swap chain image; headless, it
        // writes an HDR texture beside the offscreen target.
        let (presented_view, hdr_target) = match w.hdr_frame(out_size) {
            Some(t) if frame.is_some() => (t, Some(view)),
            Some(t) => (view, Some(t)),
            None => (view, None),
        };
        let view = capture.as_ref().map_or_else(|| presented_view.clone(), |t| t.create_view(&Default::default()));
        Some(Acquired { frame, view, presented_view, hdr_target, capture_request, capture })
    }

    /// The frame's quads, the garage's scene and the post passes.
    /// `view`: the frame's image (None: a probe, `render::streak`).
    fn draw_post(
        &mut self,
        prep: FramePrep,
        targets: &FrameTargets,
        world: WorldFrame,
        view: Option<&wgpu::TextureView>,
    ) -> PostDone {
        let FramePrep { hud, menu, loading, garage, temporal_mover, .. } = prep;
        let FrameTargets { out_size, size, samples, ref scene_color, ref scene_depth, ref base_depth, .. } = *targets;
        let WorldFrame {
            mut in_game,
            mut cleared,
            game_frame,
            ssao,
            ssr,
            volumetric,
            car_air,
            temporal_frame,
            scene_submitted,
        } = world;
        let w = self.r.window.as_mut().unwrap();
        let span = nfsu2::prof::acc::span("post passes");
        let t = self.v.poses.clocks.seconds;
        let rear_image = if in_game && self.v.rear_view_on {
            self.r.writes.drive.as_deref().and(self.r.state.world.as_ref()).and_then(|r| r.rear_view_image()).cloned()
        } else {
            None
        };
        let quads = if view.is_some() {
            frame_quads(&mut w.ui, &w.gpu, &mut self.r.rear_view_bound, rear_image, (hud, menu, loading), in_game)
        } else {
            Vec::new()
        };
        // Destination-alpha textures in the UI: the post passes' last pass
        // leaves alpha 0 for them (`ui::Alpha`).
        let masked = UiRenderer::masked(&self.v.game_data.assets, &quads);
        // The post passes are encoded on a render thread while the UI is
        // built, and go to the GPU with it.
        let mut post_encoded = None;
        // The garage's scene timed for the streak buffer's cost as the
        // world's is (`render::streak`): the first timestamp goes to the
        // GPU before the scene's submissions.
        if garage.is_some()
            && let Some(start) = self.r.state.streak.gpu_begin(&w.gpu)
        {
            w.gpu.queue.submit([start]);
        }
        let garage_drawn = render_garage(
            garage,
            &self.v.game_data.root,
            &self.v.game_data.caches,
            &mut self.r.state.garage,
            &w.gpu,
            w.config.format,
            &scene_color,
            &scene_depth,
            size,
            t,
            targets.settings.display,
            &targets.settings.opts,
            samples,
            w.post.as_mut(),
            self.v.poses.garage.as_ref(),
        );
        if garage_drawn {
            cleared = true;
            in_game = false;
        }
        if cleared && let Some(post) = w.post.as_mut() {
            let mut encoder = w.gpu.device.create_command_encoder(&Default::default());
            nfsu2::prof::gpu::mark(&mut encoder, nfsu2::prof::gpu::GAP);
            let switches = targets.settings.switches;
            let renderer = if in_game && switches.light_glow {
                self.r.writes.drive.as_deref().and(self.r.state.world.as_ref())
            } else {
                None
            };
            let depth = if in_game { &scene_depth } else { &base_depth };
            post_inputs(post, &w.gpu, self.r.writes.drive.as_deref(), self.v.poses.clocks.world, self.v.poses.vis.rain_color, self.v.poses.rain(), self.v.poses.stepped.map_or(targets.settings.opts.lens_drops, |t| t.lens_drops), in_game, out_size, (ssao, ssr, volumetric, car_air));
            // The temporal pass, then what it must not hold (particles,
            // rain, glow sprites) at the output's size without the jitter.
            let mut late = None;
            if let (true, Some(tf)) = (in_game, temporal_frame.as_ref()) {
                let fsr2_blur = || {
                    let cars = DrawnCars::of_parts(
                        self.v.mode,
                        self.v.paused_race,
                        self.v.traffic,
                        self.v.encounters,
                        self.v.shop_scene,
                        self.v.poses,
                        CarSet { paused_race: true, traffic: TrafficSet::Live },
                    );
                    let time = self.v.poses.clocks.blur;
                    let key = temporal_key(self.v.mode, temporal_mover, self.v.shop_scene.is_some());
                    (cars, time, key)
                };
                let drive = self.r.writes.drive.as_deref();
                late = Some(temporal_late(
                    &w.gpu,
                    post,
                    &mut encoder,
                    tf,
                    targets,
                    &w.depth,
                    switches,
                    renderer,
                    drive.zip(self.camera.as_ref()),
                    drive.and(self.r.state.world.as_ref()),
                    fsr2_blur,
                    temporal_mover,
                ));
            } else if let Some(t) = post.temporal.as_mut() {
                t.reset();
            }
            // Photo mode's grade after the game's post chain (which
            // draws into its intermediate then).
            let photo = self.v.photo.filter(|_| in_game && view.is_some());
            let grades = &mut self.r.state.photo_grades;
            let graded = photo.map(|p| {
                (
                    grades.slot(&p.id).get_or_insert_with(|| nfsu2::render::photo_grade::PhotoGrade::new(&w.gpu)).target(
                        &w.gpu,
                        out_size,
                        w.config.format,
                    ),
                    p,
                )
            });
            // The overbright level as the ticks left it.
            post.set_tick_level(self.v.poses.vis.overbright);
            // The streak buffer commits once per tick (`render::streak`):
            // this draw's slot when it is new (in the cheap form also the
            // slots it missed), the scratch B(α) between ticks.
            let poses = self.v.poses;
            let probed = !nfsu2::qol::temporal() && !self.r.state.streak.cheap_frame;
            let streak =
                self.r.state.streak.ticks.frame(poses.tick, poses.frame_ticks, poses.alpha, view.is_none(), probed);
            post.set_commit_stamp(self.r.state.streak.commit_stamp());
            let frame = if in_game { game_frame } else { PostFrame::FRONT_END };
            let mut glow = |_: nfsu2::render::postfx::StreakGlow,
                            e: &mut wgpu::CommandEncoder,
                            streak: &wgpu::TextureView,
                            resolve: Option<&wgpu::TextureView>| {
                if let Some(r) = renderer {
                    match &late {
                        Some((late_depth, vp)) => r.draw_glows_at(e, streak, resolve, late_depth, true, Some(*vp)),
                        None => r.draw_glows_resolved(e, streak, resolve, depth, true),
                    }
                }
            };
            match view {
                Some(view) => {
                    let post_target = graded.as_ref().map_or(view, |(t, _)| t);
                    post.apply_ticks(&w.gpu, &targets.settings.opts, &mut encoder, post_target, switches, frame, streak, masked, &mut glow);
                }
                None if streak.commits > 0 => post.commit_tick(&w.gpu, &targets.settings.opts, &mut encoder, switches, frame, &mut glow),
                None => {}
            }
            if let (Some((_, p)), Some(view)) = (graded, view) {
                let look = p.look;
                if let Some(g) = self.r.state.photo_grades.get_mut(&p.id) {
                    g.run(&w.gpu, &mut encoder, view, w.config.format, &look);
                    nfsu2::prof::gpu::mark(&mut encoder, "photo mode: grade");
                }
            }
            let unwritten = post.take_commit_stamp();
            self.r.state.streak.gpu_end(&mut encoder, !unwritten);
            post_encoded = Some(nfsu2::render::Finishing::start(encoder));
        }
        drop(span);
        PostDone { quads, masked, post_encoded, garage_drawn, in_game, cleared, scene_submitted }
    }

    /// The UI over the frame, its submission and presentation, and the
    /// showcase's picture.
    fn draw_ui_submit(&mut self, out_size: (u32, u32), settings: &nfsu2::render::FrameSettings, a: Acquired, post: PostDone) {
        let Acquired { frame, view, presented_view, hdr_target, capture_request, capture } = a;
        let PostDone { quads, masked, post_encoded, garage_drawn, in_game, cleared, scene_submitted } = post;
        let w = self.r.window.as_mut().unwrap();
        let span = nfsu2::prof::acc::span("ui");
        for t in self.r.ui_uploads.drain(..) {
            w.ui.add_texture(&w.gpu, &t);
        }
        draw_cover(
            self.v.mode,
            self.r.writes.cover_car,
            self.r.writes.cover_picture,
            &w.gpu,
            w.config.format,
            &mut w.ui,
            &self.v.game_data.assets,
            settings,
        );
        // The frame marked shown in `drawn_update`, after the draw.
        self.out.movie_uploaded = upload_movie_frame(self.v.mode, &w.gpu, &mut w.ui);
        let alpha = nfsu2::render::ui::Alpha {
            zeroed: cleared && masked,
            opaque: frame.is_some() && w.config.alpha_mode == wgpu::CompositeAlphaMode::Opaque,
        };
        nfsu2::render::display::set_scene_capped(fe_scene_capped(self.v.mode, garage_drawn));
        // Encoded (wgpu's `finish`) on a render thread, as the post passes.
        let ui = w.ui.record_encoder(&w.gpu, &self.v.game_data.assets, &view, out_size, &quads, !cleared, alpha);
        nfsu2::render::display::set_scene_capped(false);
        drop(span);
        let _span = nfsu2::prof::acc::span("post and ui: submit");
        // HDR Output's display transform, after the UI (and the showcase
        // capture's copy, below).
        let sdr_view = presented_view.clone();
        let hdr_pass = |w: &mut WindowState| -> Vec<wgpu::CommandBuffer> {
            let Some(target) = hdr_target.as_ref() else { return Vec::new() };
            let unclipped = w.post.as_mut().and_then(|p| p.take_unclipped());
            let gpu = w.gpu.clone();
            let h = w.hdr.as_mut().unwrap();
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            h.transform.run(
                &gpu,
                &mut encoder,
                &sdr_view,
                unclipped.as_ref(),
                target,
                h.signal,
                &nfsu2::qol::hdr_calibration(),
                out_size,
            );
            nfsu2::prof::gpu::mark(&mut encoder, "hdr output: display transform");
            vec![encoder.finish()]
        };
        // Submitted and presented on a render thread once the scene and the
        // post passes are encoded; the next frame waits for it.
        let captured = if let Some(texture) = capture.as_ref() {
            // Wait for the real world, native post passes and showcase overlays.
            // A private target avoids requiring COPY_SRC on the window surface.
            w.frame_end = Some(nfsu2::render::FrameEnd::start(
                w.gpu.queue.clone(),
                scene_submitted,
                post_encoded,
                ui.finish(),
                None,
            ));
            w.settle();
            // As RGBA8 whatever the window's format; one it cannot convert
            // leaves the save without a picture (as a frame without the world).
            let pixels = nfsu2::render::read_back_rgba8(&w.gpu, texture)
                .inspect_err(|e| log::warn!("showcase picture not taken: {e:#}"))
                .ok();
            let captured = pixels
                .filter(|_| in_game && cleared)
                .and_then(|pixels| screens::showcase::capture_rgba(&pixels, out_size, false));
            let blitter = wgpu::util::TextureBlitter::new(&w.gpu.device, w.config.format);
            let mut encoder = w.gpu.device.create_command_encoder(&Default::default());
            blitter.copy(&w.gpu.device, &mut encoder, &view, &presented_view);
            let mut last = vec![encoder.finish()];
            last.extend(hdr_pass(w));
            w.frame_end = Some(nfsu2::render::FrameEnd::start_with(w.gpu.queue.clone(), None, None, last, frame));
            captured
        } else {
            let ui = nfsu2::render::Finishing::start(ui);
            let last = hdr_pass(w);
            w.frame_end = Some(nfsu2::render::FrameEnd::start_ui(
                w.gpu.queue.clone(),
                scene_submitted,
                post_encoded,
                Some(ui),
                last,
                frame,
            ));
            None
        };
        if !overlap_frames() {
            w.settle();
        }
        // Native 0x4B59D0 waits for capture readiness. A missing world frame
        // must leave the request pending, never save UI alone or a stale cover.
        if in_game
            && cleared
            && let Some((milestone, dvd)) = capture_request
        {
            // Saved in `drawn_update`, after the draw.
            self.out.picture = Some(((milestone, dvd), captured));
        }
    }
}

/// The post passes' inputs from the world's scene (the rain, the output
/// size, SSAO, SSR, the volumetric fog, the car lights' air), none
/// outside the game's frames.
#[allow(clippy::type_complexity)]
fn post_inputs(
    post: &mut PostFx,
    gpu: &Gpu,
    drive: Option<&Drive>,
    world_clock: Option<f32>,
    rain_color: [f32; 3],
    drawn_rain: Option<&nfsu2::weather::RainSnapshot>,
    lens_drops: bool,
    in_game: bool,
    out_size: (u32, u32),
    (ssao, ssr, volumetric, car_air): (
        Option<nfsu2::render::ssao::Frame>,
        Option<nfsu2::render::ssr::Frame>,
        Option<nfsu2::render::volumetric::Frame>,
        Option<nfsu2::render::car_lights::Frame>,
    ),
) {
    let rain = drive
        .filter(|_| in_game)
        .map(|d| (d.rain_frame(rain_color, drawn_rain, lens_drops), d.world.textures.get(&nfsu2::formats::hash::bin_hash("RAINDROPPC"))));
    match rain {
        Some((frame, tex)) => post.set_rain(gpu, frame, tex),
        None => post.set_rain(gpu, Default::default(), None),
    }
    post.set_output_size(out_size);
    post.stage().set_ssao(if in_game { ssao } else { None });
    post.stage().set_ssr(if in_game { ssr } else { None });
    let mut volumetric = volumetric;
    if let Some(d) = drive {
        atmosphere_drive(&mut volumetric, d, world_clock.unwrap_or(d.clock));
    }
    post.stage().set_volumetric(if in_game { volumetric } else { None });
    post.stage().set_car_lights(if in_game { car_air } else { None });
}

/// The frame's quads in one allocation (a performance audit item): the mirror's
/// quad first, the HUD, the menus, the loading screen.
fn frame_quads(
    ui: &mut UiRenderer,
    gpu: &Gpu,
    rear_view_bound: &mut Option<wgpu::TextureView>,
    rear_image: Option<wgpu::TextureView>,
    (hud, menu, loading): (Vec<Quad>, Vec<Quad>, Vec<Quad>),
    in_game: bool,
) -> Vec<Quad> {
    // The frame's quads in one allocation: the
    // mirror's quad first (the frame's tail, 0x5CBF50, draws it before
    // the front end), the HUD, the menus, the loading screen.
    let mut quads = Vec::with_capacity(1 + hud.len() + menu.len() + loading.len());
    if let Some(image) = rear_image {
        if rear_view_bound.as_ref() != Some(&image) {
            ui.set_texture_clamped(gpu, nfsu2::rear_view::TEXTURE, &image);
            *rear_view_bound = Some(image);
        }
        quads.push(nfsu2::rear_view::quad());
    }
    quads.extend(hud);
    // The colour-vision palette on the in-game HUD's quads (not the
    // mirror's picture, not the menus).
    let hud_start = usize::from(quads.first().is_some_and(|q| q.texture == Some(nfsu2::rear_view::TEXTURE)));
    nfsu2::render::ui::set_hud_palette(if in_game { nfsu2::extras::palette() } else { 0 }, hud_start..quads.len());
    quads.extend(menu);
    quads.extend(loading);
    quads
}

/// The temporal pass, then what it must not hold (particles, rain,
/// glow sprites) at the output's size without the jitter, with FSR 2's
/// motion blur and depth of field before them. Returns the late
/// sprites' depth and viewport.
#[allow(clippy::too_many_arguments)]
fn temporal_late<'c>(
    gpu: &Gpu,
    post: &mut PostFx,
    encoder: &mut wgpu::CommandEncoder,
    tf: &nfsu2::render::temporal::FrameInput,
    targets: &FrameTargets,
    out_depth: &wgpu::TextureView,
    switches: Switches,
    renderer: Option<&WorldRenderer>,
    drive: Option<(&Drive, &Camera)>,
    world_renderer: Option<&WorldRenderer>,
    fsr2_blur: impl FnOnce() -> (DrawnCars<'c>, f64, u64),
    mover: u32,
) -> (wgpu::TextureView, [f32; 4]) {
    let FrameTargets { out_size, size, ref scene_depth, upscaling, staged, blur_after_upscale, .. } = *targets;
    let jitter = tf.jitter;
    // The world's animation mask for FSR 2's masks.
    if targets.settings.opts.fsr2 {
        let mask = world_renderer.and_then(|r| r.anim_mask());
        post.temporal.get_or_insert_with(|| nfsu2::render::temporal::Temporal::new(gpu)).fsr2_anim_mask(gpu, mask);
    }
    post.temporal_pass(
        gpu,
        &targets.settings.opts,
        encoder,
        tf,
        &scene_depth,
        upscaling.then_some(out_depth),
        switches.streaks(),
        &mut |e, target, resolve| {
            if let Some(r) = renderer {
                r.draw_glows_resolved(e, target, resolve, &scene_depth, true);
            }
        },
    );
    let target = post.scene_view(gpu, if staged { out_size } else { size }, &targets.settings.opts);
    let late_depth = if staged { out_depth.clone() } else { scene_depth.clone() };
    let target_size = if staged { out_size } else { size };
    let vp = nfsu2::render::temporal::late_viewport(jitter, size, target_size);
    // FSR 2's motion blur and depth of field, on the
    // upscaled lit scene before the late (additive) sprites, as
    // the native order has them before the particles.
    if let (true, Some((d, camera))) = (blur_after_upscale, drive) {
        let (cars, time, key) = fsr2_blur();
        let aspect = out_size.0 as f32 / out_size.1 as f32;
        blur_and_dof(
            post,
            gpu,
            encoder,
            &target,
            &late_depth,
            1,
            target_size,
            d,
            camera,
            cars.poses,
            aspect,
            cars.parts(),
            key,
            mover,
            time,
            None,
            true,
            &targets.settings.opts,
        );
        nfsu2::prof::gpu::mark(encoder, "graphics: fsr2 motion blur / dof");
    }
    if let Some((d, _)) = drive {
        // The detailed smoke after the native particles,
        // like them after the temporal pass (no ghosting).
        if let Some(r) = world_renderer {
            r.draw_particles(gpu, encoder, &target, &late_depth, Some(vp));
        }
        d.effects.detail.draw(gpu, encoder, &target, post.scene_format(), 1, &late_depth, 1, target_size, Some(vp));
        d.effects.cinematic.draw(gpu, encoder, &target, post.scene_format(), 1, &late_depth, 1, Some(vp));
        d.effects.sparks_fire.draw(gpu, encoder, &target, post.scene_format(), 1, &late_depth, 1, Some(vp));
        if let Some(r) = world_renderer {
            r.draw_ambient(gpu, encoder, &target, post.scene_format(), 1, &late_depth, 1, Some(vp), &targets.settings.opts);
        }
        if targets.settings.opts.animated_water {
            d.effects.water.warm(gpu, post.scene_format(), 1, 1, false);
            d.effects.water.draw_spray(gpu, encoder, &target, post.scene_format(), 1, &late_depth, 1, Some(vp));
        }
    }
    if let Some(r) = world_renderer {
        if switches.light_glow && !switches.streaks() {
            r.draw_glows_at(encoder, &target, None, &late_depth, false, Some(vp));
        }
    }
    nfsu2::prof::gpu::mark(encoder, "graphics: temporal late sprites");
    (late_depth, vp)
}

/// Whether the world is drawn under `mode` (`loaded`: a drive is up),
/// for the window and the shots alike. SPEED2.EXE has no per-screen gate:
/// its frame (0x5D2860) renders each active view (views 1–3, view +8;
/// 0x48F7A0) and the screen passes (Render_Frame 0x5CAAE0) whatever FE
/// packages are on the stack, unless 0x8383AC is set (0x4FE4A0); the
/// in-game screens (pause, post-race chain, sponsor screens, World Map,
/// SMS, dialogs) are packages pushed over the paused race or free roam
/// (0x537990), so the world shows under them unchanged: under Wine the
/// paused free roam's pixels outside the pause panel equal the unpaused
/// frame's, no dim (docs/fe-backdrops.md). The front end proper (no
/// drive) is drawn alone.
pub(crate) fn world_under(mode: &Mode, loaded: bool) -> bool {
    match mode {
        Mode::Explore | Mode::Race(_) => true,
        // UI_Sponsorship_new after UI_SponsorPopup (0x4C1340 → 0x4BFF80):
        // the post-race chain's, over the race (Wine s4 KENWOOD page).
        Mode::Menu(Menu::Icons(_, kind)) => kind.in_game() || (*kind == MenuKind::Sponsor && loaded),
        Mode::Menu(Menu::Options(o)) => o.pause.is_some(),
        Mode::Menu(Menu::Status(_)) => true,
        // The World Map key's map and the SMS screens stay over the
        // paused city (the original under Wine, 2026-10-29).
        Mode::Menu(Menu::InGameMap(_) | Menu::SmsMailbox(_) | Menu::SmsViewer(_)) => loaded,
        Mode::Menu(
            Menu::Dialog(..)
            | Menu::Engage(_)
            | Menu::ShopEngage(_)
            | Menu::Challenge(_)
            | Menu::Showcase(_)
            | Menu::Reward(_)
            | Menu::Results(..)
            | Menu::Stats(_)
            | Menu::SponsorPopup(_),
        ) => loaded,
        Mode::Menu(_) => false,
    }
}

/// A front-end 3D scene is on screen, capped at 21:9 on wider screens
/// (`display::SCENE_CAP`): the garage behind the menus, or the
/// showcase / shop-entry menus over the world. Driving, races, scenes
/// and replays are not.
pub(crate) fn fe_scene_capped(mode: &Mode, garage_drawn: bool) -> bool {
    garage_drawn || matches!(mode, Mode::Menu(Menu::Showcase(_) | Menu::ShopEngage(_)))
}
