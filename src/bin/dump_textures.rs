//! `dump_textures TRACK PATTERN OUT_DIR` — write every texture whose name (or
//! hex hash) contains PATTERN to OUT_DIR as PNG. A check on the decoders.

use nfsu2::world::World;

fn main() -> anyhow::Result<()> {
    // A developer tool: the developer cache, never the player's (docs/asset-cache.md).
    nfsu2::platform::dirs::developer_program();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [track, pattern, out] = &args[..] else {
        anyhow::bail!("usage: dump_textures TRACK PATTERN OUT_DIR");
    };
    let world = World::load(&nfsu2::game_root(), track)?;
    for t in world.textures.values() {
        if t.name.contains(pattern.as_str()) || format!("{:08X}", t.hash).contains(pattern.as_str()) {
            println!("{:08X} {:28} {}x{} {:?} blend {:?} test {}", t.hash, t.name, t.width, t.height, t.format, t.blend, t.alpha_test);
            let path = format!("{out}/{}.png", t.name);
            image::save_buffer(path, &t.decode_rgba(), t.width, t.height, image::ColorType::Rgba8)?;
        }
    }
    Ok(())
}
