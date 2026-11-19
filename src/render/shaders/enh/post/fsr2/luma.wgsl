//!pass luma
// The shading-change luma: exp of the mean log luma of each block
// (u.misc.w px a side, a multiple of 8) of the input. FSR 2 takes 32 x 32
// (its SPD mip 4); the clone uses a smaller block (docs/notes/graphics/fsr2.md "Locks":
// the game's animated signs are not flagged reactive).
@group(0) @binding(2) var input_color: texture_2d<f32>;
@group(0) @binding(3) var out_luma: texture_storage_2d<rgba16float, write>;

var<workgroup> partial: array<f32, 64>;

@compute @workgroup_size(8, 8)
fn main(@builtin(workgroup_id) wg: vec3<u32>, @builtin(local_invocation_index) li: u32, @builtin(local_invocation_id) lid: vec3<u32>) {
    let size = vec2<i32>(render_size());
    let block = i32(u.misc.w);
    let per = block / 8;
    let base = vec2<i32>(wg.xy) * block + vec2<i32>(lid.xy) * per;
    var sum = 0.0;
    for (var y = 0; y < per; y++) {
        for (var x = 0; x < per; x++) {
            let s = clamp_load(base + vec2<i32>(x, y), size);
            let rgb = textureLoad(input_color, s, 0).rgb;
            sum += log(max(EPSILON, rgb_to_luma(rgb)));
        }
    }
    partial[li] = sum / f32(per * per);
    workgroupBarrier();
    var n = 32u;
    loop {
        if li < n {
            partial[li] += partial[li + n];
        }
        workgroupBarrier();
        if n == 1u {
            break;
        }
        n = n / 2u;
    }
    if li == 0u {
        let p = vec2<i32>(wg.xy);
        if on_screen(p, vec2<i32>(u.luma.xy)) {
            textureStore(out_luma, p, vec4<f32>(partial[0] / 64.0, 0.0, 0.0, 0.0));
        }
    }
}

