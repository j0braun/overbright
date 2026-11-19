use crate::*;

/// The headlight pools of the cars on the road (0x6154A0 walks every car
/// render object, 0x890080): the player's, the race's rivals and the
/// explore mode's traffic; a car whose +0x550 (active) is clear is skipped
/// (the same test as World_DrawCars, so a traffic car not drawn throws no
/// pool). Every car's +0x93D (headlights on) is 1: the constructor sets it
/// (0x6045B0) and 0x5F61A0 sets it again unless a flicker (+0x925,
/// 0x5EC170) is under way, which the clone does not have.
/// `poses`: the frame's cars as drawn.
pub(crate) fn headlight_pools(
    d: &Drive,
    poses: &render::snapshot::FramePoses,
    race: Option<&Race>,
    traffic: Option<&Traffic>,
) -> Vec<HeadlightPool> {
    let pool =
        |car: &Car, state: &CarState| HeadlightPool { transform: state.transform(), color: car.headlight_pool_color };
    let mut pools = vec![pool(&d.car, poses.player())];
    if let Some(race) = race {
        pools.extend(race.racers.rivals.iter().map(|r| pool(&r.car, poses.car(&r.id, &r.state))));
    }
    if let Some(traffic) = traffic {
        pools.extend(traffic.cars.iter().filter(|c| crate::render::frame::traffic_drawn(c)).map(|c| pool(&c.car, poses.car(&c.id, &c.state))));
    }
    pools
}
