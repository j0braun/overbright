//! `inspect [TRACK]` — load a track and print what was found. A quick check
//! that the parsers agree with the data.

use std::collections::HashMap;

use nfsu2::world::World;

fn main() -> anyhow::Result<()> {
    // A developer tool: the developer cache, never the player's (docs/asset-cache.md).
    nfsu2::platform::dirs::developer_program();
    env_logger::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let track = args.first().cloned().unwrap_or_else(|| "L4RA".into());
    let find = args.iter().position(|a| a == "--find").and_then(|i| args.get(i + 1));
    let world = World::load(&nfsu2::game_root(), &track)?;

    let vertices: usize = world.solids.iter().map(|s| s.vertices.len()).sum();
    let triangles: usize = world.placements.iter().map(|p| world.solids[p.solid].indices.len() / 3).sum();
    println!("sections   {}", world.sections.len());
    println!("solids     {} ({vertices} vertices)", world.solids.len());
    println!("textures   {}", world.textures.len());
    println!("placements {} ({triangles} triangles placed)", world.placements.len());
    println!("missing    {} instances reference an unknown solid", world.missing_solids);

    let mut formats = HashMap::new();
    for t in world.textures.values() {
        *formats.entry(format!("{:?}", t.format)).or_insert(0) += 1;
    }
    println!("formats    {formats:?}");

    let missing_textures: usize = world
        .solids
        .iter()
        .flat_map(|s| &s.textures)
        .filter(|h| !world.textures.contains_key(h))
        .count();
    println!("solid texture refs not in this track: {missing_textures}");

    if let Some(pattern) = find {
        // Where are placements of solids whose name contains `pattern`?
        for p in world.placements.iter().filter(|p| world.solids[p.solid].name.contains(pattern.as_str())).take(8) {
            let s = &world.solids[p.solid];
            let v = s.vertices[s.indices[0] as usize].position;
            let w = p.transform.transform_point3(glam::Vec3::from(v));
            println!("{:40} section {:5} first vertex {:.1},{:.1},{:.1}", s.name, p.section, w.x, w.y, w.z);
            for m in &s.materials {
                let idx = &s.indices[m.first_index as usize..(m.first_index + m.index_count) as usize];
                let zs = idx.iter().map(|&i| s.vertices[i as usize].position[2]);
                let (lo, hi) = zs.fold((f32::MAX, f32::MIN), |(a, b), z| (a.min(z), b.max(z)));
                println!("    material slot {} indices {} z {lo:.0}..{hi:.0}", m.texture_slot, m.index_count);
            }
        }
    }

    let (lo, hi) = world.bounds();
    println!("bounds     {lo:?} .. {hi:?}");
    Ok(())
}
