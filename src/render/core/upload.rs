//! Buffer and texture uploads, depth targets, read-back (moved from render/mod.rs).

use crate::render::*;

/// A buffer created initialised to zeros (mapped at creation, unmapped at
/// once): the rule for every buffer the main thread writes with
/// `queue.write_buffer` while render threads may submit command buffers
/// that use it (docs/notes/performance/main-thread.md §1). wgpu-core 30
/// zero-fills a resource's uninitialised ranges at submission *after* the
/// pending writes it takes then (a write landing in that window is wiped)
/// and its init-status lock and the device trackers are taken in opposite
/// orders by `write_*` and `submit` (a deadlock); initialised resources
/// take no part in either. The bytes the passes read are the same zeros.
pub(crate) fn zeroed_buffer(device: &wgpu::Device, desc: &wgpu::BufferDescriptor) -> wgpu::Buffer {
    debug_assert!(desc.size % wgpu::COPY_BUFFER_ALIGNMENT == 0, "mapped at creation needs a size in 4-byte units");
    let buffer = device.create_buffer(&wgpu::BufferDescriptor { mapped_at_creation: true, ..desc.clone() });
    buffer.unmap();
    buffer
}

/// Writes `bytes` to `buffer` at `offset`: with a staging belt, as a copy
/// recorded in `encoder` (ahead of the passes recorded after it, the ones
/// reading the buffer), from one of the belt's reused chunks; without one,
/// by `queue.write_buffer` (a staging buffer of its own, copied at the
/// start of the next submission). Either way the passes read the same
/// values.
pub fn upload(
    gpu: &Gpu,
    belt: Option<&mut wgpu::util::StagingBelt>,
    encoder: &mut wgpu::CommandEncoder,
    buffer: &wgpu::Buffer,
    offset: u64,
    bytes: &[u8],
) {
    let Some(size) = wgpu::BufferSize::new(bytes.len() as u64) else { return };
    match belt {
        Some(belt) => belt.write_buffer(encoder, buffer, offset, size).copy_from_slice(bytes),
        None => gpu.queue.write_buffer(buffer, offset, bytes),
    }
}

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub fn create_depth(gpu: &Gpu, width: u32, height: u32) -> wgpu::TextureView {
    depth_texture(gpu, width, height, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
}

/// A depth buffer only ever cleared at a pass's start and discarded at
/// its end (`TRANSIENT_ATTACHMENT`: no memory on tile-based GPUs, a no-op
/// elsewhere).
pub(crate) fn transient_depth(gpu: &Gpu, width: u32, height: u32) -> wgpu::TextureView {
    depth_texture(
        gpu,
        width,
        height,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TRANSIENT_ATTACHMENT,
    )
}

pub(crate) fn depth_texture(gpu: &Gpu, width: u32, height: u32, usage: wgpu::TextureUsages) -> wgpu::TextureView {
    gpu.device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

/// An RG8 texture (the SMAA area table).
pub(crate) fn upload_rg8(gpu: &Gpu, width: u32, height: u32, rg: &[u8]) -> wgpu::TextureView {
    gpu.device
        .create_texture_with_data(
            &gpu.queue,
            &wgpu::TextureDescriptor {
                label: Some("rg8"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rg8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            rg,
        )
        .create_view(&Default::default())
}

pub(crate) fn upload_rgba(gpu: &Gpu, width: u32, height: u32, rgba: &[u8]) -> wgpu::TextureView {
    gpu.device
        .create_texture_with_data(
            &gpu.queue,
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            rgba,
        )
        .create_view(&Default::default())
}

/// Upload a game texture: DXT data as-is (with its mip chain) when the GPU
/// supports BC formats, otherwise the decoded top level.
pub(crate) fn upload_texture(gpu: &Gpu, t: &Texture, bc: bool) -> wgpu::TextureView {
    upload_texture_for(gpu, t, bc, texture_pack::Domain::Ui)
}

pub(in crate::render) fn upload_texture_for(
    gpu: &Gpu,
    t: &Texture,
    bc: bool,
    domain: texture_pack::Domain,
) -> wgpu::TextureView {
    let ((format, width, height, mips), bytes) =
        texture_layer_with_pack(domain, Some(t), bc, gpu.device.limits().max_texture_dimension_2d);
    gpu.device
        .create_texture_with_data(
            &gpu.queue,
            &wgpu::TextureDescriptor {
                label: Some(&t.name),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: mips,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &bytes,
        )
        .create_view(&Default::default())
}

/// Copy a texture (made with COPY_SRC) back to the CPU as stored: `width`
/// × `height` texels of the texture's own format, rows packed (4 bytes a
/// texel for the 8-bit formats). The rows of the copy are as wide as that
/// format's texels (a 16-bit float target has 8 bytes a texel); a format
/// without a single texel size (depth-stencil, compressed) is a caller's
/// bug and panics. A target in the window's format goes through
/// [`read_back_rgba8`] instead.
pub fn read_back(gpu: &Gpu, target: &wgpu::Texture, width: u32, height: u32) -> Vec<u8> {
    let texel = target.format().block_copy_size(None).expect("read_back: a format with one texel size");
    read_back_raw(gpu, target, width, height, texel)
}

/// [`read_back`] with the texel size given.
fn read_back_raw(gpu: &Gpu, target: &wgpu::Texture, width: u32, height: u32, texel: u32) -> Vec<u8> {
    let row = width * texel;
    let bytes_per_row = row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(bytes_per_row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
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
    let _ = gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    let mapped = slice.get_mapped_range().expect("mapped readback");
    let mut pixels = Vec::with_capacity(row as usize * height as usize);
    for line in mapped.chunks(bytes_per_row as usize).take(height as usize) {
        pixels.extend_from_slice(&line[..row as usize]);
    }
    pixels
}

/// A whole texture in the window's format (the swap chain's SDR format,
/// `display::sdr_surface_format`) read back as RGBA8, alpha kept: 8-bit
/// BGRA / RGBA as stored (BGRA swapped), 10-bit and 16-bit unorm and
/// 16-bit float scaled to 8 bits with rounding, float clamped to 0..1.
/// The values on an SDR surface are display-encoded (the game writes
/// gamma-space values whatever the format's depth, and the surface shows
/// them as sRGB-encoded), so the conversion is a plain requantisation: an
/// 8-bit value v written into any of them reads back as v. An error, not
/// a panic, for a format it cannot convert or a target without COPY_SRC
/// (nothing is submitted then).
pub fn read_back_rgba8(gpu: &Gpu, target: &wgpu::Texture) -> anyhow::Result<Vec<u8>> {
    let format = target.format();
    let texel = rgba8_texel_size(format).ok_or_else(|| anyhow::anyhow!("cannot read a {format:?} picture back"))?;
    anyhow::ensure!(
        target.usage().contains(wgpu::TextureUsages::COPY_SRC),
        "the {format:?} target cannot be copied (no COPY_SRC)"
    );
    anyhow::ensure!(
        target.sample_count() == 1 && target.dimension() == wgpu::TextureDimension::D2,
        "the target is not a single-sampled 2D texture"
    );
    let raw = read_back_raw(gpu, target, target.width(), target.height(), texel);
    to_rgba8(format, &raw)
}

/// Bytes a texel of `format` takes when [`to_rgba8`] converts it. None:
/// not convertible.
pub fn rgba8_texel_size(format: wgpu::TextureFormat) -> Option<u32> {
    use wgpu::TextureFormat as F;
    match format {
        F::Rgba8Unorm | F::Rgba8UnormSrgb | F::Bgra8Unorm | F::Bgra8UnormSrgb | F::Rgb10a2Unorm => Some(4),
        F::Rgba16Unorm | F::Rgba16Float => Some(8),
        _ => None,
    }
}

/// Texels of `format` (packed, as [`read_back`] returns them) as RGBA8;
/// see [`read_back_rgba8`] for the conversions.
pub fn to_rgba8(format: wgpu::TextureFormat, raw: &[u8]) -> anyhow::Result<Vec<u8>> {
    use wgpu::TextureFormat as F;
    let texel = rgba8_texel_size(format).ok_or_else(|| anyhow::anyhow!("cannot read a {format:?} picture back"))? as usize;
    anyhow::ensure!(raw.len() % texel == 0, "{} bytes are no whole number of {format:?} texels", raw.len());
    let unorm = |v: u32, max: u32| ((v * 255 + max / 2) / max) as u8;
    let mut out = Vec::with_capacity(raw.len() / texel * 4);
    for t in raw.chunks_exact(texel) {
        let px = match format {
            F::Rgba8Unorm | F::Rgba8UnormSrgb => [t[0], t[1], t[2], t[3]],
            F::Bgra8Unorm | F::Bgra8UnormSrgb => [t[2], t[1], t[0], t[3]],
            F::Rgb10a2Unorm => {
                let v = u32::from_le_bytes([t[0], t[1], t[2], t[3]]);
                [unorm(v & 0x3FF, 1023), unorm((v >> 10) & 0x3FF, 1023), unorm((v >> 20) & 0x3FF, 1023), unorm(v >> 30, 3)]
            }
            F::Rgba16Unorm => {
                std::array::from_fn(|i| unorm(u32::from(u16::from_le_bytes([t[2 * i], t[2 * i + 1]])), 65535))
            }
            F::Rgba16Float => std::array::from_fn(|i| {
                let v = f16_to_f32(u16::from_le_bytes([t[2 * i], t[2 * i + 1]]));
                // NaN clamps to 0 (`max` drops it).
                (v.max(0.0).min(1.0) * 255.0).round() as u8
            }),
            _ => unreachable!("rgba8_texel_size lists the formats"),
        };
        out.extend_from_slice(&px);
    }
    Ok(out)
}

/// An IEEE half-precision float's value.
fn f16_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = i32::from((h >> 10) & 0x1F);
    let mant = f32::from(h & 0x3FF);
    match exp {
        0 => sign * mant * (2.0f32).powi(-24),
        31 if mant == 0.0 => sign * f32::INFINITY,
        31 => f32::NAN,
        _ => sign * (1.0 + mant / 1024.0) * (2.0f32).powi(exp - 15),
    }
}

#[cfg(test)]
mod rgba8_tests {
    use super::*;

    /// f32 → half, round to nearest even (normal range only: the test's
    /// values are 0..=1).
    fn f32_to_f16(v: f32) -> u16 {
        if v == 0.0 {
            return 0;
        }
        let bits = v.to_bits();
        let exp = ((bits >> 23) & 0xFF) as i32 - 127 + 15;
        let mant = bits & 0x7F_FFFF;
        let (mut m, rest) = (mant >> 13, mant & 0x1FFF);
        if rest > 0x1000 || (rest == 0x1000 && m & 1 == 1) {
            m += 1;
        }
        ((exp as u32) << 10) as u16 + m as u16
    }

    #[test]
    fn every_8_bit_value_reads_back_unchanged_from_each_window_format() {
        use wgpu::TextureFormat as F;
        for v in 0..=255u8 {
            let a = 255 - v;
            let x = f32::from(v) / 255.0;
            let q = |max: u32| (x * max as f32).round() as u32;
            let cases: [(F, Vec<u8>); 5] = [
                (F::Rgba8Unorm, vec![v, v / 2, 255 - v, a]),
                (F::Bgra8Unorm, vec![255 - v, v / 2, v, a]),
                (F::Rgb10a2Unorm, (q(1023) | q(1023) << 10 | q(1023) << 20 | 3 << 30).to_le_bytes().to_vec()),
                (F::Rgba16Unorm, [q(65535) as u16; 4].iter().flat_map(|c| c.to_le_bytes()).collect()),
                (F::Rgba16Float, [f32_to_f16(x); 4].iter().flat_map(|c| c.to_le_bytes()).collect()),
            ];
            for (format, raw) in cases {
                let px = to_rgba8(format, &raw).unwrap();
                match format {
                    F::Rgba8Unorm | F::Bgra8Unorm => assert_eq!(px, [v, v / 2, 255 - v, a], "{format:?}"),
                    F::Rgb10a2Unorm => assert_eq!(px, [v, v, v, 255], "{format:?}"),
                    _ => assert_eq!(px, [v, v, v, v], "{format:?}"),
                }
            }
        }
        // Float outside 0..1 clamps; the 2-bit alpha's steps.
        let raw: Vec<u8> = [f32_to_f16(2.0), 0xBC00, 0x7E00, 0x3C00].iter().flat_map(|c| c.to_le_bytes()).collect();
        assert_eq!(to_rgba8(F::Rgba16Float, &raw).unwrap(), [255, 0, 0, 255]);
        assert_eq!(to_rgba8(F::Rgb10a2Unorm, &(1u32 << 30).to_le_bytes()).unwrap(), [0, 0, 0, 85]);
    }

    #[test]
    fn an_unconvertible_format_is_an_error() {
        for format in [wgpu::TextureFormat::Rgba32Float, wgpu::TextureFormat::R8Unorm, wgpu::TextureFormat::Depth32Float] {
            assert!(rgba8_texel_size(format).is_none());
            let e = to_rgba8(format, &[0; 16]).unwrap_err();
            assert!(e.to_string().contains("cannot read"), "{e}");
        }
        assert!(to_rgba8(wgpu::TextureFormat::Rgba16Float, &[0; 12]).is_err(), "a partial texel");
    }

    /// The GPU side: a target of each window format holding the same 8-bit
    /// picture reads back as that picture; an unconvertible target is an
    /// error with nothing submitted, not a validation panic.
    #[test]
    fn window_format_targets_read_back_as_rgba8() {
        let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance())).unwrap();
        let (w, h) = (37u32, 5u32);
        let picture: Vec<u8> = (0..w * h * 4).map(|i| (i * 7 % 256) as u8).collect();
        let make = |format| {
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("read-back test"),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        };
        let rgba8 = make(wgpu::TextureFormat::Rgba8Unorm);
        gpu.queue.write_texture(
            rgba8.as_image_copy(),
            &picture,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: None },
            rgba8.size(),
        );
        assert_eq!(read_back_rgba8(&gpu, &rgba8).unwrap(), picture);
        let wide = make(wgpu::TextureFormat::Rgba16Float);
        let halves: Vec<u8> =
            picture.iter().flat_map(|&v| f32_to_f16(f32::from(v) / 255.0).to_le_bytes()).collect();
        gpu.queue.write_texture(
            wide.as_image_copy(),
            &halves,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 8), rows_per_image: None },
            wide.size(),
        );
        assert_eq!(read_back_rgba8(&gpu, &wide).unwrap(), picture);
        assert_eq!(read_back(&gpu, &wide, w, h).len(), (w * h * 8) as usize, "raw rows as wide as the format");
        let odd = make(wgpu::TextureFormat::Rgba32Float);
        assert!(read_back_rgba8(&gpu, &odd).is_err());
    }
}

#[cfg(test)]
mod zeroed_tests {
    use super::*;

    /// A buffer from [`zeroed_buffer`] reads zeros before anything writes
    /// it: what the passes that make their buffers this way (and write no
    /// zeros of their own after it, `VolumeFx::new`) rely on.
    #[test]
    fn a_zeroed_buffer_reads_zeros() {
        let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance())).unwrap();
        let size = 3u64 << 20;
        let b = zeroed_buffer(
            &gpu.device,
            &wgpu::BufferDescriptor {
                label: Some("zeroed test"),
                size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            },
        );
        let out = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("zeroed test read-back"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut e = gpu.device.create_command_encoder(&Default::default());
        e.copy_buffer_to_buffer(&b, 0, &out, 0, size);
        gpu.queue.submit([e.finish()]);
        out.slice(..).map_async(wgpu::MapMode::Read, |r| r.unwrap());
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert!(out.slice(..).get_mapped_range().expect("mapped").iter().all(|&v| v == 0));
    }
}
