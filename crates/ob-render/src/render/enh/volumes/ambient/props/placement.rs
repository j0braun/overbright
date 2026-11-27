use super::*;

/// Prop kinds (ambient_detail.wgsl's `kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Kind {
    Can = 0,
    Bottle = 1,
    Cup = 2,
    Box = 3,
    Crumple = 4,
    Leaf = 5,
    Paper = 6,
    Stain = 7,
    Grate = 8,
    Grit = 9,
    Moth = 10,
}

impl Kind {
    pub const ALL: [Kind; 11] = [
        Kind::Can,
        Kind::Bottle,
        Kind::Cup,
        Kind::Box,
        Kind::Crumple,
        Kind::Leaf,
        Kind::Paper,
        Kind::Stain,
        Kind::Grate,
        Kind::Grit,
        Kind::Moth,
    ];

    /// The mesh it is drawn with.
    pub fn mesh(self) -> Mesh {
        match self {
            Kind::Can => Mesh::Can,
            Kind::Bottle => Mesh::Bottle,
            Kind::Cup => Mesh::Cup,
            Kind::Box => Mesh::Box,
            Kind::Crumple | Kind::Grit => Mesh::Crumple,
            Kind::Leaf | Kind::Paper => Mesh::Card,
            Kind::Stain | Kind::Grate | Kind::Moth => Mesh::Quad,
        }
    }

    /// Drawn blended over the scene without a depth write (decals, moths).
    pub fn decal(self) -> bool {
        matches!(self, Kind::Stain | Kind::Grate | Kind::Moth)
    }

    /// Blown about by the cars' wakes and the wind (Next-Gen).
    pub fn loose(self) -> bool {
        matches!(self, Kind::Leaf | Kind::Paper)
    }
}

/// The meshes (one vertex / index range each in [`meshes`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u32)]
pub enum Mesh {
    Can = 0,
    Bottle = 1,
    Cup = 2,
    Box = 3,
    Crumple = 4,
    Card = 5,
    Quad = 6,
}

pub const MESHES: usize = 7;

/// One prop instance as the GPU takes it (ambient_detail.wgsl `Inst`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Inst {
    /// xyz the origin (on the surface), w the uniform scale.
    pub pos_s: [f32; 4],
    /// The orientation (quaternion x, y, z, w).
    pub rot: [f32; 4],
    /// rgb the baked light under it (v0, 0..1), w the kind.
    pub light_kind: [f32; 4],
    /// x a seed 0..1, y / z the local y / z scale, w the opacity (fade).
    pub extra: [f32; 4],
}

/// A placed prop: its instance at rest and the section it belongs to.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    pub inst: Inst,
    pub kind: Kind,
    pub section: u32,
}

/// A surface triangle.
#[derive(Clone, Copy)]
pub(crate) struct Tri {
    pub p: [Vec3; 3],
    pub light: [Vec3; 3],
    pub section: u32,
}

pub(crate) fn hash(mut x: u64) -> u64 {
    // SplitMix64 finaliser (Steele, Lea, Flood 2014; public domain constants).
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// A deterministic stream of 0..1 floats.
pub(crate) struct Rng(pub u64);
impl Rng {
    pub fn f(&mut self) -> f32 {
        self.0 = hash(self.0);
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

const PAVEMENT_WORDS: &[&str] = &["SIDEWALK", "CURB", "KERB", "PAVE", "COBBLE", "COURTGD"];

pub(crate) fn is_pavement(name: &str) -> bool {
    let n = name.to_ascii_uppercase();
    // (RDP_ROADSTRIP_COBBLESTO and the like are road strips: not pavement.)
    !n.starts_with("RDP_") && PAVEMENT_WORDS.iter().any(|w| n.contains(w))
}

/// The road grills (RDP_ROAD_GRILL_WH: Bayview's 31 metal grills set in
/// the road; `examples/ambient_census`) steam.
pub(crate) fn is_grill(name: &str) -> bool {
    name.to_ascii_uppercase().contains("ROAD_GRILL")
}

/// Loose ground (more tyre dust).
pub(crate) fn is_gravel(name: &str) -> bool {
    let n = name.to_ascii_uppercase();
    n.contains("GRAVEL") || n.contains("DIRT") || n.contains("SAND")
}

/// Props per m² on pavements and in the gutter (the prototype's "normal").
pub const DENSITY: (f32, f32) = (0.10, 0.55);
/// Drain grates per m² of gutter.
pub const GRATE_DENSITY: f32 = 0.008;

/// The 2D distance from `p` to triangle `t` (0 inside).
pub(crate) fn tri_distance(p: Vec2, t: &[Vec3; 3]) -> f32 {
    let v: [Vec2; 3] = [t[0].truncate(), t[1].truncate(), t[2].truncate()];
    let edge = |a: Vec2, b: Vec2| {
        let ab = b - a;
        let h = ((p - a).dot(ab) / ab.length_squared().max(1e-9)).clamp(0.0, 1.0);
        (p - a - ab * h).length()
    };
    let cross = |a: Vec2, b: Vec2| (b - a).perp_dot(p - a);
    let (c0, c1, c2) = (cross(v[0], v[1]), cross(v[1], v[2]), cross(v[2], v[0]));
    if (c0 >= 0.0 && c1 >= 0.0 && c2 >= 0.0) || (c0 <= 0.0 && c1 <= 0.0 && c2 <= 0.0) {
        return 0.0;
    }
    edge(v[0], v[1]).min(edge(v[1], v[2])).min(edge(v[2], v[0]))
}

/// A uniform 2D grid of triangles (by their bounding boxes). Its map is
/// only looked up, never walked (so its hasher, the fast one, cannot reach
/// the output; std's SipHash made the city's ground grid ~0.1 s).
pub(crate) struct Grid {
    cell: f32,
    pub(super) map: crate::fxhash::FxHashMap<(i32, i32), Vec<u32>>,
}

impl Grid {
    pub fn new<'a>(tris: impl Iterator<Item = &'a [Vec3; 3]>, cell: f32) -> Grid {
        let mut map: crate::fxhash::FxHashMap<(i32, i32), Vec<u32>> = Default::default();
        for (i, t) in tris.enumerate() {
            let lo = t[0].min(t[1]).min(t[2]);
            let hi = t[0].max(t[1]).max(t[2]);
            for x in (lo.x / cell).floor() as i32..=(hi.x / cell).floor() as i32 {
                for y in (lo.y / cell).floor() as i32..=(hi.y / cell).floor() as i32 {
                    map.entry((x, y)).or_default().push(i as u32);
                }
            }
        }
        Grid { cell, map }
    }

    pub(super) fn cell(&self, p: Vec3) -> (i32, i32) {
        ((p.x / self.cell).floor() as i32, (p.y / self.cell).floor() as i32)
    }

    /// The distance (2D) from `p` to the nearest triangle within `r`
    /// whose height is within 1 m (None: none that near).
    fn nearest(&self, tris: &[Tri], p: Vec3, r: f32) -> Option<f32> {
        let (cx, cy) = self.cell(p);
        let mut best: Option<f32> = None;
        for x in cx - 1..=cx + 1 {
            for y in cy - 1..=cy + 1 {
                for &i in self.map.get(&(x, y)).into_iter().flatten() {
                    let t = &tris[i as usize];
                    if (t.p[0].z - p.z).abs() < 1.0 {
                        let d = tri_distance(p.truncate(), &t.p);
                        if d < r && best.is_none_or(|b| d < b) {
                            best = Some(d);
                        }
                    }
                }
            }
        }
        best
    }
}

/// The surfaces of a world the props use.
pub(crate) struct Surfaces {
    pub pave: Vec<Tri>,
    pub road: Vec<Tri>,
    /// Upward road triangles not worldreflect (gravel, parking lots ...).
    pub other: Vec<Tri>,
    /// The upward triangles' gravel flag, in `pave, road, other` order.
    pub gravel: Vec<bool>,
    /// The road grills' triangles.
    pub grills: Vec<Tri>,
}

/// Which of [`Surfaces`]' lists a texture slot's upward triangles go to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlotUse {
    Grill,
    Pave,
    Road,
    Other,
    /// The slot gives the props nothing.
    Unused,
}

/// [`surfaces`]' decision for texture `t` drawn by a solid that is (or is
/// not) worldreflect, and whether its ground is loose. It reads the
/// texture's name, blend, cut-out flag and size ([`crate::surface_maps::slot_class`])
/// and `reflect` alone, so one decision per texture and `reflect` holds
/// for every material that draws it.
fn slot_use(t: &crate::formats::texture::Texture, reflect: bool) -> (SlotUse, bool) {
    let class = crate::surface_maps::slot_class(t, reflect, false);
    let upper = t.name.to_ascii_uppercase();
    let used = if is_grill(&t.name) {
        SlotUse::Grill
    } else {
        match class {
            crate::surface_maps::Class::Concrete if is_pavement(&t.name) => SlotUse::Pave,
            crate::surface_maps::Class::Asphalt if reflect => SlotUse::Road,
            _ if upper.starts_with("RDP_") => SlotUse::Other,
            _ => SlotUse::Unused,
        }
    };
    (used, is_gravel(&t.name))
}

/// A world's [`slot_use`] decisions, each made once per (texture,
/// worldreflect): made per material of every placement, the name tests
/// were most of the props' build (a city: ~0.2 s of ~0.3 s).
#[derive(Default)]
pub(crate) struct SlotUses(std::collections::HashMap<(u32, bool), (SlotUse, bool)>);

impl SlotUses {
    /// The decision for material `m` of `solid`; None when its texture is
    /// not in the world.
    fn of(
        &mut self,
        world: &crate::world::World,
        solid: &crate::formats::solid::Solid,
        m: &crate::formats::solid::Material,
    ) -> Option<(SlotUse, bool)> {
        let &h = solid.textures.get(m.texture_slot as usize)?;
        let t = world.textures.get(&h)?;
        let reflect = solid.mesh_flags & crate::formats::solid::MESH_FLAG_REFLECTIVE != 0;
        Some(*self.0.entry((h, reflect)).or_insert_with(|| slot_use(t, reflect)))
    }
}

/// The world's pavement and road triangles (main LOD, placements not
/// hidden), in world space, upward-facing ([`surfaces_with`], the tests).
#[cfg(test)]
pub(crate) fn surfaces(world: &crate::world::World) -> Surfaces {
    surfaces_with(world, &mut SlotUses::default())
}

/// The world's pavement and road triangles (main LOD, placements not
/// hidden), in world space, upward-facing, with the slot decisions made so
/// far (`uses`).
pub(crate) fn surfaces_with(world: &crate::world::World, uses: &mut SlotUses) -> Surfaces {
    let mut s =
        Surfaces { pave: Vec::new(), road: Vec::new(), other: Vec::new(), gravel: Vec::new(), grills: Vec::new() };
    let mut gravel = (Vec::new(), Vec::new(), Vec::new());
    for pl in &world.placements {
        if pl.flags & 0x10 != 0 {
            continue;
        }
        let solid = &world.solids[pl.solid];
        for m in &solid.materials {
            let Some((used, loose)) = uses.of(world, solid, m) else {
                continue;
            };
            let (target, mut flags) = match used {
                SlotUse::Grill => (&mut s.grills, None),
                SlotUse::Pave => (&mut s.pave, Some(&mut gravel.0)),
                SlotUse::Road => (&mut s.road, Some(&mut gravel.1)),
                SlotUse::Other => (&mut s.other, Some(&mut gravel.2)),
                SlotUse::Unused => continue,
            };
            let idx = &solid.indices[m.first_index as usize..(m.first_index + m.index_count) as usize];
            for tri in idx.chunks_exact(3) {
                let v: [&crate::formats::solid::Vertex; 3] = [
                    &solid.vertices[tri[0] as usize],
                    &solid.vertices[tri[1] as usize],
                    &solid.vertices[tri[2] as usize],
                ];
                let p = v.map(|v| pl.transform.transform_point3(Vec3::from(v.position)));
                let n = (p[1] - p[0]).cross(p[2] - p[0]);
                if n.length_squared() < 1e-8 || n.normalize().z.abs() < 0.9 {
                    continue;
                }
                // D3DCOLOR bytes B, G, R, A.
                let light = v.map(|v| Vec3::new(v.color[2] as f32, v.color[1] as f32, v.color[0] as f32) / 255.0);
                target.push(Tri { p, light, section: pl.section });
                if let Some(f) = flags.as_mut() {
                    f.push(loose);
                }
            }
        }
    }
    s.gravel = [gravel.0, gravel.1, gravel.2].concat();
    s
}

/// One placement's part of [`SurfaceInputs`]: its index, hidden flag,
/// section and transform (as bits).
type PlacementInput = (u32, bool, u32, [u32; 16]);

/// What a world's [`surfaces`] were made from that can change after its
/// load: the scenery groups a race or free roam switches show and hide
/// placements (`World::set_up_barriers`), a backward race mirrors its
/// barriers, the track's world animations show theirs (`World::set_track`).
/// [`surfaces`] reads each placement's hidden flag, solid, section and
/// transform and the solids and textures, which a world keeps as loaded;
/// a placement whose solid has no slot of use ([`SlotUse`]) gives nothing
/// whatever its state. So these are the solids of use and, for each
/// placement drawing one, its state: equal, [`surfaces`] gives the same
/// triangles in the same order (a city: ~2 300 of ~48 000 placements, and
/// no barrier group lists any of them).
pub(crate) struct SurfaceInputs {
    /// Per solid: it has a slot of use.
    used: Vec<bool>,
    placements: usize,
    textures: usize,
    of_use: Vec<PlacementInput>,
}

impl SurfaceInputs {
    pub(crate) fn of(world: &crate::world::World, uses: &mut SlotUses) -> SurfaceInputs {
        let used: Vec<bool> = world
            .solids
            .iter()
            .map(|solid| {
                solid.materials.iter().any(|m| uses.of(world, solid, m).is_some_and(|(u, _)| u != SlotUse::Unused))
            })
            .collect();
        let of_use = Self::state(world, &used).collect();
        SurfaceInputs { used, placements: world.placements.len(), textures: world.textures.len(), of_use }
    }

    /// The state of `world`'s placements that draw a solid of use.
    fn state<'a>(world: &'a crate::world::World, used: &'a [bool]) -> impl Iterator<Item = PlacementInput> + 'a {
        world
            .placements
            .iter()
            .enumerate()
            .filter(|(_, p)| used.get(p.solid).copied().unwrap_or(true))
            .map(|(i, p)| (i as u32, p.flags & 0x10 != 0, p.section, p.transform.to_cols_array().map(f32::to_bits)))
    }

    /// `world` gives the [`surfaces`] these were taken with.
    pub(crate) fn same(&self, world: &crate::world::World) -> bool {
        self.used.len() == world.solids.len()
            && self.placements == world.placements.len()
            && self.textures == world.textures.len()
            && Self::state(world, &self.used).eq(self.of_use.iter().copied())
    }

    /// Placements drawing a solid of use (the stats, the tests).
    pub(crate) fn placements_of_use(&self) -> usize {
        self.of_use.len()
    }
}

/// A quaternion turning +z to `n` and then `yaw` about it.
fn surface_rot(n: Vec3, yaw: f32) -> Quat {
    Quat::from_rotation_arc(Vec3::Z, n) * Quat::from_rotation_z(yaw)
}

/// What [`place`] makes: the props and the drain grates' centres and long
/// axes (the steam vents' candidates).
pub struct Placement {
    pub props: Vec<Placed>,
    /// Drain grates (centre, steaming).
    pub drains: Vec<(Vec3, bool)>,
}

/// The props for a world's surfaces (deterministic: the same list every
/// run for the same world).
pub(crate) fn place(s: &Surfaces) -> Placement {
    let grid = Grid::new(s.pave.iter().map(|t| &t.p), 4.0);
    let (d_pave, d_gutter) = DENSITY;
    let mut out = Vec::new();
    let mut drains = Vec::new();
    // Per triangle (in parallel, in order: the same list every run).
    let scatter_one =
        |(i, t): (usize, &Tri), density: f32, salt: u64, gutter: bool| -> (Vec<Placed>, Vec<(Vec3, bool)>) {
            let (mut out, mut drains) = (Vec::new(), Vec::new());
            {
                let area = 0.5 * (t.p[1] - t.p[0]).cross(t.p[2] - t.p[0]).length();
                // The triangle's key: its section, index and first vertex (so a
                // list change elsewhere does not move it).
                let key = hash(
                    salt ^ ((t.section as u64) << 40)
                        ^ (i as u64)
                        ^ ((t.p[0].x.to_bits() as u64) << 8)
                        ^ t.p[0].y.to_bits() as u64,
                );
                let mut rng = Rng(key);
                let expected = area * density;
                let mut count = expected.floor() as u32;
                if rng.f() < expected.fract() {
                    count += 1;
                }
                let mut normal = (t.p[1] - t.p[0]).cross(t.p[2] - t.p[0]).normalize();
                if normal.z < 0.0 {
                    normal = -normal;
                }
                let grates = if gutter {
                    let e = area * GRATE_DENSITY;
                    u32::from(rng.f() < e)
                } else {
                    0
                };
                for g in 0..count + grates {
                    let (mut a, mut b) = (rng.f(), rng.f());
                    if a + b > 1.0 {
                        (a, b) = (1.0 - a, 1.0 - b);
                    }
                    let mut p = t.p[0] + (t.p[1] - t.p[0]) * a + (t.p[2] - t.p[0]) * b;
                    let near = if gutter { grid.nearest(&s.pave, p, 1.0) } else { Some(0.0) };
                    let Some(kerb) = near else { continue };
                    let light = t.light[0] * (1.0 - a - b) + t.light[1] * a + t.light[2] * b;
                    let seed = rng.f();
                    let yaw = rng.f() * std::f32::consts::TAU;
                    if g >= count {
                        // A drain grate: along the kerb (its long side
                        // across the distance field's gradient), 0.35 m off it.
                        let dist = |q: Vec3| grid.nearest(&s.pave, q, 2.0).unwrap_or(2.0);
                        let h = 0.25;
                        let grad = Vec2::new(
                            dist(p + Vec3::X * h) - dist(p - Vec3::X * h),
                            dist(p + Vec3::Y * h) - dist(p - Vec3::Y * h),
                        );
                        if grad.length() < 0.2 {
                            continue;
                        }
                        let away = grad.normalize();
                        let shift = 0.4 - kerb;
                        p += Vec3::new(away.x, away.y, 0.0) * shift;
                        p.z = t.p[0].z
                            + (-(normal.x * (p.x - t.p[0].x) + normal.y * (p.y - t.p[0].y)) / normal.z.max(0.5));
                        let along = away.perp();
                        let q = surface_rot(normal, along.y.atan2(along.x));
                        let steams = rng.f() < 0.5;
                        drains.push((p, steams));
                        out.push(Placed {
                            inst: Inst {
                                pos_s: [p.x, p.y, p.z, 0.9],
                                rot: q.to_array(),
                                light_kind: [light.x, light.y, light.z, Kind::Grate as u32 as f32],
                                extra: [seed, 0.5, 1.0, 1.0],
                            },
                            kind: Kind::Grate,
                            section: t.section,
                        });
                        continue;
                    }
                    let r = rng.f();
                    // Gutters collect leaves, paper and bottles; pavements paper,
                    // cups and cans.
                    let kind = if gutter {
                        match r {
                            r if r < 0.38 => Kind::Leaf,
                            r if r < 0.56 => Kind::Paper,
                            r if r < 0.64 => Kind::Can,
                            r if r < 0.69 => Kind::Bottle,
                            r if r < 0.75 => Kind::Cup,
                            r if r < 0.84 => Kind::Crumple,
                            r if r < 0.86 => Kind::Box,
                            _ => Kind::Stain,
                        }
                    } else {
                        match r {
                            r if r < 0.24 => Kind::Paper,
                            r if r < 0.44 => Kind::Leaf,
                            r if r < 0.56 => Kind::Cup,
                            r if r < 0.66 => Kind::Can,
                            r if r < 0.70 => Kind::Bottle,
                            r if r < 0.80 => Kind::Crumple,
                            r if r < 0.84 => Kind::Box,
                            _ => Kind::Stain,
                        }
                    };
                    let s1 = rng.f();
                    let s2 = rng.f();
                    // Scale, local y / z scales.
                    let (scale, sy, sz) = match kind {
                        Kind::Can => (0.95 + 0.1 * s1, 1.0, if s2 < 0.35 { 0.45 + 0.3 * s2 } else { 1.0 }),
                        Kind::Bottle => (0.9 + 0.2 * s1, 1.0, 1.0),
                        Kind::Cup => (0.9 + 0.25 * s1, 1.0, if s2 < 0.4 { 0.5 + 0.4 * s2 } else { 1.0 }),
                        Kind::Box => (0.7 + 0.6 * s1, 0.8 + 0.4 * s2, 0.35 + 0.65 * rng.f()),
                        Kind::Crumple => (0.8 + 0.6 * s1, 1.0, 0.8),
                        Kind::Leaf => (0.07 + 0.07 * s1, 0.6, 1.0),
                        Kind::Paper => (0.2 + 0.2 * s1, 0.75 + 0.2 * s2, 1.0),
                        _ => (0.3 + 0.55 * s1, 0.6 + 0.4 * s2, 1.0),
                    };
                    let q = surface_rot(normal, yaw);
                    out.push(Placed {
                        inst: Inst {
                            pos_s: [p.x, p.y, p.z, scale],
                            rot: q.to_array(),
                            light_kind: [light.x, light.y, light.z, kind as u32 as f32],
                            extra: [seed, sy, sz, 1.0],
                        },
                        kind,
                        section: t.section,
                    });
                    // Leaves lie in drifts: 2..9 more round the first, in its plane.
                    if kind == Kind::Leaf {
                        for _ in 0..2 + (rng.f() * 8.0) as u32 {
                            let off = Vec3::new(rng.f() - 0.5, rng.f() - 0.5, 0.0) * if gutter { 0.6 } else { 0.9 };
                            let q2 = p + off - normal * (normal.x * off.x + normal.y * off.y) / normal.z.max(0.5) * 0.0;
                            let z = p.z - (normal.x * off.x + normal.y * off.y) / normal.z.max(0.5);
                            out.push(Placed {
                                inst: Inst {
                                    pos_s: [q2.x, q2.y, z, 0.07 + 0.07 * rng.f()],
                                    rot: surface_rot(normal, rng.f() * std::f32::consts::TAU).to_array(),
                                    light_kind: [light.x, light.y, light.z, kind as u32 as f32],
                                    extra: [rng.f(), 0.6, 1.0, 1.0],
                                },
                                kind,
                                section: t.section,
                            });
                        }
                    }
                }
            }
            (out, drains)
        };
    use rayon::prelude::*;
    let scatter =
        |tris: &[Tri], density: f32, salt: u64, gutter: bool, out: &mut Vec<Placed>, drains: &mut Vec<(Vec3, bool)>| {
            let parts: Vec<(Vec<Placed>, Vec<(Vec3, bool)>)> =
                tris.par_iter().enumerate().map(|it| scatter_one(it, density, salt, gutter)).collect();
            for (o, d) in parts {
                out.extend(o);
                drains.extend(d);
            }
        };
    scatter(&s.pave, d_pave, 0x5041_5645, false, &mut out, &mut drains);
    scatter(&s.road, d_gutter, 0x4755_5454, true, &mut out, &mut drains);
    Placement { props: out, drains }
}

/// The road grills' steam vents: their triangles' centres, one per ~4 m,
/// every other one steaming (deterministic).
pub(crate) fn grill_vents(grills: &[Tri]) -> Vec<Vec3> {
    let mut cells: std::collections::BTreeMap<(i32, i32), (Vec3, f32)> = Default::default();
    for t in grills {
        let c = (t.p[0] + t.p[1] + t.p[2]) / 3.0;
        let a = 0.5 * (t.p[1] - t.p[0]).cross(t.p[2] - t.p[0]).length();
        let e = cells.entry(((c.x / 4.0).floor() as i32, (c.y / 4.0).floor() as i32)).or_insert((Vec3::ZERO, 0.0));
        e.0 += c * a;
        e.1 += a;
    }
    cells
        .into_iter()
        .filter(|(k, (_, a))| *a > 0.5 && hash(((k.0 as u64) << 32) ^ (k.1 as u32 as u64)) & 1 == 0)
        .map(|(_, (c, a))| c / a)
        .collect()
}
