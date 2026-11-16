//! The world's texture arrays and texture layers (moved from render/mod.rs).

use crate::render::*;

/// The world's textures packed into 2D array textures, one per shape
/// (format, size, mip count), each draw's layer and scroll in immediates:
/// a draw rebinds group 1 only when its texture is in another array, where
/// a bind group per texture made wgpu spend most of the world's recording
/// time on rebinding. A layer holds the texture's own bytes and is sampled
/// with the same sampler and coordinates, so the image does not change.
/// Used when the device has immediates (and `NFSU2_NO_TEXTURE_ARRAYS` is
/// not set); else each texture has its own bind group, as before.
pub(crate) struct TextureArrays {
    pub(crate) layout: wgpu::BindGroupLayout,
    /// Per array: the texture.
    pub(crate) arrays: Vec<wgpu::Texture>,
    /// The groups 1: one per array, or with bound arrays
    /// ([`bound_arrays`]) one per [`BOUND_ARRAYS`] arrays.
    pub(crate) groups: Vec<wgpu::BindGroup>,
    /// Per slot: its array and layer.
    pub(crate) slots: Vec<(u32, u32)>,
    /// Bound arrays ([`bound_arrays`]).
    pub(crate) bound: bool,
}

/// The immediates of a world draw ([`TextureArrays`]): `Scroll` then the
/// layer, padded as WGSL lays the struct out.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct TextureImmediates {
    pub(crate) scroll: [f32; 8],
    pub(crate) layer: u32,
    pub(crate) _pad: [u32; 3],
}

/// Arrays per group 1 with bound arrays ([`bound_arrays`]).
pub(crate) const BOUND_ARRAYS: u32 = 256;

/// Bound arrays (a faster way to the same image): with
/// `TEXTURE_BINDING_ARRAY` the world's texture arrays sit side by side in
/// one group 1 (a binding array, [`BOUND_ARRAYS`] at most), the draw's
/// array picked by an index in the immediates, so a draw changing array
/// changes only the immediates it already set, not the bind group (whose
/// switch, and the rebinding of the groups after it, was most of wgpu's
/// render-pass encoding: about 80 % of the world draws changed array).
/// `NFSU2_NO_BOUND_ARRAYS=1`: one group per array as before.
pub(crate) fn bound_arrays(device: &wgpu::Device) -> bool {
    let l = device.limits();
    device.features().contains(wgpu::Features::TEXTURE_BINDING_ARRAY)
        && l.max_binding_array_elements_per_shader_stage >= BOUND_ARRAYS
        && l.max_sampled_textures_per_shader_stage >= BOUND_ARRAYS + 32
        && !crate::render::debug_env::nfsu2_no_bound_arrays()
}

/// A texture's scroll parameters as the shader reads them (`Scroll`).
pub(crate) fn scroll_params(scroll: &Scroll) -> [f32; 8] {
    [scroll.kind as f32, scroll.speed[0], scroll.speed[1], scroll.step, scroll.fixed[0], scroll.fixed[1], 0.0, 0.0]
}

/// How a texture goes to the GPU, as [`upload_texture`] does it: its
/// format, size, mip count and bytes (all levels back to back), or the
/// white stand-in (None).
pub(crate) fn texture_layer(
    t: Option<&Texture>,
    bc: bool,
) -> ((wgpu::TextureFormat, u32, u32, u32), std::borrow::Cow<'_, [u8]>) {
    let Some(t) = t else {
        return ((wgpu::TextureFormat::Rgba8Unorm, 1, 1, 1), std::borrow::Cow::Borrowed(&[255u8, 255, 255, 255][..]));
    };
    let block_format = match t.format {
        PixelFormat::Dxt1 => Some(wgpu::TextureFormat::Bc1RgbaUnorm),
        PixelFormat::Dxt3 => Some(wgpu::TextureFormat::Bc2RgbaUnorm),
        PixelFormat::Dxt5 => Some(wgpu::TextureFormat::Bc3RgbaUnorm),
        _ => None,
    };
    match block_format {
        Some(format) if bc && t.width % 4 == 0 && t.height % 4 == 0 => {
            ((format, t.width, t.height, t.mip_count), std::borrow::Cow::Borrowed(&t.data[..]))
        }
        // An RGBA8 texture made with its mip chain (the car pack's
        // composited rims, `car::composite_wheel`); the game's own textures
        // and native composites have one level.
        _ if t.format == PixelFormat::Rgba8 && t.mip_count > 1 => {
            ((wgpu::TextureFormat::Rgba8Unorm, t.width, t.height, t.mip_count), std::borrow::Cow::Borrowed(&t.data[..]))
        }
        _ => ((wgpu::TextureFormat::Rgba8Unorm, t.width, t.height, 1), std::borrow::Cow::Owned(t.decode_rgba())),
    }
}

/// The car pack's replacement for `t` (G7b2), its top level as RGBA8.
pub(crate) fn car_pack_rgba(t: &Texture) -> Option<(u32, u32, Vec<u8>)> {
    let r = texture_pack::replacement(texture_pack::Domain::Car, t, 4096)?;
    match r.format {
        None => Some((r.width, r.height, r.bytes[..(r.width * r.height * 4) as usize].to_vec())),
        Some(format) => {
            let e = crate::upscale::Encoded {
                format,
                width: r.width,
                height: r.height,
                mips: r.mips,
                data: r.bytes.clone(),
            };
            Some((r.width, r.height, e.as_texture(t).decode_rgba()))
        }
    }
}

/// Apply only pixel data; all original material/UV metadata remains in `Texture`.
pub(in crate::render) fn texture_layer_with_pack(
    domain: texture_pack::Domain,
    t: Option<&Texture>,
    bc: bool,
    max_dimension: u32,
) -> ((wgpu::TextureFormat, u32, u32, u32), std::borrow::Cow<'_, [u8]>) {
    if let Some(r) = t.and_then(|t| texture_pack::replacement(domain, t, max_dimension)) {
        return match r.format {
            None => {
                ((wgpu::TextureFormat::Rgba8Unorm, r.width, r.height, r.mips), std::borrow::Cow::Owned(r.bytes.clone()))
            }
            // A generated DDS: as-is with BC, else decoded with a full RGBA8 chain.
            Some(PixelFormat::Dxt1) if bc => (
                (wgpu::TextureFormat::Bc1RgbaUnorm, r.width, r.height, r.mips),
                std::borrow::Cow::Owned(r.bytes.clone()),
            ),
            Some(PixelFormat::Dxt5) if bc => (
                (wgpu::TextureFormat::Bc3RgbaUnorm, r.width, r.height, r.mips),
                std::borrow::Cow::Owned(r.bytes.clone()),
            ),
            // TexMod imports keep their DXT3 art (docs/texmod-import.md).
            Some(PixelFormat::Dxt3) if bc => (
                (wgpu::TextureFormat::Bc2RgbaUnorm, r.width, r.height, r.mips),
                std::borrow::Cow::Owned(r.bytes.clone()),
            ),
            Some(format) => {
                let e = crate::upscale::Encoded {
                    format,
                    width: r.width,
                    height: r.height,
                    mips: r.mips,
                    data: r.bytes.clone(),
                };
                let top = e.as_texture(t.unwrap()).decode_rgba();
                let levels = crate::upscale::mip_chain(&top, r.width, r.height);
                (
                    (wgpu::TextureFormat::Rgba8Unorm, r.width, r.height, levels.len() as u32),
                    std::borrow::Cow::Owned(levels.into_iter().flat_map(|l| l.2).collect()),
                )
            }
        };
    }
    texture_layer(t, bc)
}

impl TextureArrays {
    /// The arrays [`prepared::arrays`] laid out (or the cache kept), uploaded.
    pub(crate) fn new(gpu: &Gpu, sampler: &wgpu::Sampler, prepared: &prepared::Arrays) -> TextureArrays {
        let bound = bound_arrays(&gpu.device);
        let layout = array_layout(&gpu.device, bound);
        // The arrays side by side (each its own texture, staging and
        // submission), in their order.
        use rayon::prelude::*;
        let made: Vec<(wgpu::Texture, wgpu::TextureView)> =
            prepared.arrays.par_iter().map(|a| make_array(gpu, a)).collect();
        let groups = if bound {
            made.chunks(BOUND_ARRAYS as usize)
                .map(|c| {
                    array_group(&gpu.device, &layout, sampler, &c.iter().map(|(_, v)| v).collect::<Vec<_>>(), true)
                })
                .collect()
        } else {
            made.iter().map(|(_, v)| array_group(&gpu.device, &layout, sampler, &[v], false)).collect()
        };
        let made: Vec<wgpu::Texture> = made.into_iter().map(|(t, _)| t).collect();
        TextureArrays { layout, arrays: made, groups, slots: prepared.slots.clone(), bound }
    }

    /// One slot's texture on its own (a 2D view of its layer).
    pub(crate) fn view(&self, slot: usize) -> wgpu::TextureView {
        let (array, layer) = self.slots[slot];
        self.arrays[array as usize].create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_array_layer: layer,
            array_layer_count: Some(1),
            ..Default::default()
        })
    }

    /// Slot `slot`'s group 1 and its array's index in it (the immediates'
    /// last word; 0 without bound arrays).
    pub(crate) fn group_of(&self, slot: usize) -> (usize, u32, u32) {
        slot_group(self.slots[slot], self.bound)
    }

    /// A group 1 of one array of its own (the shadows', the characters'
    /// textures): the array at index 0.
    pub(crate) fn single_group(&self, gpu: &Gpu, sampler: &wgpu::Sampler, array: &prepared::Array) -> wgpu::BindGroup {
        let (_, view) = make_array(gpu, array);
        array_group(&gpu.device, &self.layout, sampler, &[&view], self.bound)
    }
}

/// [`TextureArrays::group_of`] for a slot's (array, layer).
pub(crate) fn slot_group((array, layer): (u32, u32), bound: bool) -> (usize, u32, u32) {
    if bound { ((array / BOUND_ARRAYS) as usize, array % BOUND_ARRAYS, layer) } else { (array as usize, 0, layer) }
}

/// A group 1 of `views` (one, or with `bound` up to [`BOUND_ARRAYS`],
/// the rest left unbound with `PARTIALLY_BOUND_BINDING_ARRAY`, else filled
/// with the first: the shader never indexes past the views given).
pub(crate) fn array_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    views: &[&wgpu::TextureView],
    bound: bool,
) -> wgpu::BindGroup {
    let mut views = views.to_vec();
    if bound && !device.features().contains(wgpu::Features::PARTIALLY_BOUND_BINDING_ARRAY) {
        views.resize(BOUND_ARRAYS as usize, views[0]);
    }
    let texture = if bound {
        wgpu::BindingResource::TextureViewArray(&views)
    } else {
        wgpu::BindingResource::TextureView(views[0])
    };
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("world texture array"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: texture },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}

/// A texture array as [`prepared::arrays`] laid it out (each level one
/// copy for all layers, rows already at wgpu's alignment), and its view.
pub(crate) fn make_array(gpu: &Gpu, array: &prepared::Array) -> (wgpu::Texture, wgpu::TextureView) {
    let (format, width, height, mips) = array.key;
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("world texture array"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: array.layers },
        mip_level_count: mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    // Each level through a staging buffer of its own and one submission
    // for the array, not `queue.write_texture`: that copies the bytes into
    // its staging memory holding the queue's pending-writes lock, which
    // every other thread's submission and write waits for (a load on
    // another thread, `Preload` in the game, made the frames that ran
    // meanwhile 10–19 ms instead of 3–4). The same bytes reach the texture.
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("world texture array upload") });
    for (mip, level) in array.levels.iter().enumerate() {
        let target = wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: mip as u32,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        };
        let layout = wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(level.bytes_per_row),
            rows_per_image: Some(level.rows),
        };
        // Whole blocks: wgpu takes the physical size of a level.
        let extent =
            wgpu::Extent3d { width: level.extent.0, height: level.extent.1, depth_or_array_layers: array.layers };
        let len = level.data.len() as u64;
        if len == 0 || len % wgpu::COPY_BUFFER_ALIGNMENT != 0 {
            gpu.queue.write_texture(target, &level.data, layout, extent);
            continue;
        }
        let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("world texture array staging"),
            size: len,
            usage: wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: true,
        });
        {
            // The copy in 1 MB pieces on the pool (the source is mostly the
            // cache's mapping, whose first touch faults its pages in).
            use rayon::prelude::*;
            let mut mapped = staging.slice(..).get_mapped_range_mut().expect("mapped at creation");
            const PIECE: usize = 1 << 20;
            let mut view = mapped.slice(..);
            assert_eq!(view.len(), level.data.len());
            struct Dst(*mut u8);
            // SAFETY: each piece below writes its own bytes of the mapping.
            unsafe impl Send for Dst {}
            unsafe impl Sync for Dst {}
            let dst = Dst(view.as_raw_ptr().as_ptr() as *mut u8);
            let src = &level.data[..];
            (0..src.len().div_ceil(PIECE)).into_par_iter().for_each(|k| {
                let at = k * PIECE;
                let n = PIECE.min(src.len() - at);
                let dst = &dst;
                // SAFETY: `at..at + n` lies in the mapped range (as long as
                // the data) and no two pieces overlap.
                unsafe { std::ptr::copy_nonoverlapping(src.as_ptr().add(at), dst.0.add(at), n) };
            });
        }
        staging.unmap();
        encoder.copy_buffer_to_texture(wgpu::TexelCopyBufferInfo { buffer: &staging, layout }, target, extent);
    }
    gpu.queue.submit([encoder.finish()]);
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    (texture, view)
}

#[cfg(test)]
mod bound_array_tests {
    use wgpu::naga;

    fn parse(source: &str) -> naga::Module {
        let module = naga::front::wgsl::parse_str(source).expect("parse");
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all()).validate(&module).expect("valid");
        module
    }

    /// The immediates' struct and its size in a module.
    fn immediates(module: &naga::Module) -> (Vec<String>, u32) {
        let (_, var) = module.global_variables.iter().find(|(_, v)| v.space == naga::AddressSpace::Immediate).expect("immediates");
        let naga::TypeInner::Struct { members, span } = &module.types[var.ty].inner else { panic!("a struct") };
        (members.iter().map(|m| m.name.clone().unwrap()).collect(), *span)
    }

    /// Bound arrays change only how the draw's array is found — every
    /// world module form (native / lamps, multiview, mip bias) parses and
    /// validates, the immediates keep their 48 bytes (`TextureImmediates`)
    /// with the old members first and the array's index in the last word,
    /// and the sampling is the same call on the indexed array.
    #[test]
    fn bound_array_modules_match_the_array_ones() {
        for lamps in [false, true] {
            for multiview in [false, true] {
                for biased in [false, true] {
                    let plain = crate::render::world_module_source(lamps, true, multiview, biased, false);
                    let bound = crate::render::world_module_source(lamps, true, multiview, biased, true);
                    let (pm, ps) = immediates(&parse(&plain));
                    let (bm, bs) = immediates(&parse(&bound));
                    assert_eq!((ps, bs), (std::mem::size_of::<crate::render::TextureImmediates>() as u32, 48));
                    assert_eq!(&bm[..pm.len()], &pm[..]);
                    assert_eq!(bm.len(), 5);
                    assert_eq!(bm.last().map(String::as_str), Some("array"));
                    let sample = if biased { "textureSampleBias(tex[imm.array], samp, uv, imm.layer, MIP_BIAS)" } else { "textureSample(tex[imm.array], samp, uv, imm.layer)" };
                    assert!(bound.contains(sample), "{sample}");
                    assert_eq!(plain.replace("tex[imm.array]", "tex"), plain);
                    // Everything outside the texture block is the same text.
                    let strip = |s: &str| s.lines().filter(|l| !l.contains("var tex") && !l.contains("struct TextureImmediates") && !l.contains("textureSample") && !l.starts_with("enable ")).collect::<Vec<_>>().join("\n");
                    assert_eq!(strip(&plain), strip(&bound));
                }
            }
        }
        // A slot's group 1, index and layer: as before without bound
        // arrays, chunks of BOUND_ARRAYS with.
        let n = crate::render::BOUND_ARRAYS;
        assert_eq!(crate::render::slot_group((67, 3), false), (67, 0, 3));
        assert_eq!(crate::render::slot_group((67, 3), true), (0, 67, 3));
        assert_eq!(crate::render::slot_group((n + 2, 1), true), (1, 2, 1));
    }
}
