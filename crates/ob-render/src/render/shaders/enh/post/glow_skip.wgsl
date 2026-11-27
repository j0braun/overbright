// ---------------------------------------------------------------------
// High Precision Post's blur only where its result can be other than 0
// (glow_skip.rs): appended to postfx.wgsl for these entries alone. The
// prefilter marks each 8 x 8 tile of the glow where A can be other than 0;
// the blur passes are the same shaders, depth-tested against the tiles'
// need (glow_need.wgsl): where every tap they take is 0 their result is
// 0, the value their targets hold there.

struct GlowTiles {
    // x, y: the tiles across and down; z: the blur's reach in tiles; w: 0
    tiles: vec4<u32>,
    // xy: the glow's size
    glow: vec4<u32>,
}

@group(1) @binding(0) var<storage, read_write> glow_flags: array<u32>;
@group(1) @binding(1) var<uniform> glow_tiles: GlowTiles;

const GLOW_TILE: u32 = 8u;

// This texel's tile can hold an A other than 0: overbright's x (and with
// it A) is 0 in every channel at or below OVERBRIGHTOFFSET. Taken from the
// prefilter's input, not from A: A's own value would have to be kept at
// full precision for the test, and its write would round differently.
fn glow_flag(position: vec4<f32>, t0: vec4<f32>) {
    if any(t0.rgb > vec3<f32>(pass_.params.z)) {
        let t = vec2<u32>(position.xy) / GLOW_TILE;
        glow_flags[t.y * glow_tiles.tiles.x + t.x] = 1u;
    }
}

@fragment
fn fs_overbright_mrt_hdr_flag(in: VsOut) -> OverbrightOut {
    let out = overbright_mrt_hdr(in.uv);
    glow_flag(in.position, out.c);
    return out;
}

@fragment
fn fs_overbright_sat_flag(in: VsOut) -> @location(0) vec4<f32> {
    glow_flag(in.position, tap(in.uv));
    return overbright_sat(in.uv);
}

// vs_main at the depths the tiles' need is tested against (the same
// triangle and uv).
fn vs_at(i: u32, z: f32) -> VsOut {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: VsOut;
    out.position = vec4<f32>(p * 2.0 - 1.0, z, 1.0);
    out.uv = vec2<f32>(p.x, 1.0 - p.y);
    return out;
}

@vertex
fn vs_glow_across(@builtin(vertex_index) i: u32) -> VsOut {
    return vs_at(i, 0.25);
}

@vertex
fn vs_glow_down(@builtin(vertex_index) i: u32) -> VsOut {
    return vs_at(i, 0.75);
}
