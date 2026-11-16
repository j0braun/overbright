//! `world [TRACK] [--shot out.png] [--cam x,y,z] [--look yaw,pitch] [--size WxH] [--time s] [--pick x,y]`
//!
//! Interactive: fly through the city. WASD moves, Q/E down/up, Shift is fast,
//! hold the right mouse button to look around, Esc quits, P prints the camera.
//! With `--car NAME` (e.g. `--car 240SX`) you drive instead: W/S or the arrow
//! keys for throttle and brake/reverse, A/D to steer, Space for the
//! handbrake, C to change camera (bumper, hood, near, far, high), R to put
//! the car back at the start.
//!
//! With `--shot`, renders one frame off-screen to a PNG and exits.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use glam::Vec3;
use nfsu2::car::Car;
use nfsu2::chase::{ChaseCamera, VIEW_ORDER};
use nfsu2::drive::{CarState, Controls, STEP};
use nfsu2::smackable::Props;
use nfsu2::render::car::CarRenderer;
use nfsu2::render::lighting::CarLighting;
use nfsu2::render::{Camera, FrameParams, Gpu, HeadlightPool, WorldRenderer, create_depth, overview_camera};
use nfsu2::fog::FogState;
use nfsu2::sky::TimeOfDay;
use nfsu2::world::World;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

struct Args {
    track: String,
    shot: Option<String>,
    camera: Option<Vec3>,
    look: Option<(f32, f32)>,
    size: (u32, u32),
    pick: Option<(f32, f32)>,
    time: f32,
    car: Option<String>,
    drive_seconds: Option<f32>,
}

fn parse_args() -> Result<Args> {
    let mut args = Args { track: "L4RA".into(), shot: None, camera: None, look: None, size: (1280, 720), pick: None, time: 0.0, car: None, drive_seconds: None };
    let mut it = std::env::args().skip(1);
    let floats = |s: String| -> Result<Vec<f32>> {
        s.split(',').map(|v| v.trim().parse::<f32>().context("bad number")).collect()
    };
    while let Some(a) = it.next() {
        match a.as_str() {
            "--shot" => args.shot = Some(it.next().context("--shot needs a path")?),
            "--cam" => {
                let v = floats(it.next().context("--cam x,y,z")?)?;
                args.camera = Some(Vec3::new(v[0], v[1], v[2]));
            }
            "--look" => {
                let v = floats(it.next().context("--look yaw,pitch (degrees)")?)?;
                args.look = Some((v[0].to_radians(), v[1].to_radians()));
            }
            "--pick" => {
                let v = floats(it.next().context("--pick x,y (pixels)")?)?;
                args.pick = Some((v[0], v[1]));
            }
            "--drive-seconds" => args.drive_seconds = Some(it.next().context("--drive-seconds N")?.parse()?),
            "--car" => args.car = Some(it.next().context("--car NAME")?),
            "--time" => args.time = it.next().context("--time seconds")?.parse()?,
            "--size" => {
                let s = it.next().context("--size WxH")?;
                let (w, h) = s.split_once('x').context("--size WxH")?;
                args.size = (w.parse()?, h.parse()?);
            }
            other => args.track = other.to_string(),
        }
    }
    Ok(args)
}

/// Where drive mode starts when no `--cam` is given: a Chinatown street.
const DRIVE_START: (Vec3, f32, f32) = (Vec3::new(361.0, -462.0, 19.5), 203.0, -12.0);

fn initial_camera(args: &Args, world: &World) -> Camera {
    let mut camera = overview_camera(world);
    if args.car.is_some() && args.camera.is_none() {
        let (p, yaw, pitch) = DRIVE_START;
        camera = Camera::new(p, yaw.to_radians(), pitch.to_radians());
    }
    if let Some(p) = args.camera {
        camera.position = p;
    }
    if let Some((yaw, pitch)) = args.look {
        camera.yaw = yaw;
        camera.pitch = pitch;
    }
    camera
}

fn main() -> Result<()> {
    // A developer tool: the developer cache, never the player's (docs/asset-cache.md).
    nfsu2::platform::dirs::developer_program();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = parse_args()?;
    let started = Instant::now();
    let mut world = World::load(&nfsu2::game_root(), &args.track)?;
    debug_filter(&mut world);
    log::info!("loaded {} in {:.2?}", args.track, started.elapsed());

    if let Some(pixel) = args.pick {
        pick(&args, &world, pixel);
        return Ok(());
    }
    if let Some(path) = &args.shot {
        return screenshot(&args, &world, path);
    }

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let camera = initial_camera(&args, &world);
    let mut start_state = None;
    let driving = match &args.car {
        Some(name) => {
            let mut car = Car::load(&nfsu2::game_root(), name)?;
            // NFS_RACE=drag|drift|streetx|url drives with that event's
            // handling (see docs/formats.md, "Race modes").
            let race = std::env::var("NFS_RACE").ok().map_or(Some(nfsu2::race::RaceMode::FreeRoam), |r| nfsu2::race::RaceMode::parse(&r)).context("NFS_RACE is drag, drift, streetx, url or freeroam")?;
            car.set_race(race);
            // NFS_DIFFICULTY=0..2 (the race difficulty 0x89E7F4; boots at 0).
            if let Some(d) = std::env::var("NFS_DIFFICULTY").ok().and_then(|d| d.parse::<usize>().ok()) {
                car.car_type.handling.difficulty = d.min(2);
            }
            let place = car_placement(&world, &camera, &car);
            let heading = { let x = place.x_axis; x.y.atan2(x.x) };
            let mut state = CarState::new(place.w_axis.truncate(), heading);
            state.start_race(race);
            start_state = Some(state.clone());
            Some((car, state))
        }
        None => None,
    };
    let chase = driving.as_ref().map(|(car, _)| ChaseCamera::new(car, camera_view()));
    let mut app = App { driving, chase, start_state, camera, fog: FogState::default(), time_of_day: TimeOfDay::default(), started: Instant::now(), world, state: None, keys: HashSet::new(), looking: false, last: Instant::now(), props: Props::default(), moved: Default::default() };
    app.props = Props::new(&app.world);
    event_loop.run_app(&mut app)?;
    Ok(())
}

/// Diagnostics: `NFS_ONLY_FLAGS` / `NFS_SKIP_FLAGS` (comma-separated hex)
/// and `NFS_ONLY_NAME` (substring) restrict which placements are drawn.
fn debug_filter(world: &mut World) {
    let flags = |var: &str| -> Option<Vec<u16>> {
        let v = std::env::var(var).ok()?;
        Some(v.split(',').filter_map(|f| u16::from_str_radix(f.trim().trim_start_matches("0x"), 16).ok()).collect())
    };
    let only = flags("NFS_ONLY_FLAGS");
    let skip = flags("NFS_SKIP_FLAGS");
    let name = std::env::var("NFS_ONLY_NAME").ok();
    let solids = &world.solids;
    world.placements.retain(|p| {
        only.as_ref().is_none_or(|o| o.contains(&p.flags))
            && skip.as_ref().is_none_or(|s| !s.contains(&p.flags))
            && name.as_ref().is_none_or(|n| solids[p.solid].name.contains(n.as_str()))
    });
}

/// The game's own visibility rule for the camera's position; everything when
/// `NFS_ALL_SECTIONS` is set.
fn visible_sections<'w>(world: &'w World, camera: &Camera) -> Option<&'w [u32]> {
    if std::env::var_os("NFS_ALL_SECTIONS").is_some() {
        return None;
    }
    world.visible_sections_at(camera.position).map(|(_, list)| list)
}

/// Diagnostics: list the placed solids whose bounding box the ray through
/// pixel `(x, y)` of a `--size` frame passes, nearest first, with the
/// textures they use. Only sections visible from the camera are considered.
fn pick(args: &Args, world: &World, (x, y): (f32, f32)) {
    let camera = initial_camera(args, world);
    let (w, h) = (args.size.0 as f32, args.size.1 as f32);
    let ndc = glam::Vec4::new(2.0 * x / w - 1.0, 1.0 - 2.0 * y / h, 1.0, 1.0);
    let inv = camera.view_projection(w / h).inverse();
    let far = inv * ndc;
    let dir = (far.truncate() / far.w - camera.position).normalize();
    let visible = visible_sections(world, &camera);

    let mut hits = Vec::new();
    for p in &world.placements {
        if visible.is_some_and(|v| !v.contains(&p.section) && p.section != 2600) {
            continue;
        }
        let s = &world.solids[p.solid];
        // Ray against the solid's box in its local space.
        let inv_t = p.transform.inverse();
        let o = inv_t.transform_point3(camera.position);
        let d = inv_t.transform_vector3(dir);
        let (lo, hi) = (Vec3::from(s.bbox_min), Vec3::from(s.bbox_max));
        let t1 = (lo - o) / d;
        let t2 = (hi - o) / d;
        let (tmin, tmax) = (t1.min(t2).max_element(), t1.max(t2).min_element());
        if tmax >= tmin.max(0.0) {
            hits.push((tmin.max(0.0), p));
        }
    }
    hits.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (t, p) in hits.iter().take(15) {
        let s = &world.solids[p.solid];
        let textures: Vec<String> = s
            .textures
            .iter()
            .map(|h| world.textures.get(h).map_or(format!("{h:08X}?"), |t| format!("{}({:?}{})", t.name, t.blend, if t.alpha_test { ",test" } else { "" })))
            .collect();
        println!("{t:8.1}  {:36} sec {:5} flags {:#06x}  {}", s.name, p.section, p.flags, textures.join(" "));
    }
}

/// Where to park a car: on the road ahead of the camera, facing the same
/// way. The car's wheel bottoms (wheel centre z − radius) touch the road.
/// The `--car` for a still shot: with NFS_SPINNERS set, on the first rims
/// (front and rear) whose part has a second model group, the spinner.
fn load_shot_car(name: &str) -> Result<Car> {
    let root = nfsu2::game_root();
    let mut installed = std::collections::BTreeMap::new();
    if std::env::var_os("NFS_SPINNERS").is_some() {
        let globalb = nfsu2::game_fs::read(&root, "GLOBAL/GLOBALB.BUN")?;
        let db = nfsu2::formats::parts::PartDb::load(&globalb)?;
        for slot in [0x1Dusize, 0x1E] {
            let mut after = None;
            while let Some(p) = db.find_after(name, slot, after) {
                after = Some(p);
                if db.solids(p, name).len() > 1 {
                    installed.insert(slot as u8, p.hash);
                    break;
                }
            }
        }
    }
    Car::load_with(&root, name, &installed)
}

fn car_placement(world: &World, camera: &Camera, car: &Car) -> glam::Mat4 {
    let ahead = Vec3::new(camera.forward().x, camera.forward().y, 0.0).normalize_or_zero();
    let distance = std::env::var("NFS_CAR_DISTANCE").ok().and_then(|d| d.parse().ok()).unwrap_or(9.0);
    let spot = camera.position + ahead * distance;
    let road = world.road_height_below(spot + Vec3::Z * 5.0).unwrap_or(spot.z - 1.5);
    let w = car.car_type.wheels[0];
    let z = road - (w.position.z - w.radius);
    let turn: f32 = std::env::var("NFS_CAR_TURN").ok().and_then(|d| d.parse().ok()).unwrap_or(0.0);
    let heading = ahead.y.atan2(ahead.x) + turn.to_radians();
    glam::Mat4::from_translation(Vec3::new(spot.x, spot.y, z)) * glam::Mat4::from_rotation_z(heading)
}

/// The camera kind to start in: `NFS_CAMERA_VIEW` (0 far, 1 near, 2 bumper,
/// 3 driver, 4 hood, 5 high), else the near chase.
fn camera_view() -> u16 {
    std::env::var("NFS_CAMERA_VIEW").ok().and_then(|v| v.parse().ok()).unwrap_or(1)
}

/// Road reflections are on unless `NFS_NO_REFLECTIONS` is set.
fn road_reflections() -> bool {
    std::env::var_os("NFS_NO_REFLECTIONS").is_none()
}

// --- off-screen ---------------------------------------------------------

fn screenshot(args: &Args, world: &World, path: &str) -> Result<()> {
    let (width, height) = args.size;
    let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance()))?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = WorldRenderer::new(&gpu, world, format, &nfsu2::render::RenderOptions::in_force());
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shot"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let depth = create_depth(&gpu, width, height);
    let mut camera = initial_camera(args, world);
    // `--drive-seconds N` with `--car`: hold the throttle for N seconds and
    // shoot from the chase camera.
    let mut driven = None;
    if let (Some(name), Some(seconds)) = (&args.car, args.drive_seconds) {
        let car = Car::load(&nfsu2::game_root(), name)?;
        let place = car_placement(world, &camera, &car);
        let mut state = CarState::new(place.w_axis.truncate(), place.x_axis.y.atan2(place.x_axis.x));
        let mut props = Props::new(world);
        let mut moved = std::collections::HashMap::new();
        // `NFS_DRIVE_STEER=-1..1` steers (at half throttle) for the last second.
        let steer: f32 = std::env::var("NFS_DRIVE_STEER").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let steps = (seconds / STEP) as u32;
        // The camera runs at a 60 Hz frame rate alongside the physics.
        let mut chase = ChaseCamera::new(&car, camera_view());
        let frame = (1.0 / 60.0 / STEP).round() as u32;
        for i in 0..steps {
            let turning = steer != 0.0 && (steps - i) as f32 * STEP <= 1.0;
            let controls = if turning {
                Controls { throttle: 0.5, steer, ..Default::default() }
            } else {
                Controls { throttle: 1.0, ..Default::default() }
            };
            moved.extend(state.step_with_props(&car, world, &mut props, controls, STEP, &mut Default::default()));
            if i % frame == 0 {
                chase.update(&state, frame as f32 * STEP, nfsu2::qol::fov_offset());
            }
        }
        camera = chase.update(&state, frame as f32 * STEP, nfsu2::qol::fov_offset());
        println!(
            "drove {:.1} m, {:.1} km/h, gear {}, {:.0} rpm",
            state.position.distance(place.w_axis.truncate()),
            state.speed() * 3.6,
            state.gear_name(),
            state.rpm()
        );
        println!("knocked {} of {} props", props.knocked(), world.smackables.len());
        for (placement, transform) in moved {
            renderer.move_placement(&gpu, placement, transform);
        }
        driven = Some((car, state));
    }

    let bytes_per_row = (width * 4).div_ceil(256) * 256;
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (bytes_per_row * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    // `--time` also stands for seconds since the track loaded.
    let time_of_day = TimeOfDay::default().after(args.time);
    let pools: Vec<HeadlightPool> = driven.iter().map(|(car, state)| HeadlightPool { transform: state.transform(), color: car.headlight_pool_color }).collect();
    let params = FrameParams {
        camera: &camera,
        aspect: width as f32 / height as f32,
        visible_sections: visible_sections(world, &camera), far_sections: None,
        fog: FogState::default().update_weighted(world, camera.position.truncate(), time_of_day.fog_weight()),
        time: args.time,
        road_reflections: road_reflections(),
        road_reflection_plane: None,
        sky: time_of_day.sky(),
        reversed: false,
        headlight_pools: &pools,
        reflection_glows: &[],
        display: Default::default(),
        opts: &nfsu2::render::RenderOptions::in_force(),
        rear_view: None,
        occluders: &[],
    };
    renderer.render(&gpu, &mut encoder, &view, &depth, (width, height), &params);
    if let Some((car, state)) = &driven {
        let car_renderer = CarRenderer::new(&gpu, car, format, &nfsu2::render::RenderOptions::in_force());
        let view_proj = camera.view_projection(width as f32 / height as f32);
        car_renderer.render(&gpu, &mut encoder, &view, &depth, view_proj, camera.view(), &CarLighting::in_world(world, &camera, state.transform(), &nfsu2::render::RenderOptions::in_force()), state.transform(), &state.wheel_poses(car), &nfsu2::render::RenderOptions::in_force());
    } else if let Some(name) = &args.car {
        let car = load_shot_car(name)?;
        let car_renderer = CarRenderer::new(&gpu, &car, format, &nfsu2::render::RenderOptions::in_force());
        let place = car_placement(world, &camera, &car);
        let view_proj = camera.view_projection(width as f32 / height as f32);
        // NFS_SPINNER_ANGLE (degrees): the spinners' angle, the rims at rest.
        let spinner = std::env::var("NFS_SPINNER_ANGLE").ok().and_then(|a| a.parse::<f32>().ok()).unwrap_or(0.0).to_radians();
        let poses = [nfsu2::car::WheelPose { spinner, ..Default::default() }; 4];
        car_renderer.render(&gpu, &mut encoder, &view, &depth, view_proj, camera.view(), &CarLighting::in_world(world, &camera, place, &nfsu2::render::RenderOptions::in_force()), place, &poses, &nfsu2::render::RenderOptions::in_force());
    }
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bytes_per_row), rows_per_image: None },
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    gpu.queue.submit([encoder.finish()]);
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None })?;
    let mapped = slice.get_mapped_range()?;
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for row in mapped.chunks(bytes_per_row as usize).take(height as usize) {
        pixels.extend_from_slice(&row[..(width * 4) as usize]);
    }
    image::save_buffer(path, &pixels, width, height, image::ColorType::Rgba8)?;
    println!(
        "wrote {path}  (camera {:.1},{:.1},{:.1} look {:.1},{:.1})",
        camera.position.x,
        camera.position.y,
        camera.position.z,
        camera.yaw.to_degrees(),
        camera.pitch.to_degrees()
    );
    if let Some((section, list)) = world.visible_sections_at(camera.position) {
        println!("section {section}, drawing {list:?}");
    }
    Ok(())
}

// --- interactive ----------------------------------------------------------

struct WindowState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gpu: Gpu,
    depth: wgpu::TextureView,
    renderer: WorldRenderer,
    car_renderer: Option<CarRenderer>,
}

struct App {
    world: World,
    camera: Camera,
    fog: FogState,
    time_of_day: TimeOfDay,
    started: Instant,
    state: Option<WindowState>,
    keys: HashSet<KeyCode>,
    looking: bool,
    last: Instant,
    /// Drive mode (`--car NAME`): the car and its state.
    driving: Option<(Car, CarState)>,
    /// Drive mode's camera.
    chase: Option<ChaseCamera>,
    /// Where the car started, for `R`.
    start_state: Option<CarState>,
    /// Knock-over props and the placements they moved since the last frame.
    props: Props,
    moved: std::collections::HashMap<usize, glam::Mat4>,
}

impl App {
    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> Result<WindowState> {
        let window = Arc::new(event_loop.create_window(
            Window::default_attributes()
                .with_title("NFSU2 — world")
                .with_inner_size(winit::dpi::PhysicalSize::new(1280, 720)),
        )?);
        let instance = Gpu::new_instance();
        let surface = instance.create_surface(window.clone())?;
        let gpu = pollster::block_on(Gpu::new(Some(&surface), instance))?;
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .context("surface not supported by adapter")?;
        let caps = surface.get_capabilities(&gpu.adapter);
        // The game does its colour maths in gamma space and writes the result
        // straight to the screen, so present through a non-sRGB format.
        if let Some((format, space)) = nfsu2::render::display::sdr_surface_format(&caps) {
            config.format = format;
            config.color_space = space;
        }
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&gpu.device, &config);
        let depth = create_depth(&gpu, config.width, config.height);
        let renderer = WorldRenderer::new(&gpu, &self.world, config.format, &nfsu2::render::RenderOptions::in_force());
        let car_renderer = self.driving.as_ref().map(|(car, _)| CarRenderer::new(&gpu, car, config.format, &nfsu2::render::RenderOptions::in_force()));
        Ok(WindowState { window, surface, config, gpu, depth, renderer, car_renderer })
    }

    fn update_car(&mut self, dt: f32) {
        let Some((car, state)) = self.driving.as_mut() else { return };
        let key = |k| self.keys.contains(&k) as u8 as f32;
        let controls = Controls {
            throttle: key(KeyCode::KeyW).max(key(KeyCode::ArrowUp)),
            brake: key(KeyCode::KeyS).max(key(KeyCode::ArrowDown)),
            handbrake: key(KeyCode::Space),
            steer: key(KeyCode::KeyD).max(key(KeyCode::ArrowRight)) - key(KeyCode::KeyA).max(key(KeyCode::ArrowLeft)),
            nitrous: key(KeyCode::AltLeft) > 0.0,
            ..Default::default()
        };
        // Small fixed steps keep the integration stable.
        let steps = (dt / STEP).ceil().max(1.0) as u32;
        for _ in 0..steps {
            self.moved.extend(state.step_with_props(car, &self.world, &mut self.props, controls, dt / steps as f32, &mut Default::default()));
        }
        if let Some(chase) = self.chase.as_mut() {
            self.camera = chase.update(state, dt, nfsu2::qol::fov_offset());
        }
    }

    fn update_camera(&mut self, dt: f32) {
        if self.driving.is_some() {
            self.update_car(dt);
            return;
        }
        let speed = if self.keys.contains(&KeyCode::ShiftLeft) { 400.0 } else { 60.0 } * dt;
        let (f, r) = (self.camera.forward(), self.camera.right());
        let mut v = Vec3::ZERO;
        for (key, dir) in [
            (KeyCode::KeyW, f),
            (KeyCode::KeyS, -f),
            (KeyCode::KeyD, r),
            (KeyCode::KeyA, -r),
            (KeyCode::KeyE, Vec3::Z),
            (KeyCode::KeyQ, -Vec3::Z),
        ] {
            if self.keys.contains(&key) {
                v += dir;
            }
        }
        self.camera.position += v * speed;
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            match self.create_window(event_loop) {
                Ok(state) => self.state = Some(state),
                Err(e) => {
                    eprintln!("could not open a window: {e:#}");
                    event_loop.exit();
                }
            }
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: winit::event::DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event
            && self.looking
        {
            self.camera.yaw += delta.0 as f32 * 0.003;
            self.camera.pitch = (self.camera.pitch - delta.1 as f32 * 0.003).clamp(-1.55, 1.55);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                state.config.width = size.width;
                state.config.height = size.height;
                state.surface.configure(&state.gpu.device, &state.config);
                state.depth = create_depth(&state.gpu, size.width, size.height);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            if code == KeyCode::Escape {
                                event_loop.exit();
                            }
                            if code == KeyCode::KeyC
                                && !event.repeat
                                && let (Some(chase), Some((car, _))) = (&self.chase, &self.driving)
                            {
                                let at = VIEW_ORDER.iter().position(|&k| k == chase.kind()).unwrap_or(0);
                                self.chase = Some(ChaseCamera::new(car, VIEW_ORDER[(at + 1) % VIEW_ORDER.len()]));
                            }
                            if code == KeyCode::KeyR
                                && let (Some(start), Some((car, state))) = (&self.start_state, self.driving.as_mut())
                            {
                                *state = start.clone();
                                if let Some(chase) = self.chase.as_ref() {
                                    self.chase = Some(ChaseCamera::new(car, chase.kind()));
                                }
                            }
                            if code == KeyCode::KeyP {
                                let c = self.camera;
                                println!(
                                    "--cam {:.1},{:.1},{:.1} --look {:.1},{:.1}",
                                    c.position.x,
                                    c.position.y,
                                    c.position.z,
                                    c.yaw.to_degrees(),
                                    c.pitch.to_degrees()
                                );
                            }
                            self.keys.insert(code);
                        }
                        ElementState::Released => {
                            self.keys.remove(&code);
                        }
                    }
                }
            }
            WindowEvent::MouseInput { button: MouseButton::Right, state: s, .. } => {
                self.looking = s == ElementState::Pressed;
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - self.last).as_secs_f32().min(0.1);
                self.last = now;
                self.update_camera(dt);
                let Some(state) = self.state.as_mut() else { return };
                if let Some((_, car)) = &self.driving {
                    state.window.set_title(&format!(
                        "Overbright · {:.0} km/h · gear {} · {:.0} rpm",
                        car.speed().abs() * 3.6,
                        car.gear_name(),
                        car.rpm()
                    ));
                }
                let frame = match state.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                        state.surface.configure(&state.gpu.device, &state.config);
                        return;
                    }
                    _ => return,
                };
                let view = frame.texture.create_view(&Default::default());
                let mut encoder = state.gpu.device.create_command_encoder(&Default::default());
                let aspect = state.config.width as f32 / state.config.height as f32;
                let time = self.started.elapsed().as_secs_f32();
                self.time_of_day.update(1.0 + time, dt);
                let pools: Vec<HeadlightPool> = self.driving.iter().map(|(car, state)| HeadlightPool { transform: state.transform(), color: car.headlight_pool_color }).collect();
                let params = FrameParams {
                    camera: &self.camera,
                    aspect,
                    visible_sections: visible_sections(&self.world, &self.camera), far_sections: None,
                    fog: self.fog.update_weighted(&self.world, self.camera.position.truncate(), self.time_of_day.fog_weight()),
                    time,
                    road_reflections: road_reflections(),
                    road_reflection_plane: None,
                    sky: self.time_of_day.sky(),
                    reversed: false,
                    headlight_pools: &pools,
        reflection_glows: &[],
                    display: Default::default(),
        opts: &nfsu2::render::RenderOptions::in_force(),
                    rear_view: None,
        occluders: &[],
                };
                let size = (state.config.width, state.config.height);
                for (placement, transform) in self.moved.drain() {
                    state.renderer.move_placement(&state.gpu, placement, transform);
                }
                state.renderer.render(&state.gpu, &mut encoder, &view, &state.depth, size, &params);
                if let (Some(car_renderer), Some((car, car_state))) = (&state.car_renderer, &self.driving) {
                    let view_proj = self.camera.view_projection(aspect);
                    let poses = car_state.wheel_poses(car);
                    car_renderer.render(&state.gpu, &mut encoder, &view, &state.depth, view_proj, self.camera.view(), &CarLighting::in_world(&self.world, &self.camera, car_state.transform(), &nfsu2::render::RenderOptions::in_force()), car_state.transform(), &poses, &nfsu2::render::RenderOptions::in_force());
                }
                state.gpu.queue.submit([encoder.finish()]);
                state.gpu.queue.present(frame);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}
