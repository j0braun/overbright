use crate::*;

/// The cars in the rear-view mirror (World_DrawCars 0x6316E0 with view
/// 3, into its target after the world): the race's rivals, the free-roam
/// encounter racers and the traffic, each at the LOD its size in view 3
/// gives; the player's own car is left out (0x44BA39 sets the view's
/// camera +0x280 to it). **Not ported**: the per-car view mask (car
/// +0x5B4 bit 3, writer not traced; taken as set) and 0x60D500's test,
/// and what view 3 draws after the cars but its frame (particles
/// 0x48BDF0, glows 0x61AEA0 / 0x491860).
#[allow(clippy::too_many_arguments)]
pub(crate) fn rear_view_cars(
    gpu: &Gpu,
    encoder: &mut wgpu::CommandEncoder,
    color: &wgpu::TextureView,
    depth: &wgpu::TextureView,
    camera: &nfsu2::render::camera::Camera,
    settings: &options::Settings,
    world: &World,
    env: &wgpu::TextureView,
    renderers: &mut render::state::Keyed<CarRenderer>,
    cars: Vec<(&CarId, &CarState, &Car, LodDraw)>,
    opts: &nfsu2::render::RenderOptions,
) {
    let vp = camera.view_projection(1.0);
    let view = camera.view();
    // The world's lights for the mirror's camera, once for all its cars
    // (`CarLighting::in_world` per car, a performance audit item).
    let lights = CarLighting::world_lights(world, camera, opts);
    for (id, state, car, lod) in cars {
        let cr = renderers.get_mut(id).expect("listed with its renderer");
        // A car the mirror's LOD pick leaves out draws nothing: no
        // lighting, upload or pass for it (a performance audit item). Its LOD is
        // still set, as before.
        let lod = car_lod(camera, settings, state.position, lod, opts);
        cr.set_lod(lod);
        if lod.is_none() {
            continue;
        }
        cr.prepare_reflection(gpu);
        cr.set_world_environment(gpu, env);
        cr.render_reflection(
            gpu,
            encoder,
            color,
            depth,
            vp,
            view,
            &lights.car(state.transform()),
            state.transform(),
            &state.wheel_poses(car),
            opts,
        );
    }
}

/// The cars a race or free roam has on the road besides the player's, as
/// the frame draws them: a race's rivals and its traffic (not while
/// paused), free roam's encounter racers and traffic; each as the frame
/// draws it (`poses`).
pub(crate) fn rear_view_list<'a>(
    poses: &'a render::snapshot::FramePoses,
    mode: &'a Mode,
    encounters: Option<&'a Encounters>,
    traffic: Option<&'a Traffic>,
    renderers: &render::state::Keyed<CarRenderer>,
) -> Vec<(&'a CarId, &'a CarState, &'a Car, LodDraw)> {
    let mut cars = Vec::new();
    match mode {
        Mode::Race(r) => {
            for v in r.racers.rivals.iter() {
                if renderers.get(&v.id).is_some() {
                    cars.push((&v.id, poses.car(&v.id, &v.state), &v.car, LodDraw::InGame));
                }
            }
            for c in
                r.racers.traffic.iter().filter(|t| !t.paused).flat_map(|t| t.cars.iter().filter(|c| crate::render::frame::traffic_drawn(c)))
            {
                if renderers.get(&c.id).is_some() {
                    cars.push((&c.id, poses.car(&c.id, &c.state), &c.car, LodDraw::FrontEnd));
                }
            }
        }
        Mode::Explore => {
            for c in encounters.into_iter().flat_map(|e| e.cars.iter().filter(|c| c.active)) {
                if renderers.get(&c.id).is_some() {
                    cars.push((&c.id, poses.car(&c.id, &c.state), &c.car, LodDraw::InGame));
                }
            }
            for c in traffic.into_iter().flat_map(|t| t.cars.iter().filter(|c| crate::render::frame::traffic_drawn(c))) {
                if renderers.get(&c.id).is_some() {
                    cars.push((&c.id, poses.car(&c.id, &c.state), &c.car, LodDraw::FrontEnd));
                }
            }
        }
        Mode::Menu(_) => {}
    }
    cars
}
