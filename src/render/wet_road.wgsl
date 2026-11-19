// Wet Road Interaction's trail map (clone deviation,
// docs/notes/graphics/wet-road-car.md): a world-space torus around the camera, texel =
// fract(world xy / size). R = the track (water pushed away), G = the wake's
// freshness, B = closeness to the track × G, A = the wake's height × G.

struct MapUniform {
    // new centre xy, old centre xy (world)
    centres: vec4<f32>,
    // size (m), texels
    size: vec4<f32>,
    // per-channel decay this frame
    decay: vec4<f32>,
};

struct Stamp {
    // segment start, end (world xy less the torus base × size)
    seg: vec4<f32>,
    // track half width, wake half width, track strength, wake height
    shape: vec4<f32>,
    // torus copy offset (uv)
    offset: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: MapUniform;
@group(0) @binding(1) var<storage, read> stamps: array<Stamp>;

struct FullOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_full(@builtin(vertex_index) vi: u32) -> FullOut {
    let p = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    var o: FullOut;
    o.clip = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(p.x, 1.0 - p.y);
    return o;
}

// The decay, and the texels that came into the window since the last
// frame (they held the far side's trails) cleared.
@fragment
fn fs_decay(in: FullOut) -> @location(0) vec4<f32> {
    let size = u.size.x;
    let c1 = u.centres.xy;
    let c0 = u.centres.zw;
    // The texel's world position nearest the new centre.
    let w = c1 + (fract(in.uv - c1 / size + 0.5) - 0.5) * size;
    let d = abs(w - c0);
    if max(d.x, d.y) > size * 0.5 - u.size.x / u.size.y {
        return vec4<f32>(0.0);
    }
    return u.decay;
}

struct StampOut {
    @builtin(position) clip: vec4<f32>,
    // world xy relative to the segment's start
    @location(0) rel: vec2<f32>,
    @location(1) @interpolate(flat) seg: vec2<f32>,
    @location(2) @interpolate(flat) shape: vec4<f32>,
};

@vertex
fn vs_stamp(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> StampOut {
    let s = stamps[ii];
    let a = s.seg.xy;
    let b = s.seg.zw;
    let ab = b - a;
    let len = length(ab);
    let dir = select(vec2<f32>(1.0, 0.0), ab / max(len, 1e-5), len > 1e-4);
    let n = vec2<f32>(-dir.y, dir.x);
    let r = max(s.shape.x, s.shape.y);
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u));
    let p = a + dir * mix(-r, len + r, corner.x) + n * mix(-r, r, corner.y);
    // uv of the torus copy (the segment's base cell, shifted by the copy's
    // offset); y down.
    let uv = p / u.size.x + s.offset.xy;
    var o: StampOut;
    o.clip = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    o.rel = p - a;
    o.seg = ab;
    o.shape = s.shape;
    return o;
}

@fragment
fn fs_stamp(in: StampOut) -> @location(0) vec4<f32> {
    let ab = in.seg;
    let t = clamp(dot(in.rel, ab) / max(dot(ab, ab), 1e-8), 0.0, 1.0);
    let d = length(in.rel - ab * t);
    let track = in.shape.z * (1.0 - smoothstep(in.shape.x * 0.55, in.shape.x, d));
    let close = clamp(1.0 - d / in.shape.y, 0.0, 1.0);
    let wake = select(0.0, 1.0, close > 0.0 && in.shape.w > 0.0);
    if track <= 0.0 && wake <= 0.0 {
        discard;
    }
    return vec4<f32>(track, wake, close * wake, in.shape.w * wake);
}
