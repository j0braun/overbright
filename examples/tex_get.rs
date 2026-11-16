//! tex_get BUNDLE NAME OUT.png [LEVEL]: one texture by its full name (hashed, so names
//! longer than the entry's field work) or by hex hash, its top level (or LEVEL's mip) as PNG.
use nfsu2::formats::bundle::{chunks, walk};
fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let want = u32::from_str_radix(a[1].trim_start_matches("0x"), 16).ok().filter(|_| a[1].starts_with("0x")).unwrap_or_else(|| nfsu2::formats::hash::bin_hash(&a[1].to_uppercase()));
    let raw = std::fs::read(&a[0])?;
    let data = if raw.starts_with(b"JDLZ") { nfsu2::formats::jdlz::decompress(&raw)? } else { raw };
    let mut found = false;
    walk(chunks(&data, 0), &mut |c| {
        if found || c.id != 0xB330_0000 { return; }
        let Ok(ts) = nfsu2::formats::texture::parse_texture_pack(c) else { return };
        if let Some(t) = ts.into_iter().find(|t| t.hash == want) {
            println!("{} {:08x} {}x{} {:?} mips {}", t.name, t.hash, t.width, t.height, t.format, t.mip_count);
            image::save_buffer(&a[2], &t.decode_rgba(), t.width, t.height, image::ColorType::Rgba8).unwrap();
            found = true;
        }
    });
    if !found { anyhow::bail!("not found"); }
    Ok(())
}
