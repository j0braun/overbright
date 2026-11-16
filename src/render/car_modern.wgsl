// ---- Modern Lighting for the cars (render/modern_light_bold.rs
// `car_source`): appended to a car module only while the Lighting row is
// on; the native car.wgsl never holds it. ----
// Modern Lighting (clone deviation, docs/modern-lighting.md): the
// draw's four lights as the world's (modern_light_bold::light_pool: a
// street lamp the fitted pool, neon / a car's light 1 at its source), as
// diffuse through the material's diffuse scale, and a highlight.
var<private> g_car_modern_spec: vec3<f32>;
struct CarModern {
    diffuse: vec3<f32>,
    spec: vec3<f32>,
};
fn car_modern_ambient() -> f32 {
    let w = -draw.lamps[1].w;
    return select(w, w - 10.0, w > 5.0);
}
const CAR_MODERN_SOFT: f32 = 80.0;
fn car_modern(in: VertexOut) -> CarModern {
    let p = (draw.world * vec4<f32>(in.local_position, 1.0)).xyz;
    let nw = (draw.world * vec4<f32>(in.local_normal, 0.0)).xyz;
    let n = nw * inverseSqrt(max(dot(nw, nw), 1e-12));
    // local_eye: the eye's direction in the car's axes (× 2.78, 0x48F660).
    let v = normalize((draw.world * vec4<f32>(draw.local_eye.xyz, 0.0)).xyz);
    var out = CarModern(vec3<f32>(0.0), vec3<f32>(0.0));
    for (var i = 0u; i < 4u; i++) {
        let l = draw.lamps[i * 2u];
        let c = draw.lamps[i * 2u + 1u];
        if l.w <= 0.01 { break; }
        let d = l.xyz - p;
        let d2 = dot(d, d);
        let o2 = l.w * l.w;
        if d2 >= o2 { continue; }
        let dir = d * inverseSqrt(max(d2, 1e-12));
        let x = d2 / o2;
        let window = (1.0 - x * x) * (1.0 - x * x);
        // The kind: w below −5 a light of its own (neon, a car's).
        var falloff = (81.0 + CAR_MODERN_SOFT) / (d2 + CAR_MODERN_SOFT);
        if c.w < -5.0 {
            let s = o2 * 0.0625;
            falloff = s / (d2 + s);
        }
        let e = c.rgb * (falloff * window);
        out.diffuse += e * max(dot(n, dir), 0.0);
        let h = normalize(dir + v);
        out.spec += e * (pow(max(dot(n, h), 0.0), 48.0) * 0.6 * max(dot(n, dir), 0.0));
    }
    out.diffuse *= (draw.diffuse_min.xyz + draw.diffuse_range.xyz) * 0.5;
    return out;
}
