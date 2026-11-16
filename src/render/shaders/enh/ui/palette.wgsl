// Colour-vision palette for the HUD (docs/modern-extras.md §4; clone
// deviation, opt-in): saturated hues the kind confuses moved to ones it
// tells apart (Okabe–Ito): 1 deuteranopia / 2 protanopia: reds to amber
// (protanopia brighter: reds look dark to it), greens to sky blue; 3
// tritanopia: blues to vermilion, yellows to pink. Greys, whites and weak
// colours stay; value and alpha are kept. `extras::palette_rgb` is the
// same on the CPU (tests).
fn hue_near(h: f32, c: f32, w: f32) -> f32 {
    let d = abs(fract(h - c + 0.5) - 0.5);
    return 1.0 - smoothstep(w * 0.6, w, d);
}

fn palette(c: vec3<f32>, kind: f32) -> vec3<f32> {
    let mx = max(c.r, max(c.g, c.b));
    let mn = min(c.r, min(c.g, c.b));
    let d = mx - mn;
    if (mx <= 0.0 || d / mx < 0.25) {
        return c;
    }
    var h: f32;
    if (mx == c.r) {
        h = fract((c.g - c.b) / d / 6.0 + 1.0);
    } else if (mx == c.g) {
        h = ((c.b - c.r) / d + 2.0) / 6.0;
    } else {
        h = ((c.r - c.g) / d + 4.0) / 6.0;
    }
    let s = d / mx;
    var v = mx;
    var to = h;
    var w = 0.0;
    if (kind < 2.5) {
        let red = hue_near(h, 0.0, 0.09);
        let green = hue_near(h, 0.33, 0.12);
        if (red >= green) {
            to = 0.11;
            w = red;
            if (kind > 1.5) {
                v = min(1.0, v * mix(1.0, 1.35, red));
            }
        } else {
            to = 0.56;
            w = green;
        }
    } else {
        let blue = hue_near(h, 0.62, 0.1);
        let yellow = hue_near(h, 0.15, 0.06);
        if (blue >= yellow) {
            to = 0.04;
            w = blue;
        } else {
            to = 0.88;
            w = yellow;
        }
    }
    let hh = fract(h + (fract(to - h + 0.5) - 0.5) * w);
    // HSV back to RGB.
    let k = fract(vec3<f32>(hh) + vec3<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0;
    return v * mix(vec3<f32>(1.0), clamp(abs(k) - 1.0, vec3<f32>(0.0), vec3<f32>(1.0)), s);
}

