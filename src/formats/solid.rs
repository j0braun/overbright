//! Solids: the static meshes that make up the world (and props, and cars).
//!
//! ```text
//! SOLID_LIST (0x80134000)
//! ├─ SOLID_LIST_HEADER (0x80134001)   pipeline path, hash table of members
//! └─ SOLID (0x80134010)                one per mesh
//!    ├─ SOLID_HEADER   (0x00134011)   name, hash, bounding box
//!    ├─ SOLID_TEXTURES (0x00134012)   texture hashes this mesh uses
//!    └─ SOLID_MESH     (0x80134100)
//!       ├─ MESH_HEADER    (0x00134900) counts
//!       ├─ MESH_VERTICES  (0x00134B01) 24-byte vertices
//!       ├─ MESH_MATERIALS (0x00134B02) 60-byte draw ranges
//!       └─ MESH_INDICES   (0x00134B03) u16 triangle list
//! ```

use anyhow::{Context, Result, bail};

use super::bundle::{Chunk, ensure_len, id};
use super::reader::{Limits, Reader};

/// A solid header's hash (+0x10 of its 16-byte aligned struct), if the
/// header holds one.
fn header_hash(solid: &Chunk<'_>) -> Option<u32> {
    solid.child(id::SOLID_HEADER).and_then(|h| Reader::new(h.aligned(16)).u32_at(0x10).ok())
}

/// The world vertex format: position, prelit colour, one UV set.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    /// Baked lighting, stored as D3DCOLOR (bytes B, G, R, A).
    pub color: [u8; 4],
    pub uv: [f32; 2],
}

pub const VERTEX_STRIDE: usize = 24;
/// Car (and other lit) meshes: position, normal, colour, UV. Normals were
/// checked to be unit length on every 240SX vertex.
pub const LIT_VERTEX_STRIDE: usize = 36;

/// One contiguous run of indices drawn with one texture.
#[derive(Clone, Debug)]
pub struct Material {
    /// Index into [`Solid::textures`].
    pub texture_slot: u8,
    /// Index into [`Solid::light_materials`]; 0xFF = the default material.
    pub light_material: u8,
    pub first_index: u32,
    pub index_count: u32,
    /// The record's flags (+0x38): the car draw's flags. 0x5C5BB1 queues
    /// them with the draw (0x5CEB40 → entry +0x14) and 0x5BD4B0 builds
    /// car.efx's constants from them (0x5CF559): 0x100 clear → no env map,
    /// 0x80 clear → no diffuse / specular. They differ from the mesh
    /// header's (`Solid::mesh_flags`) per material, e.g. the 240SX's
    /// BASE_A / TRUNK_A DULLPLASTIC ranges carry 0x4080 in a 0xC180 mesh
    /// (docs/car-shading-compare.md).
    pub flags: u32,
}

#[derive(Clone, Debug)]
pub struct Solid {
    pub name: String,
    pub hash: u32,
    pub bbox_min: [f32; 3],
    pub bbox_max: [f32; 3],
    /// Hashes of the textures referenced by [`Material::texture_slot`].
    pub textures: Vec<u32>,
    pub vertices: Vec<Vertex>,
    /// One per vertex for 36-byte (lit) meshes, empty otherwise.
    pub normals: Vec<[f32; 3]>,
    /// Light-material name hashes (chunk 0x00134013), looked up in the
    /// global table (GLOBALB 0x00135200) to light car parts.
    pub light_materials: Vec<u32>,
    pub indices: Vec<u16>,
    pub materials: Vec<Material>,
    /// Mesh header flags (+0x0C). SPEED2.EXE 0x5C58B0 picks the mesh's
    /// effect from these through the table at 0x800464: 0x01 → worldbone,
    /// 0x40 → worldreflect (wet-road reflection), 0x80 → car; else world.
    pub mesh_flags: u32,
    /// The solid's position markers (chunk 0x13401A), as
    /// [`solid_markers`] reads them: (name hash, column-major matrix).
    pub markers: Vec<(u32, [f32; 16])>,
    /// The markers as the scenery cull 0x5D6580 reads them for its light
    /// flares (0x5D6863…0x5D69A9, walking 0x48D8A0): (record +0x04, the
    /// marker's kind; the translation row +0x40 less the header's +0x70).
    pub light_markers: Vec<(u32, glam::Vec3)>,
}

pub const MESH_FLAG_SKINNED: u32 = 0x01;
/// Set on most car meshes; 0x5BD4B0 uses it to enable the environment map.
pub const MESH_FLAG_NORMALS: u32 = 0x100;

/// Whether a mesh has the 36-byte lit vertices (with a normal): the car
/// effect's (flag 0x80, 0x5C58B0). Every one of the 18 727 meshes with 0x80
/// in CARS\*\GEOMETRY.BIN has them, the 346 without 0x100 too (the
/// HOOD_UNDER / TRUNK_UNDER models and PLATES' LICENSE_PLATE_STYLE01_A,
/// flags 0x4080).
fn mesh_flags_have_normals(flags: u32) -> bool {
    flags & (MESH_FLAG_NORMALS | MESH_FLAG_CAR_SHADER) != 0
}
pub const MESH_FLAG_REFLECTIVE: u32 = 0x40;
pub const MESH_FLAG_CAR_SHADER: u32 = 0x80;

const SOLID_LIGHT_MATERIALS: u32 = 0x0013_4013;

/// Names this long were cut off by the 28-byte header field.
const TRUNCATED_NAME_LEN: usize = 27;

/// Parse every solid in a SOLID_LIST chunk.
pub fn parse_solid_list(list: Chunk<'_>) -> Result<Vec<Solid>> {
    parse_in_order(list.children().filter(|c| c.id == id::SOLID).collect())
}

/// The solids parsed in parallel, in their order (the first error in that
/// order, as a sequential parse reports it).
pub fn parse_in_order(solids: Vec<Chunk<'_>>) -> Result<Vec<Solid>> {
    use rayon::prelude::*;
    let parsed: Vec<Result<Solid>> = solids.into_par_iter().map(parse_solid).collect();
    parsed.into_iter().collect()
}

/// Parse only the solids of a SOLID_LIST whose hash `want` accepts (the
/// header's +0x10), skipping the meshes of the others.
pub fn parse_solid_list_where(list: Chunk<'_>, want: &dyn Fn(u32) -> bool) -> Result<Vec<Solid>> {
    parse_in_order(
        list.children()
            .filter(|c| c.id == id::SOLID)
            .filter(|c| header_hash(c).is_some_and(want))
            .collect(),
    )
}

/// A solid's position markers (chunk 0x13401A, 0x50-byte records from its
/// 16-byte aligned start: +0x00 the name hash, +0x10 a 4×4 row-vector
/// matrix, translation in the last row), as 0x488FA0 searches them by
/// hash. Returned as (hash, column-major car-space matrix).
pub fn solid_markers(list: Chunk<'_>, solid_hash: u32) -> Vec<(u32, [f32; 16])> {
    let Some(solid) = list.children().filter(|c| c.id == id::SOLID).find(|c| header_hash(c) == Some(solid_hash)) else {
        return Vec::new();
    };
    markers_of(solid)
}

fn markers_of(solid: Chunk<'_>) -> Vec<(u32, [f32; 16])> {
    solid
        .children()
        .filter(|c| c.id == SOLID_MARKERS)
        .flat_map(|c| {
            Reader::new(c.aligned(16))
                .whole_records(0x50)
                .filter_map(|r| {
                    let m = r.array_at::<64>(0x10).ok()?;
                    Some((r.u32_at(0).ok()?, std::array::from_fn(|i| f32::from_le_bytes([m[i * 4], m[i * 4 + 1], m[i * 4 + 2], m[i * 4 + 3]]))))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// A solid's header +0x70: the translation row of the matrix at +0x40
/// (render info +0x30 takes the body's, 0x6389A0).
pub fn solid_translation(list: Chunk<'_>, solid_hash: u32) -> Option<glam::Vec3> {
    let solid = list.children().filter(|c| c.id == id::SOLID).find(|c| header_hash(c) == Some(solid_hash))?;
    let h = Reader::new(solid.child(id::SOLID_HEADER)?.aligned(16));
    h.vec3_at(0x70).ok().map(glam::Vec3::from)
}

/// Position markers in a solid (0x13401A).
const SOLID_MARKERS: u32 = 0x0013_401A;

fn parse_solid(solid: Chunk<'_>) -> Result<Solid> {
    // The header struct is 16-byte aligned in the file; its offset inside the
    // chunk therefore varies.
    let header = solid.child(id::SOLID_HEADER).context("solid without header")?;
    let h = Reader::new(header.aligned(16));
    ensure_len(h.data(), 0xA4, "solid header")?;
    let hash = h.u32_at(0x10)?;
    let bbox_min = h.vec3_at(0x20)?;
    let bbox_max = h.vec3_at(0x30)?;
    let mut name = h.cstr_at(0xA4, h.len() - 0xA4)?;
    if name.len() >= TRUNCATED_NAME_LEN {
        if let Some(full) = super::hash::complete_truncated_name(&name, hash, 4) {
            name = full;
        }
    }

    // Hash tables of 8-byte entries (hash, 0).
    let hashes = |c: Option<Chunk<'_>>| -> Result<Vec<u32>> {
        match c {
            Some(t) => Reader::new(t.body).whole_records(8).map(|e| Ok(e.u32_at(0)?)).collect(),
            None => Ok(Vec::new()),
        }
    };
    let textures = hashes(solid.child(id::SOLID_TEXTURES))?;

    let mut out = Solid {
        name,
        hash,
        bbox_min,
        bbox_max,
        textures,
        vertices: Vec::new(),
        normals: Vec::new(),
        light_materials: hashes(solid.child(SOLID_LIGHT_MATERIALS))?,
        indices: Vec::new(),
        materials: Vec::new(),
        mesh_flags: 0,
        markers: markers_of(solid),
        light_markers: Vec::new(),
    };
    if let Ok(origin) = h.vec3_at(0x70) {
        let origin = glam::Vec3::from(origin);
        let mut markers = Vec::new();
        for c in solid.children().filter(|c| c.id == SOLID_MARKERS) {
            for r in Reader::new(c.aligned(16)).whole_records(0x50) {
                markers.push((r.u32_at(4)?, glam::Vec3::from(r.vec3_at(0x40)?) - origin));
            }
        }
        out.light_markers = markers;
    }
    if let Some(mesh) = solid.child(id::SOLID_MESH) {
        if let Err(e) = parse_mesh(mesh, &mut out) {
            log::warn!("solid {}: mesh skipped: {e:#}", out.name);
            out.vertices.clear();
            out.indices.clear();
            out.materials.clear();
            // A lit mesh with no vertices must not look lit.
            out.normals.clear();
            out.mesh_flags = 0;
        }
    }
    Ok(out)
}

fn parse_mesh(mesh: Chunk<'_>, out: &mut Solid) -> Result<()> {
    // Like the solid header, the mesh header is 16-byte aligned in the file.
    let header = Reader::new(mesh.child(id::MESH_HEADER).context("mesh header")?.aligned(16));
    ensure_len(header.data(), 0x38, "mesh header")?;
    let vertex_count = header.u32_at(0x34)? as usize;
    out.mesh_flags = header.u32_at(0x0C)?;
    if vertex_count == 0 {
        return Ok(()); // placeholder solid with no geometry
    }

    let vertex_bytes = mesh.child(id::MESH_VERTICES).context("vertices")?.payload();
    // 24-byte world vertices or 36-byte lit vertices (with a normal).
    let stride = match vertex_bytes.len() / vertex_count {
        n if n >= LIT_VERTEX_STRIDE && mesh_flags_have_normals(out.mesh_flags) => LIT_VERTEX_STRIDE,
        _ => VERTEX_STRIDE,
    };
    if vertex_bytes.len() < vertex_count * stride {
        bail!("vertex buffer holds {} bytes, need {}", vertex_bytes.len(), vertex_count * stride);
    }
    let (color_at, uv_at) = if stride == LIT_VERTEX_STRIDE { (24, 28) } else { (12, 16) };
    let mut vertices = Reader::new(vertex_bytes);
    for v in vertices.records("vertices", vertex_count, stride, Limits::DEFAULT.max_count)? {
        let v = Reader::new(v);
        out.vertices.push(Vertex {
            position: v.vec3_at(0)?,
            color: v.array_at(color_at)?,
            uv: [v.f32_at(uv_at)?, v.f32_at(uv_at + 4)?],
        });
        if stride == LIT_VERTEX_STRIDE {
            out.normals.push(v.vec3_at(12)?);
        }
    }

    let material_count = header.u32_at(0x10)? as usize;
    let materials = mesh.child(id::MESH_MATERIALS).context("materials")?.payload();
    const MATERIAL_SIZE: usize = 0x3C;
    let mut materials = Reader::new(materials);
    out.materials = materials
        .records("material table", material_count, MATERIAL_SIZE, Limits::DEFAULT.max_count)
        .context("material table")?
        .map(|m| {
            let m = Reader::new(m);
            Ok(Material {
                index_count: m.u32_at(0x0C)?,
                texture_slot: m.u8_at(0x1C)?,
                light_material: m.u8_at(0x20)?,
                first_index: m.u32_at(0x34)?,
                flags: m.u32_at(0x38)?,
            })
        })
        .collect::<Result<_>>()?;

    let index_bytes = mesh.child(id::MESH_INDICES).context("indices")?.payload();
    // In u64 (fuzzing: two u32 fields of a corrupt material wrapped to a
    // small count that passed the length check).
    let index_count = out.materials.iter().map(|m| u64::from(m.first_index) + u64::from(m.index_count)).max().unwrap_or(0);
    let index_count = usize::try_from(index_count).ok().filter(|&n| n <= index_bytes.len() / 2).context("index buffer: too short for the materials' ranges")?;
    out.indices = Reader::new(index_bytes).u16s("index buffer", index_count, usize::MAX)?;

    if let Some(&bad) = out.indices.iter().find(|&&i| i as usize >= out.vertices.len()) {
        bail!("index {bad} out of range ({} vertices)", out.vertices.len());
    }
    Ok(())
}

crate::persist!(Solid { name, hash, bbox_min, bbox_max, textures, vertices: pod, normals: pod, light_materials, indices: pod, materials, mesh_flags, markers, light_markers });
crate::persist!(Material { texture_slot, light_material, first_index, index_count, flags });
