//! tex_find PATTERN FILE...: textures whose name contains PATTERN, with format, blend and alpha test.
use nfsu2::formats::bundle::{chunks, walk};
fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let pat = args.next().expect("pattern");
    for path in args {
        let raw = std::fs::read(&path)?;
        let data = if raw.starts_with(b"JDLZ") { nfsu2::formats::jdlz::decompress(&raw)? } else { raw };
        walk(chunks(&data, 0), &mut |c| {
            if c.id == 0xB330_0000 && let Ok(ts) = nfsu2::formats::texture::parse_texture_pack(c) {
                for t in ts.iter().filter(|t| t.name.contains(&pat)) {
                    println!("{path}: {} {}x{} {:?} {:?} test {}", t.name, t.width, t.height, t.format, t.blend, t.alpha_test);
                }
            }
        });
    }
    Ok(())
}
