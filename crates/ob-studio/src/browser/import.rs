//! Bringing an edited model back as a mod model: a glTF file that was exported from the Browser (or any glTF 2.0
//! file for the same model, edited in an outside 3D program) is read, mapped onto the original model's parts and
//! kept in the mod.
//!
//! Two steps, so that the one that reads a file from outside can run in a process of its own
//! ([`super::worker`]): [`read_gltf`] turns the file into plain vertex sets and triangle lists in the game's own
//! axes ([`Foreign`], nothing of the game is needed), and [`map_onto`] maps that onto the original [`Solid`].
//!
//! **Mapping.** The original's parts are its draw ranges grouped by the texture they draw; a part is named as an
//! export names its glTF material ([`gltf::part_name`]). Every material of the file must be one of the original's
//! parts and every part of the original must have triangles in the file, so the model keeps the structure the game
//! and the mods address it by; anything else is refused with the reason ([`Refusal`]). What the model has beyond
//! its shape (its texture list, light materials, flags and markers) is the original's.
//!
//! **Where it is kept** ([`write_model`]). A model made from the player's game files is local-only
//! (docs/modding/asset-format/manifest.md §3, `from-game-files`): it goes to the mod's `.editor/models/`, which a
//! shared copy leaves out, with the pictures its materials name as PNG. Only a model the author says is entirely
//! their own work (`own`) goes to the mod's `models/` folder, a shared file, without the game's pictures.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ob_formats::formats::solid::{Material, Solid, Vertex};
use ob_formats::formats::texture::Texture;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::gltf;

/// A file's or a mapping's refusal: the reasons, each a sentence for the verdict line.
#[derive(Clone, Debug, PartialEq)]
pub struct Refusal(pub Vec<String>);

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.join("; "))
    }
}

impl From<String> for Refusal {
    fn from(s: String) -> Self {
        Refusal(vec![s])
    }
}

type Res<T> = std::result::Result<T, Refusal>;

macro_rules! no {
    ($($a:tt)*) => { return Err(Refusal(vec![format!($($a)*)])) };
}

/// A file bigger than this is not read (a glTF is JSON; a model of this game is well under).
pub const MAX_GLTF_BYTES: u64 = 16 << 20;
/// The same for the `.bin` beside it.
pub const MAX_BIN_BYTES: u64 = 128 << 20;
/// A solid holds at most this many vertices (16-bit indices; asset format `MSH.5`).
pub const MAX_VERTICES: usize = 65_535;
/// What a read stops at, whatever a file claims: accessors and triangle lists never grow past this.
const MAX_ELEMENTS: usize = 4_000_000;

/// Vertices of one mesh with one placing in the scene, in the game's axes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VertexSet {
    pub positions: Vec<[f32; 3]>,
    /// Empty when the file has none.
    pub normals: Vec<[f32; 3]>,
    /// Empty when the file has none.
    pub uvs: Vec<[f32; 2]>,
}

/// One triangle list drawn with one material.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Primitive {
    pub material: String,
    pub set: usize,
    pub indices: Vec<u32>,
}

/// What a glTF file holds, as plain data.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Foreign {
    pub sets: Vec<VertexSet>,
    pub prims: Vec<Primitive>,
    /// `asset.extras.model`, when the file says which model it was exported from.
    pub model: Option<String>,
    /// `asset.extras.hash` (`0x…`).
    pub hash: Option<u32>,
    /// `asset.extras.origin`, when present (`from-game-files` on an export).
    pub origin: Option<String>,
}

fn num(v: &Value, what: &str) -> Res<f64> {
    v.as_f64().ok_or_else(|| Refusal::from(format!("{what} is not a number")))
}

fn matrix_of(node: &Value) -> Res<glam::Mat4> {
    if let Some(m) = node.get("matrix") {
        let a = m
            .as_array()
            .filter(|a| a.len() == 16)
            .ok_or_else(|| Refusal::from("a node's matrix is not 16 numbers".to_string()))?;
        let mut c = [0f32; 16];
        for (i, v) in a.iter().enumerate() {
            c[i] = num(v, "a matrix value")? as f32;
        }
        return Ok(glam::Mat4::from_cols_array(&c));
    }
    let arr = |k: &str, n: usize, d: &[f32]| -> Res<Vec<f32>> {
        match node.get(k) {
            None => Ok(d.to_vec()),
            Some(v) => {
                let a = v
                    .as_array()
                    .filter(|a| a.len() == n)
                    .ok_or_else(|| Refusal::from(format!("a node's {k} is not {n} numbers")))?;
                a.iter().map(|x| num(x, k).map(|f| f as f32)).collect()
            }
        }
    };
    let t = arr("translation", 3, &[0.0; 3])?;
    let r = arr("rotation", 4, &[0.0, 0.0, 0.0, 1.0])?;
    let s = arr("scale", 3, &[1.0; 3])?;
    let q = glam::Quat::from_xyzw(r[0], r[1], r[2], r[3]);
    if q.length_squared() < 1e-12 {
        no!("a node's rotation is not a rotation");
    }
    Ok(glam::Mat4::from_scale_rotation_translation(
        glam::Vec3::new(s[0], s[1], s[2]),
        q.normalize(),
        glam::Vec3::new(t[0], t[1], t[2]),
    ))
}

struct Bin<'a> {
    doc: &'a Value,
    bytes: &'a [u8],
}

impl Bin<'_> {
    /// An accessor's elements as `n` floats each (`VEC2` = 2, `VEC3` = 3, `SCALAR` = 1). Floats only for
    /// attributes; `index` reads 1/2/4-byte unsigned integers.
    fn read(&self, idx: usize, ty: &str, index: bool) -> Res<Vec<f32>> {
        let acc =
            self.doc["accessors"].get(idx).ok_or_else(|| Refusal::from(format!("accessor {idx} does not exist")))?;
        if acc.get("sparse").is_some() {
            no!("accessor {idx} is sparse: sparse accessors are not read");
        }
        if acc["type"] != ty {
            no!("accessor {idx} is {} where {ty} is needed", acc["type"]);
        }
        let comp = acc["componentType"].as_u64().unwrap_or(0);
        let size = match (index, comp) {
            (false, 5126) => 4,
            (true, 5121) => 1,
            (true, 5123) => 2,
            (true, 5125) => 4,
            (false, _) => no!("accessor {idx} does not hold 32-bit floats (componentType {comp})"),
            (true, _) => no!("indices are not unsigned integers (componentType {comp})"),
        };
        let n = acc["count"].as_u64().unwrap_or(0) as usize;
        if n > MAX_ELEMENTS {
            no!("accessor {idx} has {n} elements: too many");
        }
        let per = match ty {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            _ => no!("accessor {idx} has type {ty}"),
        };
        let view = acc["bufferView"]
            .as_u64()
            .and_then(|v| self.doc["bufferViews"].get(v as usize))
            .ok_or_else(|| Refusal::from(format!("accessor {idx} has no buffer view")))?;
        if view["buffer"].as_u64().unwrap_or(0) != 0 {
            no!("accessor {idx} reads a second buffer");
        }
        let base = view["byteOffset"].as_u64().unwrap_or(0) as usize + acc["byteOffset"].as_u64().unwrap_or(0) as usize;
        let stride = view["byteStride"].as_u64().map(|s| s as usize).unwrap_or(size * per);
        if stride < size * per {
            no!("accessor {idx} has a stride smaller than its element");
        }
        let view_end =
            view["byteOffset"].as_u64().unwrap_or(0) as usize + view["byteLength"].as_u64().unwrap_or(0) as usize;
        let mut out = Vec::with_capacity(n * per);
        for i in 0..n {
            for k in 0..per {
                let at = base + i * stride + k * size;
                let end = at + size;
                if end > self.bytes.len() || end > view_end.max(1) {
                    no!("accessor {idx} reads past the end of its data");
                }
                let b = &self.bytes[at..end];
                out.push(match size {
                    4 if !index => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                    4 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32,
                    2 => u16::from_le_bytes([b[0], b[1]]) as f32,
                    _ => b[0] as f32,
                });
            }
        }
        Ok(out)
    }

    /// Indices as integers (a `u32` does not survive `f32` past 2^24, so they are read again exactly).
    fn indices(&self, idx: usize) -> Res<Vec<u32>> {
        let acc = &self.doc["accessors"][idx];
        let comp = acc["componentType"].as_u64().unwrap_or(0);
        // A float read keeps values to 2^24: refuse a count beyond what 16-bit indices could ever name.
        let f = self.read(idx, "SCALAR", true)?;
        if comp == 5125 && f.iter().any(|&v| v >= 16_777_216.0) {
            no!("an index is past what a model of this game can hold ({MAX_VERTICES} vertices)");
        }
        Ok(f.into_iter().map(|v| v as u32).collect())
    }
}

/// Read the glTF file `path` (with the `.bin` its buffer names, beside it) into [`Foreign`]: node transforms applied,
/// the glTF's +Y up turned back to the game's +Z up. Refuses a file it cannot read as one model of triangles.
pub fn read_gltf(path: &Path) -> Res<Foreign> {
    let meta = std::fs::metadata(path).map_err(|e| Refusal::from(format!("cannot read {}: {e}", path.display())))?;
    if meta.len() > MAX_GLTF_BYTES {
        no!("{} is {} MB: too big for a model", path.display(), meta.len() >> 20);
    }
    let text = std::fs::read(path).map_err(|e| Refusal::from(format!("cannot read {}: {e}", path.display())))?;
    if text.first() == Some(&b'g') && text.starts_with(b"glTF") {
        no!(
            "a .glb file: export a .gltf with a separate .bin (the Browser's export and Blender's \"glTF Separate\" make one)"
        );
    }
    let doc: Value = serde_json::from_slice(&text)
        .map_err(|e| Refusal::from(format!("{} is not a glTF file: {e}", path.display())))?;
    if doc["asset"]["version"] != "2.0" {
        no!("only glTF 2.0 files are read (this one says {})", doc["asset"]["version"]);
    }
    for k in ["skins", "animations"] {
        if doc.get(k).and_then(Value::as_array).is_some_and(|a| !a.is_empty()) {
            no!("the file has {k}: models with skeletons or animation are not brought back");
        }
    }
    let buffers = doc["buffers"].as_array().cloned().unwrap_or_default();
    if buffers.len() != 1 {
        no!("the file has {} buffers: one .bin is read", buffers.len());
    }
    let uri = buffers[0]["uri"].as_str().unwrap_or("");
    if uri.is_empty() || uri.starts_with("data:") {
        no!("the file holds its data inside (a data URI): save it with a separate .bin");
    }
    if uri.contains("..") || uri.contains(':') || uri.starts_with('/') || uri.starts_with('\\') || uri.contains('\\') {
        no!("the file's .bin `{uri}` is not a file beside it");
    }
    let bin_path = path.parent().unwrap_or(Path::new(".")).join(uri);
    let bmeta =
        std::fs::metadata(&bin_path).map_err(|_| Refusal::from(format!("the file's .bin `{uri}` is not beside it")))?;
    if bmeta.len() > MAX_BIN_BYTES {
        no!("{uri} is {} MB: too big for a model", bmeta.len() >> 20);
    }
    let bytes = std::fs::read(&bin_path).map_err(|e| Refusal::from(format!("cannot read {uri}: {e}")))?;
    let bin = Bin { doc: &doc, bytes: &bytes };

    let mut out = Foreign::default();
    let extras = &doc["asset"]["extras"];
    out.model = extras["model"].as_str().map(str::to_string);
    out.hash = extras["hash"]
        .as_str()
        .and_then(|h| u32::from_str_radix(h.trim_start_matches("0x").trim_start_matches("0X"), 16).ok());
    out.origin = extras["origin"].as_str().map(str::to_string);

    let nodes = doc["nodes"].as_array().cloned().unwrap_or_default();
    let scene = doc["scene"].as_u64().unwrap_or(0) as usize;
    let roots: Vec<usize> = match doc["scenes"].get(scene) {
        Some(s) => s["nodes"]
            .as_array()
            .map(|a| a.iter().filter_map(|n| n.as_u64().map(|n| n as usize)).collect())
            .unwrap_or_default(),
        None => (0..nodes.len()).collect(),
    };
    // The inverse of the export's root turn (+Z up to +Y up): the game's axes again.
    let to_game = glam::Mat4::from_quat(glam::Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2).inverse());

    let mut stack: Vec<(usize, glam::Mat4, usize)> =
        roots.iter().rev().map(|&n| (n, glam::Mat4::IDENTITY, 0)).collect();
    let mut seen = 0usize;
    let mut sets: HashMap<(usize, usize, Option<usize>, Option<usize>), usize> = HashMap::new();
    while let Some((n, parent, depth)) = stack.pop() {
        seen += 1;
        if depth > 64 || seen > 100_000 {
            no!("the file's nodes nest too deep or are too many");
        }
        let node = nodes.get(n).ok_or_else(|| Refusal::from(format!("node {n} does not exist")))?;
        let world = parent * matrix_of(node)?;
        if let Some(kids) = node["children"].as_array() {
            for k in kids.iter().rev().filter_map(|k| k.as_u64()) {
                stack.push((k as usize, world, depth + 1));
            }
        }
        let Some(mi) = node["mesh"].as_u64() else { continue };
        let mesh = doc["meshes"].get(mi as usize).ok_or_else(|| Refusal::from(format!("mesh {mi} does not exist")))?;
        let m = to_game * world;
        let normal_m = glam::Mat3::from_mat4(m).inverse().transpose();
        for (pi, p) in mesh["primitives"].as_array().cloned().unwrap_or_default().iter().enumerate() {
            let mode = p["mode"].as_u64().unwrap_or(4);
            if mode != 4 {
                no!("mesh {mi} primitive {pi} is not a list of triangles (mode {mode})");
            }
            if p.get("targets").is_some() {
                no!("mesh {mi} has morph targets: they are not brought back");
            }
            let attr = |k: &str| p["attributes"][k].as_u64().map(|v| v as usize);
            let Some(pos) = attr("POSITION") else { no!("mesh {mi} primitive {pi} has no positions") };
            let (nrm, uv) = (attr("NORMAL"), attr("TEXCOORD_0"));
            let key = (n, pos, nrm, uv);
            let set = match sets.get(&key) {
                Some(&s) => s,
                None => {
                    let flat = bin.read(pos, "VEC3", false)?;
                    let positions: Vec<[f32; 3]> = flat
                        .chunks_exact(3)
                        .map(|c| m.transform_point3(glam::Vec3::new(c[0], c[1], c[2])).to_array())
                        .collect();
                    let normals = match nrm {
                        Some(a) => bin
                            .read(a, "VEC3", false)?
                            .chunks_exact(3)
                            .map(|c| (normal_m * glam::Vec3::new(c[0], c[1], c[2])).normalize_or_zero().to_array())
                            .collect(),
                        None => vec![],
                    };
                    let uvs = match uv {
                        Some(a) => bin.read(a, "VEC2", false)?.chunks_exact(2).map(|c| [c[0], c[1]]).collect(),
                        None => vec![],
                    };
                    if (!normals.is_empty() && normals.len() != positions.len())
                        || (!uvs.is_empty() && uvs.len() != positions.len())
                    {
                        no!("mesh {mi} primitive {pi}: positions, normals and texture coordinates differ in count");
                    }
                    out.sets.push(VertexSet { positions, normals, uvs });
                    sets.insert(key, out.sets.len() - 1);
                    out.sets.len() - 1
                }
            };
            let indices = match p["indices"].as_u64() {
                Some(a) => bin.indices(a as usize)?,
                None => (0..out.sets[set].positions.len() as u32).collect(),
            };
            let material = match p["material"].as_u64() {
                Some(i) => doc["materials"]
                    .get(i as usize)
                    .and_then(|m| m["name"].as_str())
                    .map_or_else(|| format!("material{i}"), str::to_string),
                None => "(no material)".to_string(),
            };
            out.prims.push(Primitive { material, set, indices });
        }
    }
    if out.prims.is_empty() {
        no!("the file has no model in it (no mesh in its scene)");
    }
    Ok(out)
}

/// A model mapped onto the original's parts.
#[derive(Clone, Debug)]
pub struct Imported {
    pub solid: Solid,
    /// Each part (the original's part name) with its triangle count, in the original's order.
    pub parts: Vec<(String, usize)>,
}

fn same_shape(a: &Solid, b: &Solid) -> bool {
    a.vertices.len() == b.vertices.len()
        && a.indices == b.indices
        && a.vertices.iter().zip(&b.vertices).all(|(x, y)| {
            x.position.iter().zip(&y.position).all(|(p, q)| (p - q).abs() < 1e-4)
                && x.uv.iter().zip(&y.uv).all(|(p, q)| (p - q).abs() < 1e-4)
        })
}

/// Map `f` onto `orig`'s parts. Every reason it cannot be mapped is in the refusal.
pub fn map_onto(f: &Foreign, orig: &Solid, textures: &HashMap<u32, Texture>) -> Res<Imported> {
    let mut why: Vec<String> = vec![];
    if let Some(m) = &f.model
        && !m.eq_ignore_ascii_case(&orig.name)
    {
        no!("this file was exported from {m}, not from {}: import it onto {m}", orig.name);
    }
    if let Some(h) = f.hash
        && h != orig.hash
    {
        no!("this file's model hash 0x{h:08X} is not {}'s (0x{:08X})", orig.name, orig.hash);
    }
    // The original's parts, in order of first appearance.
    let mut parts: Vec<(String, Material)> = vec![];
    for m in &orig.materials {
        if m.index_count == 0 || m.first_index as usize + m.index_count as usize > orig.indices.len() {
            continue;
        }
        let name = gltf::part_name(orig, textures, m.texture_slot);
        if !parts.iter().any(|(n, _)| *n == name) {
            parts.push((name, m.clone()));
        }
    }
    if parts.is_empty() {
        no!("{} has no drawable part to map onto", orig.name);
    }
    let names: Vec<&str> = parts.iter().map(|(n, _)| n.as_str()).collect();
    let lit = orig.normals.len() == orig.vertices.len() && !orig.normals.is_empty();
    for p in &f.prims {
        if !names.contains(&p.material.as_str()) {
            why.push(format!(
                "material `{}` is not a part of {} (its parts: {})",
                p.material,
                orig.name,
                names.join(", ")
            ));
        }
    }
    why.dedup();
    for (n, _) in &parts {
        if !f.prims.iter().any(|p| p.material == *n && !p.indices.is_empty()) {
            why.push(format!("the part `{n}` has no triangles in the file: every part of the model has to stay"));
        }
    }
    if !why.is_empty() {
        return Err(Refusal(why));
    }

    // Build the model: one vertex run per used set, indices rebased; draw ranges in the original's part order.
    let mut verts: Vec<Vertex> = vec![];
    let mut normals: Vec<[f32; 3]> = vec![];
    let mut base_of: HashMap<usize, usize> = HashMap::new();
    let mut indices: Vec<u16> = vec![];
    let mut materials: Vec<Material> = vec![];
    let mut report: Vec<(String, usize)> = vec![];
    for (name, first) in &parts {
        let start = indices.len();
        for p in f.prims.iter().filter(|p| p.material == *name) {
            let set = &f.sets[p.set];
            let n = set.positions.len();
            if set.uvs.is_empty() {
                no!("part `{name}` has no texture coordinates (UVs): the game draws its pictures by them");
            }
            if lit && set.normals.is_empty() {
                no!("{} is lit by normals: the file's part `{name}` has none", orig.name);
            }
            if p.indices.len() % 3 != 0 {
                no!("part `{name}` does not hold whole triangles ({} indices)", p.indices.len());
            }
            if let Some(&bad) = p.indices.iter().find(|&&i| i as usize >= n) {
                no!("part `{name}` has an index ({bad}) past its last vertex");
            }
            if set.positions.iter().flatten().chain(set.uvs.iter().flatten()).any(|v| !v.is_finite()) {
                no!("part `{name}` has a position or texture coordinate that is not a number");
            }
            let base = *base_of.entry(p.set).or_insert_with(|| {
                let b = verts.len();
                for (i, pos) in set.positions.iter().enumerate() {
                    verts.push(Vertex { position: *pos, color: [255; 4], uv: set.uvs[i] });
                    if lit {
                        normals.push(set.normals[i]);
                    }
                }
                b
            });
            if verts.len() > MAX_VERTICES {
                no!(
                    "the model has {} vertices: a model of this game holds at most {MAX_VERTICES} (split it or reduce it)",
                    verts.len()
                );
            }
            indices.extend(p.indices.iter().map(|&i| (base + i as usize) as u16));
        }
        let count = indices.len() - start;
        materials.push(Material {
            texture_slot: first.texture_slot,
            light_material: first.light_material,
            first_index: start as u32,
            index_count: count as u32,
            flags: first.flags,
        });
        report.push((name.clone(), count / 3));
    }
    // Baked vertex colours stay when the shape's vertices are the original's; else they are plain white.
    if verts.len() == orig.vertices.len() {
        for (v, o) in verts.iter_mut().zip(&orig.vertices) {
            v.color = o.color;
        }
    }
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for v in &verts {
        for k in 0..3 {
            lo[k] = lo[k].min(v.position[k]);
            hi[k] = hi[k].max(v.position[k]);
        }
    }
    let solid = Solid {
        name: orig.name.clone(),
        hash: orig.hash,
        bbox_min: lo,
        bbox_max: hi,
        textures: orig.textures.clone(),
        vertices: verts,
        normals,
        light_materials: orig.light_materials.clone(),
        indices,
        materials,
        mesh_flags: orig.mesh_flags,
        markers: orig.markers.clone(),
        light_markers: orig.light_markers.clone(),
    };
    Ok(Imported { solid, parts: report })
}

/// Where a model kept in a mod goes: `.editor/models/` (local-only) or, for the author's own, `models/` (shared).
pub fn models_dir(mod_folder: &Path, own: bool) -> PathBuf {
    if own { mod_folder.join("models") } else { mod_folder.join(".editor").join("models") }
}

/// What [`write_model`] wrote.
#[derive(Clone, Debug, PartialEq)]
pub struct Kept {
    pub dir: PathBuf,
    pub gltf: PathBuf,
    pub record: PathBuf,
    /// `true` when the model is the author's own work and shared with the mod.
    pub shared: bool,
}

/// The author's say-so that a model is entirely their own work is refused when the file still says it was made
/// from the game's files, or when it has the original's shape unchanged.
pub fn check_own(f: &Foreign, imported: &Imported, orig: &Solid) -> Res<()> {
    if f.origin.as_deref() == Some("from-game-files") {
        no!(
            "the file is marked as from your game files: it stays on this computer. Remove the mark only when the model is entirely your own work"
        );
    }
    if same_shape(&imported.solid, orig) {
        no!("the shape is the game's own, unchanged: it is not your own work");
    }
    Ok(())
}

/// Keep `imp` in the mod in `mod_folder`: `<stem>.gltf` + `.bin` (+ the pictures as PNG unless `own`) and
/// `<stem>.model.toml`, the record of what it was made from and whether it may be shared.
pub fn write_model(
    mod_folder: &Path,
    imp: &Imported,
    textures: &HashMap<u32, Texture>,
    region: &str,
    own: bool,
) -> std::result::Result<Kept, String> {
    let dir = models_dir(mod_folder, own);
    let s = &imp.solid;
    let stem = gltf::file_stem(&s.name);
    let (mut doc, buf, wanted) = gltf::build(s, textures, &stem, region)?;
    let put = |name: &str, bytes: &[u8]| -> std::result::Result<PathBuf, String> {
        let p = dir.join(name);
        ob_core::platform::fs::write_atomic(&p, bytes, false)
            .map_err(|e| format!("cannot write {}: {e}", p.display()))?;
        Ok(p)
    };
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    let (origin, share) = if own { ("own", "shareable") } else { ("from-game-files", "local-only") };
    doc["asset"]["extras"] = serde_json::json!({
        "origin": origin, "share": share, "region": region, "model": s.name,
        "hash": format!("0x{:08X}", s.hash),
        "note": if own { "A model made by its author onto the parts of a game model." } else { "Made from the player's own game files; do not share." },
    });
    if own {
        // Part names stay; the game's pictures are not part of an own model.
        if let Some(o) = doc.as_object_mut() {
            o.remove("images");
            o.remove("textures");
        }
        for m in doc["materials"].as_array_mut().into_iter().flatten() {
            if let Some(p) = m["pbrMetallicRoughness"].as_object_mut() {
                p.remove("baseColorTexture");
            }
        }
    } else {
        for (file, t) in &wanted {
            let img = image::RgbaImage::from_raw(t.width, t.height, t.decode_rgba())
                .ok_or_else(|| format!("texture {} has a bad size", t.name))?;
            let mut png = std::io::Cursor::new(vec![]);
            img.write_to(&mut png, image::ImageFormat::Png).map_err(|e| e.to_string())?;
            put(file, png.get_ref())?;
        }
        put(gltf::NOTE_FILE, b"Made from your own copy of the game, for use on this computer.\norigin = \"from-game-files\"\nshare = \"local-only\"\nDo not share or publish these files: they hold the game's models and textures.\n")?;
    }
    put(&format!("{stem}.bin"), &buf)?;
    let gltf = put(&format!("{stem}.gltf"), serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?.as_bytes())?;
    let parts: Vec<String> = imp.parts.iter().map(|(n, t)| format!("{{ name = {n:?}, triangles = {t} }}")).collect();
    let record_text = format!(
        "format = 1\nmodel = {:?}\nhash = \"0x{:08X}\"\nregion = {region:?}\norigin = \"{origin}\"\nshare = \"{share}\"\nvertices = {}\ntriangles = {}\nparts = [{}]\n",
        s.name,
        s.hash,
        s.vertices.len(),
        s.indices.len() / 3,
        parts.join(", "),
    );
    let record = put(&format!("{stem}.model.toml"), record_text.as_bytes())?;
    Ok(Kept { dir, gltf, record, shared: own })
}

#[cfg(test)]
mod tests;
