//! Write every texture of a bundle to PNG: tex_png <bundle> <out dir>.
use nfsu2::formats::bundle::{chunks, walk};
fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("bundle");
    let out = std::path::PathBuf::from(args.next().unwrap_or(".".into()));
    let raw = std::fs::read(&path)?;
    let data = if raw.starts_with(b"JDLZ") { nfsu2::formats::jdlz::decompress(&raw)? } else { raw };
    walk(chunks(&data, 0), &mut |c| {
        if c.id == 0xB330_0000 && let Ok(ts) = nfsu2::formats::texture::parse_texture_pack(c) {
            for t in ts {
                let p = out.join(format!("{}.png", t.name));
                let _ = image::save_buffer(&p, &t.decode_rgba(), t.width, t.height, image::ColorType::Rgba8);
                println!("{}", p.display());
            }
        }
    });
    Ok(())
}
