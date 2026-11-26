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

const NOTE: &str = "These files are exported from your own copy of the game, for looking at and working from in your own \
3D program.\norigin = \"from-game-files\"\nshare = \"local-only\"\nDo not share or publish them: they hold the game's \
models and textures.\n";

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

/// The glTF JSON and buffer of `solid`, with the texture files it names (`stem-<texture>.png`) wanted.
/// Returns (json, buffer, the textures to write as (file name, texture)).
pub fn build<'a>(
    solid: &Solid,
    textures: &'a HashMap<u32, Texture>,
    stem: &str,
    region: &str,
) -> Result<(Value, Vec<u8>, Vec<(String, &'a Texture)>)> {
    let n = solid.vertices.len();
    if n == 0 || solid.indices.is_empty() {
        bail!("{} has no geometry to export", solid.name);
    }
    if solid.indices.iter().any(|&i| i as usize >= n) {
        bail!("{} has an index past its last vertex", solid.name);
    }
    let lit = solid.normals.len() == n;
    let mut buf: Vec<u8> = vec![];
    let mut views = vec![];
    let mut accessors = vec![];
    let mut view = |buf: &mut Vec<u8>, data: &[u8], target: u32| -> usize {
        pad4(buf);
        views.push(json!({ "buffer": 0, "byteOffset": buf.len(), "byteLength": data.len(), "target": target }));
        buf.extend_from_slice(data);
        views.len() - 1
    };
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
    let bv = view(&mut buf, &pos, 34962);
    accessors.push(json!({ "bufferView": bv, "componentType": 5126, "count": n, "type": "VEC3", "min": lo, "max": hi }));
    let mut attrs = json!({ "POSITION": 0 });
    if lit {
        let mut nb = vec![];
        floats(solid.normals.iter().flatten().copied(), &mut nb);
        let bv = view(&mut buf, &nb, 34962);
        accessors.push(json!({ "bufferView": bv, "componentType": 5126, "count": n, "type": "VEC3" }));
        attrs["NORMAL"] = json!(accessors.len() - 1);
    }
    let mut uv = vec![];
    floats(solid.vertices.iter().flat_map(|v| v.uv), &mut uv);
    let bv = view(&mut buf, &uv, 34962);
    accessors.push(json!({ "bufferView": bv, "componentType": 5126, "count": n, "type": "VEC2" }));
    attrs["TEXCOORD_0"] = json!(accessors.len() - 1);
    let idx: Vec<u8> = solid.indices.iter().flat_map(|i| i.to_le_bytes()).collect();
    let ibv = view(&mut buf, &idx, 34963);

    // One glTF material per texture slot a draw range uses; one primitive per range.
    let mut gmaterials: Vec<Value> = vec![];
    let mut gtextures: Vec<Value> = vec![];
    let mut images: Vec<Value> = vec![];
    let mut wanted: Vec<(String, &Texture)> = vec![];
    let mut mat_of_slot: HashMap<u8, usize> = HashMap::new();
    let mut prims = vec![];
    for m in &solid.materials {
        let (first, count) = (m.first_index as usize, m.index_count as usize);
        if count == 0 || first + count > solid.indices.len() {
            continue;
        }
        let mi = *mat_of_slot.entry(m.texture_slot).or_insert_with(|| {
            let hash = solid.textures.get(m.texture_slot as usize).copied();
            let tex = hash.and_then(|h| textures.get(&h));
            let mut mat = json!({
                "name": tex.map(|t| t.name.clone()).unwrap_or_else(|| slot_name(m.texture_slot)),
                "doubleSided": true,
                "pbrMetallicRoughness": { "metallicFactor": 0.0, "roughnessFactor": 1.0 },
            });
            if let Some(t) = tex {
                let file = format!("{stem}-{}.png", file_stem(&t.name));
                if !wanted.iter().any(|w| w.0 == file) {
                    wanted.push((file.clone(), t));
                    images.push(json!({ "uri": file, "name": t.name }));
                    gtextures.push(json!({ "source": images.len() - 1 }));
                }
                let at = wanted.iter().position(|w| w.0 == file).unwrap_or(0);
                mat["pbrMetallicRoughness"]["baseColorTexture"] = json!({ "index": at });
                if !matches!(t.blend, ob_formats::formats::texture::Blend::None) {
                    mat["alphaMode"] = json!("BLEND");
                }
            }
            gmaterials.push(mat);
            gmaterials.len() - 1
        });
        accessors.push(json!({
            "bufferView": ibv, "byteOffset": first * 2, "componentType": 5123, "count": count, "type": "SCALAR",
        }));
        prims.push(json!({ "attributes": attrs, "indices": accessors.len() - 1, "material": mi, "mode": 4 }));
    }
    if prims.is_empty() {
        bail!("{} has no drawable material range", solid.name);
    }
    pad4(&mut buf);
    let mut doc = json!({
        "asset": {
            "version": "2.0",
            "generator": "Overbright Studio",
            "extras": {
                "origin": "from-game-files",
                "share": "local-only",
                "note": "Exported from the player's own game files for reference; do not share.",
                "region": region,
                "model": solid.name,
                "hash": format!("0x{:08X}", solid.hash),
            },
        },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        // The game's +Z is up; glTF's is +Y (asset-format conventions §5.1).
        "nodes": [
            { "name": "OB_Z_UP", "rotation": [-std::f32::consts::FRAC_1_SQRT_2, 0.0, 0.0, std::f32::consts::FRAC_1_SQRT_2], "children": [1] },
            { "name": solid.name, "mesh": 0 },
        ],
        "meshes": [{ "name": solid.name, "primitives": prims }],
        "buffers": [{ "uri": format!("{stem}.bin"), "byteLength": buf.len() }],
        "bufferViews": views,
        "accessors": accessors,
        "materials": gmaterials,
    });
    if !images.is_empty() {
        doc["images"] = json!(images);
        doc["textures"] = json!(gtextures);
    }
    Ok((doc, buf, wanted))
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
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    let put = |name: &str, bytes: &[u8]| -> Result<PathBuf> {
        let p = dir.join(name);
        ob_core::platform::fs::write_atomic(&p, bytes, false)
            .map_err(|e| format!("cannot write {}: {e}", p.display()))?;
        Ok(p)
    };
    let bin = put(&format!("{stem}.bin"), &buf)?;
    let mut pngs = vec![];
    for (file, t) in wanted {
        let img = image::RgbaImage::from_raw(t.width, t.height, t.decode_rgba())
            .ok_or_else(|| format!("texture {} has a bad size", t.name))?;
        let mut png = std::io::Cursor::new(vec![]);
        img.write_to(&mut png, image::ImageFormat::Png).map_err(|e| e.to_string())?;
        pngs.push(put(&file, png.get_ref())?);
    }
    let gltf = put(&format!("{stem}.gltf"), serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?.as_bytes())?;
    let note = put(NOTE_FILE, NOTE.as_bytes())?;
    Ok(Reference { gltf, bin, textures: pngs, note })
}

#[cfg(test)]
mod tests;
