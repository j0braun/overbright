use crate::ai::gps::{FRAME_WORK, Follower, MODE_PLAYER, Router};
use crate::formats::ai_routes::RouteFile;
use glam::{Vec2, Vec3};

/// The engaged GPS (0x838590: +0 the target's hash, +0x10 its point, +0x20
/// / +0x24 its map point, +0x29 engaged), with the router it drives
/// (0x81CBC0) and the player's route follower (driver +0x3E4 / +0x3F0).
#[derive(Clone, Debug)]
pub struct Gps {
    /// The target's hash (0x529490: a shop's +0x38, an event's +8, a
    /// milestone's +0x20, a challenge's name).
    pub target: u32,
    /// The route trigger's GPS point (+0x34, +0x38, +0x3C, integers; not
    /// its position).
    pub at: Vec3,
    /// The trigger's x, y (+4, +8): where the minimap's GPS icon goes.
    pub map: Vec2,
    pub router: Router,
    pub follower: Follower,
    /// MARKER_GPS_AID's look-ahead point (player +0x120, set while +0x114):
    /// kept from the last frame that found one (0x5F3CC0).
    pub aid: Option<Vec3>,
}

/// 0x40A2A0: how far ahead along the route the GPS aid looks — the car's
/// speed (driver +4 → car +0x430) × 1.5, at least 100 m.
pub fn aid_ahead(speed: f32) -> f32 {
    (speed * 1.5).max(100.0)
}

/// 0x5F3CC0's look-ahead on a route polyline: the point nearest the car
/// (0x419B20 → 0x4198C0), its distance along the route (0x40AE30, here the
/// car projected onto that point's stretch), then the point `ahead` metres
/// further (0x419D20: the stretch holding it, interpolated by distance,
/// 0x43D0C0); none past the route's end.
pub fn look_ahead(points: &[Vec3], car: Vec3, ahead: f32) -> Option<Vec3> {
    if points.is_empty() {
        return None;
    }
    let nearest = (0..points.len())
        .min_by(|&a, &b| (points[a] - car).length_squared().total_cmp(&(points[b] - car).length_squared()))?;
    let mut along = 0.0;
    for i in 0..nearest {
        along += (points[i + 1] - points[i]).length();
    }
    if nearest + 1 < points.len() {
        let d = points[nearest + 1] - points[nearest];
        let l = d.length();
        if l > 0.0 {
            along += (car - points[nearest]).dot(d / l).clamp(0.0, l);
        }
    }
    let mut s = along + ahead;
    for w in points.windows(2) {
        let l = (w[1] - w[0]).length();
        if s <= l {
            return Some(if l > 0.0 { w[0] + (w[1] - w[0]) * (s / l) } else { w[0] });
        }
        s -= l;
    }
    None
}

/// 0x5EACA0: MARKER_GPS_AID's matrix. The camera's field of view (camera
/// +0xC4, a binary angle) halved is h (quartered in split screen); t =
/// cos(h)·10 (0x43C370 is 0x43C2B0's sine of h + 0x4000; 10.0 at
/// 0x7A5560), a = −ftol(h·0.5) (0x7A5564), (s, c) = 0x43C390(a); P is the
/// camera-space point (0, s·t, c·t) — x right, y down, z forward (the view
/// matrix's inverse, 0x5BAFA0, applied by 0x441000) — in the world; the
/// model's z row is unit(P − target), x = unit(Z × z), y = unit(z × x)
/// (0x43D1E0 is a × b), and P its position: rows x, y, z, P (0x5C5940
/// draws the solid with it).
pub fn aid_matrix(eye: Vec3, right: Vec3, down: Vec3, forward: Vec3, fov: f32, target: Vec3) -> glam::Mat4 {
    use glam::Vec4;
    let fov_angle = ((fov / std::f32::consts::TAU * 65536.0) as i32) as u16;
    let h = fov_angle >> 1;
    let t = (h as f32 * std::f32::consts::TAU / 65536.0).cos() * 10.0;
    let a = -(((h as i16) as f32 * 0.5) as i32);
    let (s, c) = ((a as u16) as f32 * std::f32::consts::TAU / 65536.0).sin_cos();
    let p = eye + right * 0.0 + down * (s * t) + forward * (c * t);
    let z = (p - target).normalize_or(Vec3::Z);
    let x = Vec3::Z.cross(z).normalize_or(Vec3::X);
    let y = z.cross(x).normalize_or(Vec3::Y);
    glam::Mat4::from_cols(x.extend(0.0), y.extend(0.0), z.extend(0.0), Vec4::new(p.x, p.y, p.z, 1.0))
}

impl Gps {
    /// 0x529490 on a marker entry {trigger, …}: the trigger's GPS point; the
    /// route search 0x413120 → 0x40C440 from the player's car (+0x60, its
    /// forward row +0x90) to it in mode 2, with the car's driver as the
    /// owner. Without a car the router stays idle (the GPS reads off).
    pub fn engage(target: u32, trigger: &crate::frontend::career::Trigger, car: Option<(Vec3, Vec2)>) -> Gps {
        let at = trigger.gps;
        let mut router = Router::default();
        if let Some((p, f)) = car {
            router.start(p, f, at, MODE_PLAYER);
        }
        Gps { target, at, map: trigger.position.truncate(), router, follower: Follower::default(), aid: None }
    }

    /// 0x501100 (+0x29 set): router +4 1 → 0 (searching), 2 → 3 (found),
    /// else 4; then router +8 3 → 2 ("Unable to find a path",
    /// 0x31139B65), 4 → 1 ("Unable to contact satellite", 0x155409E8).
    pub fn state(&self) -> u8 {
        let s = match self.router.result {
            1 => 0,
            2 => 3,
            _ => 4,
        };
        match self.router.last {
            3 => 2,
            4 => 1,
            _ => s,
        }
    }

    /// 0x4AE930 is false: the GPS is on (states 0–3).
    pub fn on(&self) -> bool {
        self.state() <= 3
    }

    /// 0x529420: on and aimed at this point (each axis within 0.001).
    pub fn aims_at(&self, at: Vec3) -> bool {
        self.on() && (self.at - at).abs().max_element() <= 0.001
    }

    /// 0x4C8410: the minimap's GPS group (0x697D2BCA) shows in states 0-3
    /// and is hidden in state 4, the router idle after an arrival
    /// (0x42A360), a race's start (0x541170) or a deactivation. The map
    /// point it goes to, or None while the group stays hidden.
    pub fn ring_point(&self) -> Option<Vec2> {
        self.on().then_some(self.map)
    }

    /// The minimap's GPS_SEARCH_ERROR text (0x4C8410): states 1 and 2.
    pub fn error(&self) -> Option<u32> {
        match self.state() {
            1 => Some(0x1554_09E8),
            2 => Some(0x3113_9B65),
            _ => None,
        }
    }

    /// 0x4C8410: GPS_Search_Group shows in states 0–2 unless the player's
    /// follower holds a route (+0x3E4 1 or 2).
    pub fn searching(&self) -> bool {
        self.state() <= 2 && !matches!(self.follower.state, 1 | 2)
    }

    /// A frame of the world update: the router (0x609D32 → 0x4268A0, its
    /// result to the follower through 0x41EA30), then the player's
    /// follower (0x42A360: arrival within 30 m / 4 m of the goal clears
    /// the router; off the route a new search, 0x4188C0).
    pub fn update(&mut self, routes: &RouteFile, car: Vec3, forward: Vec2) {
        if let Some(cb) = self.router.update(routes, FRAME_WORK) {
            self.follower.hear(cb);
        }
        self.follower.update(&mut self.router, routes, car, forward);
    }

    /// 0x4131F0 on the router (a race's start, 0x541170): the follower
    /// hears code 0; the GPS stays engaged but reads off.
    pub fn drop_route(&mut self) {
        if let Some(cb) = self.router.clear() {
            self.follower.hear(cb);
        }
    }

    /// 0x5F3CC0 for the player in free roam with the chase camera (player
    /// +0x238 → +0xC = 1, the camera mover's type: 0x5F1FE0 compares it
    /// with the type it builds): with the follower in state 1 or 2 the
    /// look-ahead point is found and kept (+0x120, +0x114 = 1), or the
    /// last one stays; in any other state +0x114 is cleared. The point the
    /// aid is drawn towards, if any.
    pub fn update_aid(&mut self, routes: &RouteFile, car: Vec3, speed: f32) -> Option<Vec3> {
        if !matches!(self.follower.state, 1 | 2) {
            self.aid = None;
            return None;
        }
        if let Some(p) = look_ahead(&self.route(routes), car, aid_ahead(speed)) {
            self.aid = Some(p);
        }
        self.aid
    }

    /// The found route's points from the start to the goal.
    pub fn route(&self, routes: &RouteFile) -> Vec<Vec3> {
        match (&self.follower.node, self.follower.state) {
            (Some(n), 1 | 2) => n.points(routes),
            _ => Vec::new(),
        }
    }
}
