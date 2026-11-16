// ---- The ray-traced tier's quarter-rate resolve (ssr.rs, docs/rt-reflections.md) ----
// Appended to ssr.wgsl for this pass only. The trace ran at half size, each
// of its pixels standing for one pixel of its 2 × 2 (p.jitter.xy, a different
// one each frame); this pass rebuilds the full-size trace for the filter:
// per pixel the half trace's samples around it on the same surface (kind,
// depth; weighted by their confidence, as the filter takes them), the exact one where this frame traced the pixel itself, blended
// with the last frame's result where the surface was (the road: the point
// reprojected by the camera's motion; a car: the same pixel, as the chase
// camera keeps it), held to the current samples' range (no trails), and
// dropped where the surface was not there (depth, kind) or after a cut.

@group(1) @binding(0) var half_tex: texture_2d<f32>;
@group(1) @binding(1) var hist_tex: texture_2d<f32>;
// The history's surface: view depth, negative on a car (r32float).
@group(1) @binding(2) var hist_depth: texture_2d<f32>;

struct Resolved {
    @location(0) colour: vec4<f32>,
    @location(1) depth: vec4<f32>,
}

// The current frame's share where it traced the pixel itself, and elsewhere.
const RESOLVE_OWN: f32 = 0.5;
const RESOLVE_REST: f32 = 0.2;

@fragment
fn fs_resolve(@builtin(position) frag: vec4<f32>) -> Resolved {
    var out: Resolved;
    let c = vec2<i32>(frag.xy);
    let k = textureLoad(kinds_tex, c, 0).xy;
    if k.y <= 0.0 {
        out.colour = vec4<f32>(0.0);
        out.depth = vec4<f32>(0.0);
        return out;
    }
    let car = is_car(k.x);
    let jit = vec2<i32>(p.jitter.xy);
    let hsize = vec2<i32>(textureDimensions(half_tex));
    let full = vec2<i32>(p.size.zw);
    let h = c / 2;
    // The half trace's samples of this surface around the pixel.
    var cur = vec4<f32>(0.0);
    var wsum = 0.0;
    var lo = vec4<f32>(1.0e9);
    var hi = vec4<f32>(-1.0e9);
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            let hq = clamp(h + vec2<i32>(dx, dy), vec2<i32>(0), hsize - 1);
            let sp = min(hq * 2 + jit, full - 1);
            let ks = textureLoad(kinds_tex, sp, 0).xy;
            if ks.y <= 0.0 || is_car(ks.x) != car || abs(ks.y - k.y) > 0.05 * k.y {
                continue;
            }
            let s = pre(textureLoad(half_tex, hq, 0));
            let d = vec2<f32>(sp - c);
            let w = 1.0 / (1.0 + dot(d, d));
            cur += s * w;
            wsum += w;
            lo = min(lo, s);
            hi = max(hi, s);
        }
    }
    let own = all(c - h * 2 == jit);
    if own || wsum <= 0.0 {
        cur = pre(textureLoad(half_tex, min(h, hsize - 1), 0));
        if wsum <= 0.0 {
            lo = cur;
            hi = cur;
        }
    } else {
        cur = cur / wsum;
    }
    out.depth = vec4<f32>(select(k.y, -k.y, car), 0.0, 0.0, 0.0);
    // Where the surface was last frame.
    var valid = p.jitter.w > 0.5;
    var pc = frag.xy;
    var want = k.y;
    if !car {
        let uv = frag.xy / p.size.zw;
        let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
        let o = vec3<f32>(ndc.x / p.proj.x * k.y, ndc.y / p.proj.y * k.y, k.y);
        let q = p.right.xyz * o.x + p.up.xyz * o.y + p.forward.xyz * o.z - p.prev_eye.xyz;
        let pv = vec3<f32>(dot(q, p.prev_right.xyz), dot(q, p.prev_up.xyz), dot(q, p.prev_forward.xyz));
        if pv.z <= 0.05 {
            valid = false;
        }
        let pn = vec2<f32>(pv.x * p.prev_proj.x, pv.y * p.prev_proj.y) / max(pv.z, 0.05);
        pc = vec2<f32>(pn.x * 0.5 + 0.5, 0.5 - pn.y * 0.5) * p.size.zw;
        want = pv.z;
    }
    if any(pc < vec2<f32>(0.0)) || any(pc >= p.size.zw) {
        valid = false;
    }
    if valid {
        let hd = textureLoad(hist_depth, clamp(vec2<i32>(pc), vec2<i32>(0), full - 1), 0).x;
        if (hd < 0.0) != car || abs(abs(hd) - want) > 0.05 * want {
            valid = false;
        }
    }
    if !valid {
        out.colour = unpre(cur);
        return out;
    }
    let spread = max(hi - lo, vec4<f32>(0.0)) * 0.1;
    let hist = clamp(pre(textureSampleLevel(hist_tex, samp, pc / p.size.zw, 0.0)), lo - spread, hi + spread);
    out.colour = unpre(mix(hist, cur, select(RESOLVE_REST, RESOLVE_OWN, own)));
    return out;
}

// The trace's (colour, confidence) weighted by its confidence (as the filter
// takes its taps), and back.
fn pre(c: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(c.rgb * c.a, c.a);
}

fn unpre(c: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(c.rgb / max(c.a, 1e-4), c.a);
}
