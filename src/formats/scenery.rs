//! Scenery: where solids are placed in the world.
//!
//! Each SCENERY_SECTION holds a table of *infos* (one per distinct prop, with
//! up to three LOD solid hashes) and a table of *instances* (a placement of
//! one info).

use anyhow::{Context, Result};
use glam::{Mat3, Vec3};

use super::bundle::{Chunk, id};
use super::reader::Reader;

#[derive(Clone, Debug)]
pub struct SceneryInfo {
    pub name: String,
    /// Solid hashes, highest detail first. `0` means "no solid at this LOD".
    pub lod_solids: [u32; 3],
    /// Bounding radius (+0x3C). The scenery cull (0x5D6580) adds 6
    /// (0x7A47D4) and sizes the prop on screen with it (0x5D64E0).
    pub radius: f32,
}

#[derive(Clone, Debug)]
pub struct SceneryInstance {
    pub bbox_min: [f32; 3],
    pub bbox_max: [f32; 3],
    /// Index into the section's [`SceneryInfo`] table.
    pub info: u16,
    pub flags: u16,
    pub position: Vec3,
    /// `p_world = rotation * p_local + position`. The file stores the local
    /// X, Y, Z axes as rows and the game multiplies row vectors (`p * M`);
    /// that convention was checked against the stored bounding boxes.
    pub rotation: Mat3,
}

#[derive(Clone, Debug, Default)]
pub struct ScenerySection {
    pub section_number: u32,
    pub infos: Vec<SceneryInfo>,
    pub instances: Vec<SceneryInstance>,
}

const INFO_SIZE: usize = 0x44;
const INSTANCE_SIZE: usize = 0x40;
/// Rotation entries are signed 2.13 fixed point.
const ROTATION_ONE: f32 = 8192.0;

pub fn parse_scenery_section(section: Chunk<'_>) -> Result<ScenerySection> {
    let header = section.child(id::SCENERY_HEADER).context("scenery header")?;
    super::bundle::ensure_len(header.body, 0x10, "scenery header")?;
    let section_number = Reader::new(header.body).u32_at(0x0C)?;

    let infos = section.child(id::SCENERY_INFOS).context("scenery infos")?;
    let infos = Reader::with_base(infos.body, infos.offset)
        .whole_records(INFO_SIZE)
        .map(|r| {
            Ok(SceneryInfo {
                name: r.cstr_at(0, 0x20)?,
                lod_solids: [r.u32_at(0x20)?, r.u32_at(0x24)?, r.u32_at(0x28)?],
                radius: r.f32_at(0x3C)?,
            })
        })
        .collect::<Result<_>>()?;

    let instances = section.child(id::SCENERY_INSTANCES).context("scenery instances")?;
    let instances = Reader::new(instances.aligned(16))
        .whole_records(INSTANCE_SIZE)
        .map(|r| {
            let mut m = [0f32; 9];
            for (i, v) in m.iter_mut().enumerate() {
                *v = r.i16_at(0x2C + i * 2)? as f32 / ROTATION_ONE;
            }
            Ok(SceneryInstance {
                bbox_min: r.vec3_at(0x00)?,
                bbox_max: r.vec3_at(0x0C)?,
                info: r.u16_at(0x18)?,
                flags: r.u16_at(0x1A)?,
                position: Vec3::new(r.f32_at(0x20)?, r.f32_at(0x24)?, r.f32_at(0x28)?),
                // glam is column-major: storing the file's rows as columns
                // gives the transpose, so `rotation * p` == the game's `p * M`.
                rotation: Mat3::from_cols_array(&m),
            })
        })
        .collect::<Result<_>>()?;

    Ok(ScenerySection { section_number, infos, instances })
}
