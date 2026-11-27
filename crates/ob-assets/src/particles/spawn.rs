use super::*;

/// The direction a particle leaves in, in emitter space before the matrix.
/// Mode 0 (+0x18 = 0) turns the emitter's z axis by three random angles
/// about x, y and z (0x5BAD00, 0x5BADB0, 0x5BAE70), each `rand · cone −
/// cone / 2` degrees with `cone` the +0x50 angle (0x492539…0x492633:
/// [esp+0x50] = +0x50, [esp+0x58] = +0x50 · 0.5, × 65536 through `_ftol`,
/// / 360 into the 16-bit angle) — a jitter of ±cone/2 about each axis,
/// not every way. Otherwise (+0x18 set) a ring in the emitter's xy plane
/// tilted about y: 0x492257…0x49230A draw `a = rand · 360 − 180` degrees
/// (0x78450C, 0x784510) and `b = rand · H − H / 2` (H = +0x50 at
/// [esp+0x58], H / 2 at [esp+0x60] there), and the direction is (cos a,
/// sin a, 0) (0x43C370, 0x43C2B0) times 0x5BADB0's turn by b about y. So
/// with H = 0 (XEE_Event_*_SmokeB_01, the shop and garage markers' ground
/// smoke) every particle leaves level, spreading out over the ground, as
/// the original's apitrace shows (a garage exit, 2026-11-01: 27
/// particles about 4 m round the marker, all at its height). Not a cone
/// about z (the clone's reading until then, which sent that smoke straight
/// up as a green column).
// prov: inferred particles.direction
/// **Inferred**: the order the three turns compose in, and the draws;
/// the matrix 0x872E90 the y turn is built on is taken as the identity.
pub fn direction(d: &Description, rng: &mut Rng) -> Vec3 {
    if d.spherical {
        let half = d.cone * 0.5;
        let mut angle = || (rng.next() * d.cone - half).to_radians();
        let (a, b, c) = (angle(), angle(), angle());
        let m = glam::Mat3::from_rotation_z(c) * glam::Mat3::from_rotation_y(b) * glam::Mat3::from_rotation_x(a);
        m * Vec3::Z
    } else {
        let a = (rng.next() * 360.0 - 180.0).to_radians();
        let b = (rng.next() * d.cone - d.cone * 0.5).to_radians();
        glam::Mat3::from_rotation_y(b) * Vec3::new(a.cos(), a.sin(), 0.0)
    }
}

/// One particle's spawn (0x491E50's loop body).
pub fn spawn(d: &Description, e: &mut Emitter) -> Particle {
    let rng = &mut e.rng;
    let dir = e.transform.transform_vector3(direction(d, rng)).normalize_or_zero();
    let local = d.offset + Vec3::new(rng.next() - 0.5, rng.next() - 0.5, rng.next() - 0.5) * d.extent;
    let position = e.transform.transform_point3(local);
    let frames = if d.random_frame { d.frames() as f32 } else { 1.0 };
    let frame = (rng.next() * frames) as u8;
    let life = rng.spread(d.life).min(60.0);
    let speed = rng.spread(d.speed);
    // The share of the owner's velocity: only when +0x54 − +0x54·+0x58 is
    // not 0, clamped 0..1; taken at birth unless the description carries.
    let base = d.inherit[0] - d.inherit[0] * d.inherit[1];
    let share = if base != 0.0 { (rng.next() * d.inherit[0] * d.inherit[1] + base).clamp(0.0, 1.0) } else { 0.0 };
    let v = if d.carried { dir * speed } else { dir * speed + e.velocity * share };
    let range = d.spin_range * 182.04445;
    let mut angle = ((rng.next() * range - range * 0.5) as i32 as u16) & 0xFFFE;
    if d.random_flip && rng.next() < 0.5 {
        angle |= 1;
    }
    let mut p = Particle {
        description: e.description as u16,
        palette: e.palette,
        key: e.key,
        life: (life * TICKS) as i16,
        angle,
        speed: 0,
        carried: (share * 255.0) as u8,
        position,
        direction: [0; 3],
        frame,
        id: 0,
    };
    p.set_velocity(v);
    p
}
