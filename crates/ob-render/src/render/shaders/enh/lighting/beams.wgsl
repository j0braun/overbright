// Headlight Beams (clone deviation, docs/notes/graphics/lighting2.md): every car's
// headlights as view 9's projector (the native one is the player's only):
// the eye on the car (+0.75 m up), its frustum (2.5 rad across, × 0.6
// down), HeadlightTex (HEADLIGHTS: the soft cookie with its bright centre)
// × HeadlightClipTex (the near cut-off at ~3 m and the fall to 0 by ~48 m)
// × HeadlightColor, added to the light term as worldheadlight adds it, on
// every fogged surface in the frustum (not only view 9's culled
// instances), turned off on faces facing away from the car. The beams sit
// after the lamps in lamp_list: cluster_near.z = their count, .w = the
// first; per beam position = (eye, 1 the player's), colour = (rgb, the
// projection's y scale), direction = (forward, its x scale), shadow =
// (right, -) (`lamps::gpu_beam`, from `headlight_uniform`). The other
// cars' beams are weighed by the bake's headroom below BEAM_PEAK (as the dynamic
// lamps): where the vertex colours already light the road brightly (a
// race start under the lamps) several beams would wash the texture out. The player's takes Lamp
// Shadows' headlight layer and the other cars; the rest no shadow.
// Next-Gen car lights (docs/notes/graphics/car-lights.md, `car_lights::world_glows`;
// inferred, by eye): after the beams, position.w 2 a tail / brake light
// (colour.w its reach, direction its facing: full light behind the car, a
// quarter ahead of the lamp), 3 an underglow: a rectangle on the ground
// under the car (direction = forward, w half length; shadow = side, w half
// width) lighting what is within colour.w of its edge, softly, toward the
// nearest point of the rectangle.
fn car_glow_light(b: Lamp, p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    let rel = p - b.position.xyz;
    if b.position.w < 2.5 {
        let d2 = dot(rel, rel);
        let r2 = b.colour.w * b.colour.w;
        if d2 >= r2 { return vec3<f32>(0.0); }
        let l = -rel * inverseSqrt(max(d2, 1e-6));
        let w = 1.0 - d2 / r2;
        let lobe = 0.25 + 0.75 * smoothstep(-0.3, 0.6, -dot(l, b.direction.xyz));
        let ndl = clamp(dot(n, l) * 0.8 + 0.2, 0.0, 1.0);
        return b.colour.rgb * (w * w * lobe * ndl / (1.0 + d2 * 0.15));
    }
    let x = dot(rel, b.direction.xyz);
    let y = dot(rel, b.shadow.xyz);
    let z = rel.z;
    let cx = clamp(x, -b.direction.w, b.direction.w);
    let cy = clamp(y, -b.shadow.w, b.shadow.w);
    let out = vec3<f32>(x - cx, y - cy, z);
    let dist = length(out);
    if dist >= b.colour.w { return vec3<f32>(0.0); }
    let near = b.position.xyz + b.direction.xyz * cx + b.shadow.xyz * cy;
    let to = near - p;
    let l = to * inverseSqrt(max(dot(to, to), 1e-6));
    let fall = 1.0 - smoothstep(0.0, b.colour.w, dist);
    // Under the car (inside the rectangle) the light comes from above.
    let ndl = select(clamp(dot(n, l) * 0.7 + 0.3, 0.0, 1.0), 1.0, dist < 0.05);
    return b.colour.rgb * (fall * fall * ndl);
}

const BEAM_PEAK: f32 = 0.5;
// Look rule "the bake is the anchor" (docs/notes/graphics/look-direction.md): the other cars' beams are new light
// (the native projector is the player's only), held to the dark-gap budget:
// 0.8 from a bake of 0.2 up washed the race starts' grids white (drift start
// +10.7 mean, p95 +58); 0.3, gone from a bake of 0.25 (half the peak).
const BEAM_OTHERS: f32 = 0.3;
// `screen`: p's clip position (its cluster's cars for the player's beam).
fn beam_light(p: vec3<f32>, n_in: vec3<f32>, base: f32, screen: vec4<f32>) -> vec3<f32> {
    let others = BEAM_OTHERS * (1.0 - smoothstep(0.2 * BEAM_PEAK, 0.5 * BEAM_PEAK, base));
    let count = u32(frame.cluster_near.z);
    let first = u32(frame.cluster_near.w);
    var n = n_in * inverseSqrt(max(dot(n_in, n_in), 1e-20));
    if dot(n, frame.camera.xyz - p) < 0.0 { n = -n; }
    var sum = vec3<f32>(0.0);
    for (var i = 0u; i < count; i++) {
        let b = lamp_list[first + i];
        // Next-Gen car lights: tail / brake glows and underglows.
        if b.position.w > 1.5 {
            sum += car_glow_light(b, p, n);
            continue;
        }
        let rel = p - b.position.xyz;
        let w = dot(rel, b.direction.xyz);
        // `headlight_clip` is above 0 exactly where its texel position lies
        // between texels 32 and 48 (the clip texture's lit run, 33..47):
        // that test first, the texels only for the pixels the beam lights.
        let u = w * 0.005 + 0.5;
        let x = clamp(u, 0.0, 1.0) * 64.0 - 0.5;
        if !(x > 32.0 && x < 48.0) { continue; }
        let up = cross(b.shadow.xyz, b.direction.xyz);
        let uv = vec2<f32>(dot(rel, b.shadow.xyz) * b.direction.w, dot(rel, up) * b.colour.w) / w + 0.5;
        if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) { continue; }
        let clip = headlight_clip(u);
        let cookie = textureSampleLevel(headlight_tex, headlight_samp, uv, 0.0).rgb;
        let ndl = -dot(n, rel) * inverseSqrt(max(dot(rel, rel), 1e-6));
        // The player's beam is the native projector
        // (view 9 lights every face in its frustum at full cookie × clip ×
        // colour, no N·L): faces toward it take exactly that energy, only
        // faces turned away fade out. The road grazes the beam (N·L ~0.02..0.1),
        // where the others' lobe below took 10–15 % of the native pool.
        var facing = clamp(0.85 + 0.6 * ndl, 0.0, 1.0) * smoothstep(-0.15, 0.05, ndl);
        if b.position.w > 0.5 { facing = smoothstep(-0.1, 0.0, ndl); }
        var vis = select(others, 1.0, b.position.w > 0.5);
        if b.position.w > 0.5 && (D9_SHADOWS && frame.lamps.z > 0.5) {
            let layer = i32(frame.cluster_near.y);
            if layer >= 0 {
                vis *= mix(1.0, shadow_visibility(layer, p, vec3<f32>(0.0)), HEADLIGHT_SHADOW_DEPTH);
            }
            vis *= car_visibility(p, b.position.xyz, clusters[HEADLIGHT_CARS + cluster_at(p, screen)]);
        }
        sum += cookie * b.colour.rgb * (clip * facing * vis);
        if g_surface.on > 0.5 {
            let v = normalize(frame.camera.xyz - p);
            let l = -rel * inverseSqrt(max(dot(rel, rel), 1e-6));
            g_spec += cookie * b.colour.rgb * (clip * vis * surface_spec(g_surface.n, v, l, g_surface.rough, g_surface.f0, 0.1 * inverseSqrt(max(dot(rel, rel), 0.01))));
        }
    }
    return sum;
}

