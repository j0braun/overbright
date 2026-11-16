//! The cars' environment cubes: static and black (moved from car.rs).

use super::*;

/// The front end's static environment cube (0x87088C) from six face
/// textures in D3D face order +X, −X, +Y, −Y, +Z, −Z (0x5B9920 copies the
/// textures XPOS … ZNEG into faces 0–5); `None` unless all six are square
/// and the same size.
pub fn static_envmap(gpu: &Gpu, faces: &[crate::formats::texture::Texture]) -> Option<wgpu::TextureView> {
    let (size, data) = envmap_texels(faces)?;
    Some(cube_from_texels(gpu, size, &data))
}

/// The six faces of [`static_envmap`] decoded to RGBA, face after face
/// (`None` unless all six are square and the same size).
pub fn envmap_texels(faces: &[crate::formats::texture::Texture]) -> Option<(u32, Vec<u8>)> {
    let size = faces.first()?.width;
    if faces.len() != 6 || faces.iter().any(|f| f.width != size || f.height != size) {
        return None;
    }
    Some((size, faces.iter().flat_map(|f| f.decode_rgba()).collect()))
}

/// A platform without the six faces (**verified** in the
/// code: 0x5B9920's loop 0x5B99F0..0x5B9A5B): each missing face is the
/// default texture DEFAULTALPHA (0x4901D0 → 0x82DB08, 32 × 32 DXT3, all
/// zero), and the copy moves the source's byte size (+0x40: 1024 bytes)
/// into the face's level 0 — its first 1024 bytes become 0, the rest keeps
/// the cube's last contents (the previous platform's faces).
pub fn envmap_without_faces(size: u32, mut data: Vec<u8>) -> (u32, Vec<u8>) {
    let face = (size * size * 4) as usize;
    for f in 0..6 {
        let start = f * face;
        let end = (start + face.min(1024)).min(data.len());
        if start < end {
            data[start..end].fill(0);
        }
    }
    (size, data)
}

/// A cube from six faces' RGBA texels (`size` square, face after face).
pub fn cube_from_texels(gpu: &Gpu, size: u32, data: &[u8]) -> wgpu::TextureView {
    let texture = gpu.device.create_texture_with_data(
        &gpu.queue,
        &wgpu::TextureDescriptor {
            label: Some("frontend envmap"),
            size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 6 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        data,
    );
    texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    })
}

/// A black cube: what the car effects sample when g_CarEnvironmentMapEnable
/// (0x870CB8) is 0 and no cube exists (0x5BCAE0 binds a null texture), and
/// what a car renderer starts with ([`start_envmap`]).
pub fn black_envmap(gpu: &Gpu) -> wgpu::TextureView {
    gpu.device
        .create_texture_with_data(
            &gpu.queue,
            &wgpu::TextureDescriptor {
                label: Some("envmap off"),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 6 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &black_cube_texels(),
        )
        .create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        })
}

/// [`black_envmap`]'s six 1 × 1 faces.
fn black_cube_texels() -> Vec<u8> {
    [0, 0, 0, 255].repeat(6)
}

/// The cube a car renderer samples until one is set (
/// **verified** in the code, the cube's first contents **likely**):
/// 0x5BCAE0 binds the front end's cube 0x87088C (or the world's 0x870858)
/// without a null test. 0x87088C is created whenever 0x870CB8 ≠ 0
/// (0x5BA4F4: CreateCubeTexture 256, A8R8G8B8, D3DPOOL_MANAGED), apart from
/// any platform; 0x5B9920 copies the platform's XPOS … ZNEG into it, and a
/// missing face is DEFAULTALPHA (0x4901D0 → 0x82DB08, 1024 bytes of zero),
/// which overwrites only the face's first row. So the cube is never a
/// made-up sky: before any platform supplied faces it holds its creation
/// contents (zero under Windows / Wine: black), afterwards the last
/// platform's faces. The renderer starts black; keeping the previous
/// platform's cube is the caller's (`set_environment`).
pub(super) fn start_envmap(gpu: &Gpu) -> wgpu::TextureView {
    black_envmap(gpu)
}

#[cfg(test)]
mod tests {
    /// The start cube is black (0x87088C's creation
    /// contents), not a gradient.
    #[test]
    fn the_start_cube_is_black() {
        let texels = super::black_cube_texels();
        assert_eq!(texels.len(), 6 * 4);
        assert!(texels.chunks_exact(4).all(|t| t[..3] == [0, 0, 0]));
    }

    /// 0x5B9920: a platform without its faces zeroes the
    /// first 1024 bytes of each face of the cube it keeps.
    #[test]
    fn missing_faces_zero_only_each_faces_first_kilobyte() {
        let (size, data) = super::envmap_without_faces(32, vec![7u8; 32 * 32 * 4 * 6]);
        assert_eq!(size, 32);
        for face in data.chunks_exact(32 * 32 * 4) {
            assert!(face[..1024].iter().all(|&b| b == 0));
            assert!(face[1024..].iter().all(|&b| b == 7));
        }
        // A 256 cube (0x5BA4F4's edge): exactly its first row.
        let (_, data) = super::envmap_without_faces(256, vec![9u8; 256 * 256 * 4 * 6]);
        assert!(data[..1024].iter().all(|&b| b == 0) && data[1024] == 9);
    }
}
