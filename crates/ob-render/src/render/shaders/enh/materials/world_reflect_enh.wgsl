// Optional clone shading of the native capture, not a new planar camera,
// SSR or ray tracing. A triangle normal follows each receiving surface,
// including slopes and bridges. The bounded spatial kernel softens aliasing
// without history. Authored alpha still determines where reflections exist.
fn filtered_road_reflection(screen: vec2<f32>, centre: vec3<f32>, mask: f32, ndotv: f32) -> vec3<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(reflected_tex));
    // Keep wet/grazing highlights relatively crisp. Drier surfaces have
    // a wider footprint, at most 1.25 capture texels in each direction.
    let radius = mix(1.25, 0.45, clamp(mask, 0.0, 1.0)) * mix(0.65, 1.0, ndotv);
    let d = texel * radius;
    var sum = centre * 4.0;
    sum += textureSampleLevel(reflected_tex, reflected_samp, screen + vec2<f32>(d.x, 0.0), 0.0).rgb * 2.0;
    sum += textureSampleLevel(reflected_tex, reflected_samp, screen - vec2<f32>(d.x, 0.0), 0.0).rgb * 2.0;
    sum += textureSampleLevel(reflected_tex, reflected_samp, screen + vec2<f32>(0.0, d.y), 0.0).rgb * 2.0;
    sum += textureSampleLevel(reflected_tex, reflected_samp, screen - vec2<f32>(0.0, d.y), 0.0).rgb * 2.0;
    sum += textureSampleLevel(reflected_tex, reflected_samp, screen + d, 0.0).rgb;
    sum += textureSampleLevel(reflected_tex, reflected_samp, screen - d, 0.0).rgb;
    sum += textureSampleLevel(reflected_tex, reflected_samp, screen + vec2<f32>(d.x, -d.y), 0.0).rgb;
    sum += textureSampleLevel(reflected_tex, reflected_samp, screen + vec2<f32>(-d.x, d.y), 0.0).rgb;
    // Artistic Schlick weight: retain a 35% floor for the game's baked
    // lighting, approaching the native intensity at grazing angles.
    let fresnel = 0.35 + 0.65 * pow(1.0 - ndotv, 5.0);
    return sum * (fresnel / 16.0);
}

