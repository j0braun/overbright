//! The collision layer's content without a window (View ▸ Collision in the editor and Studio's World view): the
//! three modes it shows ([`Show`]), the scenery the cover test looks at and the layer built from the region's
//! collision packs. The window draws it; `collision show` on the command line counts what each mode shows.

use glam::Vec3;
use nfsu2::studio_api::world::{HIDDEN_FLAG, Placement, World};
use ob_map::tools::map::collision::{self, Counts, Layer};

/// What the layer shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Show {
    #[default]
    Off,
    /// Invisible walls and the race and career barriers.
    Invisible,
    /// Also the walls behind scenery and the floors.
    All,
}

impl Show {
    pub const ALL: [Show; 3] = [Show::Off, Show::Invisible, Show::All];

    /// Its name in the View menu.
    pub fn label(self) -> &'static str {
        match self {
            Show::Off => "Off",
            Show::Invisible => "Invisible barriers",
            Show::All => "All collision",
        }
    }

    /// The next state (the L key): Off, Invisible barriers, All collision, Off.
    pub fn next(self) -> Show {
        match self {
            Show::Off => Show::Invisible,
            Show::Invisible => Show::All,
            Show::All => Show::Off,
        }
    }

    /// `--collision off|invisible|all`.
    pub fn parse(v: &str) -> Option<Show> {
        match v {
            "off" => Some(Show::Off),
            "invisible" => Some(Show::Invisible),
            "all" => Some(Show::All),
            _ => None,
        }
    }
}


/// The scenery triangles the cover test looks at: the original's
/// placements as drawn in free roam (no panorama, no race-only or
/// group-switched pieces), see-through ones included (a fence is seen).
pub fn scenery(world: &World, placements: &[Placement]) -> Vec<[Vec3; 3]> {
    let mut out = Vec::new();
    for p in placements {
        let Some(solid) = world.solids.get(p.solid) else { continue };
        if solid.name.starts_with("PAN_") || p.flags & (HIDDEN_FLAG | RACE_ONLY) != 0 {
            continue;
        }
        for tri in solid.indices.chunks_exact(3) {
            let Some(c) = (0..3)
                .map(|i| {
                    solid.vertices.get(tri[i] as usize).map(|v| p.transform.transform_point3(Vec3::from(v.position)))
                })
                .collect::<Option<Vec<Vec3>>>()
            else {
                continue;
            };
            out.push([c[0], c[1], c[2]]);
        }
    }
    out
}

/// The race-only scenery flag (`world::scenery`'s 0x200).
const RACE_ONLY: u16 = 0x0200;


/// The layer of `world` (the region as the original has it), the cover test over `placements`.
pub fn layer_of(world: &World, placements: &[Placement]) -> Layer {
    collision::build(world.ground.listed_faces(), &world.barrier_groups, &scenery(world, placements))
}

impl Show {
    /// What this mode lists, as the view's legend does: (what, how many of `c`).
    pub fn rows(self, c: &Counts, off_ground_floors: usize) -> Vec<(&'static str, usize)> {
        let mut rows = Vec::new();
        if self == Show::Off {
            return rows;
        }
        rows.push(("Invisible walls", c.invisible));
        rows.push(("Race and career barriers", c.barriers));
        if self == Show::All {
            rows.push(("Walls behind scenery", c.visible));
            rows.push(("Road floors", c.floors - off_ground_floors));
            rows.push(("Floors that are not ground", off_ground_floors));
        }
        rows
    }
}
