use super::*;

/// g_TextureFiltering (0x870D18, custom 20), as the world effects get it
/// at load (0x5D1E30): BASEMINTEXTUREFILTER 0x5D2096 and
/// BASEMAGTEXTUREFILTER 0x5BE570 = 1 (point) at 0, 2 (linear) at 1, 3
/// (anisotropic) at 2 (2 on Matrox, vendor 0x102B, and for the
/// magnification without D3DPTFILTERCAPS_MAGFANISOTROPIC);
/// BASETEXTUREFILTERPARAM (0x5D2133) = the device's MaxAnisotropy
/// (D3DCAPS9 +0x6C, 0x86E634). Read when a renderer builds its samplers.
static TEXTURE_FILTER: AtomicU8 = AtomicU8::new(2);

pub fn set_texture_filter(level: u8) {
    TEXTURE_FILTER.store(level.min(2), Ordering::Relaxed);
}

/// The sampler filters for the current [`set_texture_filter`] level:
/// point, linear, or linear with the anisotropy wgpu allows at most (16,
/// the usual MaxAnisotropy). **Inferred**: the effects' MipFilter, which
/// these parameters do not set, stays linear. The Graphics page's Texture
/// Filtering (2× … 16×, [`crate::qol::anisotropy`]) replaces the level
/// when set. Only the world's, the cars' and the shadows' textures take
/// this sampler (the UI, the reflections, the headlight projector and the
/// screen passes keep their own clamped / point ones).
/// `anisotropy`: the Graphics page's Texture Filtering in force
/// (`RenderOptions::anisotropy`).
pub fn texture_sampler(label: &str, anisotropy: u16) -> wgpu::SamplerDescriptor<'_> {
    let (filter, anisotropy) = match (anisotropy, TEXTURE_FILTER.load(Ordering::Relaxed)) {
        // The Graphics page's Texture Filtering (clone deviation): linear
        // with that anisotropy, whatever the native row says.
        (n @ 2.., _) => (wgpu::FilterMode::Linear, n.min(16)),
        (_, 0) => (wgpu::FilterMode::Nearest, 1),
        (_, 1) => (wgpu::FilterMode::Linear, 1),
        _ => (wgpu::FilterMode::Linear, 16),
    };
    wgpu::SamplerDescriptor {
        label: Some(label),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        mag_filter: filter,
        min_filter: filter,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        anisotropy_clamp: anisotropy,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    /// The TIBURON wheel's cap samples WRAP (car::tests, the wheel line): car.efx's sampler 1.
    #[test]
    fn the_car_sampler_wraps() {
        let s = super::texture_sampler("car", 0);
        assert_eq!(
            (s.address_mode_u, s.address_mode_v),
            (wgpu::AddressMode::Repeat, wgpu::AddressMode::Repeat),
            "car.efx's sampler 1 is WRAP"
        );
    }
}
