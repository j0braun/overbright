//! `car_inspect CAR` — list a car's stock-candidate solids with recovered
//! full names, and the textures and light materials their materials use.
use std::collections::HashMap;

use nfsu2::formats::bundle::{self, id};
use nfsu2::formats::hash::bin_hash;
use nfsu2::formats::solid::parse_solid_list;
use nfsu2::formats::texture::parse_texture_pack;

fn main() -> anyhow::Result<()> {
    // A developer tool: the developer cache, never the player's (docs/asset-cache.md).
    nfsu2::platform::dirs::developer_program();
    let car = std::env::args().nth(1).unwrap_or_else(|| "240SX".into());
    let root = nfsu2::game_root();
    let mut names: HashMap<u32, String> = HashMap::new();
    for file in [nfsu2::game_fs::path(&root, &format!("CARS/{car}/TEXTURES.BIN")), nfsu2::game_fs::path(&root, "CARS/TEXTURES.BIN")] {
        let data = std::fs::read(&file)?;
        bundle::walk(bundle::chunks(&data, 0), &mut |c| {
            if c.id == id::TEXTURE_PACK {
                for t in parse_texture_pack(c).unwrap_or_default() {
                    names.insert(t.hash, t.name);
                }
            }
        });
    }
    // Light materials from GLOBALB (name at +0x14 of each 0xA0 record).
    let globalb = nfsu2::game_fs::read(&root, "GLOBAL/GLOBALB.BUN")?;
    for c in bundle::chunks(&globalb, 0).filter(|c| c.id == 0x0013_5200) {
        let name = nfsu2::formats::bundle::read_cstr(c.body, 0x14, 0x1C);
        names.insert(bin_hash(&name), format!("LM:{name}"));
    }
    if std::env::args().any(|a| a == "--textures") {
        let mut v: Vec<_> = names.values().filter(|n| !n.starts_with("LM:")).cloned().collect();
        v.sort();
        println!("{}", v.join(" "));
        return Ok(());
    }
    let data = nfsu2::game_fs::read(&root, &format!("CARS/{car}/GEOMETRY.BIN"))?;
    let mut lists = Vec::new();
    bundle::walk(bundle::chunks(&data, 0), &mut |c| {
        if c.id == id::SOLID_LIST {
            lists.push(c)
        }
    });
    let n = |h: &u32| names.get(h).cloned().unwrap_or(format!("{h:08X}?"));
    for list in lists {
        for s in parse_solid_list(list)? {
            if !(s.name.contains("KIT00") || s.name.contains("BASE")) || !s.name.ends_with("_A") {
                continue;
            }
            println!("{} flags {:#x}", s.name, s.mesh_flags);
            for m in &s.materials {
                let tex = s.textures.get(m.texture_slot as usize).map(n).unwrap_or("-".into());
                let lm = s.light_materials.get(m.light_material as usize).map(n).unwrap_or("default".into());
                println!("    {:30} {:20} idx {}", tex, lm, m.index_count);
            }
        }
    }
    Ok(())
}
