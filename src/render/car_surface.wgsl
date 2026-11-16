// Surface Detail on cars (G7c2, clone deviation, opt-in: the Graphics
// page's Surface Detail at Materials + Normals; docs/surface-maps.md
// "Cars"). Appended to car.wgsl (or Enhanced Paint's / Wet Car's form of
// it) for the surface modules only (`car_source(.., surface: true)`); the
// native module never has it. The texture's normal map, generated on the
// player's machine from the player's own texture (surface_maps::build_car_normals),
// is bound at group 2 binding 2; it is flat (0, 0) wherever the draw's light
// material has no relief (paint, vinyls, glass, lights, chrome) and a
// 1 × 1 flat stand-in for textures without a map, so those pixels shade
// exactly as natively. Where it tilts, the per-vertex lighting car.efx
// computed is re-weighted per pixel: the diffuse by the harmonic light's
// change with the mapped normal, the specular highlight likewise, and the
// env cube's direction turned by the normal's tilt.

@group(2) @binding(2) var surface_map: texture_2d<f32>;
@group(2) @binding(3) var surface_samp: sampler;

struct SurfaceRelief {
    // Factors on the vertex stage's diffuse and specular (1 = native).
    diffuse: f32,
    specular: f32,
    // The env direction's turn (added to car_colour's `direction`).
    turn: vec3<f32>,
};

// The mapped normal in the part's space (xyz) and whether the map tilts
// here (w). Derivatives first: call in uniform control flow only.
fn surface_local_normal(in: VertexOut) -> vec4<f32> {
    let dpx = dpdx(in.local_position);
    let dpy = dpdy(in.local_position);
    let duvx = dpdx(in.uv);
    let duvy = dpdy(in.uv);
    let ng = in.local_normal * inverseSqrt(max(dot(in.local_normal, in.local_normal), 1e-12));
    let t = textureSampleGrad(surface_map, surface_samp, in.uv, duvx, duvy).xy;
    if dot(t, t) < 1e-5 {
        return vec4<f32>(ng, 0.0);
    }
    // The cotangent frame of the position over the uv (no vertex tangents
    // in the car meshes; mirrored UVs flip it with them).
    let dp2perp = cross(dpy, ng);
    let dp1perp = cross(ng, dpx);
    let tu = dp2perp * duvx.x + dp1perp * duvy.x;
    let tv = dp2perp * duvx.y + dp1perp * duvy.y;
    let inv = inverseSqrt(max(max(dot(tu, tu), dot(tv, tv)), 1e-30));
    let z = sqrt(max(1.0 - dot(t, t), 0.0));
    let n = tu * (inv * t.x) + tv * (inv * t.y) + ng * z;
    return vec4<f32>(n * inverseSqrt(max(dot(n, n), 1e-20)), 1.0);
}

const SURFACE_LUMA: vec3<f32> = vec3<f32>(0.2126, 0.7152, 0.0722);

// car.efx's vertex diffuse for normal `n` (luminance).
fn surface_diffuse(n: vec3<f32>, v: vec3<f32>) -> f32 {
    let ndv = max(dot(v, n), 0.01);
    return dot(harmonic(n).xyz * (draw.diffuse_min.xyz + ndv * draw.diffuse_range.xyz), SURFACE_LUMA);
}

// car.efx's vertex specular for normal `n` (luminance).
fn surface_highlight(n: vec3<f32>, v: vec3<f32>) -> f32 {
    let ndv = max(dot(v, n), 0.01);
    let r = 2.0 * ndv * n - v;
    let h = harmonic(r);
    let hl = exp2(log2(max(clamp(h.w, 0.0, 1.0), 1e-20)) * draw.powers.x);
    let scale = draw.specular_min.xyz + exp2(log2(ndv) * draw.powers.x) * draw.specular_range.xyz;
    return dot(max(h.xyz, vec3<f32>(0.0)) * hl * scale, SURFACE_LUMA);
}

fn surface_relief(in: VertexOut) -> SurfaceRelief {
    let s = surface_local_normal(in);
    var r = SurfaceRelief(1.0, 1.0, vec3<f32>(0.0));
    if s.w < 0.5 {
        return r;
    }
    let ng = in.local_normal * inverseSqrt(max(dot(in.local_normal, in.local_normal), 1e-12));
    let eye = draw.local_eye.xyz - in.local_position;
    let v = eye * inverseSqrt(max(dot(eye, eye), 1e-12));
    // The relief as the light's change, × 1.5 (the harmonic light is soft:
    // as the world's baked light toward the lamps), within 0.6–1.4.
    let d0 = surface_diffuse(ng, v);
    let d1 = surface_diffuse(s.xyz, v);
    r.diffuse = clamp(1.0 + 1.5 * (d1 - d0) / max(d0, 0.02), 0.6, 1.4);
    let h0 = surface_highlight(ng, v);
    let h1 = surface_highlight(s.xyz, v);
    r.specular = clamp((h1 + 0.01) / (h0 + 0.01), 0.25, 2.5);
    // The reflection's turn, in the space car_colour samples the cube in.
    let ndv0 = clamp(dot(v, ng), 0.01, 1.0);
    let ndv1 = clamp(dot(v, s.xyz), 0.01, 1.0);
    let delta = (2.0 * ndv1 * s.xyz - v) - (2.0 * ndv0 * ng - v);
    if draw.harmonic_scale.y > 0.5 && draw.harmonic_scale.z > 0.5 {
        r.turn = (draw.world * vec4<f32>(delta, 0.0)).xyz;
    } else {
        let rv = (frame.view * draw.world * vec4<f32>(delta, 0.0)).xyz;
        r.turn = vec3<f32>(rv.x, -rv.y, -rv.z);
    }
    return r;
}

// The SSR mask's world normal (car_ssr.wgsl) with the map's tilt.
fn surface_world_normal(in: VertexOut, nw0: vec3<f32>) -> vec3<f32> {
    let s = surface_local_normal(in);
    var nw = nw0;
    if s.w > 0.5 {
        nw = (draw.world * vec4<f32>(s.xyz, 0.0)).xyz;
    }
    return nw * inverseSqrt(max(dot(nw, nw), 1e-12));
}
