use crate::*;

/// Particles High / Ultra (docs/notes/graphics/smoke.md "Particles High and Ultra: GPU particles and detailed smoke"): replay the ticks'
/// smoke births on the GPU and set the frame's view and lights; then the
/// native particles' quads, without the smoke when the GPU path draws it.
/// Original: the native quads, as before.
/// From the frame's camera and poses (the player's car as drawn).
/// The particles as the frame draws them between the ticks
/// (`FramePoses::particles`, the frame's own list: the drive's are not
/// written), the newborn ones scaled in.
#[allow(clippy::too_many_arguments)]
pub(crate) fn particle_quads(
    gpu: &Gpu,
    encoder: &mut wgpu::CommandEncoder,
    d: &mut Drive,
    camera: &Camera,
    poses: &render::snapshot::FramePoses,
    world_renderer: Option<&WorldRenderer>,
    pools: &[HeadlightPool],
    aspect: f32,
    fog_drawn: Option<nfsu2::fog::Fog>,
    opts: &nfsu2::render::RenderOptions,
    stepped: crate::render::frame::Stepped,
) -> Vec<nfsu2::particles::Quad> {
    let particles = poses.particles(&d.effects.system.particles);
    let scale = poses.particle_scale();
    let axes = CameraAxes::look(camera.forward(), camera.up);
    // The bound's scale is view +0x0C (`cull_scale`), not the screen's
    // pixels (an apitrace of the original's event markers: their rings
    // unclamped at 7 m across).
    let pixels_per_metre = nfsu2::render::cull_scale(camera.game_fov.unwrap_or(camera.fov_y));
    // The tiers the ticks recorded the births with.
    let row = stepped.particles;
    let cinematic = nfsu2::render::smoke_cinematic::active(row);
    // Cinematic replaces High / Ultra's path (which then drops its puffs).
    let tier = if cinematic { 0 } else { row };
    if cinematic || d.effects.cinematic.replaces() {
        let lamps = world_renderer.map(|r| nfsu2::render::volumetric::nearest_lamps(r.lamp_list(), camera.position, camera.forward(), 16)).unwrap_or_default();
        let view = nfsu2::render::smoke_cinematic::View {
            camera: *camera,
            aspect,
            reversed: opts.reversed_z,
            pixels_per_metre,
            lamps,
            cars: pools.iter().map(|p| (p.transform, p.color)).collect(),
            wet: d.weather.amount,
            modern: nfsu2::render::modern_light::enabled(opts),
            // The player's underglow lights the smoke (as R, G,
            // B; reading neon_color's B, G, R bytes as R, G, B was a bug).
            neons: if opts.car_lights_next_gen {
                // Every car's underglow (the frame's car lights).
                nfsu2::render::car_lights::cars(opts.car_lights_next_gen).iter().filter_map(|c| c.neon.map(|n| (c.transform, n))).collect()
            } else {
                nfsu2::shadow::neon_rgb(&d.car.neon, d.effects.cinematic.feeder.time as f32)
                    .filter(|_| d.car.texture_for(nfsu2::formats::hash::bin_hash_from(nfsu2::formats::hash::bin_hash(&d.car.name), "_NEON")).is_some())
                    .map(|c| (poses.player().transform(), c))
                    .into_iter()
                    .collect()
            },
            // The native fog as the world drew it this frame.
            fog: fog_drawn.map(|f| (f.value, f.color)),
            // The brake lights glow in the smoke (the tails glow through `neons`).
            points: nfsu2::render::car_lights::smoke_points(&nfsu2::render::car_lights::cars(opts.car_lights_next_gen)),
        };
        d.effects.cinematic.prepare(gpu, encoder, cinematic, &d.world.emitters, &d.effects.system.palettes, &d.world.textures, &view);
    }
    // Sparks & Fire: this frame's fragments, flames and lights; their
    // native quads are left out below.
    {
        let on = stepped.sparks_fire != 0;
        if on || d.effects.sparks_fire.active {
            let view = nfsu2::render::sparks_fire::View {
                projection: camera.projection_depth(aspect, opts.reversed_z),
                forward: camera.forward(),
                up: camera.up,
                eye: camera.position,
                near: camera.near,
                far: camera.far,
                reversed: opts.reversed_z,
                hdr: opts.hdr_active,
            };
            d.effects.sparks_fire.prepare(gpu, on, &d.world.emitters, particles, &view, poses.alpha);
        }
    }
    let sf = &d.effects.sparks_fire;
    if d.effects.cinematic.replaces() {
        let feeder = &d.effects.cinematic.feeder;
        return d.effects.system.quads_scaled_where(particles, scale, &d.world.emitters, camera.position, axes.right, -axes.down, axes.forward, pixels_per_metre, |p| !feeder.smoke(p.description as usize) && !sf.replaces(p));
    }
    if tier != 0 || d.effects.detail.replaces() {
        let lamps = world_renderer.map(|r| nfsu2::render::volumetric::nearest_lamps(r.lamp_list(), camera.position, camera.forward(), 16)).unwrap_or_default();
        let view = nfsu2::render::gpu_particles::View {
            camera: *camera,
            aspect,
            reversed: opts.reversed_z,
            pixels_per_metre,
            lamps,
            cars: pools.iter().map(|p| (p.transform, p.color)).collect(),
        };
        d.effects.detail.prepare(gpu, encoder, tier, &d.world.emitters, &d.effects.system.palettes, &d.world.textures, &view);
    }
    if d.effects.detail.replaces() {
        let feeder = &d.effects.detail.feeder;
        d.effects.system.quads_scaled_where(particles, scale, &d.world.emitters, camera.position, axes.right, -axes.down, axes.forward, pixels_per_metre, |p| !feeder.smoke(p.description as usize) && !sf.replaces(p))
    } else if sf.active {
        d.effects.system.quads_scaled_where(particles, scale, &d.world.emitters, camera.position, axes.right, -axes.down, axes.forward, pixels_per_metre, |p| !sf.replaces(p))
    } else {
        d.effects.system.quads_scaled_where(particles, scale, &d.world.emitters, camera.position, axes.right, -axes.down, axes.forward, pixels_per_metre, |_| true)
    }
}
