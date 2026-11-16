// The temporal pass (docs/temporal.md; a clone deviation, opt-in):
// TAA and the temporal upscaler in one resolve. Per output pixel: the
// current frame reconstructed from the jittered input's 3x3 samples
// (Blackman-Harris weights by each sample's true position), the history
// reprojected by the motion of the closest surface in that 3x3 (camera
// motion from the depth; per-car rigid motion for points inside a drawn
// car's box, its wheels reactive), sampled Catmull-Rom, clipped to the
// neighbourhood's colour box (YCoCg, variance clipping), blended in a
// luma-weighted space. Glow sprites, streaks, particles, rain and the HUD
// are drawn after this pass (they write no depth and would ghost).

struct Car {
    // camera-relative world -> car space (this frame's drawn pose)
    inv: mat4x4<f32>,
    // camera-relative point on the car now -> where it was last frame
    // (camera-relative to this frame's eye)
    motion: mat4x4<f32>,
    // car-space box; lo.w = 1 for a car with no last pose (no history)
    lo: vec4<f32>,
    hi: vec4<f32>,
    // wheel centres (xyz) and radii (w), car space; half widths
    wheel: array<vec4<f32>, 4>,
    wheel_hw: vec4<f32>,
    // camera-relative bounding sphere (xyz centre, w radius²)
    sphere: vec4<f32>,
};

struct Uniforms {
    // jittered clip (this frame) -> camera-relative world
    clip_to_rel: mat4x4<f32>,
    // camera-relative world (this frame's eye) -> last frame's clip
    prev_rel_to_clip: mat4x4<f32>,
    // input size, 1 / input size
    input: vec4<f32>,
    // output size, 1 / output size
    output: vec4<f32>,
    // jitter (input px, x right y down), history valid (0/1), reversed depth (0/1)
    jitter: vec4<f32>,
    // base blend, car count, output px per input px, clip gamma
    params: vec4<f32>,
    // x: debug view (1 motion, 2 car / wheel flags)
    debug: vec4<f32>,
    // x: the frame's span in world ticks (1 at a steady 60 Hz)
    rate: vec4<f32>,
};

const CROSS = array<vec2<i32>, 5>(vec2<i32>(0, 0), vec2<i32>(-1, 0), vec2<i32>(1, 0), vec2<i32>(0, -1), vec2<i32>(0, 1));

@group(0) @binding(0) var input_color: texture_2d<f32>;
@group(0) @binding(1) var input_depth: texture_depth_2d;
@group(0) @binding(2) var history: texture_2d<f32>;
@group(0) @binding(3) var linear_clamp: sampler;
@group(0) @binding(4) var<uniform> u: Uniforms;
@group(0) @binding(5) var<uniform> cars: array<Car, 32>;

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

fn rgb_to_ycocg(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        0.25 * c.r + 0.5 * c.g + 0.25 * c.b,
        0.5 * c.r - 0.5 * c.b,
        -0.25 * c.r + 0.5 * c.g - 0.25 * c.b);
}

fn ycocg_to_rgb(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(c.x + c.y - c.z, c.x + c.z, c.x - c.y - c.z);
}

fn load_input(p: vec2<i32>) -> vec4<f32> {
    let size = vec2<i32>(u.input.xy);
    return textureLoad(input_color, clamp(p, vec2<i32>(0), size - 1), 0);
}

fn load_depth(p: vec2<i32>) -> f32 {
    let size = vec2<i32>(u.input.xy);
    return textureLoad(input_depth, clamp(p, vec2<i32>(0), size - 1), 0);
}

// Catmull-Rom history in five bilinear taps (the corners dropped).
fn sample_history(uv: vec2<f32>) -> vec4<f32> {
    let size = u.output.xy;
    let pos = uv * size;
    let c = floor(pos - 0.5) + 0.5;
    let f = pos - c;
    let w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    let w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    let w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    let w3 = f * f * (-0.5 + 0.5 * f);
    let w12 = w1 + w2;
    let t0 = (c - 1.0) * u.output.zw;
    let t3 = (c + 2.0) * u.output.zw;
    let t12 = (c + w2 / w12) * u.output.zw;
    var r = vec4<f32>(0.0);
    r += textureSampleLevel(history, linear_clamp, vec2<f32>(t12.x, t0.y), 0.0) * (w12.x * w0.y);
    r += textureSampleLevel(history, linear_clamp, vec2<f32>(t0.x, t12.y), 0.0) * (w0.x * w12.y);
    r += textureSampleLevel(history, linear_clamp, vec2<f32>(t12.x, t12.y), 0.0) * (w12.x * w12.y);
    r += textureSampleLevel(history, linear_clamp, vec2<f32>(t3.x, t12.y), 0.0) * (w3.x * w12.y);
    r += textureSampleLevel(history, linear_clamp, vec2<f32>(t12.x, t3.y), 0.0) * (w12.x * w3.y);
    let wsum = w12.x * w0.y + w0.x * w12.y + w12.x * w12.y + w3.x * w12.y + w12.x * w3.y;
    return max(r / wsum, vec4<f32>(0.0));
}

// Where a camera-relative point `rel` (on screen now) was last frame, as
// uv, and how it moves: x = 1 when a car's motion was used, y = 1 inside
// a wheel (reactive), z = 1 for a car without a last pose.
struct Motion {
    prev_uv: vec2<f32>,
    flags: vec3<f32>,
};

fn motion_of(rel: vec3<f32>) -> Motion {
    var m: Motion;
    m.flags = vec3<f32>(0.0);
    var p = vec4<f32>(rel, 1.0);
    let count = i32(u.params.y);
    for (var i = 0; i < count; i++) {
        // fields read one by one: a whole Car copied per car spills
        let sphere = cars[i].sphere;
        let to = rel - sphere.xyz;
        if dot(to, to) > sphere.w {
            continue;
        }
        let local = (cars[i].inv * vec4<f32>(rel, 1.0)).xyz;
        let lo = cars[i].lo;
        let hi = cars[i].hi;
        if all(local >= lo.xyz) && all(local <= hi.xyz) {
            p = cars[i].motion * vec4<f32>(rel, 1.0);
            m.flags.x = 1.0;
            m.flags.z = lo.w;
            let hw = cars[i].wheel_hw;
            for (var w = 0; w < 4; w++) {
                let wheel = cars[i].wheel[w];
                let d = local - wheel.xyz;
                if abs(d.y) < hw[w] && dot(d.xz, d.xz) < wheel.w * wheel.w {
                    m.flags.y = 1.0;
                }
            }
            break;
        }
    }
    let clip = u.prev_rel_to_clip * p;
    let ndc = clip.xy / max(clip.w, 1e-6);
    m.prev_uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if clip.w <= 0.0 {
        m.prev_uv = vec2<f32>(-1.0);
    }
    return m;
}

// Camera-relative position of input pixel `p` with raw depth `d`.
fn position_of(p: vec2<i32>, d: f32) -> vec3<f32> {
    let uv = (vec2<f32>(p) + 0.5) * u.input.zw;
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
    let h = u.clip_to_rel * ndc;
    return h.xyz / h.w;
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

// Bright pixels count less (Karis): a lamp's flicker does not dominate.
fn tonemap(c: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(c.rgb / (1.0 + luma(c.rgb)), c.a);
}

fn untonemap(c: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(c.rgb / max(1.0 - luma(c.rgb), 1e-4), c.a);
}

struct Two {
    @location(0) history: vec4<f32>,
    @location(1) scene: vec4<f32>,
};

// Upscaling: the history and the scene (another texture) in one pass.
@fragment
fn fs_resolve_mrt(in: VsOut) -> Two {
    let c = resolve(in.position.xy);
    var o: Two;
    o.history = c;
    o.scene = c;
    return o;
}

@fragment
fn fs_resolve(in: VsOut) -> @location(0) vec4<f32> {
    return resolve(in.position.xy);
}

fn resolve(position: vec2<f32>) -> vec4<f32> {
    let uv = position * u.output.zw;
    // This output pixel in input pixels (unjittered content), and where
    // that content lies in the jittered input.
    let at = uv * u.input.xy;
    let jit = u.jitter.xy * u.debug.w;
    let jittered = at + jit;
    let centre = vec2<i32>(floor(jittered));
    let scale = u.params.z;
    let reversed = u.jitter.w > 0.5;

    var sum = vec4<f32>(0.0);
    var wsum = 0.0;
    var m1 = vec4<f32>(0.0);
    var m2 = vec4<f32>(0.0);
    var lo = vec4<f32>(1e9);
    var hi = vec4<f32>(-1e9);
    var coverage = 0.0;
    var closest = centre;
    var closest_d = select(2.0, -1.0, reversed);
    // The cross of five input samples (the centre and its four sides):
    // colour statistics, the closest depth and the reconstruction.
    for (var k = 0; k < 5; k++) {
        let o = CROSS[k];
        {
            let x = o.x;
            let y = o.y;
            let p = centre + o;
            let c = tonemap(load_input(p));
            let ycc = vec4<f32>(rgb_to_ycocg(c.rgb), c.a);
            // the sample's content position, input px
            let s = vec2<f32>(p) + 0.5 - jit;
            let d = s - at;
            // the distance in output pixels: at 1x only the pixel's own
            // sample counts (the jitter's accumulation is the filter)
            let dout = d * scale;
            let w = select(exp(-2.29 * dot(dout, dout)), select(0.0, 1.0, x == 0 && y == 0), scale < 1.001);
            sum += c * w;
            wsum += w;
            m1 += ycc;
            m2 += ycc * ycc;
            lo = min(lo, ycc);
            hi = max(hi, ycc);
            coverage = max(coverage, exp(-2.29 * dot(dout, dout)));
            let z = load_depth(p);
            if (reversed && z > closest_d) || (!reversed && z < closest_d) {
                closest_d = z;
                closest = p;
            }
        }
    }
    let current = sum / max(wsum, 1e-6);
    let mean = m1 / 5.0;
    let sigma = sqrt(max(m2 / 5.0 - mean * mean, vec4<f32>(0.0)));

    // Reprojection by the closest surface's motion (dilated: edges follow
    // the object in front).
    let rel = position_of(closest, closest_d);
    let m = motion_of(rel);
    let closest_uv = (vec2<f32>(closest) + 0.5 - jit) * u.input.zw;
    var prev_uv = uv + (m.prev_uv - closest_uv);
    if u.debug.x > 5.5 && u.debug.x < 6.5 {
        prev_uv = uv;
    }
    let velocity_px = length((prev_uv - uv) * u.output.xy);
    if u.debug.x > 0.5 && u.debug.x < 4.5 {
        let v = (prev_uv - uv) * u.output.xy;
        if u.debug.x > 3.5 {
            return vec4<f32>(length(rel) / 100.0, fract(length(rel)), closest_d, 1.0);
        }
        if u.debug.x > 1.5 {
            return vec4<f32>(m.flags, 1.0) * 0.5 + vec4<f32>(current.rgb * 0.5, 1.0);
        }
        return vec4<f32>(abs(v.x) * 0.5, abs(v.y) * 0.5, select(0.0, 1.0, u.jitter.z > 0.5) * 0.3, 1.0);
    }

    var alpha = u.params.x * (0.5 + coverage);
    var gamma = u.params.w;
    if m.flags.y > 0.5 {
        // spinning wheels: their texture moves where the rigid motion does not
        alpha = max(alpha, 0.35);
        gamma = 0.75;
    }
    // fast motion keeps less history (Catmull-Rom still softens a little)
    // The motion per world tick, and the blend over the frame's
    // span (the history's time constant the same at every display rate;
    // a span of 1 is the per-frame blend exactly)
    let span = max(u.rate.x, 1e-3);
    alpha = min(alpha + clamp(velocity_px / span * u.debug.y, 0.0, u.debug.z), 1.0);
    if span != 1.0 {
        alpha = 1.0 - pow(1.0 - alpha, span);
    }
    if u.debug.x > 4.5 && u.debug.x < 5.5 {
        alpha = 1.0;
    }
    let outside = any(prev_uv < vec2<f32>(0.0)) || any(prev_uv > vec2<f32>(1.0));
    if u.jitter.z < 0.5 || outside || m.flags.z > 0.5 {
        return untonemap(current);
    }
    var h = tonemap(sample_history(prev_uv));
    var hy = vec4<f32>(rgb_to_ycocg(h.rgb), h.a);
    // clip toward the mean, into the box of mean ± gamma·sigma within the
    // neighbourhood's min / max
    let bmin = max(lo, mean - gamma * sigma);
    let bmax = min(hi, mean + gamma * sigma);
    let ccentre = 0.5 * (bmax + bmin);
    let extent = max(0.5 * (bmax - bmin), vec4<f32>(1e-5));
    let offset = hy - ccentre;
    let units = abs(offset / extent);
    let most = max(max(units.x, units.y), max(units.z, units.w));
    if most > 1.0 {
        hy = ccentre + offset / most;
    }
    h = vec4<f32>(ycocg_to_rgb(hy.xyz), hy.w);
    let result = mix(h, current, alpha);
    return untonemap(max(result, vec4<f32>(0.0)));
}

// The history to the scene's texture (its own format).
@fragment
fn fs_copy(in: VsOut) -> @location(0) vec4<f32> {
    return textureLoad(history, vec2<i32>(in.position.xy), 0);
}

// The input's depth at the output's size (Upscaling): the late sprites'
// depth test, at the unjittered content's place.
@fragment
fn fs_depth(in: VsOut) -> @builtin(frag_depth) f32 {
    let uv = in.position.xy * u.output.zw;
    let p = vec2<i32>(floor(uv * u.input.xy + u.jitter.xy));
    return load_depth(p);
}
