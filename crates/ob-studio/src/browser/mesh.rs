//! The Browser's mesh viewer: a model's geometry in numbers (vertices, triangles, size, its parts and what each
//! part draws). The window's viewer under the model's picture and `browser mesh` on the command line read it, so
//! both say the same.

use std::collections::HashMap;

use ob_formats::formats::solid::Solid;
use ob_formats::formats::texture::Texture;

/// One part (a draw range of one texture slot).
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    /// The part's name as a glTF export gives it (the texture's name, else `slot<N>`).
    pub name: String,
    pub triangles: usize,
    /// The texture's hash, when the slot has one.
    pub texture: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Facts {
    pub name: String,
    pub hash: u32,
    pub vertices: usize,
    pub triangles: usize,
    /// Whether it carries normals (lit).
    pub lit: bool,
    pub min: [f32; 3],
    pub max: [f32; 3],
    /// Metres, per axis (the game's X, Y, Z-up).
    pub size: [f32; 3],
    pub parts: Vec<Part>,
    pub markers: usize,
}

/// The facts of `solid`. Parts of one slot are one part, as the glTF export makes them.
pub fn facts(solid: &Solid, textures: &HashMap<u32, Texture>) -> Facts {
    let (mut min, mut max) = ([f32::MAX; 3], [f32::MIN; 3]);
    for v in &solid.vertices {
        for k in 0..3 {
            min[k] = min[k].min(v.position[k]);
            max[k] = max[k].max(v.position[k]);
        }
    }
    if solid.vertices.is_empty() {
        (min, max) = ([0.0; 3], [0.0; 3]);
    }
    let mut parts: Vec<(u8, Part)> = vec![];
    for m in &solid.materials {
        let (first, count) = (m.first_index as usize, m.index_count as usize);
        if count == 0 || first + count > solid.indices.len() {
            continue;
        }
        match parts.iter_mut().find(|p| p.0 == m.texture_slot) {
            Some(p) => p.1.triangles += count / 3,
            None => parts.push((
                m.texture_slot,
                Part {
                    name: super::gltf::part_name(solid, textures, m.texture_slot),
                    triangles: count / 3,
                    texture: solid.textures.get(m.texture_slot as usize).copied(),
                },
            )),
        }
    }
    Facts {
        name: solid.name.clone(),
        hash: solid.hash,
        vertices: solid.vertices.len(),
        triangles: solid.indices.len() / 3,
        lit: !solid.normals.is_empty() && solid.normals.len() == solid.vertices.len(),
        min,
        max,
        size: [max[0] - min[0], max[1] - min[1], max[2] - min[2]],
        parts: parts.into_iter().map(|p| p.1).collect(),
        markers: solid.markers.len(),
    }
}

/// What a model without geometry (an animation helper) says in place of its sizes.
pub const NO_MESH: &str = "No mesh (helper)";

impl Facts {
    /// The model has no geometry at all: a helper that only carries a name and a place.
    pub fn is_helper(&self) -> bool {
        self.vertices == 0 && self.triangles == 0
    }

    /// The lines the viewer and the command line show.
    pub fn lines(&self) -> Vec<String> {
        if self.is_helper() {
            return vec![NO_MESH.to_string()];
        }
        let mut l = vec![
            format!(
                "{} vertices · {} triangles · {}",
                self.vertices,
                self.triangles,
                if self.lit { "lit (normals)" } else { "unlit" }
            ),
            format!("Size {:.2} × {:.2} × {:.2} m (Z up)", self.size[0], self.size[1], self.size[2]),
        ];
        for p in &self.parts {
            l.push(format!("Part {}: {} triangles", p.name, p.triangles));
        }
        l
    }
}

#[cfg(test)]
mod tests;
