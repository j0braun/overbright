//! A model as a glTF 2.0 reference for an outside 3D program (Blender, any glTF viewer).
//!
//! The files are read-only references: the geometry and textures are the player's own game files, so every export
//! says so (the glTF's `asset.extras` and a `LOCAL-ONLY.txt` beside it: `origin = "from-game-files"`,
//! `share = "local-only"`, the asset format's provenance rules, docs/modding/asset-format/manifest.md §3) and the
//! shared copy of a mod never carries them ([`reference_dir`] is under the mod's `.editor/`). Nothing is written
//! into the game's folder ([`check_target`]). The layout follows docs/modding/asset-format/meshes.md §2.2 and
//! §3: `.gltf` plus `.bin`, a root node turning the game's +Z up into glTF's +Y up, one primitive per material.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use nfsu2::studio_api::world::{HIDDEN_FLAG, Placement};
use ob_formats::formats::solid::Solid;
use ob_formats::formats::texture::Texture;
use serde_json::{Value, json};

/// An export's failure, as words for the verdict line.
pub type Result<T> = std::result::Result<T, String>;

macro_rules! bail {
    ($($a:tt)*) => { return Err(format!($($a)*)) };
}

/// The note written beside every export.
pub const NOTE_FILE: &str = "LOCAL-ONLY.txt";

pub(super) const NOTE: &str = "These files are exported from your own copy of the game, for looking at and working from in your own \
3D program.\norigin = \"from-game-files\"\nshare = \"local-only\"\nDo not share or publish them: they hold the game's \
models, textures and sounds.\n";

/// Where a mod's references go by default: its `.editor/reference/` folder, which the mod's exports leave out.
pub fn reference_dir(mod_folder: &Path) -> PathBuf {
    mod_folder.join(".editor").join("reference")
}

/// A refusal to write into the game's own files: `dir` is the game folder or inside it.
pub fn check_target(dir: &Path, game_root: Option<&Path>) -> Result<()> {
    let Some(root) = game_root else { return Ok(()) };
    let canon = |p: &Path| {
        // The folder may not exist yet: resolve its nearest existing parent.
        let mut tail = PathBuf::new();
        let mut cur = p.to_path_buf();
        loop {
            if let Ok(c) = cur.canonicalize() {
                return c.join(tail);
            }
            match (cur.file_name().map(PathBuf::from), cur.parent().map(Path::to_path_buf)) {
                (Some(n), Some(par)) => {
                    tail = PathBuf::from(n).join(tail);
                    cur = par;
                }
                _ => return p.to_path_buf(),
            }
        }
    };
    if canon(dir).starts_with(canon(root)) {
        bail!("not into the game's own folder: choose the mod's folder or another one");
    }
    Ok(())
}

/// A file-name-safe form of a model name (letters, digits, `-`, `_`, `.`; never empty, never a path).
pub fn file_stem(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') { c } else { '_' })
        .collect();
    if s.is_empty() { "model".into() } else { s }
}

/// What an export wrote.
#[derive(Clone, Debug, PartialEq)]
pub struct Reference {
    pub gltf: PathBuf,
    pub bin: PathBuf,
    pub textures: Vec<PathBuf>,
    pub note: PathBuf,
}

/// The name of a part whose texture is not known: `slot<N>`.
pub(crate) fn slot_name(slot: u8) -> String {
    format!("slot{slot}")
}

/// The name an export gives the part (glTF material) that draws texture slot `slot` of `solid`: the texture's
/// name, else [`slot_name`]. An import maps a file's materials back onto the original's parts by it.
pub fn part_name(solid: &Solid, textures: &HashMap<u32, Texture>, slot: u8) -> String {
    solid
        .textures
        .get(slot as usize)
        .and_then(|h| textures.get(h))
        .map_or_else(|| slot_name(slot), |t| t.name.clone())
}

fn pad4(v: &mut Vec<u8>) {
    while v.len() % 4 != 0 {
        v.push(0);
    }
}

fn floats(v: impl Iterator<Item = f32>, out: &mut Vec<u8>) {
    for f in v {
        out.extend_from_slice(&f.to_le_bytes());
    }
}

/// A glTF being built: one buffer, the meshes added so far and the materials and images they share.
struct Scene<'a> {
    textures: &'a HashMap<u32, Texture>,
    /// Whether two texture slots (of one model or of two) that draw one texture share one glTF material (an
    /// area does; a single model keeps one material per slot, as an import maps them back).
    by_texture: bool,
    buf: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
    materials: Vec<Value>,
    gtextures: Vec<Value>,
    images: Vec<Value>,
    wanted: Vec<(String, &'a Texture)>,
    meshes: Vec<Value>,
    /// (texture hash, material) for [`Scene::by_texture`].
    shared: Vec<(u32, usize)>,
}

impl<'a> Scene<'a> {
    fn new(textures: &'a HashMap<u32, Texture>, by_texture: bool) -> Scene<'a> {
        Scene {
            textures,
            by_texture,
            buf: vec![],
            views: vec![],
            accessors: vec![],
            materials: vec![],
            gtextures: vec![],
            images: vec![],
            wanted: vec![],
            meshes: vec![],
            shared: vec![],
        }
    }

    fn view(&mut self, data: &[u8], target: u32) -> usize {
        pad4(&mut self.buf);
        self.views.push(json!({ "buffer": 0, "byteOffset": self.buf.len(), "byteLength": data.len(), "target": target }));
        self.buf.extend_from_slice(data);
        self.views.len() - 1
    }

    /// Add `solid` as the next mesh; `stem` names the texture files.
    fn add_mesh(&mut self, solid: &Solid, stem: &str) -> Result<usize> {
        let n = solid.vertices.len();
        if n == 0 || solid.indices.is_empty() {
            bail!("{} has no geometry to export", solid.name);
        }
        if solid.indices.iter().any(|&i| i as usize >= n) {
            bail!("{} has an index past its last vertex", solid.name);
        }
        let lit = solid.normals.len() == n;
        // Positions, with the exact bounds glTF requires.
        let mut pos = vec![];
        floats(solid.vertices.iter().flat_map(|v| v.position), &mut pos);
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for v in &solid.vertices {
            for k in 0..3 {
                lo[k] = lo[k].min(v.position[k]);
                hi[k] = hi[k].max(v.position[k]);
            }
        }
        let bv = self.view(&pos, 34962);
        self.accessors.push(json!({ "bufferView": bv, "componentType": 5126, "count": n, "type": "VEC3", "min": lo, "max": hi }));
        let mut attrs = json!({ "POSITION": self.accessors.len() - 1 });
        if lit {
            let mut nb = vec![];
            floats(solid.normals.iter().flatten().copied(), &mut nb);
            let bv = self.view(&nb, 34962);
            self.accessors.push(json!({ "bufferView": bv, "componentType": 5126, "count": n, "type": "VEC3" }));
            attrs["NORMAL"] = json!(self.accessors.len() - 1);
        }
        let mut uv = vec![];
        floats(solid.vertices.iter().flat_map(|v| v.uv), &mut uv);
        let bv = self.view(&uv, 34962);
        self.accessors.push(json!({ "bufferView": bv, "componentType": 5126, "count": n, "type": "VEC2" }));
        attrs["TEXCOORD_0"] = json!(self.accessors.len() - 1);
        let idx: Vec<u8> = solid.indices.iter().flat_map(|i| i.to_le_bytes()).collect();
        let ibv = self.view(&idx, 34963);

        // One glTF material per texture slot a draw range uses; one primitive per range.
        let mut mat_of_slot: HashMap<u8, usize> = HashMap::new();
        let mut prims = vec![];
        for m in &solid.materials {
            let (first, count) = (m.first_index as usize, m.index_count as usize);
            if count == 0 || first + count > solid.indices.len() {
                continue;
            }
            let hash = solid.textures.get(m.texture_slot as usize).copied();
            let known = if self.by_texture { hash.and_then(|h| self.shared_material(h)) } else { None };
            let mi = match known.or_else(|| mat_of_slot.get(&m.texture_slot).copied()) {
                Some(mi) => mi,
                None => {
                    let mi = self.new_material(solid, m.texture_slot, stem);
                    mat_of_slot.insert(m.texture_slot, mi);
                    if self.by_texture && let Some(h) = hash {
                        self.shared.push((h, mi));
                    }
                    mi
                }
            };
            self.accessors.push(json!({
                "bufferView": ibv, "byteOffset": first * 2, "componentType": 5123, "count": count, "type": "SCALAR",
            }));
            prims.push(json!({ "attributes": attrs, "indices": self.accessors.len() - 1, "material": mi, "mode": 4 }));
        }
        if prims.is_empty() {
            bail!("{} has no drawable material range", solid.name);
        }
        self.meshes.push(json!({ "name": solid.name, "primitives": prims }));
        Ok(self.meshes.len() - 1)
    }

    fn shared_material(&self, hash: u32) -> Option<usize> {
        self.shared.iter().find(|s| s.0 == hash).map(|s| s.1)
    }

    fn new_material(&mut self, solid: &Solid, slot: u8, stem: &str) -> usize {
        let hash = solid.textures.get(slot as usize).copied();
        let tex = hash.and_then(|h| self.textures.get(&h));
        let mut mat = json!({
            "name": tex.map(|t| t.name.clone()).unwrap_or_else(|| slot_name(slot)),
            "doubleSided": true,
            "pbrMetallicRoughness": { "metallicFactor": 0.0, "roughnessFactor": 1.0 },
        });
        if let Some(t) = tex {
            let file = format!("{stem}-{}.png", file_stem(&t.name));
            if !self.wanted.iter().any(|w| w.0 == file) {
                self.wanted.push((file.clone(), t));
                self.images.push(json!({ "uri": file, "name": t.name }));
                self.gtextures.push(json!({ "source": self.images.len() - 1 }));
            }
            let at = self.wanted.iter().position(|w| w.0 == file).unwrap_or(0);
            mat["pbrMetallicRoughness"]["baseColorTexture"] = json!({ "index": at });
            if !matches!(t.blend, ob_formats::formats::texture::Blend::None) {
                mat["alphaMode"] = json!("BLEND");
            }
        }
        self.materials.push(mat);
        self.materials.len() - 1
    }

    /// The document: `nodes` hang under the Z-up root (node 0).
    fn finish(mut self, stem: &str, extras: Value, nodes: Vec<Value>) -> (Value, Vec<u8>, Vec<(String, &'a Texture)>) {
        pad4(&mut self.buf);
        let children: Vec<usize> = (1..=nodes.len()).collect();
        let mut all = vec![json!({
            "name": "OB_Z_UP",
            // The game's +Z is up; glTF's is +Y (asset-format conventions §5.1).
            "rotation": [-std::f32::consts::FRAC_1_SQRT_2, 0.0, 0.0, std::f32::consts::FRAC_1_SQRT_2],
            "children": children,
        })];
        all.extend(nodes);
        let mut doc = json!({
            "asset": { "version": "2.0", "generator": "Overbright Studio", "extras": extras },
            "scene": 0,
            "scenes": [{ "nodes": [0] }],
            "nodes": all,
            "meshes": self.meshes,
            "buffers": [{ "uri": format!("{stem}.bin"), "byteLength": self.buf.len() }],
            "bufferViews": self.views,
            "accessors": self.accessors,
            "materials": self.materials,
        });
        if !self.images.is_empty() {
            doc["images"] = json!(self.images);
            doc["textures"] = json!(self.gtextures);
        }
        (doc, self.buf, self.wanted)
    }
}

/// What every export's `asset.extras` says about where the files come from.
fn extras(region: &str, more: Value) -> Value {
    let mut e = json!({
        "origin": "from-game-files",
        "share": "local-only",
        "note": "Exported from the player's own game files for reference; do not share.",
        "region": region,
    });
    if let (Some(e), Some(m)) = (e.as_object_mut(), more.as_object()) {
        e.extend(m.clone());
    }
    e
}

/// The glTF JSON and buffer of `solid`, with the texture files it names (`stem-<texture>.png`) wanted.
/// Returns (json, buffer, the textures to write as (file name, texture)).
pub fn build<'a>(
    solid: &Solid,
    textures: &'a HashMap<u32, Texture>,
    stem: &str,
    region: &str,
) -> Result<(Value, Vec<u8>, Vec<(String, &'a Texture)>)> {
    let mut scene = Scene::new(textures, false);
    scene.add_mesh(solid, stem)?;
    let more = json!({ "model": solid.name, "hash": format!("0x{:08X}", solid.hash) });
    Ok(scene.finish(stem, extras(region, more), vec![json!({ "name": solid.name, "mesh": 0 })]))
}

/// One model standing in an area: the solid and where it stands (local to the game's Z-up world).
pub struct Standing<'a> {
    pub solid: &'a Solid,
    pub transform: glam::Mat4,
}

/// The glTF of an area: each distinct model once as a mesh, each standing placement a node with its matrix.
/// Models that cannot be exported (no geometry) are left out and counted in the second answer.
pub fn build_area<'a>(
    standing: &[Standing],
    textures: &'a HashMap<u32, Texture>,
    stem: &str,
    region: &str,
    centre: [f32; 3],
    radius: f32,
) -> Result<(Value, Vec<u8>, Vec<(String, &'a Texture)>, usize)> {
    let mut scene = Scene::new(textures, true);
    let mut mesh_of: HashMap<u32, Option<usize>> = HashMap::new();
    let mut nodes = vec![];
    let mut skipped = 0;
    for s in standing {
        let m = *mesh_of.entry(s.solid.hash).or_insert_with(|| scene.add_mesh(s.solid, stem).ok());
        match m {
            Some(m) => {
                let mut node = json!({ "name": s.solid.name, "mesh": m });
                if s.transform != glam::Mat4::IDENTITY {
                    node["matrix"] = json!(s.transform.to_cols_array());
                }
                nodes.push(node);
            }
            None => skipped += 1,
        }
    }
    if nodes.is_empty() {
        bail!("nothing with geometry stands within {radius} m of the camera");
    }
    let more = json!({
        "area": { "centre": centre, "radius": radius, "placements": nodes.len(), "models": scene.meshes.len() },
    });
    let (doc, buf, wanted) = scene.finish(stem, extras(region, more), nodes);
    Ok((doc, buf, wanted, skipped))
}

/// Write a built document as `<dir>/<stem>.gltf` + `.bin` + its textures as PNG + [`NOTE_FILE`].
fn write_files(
    dir: &Path,
    stem: &str,
    doc: &Value,
    buf: &[u8],
    wanted: Vec<(String, &Texture)>,
) -> Result<Reference> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    let put = |name: &str, bytes: &[u8]| -> Result<PathBuf> {
        let p = dir.join(name);
        ob_core::platform::fs::write_atomic(&p, bytes, false)
            .map_err(|e| format!("cannot write {}: {e}", p.display()))?;
        Ok(p)
    };
    let bin = put(&format!("{stem}.bin"), buf)?;
    let mut pngs = vec![];
    for (file, t) in wanted {
        let img = image::RgbaImage::from_raw(t.width, t.height, t.decode_rgba())
            .ok_or_else(|| format!("texture {} has a bad size", t.name))?;
        let mut png = std::io::Cursor::new(vec![]);
        img.write_to(&mut png, image::ImageFormat::Png).map_err(|e| e.to_string())?;
        pngs.push(put(&file, png.get_ref())?);
    }
    let gltf = put(&format!("{stem}.gltf"), serde_json::to_string_pretty(doc).map_err(|e| e.to_string())?.as_bytes())?;
    let note = put(NOTE_FILE, NOTE.as_bytes())?;
    Ok(Reference { gltf, bin, textures: pngs, note })
}

/// Write `solid` as `<dir>/<stem>.gltf` + `.bin` + its textures as PNG + [`NOTE_FILE`]. `game_root`, when known,
/// is refused as a target. Only these names are written; an earlier export of the same model is replaced.
pub fn write_reference(
    dir: &Path,
    game_root: Option<&Path>,
    solid: &Solid,
    textures: &HashMap<u32, Texture>,
    region: &str,
) -> Result<Reference> {
    check_target(dir, game_root)?;
    let stem = file_stem(&solid.name);
    let (doc, buf, wanted) = build(solid, textures, &stem, region)?;
    write_files(dir, &stem, &doc, &buf, wanted)
}

/// The most placements one area export holds (a bigger area is refused, with the radius that fits).
pub const AREA_MAX_PLACEMENTS: usize = 12000;
/// The largest radius (metres) an area is taken at.
pub const AREA_MAX_RADIUS: f32 = 300.0;
/// The radius used when none is given.
pub const AREA_RADIUS: f32 = 60.0;

/// The placements of a world within `radius` metres of `centre` (ground distance, shown ones only), nearest
/// first, as the models standing there.
pub fn standing_near<'a>(
    placements: &'a [Placement],
    solids: &'a [Solid],
    centre: glam::Vec3,
    radius: f32,
) -> Vec<Standing<'a>> {
    let mut near: Vec<(f32, Standing)> = placements
        .iter()
        .filter(|p| p.flags & HIDDEN_FLAG == 0)
        .filter_map(|p| {
            let at = p.transform.w_axis.truncate();
            let d = (at.truncate() - centre.truncate()).length();
            (d <= radius).then(|| (d, solids.get(p.solid).map(|solid| Standing { solid, transform: p.transform })))
        })
        .filter_map(|(d, s)| s.map(|s| (d, s)))
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    near.into_iter().map(|n| n.1).collect()
}

/// What an area export wrote.
#[derive(Clone, Debug, PartialEq)]
pub struct AreaReference {
    pub files: Reference,
    pub placements: usize,
    pub models: usize,
    /// Placements left out for having no geometry.
    pub skipped: usize,
}

/// Write the area of `radius` metres around `centre` as `<dir>/area-<region>.gltf` (+ `.bin`, textures, note),
/// marked as from the game files like a model's export, and refused into the game folder the same way.
#[allow(clippy::too_many_arguments)]
pub fn write_area(
    dir: &Path,
    game_root: Option<&Path>,
    placements: &[Placement],
    solids: &[Solid],
    textures: &HashMap<u32, Texture>,
    region: &str,
    centre: glam::Vec3,
    radius: f32,
) -> Result<AreaReference> {
    check_target(dir, game_root)?;
    if !(1.0..=AREA_MAX_RADIUS).contains(&radius) {
        bail!("the radius is 1 to {AREA_MAX_RADIUS} metres");
    }
    let standing = standing_near(placements, solids, centre, radius);
    if standing.len() > AREA_MAX_PLACEMENTS {
        bail!(
            "{} placements stand within {radius} m: too many for one file (at most {AREA_MAX_PLACEMENTS}); use a smaller radius",
            standing.len()
        );
    }
    let stem = format!("area-{}", file_stem(region));
    let (doc, buf, wanted, skipped) = build_area(&standing, textures, &stem, region, centre.to_array(), radius)?;
    let models = doc["meshes"].as_array().map_or(0, Vec::len);
    let placed = doc["nodes"].as_array().map_or(0, Vec::len).saturating_sub(1);
    let files = write_files(dir, &stem, &doc, &buf, wanted)?;
    Ok(AreaReference { files, placements: placed, models, skipped })
}

#[cfg(test)]
mod tests;
