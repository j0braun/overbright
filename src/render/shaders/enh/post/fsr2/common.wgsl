// FSR 2 temporal upscaler (docs/notes/graphics/fsr2.md; a clone deviation, opt-in on
// the Upscaling row). A WGSL port of the AMD FidelityFX Super Resolution
// 2.2 passes (MIT licence, Copyright (c) 2022-2023 Advanced Micro Devices,
// Inc.; THIRD-PARTY-NOTICES.md): reconstruct-and-dilate
// (ffx_fsr2_reconstruct_dilated_velocity_and_previous_depth.h), depth clip
// (ffx_fsr2_depth_clip.h), lock (ffx_fsr2_lock.h), accumulate
// (ffx_fsr2_accumulate.h, _upsample.h, _reproject.h,
// _postprocess_lock_status.h, _common.h, _sample.h) and RCAS
// (ffx_fsr2_rcas.h / ffx_fsr1.h). Differences from the original, all
// because of the clone's inputs (see docs/notes/graphics/fsr2.md):
// * motion vectors are made here from the depth and the drawn cars' rigid
//   poses (the TAA pass's reprojection, temporal.wgsl), not read from a velocity
//   buffer (the clone's renderer writes none);
// * the shading-change luma is a 32 x 32 average of the log luma in one
//   pass (FSR 2 takes the same mip level from its SPD pyramid); no auto
//   exposure (exposure 1, the scene is display-referred);
// * the transparency-and-composition mask comes from the scene before
//   and after the screen-space composites (SSR, volumetric fog); the
//   reactive mask from the cars' spinning wheels and fresh cars;
// * the scene's alpha (NFSU2's bleach mask) is accumulated alongside, in
//   the lock-status texture's third channel;
// * 32-bit float textures where FSR 2 uses 16-bit or 8-bit ones (WebGPU's
//   storage formats).
// The file is split into modules at the `//!pass` markers; the common part
// (everything before the first) is prepended to each.

struct Car {
    inv: mat4x4<f32>,
    motion: mat4x4<f32>,
    lo: vec4<f32>,
    hi: vec4<f32>,
    wheel: array<vec4<f32>, 4>,
    wheel_hw: vec4<f32>,
    sphere: vec4<f32>,
};

struct Uniforms {
    // jittered clip (this frame) -> camera-relative world
    clip_to_rel: mat4x4<f32>,
    // camera-relative world (this frame's eye) -> last frame's clip
    prev_rel_to_clip: mat4x4<f32>,
    // render size, 1 / render size
    render: vec4<f32>,
    // display size, 1 / display size
    display: vec4<f32>,
    // jitter (render px, x right y down), reset (1: no history), reversed depth (0/1)
    jitter: vec4<f32>,
    // car count, HDR input (0/1), jitter phase count, RCAS sharpness (linear)
    params: vec4<f32>,
    // FSR 2's DeviceToViewSpaceTransformFactors
    depth: vec4<f32>,
    // x: debug view, y: composition mask gain, z: wheel reactive, w: luma mip level size divisor
    misc: vec4<f32>,
    // luma mip size (xy), 1 / size (zw)
    luma: vec4<f32>,
    // x: a composition mask was made this frame (0/1)
    extra: vec4<f32>,
    // x the animation mask is bound (0/1)
    anim: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var<uniform> cars: array<Car, 32>;

const EPSILON: f32 = 1e-3;
const FP16_MAX: f32 = 65504.0;
const UPSAMPLE_WEIGHT_SCALE: f32 = 1.0 / 12.0;
const MAX_ACCUMULATION_WEIGHT: f32 = 1.0;
const AVERAGE_WEIGHT_PER_FRAME: f32 = 0.74 / 12.0;
const RECONSTRUCTED_DEPTH_THRESHOLD: f32 = 0.01;

fn render_size() -> vec2<f32> { return u.render.xy; }
fn display_size() -> vec2<f32> { return u.display.xy; }
fn jitter() -> vec2<f32> { return u.jitter.xy; }
fn reversed() -> bool { return u.jitter.w > 0.5; }
fn hdr() -> bool { return u.params.y > 0.5; }

fn on_screen(p: vec2<i32>, size: vec2<i32>) -> bool {
    return all(p >= vec2<i32>(0)) && all(p < size);
}

fn clamp_load(p: vec2<i32>, size: vec2<i32>) -> vec2<i32> {
    return clamp(p, vec2<i32>(0), size - 1);
}

fn rgb_to_ycocg(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        0.25 * c.r + 0.5 * c.g + 0.25 * c.b,
        0.5 * c.r - 0.5 * c.b,
        -0.25 * c.r + 0.5 * c.g - 0.25 * c.b);
}

fn ycocg_to_rgb(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(c.x + c.y - c.z, c.x + c.z, c.x - c.y - c.z);
}

fn rgb_to_luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn perceived_luma(c: vec3<f32>) -> f32 {
    let l = rgb_to_luma(c);
    var p = 0.0;
    if l <= 216.0 / 24389.0 {
        p = l * (24389.0 / 27.0);
    } else {
        p = pow(l, 1.0 / 3.0) * 116.0 - 16.0;
    }
    return p * 0.01;
}

fn tonemap(c: vec3<f32>) -> vec3<f32> {
    return c / (max(max(0.0, c.r), max(c.g, c.b)) + 1.0);
}

fn inverse_tonemap(c: vec3<f32>) -> vec3<f32> {
    return c / max(1.0 / FP16_MAX, 1.0 - max(c.r, max(c.g, c.b)));
}

fn min_divided_by_max(a: f32, b: f32) -> f32 {
    let m = max(a, b);
    return select(0.0, min(a, b) / m, m != 0.0);
}

// FSR 2's GetViewSpaceDepth: device depth -> distance along the view.
fn view_depth(d: f32) -> f32 {
    return u.depth.y / (d - u.depth.x);
}

fn far_depth() -> f32 {
    return select(1.0, 0.0, reversed());
}

// FSR 2's ComputeHrPosFromLrPos.
fn hr_from_lr(p: vec2<i32>) -> vec2<i32> {
    let s = vec2<f32>(p) + 0.5 - jitter();
    return vec2<i32>(floor(s / render_size() * display_size()));
}

// The FSR 1 Lanczos 2 approximation of x² (<= 4).
fn lanczos2_approx_sq(x2: f32) -> f32 {
    let x = min(x2, 4.0);
    let a = (2.0 / 5.0) * x - 1.0;
    let b = (1.0 / 4.0) * x - 1.0;
    return ((25.0 / 16.0) * a * a - (25.0 / 16.0 - 1.0)) * (b * b);
}

fn lanczos2(x0: f32) -> f32 {
    let x = min(abs(x0), 2.0);
    let pi = 3.141592653589793;
    if x < EPSILON {
        return 1.0;
    }
    return (sin(pi * x) / (pi * x)) * (sin(0.5 * pi * x) / (0.5 * pi * x));
}

struct Bilinear {
    base: vec2<i32>,
    weights: vec4<f32>,
};

fn bilinear_data(uv: vec2<f32>, size: vec2<f32>) -> Bilinear {
    let px = uv * size - 0.5;
    var b: Bilinear;
    b.base = vec2<i32>(floor(px));
    let f = fract(px);
    b.weights = vec4<f32>((1.0 - f.x) * (1.0 - f.y), f.x * (1.0 - f.y), (1.0 - f.x) * f.y, f.x * f.y);
    return b;
}

const BILINEAR_OFFSETS = array<vec2<i32>, 4>(vec2<i32>(0, 0), vec2<i32>(1, 0), vec2<i32>(0, 1), vec2<i32>(1, 1));

