//! Surface Detail's GPU side (a clone deviation, opt-in; docs/surface-maps.md,
//! `crate::surface_maps`): the generated normal maps as one RG8 snorm
//! texture array ([`MAP_SIZE`]², full mips), bound at group 0 bindings 9–10
//! of the world views (a 1-layer placeholder everywhere else), and per world
//! texture slot its layer (0 = none) and packed material, which the draws
//! carry in the immediates' padding (`TextureImmediates`). Only the lamps'
//! module (world.wgsl's LAMPS blocks) reads any of it; the native shaders
//! never do.

use crate::surface_maps::{Builder, Class, MAP_MIPS, MAP_SIZE, NormalMap};
use std::sync::atomic::{AtomicU32, Ordering};

pub(in crate::render) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg8Snorm;

/// Group 0's entries 9 (the maps) and 10 (their sampler).
pub(in crate::render) fn layout_entries() -> [wgpu::BindGroupLayoutEntry; 2] {
    [
        wgpu::BindGroupLayoutEntry {
            binding: 9,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2Array,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 10,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
    ]
}

fn array(device: &wgpu::Device, size: u32, mips: u32, layers: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("surface maps"),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: layers },
        mip_level_count: mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

/// Every layer and mip of the array set to zeros on the GPU: copies from one
/// zeroed buffer (16 layers of the top mip, made zero by `zeroed_buffer`),
/// recorded and submitted at once, so the texture is initialised before any
/// bind group holds it. The bytes are the zeros a `write_texture` of a
/// zero-filled vector gave, without the CPU writing ~170 KB per layer
/// (~45 ms for a city's array on the main thread).
fn zero_layers(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture, layers: u32) {
    // Rows padded to the copies' alignment (the source is zeros throughout).
    let row = |s: u32| (s * 2).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let size = u64::from(row(MAP_SIZE)) * u64::from(MAP_SIZE) * u64::from(16u32.min(layers));
    let zeros = crate::render::zeroed_buffer(
        device,
        &wgpu::BufferDescriptor {
            label: Some("surface maps zeros"),
            size,
            usage: wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        },
    );
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("surface maps zeros") });
    for mip in 0..MAP_MIPS {
        let s = (MAP_SIZE >> mip).max(1);
        let per_layer = u64::from(row(s)) * u64::from(s);
        let per_copy = (size / per_layer).max(1) as u32;
        let mut z = 0;
        while z < layers {
            let n = per_copy.min(layers - z);
            encoder.copy_buffer_to_texture(
                wgpu::TexelCopyBufferInfo {
                    buffer: &zeros,
                    layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row(s)), rows_per_image: Some(s) },
                },
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: mip,
                    origin: wgpu::Origin3d { x: 0, y: 0, z },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d { width: s, height: s, depth_or_array_layers: n },
            );
            z += n;
        }
    }
    queue.submit([encoder.finish()]);
}

fn array_view(t: &wgpu::Texture) -> wgpu::TextureView {
    t.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    })
}

/// The texture pack's replacement of `t` in `domain`, as a map source
/// (None: no pack, or the pack does not replace it).
pub(in crate::render) fn pack_source(
    domain: crate::render::texture_pack::Domain,
    t: &crate::formats::texture::Texture,
) -> Option<crate::surface_maps::PackSource> {
    if !crate::render::texture_pack::enabled(domain) {
        return None;
    }
    let r = crate::render::texture_pack::replacement(domain, t, 4096)?;
    let key = xxhash_rust::xxh3::xxh3_64(&r.bytes) ^ ((r.width as u64) << 32 | r.height as u64);
    let t = t.clone();
    Some(crate::surface_maps::PackSource {
        key,
        decode: Box::new(move || {
            let (w, h) = (r.width, r.height);
            match r.format {
                None => r.bytes.get(..(w * h * 4) as usize).map(|b| (b.to_vec(), w, h)),
                Some(format) => {
                    let e =
                        crate::upscale::Encoded { format, width: w, height: h, mips: r.mips, data: r.bytes.clone() };
                    Some((e.as_texture(&t).decode_rgba(), w, h))
                }
            }
        }),
    })
}

/// The 1 × 1 one-layer stand-in (never sampled: layer 0 means none).
pub(in crate::render) fn placeholder(device: &wgpu::Device) -> wgpu::TextureView {
    array_view(&array(device, 1, 1, 1))
}

/// Tiling, trilinear, 8× anisotropic (the maps' grain at grazing angles).
pub(in crate::render) fn sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("surface maps"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        anisotropy_clamp: 8,
        ..Default::default()
    })
}

/// Per slot: its material (immutable) and its map's layer (filled in as
/// the maps arrive), read by every world draw.
pub(in crate::render) struct SlotSurfaces {
    pub material: Vec<u32>,
    pub layer: Vec<AtomicU32>,
}

impl SlotSurfaces {
    pub fn new(classes: &[Class]) -> SlotSurfaces {
        SlotSurfaces {
            material: classes.iter().map(|c| c.packed()).collect(),
            layer: classes.iter().map(|_| AtomicU32::new(0)).collect(),
        }
    }

    /// The immediates' padding for `slot`: (map layer, material, 0).
    pub fn pad(&self, slot: usize) -> [u32; 3] {
        match (self.layer.get(slot), self.material.get(slot)) {
            (Some(l), Some(&m)) => [l.load(Ordering::Relaxed), m, 0],
            _ => [0; 3],
        }
    }
}

/// The maps of one world renderer, made when Materials + Normals is
/// first on: the array (a layer per candidate slot, + layer 0 unused) and
/// the background builder.
pub(in crate::render) struct SurfaceMaps {
    texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    builder: Builder,
    next: u32,
    layers: u32,
}

impl SurfaceMaps {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        jobs: Vec<(usize, crate::formats::texture::Texture, Class, bool)>,
    ) -> SurfaceMaps {
        let max = device.limits().max_texture_array_layers.max(2);
        let jobs: Vec<_> = jobs.into_iter().take(max as usize - 1).collect();
        let layers = jobs.len() as u32 + 1;
        let texture = array(device, MAP_SIZE, MAP_MIPS, layers);
        // Initialised (zeros) at creation, before any draw binds it (as
        // `super::zeroed_buffer`): `pump` writes its layers from the main
        // thread over many frames while the render threads submit the world
        // views that sample it, the first-write hazard of wgpu-core 30
        // (docs/perf-main-thread.md §1). Unwritten layers read zeros either
        // way (and are never sampled: their slots keep layer 0).
        zero_layers(device, queue, &texture, layers);
        let view = array_view(&texture);
        // Texture packs (docs/surface-maps.md "Texture packs"): a slot the
        // world pack replaces gets its map from the replacement's pixels
        // (looked up on the builder's thread).
        let jobs = jobs
            .into_iter()
            .map(|(slot, t, class, reflect)| {
                (
                    slot,
                    Box::new(move || {
                        crate::surface_maps::map_for_source(
                            &t,
                            class,
                            reflect,
                            pack_source(crate::render::texture_pack::Domain::World, &t),
                        )
                    }) as crate::surface_maps::Job<NormalMap>,
                )
            })
            .collect();
        SurfaceMaps { texture, view, builder: Builder::spawn(jobs), next: 1, layers }
    }

    /// Upload what the builder finished (all of it when `wait`), and give
    /// each map's slot its layer. Returns how many maps arrived.
    pub fn pump(&mut self, queue: &wgpu::Queue, slots: &SlotSurfaces, wait: bool) -> usize {
        if self.builder.done() {
            return 0;
        }
        let results = if wait { self.builder.wait() } else { self.builder.poll() };
        let mut n = 0;
        for (slot, map) in results {
            let Some(map) = map else { continue };
            if self.next >= self.layers {
                break;
            }
            self.upload(queue, self.next, &map);
            if let Some(l) = slots.layer.get(slot) {
                l.store(self.next, Ordering::Relaxed);
            }
            self.next += 1;
            n += 1;
        }
        if self.builder.done() {
            log::info!("surface maps: {} normal maps of {} candidates", self.next - 1, self.layers - 1);
        }
        n
    }

    fn upload(&self, queue: &wgpu::Queue, layer: u32, map: &NormalMap) {
        for (mip, level) in map.levels.iter().enumerate() {
            let s = (MAP_SIZE >> mip).max(1);
            let bytes: Vec<u8> = level.iter().flat_map(|p| [p[0] as u8, p[1] as u8]).collect();
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: mip as u32,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: layer },
                    aspect: wgpu::TextureAspect::All,
                },
                &bytes,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(s * 2), rows_per_image: Some(s) },
                wgpu::Extent3d { width: s, height: s, depth_or_array_layers: 1 },
            );
        }
    }
}

/// Cars (G7c2): the flat 1 × 1 map a car texture without one binds
/// (wgpu zero-initialises it: (0, 0), no tilt; car_surface.wgsl leaves
/// such pixels native).
pub(in crate::render) fn flat_map(device: &wgpu::Device) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("car surface map (flat)"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

/// A car texture's map ([`crate::surface_maps::CarMap`]) uploaded.
pub(in crate::render) fn car_map_view(
    gpu: &crate::render::Gpu,
    map: &crate::surface_maps::CarMap,
) -> wgpu::TextureView {
    use wgpu::util::DeviceExt;
    let bytes: Vec<u8> = map.levels.iter().flat_map(|l| l.iter().flat_map(|p| [p[0] as u8, p[1] as u8])).collect();
    gpu.device
        .create_texture_with_data(
            &gpu.queue,
            &wgpu::TextureDescriptor {
                label: Some("car surface map"),
                size: wgpu::Extent3d { width: map.width, height: map.height, depth_or_array_layers: 1 },
                mip_level_count: map.levels.len() as u32,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &bytes,
        )
        .create_view(&Default::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every texel of a mip of the array, all layers, read back.
    fn read_mip(gpu: &crate::render::Gpu, texture: &wgpu::Texture, mip: u32, layers: u32) -> Vec<u8> {
        let s = (MAP_SIZE >> mip).max(1);
        let row = (s * 2).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("surface maps read-back"),
            size: u64::from(row * s * layers),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut e = gpu.device.create_command_encoder(&Default::default());
        e.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture, mip_level: mip, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(s) },
            },
            wgpu::Extent3d { width: s, height: s, depth_or_array_layers: layers },
        );
        gpu.queue.submit([e.finish()]);
        buffer.slice(..).map_async(wgpu::MapMode::Read, |r| r.unwrap());
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let data = buffer.slice(..).get_mapped_range().expect("mapped").to_vec();
        // The texels only (each row's padding left out).
        data.chunks(row as usize).flat_map(|r| r[..(s * 2) as usize].to_vec()).collect()
    }

    /// The array's zeros made on the GPU: a texture written full of other
    /// bytes first reads zeros in every layer of every mip afterwards (37
    /// layers: more than one copy of the top mip, a part one at the end).
    #[test]
    fn the_array_is_zeroed_in_every_layer_and_mip() {
        let gpu = pollster::block_on(crate::render::Gpu::new(None, crate::render::Gpu::new_instance())).unwrap();
        let layers = 37;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("surface maps test"),
            size: wgpu::Extent3d { width: MAP_SIZE, height: MAP_SIZE, depth_or_array_layers: layers },
            mip_level_count: MAP_MIPS,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        for mip in 0..MAP_MIPS {
            let s = (MAP_SIZE >> mip).max(1);
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: mip, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                &vec![0x5Au8; (s * s * 2 * layers) as usize],
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(s * 2), rows_per_image: Some(s) },
                wgpu::Extent3d { width: s, height: s, depth_or_array_layers: layers },
            );
        }
        gpu.queue.submit([]);
        assert!(read_mip(&gpu, &texture, 0, layers).iter().all(|&b| b == 0x5A), "the test's own fill");
        zero_layers(&gpu.device, &gpu.queue, &texture, layers);
        for mip in 0..MAP_MIPS {
            let texels = read_mip(&gpu, &texture, mip, layers);
            let s = (MAP_SIZE >> mip).max(1);
            assert_eq!(texels.len(), (s * s * 2 * layers) as usize);
            assert!(texels.iter().all(|&b| b == 0), "mip {mip}: a texel is not zero");
        }
    }
}
