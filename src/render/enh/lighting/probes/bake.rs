use super::*;
// ---------------------------------------------------------------------
// GPU side: the bake target and the read-back.

use crate::render::{FrameParams, FrameUniform, Gpu, PassCull, WorldRenderer, WorldView};

/// Bytes per read-back row (wgpu's 256-byte alignment).
pub(in crate::render) const ROW_BYTES: u32 = 256;

/// The outcome of a GPU read-back map, set by its `map_async` callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::render) enum MapState {
    Pending,
    Ready,
    Failed,
}

/// A read-back map's tri-state flag: a failed map (device lost, out of
/// memory) must be seen as failed, not as "still pending".
#[derive(Default)]
pub(in crate::render) struct MapFlag(std::sync::atomic::AtomicU8);

impl MapFlag {
    pub(in crate::render) fn set(&self, ok: bool) {
        self.0.store(if ok { 1 } else { 2 }, std::sync::atomic::Ordering::Release);
    }

    pub(in crate::render) fn state(&self) -> MapState {
        match self.0.load(std::sync::atomic::Ordering::Acquire) {
            0 => MapState::Pending,
            1 => MapState::Ready,
            _ => MapState::Failed,
        }
    }

    /// Drives `poll` (false = the device poll itself failed) until the map
    /// settles, at most `polls` times; a map still pending after that, or a
    /// failing poll, counts as failed.
    pub(in crate::render) fn wait(&self, polls: u32, mut poll: impl FnMut() -> bool) -> MapState {
        for _ in 0..polls {
            match self.state() {
                MapState::Pending => {
                    if !poll() {
                        break;
                    }
                }
                s => return s,
            }
        }
        match self.state() {
            MapState::Pending => MapState::Failed,
            s => s,
        }
    }
}

/// Says once that a probe read-back failed.
pub(in crate::render) fn warn_read_back_failed() {
    static SAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !SAID.swap(true, std::sync::atomic::Ordering::Relaxed) {
        log::warn!("light probes: a read-back failed; the probe is skipped");
    }
}

pub(in crate::render) struct ProbeBaker {
    field: Arc<RwLock<ProbeField>>,
    faces: Vec<wgpu::TextureView>,
    texture: wgpu::Texture,
    depth: wgpu::TextureView,
    frames: Vec<(wgpu::Buffer, wgpu::BindGroup)>,
    readback: wgpu::Buffer,
    /// The probe being read back and whether its map has completed.
    pending: Option<(u32, Arc<MapFlag>)>,
    /// The pending probe was given up by a set-up's switch after it was
    /// drawn: its read-back is not stored.
    discard: bool,
    /// The race direction the field was baked under (the environment view
    /// culls the forward-only and backward-only scenery by it).
    reversed: Option<bool>,
    /// Probes given up since the last drive's start (the tests).
    released: usize,
    /// The last world tick a bake was planned at
    /// (`render::enh::tick_views`): at most one new probe a tick.
    gate: crate::render::enh::tick_views::TickGate,
}

/// The six face axes in world XYZ (`env_face_axes` of the enhanced cube).
pub(in crate::render) fn face_axes() -> [(Vec3, Vec3, Vec3); 6] {
    crate::render::env_face_axes(Vec3::X, Vec3::Y, Vec3::Z)
}

/// Which ambient-cube slot (+x, −x, +y, −y, +z, −z) a face looks along.
pub(in crate::render) fn slot_of(forward: Vec3) -> usize {
    let a = forward.abs();
    if a.x >= a.y && a.x >= a.z {
        usize::from(forward.x < 0.0)
    } else if a.y >= a.z {
        2 + usize::from(forward.y < 0.0)
    } else {
        4 + usize::from(forward.z < 0.0)
    }
}

impl WorldRenderer {
    /// Light Probes: bake the probes near the player's car (one a frame,
    /// or a burst on arrival) and publish the field for the car lighting.
    pub(in crate::render) fn bake_probes(&mut self, gpu: &Gpu, params: &FrameParams) {
        if !params.opts.light_probes {
            return;
        }
        let _span = crate::prof::acc::span("world: light probes");
        if !self.ensure_probe_baker(gpu) {
            return;
        }
        // Around the player's car (the first headlight pool; the
        // probes light the car, `terms_at`), not the eye: a camera flying
        // away from the car (replays, photo mode, the bench's fountain
        // orbit) baked new probes every frame. Without a car: the eye.
        let eye = params.headlight_pools.first().map_or(params.camera.position, |p| p.transform.w_axis.truncate());
        // The read-back of the last probe, if done.
        let device = &gpu.device;
        let _ = device.poll(wgpu::PollType::Poll);
        let mut baker = self.probes.take().unwrap();
        if let Some((probe, done)) = baker.pending.clone() {
            match done.state() {
                MapState::Ready if baker.discard => {
                    baker.readback.unmap();
                    baker.pending = None;
                    baker.discard = false;
                }
                MapState::Ready => baker.finish(probe),
                MapState::Failed => {
                    warn_read_back_failed();
                    baker.pending = None;
                    baker.discard = false;
                }
                MapState::Pending => {}
            }
        }
        // A race direction other than the one the field was baked under:
        // the probes that see scenery whose culling it switches are baked
        // again.
        if baker.reversed.is_some_and(|r| r != params.reversed) {
            let bounds = self.direction_switched_bounds();
            self.release_probes(&mut baker, &bounds);
        }
        baker.reversed = Some(params.reversed);
        publish(Some(baker.field.clone()));
        let due = match baker.gate.due(crate::render::enh::tick_views::frame().as_deref()) {
            crate::render::enh::tick_views::Due::Draw => true,
            crate::render::enh::tick_views::Due::Ticks(v) => !v.is_empty(),
        };
        let plan = baker.field.read().unwrap().plan(eye);
        match plan {
            // Arrival: every unbaked probe within the burst radius now. Each
            // probe is drawn and copied into a read-back buffer of its own
            // and submitted without waiting for the one before, then the
            // maps are waited for together: one wait for the GPU for the
            // whole burst instead of one per probe (each wait costs a turn
            // of the GPU's queue when another program is drawing too). The
            // probes are drawn with the same commands in the same order and
            // stored in the same order, so the field is the same.
            BakePlan::Burst(near) if baker.pending.is_none() => {
                let serial = burst_serial();
                let burst: Vec<(u32, wgpu::Buffer, Arc<MapFlag>)> = near
                    .into_iter()
                    .map(|i| {
                        let readback = ProbeBaker::readback_buffer(device);
                        let done = self.draw_probe(gpu, &baker, params, i, &readback);
                        if serial {
                            // The check aid: each probe waited for before the next.
                            done.wait(64, || device.poll(wgpu::PollType::wait_indefinitely()).is_ok());
                        }
                        (i, readback, done)
                    })
                    .collect();
                for (i, readback, done) in &burst {
                    if done.wait(64, || device.poll(wgpu::PollType::wait_indefinitely()).is_ok()) != MapState::Ready {
                        // Give up for this frame.
                        warn_read_back_failed();
                        break;
                    }
                    baker.store(*i, readback);
                }
                baker.field.write().unwrap().rebake = false;
            }
            // One probe a world tick (a draw without published
            // ticks: one a draw, as before).
            BakePlan::One(i) if baker.pending.is_none() && due => {
                baker.field.write().unwrap().rebake = false;
                self.bake_one(gpu, &mut baker, params, i)
            }
            BakePlan::Nothing => baker.field.write().unwrap().rebake = false,
            _ => {}
        }
        if log_on() {
            let field = baker.field.read().unwrap();
            let near = field.near(eye, 40.0);
            log::warn!(
                "light probes: {} placed, {} baked, eye {eye}, near {:?}",
                field.positions.len(),
                field.baked.iter().filter(|b| b.is_some()).count(),
                near.iter().take(3).map(|(d, i)| (*d, field.baked[*i as usize])).collect::<Vec<_>>()
            );
        }
        self.probes = Some(baker);
    }

    /// The probe field along the region's roads and its baker, made once
    /// (false: no region to place them in). Also at a drive's load
    /// ([`WorldRenderer::warm_drive`]).
    pub(in crate::render) fn ensure_probe_baker(&mut self, gpu: &Gpu) -> bool {
        if self.probes.is_none() {
            let Some((root, region)) = self.probe_source.clone() else { return false };
            let field = Arc::new(RwLock::new(ProbeField::along_roads(&region_routes(&root, &region))));
            self.probes = Some(ProbeBaker::new(gpu, self, field));
        }
        true
    }

    /// Light Probes at a drive's start, once its barriers are set up (every
    /// path: a new renderer, a kept one's reset, the menus' kept world, a
    /// drive kept on the spot): the probes whose bake can see a placement
    /// the set-up shows, hides or moves since the last drive are given up and
    /// baked again — the nearest at once in the first frame, as a fresh
    /// load bakes them — the others kept. Nothing to do when no placement
    /// changed (a probe draws only placements; the fog maps are the world's
    /// and the same for every drive).
    ///
    /// `on`: Light Probes in force. Off, nothing is recorded and a field
    /// left from before is dropped (made anew if the row is turned on).
    pub fn new_drive_probes(&mut self, world: &crate::world::World, on: bool) {
        if !on {
            if self.probes.take().is_some() {
                publish(None);
            }
            self.probe_scene = None;
            return;
        }
        let now: Vec<(bool, glam::Mat4)> =
            world.placements.iter().map(|p| (p.flags & crate::world::HIDDEN_FLAG != 0, p.transform)).collect();
        let was = self.probe_scene.replace(now);
        let Some(mut baker) = self.probes.take() else { return };
        baker.released = 0;
        match &was {
            Some(was) => {
                let bounds = self.scene_switched_bounds(world, was);
                self.release_probes(&mut baker, &bounds);
            }
            // Baked before any record (none: a field is made in a drive's
            // frames, after its start): all of it again.
            None => {
                let f = baker.field.clone();
                let mut field = f.write().unwrap();
                baker.released = field.baked.iter().filter(|b| b.is_some()).count();
                field.baked.fill(None);
                field.rebake = true;
                baker.discard = baker.pending.is_some();
            }
        }
        self.probes = Some(baker);
    }

    /// The bounds (before and after) of the placements shown in either
    /// state whose hidden flag or transform differs between `was` and the
    /// record now, for the groups the environment view can draw.
    fn scene_switched_bounds(&self, world: &crate::world::World, was: &[(bool, glam::Mat4)]) -> Vec<(Vec3, f32)> {
        let Some(now) = self.probe_scene.as_ref() else { return Vec::new() };
        let mut bounds = Vec::new();
        for (i, (a, b)) in was.iter().zip(now).enumerate() {
            if a == b || (a.0 && b.0) {
                continue;
            }
            let has_matrix = world.placements[i].flags & crate::render::NO_MATRIX_FLAG == 0;
            for &slot in self.placement_slots.get(i).map_or(&[][..], |s| &s[..]) {
                let g = self.slot_group[slot as usize] as usize;
                if !self.environment_can_draw(g) {
                    continue;
                }
                let local = self.group_local[g];
                for (hidden, m) in [a, b] {
                    if !hidden {
                        bounds.push(crate::render::instance_spheres(local, *m, has_matrix)[0]);
                    }
                }
            }
        }
        bounds
    }

    /// The bounds of the instances of groups the environment view culls in
    /// one race direction and not in the other.
    fn direction_switched_bounds(&self) -> Vec<(Vec3, f32)> {
        let spheres = self.slot_spheres.read().unwrap();
        let mut bounds = Vec::new();
        for (g, (is_sky, flags, _, instances)) in self.group_key.iter().enumerate() {
            let env = WorldView::Environment;
            if *is_sky || env.culls(*flags, false) == env.culls(*flags, true) || !self.environment_can_draw(g) {
                continue;
            }
            bounds.extend(instances.clone().map(|i| spheres[i as usize][0]));
        }
        bounds
    }

    /// Whether the environment view (a probe's bake) draws group `g` in
    /// some race direction: not the dome, which follows the probe.
    fn environment_can_draw(&self, g: usize) -> bool {
        let (is_sky, flags, _, _) = self.group_key[g];
        let env = WorldView::Environment;
        !is_sky && !(env.culls(flags, false) && env.culls(flags, true))
    }

    /// Gives up the baked probes that see one of `bounds` (a pending
    /// probe among them is not stored).
    fn release_probes(&self, baker: &mut ProbeBaker, bounds: &[(Vec3, f32)]) {
        if bounds.is_empty() {
            return;
        }
        let pending = baker.pending.as_ref().map(|p| p.0);
        let f = baker.field.clone();
        let mut field = f.write().unwrap();
        let released = field.release_seeing(bounds, crate::render::ENV_FAR);
        // A probe drawn but not read back yet is drawn with the scenery
        // before the switch when a bound is within its reach.
        if let Some(p) = pending {
            let c = field.positions[p as usize];
            let far = crate::render::ENV_FAR;
            if bounds.iter().any(|&(b, r)| ((b - c).abs() - Vec3::splat(far)).max(Vec3::ZERO).length_squared() <= r * r)
            {
                baker.discard = true;
                field.rebake = true;
            }
        }
        baker.released += released.len();
        if log_on() {
            log::warn!("light probes: {} given up by a set-up's switch", released.len());
        }
    }

    /// The probes' field dropped as a new renderer has it (none yet: made
    /// and baked anew), the tests' reference for the field a fresh load
    /// bakes.
    pub fn probes_afresh(&mut self) {
        self.probes = None;
        publish(None);
    }

    /// Probes given up since the last drive's start (None: no field yet).
    pub fn probes_released(&self) -> Option<usize> {
        self.probes.as_ref().map(|b| b.released)
    }

    /// Draw probe `i`'s six faces and start their read-back.
    pub(super) fn bake_one(&self, gpu: &Gpu, baker: &mut ProbeBaker, params: &FrameParams, i: u32) {
        let done = self.draw_probe(gpu, baker, params, i, &baker.readback);
        baker.pending = Some((i, done));
    }

    /// Draw probe `i`'s six faces, copy them into `readback` and start its
    /// map (the flag is set when the map completes). The submission is
    /// not waited for.
    fn draw_probe(
        &self,
        gpu: &Gpu,
        baker: &ProbeBaker,
        params: &FrameParams,
        i: u32,
        readback: &wgpu::Buffer,
    ) -> Arc<MapFlag> {
        let centre = baker.field.read().unwrap().positions[i as usize];
        let visible = crate::render::visible_sections(params);
        let main_lod = if params.display.world_lod_b { crate::render::MAIN_LOD_B } else { crate::render::MAIN_LOD };
        let fog = params.fog.unwrap_or(crate::render::NO_FOG);
        let projection = glam::Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.5, crate::render::ENV_FAR);
        let planes = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z]
            .map(|n| n.extend(crate::render::ENV_FAR - n.dot(centre)))
            .to_vec();
        let cube =
            PassCull { planes, mirrored: false, eye: centre, forward: Vec3::Z, scale: 0.0, min_size: 0.0, fades: None };
        let mut encoder =
            gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("light probe") });
        // The GPU's time timed from here (the submissions before excluded).
        crate::prof::gpu::mark(&mut encoder, crate::prof::gpu::GAP);
        let core = &self.core;
        let groups = core.kept_groups(&visible, &cube, WorldView::Environment, params.reversed, false, None, main_lod);
        for (f, (r, d, fwd)) in face_axes().into_iter().enumerate() {
            let rotation =
                glam::Mat4::from_cols(r.extend(0.0), (-d).extend(0.0), (-fwd).extend(0.0), Vec4::W).transpose();
            let view_proj = projection * rotation * glam::Mat4::from_translation(-centre);
            let uniform = FrameUniform {
                view_proj: view_proj.to_cols_array_2d(),
                camera: centre.extend(1.0).to_array(),
                forward: fwd.extend(0.0).to_array(),
                fog_value: [fog.value[0], fog.value[1], fog.value[2], 0.0],
                fog_color: [fog.color[0], fog.color[1], fog.color[2], 1.0],
                clip: [f32::MIN, params.time, 0.0, 0.0],
                cull: [crate::render::cull_scale(std::f32::consts::FRAC_PI_2), 0.0, 0.0, 0.0],
                headlight: [[0.0; 4]; 5],
                wet: [0.0; 4],
                lamps: [0.0; 4],
                cluster: [0.0; 4],
                cluster_near: [0.0; 4],
            };
            gpu.queue.write_buffer(&baker.frames[f].0, 0, bytemuck::bytes_of(&uniform));
            let cull = PassCull {
                planes: crate::render::frustum_planes(view_proj),
                mirrored: false,
                eye: centre,
                forward: fwd,
                scale: crate::render::cull_scale(std::f32::consts::FRAC_PI_2),
                min_size: 0.0,
                fades: None,
            };
            let order = core.group_order(&core.kept_groups(
                &visible,
                &cull,
                WorldView::Environment,
                params.reversed,
                false,
                Some(&groups),
                main_lod,
            ));
            core.draw_pass(
                &mut encoder,
                &baker.faces[f],
                &baker.depth,
                wgpu::Color::BLACK,
                &baker.frames[f].1,
                &core.no_reflection,
                &order,
                params.time,
                None,
                WorldView::Environment,
                (0, 0),
            );
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &baker.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(ROW_BYTES),
                    rows_per_image: Some(SIZE),
                },
            },
            wgpu::Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 6 },
        );
        crate::prof::gpu::mark(&mut encoder, "world: light probes");
        gpu.queue.submit([encoder.finish()]);
        let done = Arc::new(MapFlag::default());
        let flag = done.clone();
        readback.slice(..).map_async(wgpu::MapMode::Read, move |r| flag.set(r.is_ok()));
        done
    }
}

impl ProbeBaker {
    pub(super) fn new(gpu: &Gpu, renderer: &WorldRenderer, field: Arc<RwLock<ProbeField>>) -> ProbeBaker {
        let device = &gpu.device;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("light probe"),
            size: wgpu::Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 6 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: crate::render::REFLECTION_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let faces = (0..6)
            .map(|i| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: i,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let placeholder = crate::render::lamps::LampBindings::placeholder(device);
        let frames = (0..6)
            .map(|_| {
                let buffer = crate::render::zeroed_buffer(
                    device,
                    &wgpu::BufferDescriptor {
                        label: Some("light probe frame"),
                        size: std::mem::size_of::<FrameUniform>() as u64,
                        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    },
                );
                let group = renderer.frame_group(device, &buffer, &placeholder);
                (buffer, group)
            })
            .collect();
        let readback = ProbeBaker::readback_buffer(device);
        ProbeBaker {
            field,
            faces,
            texture,
            depth: crate::render::transient_depth(gpu, SIZE, SIZE),
            frames,
            readback,
            pending: None,
            discard: false,
            reversed: None,
            released: 0,
            gate: Default::default(),
        }
    }

    /// A buffer one probe's six faces are read back through.
    fn readback_buffer(device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("light probe read-back"),
            size: u64::from(ROW_BYTES * SIZE * 6),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Store the pending probe's ambient cube from the baker's own
    /// read-back buffer.
    pub(super) fn finish(&mut self, probe: u32) {
        let readback = self.readback.clone();
        self.store(probe, &readback);
        self.pending = None;
    }

    /// Store probe `probe`'s ambient cube from `readback`, whose map has
    /// completed (the buffer is unmapped again).
    fn store(&mut self, probe: u32, readback: &wgpu::Buffer) {
        let means = {
            let Ok(data) = readback.slice(..).get_mapped_range() else {
                return;
            };
            face_means(&data, ROW_BYTES as usize)
        };
        readback.unmap();
        let mut cube = [Vec3::ZERO; 6];
        for (f, (_, _, fwd)) in face_axes().into_iter().enumerate() {
            cube[slot_of(fwd)] = means[f];
        }
        if log_on() {
            let chroma: f32 = cube.iter().map(|c| (*c - Vec3::splat(c.dot(LUMA_WEIGHTS))).length()).fold(0.0, f32::max);
            log::warn!(
                "probe {probe} at {} chroma {chroma:.3} cube {cube:?}",
                self.field.read().unwrap().positions[probe as usize]
            );
        }
        self.field.write().unwrap().baked[probe as usize] = Some(cube);
    }
}
