//! The native screen passes' plumbing: switches, frame values, gain/ramp maps, uniforms and full-screen passes (moved from render/postfx.rs).

use super::*;

/// The switches of 0x5BE880 that these passes read, from the detail level
/// (0x87099C) as the game sets them on a fresh run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Switches {
    /// g_LightGlowEnable (0x870CE4): detail above 1.
    pub light_glow: bool,
    /// g_LightStreaksEnable (0x870CE0): detail above 2.
    pub light_streaks: bool,
    /// g_OverBrightEnable (0x870CFC): detail above 2.
    pub overbright: bool,
    /// g_BleachByPassEnable (0x870D00): detail above 3.
    pub bleach_bypass: bool,
    /// g_MotionBlurEnable (0x870CDC): detail above 2.
    pub motion_blur: bool,
    /// g_HorizonFogEnable (0x870D0C): detail above 2.
    pub horizon_fog: bool,
    /// g_TintingEnable (0x870D04): detail above 1 (the rain's tint pass).
    pub tinting: bool,
    /// g_RainEnable (0x870D10, detail above 3) with the Particle System
    /// (0x870CEC): 0x8707D2, the lens drops (0x5CB7F6).
    pub rain_splatter: bool,
}

impl Switches {
    pub fn from_detail(detail: u8) -> Switches {
        Switches {
            light_glow: detail > 1,
            light_streaks: detail > 2,
            overbright: detail > 2,
            bleach_bypass: detail > 3,
            motion_blur: detail > 2,
            horizon_fog: detail > 2,
            tinting: detail > 1,
            rain_splatter: detail > 3,
        }
    }

    /// The switches as the frame function reads them: UI_PC_Customize_Options'
    /// values (0x870CB8…, in `custom` by its row order): Light Trails 8
    /// (0x870CE0), Light Glow 9 (0x870CE4), Motion Blur 11 (0x870CDC),
    /// Horizon Fog 16 (0x870D0C), Over Bright 17 (0x870CFC), Enhanced
    /// Contrast 18 (0x870D00). [`Switches::from_detail`] is these as a
    /// Level Of Detail presets them (0x5BE880).
    pub fn from_custom(custom: &[u8; 22]) -> Switches {
        Switches {
            light_glow: custom[9] != 0,
            light_streaks: custom[8] != 0,
            overbright: custom[17] != 0,
            bleach_bypass: custom[18] != 0,
            motion_blur: custom[11] != 0,
            horizon_fog: custom[16] != 0,
            tinting: custom[15] != 0,
            rain_splatter: custom[19] != 0 && custom[10] != 0,
        }
    }

    /// 0x8707CE: the overbright pass needs all three switches (0x5CB778).
    pub(super) fn overbright_pass(self) -> bool {
        self.overbright && self.light_glow && self.light_streaks
    }

    /// 0x5CBB10 (the streak buffer's decay and draw) runs when the glow and
    /// the streaks are both on.
    pub fn streaks(self) -> bool {
        self.light_glow && self.light_streaks
    }
}

/// The per-frame values of 0x5CB8A9…0x5CB9A9.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// The front end (mode 0x8654A4 ≠ 6) or the game.
    pub front_end: bool,
    /// The bleach bypass amount as 0x5C68E0 gets it (0x5CB9F6): the
    /// frame's value lerped from 0x7FF180 (0.2) to 0x7FF184 (1). It is
    /// FILTERBLEND (0x86EC98), lerps the gain map's curves from 0.5
    /// (0x7FF12C…) to 0.2 (0x7FF13C…) and scales TINTCOLOR.rgb by
    /// lerp(0, 0.25 (0x7FF128), amount) (0x7FF16C; 0x8708EC is BSS).
    pub bleach: f32,
    /// TINTCOLOR.w (0x7FF188): how far p2 pulls towards grey.
    pub desaturate: f32,
    /// TINTCOLOR.rgb before that scale (0x7FF15C…0x7FF164).
    pub tint: [f32; 3],
}

/// 0x5CB9F6: lerp(0x7FF180, 0x7FF184, value).
pub(super) fn bleach_amount(value: f32) -> f32 {
    (1.0 - 0.2) * value + 0.2
}

impl Frame {
    /// The front end (0x5CB943): 0.8, 0.2 and grey 0.5 when screenfilter.efx
    /// runs its first technique (effect +0x0C = 0), as on any ps_1_4 card.
    pub const FRONT_END: Frame = Frame { front_end: true, bleach: 0.2 + 0.8 * 0.8, desaturate: 0.2, tint: [0.5; 3] };

    /// In game without a screen-tint record (slot 3 off, 0x5CB91B): the
    /// value is 0 and TINTCOLOR keeps the front end's.
    pub const GAME: Frame = Frame { front_end: false, bleach: 0.2, desaturate: 0.2, tint: [0.5; 3] };

    /// In game, 0x5CB91B takes the value, TINTCOLOR.w and .rgb from the main
    /// view's screen-tint record (+0x188, +0x1CC, +0x178…), filled from the
    /// world's tint maps by 0x5D7800 (see [`crate::screen_tint`]).
    pub fn game(tint: &crate::screen_tint::ScreenTint) -> Frame {
        if !tint.enabled {
            return Frame { desaturate: tint.grey, ..Frame::GAME };
        }
        Frame { front_end: false, bleach: bleach_amount(tint.bleach), desaturate: tint.grey, tint: tint.color }
    }

    /// TINTCOLOR.rgb as screenfilter.efx gets it (0x5C66E4).
    pub(crate) fn tint_color(&self) -> [f32; 3] {
        let scale = TINT_SCALE * self.bleach;
        self.tint.map(|c| c * scale)
    }
}

/// 0x7FF128: TINTCOLOR.rgb's scale at amount 1.
const TINT_SCALE: f32 = 0.25;
/// SATURATIONSCALE (0x5C6737: SetFloat of 0x7A27DC).
pub(crate) const SATURATION_SCALE: f32 = 0.4;

/// 5BBE90's 2D gain map size (0x7FF798) and 1D ramp size (0x7FF7A0).
pub(crate) const GAIN_SIZE: usize = 32;
pub(super) const RAMP_SIZE: usize = 256;
/// The gain curves' pivot (0x7FF7AC, reset to 0.1 below 0.1).
pub(super) const GAIN_PIVOT: f32 = 0.4;
/// The ramp's bias (0x7FF18C).
const RAMP_BIAS: f32 = 0.45;

/// 0x5BBDC0: the gain curve through (pivot, pivot) with steepness `g`
/// (0.5 is the identity).
pub fn gain(x: f32, g: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    if x > 1.0 {
        return 1.0;
    }
    let p = if GAIN_PIVOT < 0.1 { 0.1 } else { GAIN_PIVOT };
    let a = (1.0 - x / p) * (1.0 / g - 2.0);
    if x < p { x / (a + 1.0) } else { (a - x) / (a - 1.0) }
}

/// __ftol of `v · 255` (0x75BED4 truncates).
fn byte(v: f32) -> u8 {
    (v * 255.0) as i32 as u8
}

/// 0x5BBE90's gain map: texel (column j, row i) with u = j / 31,
/// v = i / 31 holds A = gain(u, 0x7FF158), R = gain(v, 0x7FF14C),
/// G = gain(u, 0x7FF150), B = gain(v, 0x7FF154), the four curves being
/// lerp(0.5, 0.2, amount) each (0x5C68E0).
pub fn gain_map(amount: f32) -> Vec<u8> {
    let g = (0.2 - 0.5) * amount + 0.5;
    let mut rgba = vec![0u8; GAIN_SIZE * GAIN_SIZE * 4];
    let n = (GAIN_SIZE - 1) as f32;
    for i in 0..GAIN_SIZE {
        for j in 0..GAIN_SIZE {
            let (u, v) = (j as f32 / n, i as f32 / n);
            let o = (i * GAIN_SIZE + j) * 4;
            rgba[o..o + 4].copy_from_slice(&[byte(gain(v, g)), byte(gain(u, g)), byte(gain(v, g)), byte(gain(u, g))]);
        }
    }
    rgba
}

/// 0x5BC050's ramp, one texel wide and 256 high: R = G = A = x and
/// B = x / (1 + (1/0.45 − 2)(1 − x)), x = row / 255.
pub fn ramp_map() -> Vec<u8> {
    let mut rgba = vec![0u8; RAMP_SIZE * 4];
    for i in 0..RAMP_SIZE {
        let x = i as f32 / (RAMP_SIZE - 1) as f32;
        let b = if x <= 0.0 {
            0.0
        } else if x >= 1.0 {
            1.0
        } else {
            x / ((1.0 / RAMP_BIAS - 2.0) * (1.0 - x) + 1.0)
        };
        rgba[i * 4..i * 4 + 4].copy_from_slice(&[byte(x), byte(x), byte(b), byte(x)]);
    }
    rgba
}

/// filter.efx taps (weight, du, dv in source texels) of the blur pair
/// 0x7FF1CC (across) and 0x7FF19C (down), run 0x7FF110 = 2 times.
pub(super) const BLUR_ACROSS: [(f32, f32, f32); 4] =
    [(1.0 / 3.0, 1.0, 0.0), (1.0 / 6.0, 3.0, 0.0), (1.0 / 3.0, -1.0, 0.0), (1.0 / 6.0, -3.0, 0.0)];
pub(super) const BLUR_DOWN: [(f32, f32, f32); 4] =
    [(1.0 / 3.0, 0.0, 1.0), (1.0 / 6.0, 0.0, 3.0), (1.0 / 3.0, 0.0, -1.0), (1.0 / 6.0, 0.0, -3.0)];
pub(super) const BLUR_ROUNDS: usize = 2;
/// OVERBRIGHTGREYSCALE (0x5C63FB): 150/255 × 3, 200/255.
pub(crate) const OVERBRIGHT_GREY: [f32; 4] = [150.0 / 255.0, 150.0 / 255.0, 150.0 / 255.0, 200.0 / 255.0];
/// D3DRS_BLENDFACTOR of the subtraction (0x5CCAED): 0xB0 in game, 0xA0 in
/// the front end.
pub(super) const SUBTRACT_GAME: f64 = 176.0 / 255.0;
pub(super) const SUBTRACT_FRONT_END: f64 = 160.0 / 255.0;
/// D3DRS_BLENDFACTOR of the glow's add into the streak buffer (0x5CCB59):
/// 0x7FF114 ([`OverbrightLevel::add`], 22 in a settled game frame) in game,
/// 80 (0x7875B8) in the front end.
pub(super) const ADD_FRONT_END: f64 = 80.0 / 255.0;

pub(crate) const INTERMEDIATE: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub(crate) const SLOT: u64 = 256;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PassUniform {
    pub(crate) weights: [[f32; 4]; 4],
    pub(crate) offsets: [[f32; 4]; 4],
    pub(crate) params: [f32; 4],
    pub(crate) grey: [f32; 4],
    pub(crate) tint: [f32; 4],
}

/// Uniform slots.
pub(super) const COPY: u32 = 0;
pub(super) const OVERBRIGHT: u32 = 1;
pub(super) const ACROSS: u32 = 2;
pub(super) const DOWN: u32 = 3;
pub(super) const BLEACH: u32 = 4;
/// The slots `apply` fills each frame.
pub(super) const SLOTS: u64 = 5;
/// Motion blur's two passes (0x5CC470) and its one-tap copy, filled by
/// `motion_blur`: slots of its own, so the end-of-frame passes' writes
/// never touch what the scene's command buffers read.
pub(super) const MOTION1: u32 = 5;
pub(super) const MOTION2: u32 = 6;
pub(super) const MOTION_COPY: u32 = 7;
/// High Precision Post's glow prefilter (written only with it on).
pub(super) const PREFILTER: u32 = 8;
/// The resolve to the screen's size under SSAA with High Precision Post
/// (docs/notes/graphics/ssaa.md "Effect and post passes at the screen's size under SSAA"): the bleach bypass's values, offsets[1]
/// the scene's texels per screen pixel.
pub(super) const RESOLVE: u32 = 9;
pub(super) const ALL_SLOTS: u64 = 10;

/// High Precision Post's glow blur (docs/notes/graphics/hdr-post.md): one Gaussian pass
/// across and one down standing in for filter.efx's two rounds of the
/// ±1/±3-texel pair at half size. Its σ, in half-size texels, has the
/// variance of those two rounds (2 · 11/3); at the screen's size it is
/// doubled, with the native half-size tap's 2×2 box and its bilinear
/// upsampling added and this path's own 3×3 tent taken off.
pub(super) const HDR_SIGMA_HALF: f32 = 2.708_013;
pub(super) const HDR_SIGMA_FULL: f32 = 5.454_356;

/// The glow targets' (A–C) size for a scene `scene` big shown `out` big
/// (docs/notes/graphics/ssaa.md "Post effects under SSAA and Render Scale"): native, half the screen's (0x5BB850), or High
/// Precision Post's 2, the screen's. With SSAA or Render Scale the scene
/// is not the screen: the glow is then made at the size native makes it
/// for the screen, so its taps (±1/±3 texels, σ) span the same part of the
/// screen whatever the scene's size. At 1× this is the scene's own.
pub fn glow_size(scene: (u32, u32), out: (u32, u32), hdr: Option<u8>) -> (u32, u32) {
    let base = if scene == out { scene } else { out };
    if hdr == Some(2) { (base.0.max(1), base.1.max(1)) } else { ((base.0 / 2).max(1), (base.1 / 2).max(1)) }
}

/// The most box taps a side ([`box_prefilter`]; postfx.wgsl `fs_box`).
pub(super) const BOX_MAX: u32 = 8;

/// The prefilter from a scene `scene` big to glow targets `glow` big when
/// the two are not native's 2:1 (or 1:1): an `n`×`n` grid of bilinear taps
/// (each the mean of 2×2 texels), `step` apart in uv, averaging a box
/// `width` glow texels wide. Native's glow texel is the 2×2 box of the
/// screen's texels under it (`width` 1); the same box measured on the
/// screen covers 2·s scene texels at a scale s, which `n` = ⌈2s / 2⌉ taps
/// cover exactly for whole s.
pub fn box_prefilter(scene: (u32, u32), glow: (u32, u32), width: f32) -> (u32, [f32; 2]) {
    let f = [width * scene.0 as f32 / glow.0.max(1) as f32, width * scene.1 as f32 / glow.1.max(1) as f32];
    let n = ((f[0].max(f[1]) / 2.0 - 1e-3).ceil() as u32).clamp(1, BOX_MAX);
    (n, [f[0] / n as f32 / scene.0.max(1) as f32, f[1] / n as f32 / scene.1.max(1) as f32])
}

/// High Precision Post 2's prefilter at the screen's size is a 3×3 tent
/// (¼ ½ ¼, variance ½ texel², with the 1-texel box of the screen's own
/// pixel 7/12): the box of that variance, √7 glow texels wide, stands in
/// for it when the scene is not the screen's size.
pub(super) const HDR_FULL_BOX: f32 = 2.645_751_3;

/// Bind groups, one per texture read; the `…Streak` ones also hold the
/// streak buffer (for `fs_add`).
#[derive(Clone, Copy)]
pub(super) enum Src {
    Scene,
    Full0,
    Full1,
    A,
    B,
    C,
    SceneStreak,
    Full0Streak,
    Full1Streak,
    /// The streak buffer itself (copied into the multisampled one).
    Streak,
    /// Half B with half C beside it (High Precision Post's blur and
    /// subtraction in one pass).
    BWithC,
    /// The image resolved to the screen's size (SSAA with High Precision
    /// Post, [`resolves_first`]), and with the streak buffer beside it.
    Resolved,
    ResolvedStreak,
}

impl Src {
    /// The image with the streak buffer beside it.
    pub(super) fn with_streak(self) -> Src {
        match self {
            Src::Scene => Src::SceneStreak,
            Src::Full0 => Src::Full0Streak,
            Src::Full1 => Src::Full1Streak,
            Src::Resolved => Src::ResolvedStreak,
            other => other,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Blend {
    Replace,
    /// dst − src · constant (BLENDOP_REVSUBTRACT, BLENDFACTOR / ONE).
    SubtractConstant,
    /// dst + src · constant (BLENDFACTOR / ONE).
    AddConstant,
    /// dst · constant (ZERO / BLENDFACTOR).
    ScaleDest,
    /// src · dst + dst · constant (DESTCOLOR / BLENDFACTOR): with `fs_one`,
    /// dst · (1 + constant) — the part of a DESTCOLOR / SRCCOLOR multiply
    /// (2 · src · dst) above 1, which a blend constant cannot carry (it is
    /// clamped to 1 on a normalised target).
    AddScaledDest,
    /// src · constant + dst · (1 − constant) (BLENDFACTOR / INVBLENDFACTOR).
    LerpConstant,
    /// min(src, dst): with `fs_one`, the 8-bit target's saturation of the
    /// half-float streak buffer (High Precision Post).
    Min,
}

impl Blend {
    pub(super) fn state(self) -> Option<wgpu::BlendState> {
        use wgpu::{BlendComponent, BlendFactor as F, BlendOperation as O};
        let c = |src_factor, dst_factor, operation| {
            let comp = BlendComponent { src_factor, dst_factor, operation };
            Some(wgpu::BlendState { color: comp, alpha: comp })
        };
        match self {
            Blend::Replace => None,
            Blend::SubtractConstant => c(F::Constant, F::One, O::ReverseSubtract),
            Blend::AddConstant => c(F::Constant, F::One, O::Add),
            Blend::ScaleDest => c(F::Zero, F::Constant, O::Add),
            Blend::AddScaledDest => c(F::Dst, F::Constant, O::Add),
            Blend::LerpConstant => c(F::Constant, F::OneMinusConstant, O::Add),
            Blend::Min => c(F::One, F::One, O::Min),
        }
    }
}

pub(crate) fn texture(
    gpu: &Gpu,
    label: &str,
    (width, height): (u32, u32),
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

pub(crate) fn write_texture(gpu: &Gpu, t: &wgpu::Texture, rgba: &[u8], width: u32, height: u32) {
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: t,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(width * 4), rows_per_image: Some(height) },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
}

/// A pass that only clears `target`.
pub(crate) fn clear(encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, color: wgpu::Color) {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(color), store: wgpu::StoreOp::Store },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
}

/// One full-screen triangle with `pipeline` into `targets`.
#[allow(clippy::too_many_arguments)]
pub(super) fn full_pass(
    encoder: &mut wgpu::CommandEncoder,
    label: &str,
    pipeline: &wgpu::RenderPipeline,
    targets: &[&wgpu::TextureView],
    group: &wgpu::BindGroup,
    slot: u32,
    constant: Option<wgpu::Color>,
    clear: Option<wgpu::Color>,
) {
    full_pass_op(
        encoder,
        label,
        pipeline,
        targets,
        group,
        slot,
        constant,
        clear.map_or(wgpu::LoadOp::Load, wgpu::LoadOp::Clear),
    );
}

/// A replacing full-screen pass's load: every pixel is written (one
/// full-screen triangle, blending off, no discard in postfx.wgsl), so the
/// target's old contents are neither read in nor cleared (load/store
/// hygiene: tile-based GPUs skip the read; a clear instead cost AMD's RDNA
/// ~0.04 ms a frame in fast-clear work, measured).
pub(crate) fn replace_load() -> wgpu::LoadOp<wgpu::Color> {
    // SAFETY: only for passes that write every pixel of every target
    // before anything reads or stores them (see above).
    wgpu::LoadOp::DontCare(unsafe { wgpu::LoadOpDontCare::enabled() })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn full_pass_op(
    encoder: &mut wgpu::CommandEncoder,
    label: &str,
    pipeline: &wgpu::RenderPipeline,
    targets: &[&wgpu::TextureView],
    group: &wgpu::BindGroup,
    slot: u32,
    constant: Option<wgpu::Color>,
    load: wgpu::LoadOp<wgpu::Color>,
) {
    let attachments: Vec<Option<wgpu::RenderPassColorAttachment>> = targets
        .iter()
        .map(|&view| {
            Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
            })
        })
        .collect();
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[slot * SLOT as u32]);
    if let Some(c) = constant {
        pass.set_blend_constant(c);
    }
    pass.draw(0..3, 0..1);
}
