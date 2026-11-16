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
                MapState::Ready => baker.finish(probe),
                MapState::Failed => {
                    warn_read_back_failed();
                    baker.pending = None;
                }
                MapState::Pending => {}
            }
        }
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
            }
            // One probe a world tick (a draw without published
            // ticks: one a draw, as before).
            BakePlan::One(i) if baker.pending.is_none() && due => self.bake_one(gpu, &mut baker, params, i),
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
