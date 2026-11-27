//!pass depth_clip
// ffx_fsr2_depth_clip.h, with the clone's masks.
@group(0) @binding(2) var input_depth: texture_depth_2d;
@group(0) @binding(3) var input_color: texture_2d<f32>;
@group(0) @binding(5) var motion: texture_2d<f32>;
@group(0) @binding(6) var dilated_motion: texture_2d<f32>;
@group(0) @binding(7) var prev_dilated_motion: texture_2d<f32>;
@group(0) @binding(8) var dilated_depth: texture_2d<f32>;
@group(0) @binding(9) var<storage, read> reconstructed: array<u32>;
@group(0) @binding(10) var out_prepared: texture_storage_2d<rgba16float, write>;
@group(0) @binding(11) var out_masks: texture_storage_2d<rgba8unorm, write>;

fn load_reconstructed(p0: vec2<i32>) -> f32 {
    let p = clamp_load(p0, vec2<i32>(render_size()));
    let a = bitcast<f32>(reconstructed[u32(p.y) * u32(render_size().x) + u32(p.x)]);
    return select(1.0 - a, a, reversed());
}

fn depth_clip(uv: vec2<f32>, current: f32) -> f32 {
    let current_view = view_depth(current);
    let b = bilinear_data(uv, render_size());
    let size = vec2<i32>(render_size());
    var depth = 0.0;
    var wsum = 0.0;
    // Kfov = |corner| / |centre| of the view at a depth
    let kfov = sqrt(1.0 + u.depth.z * u.depth.z + u.depth.w * u.depth.w);
    let half_width = length(render_size());
    let resolution = clamp(length(render_size()) / length(vec2<f32>(1920.0, 1080.0)), 0.0, 1.0);
    let power = mix(1.0, 3.0, resolution);
    for (var i = 0; i < 4; i++) {
        let s = b.base + BILINEAR_OFFSETS[i];
        if on_screen(s, size) {
            let w = b.weights[i];
            if w > RECONSTRUCTED_DEPTH_THRESHOLD {
                let prev = load_reconstructed(s);
                let prev_view = view_depth(prev);
                let diff = current_view - prev_view;
                if diff > 0.0 {
                    let threshold = max(current_view, prev_view);
                    let required = 1.37e-05 * kfov * half_width * threshold;
                    depth += pow(clamp(required / diff, 0.0, 1.0), power) * w;
                    wsum += w;
                }
            }
        }
    }
    return select(0.0, clamp(1.0 - depth / wsum, 0.0, 1.0), wsum > 0.0);
}

// ComputeMotionDivergence over the 3 x 3 motion already loaded.
fn motion_divergence(mvs: array<vec2<f32>, 9>) -> f32 {
    var minconvergence = 1.0;
    let nucleus = mvs[4];
    let nucleus_lr = length(nucleus * render_size());
    var max_uv = length(nucleus);
    if nucleus_lr > 1e-2 {
        for (var i = 0; i < 9; i++) {
            let mv = mvs[i];
            var v = length(mv);
            max_uv = max(v, max_uv);
            v = max(v, max_uv);
            minconvergence = min(minconvergence, dot(mv / v, nucleus / v));
        }
    }
    return clamp(1.0 - minconvergence, 0.0, 1.0) * clamp(max_uv / 0.01, 0.0, 1.0);
}

fn depth_divergence(p: vec2<i32>) -> f32 {
    let size = vec2<i32>(render_size());
    let max_dist = view_depth(far_depth());
    var dmax = 0.0;
    var dmin = max_dist;
    var found = false;
    for (var y = -1; y < 2; y++) {
        for (var x = -1; x < 2; x++) {
            let s = p + vec2<i32>(x, y);
            var d = 0.0;
            if on_screen(s, size) {
                d = view_depth(textureLoad(dilated_depth, s, 0).x);
            }
            found = found || (max_dist == d);
            dmin = min(dmin, d);
            dmax = max(dmax, d);
        }
    }
    return select((1.0 - dmin / dmax), 0.0, found);
}

fn temporal_motion_divergence(p: vec2<i32>) -> f32 {
    let uv = (vec2<f32>(p) + 0.5) * u.render.zw;
    let mv = textureLoad(dilated_motion, p, 0).xy;
    let ruv = clamp(uv + mv, vec2<f32>(0.0), vec2<f32>(1.0));
    let prev = textureLoad(prev_dilated_motion, clamp_load(vec2<i32>(ruv * render_size()), vec2<i32>(render_size())), 0).xy;
    let px = length(mv * display_size());
    if px > 1.0 {
        return mix(0.0, 1.0 - clamp(length(prev) / length(mv), 0.0, 1.0), clamp(pow(px / 20.0, 3.0), 0.0, 1.0));
    }
    return 0.0;
}

fn evaluate_surface(p: vec2<i32>) -> f32 {
    let d0 = view_depth(load_reconstructed(p + vec2<i32>(0, -1)));
    let d1 = view_depth(load_reconstructed(p));
    let d2 = view_depth(load_reconstructed(p + vec2<i32>(0, 1)));
    return 1.0 - f32(((d0 - d1) > (d1 * 0.01)) && ((d1 - d2) > (d2 * 0.01)));
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let p = vec2<i32>(id.xy);
    let size = vec2<i32>(render_size());
    if !on_screen(p, size) {
        return;
    }
    let depth_uv = (vec2<f32>(p) + 0.5) * u.render.zw;
    var mv = textureLoad(dilated_motion, p, 0).xy;
    if length(mv * display_size()) <= 0.01 {
        mv = vec2<f32>(0.0);
    }
    let dilated = textureLoad(dilated_depth, p, 0).x;
    let clip = depth_clip(depth_uv + mv, dilated) * evaluate_surface(p);
    let c = textureLoad(input_color, p, 0);
    let rgb = clamp(c.rgb, vec3<f32>(0.0), vec3<f32>(FP16_MAX));
    textureStore(out_prepared, p, vec4<f32>(rgb_to_ycocg(rgb), c.a));
    // the 3 x 3 of motion once: the divergence and the masks' dilation
    // (PreProcessReactiveMasks, weighted by colour similarity)
    var mvs: array<vec2<f32>, 9>;
    var factor = vec2<f32>(0.0);
    for (var i = 0; i < 9; i++) {
        let s = clamp_load(p + vec2<i32>(i % 3 - 1, i / 3 - 1), size);
        let m = textureLoad(motion, s, 0);
        mvs[i] = m.xy;
        let k = m.zw;
        if k.x + k.y > 0.0 {
            let cs = textureLoad(input_color, s, 0).rgb;
            let max_len_sq = max(dot(c.rgb, c.rgb), dot(cs, cs));
            let similarity = select(1.0, dot(c.rgb, cs) / max_len_sq, max_len_sq > 0.0);
            let power = 1.0 + (6.0 - similarity * 6.0);
            factor = max(factor, vec2<f32>(pow(k.x, power), pow(k.y, power)));
        }
    }
    let motion_div = motion_divergence(mvs);
    var temporal_div = temporal_motion_divergence(p);
    if temporal_div > 0.0 {
        temporal_div = clamp(temporal_div - depth_divergence(p), 0.0, 1.0);
    }
    let masks = max(factor, vec2<f32>(0.0, max(temporal_div, motion_div)));
    textureStore(out_masks, p, vec4<f32>(masks, clip, 0.0));
}

