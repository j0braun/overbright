use super::*;
/// A downward lamp's shadow view: from the light, looking down (a spot
/// pointing down keeps its direction), 140° wide, near 0.6 m (past the
/// lamp's own head), far at its outer radius.
pub fn lamp_view(l: &WorldLight) -> Mat4 {
    let dir = if l.kind == 3 && l.direction.z < -0.5 { l.direction.normalize() } else { Vec3::NEG_Z };
    let up = if dir.z.abs() > 0.99 { Vec3::Y } else { Vec3::Z };
    let proj = Mat4::perspective_rh(140f32.to_radians(), 1.0, 0.6, l.outer.max(2.0) * 1.5);
    proj * Mat4::look_to_rh(l.position, dir, up)
}

/// Whether a lamp can cast a downward shadow (it sits above what it lights).
pub fn casts_down(l: &WorldLight) -> bool {
    l.kind == 2 || l.direction.z < -0.5
}

/// Keep the lamps that hold a layer when they are still wanted, give free
/// layers to new ones (at most `budget` new per frame: each costs a draw
/// of the scenery). `layers[i]` is the lamp a layer holds; returns the
/// layers to (re)draw.
pub fn assign_layers(layers: &mut [Option<usize>], wanted: &[usize], budget: usize) -> Vec<usize> {
    for slot in layers.iter_mut() {
        if slot.is_some_and(|l| !wanted.contains(&l)) {
            *slot = None;
        }
    }
    let mut fresh = Vec::new();
    for &l in wanted {
        if fresh.len() >= budget {
            break;
        }
        if layers.contains(&Some(l)) {
            continue;
        }
        if let Some(free) = layers.iter().position(|s| s.is_none()) {
            layers[free] = Some(l);
            fresh.push(free);
        }
    }
    fresh
}

/// The shadow layers' depth format.
pub(in crate::render) const MAP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// New lamp layers drawn per frame at most (each one a draw of the
/// scenery around the lamp; the rest wait for the next frames).
pub(in crate::render) const NEW_LAYERS_PER_FRAME: usize = 2;
/// Lamps farther than this from the eye get no shadow layer.
pub(in crate::render) const SHADOW_REACH: f32 = 70.0;
/// The normal offset of the shadow lookups (metres; grows 2 % a metre).
pub(in crate::render) const SHADOW_BIAS: f32 = 0.06;
/// Shadow layers in the views buffer at most.
pub(in crate::render) const MAX_LAYERS: usize = 32;

pub(in crate::render) struct ShadowMaps {
    pub(in crate::render) quality: u8,
    pub(in crate::render) layers: Vec<wgpu::TextureView>,
    /// Per layer: its frame values and group 0 (with the placeholder maps:
    /// a layer's pass cannot sample the array it draws into).
    pub(in crate::render) frames: Vec<(wgpu::Buffer, wgpu::BindGroup)>,
    /// The lamp each lamp layer holds (the last layer is the headlight's).
    pub(in crate::render) holders: Vec<Option<usize>>,
    pub(in crate::render) views: Vec<GpuShadowView>,
    pub(in crate::render) pipelines: [wgpu::RenderPipeline; 2],
    /// Per layer, when it was last drawn: the count of layer draws then
    /// (`draws`). A lamp's layer is drawn once when it is handed out; a
    /// scenery switch in its view gives it up, so it is handed out and
    /// drawn again ([`WorldRenderer::release_lamp_layers_seeing`]).
    pub(in crate::render) drawn: Vec<u64>,
    pub(in crate::render) draws: u64,
}

/// One shadow layer to draw this frame.
pub(in crate::render) struct ShadowPass {
    pub(in crate::render) target: wgpu::TextureView,
    pub(in crate::render) frame: wgpu::BindGroup,
    pub(in crate::render) cull: PassCull,
    pub(in crate::render) pipelines: [wgpu::RenderPipeline; 2],
}

/// The depth-only pipelines (opaque, alpha-tested) for the shadow layers.
pub(in crate::render) fn shadow_pipelines(gpu: &Gpu, arrays: bool) -> [wgpu::RenderPipeline; 2] {
    let mode = gpu.world_shared().mode_with(&gpu.device, arrays, false, true, false);
    let vertex_attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Unorm8x4, 2 => Float32x2];
    let instance_attributes = wgpu::vertex_attr_array![3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4];
    let buffers = [
        Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<crate::formats::solid::Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &vertex_attributes,
        }),
        Some(wgpu::VertexBufferLayout {
            array_stride: 64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &instance_attributes,
        }),
    ];
    [false, true].map(|alpha_test| {
        crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
            label: Some(if alpha_test { "lamp shadow (alpha test)" } else { "lamp shadow" }),
            layout: Some(&mode.layout),
            vertex: wgpu::VertexState {
                module: &mode.shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &buffers,
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: MAP_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState { constant: 2, slope_scale: 2.5, clamp: 0.0 },
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: alpha_test.then(|| wgpu::FragmentState {
                module: &mode.shader,
                entry_point: Some("fs_shadow_alpha_test"),
                compilation_options: Default::default(),
                targets: &[],
            }),
            multiview_mask: None,
            cache: gpu.disk_pipeline_cache(),
        })
    })
}

/// The headlight projector's frustum (view 9, [`super::headlight_uniform`])
/// as a view-projection, for its shadow layer.
pub fn headlight_view(h: &[[f32; 4]; 5]) -> Option<Mat4> {
    if h[0][3] < 0.5 {
        return None;
    }
    let eye = Vec3::new(h[0][0], h[0][1], h[0][2]);
    let up = Vec3::new(h[2][0], h[2][1], h[2][2]);
    let dir = Vec3::new(h[3][0], h[3][1], h[3][2]);
    let (across, down) = (h[1][3], h[2][3]);
    if across <= 0.0 || down <= 0.0 {
        return None;
    }
    let proj = Mat4::perspective_rh(2.0 * (1.0 / down).atan(), down / across, 0.3, 60.0);
    Some(proj * Mat4::look_to_rh(eye, dir, up))
}

impl WorldCore {
    /// Draw this frame's shadow layers (before the main view samples them).
    pub(in crate::render) fn record_lamp_shadows(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        passes: &[ShadowPass],
        visible: &[bool],
        reversed: bool,
        time: f32,
        lod: u8,
    ) {
        for p in passes {
            // The main view's LOD: the surfaces the shadows fall on are those.
            let order =
                self.group_order(&self.kept_groups(visible, &p.cull, WorldView::Main, reversed, false, None, lod));
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("lamp shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &p.target,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &p.frame, &[]);
            pass.set_bind_group(2, &self.no_reflection, &[]);
            pass.set_vertex_buffer(0, self.vertices.slice(..));
            pass.set_vertex_buffer(1, self.instances.slice(..));
            pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);
            let (mut current, mut current_texture, mut bound) = (None, usize::MAX, usize::MAX);
            for d in order.iter().map(|&i| &self.draws[i as usize]) {
                if d.sky || d.key.blend != crate::formats::texture::Blend::None {
                    continue;
                }
                let texture = match d.texture {
                    TextureRef::Fixed(t) => t,
                    TextureRef::Animated(a) => {
                        let anim = &self.animations[a];
                        anim.frames[(time * anim.fps) as usize % anim.frames.len()]
                    }
                    TextureRef::Sky(_) => continue,
                };
                if current != Some(d.key.alpha_test) {
                    pass.set_pipeline(&p.pipelines[usize::from(d.key.alpha_test)]);
                    current = Some(d.key.alpha_test);
                    current_texture = usize::MAX;
                }
                if texture != current_texture {
                    self.set_world_texture(&mut pass, texture, &mut bound);
                    current_texture = texture;
                }
                pass.draw_indexed(d.first_index..d.first_index + d.index_count, d.base_vertex, d.instances.clone());
            }
        }
        if !passes.is_empty() {
            crate::prof::gpu::mark(encoder, "graphics: lamp shadows");
        }
    }
}

/// docs/notes/graphics/atmosphere.md: this frame's lamp shadow layers for the
/// Next-Gen atmosphere's light shafts: the maps (a 2D array view), each
/// layer's view, and the lamp each lamp layer holds (an index into
/// [`WorldRenderer::lamp_list`]); the last layer is the headlight's.
pub struct ShadowLayers {
    pub maps: wgpu::TextureView,
    pub views: Vec<GpuShadowView>,
    pub holders: Vec<Option<usize>>,
    /// Per layer, the count of layer draws when it was last drawn (0:
    /// never): a layer kept as drawn keeps its number (the tests).
    pub drawn: Vec<u64>,
    /// The headlight's layer, when it was drawn this frame.
    pub headlight: Option<usize>,
}

impl WorldRenderer {
    /// The lamp shadow layers (None with Lamp Shadows and Modern
    /// Lighting's layers off). Read-only; nothing here changes a frame.
    pub fn lamp_shadow_layers(&self) -> Option<ShadowLayers> {
        let state = self.lamp.as_ref()?;
        let m = state.maps.as_ref()?;
        let headlight = (m.views.len() > m.holders.len() && m.views[m.holders.len()].view_proj != [[0.0; 4]; 4])
            .then_some(m.holders.len());
        Some(ShadowLayers {
            maps: state.bindings.maps.clone(),
            views: m.views.clone(),
            holders: m.holders.clone(),
            drawn: m.drawn.clone(),
            headlight,
        })
    }

    /// A placement a scenery switch changed (`update_placement`: a
    /// drive's barriers, a backward race's mirrored barriers, the track's
    /// world animations), as its instances' bounds before and after: each
    /// lamp's shadow layer whose view holds one of them is given up. A
    /// layer is drawn once, when it is handed out; given up, it is handed
    /// out again at the next step of the layers (the lamp still wanted)
    /// and drawn with the scenery as it is now, as a new drive's layer is.
    /// The others are kept as drawn. Nothing per frame: only a switch
    /// calls this.
    pub(in crate::render) fn release_lamp_layers_seeing(&mut self, bounds: &[(Vec3, f32)]) {
        let Some(m) = self.lamp.as_ref().and_then(|s| s.maps.as_ref()) else { return };
        let stale: Vec<usize> = (0..m.holders.len())
            .filter(|&layer| m.holders[layer].is_some_and(|l| self.lamp_view_holds(l, bounds)))
            .collect();
        if let Some(m) = self.lamp.as_mut().and_then(|s| s.maps.as_mut()) {
            for layer in stale {
                m.holders[layer] = None;
            }
        }
    }

    /// Shadow layer `layer`'s depth as drawn (its bytes, row by row; the
    /// tests: a layer kept as drawn against one drawn anew). None without
    /// the layers.
    pub fn lamp_shadow_layer_depth(&self, gpu: &Gpu, layer: u32) -> Option<Vec<u8>> {
        let m = self.lamp.as_ref()?.maps.as_ref()?;
        let texture = m.layers.get(layer as usize)?.texture();
        let size = texture.width();
        let row = size * 4;
        let bytes_per_row = row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lamp shadow layer read-back"),
            size: u64::from(bytes_per_row) * u64::from(size),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y: 0, z: layer },
                aspect: wgpu::TextureAspect::DepthOnly,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        );
        gpu.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        let mapped = slice.get_mapped_range().ok()?;
        Some(mapped.chunks(bytes_per_row as usize).flat_map(|line| line[..row as usize].to_vec()).collect())
    }

    /// The lamps' state dropped as a new renderer has it (none yet: every
    /// shadow layer handed out and drawn anew), as a kept renderer's reset
    /// drops it (`reset_instances`). The tests' reference for the layers a
    /// fresh load draws.
    pub fn lamps_afresh(&mut self) {
        self.lamp = None;
    }

    /// Lamp `lamp`'s shadow view ([`lamp_view`]: it reaches 1.5 × the
    /// outer radius) holds one of `bounds` (spheres).
    pub fn lamp_view_holds(&self, lamp: usize, bounds: &[(Vec3, f32)]) -> bool {
        let Some(l) = self.lamps.get(lamp).map(|l| &l.light) else { return false };
        let reach = l.outer.max(2.0) * 1.5;
        let mut near = bounds.iter().filter(|(c, r)| c.distance(l.position) <= reach + r).peekable();
        if near.peek().is_none() {
            return false;
        }
        let planes = crate::render::frustum_planes(lamp_view(l));
        near.any(|&(c, r)| !crate::render::outside(&planes, c, r))
    }
}
