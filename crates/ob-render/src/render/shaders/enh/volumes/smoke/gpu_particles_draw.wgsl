// ---- draw ----

struct Frame {
    view_proj: mat4x4<f32>,
    // eye, pixels per metre (view +0x0C)
    eye: vec4<f32>,
    // right, near
    right: vec4<f32>,
    // up, far
    up: vec4<f32>,
    // forward, reversed (0/1)
    forward: vec4<f32>,
    // lamps, spots, points, pool
    counts: vec4<u32>,
    // time, detail, light gain, linger
    params: vec4<f32>,
    // per lamp: camera-relative position + outer radius, colour
    lamps: array<vec4<f32>, 32>,
    // per spot: relative eye + range, direction + cos outer, colour + cos inner
    spots: array<vec4<f32>, 12>,
    // per point: relative position + radius, colour
    points: array<vec4<f32>, 32>,
    // The depth texel under a fragment = clip.xy · xy + zw (the
    // half-size draw's scale and the viewport's origin; (1, 1, 0, 0) at
    // full size)
    depth_map: vec4<f32>,
}

@group(0) @binding(0) var<storage, read> puffs: array<Puff>;
@group(0) @binding(1) var<storage, read> ddescs: array<Desc>;
@group(0) @binding(2) var<storage, read> colors: array<array<vec4<f32>, 4>>;
@group(0) @binding(3) var<uniform> f: Frame;
@group(0) @binding(4) var depth_tex: texture_depth_2d;
@group(0) @binding(5) var smoke_tex: texture_2d_array<f32>;
@group(0) @binding(6) var smoke_samp: sampler;

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) cell: vec4<f32>,
    @location(2) color: vec4<f32>,
    // camera-relative position, view depth
    @location(3) rel: vec4<f32>,
    // size (m), age fraction, seed, layer
    @location(4) misc: vec4<f32>,
    // the billboard's right and up (turned)
    @location(5) ax: vec3<f32>,
    @location(6) ay: vec3<f32>,
    // the light at the puff (lamps, headlights, tail lights), and where
    // most of it comes from (a weighted direction)
    @location(7) light: vec3<f32>,
    @location(8) ldir: vec3<f32>,
}

const CORNERS = array<vec2<f32>, 6>(vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0));

// The light at a camera-relative point (once per puff corner, not per
// pixel: the lights are smooth across a puff).
fn light_at(p: vec3<f32>, light: ptr<function, vec3<f32>>, ldir: ptr<function, vec3<f32>>) {
    for (var i = 0u; i < f.counts.x; i++) {
        let l = f.lamps[i * 2u];
        let to = l.xyz - p;
        let w = clamp(1.0 - dot(to, to) / (l.w * l.w), 0.0, 1.0);
        let c = f.lamps[i * 2u + 1u].rgb * w * w;
        *light += c;
        *ldir += normalize(to) * dot(c, vec3(0.33));
    }
    let view_dir = normalize(p);
    for (var i = 0u; i < f.counts.y; i++) {
        let e = f.spots[i * 3u];
        let dir = f.spots[i * 3u + 1u];
        let col = f.spots[i * 3u + 2u];
        let to = p - e.xyz;
        let dist = length(to);
        let cone = smoothstep(dir.w, col.w, dot(to / max(dist, 1e-3), dir.xyz));
        let att = clamp(1.0 - dist / e.w, 0.0, 1.0);
        // Forward scattering: brighter looking into the beam.
        let phase = 0.6 + 0.6 * max(dot(-view_dir, dir.xyz), 0.0);
        let c = col.rgb * cone * att * att * phase * 0.8;
        *light += c;
        *ldir += -dir.xyz * dot(c, vec3(0.33));
    }
    for (var i = 0u; i < f.counts.z; i++) {
        let l = f.points[i * 2u];
        let to = l.xyz - p;
        let w = clamp(1.0 - dot(to, to) / (l.w * l.w), 0.0, 1.0);
        let c = f.points[i * 2u + 1u].rgb * w * w * 0.8;
        *light += c;
        *ldir += normalize(to) * dot(c, vec3(0.33));
    }
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> Out {
    var o: Out;
    let puff = puffs[ii];
    o.clip = vec4(0.0, 0.0, -2.0, 1.0);
    if (puff.p1.w <= 0.0) {
        return o;
    }
    let d = ddescs[puff.ids.x];
    let t = clamp(puff.p0.w / puff.p1.w, 0.0, 1.0);
    // The size over the native life (it spreads as the native particle
    // does), growing on slowly while it lingers; the colour and opacity
    // over the stretched life.
    let tn = t * f.params.w;
    var size = cubic(d.size, min(tn, 1.0)).x * puff.p3.x * (1.0 + 0.35 * max(tn - 1.0, 0.0));
    var col = clamp(cubic(colors[puff.ids.y], t), vec4(0.0), vec4(255.0)) / 255.0;
    // Fade in over the first 60 ms and out over the last 25 % of the life.
    col.a *= smoothstep(0.0, 0.06, puff.p0.w) * (1.0 - smoothstep(0.75, 1.0, t));
    let rel = puff.p0.xyz - f.eye.xyz;
    let depth = dot(rel, f.forward.xyz);
    if (size <= 0.0 || depth <= 0.05 || col.a <= 0.002) {
        return o;
    }
    // The native on-screen bound (0x48BDF0: size / depth · view +0x0C).
    let bound = d.params.z;
    if (size / depth * f.eye.w > bound) {
        size = depth / f.eye.w * bound;
    }
    let a = puff.p3.y + puff.p3.z * puff.p0.w;
    let cs = cos(a);
    let sn = sin(a);
    let ax = f.right.xyz * cs + f.up.xyz * sn;
    let ay = -f.right.xyz * sn + f.up.xyz * cs;
    let c = CORNERS[vi];
    let half = size * 0.5 * d.params2.xy;
    let p = rel + ax * c.x * half.x + ay * c.y * half.y;
    o.clip = f.view_proj * vec4(p, 1.0);
    o.uv = c * 0.5 + 0.5;
    // One cell of the flip-book, picked by the seed.
    let cells = max(d.params2.z * d.params2.w, 1.0);
    let frame = f32(puff.ids.z % u32(cells));
    let cw = 1.0 / max(d.params2.z, 1.0);
    let rh = 1.0 / max(d.params2.w, 1.0);
    o.cell = vec4((frame % max(d.params2.z, 1.0)) * cw, floor(frame / max(d.params2.z, 1.0)) * rh, cw, rh);
    o.color = col;
    o.rel = vec4(p, dot(p, f.forward.xyz));
    o.misc = vec4(size, t, f32(puff.ids.z & 1023u), d.params.w);
    o.ax = ax;
    o.ay = ay;
    var light = vec3(0.0);
    var ldir = vec3(0.0);
    light_at(p, &light, &ldir);
    o.light = light;
    o.ldir = ldir;
    return o;
}

fn linear_depth(d: f32) -> f32 {
    let n = f.right.w;
    let fa = f.up.w;
    if (f.forward.w > 0.5) {
        return n * fa / (n + d * (fa - n));
    }
    return n * fa / (fa - d * (fa - n));
}

fn scene_depth(px: vec2<f32>) -> f32 {
    let dims = vec2<i32>(textureDimensions(depth_tex));
    let c = clamp(vec2<i32>(px), vec2(0), dims - 1);
    return textureLoad(depth_tex, c, 0);
}

// One fragment of a puff: premultiplied colour and its alpha.
fn shade(in: Out) -> vec4<f32> {
    let tex = textureSample(smoke_tex, smoke_samp, in.cell.xy + in.uv * in.cell.zw, i32(in.misc.w));
    // The native shading: 2 · tex · colour, alpha tex.a · colour.a.
    var a = tex.a * in.color.a;
    if (a <= 0.003) {
        discard;
    }
    // Soft against the scene (no hard edges on the road and the cars),
    // and away from the eye's near plane.
    let z = in.rel.w;
    let sz = linear_depth(scene_depth(in.clip.xy * f.depth_map.xy + f.depth_map.zw));
    a *= clamp((sz - z) / clamp(0.25 * in.misc.x, 0.15, 0.6), 0.0, 1.0);
    a *= clamp((z - 0.3) / 1.2, 0.0, 1.0);
    if (a <= 0.003) {
        discard;
    }
    let thickness = tex.a * in.color.a;
    // Procedural detail: noise that erodes the puff as it ages.
    let detail = f.params.y;
    if (detail > 0.0) {
        let q = vec3(in.uv * 2.6 + in.misc.z * 0.37, in.misc.y * 1.4 + f.params.x * 0.15);
        let n = vnoise(q) * 0.65 + vnoise(q * 2.07 + 11.0) * 0.35;
        let erosion = 0.18 + 0.5 * in.misc.y * in.misc.y;
        let m = smoothstep(erosion, erosion + 0.45, n);
        a *= mix(1.0, m * 1.5, detail);
    }
    var rgb = 2.0 * tex.rgb * in.color.rgb;
    // A sphere-like normal across the billboard for the light.
    let c = in.uv * 2.0 - 1.0;
    let nz = sqrt(max(1.0 - dot(c, c), 0.0));
    let normal = in.ax * c.x + in.ay * c.y - f.forward.xyz * nz;
    // Top lit a little more than the bottom; a thick core darker.
    rgb *= (0.86 + 0.14 * normal.z) * (1.0 - 0.18 * thickness);
    let albedo = clamp(rgb, vec3(0.0), vec3(1.0));
    let lw = length(in.ldir);
    let wrap = select(1.0, 0.55 + 0.45 * dot(normal, in.ldir / max(lw, 1e-4)), lw > 1e-4);
    // Light through the puff: less into a thick core.
    rgb += albedo * in.light * wrap * (1.0 - 0.45 * thickness) * f.params.z;
    a = clamp(a, 0.0, 1.0);
    return vec4(rgb * a, a);
}

@fragment
fn fs_main(in: Out) -> @location(0) vec4<f32> {
    return shade(in);
}

// The half-size draw (gpu_particles.rs `HalfSize`): the colour over
// and what is left of the scene behind (target 0: rgb, alpha the
// transmittance), and the native alpha blend's own sum (target 1: r =
// a · a + (1 − a) · r), composited by smoke_half.wgsl.
struct HalfOut {
    @location(0) colour: vec4<f32>,
    @location(1) alpha: vec4<f32>,
}

@fragment
fn fs_half(in: Out) -> HalfOut {
    let c = shade(in);
    return HalfOut(c, vec4(c.a * c.a, 0.0, 0.0, c.a));
}
