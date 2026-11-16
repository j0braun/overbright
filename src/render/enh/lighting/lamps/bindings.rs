use super::*;
// ---------------------------------------------------------------------
// GPU side: the world views' group 0 bindings 3–8 and the shadow layers.

/// The world views' group 0, bindings 3–8.
pub(in crate::render) fn layout_entries() -> [wgpu::BindGroupLayoutEntry; 6] {
    let storage = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };
    [
        storage(3),
        storage(4),
        storage(5),
        storage(6),
        wgpu::BindGroupLayoutEntry {
            binding: 7,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2Array,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 8,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
            count: None,
        },
    ]
}

/// What bindings 3–10 point at (9–10: Surface Detail's normal maps,
/// [`super::surface`]).
#[derive(Clone)]
pub(in crate::render) struct LampBindings {
    pub(in crate::render) lights: wgpu::Buffer,
    pub(in crate::render) clusters: wgpu::Buffer,
    pub(in crate::render) views: wgpu::Buffer,
    pub(in crate::render) occluders: wgpu::Buffer,
    pub(in crate::render) maps: wgpu::TextureView,
    pub(in crate::render) sampler: wgpu::Sampler,
    pub(in crate::render) surface: wgpu::TextureView,
    pub(in crate::render) surface_sampler: wgpu::Sampler,
    /// 12–14: Wet Road Interaction's values, trail map and sampler (
    /// [`super::wet_road`]; placeholders off the main view or with it off).
    pub(in crate::render) wet: crate::render::wet_road::WetBindings,
}

pub(in crate::render) fn storage_buffer(device: &wgpu::Device, label: &str, size: u64) -> wgpu::Buffer {
    crate::render::zeroed_buffer(
        device,
        &wgpu::BufferDescriptor {
            label: Some(label),
            size: size.max(256),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        },
    )
}

pub(in crate::render) fn map_array(device: &wgpu::Device, size: u32, layers: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("lamp shadow maps"),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: layers },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: MAP_FORMAT,
        // (COPY_SRC: a layer read back, `WorldRenderer::lamp_shadow_layer_depth`.)
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

pub(in crate::render) fn array_view(t: &wgpu::Texture) -> wgpu::TextureView {
    t.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    })
}

impl LampBindings {
    /// Small placeholders: what every view but the main one binds (its
    /// frame never turns the lamps on), and the shadow layers' own passes.
    pub(in crate::render) fn placeholder(device: &wgpu::Device) -> LampBindings {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("lamp shadow compare"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        LampBindings {
            lights: storage_buffer(device, "lamps (none)", 256),
            clusters: storage_buffer(device, "lamp clusters (none)", 256),
            views: storage_buffer(device, "lamp shadow views (none)", 256),
            occluders: storage_buffer(device, "car occluders (none)", 256),
            maps: array_view(&map_array(device, 1, 1)),
            sampler,
            surface: crate::render::surface::placeholder(device),
            surface_sampler: crate::render::surface::sampler(device),
            wet: crate::render::wet_road::WetBindings::placeholder(device),
        }
    }

    pub(in crate::render) fn entries(&self) -> [wgpu::BindGroupEntry<'_>; 11] {
        let [w12, w13, w14] = self.wet.entries();
        [
            wgpu::BindGroupEntry { binding: 3, resource: self.lights.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 4, resource: self.clusters.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 5, resource: self.views.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 6, resource: self.occluders.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(&self.maps) },
            wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::TextureView(&self.surface) },
            wgpu::BindGroupEntry { binding: 10, resource: wgpu::BindingResource::Sampler(&self.surface_sampler) },
            w12,
            w13,
            w14,
        ]
    }
}

/// The lamps' per-renderer state, made when an option first asks.
pub(in crate::render) struct LampState {
    pub(in crate::render) bindings: LampBindings,
    pub(in crate::render) placeholder_maps: wgpu::TextureView,
    pub(in crate::render) maps: Option<ShadowMaps>,
    pub(in crate::render) main_group: wgpu::BindGroup,
    /// Surface Detail's maps bound in `main_group` (else the
    /// placeholder).
    pub(in crate::render) surface_bound: bool,
    /// The last world tick the lamps' fades and shadow layers
    /// stepped (`render::enh::tick_views`).
    pub(in crate::render) gate: crate::render::enh::tick_views::TickGate,
}

impl WorldRenderer {
    /// Wet Road Interaction: the lamps' bindings take `wet` (their
    /// main group rebuilt), when the lamps' state exists.
    pub(in crate::render) fn rebind_lamps_wet(
        &mut self,
        gpu: &crate::render::Gpu,
        wet: crate::render::wet_road::WetBindings,
    ) {
        let device = &gpu.device;
        let Some(state) = self.lamp.as_mut() else { return };
        state.bindings.wet = wet;
        let bindings = state.bindings.clone();
        let group = self.frame_group(device, &self.core.main_frame.0, &bindings);
        self.lamp.as_mut().unwrap().main_group = group;
        // Modern + RT: its scene group carries the same bindings.
        if self.core.scene_pipelines().mode.modern == 2 {
            let rt = self.frame_group_rt(gpu, &self.core.main_frame.0, &bindings);
            *self.core.lamp_main_rt.write().unwrap() = Some(rt);
        }
    }

    /// The lamps' light list and clusters (Modern + RT's traced pass).
    pub(in crate::render) fn lamp_rt_buffers(&self) -> Option<(wgpu::Buffer, wgpu::Buffer)> {
        self.lamp.as_ref().map(|s| (s.bindings.lights.clone(), s.bindings.clusters.clone()))
    }

    /// The lamps' main group (with the row's bindings once rebound).
    pub(in crate::render) fn lamp_main_group_with_wet(&self, _device: &wgpu::Device) -> Option<wgpu::BindGroup> {
        self.lamp.as_ref().map(|s| s.main_group.clone())
    }

    /// A world view's group 0 with `buffer` as its frame values.
    pub(in crate::render) fn frame_group(
        &self,
        device: &wgpu::Device,
        buffer: &wgpu::Buffer,
        lamps: &LampBindings,
    ) -> wgpu::BindGroup {
        let [e3, e4, e5, e6, e7, e8, e9, e10, e12, e13, e14] = lamps.entries();
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame (lamps)"),
            layout: &self.frame_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.headlight.0) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.headlight.1) },
                e3,
                e4,
                e5,
                e6,
                e7,
                e8,
                e9,
                e10,
                e12,
                e13,
                e14,
            ],
        })
    }
}
