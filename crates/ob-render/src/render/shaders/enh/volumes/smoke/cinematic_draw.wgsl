// ---- draw ----

struct Frame {
    view_proj: mat4x4<f32>,
    // eye, pixels per metre
    eye: vec4<f32>,
    // right, near
    right: vec4<f32>,
    // up, far
    up: vec4<f32>,
    // forward, reversed (0/1)
    forward: vec4<f32>,
    // lamps, spots, points, modern lighting (0/1)
    counts: vec4<u32>,
    // time, detail, light gain, linger
    params: vec4<f32>,
    // low-res size, full-res size
    sizes: vec4<f32>,
    // divisor, ambient level, steam tint, 0
    params2: vec4<f32>,
    lamps: array<vec4<f32>, 32>,
    spots: array<vec4<f32>, 12>,
    points: array<vec4<f32>, 32>,
}

@group(0) @binding(0) var<storage, read> puffs: array<Puff>;
@group(0) @binding(1) var<storage, read> ddescs: array<Desc>;
@group(0) @binding(2) var<storage, read> colors: array<array<vec4<f32>, 4>>;
@group(0) @binding(3) var<uniform> f: Frame;
@group(0) @binding(4) var low_depth: texture_2d<f32>;
@group(0) @binding(5) var smoke_tex: texture_2d_array<f32>;
@group(0) @binding(6) var smoke_samp: sampler;
@group(0) @binding(7) var vnoise_tex: texture_3d<f32>;
@group(0) @binding(8) var vnoise_samp: sampler;

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) cell: vec4<f32>,
    @location(2) color: vec4<f32>,
    // view depth, size (m), age fraction, layer
    @location(3) misc: vec4<f32>,
    @location(4) ax: vec3<f32>,
    @location(5) ay: vec3<f32>,
    @location(6) light: vec3<f32>,
    @location(7) ldir: vec3<f32>,
    // noise offset (seed), roll angle
    @location(8) noise: vec4<f32>,
    // steam, haze, 0, 0
    @location(9) kind: vec4<f32>,
}

const CORNERS = array<vec2<f32>, 6>(vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0));

// The street lamps' falloff: the light pack's (1 − d²/r²)² or, with
// Modern Lighting, its physical pool (smoke_cinematic.rs `lamp_falloff`).
fn lamp_falloff(d2: f32, r: f32) -> f32 {
    let x = clamp(1.0 - d2 / (r * r), 0.0, 1.0);
    if (f.counts.w == 1u) {
        let y = d2 / (r * r);
        let window = max(1.0 - y * y, 0.0);
        return min((81.0 + 150.0) / (d2 + 150.0), 1.6) * window * window;
    }
    return x * x;
}

fn light_at(p: vec3<f32>, light: ptr<function, vec3<f32>>, ldir: ptr<function, vec3<f32>>) {
    for (var i = 0u; i < f.counts.x; i++) {
        let l = f.lamps[i * 2u];
        let to = l.xyz - p;
        let c = f.lamps[i * 2u + 1u].rgb * lamp_falloff(dot(to, to), l.w);
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
        // Forward scattering (Henyey-Greenstein-like): bright looking into the beam.
        let g = max(dot(-view_dir, dir.xyz), 0.0);
        let phase = 0.5 + 1.2 * g * g * g;
        let c = col.rgb * cone * att * att * phase * 0.9;
        *light += c;
        *ldir += -dir.xyz * dot(c, vec3(0.33));
    }
    for (var i = 0u; i < f.counts.z; i++) {
        let l = f.points[i * 2u];
        let to = l.xyz - p;
        let w = clamp(1.0 - dot(to, to) / (l.w * l.w), 0.0, 1.0);
        let c = f.points[i * 2u + 1u].rgb * w * w * 0.9;
        *light += c;
        *ldir += normalize(to) * dot(c, vec3(0.33));
    }
}

@vertex
fn vs_puff(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> Out {
    var o: Out;
    let puff = puffs[ii];
    o.clip = vec4(0.0, 0.0, -2.0, 1.0);
    if (puff.p1.w <= 0.0) {
        return o;
    }
    let d = ddescs[puff.ids.x];
    let haze = puff.p4.w > 0.5;
    let t = clamp(puff.p0.w / puff.p1.w, 0.0, 1.0);
    let tn = t * select(f.params.w, 1.0, haze);
    // Expansion: the native size curve over the native life, then on
    // growing (√age: a cloud spreads ever slower); the opacity thins as the
    // same smoke covers more (mass kept roughly).
    let grow = 1.0 + puff.p4.z * sqrt(puff.p0.w);
    var size = cubic(d.size, min(tn, 1.0)).x * puff.p3.x * grow;
    var col = clamp(cubic(colors[puff.ids.y], select(t, min(t * 1.6, 1.0), haze)), vec4(0.0), vec4(255.0)) / 255.0;
    // The procedural lump replaces the sprite's opacity: the same smoke
    // mass (the sprite's mean opacity over a lump's, about 0.4).
    col.a *= puff.p4.y / pow(grow, 0.5) * d.params3.x * 2.5;
    col.a *= smoothstep(0.0, select(0.08, 0.6, haze), puff.p0.w) * (1.0 - smoothstep(select(0.6, 0.4, haze), 1.0, t));
    let rel = puff.p0.xyz - f.eye.xyz;
    let depth = dot(rel, f.forward.xyz);
    if (size <= 0.0 || depth <= 0.05 || col.a <= 0.002) {
        return o;
    }
    // The native on-screen bound (0x48BDF0), twice for the haze.
    let bound = d.params.z * select(1.0, 2.0, haze);
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
    let cells = max(d.params2.z * d.params2.w, 1.0);
    let frame = f32(puff.ids.z % u32(cells));
    let cw = 1.0 / max(d.params2.z, 1.0);
    let rh = 1.0 / max(d.params2.w, 1.0);
    o.cell = vec4((frame % max(d.params2.z, 1.0)) * cw, floor(frame / max(d.params2.z, 1.0)) * rh, cw, rh);
    o.color = col;
    o.misc = vec4(dot(p, f.forward.xyz), size, t, d.params.w);
    o.ax = ax;
    o.ay = ay;
    var light = vec3(0.0);
    var ldir = vec3(0.0);
    light_at(p, &light, &ldir);
    o.light = light;
    o.ldir = ldir;
    let seed = puff.ids.z;
    // The noise rolls: its slice advances with age, its frame turns
    // against the billboard's own spin (a billow turning over).
    o.noise = vec4(f32(seed & 63u) / 64.0, f32((seed >> 6u) & 63u) / 64.0, f32((seed >> 12u) & 63u) / 64.0 + puff.p0.w * 0.045, -puff.p3.z * 0.6 * puff.p0.w);
    o.kind = vec4(puff.p4.x, select(0.0, 1.0, haze), 0.0, 0.0);
    return o;
}

fn rot(v: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2(c * v.x - s * v.y, s * v.x + c * v.y);
}

struct OitOut {
    @location(0) accum: vec4<f32>,
    @location(1) reveal: vec4<f32>,
}

@fragment
fn fs_puff(in: Out) -> OitOut {
    var o: OitOut;
    if (f.params2.w == 2.0) {
        o.accum = vec4(1.0, 0.0, 0.0, 1.0);
        o.reveal = vec4(0.5, 0.0, 0.0, 0.0);
        return o;
    }
    let c = in.uv * 2.0 - 1.0;
    let r2 = dot(c, c);
    if (r2 >= 1.0) {
        discard;
    }
    var a = in.color.a;
    let z = in.misc.x;
    // Soft against the scene (the reduced-resolution nearest depth) and
    // away from the eye.
    let lp = clamp(vec2<i32>(in.clip.xy), vec2(0), vec2<i32>(f.sizes.xy) - 1);
    let sz = textureLoad(low_depth, lp, 0).r;
    a *= clamp((sz - z) / clamp(0.3 * in.misc.y, 0.2, 1.2), 0.0, 1.0);
    a *= clamp((z - 0.3) / 1.5, 0.0, 1.0);
    if (a <= 0.002) {
        discard;
    }
    let haze = in.kind.y;
    // Billow density: two octaves of the volume (value + gradient in one
    // fetch each), the second warped by the first; the slice rolls.
    let q = vec3(rot(c, in.noise.w) * mix(0.3, 0.2, haze) + in.noise.xy, in.noise.z);
    let n1 = textureSample(vnoise_tex, vnoise_samp, q);
    let q2 = q * 2.13 + (n1.gba - 0.5) * 0.12 + vec3(0.31, 0.57, 0.11);
    let n2 = textureSample(vnoise_tex, vnoise_samp, q2);
    let dens = n1.r * 0.62 + n2.r * 0.38;
    let grad = ((n1.gba - 0.5) * 0.62 + (n2.gba - 0.5) * 0.38 * 2.13).xy;
    let t = in.misc.z;
    let detail = f.params.y;
    // A lumpy ball: the radial profile displaced by the density (round
    // cauliflower lumps with a crisp-ish edge), thinning and breaking up
    // with age; the haze wider and softer.
    let r = sqrt(r2);
    let lump = (1.0 - r) * 1.35 + (dens - 0.5) * mix(1.1, 0.7, haze) * detail - mix(0.12 + 0.5 * t * t, 0.1, haze);
    let shape = smoothstep(0.0, mix(0.32, 0.65, haze), lump);
    a *= shape;
    if (a <= 0.002) {
        discard;
    }
    a = clamp(a, 0.0, 0.97);
    // The native colour (2 · tex · colour) as the smoke's albedo; steam
    // (wet road) whiter.
    let tex = textureSampleLevel(smoke_tex, smoke_samp, in.cell.xy + in.uv * in.cell.zw, i32(in.misc.w), 0.0);
    var albedo = clamp(2.0 * tex.rgb * in.color.rgb, vec3(0.0), vec3(1.0));
    albedo = mix(albedo, max(albedo, vec3(f.params2.z)), in.kind.x);
    // A normal from the billow's shape: a sphere bent by the density
    // gradient (bumps face out of the cloud).
    let nz = sqrt(max(1.0 - r2, 0.0));
    let bend = rot(grad, -in.noise.w) * 2.0 * detail;
    let normal = normalize(in.ax * (c.x * 0.8 - bend.x) + in.ay * (c.y * 0.8 - bend.y) - f.forward.xyz * (nz + 0.25));
    // Self-shadow: the density a step toward the light, in the billboard.
    let lw = length(in.ldir);
    let l = in.ldir / max(lw, 1e-4);
    let lp2 = vec2(dot(l, in.ax), dot(l, in.ay));
    let ns = textureSampleLevel(vnoise_tex, vnoise_samp, q + vec3(rot(lp2, in.noise.w) * 0.09, 0.0), 0.0).r;
    let thick = shape * in.color.a;
    let shadow = exp(-(ns * 1.2 + thick * 1.0 + nz * 0.5) * (1.0 - haze * 0.6));
    // Ambient: the sky from above, a darker underside and core.
    let amb = mix(f.params2.y, 1.15, 0.5 + 0.5 * normal.z) * (1.0 - 0.35 * dens * thick * (1.0 - haze));
    // Steam scatters more evenly (less dark underside).
    var rgb = albedo * mix(amb, 1.0, 0.6 * in.kind.x);
    let wrap = select(1.0, clamp(0.4 + 0.6 * dot(normal, l), 0.0, 1.0), lw > 1e-4);
    // The edge lets more light through (thin), the core less.
    let rim = 0.7 + 0.6 * (1.0 - nz);
    rgb += albedo * in.light * wrap * shadow * rim * f.params.z;
    // Weighted blended OIT (McGuire & Bavoil 2013, eq. 10 on the view depth).
    let w = clamp(10.0 / (1e-5 + pow(z / 5.0, 2.0) + pow(z / 200.0, 6.0)), 1e-2, 3e3);
    o.accum = vec4(rgb * a, a) * w;
    o.reveal = vec4(a, 0.0, 0.0, 0.0);
    return o;
}

