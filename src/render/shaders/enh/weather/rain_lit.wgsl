// The falling rain's drops under the lamps' module, lit at their corners
// (docs/lamps-shadows.md "The falling rain under the lamps").
//
// A drop is a parallelogram c ± A ± B, 6 cm across, its texture's alpha
// times the template's at most 0.03. world.efx's colour with the lamps
// (`world_colour` fogged, `lamp_shade`) is linear in the texel:
// rgb = texel.rgb · K + S, K and S depending on the position, not the
// texel. A first pass ("probe") draws each drop as a 2 x 2 block of a small
// target, one pixel per corner: the drop's vertices placed twice as far
// from its centre (positions and UVs, `upload_particles`), so the block's
// pixel centres fall on the corners and every interpolated input (world
// position, clip position, headlight projection) is the corner's own;
// the block's derivatives span the drop. K and S are written to the
// target's upper and lower half (two pipelines, two viewports). The drop
// then draws with K and S filtered between its corners (the target's
// texels, bilinear) instead of the shading per pixel.

// The probe target: DROP_BLOCKS blocks of 2 x 2 per row, K in rows
// [0, DROP_HALF), S below (particle quad q at block (q % DROP_BLOCKS,
// q / DROP_BLOCKS); sprites.rs DROP_* the same).
const DROP_BLOCKS: u32 = 128u;
const DROP_WIDTH: f32 = 256.0;
const DROP_HALF: f32 = 16.0;

// The world's vertex input with the vertex's index (the quad's: 4 a quad).
struct DropIn {
    @builtin(vertex_index) vi: u32,
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) m0: vec4<f32>,
    @location(4) m1: vec4<f32>,
    @location(5) m2: vec4<f32>,
    @location(6) m3: vec4<f32>,
};

fn drop_vertex(d: DropIn) -> VertexIn {
    return VertexIn(d.position, d.color, d.uv, d.m0, d.m1, d.m2, d.m3);
}

// The drop's colour terms at a corner: world_colour's fogged colour
// before the fog for `texel` (texel.rgb · K + S).
fn drop_terms(in: VertexOut, texel: vec4<f32>) -> vec3<f32> {
    let n = cross(dpdx(in.world_position), dpdy(in.world_position));
    g_surface = surface_at(in, n);
    var rgb = shade(texel, in.color).rgb;
    if (D9_LAMPS && frame.lamps.x > 0.5) || (D9_CONTACT && frame.lamps.y > 0.0) || (D9_BEAMS && frame.cluster_near.z > 0.5) || g_surface.on > 0.5 {
        let base = select(in.color.rgb, headlit(in), in.lit > 0.5);
        rgb = lamp_shade(in, texel, base, n);
    }
    return rgb;
}

// The probe's vertices: the main view's (`vs_main`), placed at the drop's
// block; the clip position the lights' clusters are found by kept in
// front of the eye and on the screen.
@vertex
#if !MULTIVIEW
fn vs_drop_probe(d: DropIn) -> VertexOut {
#else
fn vs_drop_probe(d: DropIn, @builtin(view_index) view_index: u32) -> VertexOut {
    frame = frames[view_index];
#endif
    let v = drop_vertex(d);
    let vi = d.vi;
    var out = vs_common(v);
    out.uv = v.uv + texture_offset(frame.clip.y);
    let q = vi / 4u;
    // The corner's own UV (the vertex's is placed twice as far out).
    let corner = (v.uv + vec2<f32>(0.5)) * 0.5;
    let px = vec2<f32>(f32(q % DROP_BLOCKS), f32(q / DROP_BLOCKS)) * 2.0 + corner * 2.0;
    let w = max(out.clip.w, 1e-4);
    out.screen = vec4<f32>(clamp(out.clip.xy, vec2<f32>(-w), vec2<f32>(w)), out.clip.z, w);
    out.clip = vec4<f32>(px.x / DROP_WIDTH * 2.0 - 1.0, 1.0 - px.y / DROP_HALF * 2.0, 0.5, 1.0);
    return out;
}

@fragment
#if !MULTIVIEW
fn fs_drop_k(in: VertexOut) -> @location(0) vec4<f32> {
#else
fn fs_drop_k(in: VertexOut, @builtin(view_index) view_index: u32) -> @location(0) vec4<f32> {
    frame = frames[view_index];
#endif
    let s = drop_terms(in, vec4<f32>(0.0, 0.0, 0.0, 1.0));
    let one = drop_terms(in, vec4<f32>(1.0));
    return vec4<f32>(one - s, 1.0);
}

@fragment
#if !MULTIVIEW
fn fs_drop_s(in: VertexOut) -> @location(0) vec4<f32> {
#else
fn fs_drop_s(in: VertexOut, @builtin(view_index) view_index: u32) -> @location(0) vec4<f32> {
    frame = frames[view_index];
#endif
    return vec4<f32>(drop_terms(in, vec4<f32>(0.0, 0.0, 0.0, 1.0)), 1.0);
}

// The drop itself: the main view's vertices, with where its corners'
// terms are (in `hl_tex`, which this path does not project).
@vertex
#if !MULTIVIEW
fn vs_drop(d: DropIn) -> VertexOut {
#else
fn vs_drop(d: DropIn, @builtin(view_index) view_index: u32) -> VertexOut {
    frame = frames[view_index];
#endif
    let v = drop_vertex(d);
    let vi = d.vi;
    var out = vs_common(v);
    out.uv = v.uv + texture_offset(frame.clip.y);
    let q = vi / 4u;
    let at = vec2<f32>(f32(q % DROP_BLOCKS), f32(q / DROP_BLOCKS)) * 2.0 + vec2<f32>(0.5) + v.uv;
    out.hl_tex = vec4<f32>(at.x / DROP_WIDTH, at.y / (2.0 * DROP_HALF), 0.0, 1.0);
    return out;
}

// world.efx's colour (`fs_world`) with K and S between the corners
// (group 2: the probe target and a linear sampler).
@fragment
#if !MULTIVIEW
fn fs_drop(in: VertexOut) -> @location(0) vec4<f32> {
#else
fn fs_drop(in: VertexOut, @builtin(view_index) view_index: u32) -> @location(0) vec4<f32> {
    frame = frames[view_index];
#endif
    let texel = sample_tex(in.uv);
    let c = shade(texel, in.color);
    // Alpha 0 blends to the target as it was (`clear_left`).
    if c.a == 0.0 {
        discard;
    }
    let k = textureSampleLevel(reflected_tex, reflected_samp, in.hl_tex.xy, 0.0).rgb;
    let s = textureSampleLevel(reflected_tex, reflected_samp, in.hl_tex.xy + vec2<f32>(0.0, 0.5), 0.0).rgb;
    return vec4<f32>(apply_fog(texel.rgb * k + s, in.fog), c.a);
}
