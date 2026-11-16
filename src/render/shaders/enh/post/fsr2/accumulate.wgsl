//!pass accumulate
// ffx_fsr2_accumulate.h and the files it includes.
@group(0) @binding(2) var prepared: texture_2d<f32>;
@group(0) @binding(3) var masks: texture_2d<f32>;
@group(0) @binding(4) var dilated_motion: texture_2d<f32>;
@group(0) @binding(5) var history: texture_2d<f32>;
@group(0) @binding(6) var lock_status: texture_2d<f32>;
@group(0) @binding(7) var luma_history: texture_2d<f32>;
@group(0) @binding(8) var luma_mip: texture_2d<f32>;
@group(0) @binding(9) var linear_clamp: sampler;
@group(0) @binding(10) var<storage, read_write> new_locks: array<f32>;
@group(0) @binding(11) var out_history: texture_storage_2d<rgba16float, write>;
@group(0) @binding(12) var out_lock_status: texture_storage_2d<rgba16float, write>;
@group(0) @binding(13) var out_luma_history: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(14) var out_color: texture_storage_2d<rgba16float, write>;

struct Params {
    hr_pos: vec2<i32>,
    hr_uv: vec2<f32>,
    lr_uv: vec2<f32>,
    mv: vec2<f32>,
    reprojected: vec2<f32>,
    velocity: f32,
    depth_clip: f32,
    reactive: f32,
    accumulation_mask: f32,
    existing: bool,
    new_sample: bool,
};

struct Box {
    center: vec3<f32>,
    vec: vec3<f32>,
    lo: vec3<f32>,
    hi: vec3<f32>,
    weight: f32,
};

fn clamp_uv(uv: vec2<f32>) -> vec2<f32> {
    let half = 0.5 * u.render.zw;
    return clamp(uv, half, vec2<f32>(1.0) - half);
}

fn uv_inside(uv: vec2<f32>) -> bool {
    return all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0));
}

// Lanczos 2 history (4 x 4 loads) with deringing to the inner 2 x 2.
fn sample_history(uv: vec2<f32>) -> vec4<f32> {
    let size = vec2<i32>(display_size());
    var px = uv * display_size() - 0.5;
    px = clamp(px, vec2<f32>(0.0), display_size());
    let base = vec2<i32>(floor(px));
    let f = fract(px);
    var rows: array<vec4<f32>, 4>;
    var lo = vec4<f32>(3.4e38);
    var hi = vec4<f32>(-3.4e38);
    // FSR 2's approximate Lanczos 2 (FFX_FSR2_OPTION_REPROJECT_USE_LANCZOS_TYPE 2)
    let ox = vec4<f32>(-1.0, 0.0, 1.0, 2.0) - f.x;
    let oy = vec4<f32>(-1.0, 0.0, 1.0, 2.0) - f.y;
    let wx = vec4<f32>(lanczos2_approx_sq(ox.x * ox.x), lanczos2_approx_sq(ox.y * ox.y), lanczos2_approx_sq(ox.z * ox.z), lanczos2_approx_sq(ox.w * ox.w));
    let wy = vec4<f32>(lanczos2_approx_sq(oy.x * oy.x), lanczos2_approx_sq(oy.y * oy.y), lanczos2_approx_sq(oy.z * oy.z), lanczos2_approx_sq(oy.w * oy.w));
    for (var y = 0; y < 4; y++) {
        var r = vec4<f32>(0.0);
        for (var x = 0; x < 4; x++) {
            let s = textureLoad(history, clamp_load(base + vec2<i32>(x - 1, y - 1), size), 0);
            r += s * wx[x];
            if (x == 1 || x == 2) && (y == 1 || y == 2) {
                lo = min(lo, s);
                hi = max(hi, s);
            }
        }
        rows[y] = r / (wx.x + wx.y + wx.z + wx.w);
    }
    let c = (rows[0] * wy.x + rows[1] * wy.y + rows[2] * wy.z + rows[3] * wy.w) / (wy.x + wy.y + wy.z + wy.w);
    return clamp(c, lo, hi);
}

fn shading_change_luma(uv: vec2<f32>) -> f32 {
    // ClampUv to the mip's render-covered size, then a bilinear sample
    let mip = u.luma.xy;
    let covered = render_size() / u.misc.w;
    let half = 0.5 / mip;
    let uv_c = clamp(uv * covered / mip, half, covered / mip - half);
    let l = exp(textureSampleLevel(luma_mip, linear_clamp, uv_c, 0.0).x);
    return pow(l, 1.0 / 6.0);
}

fn max_kernel_weight() -> f32 {
    let downscale = render_size().x / display_size().x;
    return min(1.99, 1.0 + (1.0 / downscale - 1.0));
}

struct Upsampled {
    color: vec4<f32>,
    alpha: f32,
    box_: Box,
};

fn upsample(params: Params, reactive: f32) -> Upsampled {
    let size = vec2<i32>(render_size());
    let downscale = render_size() / display_size();
    let dst = vec2<f32>(params.hr_pos) + 0.5;
    let src_out = dst * downscale;
    let src_in = vec2<i32>(floor(src_out));
    let unjittered = vec2<f32>(src_in) + 0.5 - jitter();
    let base_offset = unjittered - src_out;
    let kernel_reactive = max(reactive, f32(params.new_sample));
    let bias_max = max_kernel_weight() * (1.0 - kernel_reactive);
    let bias_min = max(1.0, (1.0 + bias_max) * 0.3);
    let bias_factor = max(0.0, max(0.25 * params.depth_clip, kernel_reactive));
    let kernel_bias = mix(bias_max, bias_min, bias_factor);
    let curve_bias = mix(-2.0, -3.0, clamp(params.velocity / 50.0, 0.0, 1.0));
    var acc = vec4<f32>(0.0);
    var alpha = 0.0;
    var b: Box;
    b.center = vec3<f32>(0.0);
    b.vec = vec3<f32>(0.0);
    b.lo = vec3<f32>(3.4e38);
    b.hi = vec3<f32>(-3.4e38);
    b.weight = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let o = vec2<i32>(x, y);
            let sp = src_in + o;
            let s = textureLoad(prepared, clamp_load(sp, size), 0);
            let off = base_offset + vec2<f32>(o);
            let ob = off * kernel_bias;
            let w = select(0.0, lanczos2_approx_sq(dot(ob, ob)), on_screen(sp, size));
            acc += vec4<f32>(s.rgb * w, w);
            alpha += s.a * w;
            let bw = exp(curve_bias * dot(off, off));
            b.lo = min(b.lo, s.rgb);
            b.hi = max(b.hi, s.rgb);
            b.center += s.rgb * bw;
            b.vec += s.rgb * s.rgb * bw;
            b.weight += bw;
        }
    }
    let bwt = select(1.0, b.weight, abs(b.weight) > EPSILON);
    b.center /= bwt;
    b.vec /= bwt;
    b.vec = sqrt(abs(b.vec - b.center * b.center));
    var out: Upsampled;
    if acc.w > EPSILON {
        out.color = vec4<f32>(clamp(acc.rgb / acc.w, b.lo, b.hi), acc.w * UPSAMPLE_WEIGHT_SCALE);
        out.alpha = alpha / acc.w;
    } else {
        out.color = vec4<f32>(0.0);
        out.alpha = textureLoad(prepared, clamp_load(src_in, size), 0).a;
    }
    out.box_ = b;
    return out;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let hr = vec2<i32>(id.xy);
    let dsize = vec2<i32>(display_size());
    if !on_screen(hr, dsize) {
        return;
    }
    let lock_index = u32(hr.y) * u32(display_size().x) + u32(hr.x);
    // InitParams
    var params: Params;
    params.hr_pos = hr;
    params.hr_uv = (vec2<f32>(hr) + 0.5) * u.display.zw;
    params.lr_uv = clamp_uv(params.hr_uv + jitter() * u.render.zw);
    let lr = clamp_load(vec2<i32>(params.hr_uv * render_size()), vec2<i32>(render_size()));
    params.mv = textureLoad(dilated_motion, lr, 0).xy;
    params.velocity = length(params.mv * display_size());
    params.reprojected = params.hr_uv + params.mv;
    params.existing = uv_inside(params.reprojected);
    let m = textureSampleLevel(masks, linear_clamp, params.lr_uv, 0.0);
    params.depth_clip = clamp(m.z, 0.0, 1.0);
    params.reactive = m.x;
    params.accumulation_mask = m.y;
    let reset = u.jitter.z > 0.5;
    params.new_sample = !params.existing || reset;

    // ReprojectHistoryColor / ReprojectHistoryLockStatus
    var history_color = vec3<f32>(0.0);
    var lock = vec4<f32>(0.0);
    var temporal_reactive = 0.0;
    var in_motion_last = false;
    var new_lock = false;
    var history_alpha = 0.0;
    if params.existing && !reset {
        let h = sample_history(params.reprojected);
        history_color = rgb_to_ycocg(clamp(h.rgb, vec3<f32>(0.0), vec3<f32>(FP16_MAX)));
        temporal_reactive = clamp(abs(h.w), 0.0, 1.0);
        in_motion_last = h.w < 0.0;
        new_lock = new_locks[lock_index] > 127.0 / 255.0;
        lock = textureSampleLevel(lock_status, linear_clamp, params.reprojected, 0.0);
        history_alpha = lock.z;
    }
    var this_reactive = max(params.reactive, temporal_reactive);

    // UpdateLockStatus
    let shading = shading_change_luma(params.hr_uv);
    if lock.y == 0.0 {
        lock.y = shading;
    }
    let luma_diff = 1.0 - min_divided_by_max(lock.y, shading);
    if new_lock {
        lock.y = shading;
        lock.x = select(1.0, 2.0, lock.x != 0.0);
    } else if lock.x <= 1.0 {
        lock.y = mix(lock.y, shading, 0.5);
    } else if luma_diff > 0.1 {
        lock.x = 0.0;
    }
    this_reactive = max(this_reactive, clamp((luma_diff - 0.1) * 10.0, 0.0, 1.0));
    lock.x *= 1.0 - this_reactive;
    lock.x *= clamp(1.0 - params.accumulation_mask, 0.0, 1.0);
    lock.x *= f32(params.depth_clip < 0.1);
    let lifetime_contribution = clamp(lock.x - 1.0, 0.0, 1.0);
    let shading_contribution = clamp(min_divided_by_max(lock.y, shading), 0.0, 1.0);
    var lock_contribution = clamp(clamp(lifetime_contribution * 4.0, 0.0, 1.0) * shading_contribution, 0.0, 1.0);
    // the clone's lock switch (docs/fsr2.md "Locks")
    if u.extra.y < 0.5 {
        lock_contribution = 0.0;
    }

    // ComputeUpsampledColorAndWeight
    let up = upsample(params, this_reactive);
    let b = up.box_;

    // The clone's history-disagreement reactive (docs/fsr2.md "Animated
    // textures"): the game's flip-book textures (scrolling chevron
    // barriers, blinking signs) carry no reactive mask; history whose luma
    // lies well outside the current 3 x 3's range (not jitter: converged
    // history is a mean of such samples) is taken as changed shading and
    // replaced faster, as TAA's clamp would.
    if !params.new_sample && u.extra.w > 0.0 {
        let range = max(b.hi.x - b.lo.x, 0.02);
        let outside = max(max(b.lo.x - history_color.x, history_color.x - b.hi.x), 0.0) / range;
        this_reactive = max(this_reactive, clamp((outside - 0.25) * u.extra.w, 0.0, 0.9));
    }

    // ComputeLumaInstabilityFactor
    var current_luma = b.center.x;
    if hdr() {
        current_luma = current_luma / (1.0 + max(0.0, current_luma));
    }
    current_luma = round(current_luma * 255.0) / 255.0;
    let sample_luma = max(max(params.depth_clip, params.accumulation_mask), luma_diff) < 0.1 && !params.new_sample;
    var luma_hist = vec4<f32>(0.0);
    if sample_luma {
        luma_hist = textureSampleLevel(luma_history, linear_clamp, params.reprojected, 0.0);
    }
    var instability = 0.0;
    let diff0 = current_luma - luma_hist.x;
    var dmin = abs(diff0);
    if dmin >= 1.0 / 255.0 {
        for (var i = 1; i <= 3; i++) {
            let diff1 = current_luma - luma_hist[i];
            if sign(diff0) == sign(diff1) {
                dmin = min(dmin, abs(diff1));
            }
        }
        let box_size_factor = pow(clamp(b.vec.x / 0.1, 0.0, 1.0), 6.0);
        instability = f32(dmin != abs(diff0)) * box_size_factor;
        instability = f32(instability > 1.0 / 255.0);
        instability *= 1.0 - max(params.accumulation_mask, pow(this_reactive, 1.0 / 6.0));
    }
    let new_luma_hist = vec4<f32>(current_luma, luma_hist.x, luma_hist.y, luma_hist.z);
    textureStore(out_luma_history, hr, new_luma_hist);
    instability *= f32(new_luma_hist.w != 0.0);

    // ComputeBaseAccumulationWeight
    var base = MAX_ACCUMULATION_WEIGHT * f32(params.existing) * (1.0 - this_reactive) * (1.0 - params.depth_clip);
    base = min(base, mix(base, up.color.w * 10.0, max(f32(in_motion_last), clamp(params.velocity * 10.0, 0.0, 1.0))));
    base = min(base, mix(base, up.color.w, clamp(params.velocity / 20.0, 0.0, 1.0)));
    var accumulation = vec3<f32>(base);

    var color: vec3<f32>;
    var out_alpha: f32;
    if params.new_sample {
        color = ycocg_to_rgb(up.color.rgb);
        out_alpha = up.alpha;
    } else {
        // RectifyHistory
        let downscale = render_size() / display_size();
        let scale_influence = min(u.extra.z, pow(1.0 / abs(downscale.x * downscale.y), 3.0));
        let velocity_factor = clamp(params.velocity / 20.0, 0.0, 1.0);
        let box_t = max(params.depth_clip, max(params.accumulation_mask, velocity_factor));
        let box_scale = mix(scale_influence, 1.0, box_t);
        let scaled = b.vec * box_scale;
        let bmin = max(b.lo, b.center - scaled);
        let bmax = min(b.hi, b.center + scaled);
        if any(bmin > history_color) || any(history_color > bmax) {
            let clamped = clamp(history_color, bmin, bmax);
            var contribution = max(instability, lock_contribution);
            contribution *= 1.0 - pow(params.reactive, 0.5);
            history_color = mix(clamped, history_color, clamp(contribution, 0.0, 1.0));
            let acc_min = min(accumulation, vec3<f32>(0.1));
            accumulation = mix(acc_min, accumulation, clamp(contribution, 0.0, 1.0));
        }
        // Accumulate
        accumulation = max(vec3<f32>(EPSILON), accumulation + up.color.www);
        var current = up.color.rgb;
        var hist = history_color;
        if hdr() {
            current = rgb_to_ycocg(tonemap(ycocg_to_rgb(current)));
            hist = rgb_to_ycocg(tonemap(ycocg_to_rgb(hist)));
        }
        let a = up.color.www / accumulation;
        hist = mix(hist, current, a);
        color = ycocg_to_rgb(hist);
        if hdr() {
            color = inverse_tonemap(color);
        }
        out_alpha = mix(history_alpha, up.alpha, a.x);
    }

    // FinalizeLockStatus
    let next_uv = params.hr_uv - params.mv;
    if !uv_inside(next_uv) {
        lock.x = 0.0;
    } else {
        let decrease_max = u.params.z * AVERAGE_WEIGHT_PER_FRAME;
        lock.x = max(0.0, lock.x - up.color.w / decrease_max);
    }
    textureStore(out_lock_status, hr, vec4<f32>(lock.x, lock.y, out_alpha, 0.0));

    // ComputeTemporalReactiveFactor
    var factor = min(0.99, this_reactive);
    factor = max(factor, mix(factor, 0.4, clamp(params.velocity, 0.0, 1.0)));
    factor = max(factor * factor, max(params.depth_clip * 0.1, params.reactive));
    if params.new_sample {
        factor = 1.0;
    }
    if clamp(params.velocity * 10.0, 0.0, 1.0) >= 1.0 {
        factor = max(EPSILON, factor) * -1.0;
    }
    color = max(color, vec3<f32>(0.0));
    textureStore(out_history, hr, vec4<f32>(color, factor));
    var shown = vec4<f32>(color, out_alpha);
    let dbg = u.misc.x;
    if dbg > 0.5 && dbg < 1.5 {
        shown = vec4<f32>(abs(params.mv * display_size()) * 0.5, select(0.0, 0.3, params.new_sample), 1.0);
    } else if (dbg > 1.5 && dbg < 2.5) || dbg > 6.5 {
        shown = vec4<f32>(params.reactive, params.accumulation_mask, params.depth_clip, 1.0);
    } else if dbg > 2.5 && dbg < 3.5 {
        shown = vec4<f32>(lock_contribution, clamp(lock.x * 0.5, 0.0, 1.0), instability, 1.0);
    }
    textureStore(out_color, hr, shown);
    new_locks[lock_index] = 0.0;
}

