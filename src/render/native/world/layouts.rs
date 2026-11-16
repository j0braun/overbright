//! The native world's bind group layouts and texture bind groups (audit
//! plan render §1: moved from render/mod.rs).

use wgpu::util::DeviceExt;

use crate::formats::texture::Scroll;
use crate::render::{BOUND_ARRAYS, lamps, scroll_params, surface, wet_road};

/// A world texture's bind group: the texture, the sampler and its scroll
/// parameters.
pub(crate) fn bind_world_texture(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    view: &wgpu::TextureView,
    scroll: &Scroll,
) -> wgpu::BindGroup {
    let params = scroll_params(scroll);
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("scroll"),
        contents: bytemuck::cast_slice(&params),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: buffer.as_entire_binding() },
        ],
    })
}

/// The texture arrays' group 1 ([`TextureArrays`]).
pub(crate) fn array_layout(device: &wgpu::Device, bound: bool) -> wgpu::BindGroupLayout {
    crate::render::core::device_cache::uncached_bind_group_layout(device, &wgpu::BindGroupLayoutDescriptor {
        label: Some("world texture array"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: bound.then(|| std::num::NonZeroU32::new(BOUND_ARRAYS).unwrap()),
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// The world views' group 0: the frame's uniforms and HEADLIGHTTEX.
pub(crate) fn frame_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    frame_layout_with(device, &[])
}

/// [`frame_layout`] with one more entry (Modern + RT: the traced
/// scene at binding 11, `modern_light_bold::frame_layout_rt`).
pub(crate) fn frame_layout_with(device: &wgpu::Device, extra: &[wgpu::BindGroupLayoutEntry]) -> wgpu::BindGroupLayout {
    let [l3, l4, l5, l6, l7, l8] = lamps::layout_entries();
    let [l9, l10] = surface::layout_entries();
    let [l12, l13, l14] = wet_road::layout_entries();
    let base = [
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
        // 3–8: Dynamic Lamps / Lamp Shadows / Contact Shadows (clone
        // deviations, [`lamps::layout_entries`]).
        l3,
        l4,
        l5,
        l6,
        l7,
        l8,
        // 9–10: Surface Detail's normal maps ([`surface`]).
        l9,
        l10,
        // 12–14: Wet Road Interaction ([`wet_road`]; 11 is the RT
        // tier's scene, `extra`).
        l12,
        l13,
        l14,
    ];
    let mut entries = base.to_vec();
    entries.extend_from_slice(extra);
    crate::render::core::device_cache::uncached_bind_group_layout(device, &wgpu::BindGroupLayoutDescriptor { label: Some("frame"), entries: &entries })
}

/// Texture + sampler only.
pub(crate) fn make_texture_layout_plain(device: &wgpu::Device, label: &str) -> wgpu::BindGroupLayout {
    make_texture_layout(device, label, false)
}

/// Texture + sampler, plus (for game textures) the scroll uniform.
pub(crate) fn make_texture_layout(device: &wgpu::Device, label: &str, with_scroll: bool) -> wgpu::BindGroupLayout {
    let scroll = wgpu::BindGroupLayoutEntry {
        binding: 2,
        visibility: wgpu::ShaderStages::VERTEX,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };
    let entries = [
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
        scroll,
    ];
    let entries = if with_scroll { &entries[..] } else { &entries[..2] };
    crate::render::core::device_cache::uncached_bind_group_layout(device, &wgpu::BindGroupLayoutDescriptor { label: Some(label), entries })
}

pub(crate) fn bind_texture(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}
