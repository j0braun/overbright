use crate::frontend::career::triggers::{RouteTriggers, Trigger};
use glam::{Vec2, Vec3};
use std::path::Path;

/// The zone kinds in 0x5D9C80's walk order.
const KINDS: [u32; 4] = [5, 6, 7, 0x14];

/// What the tunnel update reads of the car (car +0x60, +0x90, +0x42C,
/// +0x430).
#[derive(Clone, Copy, Debug, Default)]
pub struct CarIn {
    pub position: Vec3,
    pub forward: Vec3,
    /// Along the heading (+0x42C), m/s.
    pub forward_speed: f32,
    /// |velocity| (+0x430), m/s.
    pub speed: f32,
}

/// SFXCTL_Tunnel's drive-by part.
#[derive(Clone, Debug, Default)]
pub struct Tunnel {
    pub(super) zones: [Vec<Trigger>; 4],
    /// The region whose route data the zones are (the loaded track's
    /// triggers 0x88F298).
    region: String,
    entry_cool: f32,
    exit_cool: f32,
    last_entry: Option<(usize, usize)>,
    /// +0x7C / +0x9C: this frame's entry / exit.
    pub entry: bool,
    pub exit: bool,
    /// +0x74: the zone kind ahead (2 none).
    pub ahead: u32,
    /// +0x70 / +0x78.
    pub inside: bool,
    pub inside_kind: u32,
    /// +0x90 / +0xA0: the ramp at the last entry / exit.
    pub entry_ramp: f32,
    pub exit_ramp: f32,
    /// +0x80..+0x88: the point of the last entry / exit.
    pub point: Vec3,
}

/// The ramp 0x7F0B40 (0x4599E0 with y 0 → 1 over x 18 → 45, set up by
/// 0x77C5B0 from 0x788868 / 0x788864).
pub fn speed_ramp(speed: f32) -> f32 {
    let f = (speed - 18.0) / (45.0 - 18.0);
    if f > 0.0 { f.min(1.0) } else { 0.0 }
}

impl Tunnel {
    pub fn new(zones: [Vec<Trigger>; 4]) -> Tunnel {
        Tunnel { zones, ahead: 2, inside_kind: 2, ..Default::default() }
    }

    pub fn load(root: &Path) -> Tunnel {
        Tunnel { region: "L4RA".into(), ..Tunnel::new(KINDS.map(|k| RouteTriggers::load_kind(root, k))) }
    }

    /// The zones of the region the car drives in (0x5D9C80 walks the
    /// loaded track's route triggers): L4RA in free roam, L4RG on the
    /// dyno. A new region starts outside any zone.
    pub fn set_region(&mut self, root: &Path, region: &str) {
        if self.region != region {
            *self = Tunnel {
                region: region.into(),
                ..Tunnel::new(KINDS.map(|k| RouteTriggers::load_kind_in(root, region, k)))
            };
        }
    }

    /// SFXCTL_Tunnel's mix fields f0..f3 (vtable 0x78A064 slot 6,
    /// 0x45DB40): all 0 outside a zone (+0x70 clear); inside, f3 = 1 and
    /// f1 for a kind 6 / 7 zone, f2 for kind 0x14, else (kind 5) f0
    /// (+0x78). The mix map reads them as section 1 class 7 (OutID
    /// 0x10070): MAPOUTPUT's event control 20 (f2, −510 mB) ducks the
    /// music while the car is in the dyno's kind-0x14 zone (under Wine,
    /// DynoEnter: f2 0 → 1 about 7 s in as the car rolls onto the dyno,
    /// the song's level 18 → 10).
    pub fn fields(&self) -> [i32; 4] {
        if !self.inside {
            return [0; 4];
        }
        match self.inside_kind {
            6 | 7 => [0, 1, 0, 1],
            0x14 => [0, 0, 1, 1],
            _ => [1, 0, 0, 1],
        }
    }

    /// The first zone of kind slots `kinds` holding `at` that passes the
    /// height test (0x5D9C80 walked on past each refused hit).
    fn find(&self, kinds: std::ops::Range<usize>, at: Vec2, z: f32) -> Option<(usize, usize)> {
        for k in kinds {
            if let Some(i) =
                self.zones[k].iter().position(|t| t.contains(at) && (t.position.z == 0.0 || t.position.z >= z))
            {
                return Some((k, i));
            }
        }
        None
    }

    /// One frame (0x483250 → 0x471080).
    pub fn update(&mut self, car: &CarIn, dt: f32) {
        self.entry = false;
        self.exit = false;
        self.entry_cool = (self.entry_cool - dt).max(0.0);
        self.exit_cool = (self.exit_cool - dt).max(0.0);
        let f = Vec2::new(car.forward.x, car.forward.y).normalize_or_zero();
        let ahead = Vec2::new(car.position.x, car.position.y) + f * (car.forward_speed * 0.3);
        let hit = self.find(0..3, ahead, car.position.z);
        self.ahead = hit.map_or(2, |(k, _)| KINDS[k]);
        let ramp = speed_ramp(car.speed);
        let point = Vec3::new(ahead.x, ahead.y, car.position.z + 10.0);
        if !self.inside {
            if hit.is_some() && (self.entry_cool < 0.01 || self.last_entry != hit) && ramp > 0.01 {
                self.entry = true;
                self.entry_cool = 3.0;
                self.last_entry = hit;
                self.point = point;
                self.entry_ramp = ramp;
            }
        } else if self.ahead == 2 && self.inside_kind == 5 && self.exit_cool < 0.01 && ramp > 0.01 {
            self.exit = true;
            self.exit_cool = 3.0;
            self.point = point;
            self.exit_ramp = ramp;
        }
        // The car's own zone (0x4832F7..0x483435).
        let here = self.find(0..4, Vec2::new(car.position.x, car.position.y), car.position.z);
        self.inside_kind = here.map_or(0x14, |(k, _)| KINDS[k]);
        self.inside = here.is_some();
    }
}
