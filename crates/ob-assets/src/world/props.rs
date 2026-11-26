//! The knock-over props a track places (`smackable`, the knock-over simulation, steps them): one prop's data and
//! the pairing of the track's object instances with their infos.

use glam::{Mat3, Mat4, Vec3};

use crate::formats::collision::CollisionVolume;
use crate::formats::smackable::{ObjectInfo, ObjectInstance, SceneryGroup};

use super::World;

/// One knock-over prop: what it is and where it stands.
#[derive(Clone, Debug)]
pub struct Prop {
    /// The world placement it moves.
    pub placement: usize,
    /// The `0x34107` table entry it stands in for (instance +4).
    pub scenery: u16,
    pub info: ObjectInfo,
    /// The collision shape about the centre of mass, prop axes: the `ZCS_`
    /// volume less the centre of mass, or the box (0x5B1550).
    pub shape: Vec<Vec3>,
    /// The shape's offset (+0x40 with +0x3E set): the info's centre of
    /// mass for a `ZCS_` volume (0x5B1690 looks the volume up with the
    /// rigid body's +0x1F0; 0x5B5060 writes the body's vtable +0x1C, the
    /// same +0x1F0, before each detection), none for the box (a type-3
    /// shape). The distance query places the shape's fresh supports
    /// R·this off ([`crate::narrow::Raw`]).
    pub offset: Vec3,
    /// The `ZCS_` volume's vertices as stored (the offset not removed),
    /// prop axes: a fresh support's point is one of these through the
    /// body's transform (0x5AFBB0 / 0x5AFF2A); empty for the box.
    pub raw_shape: Vec<Vec3>,
    /// The volume's vertex links (0x5AFBB0's climb); empty for the box.
    pub links: crate::formats::collision::HullLinks,
    /// The body's reach (0x5AE530): the farthest shape point, and the
    /// shape's height above and below the centre of mass plus 0.05 m.
    pub reach: f32,
    pub above: f32,
    pub below: f32,
    pub rest: Mat4,
    /// The unhit body's world box (0x5A0690) and radius (0x5AD900).
    pub world_min: Vec3,
    pub world_max: Vec3,
    pub radius: f32,
}

impl Prop {
    /// Pair instances with infos (by key, as 0x5AD900) and with the scenery
    /// placement they stand in for (`placement_of`: the instance's +4
    /// through the `0x34107` table, 0x5D6440, to a placement and the
    /// table's kind). Kind 1 becomes an unhit body (0x5B0C40); another kind
    /// only when the entry is listed in a scenery group (`0x34108`) that is
    /// on and is not the one named `SMOKEABLE` (0x5B0C40 at 0x5B0D7A skips
    /// that name: it lists every prop, 601 entries in L4RA covering all
    /// 514 placements, all kind 1).
    pub fn pair(
        world: &World,
        infos: &[ObjectInfo],
        instances: &[ObjectInstance],
        volumes: &[CollisionVolume],
        placement_of: impl Fn(u16) -> Option<(usize, u8)>,
        groups: &[SceneryGroup],
    ) -> Vec<Prop> {
        let listed =
            |scenery: u16| groups.iter().any(|g| g.on && g.name != "SMOKEABLE" && g.members.contains(&scenery));
        let mut out = Vec::new();
        for inst in instances {
            let Some(info) = infos.iter().find(|i| i.key == inst.info_key) else { continue };
            let Some((placement, kind)) = placement_of(inst.scenery) else { continue };
            if kind != 1 && !listed(inst.scenery) {
                continue;
            }
            let p = &world.placements[placement];
            let solid = &world.solids[p.solid];
            let com = info.centre_of_mass;
            let volume = volumes.iter().find(|v| v.hash == info.key && !v.vertices.is_empty());
            let offset = if volume.is_some() { com } else { Vec3::ZERO };
            let links = volume.map(|v| v.links.clone()).unwrap_or_default();
            let shape: Vec<Vec3> = match volume {
                Some(v) => v.vertices.iter().map(|q| *q - com).collect(),
                None => {
                    let half = (Vec3::from(solid.bbox_max) - Vec3::from(solid.bbox_min)) * 0.5;
                    (0..8)
                        .map(|i| {
                            half * Vec3::new(
                                if i & 1 == 0 { -1.0 } else { 1.0 },
                                if i & 2 == 0 { -1.0 } else { 1.0 },
                                if i & 4 == 0 { -1.0 } else { 1.0 },
                            )
                        })
                        .collect()
                }
            };
            let reach = shape.iter().fold(0.0f32, |r, q| r.max(q.length()));
            let top = shape.iter().fold(0.0f32, |t, q| t.max(q.z));
            let bottom = shape.iter().fold(0.0f32, |b, q| b.min(q.z));
            out.push(Prop {
                placement,
                scenery: inst.scenery,
                info: info.clone(),
                shape,
                offset,
                raw_shape: volume.map(|v| v.vertices.clone()).unwrap_or_default(),
                links,
                reach,
                above: top + 0.05,
                below: 0.05 - bottom,
                rest: p.transform,
                world_min: inst.bbox_min,
                world_max: inst.bbox_max,
                radius: (inst.bbox_max - inst.bbox_min).length() * 0.5,
            });
        }
        out
    }
}

/// The prop as a rigid body (the knock-over simulation, `smackable`, steps it).
impl Prop {
    pub fn mass(&self) -> f32 {
        self.info.mass.max(1e-4)
    }

    /// World inverse inertia R·I⁻¹·Rᵀ: the rigid body's own update
    /// (0x59BC70, 0x5A0D90) turns its body-space one (+0x1B0, the yaw term
    /// × info +0x70, which is 1 for every L4RA kind; +0x158 is set).
    pub fn inverse_inertia(&self, rot: Mat3) -> Mat3 {
        rot * self.body_inverse_inertia() * rot.transpose()
    }

    /// The body-space inverse inertia as the contact impulse 0x5AF0F0 takes
    /// it: vtable +0x60 (0x5B1C50) returns body +0x1B0 (+0x170 without
    /// +0x158) and 0x43C9C0 applies it to the world-space r × n unturned,
    /// as for a car (0x593A50, `contact::Body::inverse_inertia`). As
    /// observed under Wine: the player's Skyline through a ZCS_FenceH on
    /// S4_H_DRAG_15 took |J| 5.13 (ΔP (−3.121, −1.645, 0.021) × 1.85 / m), the
    /// turned tensor gives 3.36, the unturned one 5.1.
    pub fn body_inverse_inertia(&self) -> Mat3 {
        if self.info.inertia.determinant().abs() > 1e-12 {
            self.info.inertia.inverse()
        } else {
            Mat3::from_diagonal(Vec3::splat(1e6))
        }
    }
}

// The world's cache keeps the props.
crate::persist!(Prop { placement, scenery, info, shape, offset, raw_shape, links, reach, above, below, rest, world_min, world_max, radius });
