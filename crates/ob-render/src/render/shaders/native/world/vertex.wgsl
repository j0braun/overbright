

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,   // D3DCOLOR bytes arrive as (B, G, R, A)
    @location(2) uv: vec2<f32>,
    // Per-instance local->world matrix, one column per attribute.
    @location(3) m0: vec4<f32>,
    @location(4) m1: vec4<f32>,
    @location(5) m2: vec4<f32>,
    @location(6) m3: vec4<f32>,
};

struct VertexOut {
#if !INVARIANT_POSITION
    @builtin(position) clip: vec4<f32>,
#else
    @builtin(position) @invariant clip: vec4<f32>,
#endif
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    // D3D fog factor: 1 = no fog.
    @location(2) fog: f32,
    // Clip-space position, for screen-space reflection lookups.
    @location(3) screen: vec4<f32>,
    // worldheadlight's t1 (projective) and t2.x; `lit` is 1 for an
    // instance view 9's cull took (scenery flag 0x8000 → draw flag 0x200).
    @location(4) hl_tex: vec4<f32>,
    @location(5) hl_clip: f32,
    @location(6) @interpolate(flat) lit: f32,
    @location(8) world_position: vec3<f32>,
};

// 0x5D64E0: how big the scenery cull finds an instance (position p,
// radius r) from an eye looking along `fwd`, truncated: 0 when it is
// behind (forward distance below -r), the scale itself when the eye is
// within 2r of it, else scale * r / (|eye - p| - r).
fn cull_size(p: vec3<f32>, r: f32, eye: vec3<f32>, fwd: vec3<f32>, scale: f32) -> f32 {
    let d = p - eye;
    if dot(d, fwd) < -r {
        return 0.0;
    }
    let gap = length(d) - r;
    if gap <= r {
        return trunc(scale);
    }
    return trunc(scale / gap * r);
}

// TextureOffset as 0x48B450 computes it: x - trunc(x).
fn texture_offset(t: f32) -> vec2<f32> {
    let scroll = tex_scroll();
    let kind = scroll.kind_speed_step.x;
    let speed = scroll.kind_speed_step.yz;
    let step = scroll.kind_speed_step.w;
    if kind == 0.0 {
        return vec2<f32>(0.0);
    }
    if kind == 3.0 {
        return scroll.fixed.xy;
    }
    var time = t;
    if kind == 2.0 && step != 0.0 {
        time = trunc(t / step);
    }
    let x = time * speed;
    return x - trunc(x);
}

// world.efx / worldnofog.efx / skymorph.efx: uv + TextureOffset.
@vertex
#if !MULTIVIEW
fn vs_main(v: VertexIn) -> VertexOut {
#else
fn vs_main(v: VertexIn, @builtin(view_index) view_index: u32) -> VertexOut {
    frame = frames[view_index];
#endif
    var out = vs_common(v);
    out.uv = v.uv + texture_offset(frame.clip.y);
    return out;
}

// worldreflect.efx passes the UVs through unchanged.
@vertex
#if !MULTIVIEW
fn vs_reflect(v: VertexIn) -> VertexOut {
#else
fn vs_reflect(v: VertexIn, @builtin(view_index) view_index: u32) -> VertexOut {
    frame = frames[view_index];
#endif
    return vs_common(v);
}

fn vs_common(v: VertexIn) -> VertexOut {
    // m0.w flags a draw the game gives a matrix (not null-transform
    // geometry). In the road reflection (clip.w = 1) such matrices have
    // m22 negated (0x5D67B4), flipping each prop about its own origin.
    var m2 = v.m2;
    if frame.clip.w > 0.5 && frame.clip.w < 1.5 && v.m0.w > 0.5 {
        m2.z = -m2.z;
    }
    let model = mat4x4<f32>(vec4<f32>(v.m0.xyz, 0.0), vec4<f32>(v.m1.xyz, 0.0), m2, v.m3);
    let world = model * vec4<f32>(v.position, 1.0);
    let view_z = dot(world.xyz - frame.camera.xyz, frame.forward.xyz);
    var out: VertexOut;
    out.clip = frame.view_proj * world;
    // m1.w: the instance's cull radius (0: not scenery). The cull
    // (0x5D6580) drops it below the view's size; it is sized about its
    // position (instance +0x20, the matrix's translation; 0 for
    // null-transform geometry, whose radius reaches the origin).
    let radius = v.m1.w;
    out.lit = 0.0;
    // A negative radius: the instance's flags have 0x10, which every
    // view's cull flags have (0x5B9C30), so no view draws it.
    if radius < 0.0 {
        out.clip = vec4<f32>(0.0);
    }
    if radius > 0.0 {
        if cull_size(v.m3.xyz, radius, frame.camera.xyz, frame.forward.xyz, frame.cull.x) < frame.cull.y {
            out.clip = vec4<f32>(0.0);
        }
        // View 9's cull flags what it takes (0x488B90); its context has
        // base 0, so it needs 17 (0x5D66A4) at its own scale.
        if frame.hl_eye.w > 0.5 && cull_size(v.m3.xyz, radius, frame.hl_eye.xyz, frame.hl_forward.xyz, frame.hl_forward.w) >= 17.0 {
            out.lit = 1.0;
        }
    }
    #if LAMPS
    // BEGIN LAMPS
    #if MODERN
    // BEGIN MODERN
    // The NIS characters' instance (m0.w = ML_CHARACTER_MARK, which the
    // native reads as 0, `characters.rs`): Modern Lighting lights them
    // without the face normal.
    if abs(v.m0.w - ML_CHARACTER_MARK) < 0.01 {
        out.lit = -1.0;
    }
    // END MODERN
    #endif
    // END LAMPS
    #endif
    // HeadlightWVP = World * view 9 * (identity with 0.5 added to x and y
    // per w, 0x80052C / 0x800530); the clip texture's u = 0.005 w + 0.5
    // (the vs's own c13).
    let rel = world.xyz - frame.hl_eye.xyz;
    let w = dot(rel, frame.hl_forward.xyz);
    out.hl_tex = vec4<f32>(dot(rel, frame.hl_right.xyz) * frame.hl_right.w + 0.5 * w, dot(rel, frame.hl_up.xyz) * frame.hl_up.w + 0.5 * w, 0.0, w);
    out.hl_clip = w * 0.005 + 0.5;
    out.color = v.color.zyxw;
    out.uv = v.uv;
    out.fog = clamp((frame.fog_value.x - view_z) * frame.fog_value.y, frame.fog_value.z, 1.0);
    out.screen = out.clip;
    out.world_position = world.xyz;
    return out;
}

