// SMAA 1x (the Graphics page's Anti-Aliasing, a clone deviation; see
// smaa.rs and enhance.rs): Jimenez et al.'s three passes on the scene's
// image after the native post passes — luma edges, blending weights from
// the orthogonal area table (no diagonal search), neighbourhood blending.
// The searches step a pixel at a time with loads (the original's bilinear
// two-pixel steps and search table give the same ends).

struct Pass {
    // xy: 1 / the image's size; zw: its size.
    texel: vec4<f32>,
    params: vec4<f32>,
}

@group(0) @binding(0) var tex_a: texture_2d<f32>;
@group(0) @binding(1) var tex_b: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
@group(0) @binding(3) var<uniform> pass_: Pass;

const THRESHOLD: f32 = 0.1;
const CONTRAST_ADAPTATION: f32 = 2.0;
const MAX_SEARCH: i32 = 32;
const CORNER_ROUNDING: f32 = 0.25;
const AREA_SLOT: f32 = 16.0;
const AREA_SIDE: f32 = 80.0;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: VsOut;
    out.position = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(p.x, 1.0 - p.y);
    return out;
}

fn size() -> vec2<i32> {
    return vec2<i32>(pass_.texel.zw);
}

fn load_a(p: vec2<i32>) -> vec4<f32> {
    return textureLoad(tex_a, clamp(p, vec2<i32>(0), size() - 1), 0);
}

fn luma_at(p: vec2<i32>) -> f32 {
    return dot(load_a(p).rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// Pass 1: r = an edge on the pixel's left, g = on its top.
@fragment
fn fs_edges(in: VsOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.position.xy);
    let l = luma_at(p);
    let left = luma_at(p + vec2<i32>(-1, 0));
    let top = luma_at(p + vec2<i32>(0, -1));
    var delta = abs(vec2<f32>(l) - vec2<f32>(left, top));
    var edges = step(vec2<f32>(THRESHOLD), delta);
    if dot(edges, vec2<f32>(1.0)) == 0.0 {
        discard;
    }
    let right = luma_at(p + vec2<i32>(1, 0));
    let bottom = luma_at(p + vec2<i32>(0, 1));
    var max_delta = max(delta, abs(vec2<f32>(l) - vec2<f32>(right, bottom)));
    let leftleft = luma_at(p + vec2<i32>(-2, 0));
    let toptop = luma_at(p + vec2<i32>(0, -2));
    max_delta = max(max_delta, abs(vec2<f32>(left, top) - vec2<f32>(leftleft, toptop)));
    let final_delta = max(max_delta.x, max_delta.y);
    edges = edges * step(vec2<f32>(final_delta), CONTRAST_ADAPTATION * delta);
    return vec4<f32>(edges, 0.0, 0.0);
}

fn edge(p: vec2<i32>) -> vec2<f32> {
    let s = size();
    if any(p < vec2<i32>(0)) || any(p >= s) {
        return vec2<f32>(0.0);
    }
    return textureLoad(tex_a, p, 0).rg;
}

// The area table's two weights for the distances `d` to the line's ends
// and the crossing edges `e` (0, 1, 3 or 4 each).
fn area(d: vec2<f32>, e: vec2<f32>) -> vec2<f32> {
    let t = AREA_SLOT * e + sqrt(d);
    return textureSampleLevel(tex_b, samp, (t + 0.5) / AREA_SIDE, 0.0).rg;
}

// Pass 2: the blending weights (rg: a horizontal edge on top, ba: a
// vertical one on the left).
@fragment
fn fs_weights(in: VsOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.position.xy);
    let e = edge(p);
    var weights = vec4<f32>(0.0);
    if e.g > 0.0 {
        // Left end: stop at a crossing edge on a pixel's left (this row
        // or the one above) or where the top edge ends.
        var xl = 0;
        for (var i = 0; i < MAX_SEARCH; i++) {
            let q = p - vec2<i32>(xl, 0);
            if edge(q).r > 0.0 || edge(q + vec2<i32>(0, -1)).r > 0.0 || edge(q + vec2<i32>(-1, 0)).g == 0.0 {
                break;
            }
            xl += 1;
        }
        var xr = 0;
        for (var i = 0; i < MAX_SEARCH; i++) {
            let q = p + vec2<i32>(xr, 0);
            if edge(q + vec2<i32>(1, 0)).r > 0.0 || edge(q + vec2<i32>(1, -1)).r > 0.0 || edge(q + vec2<i32>(1, 0)).g == 0.0 {
                break;
            }
            xr += 1;
        }
        let le = p - vec2<i32>(xl, 0);
        let re = p + vec2<i32>(xr, 0);
        let e1 = 3.0 * edge(le).r + edge(le + vec2<i32>(0, -1)).r;
        let e2 = 3.0 * edge(re + vec2<i32>(1, 0)).r + edge(re + vec2<i32>(1, -1)).r;
        let d = vec2<f32>(f32(xl), f32(xr));
        var w = area(d, vec2<f32>(e1, e2));
        // Corners (SMAADetectHorizontalCornerPattern).
        let lr = step(d.xy, d.yx);
        let rounding = (1.0 - CORNER_ROUNDING) * lr / (lr.x + lr.y);
        var factor = vec2<f32>(1.0);
        factor.x -= rounding.x * edge(le + vec2<i32>(0, 1)).r;
        factor.x -= rounding.y * edge(re + vec2<i32>(1, 1)).r;
        factor.y -= rounding.x * edge(le + vec2<i32>(0, -2)).r;
        factor.y -= rounding.y * edge(re + vec2<i32>(1, -2)).r;
        w *= clamp(factor, vec2<f32>(0.0), vec2<f32>(1.0));
        weights = vec4<f32>(w, weights.zw);
    }
    if e.r > 0.0 {
        var yu = 0;
        for (var i = 0; i < MAX_SEARCH; i++) {
            let q = p - vec2<i32>(0, yu);
            if edge(q).g > 0.0 || edge(q + vec2<i32>(-1, 0)).g > 0.0 || edge(q + vec2<i32>(0, -1)).r == 0.0 {
                break;
            }
            yu += 1;
        }
        var yd = 0;
        for (var i = 0; i < MAX_SEARCH; i++) {
            let q = p + vec2<i32>(0, yd);
            if edge(q + vec2<i32>(0, 1)).g > 0.0 || edge(q + vec2<i32>(-1, 1)).g > 0.0 || edge(q + vec2<i32>(0, 1)).r == 0.0 {
                break;
            }
            yd += 1;
        }
        let ue = p - vec2<i32>(0, yu);
        let de = p + vec2<i32>(0, yd);
        let e1 = 3.0 * edge(ue).g + edge(ue + vec2<i32>(-1, 0)).g;
        let e2 = 3.0 * edge(de + vec2<i32>(0, 1)).g + edge(de + vec2<i32>(-1, 1)).g;
        let d = vec2<f32>(f32(yu), f32(yd));
        var w = area(d, vec2<f32>(e1, e2));
        // Corners (SMAADetectVerticalCornerPattern).
        let lr = step(d.xy, d.yx);
        let rounding = (1.0 - CORNER_ROUNDING) * lr / (lr.x + lr.y);
        var factor = vec2<f32>(1.0);
        factor.x -= rounding.x * edge(ue + vec2<i32>(1, 0)).g;
        factor.x -= rounding.y * edge(de + vec2<i32>(1, 1)).g;
        factor.y -= rounding.x * edge(ue + vec2<i32>(-2, 0)).g;
        factor.y -= rounding.y * edge(de + vec2<i32>(-2, 1)).g;
        w *= clamp(factor, vec2<f32>(0.0), vec2<f32>(1.0));
        weights = vec4<f32>(weights.xy, w);
    }
    return weights;
}

fn weights_at(p: vec2<i32>) -> vec4<f32> {
    return textureLoad(tex_b, clamp(p, vec2<i32>(0), size() - 1), 0);
}

// Pass 3: each pixel blended with its neighbours by the weights of the
// edges around it (SMAANeighborhoodBlendingPS). The alpha (the UI's
// destination-alpha mask) stays the pixel's own.
@fragment
fn fs_blend(in: VsOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.position.xy);
    let centre = load_a(p);
    var a: vec4<f32>;
    a.x = weights_at(p + vec2<i32>(1, 0)).a;
    a.y = weights_at(p + vec2<i32>(0, 1)).g;
    let own = weights_at(p);
    a.w = own.r;
    a.z = own.b;
    if dot(a, vec4<f32>(1.0)) < 1e-5 {
        return centre;
    }
    let uv = (vec2<f32>(p) + 0.5) * pass_.texel.xy;
    let h = max(a.x, a.z) > max(a.y, a.w);
    var offset: vec4<f32>;
    var weight: vec2<f32>;
    if h {
        offset = vec4<f32>(a.x, 0.0, a.z, 0.0);
        weight = a.xz;
    } else {
        offset = vec4<f32>(0.0, a.y, 0.0, a.w);
        weight = a.yw;
    }
    weight = weight / dot(weight, vec2<f32>(1.0));
    let c1 = uv + offset.xy * pass_.texel.xy;
    let c2 = uv - offset.zw * pass_.texel.xy;
    let rgb = weight.x * textureSampleLevel(tex_a, samp, c1, 0.0).rgb + weight.y * textureSampleLevel(tex_a, samp, c2, 0.0).rgb;
    return vec4<f32>(rgb, centre.a);
}
