
// The HUD quads under a palette (their own pipelines, `UiRenderer`):
// the same shading, then `palette`. fs_main stays the native shader as
// it was, untouched.
@fragment
fn fs_palette(in: VsOut) -> @location(0) vec4<f32> {
    let kind = floor(in.clip.z / 16.0);
    var o = shade(in, in.clip.z - kind * 16.0);
    if (kind > 0.5) {
        o = vec4<f32>(palette(o.rgb, kind), o.a);
    }
    return o;
}

fn shade(in: VsOut, mode_in: f32) -> vec4<f32> {
    // fe.efx's sampling (bilinear, wrap, mipmaps), what the game does.
    var t = textureSample(tex, samp, in.uv);
    // Target pixels per texel on each axis (for HD UI Art).
    let x = in.uv * vec2<f32>(textureDimensions(tex, 0));
    let ppt = 1.0 / max(vec2<f32>(length(vec2<f32>(dpdx(x.x), dpdy(x.x))), length(vec2<f32>(dpdx(x.y), dpdy(x.y)))), vec2<f32>(1e-6));
    var mode = mode_in;
    if (mode > 3.5) {
        let premultiplied = mode > 7.5;
        mode -= select(4.0, 8.0, premultiplied);
        if (min(ppt.x, ppt.y) > 1.0) {
            t = sharp(in.uv, ppt, premultiplied);
        }
    }
    // The texture's alpha test (entry byte 0x55): ALPHAFUNC GREATER,
    // ALPHAREF 95, on fe.efx's output alpha (texture × vertex).
    if (mode > 1.5) {
        let o = t * in.color;
        if (o.a * 255.0 <= 95.0) {
            discard;
        }
        return o;
    }
    if (mode > 0.5) {
        // Destination-alpha texture inside the mask's circle: opaque there.
        let r = length(in.clip.xy);
        let edge = clamp((1.0 - r) * 40.0, 0.0, 1.0);
        return vec4<f32>(t.rgb * in.color.rgb, in.color.a * edge);
    }
    return t * in.color;
}
