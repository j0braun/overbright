// ---- The ray-traced tier's quarter-rate resolve (ssr.rs, docs/notes/graphics/reflections.md "Ray-traced reflections (experimental)") ----
// Appended to ssr.wgsl for this pass only. The trace ran at half size, each
// of its pixels standing for one pixel of its 2 × 2 (p.jitter.xy, a different
// one each frame); this pass rebuilds the full-size trace for the filter:
// per pixel the half trace's samples around it on the same surface (kind,
// depth; weighted by their confidence, as the filter takes them), the exact one where this frame traced the pixel itself, blended
// with the last frame's result where the surface was (the road: the point
// reprojected by the camera's motion; a car: the same pixel, as the chase
// camera keeps it), held to the current samples' range (no trails), and
// dropped where the surface was not there (depth, kind) or after a cut.
// A mirror-like wet road (the primary reflection's rays, their path to the
// hit in dist_tex) takes its history where the reflected point was: the
// reflection seen in a mirror lies behind it, along the eye's ray, as far as
// the eye's path to the surface plus the ray's path to the hit; a glossy one
// blends that and the surface's own reprojection by its roughness.

@group(1) @binding(0) var half_tex: texture_2d<f32>;
@group(1) @binding(1) var hist_tex: texture_2d<f32>;
// The history's surface: view depth, negative on a car (r32float).
@group(1) @binding(2) var hist_depth: texture_2d<f32>;
// The half trace's rays' paths to their hits (m; -1 none), written by the
// list's compute pass for the pixels it traced this frame.
@group(1) @binding(3) var dist_tex: texture_2d<f32>;

struct Resolved {
    @location(0) colour: vec4<f32>,
    @location(1) depth: vec4<f32>,
}

// The current frame's share where it traced the pixel itself, and elsewhere.
const RESOLVE_OWN: f32 = 0.5;
const RESOLVE_REST: f32 = 0.2;
// The roughness up to which a wet road's history follows the reflected
// point alone, and from which the surface alone (a blend between).
const VIRTUAL_ROUGH: vec2<f32> = vec2<f32>(0.15, 0.45);

// The trace pixel (full size) where the last frame saw the view-space point
// `o` (this frame's view), and that point's depth then (z; 0: behind the eye).
fn resolve_prev(o: vec3<f32>) -> vec3<f32> {
    let q = p.right.xyz * o.x + p.up.xyz * o.y + p.forward.xyz * o.z - p.prev_eye.xyz;
    let pv = vec3<f32>(dot(q, p.prev_right.xyz), dot(q, p.prev_up.xyz), dot(q, p.prev_forward.xyz));
    if pv.z <= 0.05 {
        return vec3<f32>(-1.0, -1.0, 0.0);
    }
    let pn = vec2<f32>(pv.x * p.prev_proj.x, pv.y * p.prev_proj.y) / pv.z;
    return vec3<f32>(vec2<f32>(pn.x * 0.5 + 0.5, 0.5 - pn.y * 0.5) * p.size.zw, pv.z);
}

// The history at trace pixel `pc` when it holds a road (a car: `car`) of
// depth `want` (within `tol` of it), confidence-weighted; a −1: none.
fn resolve_hist(pc: vec2<f32>, want: f32, tol: f32, car: bool) -> vec4<f32> {
    if any(pc < vec2<f32>(0.0)) || any(pc >= p.size.zw) {
        return vec4<f32>(0.0, 0.0, 0.0, -1.0);
    }
    let hd = textureLoad(hist_depth, clamp(vec2<i32>(pc), vec2<i32>(0), vec2<i32>(p.size.zw) - 1), 0).x;
    if (hd < 0.0) != car || hd == 0.0 || abs(abs(hd) - want) > tol * want {
        return vec4<f32>(0.0, 0.0, 0.0, -1.0);
    }
    return pre(textureSampleLevel(hist_tex, samp, pc / p.size.zw, 0.0));
}

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
    let uv = frag.xy / p.size.zw;
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let o = vec3<f32>(ndc.x / p.proj.x * k.y, ndc.y / p.proj.y * k.y, k.y);
    var hist = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    if p.jitter.w > 0.5 {
        if car {
            hist = resolve_hist(frag.xy, k.y, 0.05, true);
        } else {
            let ps = resolve_prev(o);
            if ps.z > 0.0 {
                hist = resolve_hist(ps.xy, ps.z, 0.05, false);
            }
            // A wet road traced by the primary reflection's rays: the
            // reflected point's history (its path from this half sample: on
            // the same surface, the 2 × 2's).
            let rough = fract(k.x);
            let ks = textureLoad(kinds_tex, min(h * 2 + jit, full - 1), 0).xy;
            let dist = textureLoad(dist_tex, min(h, hsize - 1), 0).x;
            let to_virtual = 1.0 - smoothstep(VIRTUAL_ROUGH.x, VIRTUAL_ROUGH.y, rough);
            if rt_is_primary_road() && to_virtual > 0.0 && dist > 0.0 && !is_car(ks.x) && abs(ks.y - k.y) <= 0.05 * k.y {
                let pv = resolve_prev(o * (1.0 + dist / length(o)));
                // On the road's history (its depth there within a quarter
                // of this pixel's reprojected one: the same stretch of road).
                let hv = resolve_hist(pv.xy, max(ps.z, 0.05), 0.25, false);
                if pv.z > 0.0 && hv.a >= 0.0 {
                    hist = select(hv, mix(hist, hv, to_virtual), hist.a >= 0.0);
                }
            }
        }
    }
    if hist.a < 0.0 {
        out.colour = unpre(cur);
        return out;
    }
    let spread = max(hi - lo, vec4<f32>(0.0)) * 0.1;
    hist = clamp(hist, lo - spread, hi + spread);
    out.colour = unpre(mix(hist, cur, select(RESOLVE_REST, RESOLVE_OWN, own)));
    return out;
}

// The primary reflection's wet road (rt.wgsl `rt_is_primary` for a road).
fn rt_is_primary_road() -> bool {
    return p.prev_proj.z > 0.5 && p.right.w < 0.5 && p.fog_color.w >= 0.5;
}

// The trace's (colour, confidence) weighted by its confidence (as the filter
// takes its taps), and back.
fn pre(c: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(c.rgb * c.a, c.a);
}

fn unpre(c: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(c.rgb / max(c.a, 1e-4), c.a);
}
