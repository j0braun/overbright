// What High Precision Post's blur passes need, per 8 x 8 tile of the glow
// (glow_skip.rs): from the tiles where A can be other than 0
// (glow_skip.wgsl), widened by the blur's reach. The pass across reads A
// within the reach along its own row (its taps sit on the texel's row: the
// rows beside it are fetched at a weight of 0 at most), so its result can
// be other than 0 only within k tiles across of such a tile; the pass down
// reads that result within the reach along its column: k tiles down of
// those. 0: both passes, 1: the pass down only, 2: neither.

struct GlowTiles {
    tiles: vec4<u32>,
    glow: vec4<u32>,
}

// cs_rows: the flags into the rows; cs_need: the rows into the need (each
// its own pass: the rows are written before they are read).
@group(0) @binding(0) var<storage, read> flags: array<u32>;
@group(0) @binding(1) var<storage, read_write> rows: array<u32>;
@group(0) @binding(2) var<storage, read> rows_in: array<u32>;
@group(0) @binding(3) var<storage, read_write> need: array<u32>;
@group(0) @binding(6) var<uniform> info: GlowTiles;

const GLOW_TILE: u32 = 8u;

// Along the rows: 1 with a flag within k tiles across (the pass across).
@compute @workgroup_size(64)
fn cs_rows(@builtin(global_invocation_id) id: vec3<u32>) {
    let n = info.tiles.xy;
    if id.x >= n.x || id.y >= n.y {
        return;
    }
    let k = i32(info.tiles.z);
    let x = i32(id.x);
    let row = id.y * n.x;
    var lit = 0u;
    for (var d = -k; d <= k; d++) {
        let xx = x + d;
        if xx >= 0 && xx < i32(n.x) && flags[row + u32(xx)] != 0u {
            lit = 1u;
        }
    }
    rows[row + id.x] = lit;
}

// Down the columns: across where this tile's row is lit, down where one
// within k tiles down is.
@compute @workgroup_size(64)
fn cs_need(@builtin(global_invocation_id) id: vec3<u32>) {
    let n = info.tiles.xy;
    if id.x >= n.x || id.y >= n.y {
        return;
    }
    let k = i32(info.tiles.z);
    let y = i32(id.y);
    let across = rows_in[id.y * n.x + id.x] != 0u;
    var down = false;
    for (var d = -k; d <= k; d++) {
        let yy = y + d;
        if yy >= 0 && yy < i32(n.y) && rows_in[u32(yy) * n.x + id.x] != 0u {
            down = true;
        }
    }
    need[id.y * n.x + id.x] = select(select(2u, 1u, down), 0u, across);
}

// The need as depth (0, 0.5, 1; cleared to 1): one quad per tile, none
// for a tile that needs neither pass.
@group(0) @binding(4) var<storage, read> need_in: array<u32>;
@group(0) @binding(5) var<uniform> info_in: GlowTiles;

@vertex
fn vs_need(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> @builtin(position) vec4<f32> {
    let n = need_in[i];
    if n >= 2u {
        return vec4<f32>(2.0, 2.0, 2.0, 1.0);
    }
    let t = vec2<u32>(i % info_in.tiles.x, i / info_in.tiles.x);
    let corner = array<vec2<u32>, 6>(vec2<u32>(0u, 0u), vec2<u32>(1u, 0u), vec2<u32>(0u, 1u), vec2<u32>(1u, 0u), vec2<u32>(1u, 1u), vec2<u32>(0u, 1u));
    let px = vec2<f32>((t + corner[v]) * GLOW_TILE) / vec2<f32>(info_in.glow.xy);
    return vec4<f32>(px.x * 2.0 - 1.0, 1.0 - px.y * 2.0, f32(n) * 0.5, 1.0);
}
