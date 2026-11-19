// ---------------------------------------------------------------------
// Dynamic Lamps, Lamp Shadows, Contact Shadows (clone deviations, opt-in;
// docs/notes/graphics/lamps-shadows.md, render/lamps.rs). The lines between the LAMPS
// markers are only in the lamps' module (`WorldShared::mode`): the native
// one is this file without them, its shaders untouched. Only the main view's frame has
// lamps.x / lamps.z / lamps.w set; every other view (and the native path)
// skips all of it through uniform branches. No derivatives or implicit-LOD
// samples here (the multiview cube copies `frame` into private storage).
// For speed (docs/notes/performance/gpu.md, lamp shader specialisation): the lamps' module specialised per feature
// set by pipeline constants (`SceneMode::lamp_constants`). Each `false`
// is set only where the option is off everywhere, so its uniform is 0 in
// every frame and the branch it removes never ran: the same pixels, with
// the dead code (and its registers) gone. D9_SHADOW_Q: Lamp Shadows'
// quality in force (1 Low, 2 High; 0 unknown: the uniform decides), the
// PCF grid's radius below (lamps.z is the row's value wherever it is
// above 0.5). Defaults: every branch kept (the module as before).
override D9_LAMPS: bool = true;
override D9_CONTACT: bool = true;
override D9_SHADOWS: bool = true;
override D9_SHADOW_Q: u32 = 0u;
override D9_BEAMS: bool = true;
override D9_SURFACE: bool = true;
// BEGIN RTVIS LAMP
struct Lamp {
    position: vec4<f32>,   // xyz, w outer radius
    colour: vec4<f32>,     // rgb, w inner radius
    direction: vec4<f32>,  // xyz spot direction, w cone cosine (-2 point)
    shadow: vec4<f32>,     // x shadow layer (-1 none)
};
// END RTVIS LAMP
struct ShadowView {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
};
struct Occluder {
    centre: vec4<f32>,
    x: vec4<f32>,
    y: vec4<f32>,
    z: vec4<f32>,
};
@group(0) @binding(3) var<storage, read> lamp_list: array<Lamp>;
@group(0) @binding(4) var<storage, read> clusters: array<u32>;
@group(0) @binding(5) var<storage, read> shadow_views: array<ShadowView>;
@group(0) @binding(6) var<storage, read> occluders: array<Occluder>;
@group(0) @binding(7) var shadow_maps: texture_depth_2d_array;
@group(0) @binding(8) var shadow_samp: sampler_comparison;
// Surface Detail (clone deviation, docs/notes/graphics/surface-maps.md): the normal
// maps generated from the player's own textures (render/surface.rs).
@group(0) @binding(9) var surface_maps: texture_2d_array<f32>;
@group(0) @binding(10) var surface_samp: sampler;

// The pixel's surface (`surface_at`, taken at the top of the fragment):
// the shading normal (facing the eye), roughness, F0, on (0 = no material:
// the native / lamps path unchanged).
struct Surface {
    n: vec3<f32>,
    rough: f32,
    f0: f32,
    on: f32,
};
var<private> g_surface: Surface;
// The specular light the lamps, beams and the headlight projector add at
// this pixel (output units, before fog), gathered by lamp_light /
// beam_light / lamp_shade.
var<private> g_spec: vec3<f32>;
// The lamps' light direction at this pixel (xyz, summed by weight) and
// their total weight (w), gathered by lamp_light for the map's relief.
var<private> g_dir: vec4<f32>;

// frame.cull.z (main view only): 1 Materials, 2 Materials + Normals. The
// slot's material (immediates): roughness, F0 (bytes 0, 1, / 255), the
// map's strength (byte 2, / 64), byte 3 = 1. Explicit-gradient sample only
// (no implicit derivatives inside branches: the multiview form).
fn surface_at(in: VertexOut, n_geo: vec3<f32>) -> Surface {
    let dpx = dpdx(in.world_position);
    let dpy = dpdy(in.world_position);
    let duvx = dpdx(in.uv);
    let duvy = dpdy(in.uv);
    var ng = n_geo * inverseSqrt(max(dot(n_geo, n_geo), 1e-20));
    if dot(ng, frame.camera.xyz - in.world_position) < 0.0 { ng = -ng; }
    var s = Surface(ng, 1.0, 0.0, 0.0);
    g_spec = vec3<f32>(0.0);
    g_dir = vec4<f32>(0.0);
    let slot = surface_slot();
    if (!D9_SURFACE || frame.cull.z < 0.5) || slot.y == 0u {
        return s;
    }
    let m = slot.y;
    s.rough = f32(m & 0xffu) / 255.0;
    s.f0 = f32((m >> 8u) & 0xffu) / 255.0;
    s.on = 1.0;
    let k = f32((m >> 16u) & 0xffu) / 64.0;
    if frame.cull.z > 1.5 && slot.x > 0u && k > 0.0 {
        let t = textureSampleGrad(surface_maps, surface_samp, in.uv, i32(slot.x), duvx, duvy).xy;
        // The tangent frame from the derivatives (no vertex tangents in the
        // world's meshes): the cotangent frame of p over uv.
        let dp2perp = cross(dpy, ng);
        let dp1perp = cross(ng, dpx);
        let tu = dp2perp * duvx.x + dp1perp * duvy.x;
        let tv = dp2perp * duvx.y + dp1perp * duvy.y;
        let inv = inverseSqrt(max(max(dot(tu, tu), dot(tv, tv)), 1e-30));
        let z = sqrt(max(1.0 - dot(t, t), 0.0));
        let n = tu * (inv * t.x) + tv * (inv * t.y) + ng * z;
        s.n = n * inverseSqrt(max(dot(n, n), 1e-20));
        // Specular anti-aliasing: where the map is minified its averaged
        // grain becomes roughness (the mip's normals are renormalised).
        let footprint = max(length(duvx), length(duvy)) * 256.0;
        let spread = clamp(0.03 * log2(max(footprint, 1.0)), 0.0, 0.2);
        s.rough = sqrt(s.rough * s.rough + spread);
    }
    return s;
}

// GGX specular (Smith-Schlick visibility, Schlick Fresnel) × N·L for a light
// toward `l`, roughness widened by the light's size seen from the pixel
// (`size` = its radius over its distance).
fn surface_spec(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, rough: f32, f0: f32, size: f32) -> f32 {
    let ndl = dot(n, l);
    let ndv = max(dot(n, v), 1e-3);
    if ndl <= 0.0 { return 0.0; }
    let h = normalize(l + v);
    let ndh = max(dot(n, h), 0.0);
    let vdh = max(dot(v, h), 0.0);
    let a = clamp(rough * rough + 0.5 * size, 0.02, 1.0);
    let a2 = a * a;
    let d = ndh * ndh * (a2 - 1.0) + 1.0;
    let dist = a2 / (3.14159265 * d * d);
    let kk = a * 0.5;
    let vis = 0.25 / ((ndl * (1.0 - kk) + kk) * (ndv * (1.0 - kk) + kk));
    let f = f0 + (1.0 - f0) * pow(1.0 - vdh, 5.0);
    return min(dist * vis * f * ndl, SURFACE_SPEC_MAX);
}
const SURFACE_SPEC_MAX: f32 = 4.0;
// How strongly the specular light shows (output units per light unit).
const SURFACE_SPEC: f32 = 0.6;
// How strongly the map's N·L change shows in the headlight projector's light.
const SURFACE_RELIEF: f32 = 1.5;
// ... and in the baked light, toward the lamps lighting the pixel (full
// from a summed lamp weight of SURFACE_LAMP_FULL).
const SURFACE_RELIEF_BAKED: f32 = 1.5;
const SURFACE_LAMP_FULL: f32 = 0.25;

// The native light weight (0x4893A0): 1 inside the inner radius, falling
// by d² to 0 at the outer one.
fn lamp_weight(d2: f32, inner: f32, outer: f32) -> f32 {
    let i2 = inner * inner;
    let o2 = outer * outer;
    if d2 >= o2 { return 0.0; }
    if d2 <= i2 || o2 <= i2 { return 1.0; }
    return 1.0 - (d2 - i2) / (o2 - i2);
}

// A shadow layer's visibility at p (normal-offset, PCF over a 3×3 or 2×2
// grid of bilinear compares); outside its frustum 1.
fn shadow_visibility(layer: i32, p: vec3<f32>, n: vec3<f32>) -> f32 {
    let v = shadow_views[layer];
    let dist = length(p - v.eye.xyz);
    // Normal offset, and an offset toward the light that grows with the
    // distance (the headlight sees the road at a grazing angle).
    let q = p + n * (v.eye.w * (1.0 + dist * 0.02)) + (v.eye.xyz - p) / max(dist, 1e-3) * (v.eye.w + dist * 0.03);
    let c = v.view_proj * vec4<f32>(q, 1.0);
    if c.w <= 0.0 { return 1.0; }
    let ndc = c.xyz / c.w;
    if any(abs(ndc.xy) > vec2<f32>(1.0)) || ndc.z > 1.0 || ndc.z < 0.0 { return 1.0; }
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    let texel = 1.0 / f32(textureDimensions(shadow_maps).x);
    // Fade out towards the frustum's edge.
    let edge = 1.0 - smoothstep(0.8, 1.0, max(abs(ndc.x), abs(ndc.y)));
    var sum = 0.0;
    var taps = 0.0;
    let r = select(select(1, 0, frame.lamps.z < 1.5), i32(D9_SHADOW_Q) - 1, D9_SHADOW_Q != 0u);
    for (var y = -r; y <= r; y++) {
        for (var x = -r; x <= r; x++) {
            let o = vec2<f32>(f32(x), f32(y)) * texel * 1.5;
            sum += textureSampleCompareLevel(shadow_maps, shadow_samp, uv + o, layer, ndc.z);
            taps += 1.0;
        }
    }
    return mix(1.0, sum / taps, edge);
}

// The cars per cluster (lamps::car_masks): beside each entry of a
// cluster's lamp list, at its word + LAMP_CAR_MASKS, the cars that can
// shade a point of the cluster from that lamp, a bit each (the others
// give exactly 1 there).
const LAMP_CAR_MASKS: u32 = 262144u;

// The cars `cars` (a bit each) as soft ellipsoids between p and a light at
// l, in their order.
fn car_visibility(p: vec3<f32>, l: vec3<f32>, cars: u32) -> f32 {
    var vis = 1.0;
    #if !SUBGROUPS
    var m = cars;
    #else
    // The subgroup's cars together: each car's record read once for the
    // subgroup (scalar loads), every lane taking only its own, in order.
    var m = subgroupOr(cars);
    #endif
    while m != 0u {
        let i = firstTrailingBit(m);
        m &= m - 1u;
        #if !SUBGROUPS
        #else
        if (cars & (1u << i)) == 0u { continue; }
        #endif
        let o = occluders[i];
        let rel0 = p - o.centre.xyz;
        // A car farther than 12 m from p shades it from no lamp here.
        if dot(rel0, rel0) > 144.0 { continue; }
        let rel1 = l - o.centre.xyz;
        // Into the unit sphere's space (the box's half extents, the
        // ellipsoid through the box's face centres, a little wider).
        let s = vec3<f32>(o.x.w, o.y.w, o.z.w) * 1.15;
        let a = vec3<f32>(dot(rel0, o.x.xyz), dot(rel0, o.y.xyz), dot(rel0, o.z.xyz)) / s;
        let b = vec3<f32>(dot(rel1, o.x.xyz), dot(rel1, o.y.xyz), dot(rel1, o.z.xyz)) / s;
        if dot(a, a) < 1.0 { continue; } // p is under/inside the car: the contact shadow's job
        let d = b - a;
        let t = clamp(-dot(a, d) / max(dot(d, d), 1e-6), 0.0, 1.0);
        let closest = length(a + d * t);
        vis *= mix(1.0 - SHADOW_DEPTH_CARS, 1.0, smoothstep(0.7, 1.3, closest));
    }
    return vis;
}
const SHADOW_DEPTH_CARS: f32 = 0.6;

// The light cluster of p (`screen`: its clip position): its tile and its
// view depth's slice.
fn cluster_at(p: vec3<f32>, screen: vec4<f32>) -> u32 {
    let ndc = screen.xy / screen.w;
    let tiles = frame.cluster.xy;
    let tx = u32(clamp(floor((ndc.x * 0.5 + 0.5) * tiles.x), 0.0, tiles.x - 1.0));
    let ty = u32(clamp(floor((ndc.y * 0.5 + 0.5) * tiles.y), 0.0, tiles.y - 1.0));
    let z = dot(p - frame.camera.xyz, frame.forward.xyz);
    let slice = u32(clamp(floor(log(max(z, frame.cluster_near.x) / frame.cluster_near.x) * frame.cluster.w), 0.0, frame.cluster.z - 1.0));
    return (slice * u32(tiles.y) + ty) * u32(tiles.x) + tx;
}

// The added lamp light at p (vertex-colour units: worldheadlight's r0).
fn lamp_light(p: vec3<f32>, n_in: vec3<f32>, screen: vec4<f32>) -> vec3<f32> {
    let c = cluster_at(p, screen);
    var n = n_in * inverseSqrt(max(dot(n_in, n_in), 1e-20));
    // Two-sided (world.efx draws with CULLMODE NONE): the side facing the eye.
    if dot(n, frame.camera.xyz - p) < 0.0 { n = -n; }
    #if !SUBGROUPS
    return cluster_light(c, p, n);
    #else
    // The same sum, one cluster at a time per subgroup (docs/notes/graphics/lamps-shadows.md,
    // "The cluster loop"): the first active lane's cluster goes to every lane,
    // the lanes in it add its lamps in their order (the cluster's index and
    // so its lamp list are uniform: scalar loads, not one per lane) and leave;
    // the rest take the next lane's. Each round ends at least the first
    // lane's cluster, so the subgroup's lanes (at most 128) are all done
    // within 128 rounds; the bound only stops a lane the broadcast leaves out
    // (a helper lane where wave operations skip helpers), whose light no
    // pixel and no derivative reads.
    for (var round = 0u; round < 128u; round++) {
        let first_lane = subgroupBroadcastFirst(c);
        if c == first_lane {
            return cluster_light(first_lane, p, n);
        }
    }
    return vec3<f32>(0.0);
    #endif
}

// The lamps of cluster `c` at p (normal n facing the eye), summed in the
// cluster's order; the specular and the light direction into g_spec / g_dir.
fn cluster_light(c: u32, p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    let first = clusters[c * 2u];
    let count = clusters[c * 2u + 1u];
    var sum = vec3<f32>(0.0);
    for (var i = 0u; i < count; i++) {
        let l = lamp_list[clusters[first + i]];
        let d = l.position.xyz - p;
        let d2 = dot(d, d);
        var w = lamp_weight(d2, l.colour.w, l.position.w);
        if w <= 0.0 { continue; }
        let dir = d * inverseSqrt(max(d2, 1e-12));
        if l.direction.w > -1.5 {
            let f = dot(-dir, l.direction.xyz);
            w *= select(0.0, (f - l.direction.w) / max(1.0 - l.direction.w, 1e-4), f > l.direction.w);
            // Outside the cone: nothing to add.
            if w <= 0.0 { continue; }
        }
        // Lambert, and the native weight squared: still exactly 0 at the
        // authored outer radius, but a pool under the lamp rather than a
        // flat lift between lamps (the packs' radii overlap along a street).
        let ndl = max(dot(n, dir), 0.0);
        // A face turned away adds nothing (and needs no shadow), unless
        // Surface Detail's specular and light direction take the lamp.
        if ndl <= 0.0 && g_surface.on < 0.5 { continue; }
        w *= w;
        var vis = 1.0;
        if (D9_SHADOWS && frame.lamps.z > 0.5) {
            if l.shadow.x >= 0.0 {
                vis = mix(1.0, shadow_visibility(i32(l.shadow.x), p, n), SHADOW_DEPTH_LAMPS * (1.0 - fract(l.shadow.x)));
            }
            vis *= car_visibility(p, l.position.xyz, clusters[first + i + LAMP_CAR_MASKS]);
        }
        sum += l.colour.rgb * (w * ndl * vis);
        if g_surface.on > 0.5 {
            let v = normalize(frame.camera.xyz - p);
            g_spec += l.colour.rgb * (w * vis * surface_spec(g_surface.n, v, dir, g_surface.rough, g_surface.f0, 0.3 * inverseSqrt(max(d2, 0.09))));
            g_dir += vec4<f32>(dir, 1.0) * (w * vis * dot(l.colour.rgb, vec3<f32>(0.2126, 0.7152, 0.0722)));
        } else if all(sum >= vec3<f32>(LAMP_MAX)) {
            // `lamp_shade` takes at most LAMP_MAX of the sum, and the
            // terms are not negative: the rest add nothing.
            break;
        }
    }
    return sum;
}
const SHADOW_DEPTH_LAMPS: f32 = 0.7;
// Against over-bright lamp spots: the baked light level (vertex-colour units, 0.5 = the texture at
// 1×) from which Dynamic Lamps add nothing, and the most they add.
const LAMP_PEAK: f32 = 0.5;
const LAMP_MAX: f32 = 0.12;

// Contact shadows: how much of the light remains at p under the cars
// (`screen`: its clip position). Only the cars whose 4 m reach touches p's
// cluster (lamps::contact_masks, a word per cluster at CONTACT_CARS; the
// others leave p as it is), in their order.
const CONTACT_CORE: f32 = 0.75;
const CONTACT_CARS: u32 = 531200u;
fn contact_visibility(p: vec3<f32>, screen: vec4<f32>) -> f32 {
    var vis = 1.0;
    let cars = clusters[CONTACT_CARS + cluster_at(p, screen)];
    #if !SUBGROUPS
    var m = cars;
    #else
    var m = subgroupOr(cars);
    #endif
    while m != 0u {
        let i = firstTrailingBit(m);
        m &= m - 1u;
        #if !SUBGROUPS
        #else
        if (cars & (1u << i)) == 0u { continue; }
        #endif
        let o = occluders[i];
        let rel = p - o.centre.xyz;
        if dot(rel, rel) > 16.0 { continue; }
        let q = vec3<f32>(dot(rel, o.x.xyz), dot(rel, o.y.xyz), dot(rel, o.z.xyz));
        let h = vec3<f32>(o.x.w, o.y.w, o.z.w);
        // Full inside CONTACT_CORE of the box, gone at its edge (the
        // body + 0.1 m): past the body the native shadow texture alone
        // grounds the car (the old 0.35 m + reach past it stacked on it).
        let e = max(abs(q.xy) - h.xy * CONTACT_CORE, vec2<f32>(0.0)) / (h.xy * (1.0 - CONTACT_CORE));
        let footprint = 1.0 - smoothstep(0.0, 1.0, length(e));
        // Height above the box's bottom: full at and below it, gone by
        // half the box's height above it; and fading 0.6 m under it.
        let above = q.z + h.z;
        let vertical = (1.0 - smoothstep(0.0, h.z, above)) * (1.0 - smoothstep(0.0, 0.6, -above - 0.3));
        // centre.w: the share kept under the car's underglow (see
        // lamps::contact_share): the glow lights what the car shades.
        vis *= 1.0 - frame.lamps.y * footprint * vertical * o.centre.w;
    }
    return vis;
}

// The cars per cluster that can shade it from the player's headlight
// (lamps::headlight_masks, a word per cluster at HEADLIGHT_CARS; never the
// headlight's own car 0).
const HEADLIGHT_CARS: u32 = 534656u;

// The headlight projector's light at p as Lamp Shadows leaves it: its
// depth layer (cluster_near.y, -1 none) and the other cars (`screen`: p's
// clip position).
fn headlight_visibility(p: vec3<f32>, screen: vec4<f32>) -> f32 {
    var vis = 1.0;
    let layer = i32(frame.cluster_near.y);
    if layer >= 0 {
        vis = mix(1.0, shadow_visibility(layer, p, vec3<f32>(0.0)), HEADLIGHT_SHADOW_DEPTH);
    }
    return vis * car_visibility(p, frame.hl_eye.xyz, clusters[HEADLIGHT_CARS + cluster_at(p, screen)]);
}
const HEADLIGHT_SHADOW_DEPTH: f32 = 0.7;

