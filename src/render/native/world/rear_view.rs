//! The rear-view mirror (view 3) of the bumper and hood cameras.
//!
//! **Traced** (SPEED2.EXE; see docs/formats.md, *Rear-view mirror*):
//!
//! * When: 0x6000A0 (HUD_BuildFeatures) adds HUD_FEATURE_RVM (low 0x1000,
//!   name table 0x7F6728 index 12) in its race / free-roam branch (racer
//!   +0x28 = 0) when the Rearview Mirror option (0x83AA18, settings
//!   +0x14) is on, the look-back key is not held (player +0x249) and the
//!   player's camera setting (0x83AA74 + player × 0x28) is 0 (bumper) or 1
//!   (hood). 0x4CA090 then makes (0x5F1E20 with 1) or drops the mirror's
//!   camera mover (0x44BA10, vtable 0x787D98) on the player's second view
//!   (player +0x1C, view 3; mover at player +0x244). 0x5C21F0 turns view
//!   3 on (layout 0x8707DC = 2) while it has a mover and view 1's mover
//!   is not a chase kind (vtable +0x24).
//! * The camera (0x44BA80, every frame): the car's matrix (car +0xD0,
//!   transposed, 0x5BAF90) and position (+0x50), turned by RotateX(0x4000)
//!   and RotateY(0x4000) so the view looks out of the car's back with its
//!   up the car's up, then the view's translation + (0, 0.5, 0)
//!   (0x7EC4C0): the eye 0.5 m above the car's origin along its up axis.
//!   Field of view 20000 (binary angle, 109.9°; camera +0xC4, unless
//!   0x828640), near 0.75 (0x7870D4), far 192 (0x7870D8); the view leaves
//!   the car out (camera +0x280 = the car, 0x44BA39, which World_DrawCars
//!   0x6316E0 skips).
//! * The projection (0x5C7ED0, view 3): no 1.1 factor, the target's h / w
//!   (256 / 256) for the vertical angle: square, 109.9° both ways.
//! * The image goes on screen in the frame's tail 0x5CBF50 (after the post
//!   passes, before the letterbox 0x5CBDD0 and the front end 0x5378D0):
//!   one FE quad (0x5C5030) with 0x8708A4, clamped (ADDRESSU / V 3), alpha
//!   blended (0x5BCBD0(1, 5, 6)), colour 0xFFFFFFFF, corners (440, 15),
//!   (200, 15), (200, 95), (440, 95) on the 640 × 480 canvas with the
//!   quad's default UVs (0x5BB1D0: (0, 0), (1, 0), (1, 1), (0, 1)) — so u
//!   runs right to left and the image is flipped across: a mirror. Its
//!   shape and tint come from view 3's own passes (`RVM_MASK` into the
//!   alpha, `RVM` over the colour; [`crate::render::rear_view`]).

//!
//! The rear-view mirror's frame passes (view 3, 0x5CAF4D..0x5CAFF9): after
//! the world and the cars, two full-target quads (0x5C27C0) with effect
//! 0x86EB48 (fe.efx), colour 0xFFFFFFFF, clamped (stages 0–3 ADDRESSU / V
//! 3), no depth test or write, alpha test off:
//!
//! * `RVM` (0x8707E8, texture hash 0xDBD4; black, alpha ≈ 0.53 inside a
//!   trapezoid) with SRCALPHA / INVSRCALPHA, the effect's colour mask
//!   (+0x31C) 0xF: the glass's tint;
//! * `RVM_MASK` (0x8707EC, 0xCF29D0DF; alpha 1 inside a slightly smaller
//!   trapezoid, 0 outside) with ONE / ZERO and the colour mask 8 (alpha
//!   only): the mirror's shape, which the screen quad's SRCALPHA blend
//!   (0x5CBF50) cuts out.
//!
//! Both are loaded by 0x5B98C0 (0x43DB50 → 0x4901D0). The quad's corners
//! are (−1, −1) … (1, 1) at z 0 with UVs (0, 0) at the top left; fe.efx's
// prov: inferred render.rear_view.module
//! pixel shader is texture × colour with the rgb doubled. **Inferred**:
//! D3D9's half-pixel offset (the quad is not shifted by half a texel) is
//! left out — a sub-texel difference in a 256 × 256 image.
use glam::{Mat3, Quat, Vec2, Vec3};

use crate::frontend::Quad;
use crate::render::Camera;

use crate::formats::texture::Texture;
use crate::render::{Gpu, REFLECTION_FORMAT, WorldRenderer, bind_texture, create_depth, make_texture_layout_plain, upload_texture};

/// Camera +0xC4 for view 3 (0x44BE48: 0x4E20), a binary angle.
pub const FOV: u16 = 20000;
/// Camera +0xBC / +0xC0 (0x7870D4 / 0x7870D8).
pub const NEAR: f32 = 0.75;
pub const FAR: f32 = 192.0;
/// The eye above the car's origin along its up axis: 0x7EC4C4 added to
/// the view's translation y (the view's y points down).
pub const EYE_UP: f32 = 0.5;
/// The overlay quad on the 640 × 480 canvas (0x5CC105..0x5CC13D): left,
/// top, right, bottom.
pub const RECT: [f32; 4] = [200.0, 15.0, 440.0, 95.0];
/// The UI texture slot the mirror's image is bound under (not a game
/// hash: the game binds the render target 0x8708A4 directly).
pub const TEXTURE: u32 = 0x5245_4152;

/// The field of view in radians.
pub fn fov() -> f32 {
    f32::from(FOV) * std::f32::consts::TAU / 65536.0
}

/// View 3's camera for a car at `position` turned by `orientation`
/// (0x44BA80): at the car's origin + 0.5 m up its up axis, looking along
/// its −x (out of the back), its up the car's up, square at 109.9°.
pub fn camera(position: Vec3, orientation: Quat) -> Camera {
    let m = Mat3::from_quat(orientation);
    let eye = position + m.z_axis * EYE_UP;
    let back = -m.x_axis;
    let mut c = Camera::new(eye, back.x.atan2(back.y), back.z.clamp(-1.0, 1.0).asin());
    c.up = m.z_axis;
    c.fov_y = fov();
    c.near = NEAR;
    c.far = FAR;
    c
}

/// The overlay quad (0x5CBF50): the image flipped across (u from 1 at the
/// left edge to 0 at the right), white, under the front end.
pub fn quad() -> Quad {
    Quad {
        texture: Some(TEXTURE),
        rect: [Vec2::new(RECT[0], RECT[1]), Vec2::new(RECT[2], RECT[3])],
        uv: [Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)],
        // Colour 0xFFFFFFFF through fe.efx's doubled rgb (texture ×
        // diffuse × 2): the image the tint pass darkened comes back.
        color: [2.0, 2.0, 2.0, 1.0],
        z: 255.0,
        angle: 0.0,
        place: crate::render::display::Place::TOP_CENTRE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_out_of_the_back() {
        // A car heading along +y.
        let q = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let c = camera(Vec3::new(10.0, 20.0, 1.0), q);
        assert!((c.position - Vec3::new(10.0, 20.0, 1.5)).length() < 1e-5);
        assert!((c.forward() - Vec3::new(0.0, -1.0, 0.0)).length() < 1e-5);
        // A point behind the car lands in the middle of the view; one to
        // the car's left (−x here) lands on the view's right.
        let vp = c.view_projection(1.0);
        let behind = vp.project_point3(Vec3::new(10.0, 10.0, 1.5));
        assert!(behind.x.abs() < 1e-4 && behind.y.abs() < 1e-4);
        let left = vp.project_point3(Vec3::new(9.0, 10.0, 1.5));
        assert!(left.x > 0.0);
        // Square: 109.9° both ways.
        assert!((fov().to_degrees() - 109.86).abs() < 0.01);
        let edge = vp.project_point3(Vec3::new(10.0 + 10.0 * (fov() * 0.5).tan(), 10.0, 1.5));
        assert!((edge.x + 1.0).abs() < 1e-3);
    }

    #[test]
    fn quad_is_flipped_across() {
        let q = quad();
        assert_eq!(q.rect[0], Vec2::new(200.0, 15.0));
        assert_eq!(q.rect[1], Vec2::new(440.0, 95.0));
        assert_eq!(q.uv[0].x, 1.0);
        assert_eq!(q.uv[1].x, 0.0);
    }
}

/// `RVM` and `RVM_MASK` (bin hashes of the names at 0x7A35B8 / 0x7A35AC).
pub const RVM: u32 = 0x0000_DBD4;
pub const RVM_MASK: u32 = 0xCF29_D0DF;

const SHADER: &str = r#"
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    // A strip over the target: (-1, 1), (1, 1), (-1, -1), (1, -1).
    let x = f32(i & 1u) * 2.0 - 1.0;
    let y = 1.0 - f32(i >> 1u) * 2.0;
    var out: VsOut;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // fe.efx: texture x diffuse (white), rgb doubled.
    let t = textureSample(tex, samp, in.uv);
    return vec4<f32>(min(t.rgb * 2.0, vec3<f32>(1.0)), t.a);
}
"#;

/// The two passes' pipelines and textures.
pub struct RearViewFrame {
    tint: Option<(wgpu::RenderPipeline, wgpu::BindGroup)>,
    mask: Option<(wgpu::RenderPipeline, wgpu::BindGroup)>,
}

impl RearViewFrame {
    pub fn new(gpu: &Gpu, rvm: Option<&Texture>, mask: Option<&Texture>) -> RearViewFrame {
        let device = &gpu.device;
        let layout = make_texture_layout_plain(device, "rear view frame");
        let shader = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("rear view frame"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline_layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("rear view frame"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("rear view frame"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bc = device.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let make = |label: &str, blend: wgpu::BlendState, write_mask: wgpu::ColorWrites| {
            crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: REFLECTION_FORMAT,
                        blend: Some(blend),
                        write_mask,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: gpu.disk_pipeline_cache(),
            })
        };
        // No effect enables SEPARATEALPHABLENDENABLE: alpha blends with the
        // colour factors.
        let same = |src: wgpu::BlendFactor, dst: wgpu::BlendFactor| {
            let c = wgpu::BlendComponent { src_factor: src, dst_factor: dst, operation: wgpu::BlendOperation::Add };
            wgpu::BlendState { color: c, alpha: c }
        };
        use wgpu::BlendFactor as F;
        let bind = |t: &Texture| bind_texture(device, &layout, &upload_texture(gpu, t, bc), &sampler);
        let tint = rvm
            .map(|t| (make("rear view tint", same(F::SrcAlpha, F::OneMinusSrcAlpha), wgpu::ColorWrites::ALL), bind(t)));
        let mask = mask.map(|t| (make("rear view mask", same(F::One, F::Zero), wgpu::ColorWrites::ALPHA), bind(t)));
        RearViewFrame { tint, mask }
    }

    /// Draw the tint, then the mask, into the mirror's image.
    pub fn record(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("rear view frame"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        for (pipeline, group) in [&self.tint, &self.mask].into_iter().flatten() {
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..4, 0..1);
        }
    }
}

// Refactor: from render/mod.rs
/// The rear-view mirror's target (render-target record 3, 0x86E7BC): 256 ×
/// 256 (0x7FEE80 / 0x7FEE84), one level, A8R8G8B8 (0x7FEE78 = 21) with a
/// D16 depth buffer (0x7FEE7C = 80), made by 0x5BA1B0.
pub const REAR_VIEW_SIZE: (u32, u32) = (256, 256);

/// The rear-view mirror's image (view 3) and its depth.
pub(in crate::render) struct RearViewTarget {
    pub(in crate::render) texture: wgpu::Texture,
    pub(in crate::render) color: wgpu::TextureView,
    pub(in crate::render) depth: wgpu::TextureView,
}

impl WorldRenderer {
    /// View 3's image (the rear-view mirror), once drawn: what the frame's
    /// overlay quad samples (0x5CC16F: 0x8708A4).
    pub fn rear_view_image(&self) -> Option<&wgpu::TextureView> {
        self.rear.as_ref().map(|r| &r.color)
    }

    /// The rear-view mirror's texture (for a read-back).
    pub fn rear_view_texture(&self) -> Option<&wgpu::Texture> {
        self.rear.as_ref().map(|r| &r.texture)
    }

    /// The rear-view mirror's depth, for the cars drawn into it.
    pub fn rear_view_depth(&self) -> Option<&wgpu::TextureView> {
        self.rear.as_ref().map(|r| &r.depth)
    }

    /// The rear-view mirror's target and frame passes (their pipelines and
    /// the `RVM` textures' upload), made now (a drive's load, when its
    /// first frame shows the mirror) instead of on the frame that first
    /// draws it. The view clears its target before every draw.
    pub fn warm_rear_view(&mut self, gpu: &Gpu) {
        self.ensure_rear_target(gpu);
    }

    pub(in crate::render) fn ensure_rear_target(&mut self, gpu: &Gpu) {
        if self.rear.is_some() {
            return;
        }
        let (w, h) = REAR_VIEW_SIZE;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rear view"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: REFLECTION_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let color = texture.create_view(&Default::default());
        let depth = create_depth(gpu, w, h);
        self.rear = Some(RearViewTarget { texture, color, depth });
        self.rear_frame_passes = Some(crate::render::rear_view::RearViewFrame::new(gpu, self.rear_textures.0.as_ref(), self.rear_textures.1.as_ref()));
    }

    /// View 3's frame passes (`RVM`, then `RVM_MASK`; see
    /// [`rear_view`]), after its world and cars.
    pub fn record_rear_view_frame(&self, encoder: &mut wgpu::CommandEncoder) {
        if let (Some(target), Some(passes)) = (&self.rear, &self.rear_frame_passes) {
            passes.record(encoder, &target.color);
        }
    }
}
