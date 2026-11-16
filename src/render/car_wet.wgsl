
// Wet Car (clone deviation, off by default; docs/weather.md). Appended to
// car.wgsl (and after car_paint.wgsl when Enhanced Car Paint is on) for the
// wet pipelines only: `CarShared::module` builds them, and a car is drawn
// with them only while its water film is above zero (`wet_car::WET_MIN`).
// A dry car keeps the native / paint pipelines, so a dry car is the same
// image bit for bit (the R2 lesson, docs/car-raindrops.md).
//
// draw.wet[0] = (film, rain now, class, world clock s)
// draw.wet[1] = (the car's velocity in its own axes m/s, spray film)
// draw.wet[2] = (hood start x, sill z, next-gen rain 0/1, 0) in the car's axes
// class: 0 none (lights, decals, interior), 1 paint, 2 glass, 3 tyre, 4 trim.
//
// Built from car.efx's own terms: the colour in is the native (or paint)
// result; the layer takes off part of the diffuse (a wet surface is
// darker), then adds a thin water film's reflection of the same env cube
// (Fresnel of water, F0 0.02) and a sharp highlight of the harmonic's
// dominant light, both along the water surface's normal: beads (a dome per
// drop) and rivulets (streaks running back with the speed, down with
// gravity). Drops and rivulets are filtered to their mean once smaller
// than a pixel, so nothing sparkles at a distance.

fn wet_hash(p: vec3<f32>) -> vec3<f32> {
    var q = vec3<u32>(vec3<i32>(p) + vec3<i32>(1 << 20));
    q = q * vec3<u32>(1664525u, 1013904223u, 2654435761u);
    q.x += q.y * q.z;
    q.y += q.z * q.x;
    q.z += q.x * q.y;
    q = q ^ (q >> vec3<u32>(16u));
    q.x += q.y * q.z;
    q.y += q.z * q.x;
    q.z += q.x * q.y;
    return vec3<f32>(q & vec3<u32>(0xFFFFu)) / 65535.0;
}

fn wet_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = p - i;
    let u = f * f * (3.0 - 2.0 * f);
    let a = wet_hash(vec3<f32>(i, 7.0)).x;
    let b = wet_hash(vec3<f32>(i + vec2<f32>(1.0, 0.0), 7.0)).x;
    let c = wet_hash(vec3<f32>(i + vec2<f32>(0.0, 1.0), 7.0)).x;
    let d = wet_hash(vec3<f32>(i + vec2<f32>(1.0, 1.0), 7.0)).x;
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Bead cell edge (m): drops of 2..5 mm radius on waxed paint.
const BEAD_CELL: f32 = 0.016;
// Rivulet spacing across the flow (m) and their length scale along it.
const RIVULET_ACROSS: f32 = 0.011;
const RIVULET_ALONG: f32 = 0.16;

// car.wgsl's final combine in the wet module: `colour` is the native (or
// Enhanced Car Paint) result. Derivatives first, in uniform control flow.
fn wet_layer(in: VertexOut, colour: vec3<f32>, r1: vec4<f32>, native_direction: vec3<f32>) -> vec3<f32> {
    let footprint = max(length(dpdx(in.local_position)), length(dpdy(in.local_position)));
    let normal_spread = fwidth(in.local_normal);
    let w0 = draw.wet[0];
    if w0.z < 0.5 || max(w0.x, draw.wet[1].w) <= 0.0 {
        return colour;
    }
    return wet_surface(in, colour, r1, native_direction, footprint, normal_spread);
}

fn wet_env(reflected: vec3<f32>) -> vec3<f32> {
    var dir: vec3<f32>;
    if draw.harmonic_scale.y > 0.5 && draw.harmonic_scale.z > 0.5 {
        dir = (draw.world * vec4<f32>(reflected, 0.0)).xyz;
    } else {
        let rv = (frame.view * draw.world * vec4<f32>(reflected, 0.0)).xyz;
        dir = vec3<f32>(rv.x, -rv.y, -rv.z);
    }
    return textureSampleLevel(envmap, envmap_sampler, dir, 0.0).rgb;
}

fn wet_surface(in: VertexOut, colour: vec3<f32>, r1: vec4<f32>, native_direction: vec3<f32>, footprint: f32, normal_spread: vec3<f32>) -> vec3<f32> {
    let w0 = draw.wet[0];
    let w1 = draw.wet[1];
    let w2 = draw.wet[2];
    let kind = u32(w0.z + 0.5);
    let clock = w0.w;
    let p = in.local_position;
    let n = in.local_normal * inverseSqrt(max(dot(in.local_normal, in.local_normal), 1e-12));
    let eye = draw.local_eye.xyz - p;
    let v = eye * inverseSqrt(max(dot(eye, eye), 1e-12));
    // World up in the draw's axes (the part's matrix is a rotation).
    let rot = mat3x3<f32>(draw.world[0].xyz, draw.world[1].xyz, draw.world[2].xyz);
    let up = normalize(transpose(rot) * vec3<f32>(0.0, 0.0, 1.0));
    let facing_up = dot(n, up);

    // Where the water is: the film everywhere, the spray from the wheels on
    // the lower body (all of a tyre), and the hood over the engine dries
    // faster (film², the same film is lower there as it dries).
    var film = w0.x;
    var lower = 1.0;
    if kind != 3u {
        lower = 1.0 - smoothstep(w2.y, w2.y + 0.35, p.z);
        let hood = smoothstep(w2.x - 0.3, w2.x + 0.1, p.x) * smoothstep(0.35, 0.75, facing_up);
        film = mix(film, film * film, hood);
    }
    film = clamp(max(film, w1.w * lower), 0.0, 1.0);
    if film <= 0.001 {
        return colour;
    }

    // Darker while wet: part of the diffuse taken off (water fills the
    // surface's micro roughness; less light scattered back).
    var dark = 0.0;
    var film_gain = 0.0;
    var beads = 0.0;
    switch kind {
        case 1u: { dark = 0.3; film_gain = 0.3; beads = 1.0; }
        case 2u: { dark = 0.0; film_gain = 0.4; beads = 0.8; }
        case 3u: { dark = 0.45; film_gain = 0.45; beads = 0.0; }
        default: { dark = 0.3; film_gain = 0.35; beads = 0.0; }
    }
    var base = max(colour - r1.xyz * (dark * film), vec3<f32>(0.0));

    // The water's flow on the surface: back along the car with the air at
    // speed, down with gravity, in the surface's plane.
    let vel = w1.xyz;
    let speed = length(vel);
    let air = -vel / max(speed, 1e-3) * smoothstep(3.0, 25.0, speed) * 1.6;
    let flow3 = air - up * 0.6;
    let ft = flow3 - n * dot(n, flow3);
    let flow_len = length(ft);
    let fdir = ft / max(flow_len, 1e-4);
    let across_dir = cross(n, fdir);
    let along = dot(p, fdir);
    let across = dot(p, across_dir);
    let flow_speed = 0.25 + 0.06 * speed;
    // Rivulets: thin lines along the flow, moving with it.
    let lines = wet_noise(vec2<f32>(across / RIVULET_ACROSS, (along - clock * flow_speed) / RIVULET_ALONG));
    let rivulet_detail = 1.0 - smoothstep(RIVULET_ACROSS * 0.25, RIVULET_ACROSS * 0.8, footprint);
    let rivulet_mean = 0.12;
    let rivulet = mix(rivulet_mean, smoothstep(0.72, 0.9, lines), rivulet_detail) * clamp(flow_len, 0.0, 1.0) * smoothstep(0.25, 0.7, film);

    // Beads (waxed paint, glass): a dome per drop, each cell's drop there
    // while the film covers it, blown off the front and top at speed.
    var ns = n;
    var bead_mask = 0.0;
    var rim = 0.0;
    let bead_detail = 1.0 - smoothstep(0.002, 0.005, footprint);
    if beads > 0.0 && bead_detail > 0.0 {
        let vn = vel / max(speed, 1e-3);
        let blow = smoothstep(12.0, 32.0, speed) * (0.45 + 0.55 * max(dot(n, vn), max(facing_up, 0.0)));
        let cover = beads * pow(film, 0.7) * (1.0 - blow) * (1.0 - 0.6 * rivulet);
        let q = p / BEAD_CELL;
        let c = floor(q);
        // The cell and its neighbours along the surface's main axes (drops
        // near a cell edge).
        for (var k = 0; k < 4; k = k + 1) {
            let off = vec3<f32>(f32(k & 1), f32((k >> 1) & 1), 0.0) - vec3<f32>(0.5);
            let cell = floor(q + off * 0.999) ;
            let r = wet_hash(cell);
            if r.x >= cover {
                continue;
            }
            let centre = (cell + 0.2 + 0.6 * wet_hash(cell + vec3<f32>(31.0)).xyz) * BEAD_CELL;
            let radius = BEAD_CELL * (0.17 + 0.2 * r.z);
            let d = p - centre;
            let dt = d - n * dot(d, n);
            let s = length(dt) / radius;
            // A cell's drop sits on the surface only where its centre is
            // close to it (3D cells cut by the panel).
            let near = 1.0 - smoothstep(radius * 0.6, radius * 1.4, abs(dot(d, n)));
            if s < 1.0 && near > 0.0 {
                let h = sqrt(max(1.0 - s * s, 0.0));
                let slope = dt / radius / max(h, 0.25);
                ns = normalize(n + slope * 0.85 * near);
                bead_mask = max(bead_mask, near);
                rim = max(rim, smoothstep(0.7, 1.0, s) * near);
            }
        }
        bead_mask *= bead_detail;
        rim *= bead_detail;
        ns = normalize(mix(n, ns, bead_detail));
    }
    // Rivulets ripple the film's normal a little.
    ns = normalize(ns + fdir * (rivulet - rivulet_mean) * 0.15 * rivulet_detail);
    // Rain Detail Next-Gen: the falling rain hitting the car.
    var impact = 0.0;
    if w2.z > 0.5 && w0.y > 0.0 {
        let r = rain_on_car(p, n, up, facing_up, fdir, across_dir, along, across, clock, speed, w0.y, film, kind, footprint);
        ns = normalize(ns + r.xyz);
        impact = r.w;
    }

    // The water film's reflection (Fresnel of water) and its sharp
    // highlight of the dominant light, along the water's normal.
    let ndv = clamp(dot(v, ns), 0.01, 1.0);
    let reflected = 2.0 * ndv * ns - v;
    let env = wet_env(reflected);
    let fresnel = 0.02 + 0.98 * pow(1.0 - ndv, 5.0);
    let cover = clamp(film * 1.25, 0.0, 1.0) * (0.7 + 0.3 * rivulet) * film_gain;
    let water = 1.0 + 2.0 * bead_mask;
    var add = 2.0 * env * fresnel * cover * water * (1.0 + 1.5 * impact);

    let h = draw.harmonic;
    let lin = vec3<f32>(h[7].w, h[8].w, h[9].w);
    let has_light = select(0.0, 1.0, dot(lin, lin) > 1e-8);
    let light_dir = lin * inverseSqrt(max(dot(lin, lin), 1e-12));
    let light = max(harmonic(light_dir), vec4<f32>(0.0));
    let spread = dot(normal_spread, normal_spread);
    let power_max = select(300.0, 60.0, kind == 3u);
    let power = power_max / (1.0 + power_max * spread * 4.0);
    let norm = (power + 2.0) / (power_max + 2.0);
    let lobe = pow(max(dot(reflected, light_dir), 0.0), power) * norm * has_light;
    add += lobe * light.xyz * cover * (0.35 + 0.9 * bead_mask);

    // A drop's rim refracts the darker surroundings.
    base *= 1.0 - 0.25 * rim * film_gain;
    // Glass is drawn blended at its alpha: the water on it is not
    // see-through, so its light is not scaled down with the glass's.
    if kind == 2u {
        add /= clamp(r1.w, 0.25, 1.0);
    }
    return base + add;
}

// Next-Gen (Rain Detail "Next-Gen", docs/rain-nextgen.md): the rain
// falling on the car. Returns the water surface's normal offset (xyz) and
// the fresh impacts' brightness (w).
// * Impacts: on the surfaces facing up (roof, bonnet, boot, windshield),
//   a capillary ring per 5 cm cell at a random place and time, at a rate
//   by the rain; filtered to nothing once a ring is under ~3 px.
// * Sheeting on glass: the water film flows in sheets along the flow
//   (back with the air, down with gravity), a moving wave field that bends
//   what the glass reflects; filtered to its mean under a pixel.
const IMPACT_CELL: f32 = 0.05;

fn rain_on_car(p: vec3<f32>, n: vec3<f32>, up: vec3<f32>, facing_up: f32, fdir: vec3<f32>, across_dir: vec3<f32>, along: f32, across: f32, clock: f32, speed: f32, rain: f32, film: f32, kind: u32, footprint: f32) -> vec4<f32> {
    var off = vec3<f32>(0.0);
    var flash = 0.0;
    // Tangent axes of the surface (any pair; the cells are 2D on it).
    let ta = normalize(select(cross(n, vec3<f32>(0.0, 1.0, 0.0)), cross(n, vec3<f32>(1.0, 0.0, 0.0)), abs(n.y) > 0.9));
    let tb = cross(n, ta);
    let uv = vec2<f32>(dot(p, ta), dot(p, tb)) / IMPACT_CELL;
    let detail = 1.0 - smoothstep(IMPACT_CELL * 0.06, IMPACT_CELL * 0.2, footprint);
    let top = smoothstep(0.25, 0.6, facing_up);
    if detail > 0.0 && top > 0.0 && (kind == 1u || kind == 2u || kind == 4u) {
        for (var k = 0; k < 2; k = k + 1) {
            let g = uv + f32(k) * vec2<f32>(0.5, 0.31);
            let c = floor(g);
            let h = wet_hash(vec3<f32>(c, 11.0 + f32(k)));
            // A ring a cell about every 0.6 s in heavy rain.
            let rate = 0.9 + 0.8 * h.z;
            let life = fract(clock * rate * (0.3 + 0.7 * rain) + h.x);
            if h.y > 0.25 + 0.75 * rain {
                continue;
            }
            let centre = c + 0.25 + 0.5 * wet_hash(vec3<f32>(c, 23.0 + f32(k))).xy;
            let d = (g - centre) * IMPACT_CELL;
            let r = length(d);
            let ring = (0.004 + 0.022 * sqrt(life));
            let x = (r - ring) / 0.0035;
            let wave = sin(x * 3.14159) * exp(-x * x * 0.4) * (1.0 - life) * (1.0 - life);
            let dir2 = d / max(r, 1e-5);
            off += (ta * dir2.x + tb * dir2.y) * wave * 0.35;
            flash = max(flash, exp(-r * r / 0.00002) * smoothstep(0.08, 0.0, life));
        }
        off *= detail * top;
        flash *= detail * top;
    }
    if kind == 2u {
        // Sheets: waves across the flow, moving with it; faster with speed.
        let flow_speed = 0.35 + 0.05 * speed;
        let wv = along * 28.0 - clock * flow_speed * 28.0 + sin(across * 45.0 + clock * 0.7) * 1.6;
        let sheet_detail = 1.0 - smoothstep(0.004, 0.012, footprint);
        let amp = 0.06 * smoothstep(0.2, 0.7, film) * sheet_detail;
        off += fdir * sin(wv) * amp + across_dir * cos(wv * 0.7 + across * 30.0) * amp * 0.5;
    }
    return vec4<f32>(off, flash * 0.6);
}
