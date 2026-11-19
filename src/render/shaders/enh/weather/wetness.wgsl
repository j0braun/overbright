// ---------------------------------------------------------------------
// One wetness (docs/notes/graphics/reflections.md "One wetness"):
// the scene's wetness and the puddle field every wet effect reads. CPU
// twin: render::enh::materials::world::wetness (`Wetness::of`) and
// ::puddle (`puddle_at`). Included by the world module (Wet Roads, the
// SSR mask) and the weather module (Rain Next-Gen's rings).

// The water film on the surfaces (0..1) for the rain's amount and the
// road's wetness from the rain: full from a rain of 0.6 (`FULL_RAIN`).
fn wetness_film(rain: f32, road: f32) -> f32 {
    return clamp(road, 0.0, 1.0) * (0.35 + 0.65 * min(max(rain, 0.0) / 0.6, 1.0));
}

// The puddles' share: they dry more slowly than the film.
fn wetness_puddles(film: f32) -> f32 {
    return sqrt(max(film, 0.0));
}

fn wetness_hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn wetness_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = p - i;
    let u = f * f * (3.0 - 2.0 * f);
    let a = wetness_hash12(i);
    let b = wetness_hash12(i + vec2<f32>(1.0, 0.0));
    let c = wetness_hash12(i + vec2<f32>(0.0, 1.0));
    let d = wetness_hash12(i + vec2<f32>(1.0, 1.0));
    return a + (b - a) * u.x + (c - a) * u.y + (a - b - c + d) * u.x * u.y;
}

// The puddle field's mask at world `p` (0..1, before the puddles' share):
// low-frequency patches, wider the wetter.
fn wetness_patch(p: vec2<f32>, film: f32) -> f32 {
    let field = 0.65 * wetness_noise(p * (1.0 / 7.0)) + 0.35 * wetness_noise(p * (1.0 / 2.3) + 17.0);
    let edge = 0.66 - 0.22 * film;
    return smoothstep(edge, edge + 0.08, field);
}

// The puddle at world `p` (the texture's reflectivity not applied).
fn wetness_puddle(p: vec2<f32>, film: f32) -> f32 {
    return wetness_patch(p, film) * wetness_puddles(film);
}

// The water film's reflectance (Schlick, water f0 0.02) at the view
// angle's cosine.
fn wetness_film_fresnel(ndotv: f32) -> f32 {
    let m = 1.0 - clamp(ndotv, 0.0, 1.0);
    let m2 = m * m;
    return 0.02 + 0.98 * m2 * m2 * m;
}
