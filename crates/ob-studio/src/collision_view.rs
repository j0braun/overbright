//! The collision layer's content without a window (View ▸ Collision in the editor and Studio's World view): the
//! three modes it shows ([`Show`]), the scenery the cover test looks at and the layer built from the region's
//! collision packs. The window draws it; `collision show` on the command line counts what each mode shows.

use glam::Vec3;
use nfsu2::studio_api::world::{CollisionOrigin, HIDDEN_FLAG, Placement, World, face_key};
use ob_map::tools::map::collision::{self, Counts, Layer, Mine};

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


/// What colours the layer: the kinds a player meets, or the surface id of each face.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Colour {
    #[default]
    Kind,
    Surface,
}

impl Colour {
    pub const ALL: [Colour; 2] = [Colour::Kind, Colour::Surface];

    /// Its name in the View menu.
    pub fn label(self) -> &'static str {
        match self {
            Colour::Kind => "By kind",
            Colour::Surface => "By surface",
        }
    }
}

/// One class of the layer the legend lists and a click can switch off: what the walls and floors are, and whose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Invisible,
    Barrier,
    Behind,
    Road,
    NotGround,
    Added,
    Moved,
    Drawn,
}

impl Class {
    pub const ALL: [Class; 8] = [
        Class::Invisible,
        Class::Barrier,
        Class::Behind,
        Class::Road,
        Class::NotGround,
        Class::Added,
        Class::Moved,
        Class::Drawn,
    ];

    /// Its name in the View menu and the key.
    pub fn label(self) -> &'static str {
        match self {
            Class::Invisible => "Invisible walls",
            Class::Barrier => "Barriers",
            Class::Behind => "Walls behind scenery",
            Class::Road => "Road floors",
            Class::NotGround => "Floors that are not ground",
            Class::Added => "Added props'",
            Class::Moved => "Moved props'",
            Class::Drawn => "Drawn by this mod",
        }
    }

    fn bit(self) -> u16 {
        1 << self as u16
    }
}

/// The opacities the View menu offers (percent of the layer's own strength).
pub const OPACITIES: [u8; 4] = [25, 50, 75, 100];

/// An opacity's name in the View menu.
pub fn opacity_label(percent: u8) -> String {
    format!("{percent} %")
}

/// How the layer is drawn besides what it shows: classes and surfaces switched off, opacity, wireframe and full-height
/// walls. View state only: nothing of it is ever written to the mod.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look {
    hidden: u16,
    surfaces: [u64; 4],
    /// Percent of the layer's own strength, 10..=100.
    pub opacity: u8,
    /// Outlines only, no fill.
    pub wireframe: bool,
    /// Walls the game makes reach from -2048 m to 2048 m drawn that tall, not just beside the floors.
    pub full_height: bool,
}

impl Default for Look {
    fn default() -> Self {
        Look::DEFAULT
    }
}

impl Look {
    pub const DEFAULT: Look = Look { hidden: 0, surfaces: [0; 4], opacity: 100, wireframe: false, full_height: false };

    pub fn shows(&self, c: Class) -> bool {
        self.hidden & c.bit() == 0
    }

    pub fn toggle(&mut self, c: Class) {
        self.hidden ^= c.bit();
    }

    pub fn shows_surface(&self, id: u8) -> bool {
        self.surfaces[(id >> 6) as usize] & (1 << (id & 63)) == 0
    }

    pub fn toggle_surface(&mut self, id: u8) {
        self.surfaces[(id >> 6) as usize] ^= 1 << (id & 63);
    }

    /// Everything shown again (the opacity, wireframe and height stay).
    pub fn show_every_kind(&mut self) {
        self.hidden = 0;
        self.surfaces = [0; 4];
    }

    pub fn set_opacity(&mut self, percent: u8) {
        self.opacity = percent.clamp(10, 100);
    }

    /// A colour's alpha under the opacity.
    pub fn alpha(&self, a: u8) -> u8 {
        (a as u32 * self.opacity as u32 / 100) as u8
    }
}

/// Mark the faces of `layer` the mod wrote (`world.map_overlay.collision.marks`), so a view can set them apart
/// from the original's.
pub fn mark(layer: &mut Layer, world: &World) {
    let marks: std::collections::HashMap<&Vec<[i32; 3]>, Mine> = world
        .map_overlay
        .collision
        .marks
        .iter()
        .map(|(o, k)| {
            (
                k,
                match o {
                    CollisionOrigin::Added => Mine::Added,
                    CollisionOrigin::Moved => Mine::Moved,
                    CollisionOrigin::Drawn => Mine::Drawn,
                },
            )
        })
        .collect();
    if marks.is_empty() {
        return;
    }
    layer.mark(&|corners| marks.get(&face_key(corners)).copied().unwrap_or(Mine::Original));
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
    let mut layer = collision::build(world.ground.listed_faces(), &world.barrier_groups, &scenery(world, placements));
    mark(&mut layer, world);
    layer
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
