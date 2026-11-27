//!pass output
// The accumulated image into the scene: RCAS (ffx_fsr2_rcas.h, FSR 1's
// FsrRcasF) or a copy; and the depth at the display's size for the late
// sprites' depth test.
@group(0) @binding(2) var src: texture_2d<f32>;
@group(0) @binding(3) var depth_in: texture_depth_2d;

struct VsOut {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: VsOut;
    o.position = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    return o;
}

const RCAS_LIMIT: f32 = 0.25 - (1.0 / 16.0);

@fragment
fn fs_copy(in: VsOut) -> @location(0) vec4<f32> {
    return textureLoad(src, vec2<i32>(in.position.xy), 0);
}

fn rcas(p: vec2<i32>) -> vec4<f32> {
    let size = vec2<i32>(display_size());
    let b4 = textureLoad(src, clamp_load(p + vec2<i32>(0, -1), size), 0);
    let d4 = textureLoad(src, clamp_load(p + vec2<i32>(-1, 0), size), 0);
    let e4 = textureLoad(src, p, 0);
    let f4 = textureLoad(src, clamp_load(p + vec2<i32>(1, 0), size), 0);
    let h4 = textureLoad(src, clamp_load(p + vec2<i32>(0, 1), size), 0);
    // the lobe from the 0..1 image (a float scene may go over 1)
    let b = clamp(b4.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let d = clamp(d4.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let e = clamp(e4.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let f = clamp(f4.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let h = clamp(h4.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let bl = b.b * 0.5 + (b.r * 0.5 + b.g);
    let dl = d.b * 0.5 + (d.r * 0.5 + d.g);
    let el = e.b * 0.5 + (e.r * 0.5 + e.g);
    let fl = f.b * 0.5 + (f.r * 0.5 + f.g);
    let hl = h.b * 0.5 + (h.r * 0.5 + h.g);
    var nz = 0.25 * bl + 0.25 * dl + 0.25 * fl + 0.25 * hl - el;
    let range = max(max(max(bl, dl), max(el, fl)), hl) - min(min(min(bl, dl), min(el, fl)), hl);
    nz = clamp(abs(nz) / max(range, 1e-8), 0.0, 1.0);
    nz = -0.5 * nz + 1.0;
    let mn4 = min(min(b, d), min(f, h));
    let mx4 = max(max(b, d), max(f, h));
    let hit_min = min(mn4, e) / max(4.0 * mx4, vec3<f32>(1e-8));
    let hit_max = (1.0 - max(mx4, e)) / min(4.0 * mn4 - 4.0, vec3<f32>(-1e-8));
    let lobe_rgb = max(-hit_min, hit_max);
    var lobe = max(-RCAS_LIMIT, min(max(lobe_rgb.r, max(lobe_rgb.g, lobe_rgb.b)), 0.0)) * u.params.w;
    lobe *= nz;
    let rcp_l = 1.0 / (4.0 * lobe + 1.0);
    let rgb = (lobe * b4.rgb + lobe * d4.rgb + lobe * h4.rgb + lobe * f4.rgb + e4.rgb) * rcp_l;
    return vec4<f32>(rgb, e4.a);
}

@fragment
fn fs_rcas(in: VsOut) -> @location(0) vec4<f32> {
    return rcas(vec2<i32>(in.position.xy));
}

// The render depth at the display's size (the unjittered content's place).
fn depth_at(position: vec2<f32>) -> f32 {
    let uv = position * u.display.zw;
    let p = vec2<i32>(floor(uv * render_size() + jitter()));
    return textureLoad(depth_in, clamp_load(p, vec2<i32>(render_size())), 0);
}

@fragment
fn fs_depth(in: VsOut) -> @builtin(frag_depth) f32 {
    return depth_at(in.position.xy);
}
