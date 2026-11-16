//! `bundle_dump FILE [DEPTH]` — print a bundle's chunk tree (JDLZ files are
//! decompressed first): id, size, and the first printable text in the
//! payload. For finding the front end's chunks.

use nfsu2::formats::bundle::{Chunk, chunks};

fn text(b: &[u8]) -> String {
    let mut out = String::new();
    for &c in b.iter().take(256) {
        if (0x20..0x7f).contains(&c) {
            out.push(c as char);
        } else if !out.is_empty() && out.len() >= 4 {
            break;
        } else {
            out.clear();
        }
    }
    if out.len() >= 4 { out } else { String::new() }
}

fn show(c: Chunk, depth: usize, max: usize) {
    let p = c.payload();
    println!("{:indent$}{:08X} {:8} {}", "", c.id, p.len(), text(p), indent = depth * 2);
    if c.is_container() && depth < max {
        for k in c.children() {
            show(k, depth + 1, max);
        }
    }
}

fn main() -> anyhow::Result<()> {
    // A developer tool: the developer cache, never the player's (docs/asset-cache.md).
    nfsu2::platform::dirs::developer_program();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut data = std::fs::read(&args[0])?;
    if data.starts_with(b"JDLZ") {
        data = nfsu2::formats::jdlz::decompress(&data)?;
    }
    if let Some(out) = args.iter().position(|a| a == "--save").and_then(|i| args.get(i + 1)) {
        std::fs::write(out, &data)?;
    }
    if let Some(dir) = args.iter().position(|a| a == "--fng").and_then(|i| args.get(i + 1)) {
        std::fs::create_dir_all(dir)?;
        let mut n = 0;
        nfsu2::formats::bundle::walk(chunks(&data, 0), &mut |c| {
            let raw = match c.id {
                0x30210 => {
                    let all = c.body;
                    let at = all.windows(4).take(32).position(|w| w == b"JDLZ" || w == b"HUFF").unwrap_or(4);
                    let p = &all[at..];
                    if p.starts_with(b"JDLZ") {
                        nfsu2::formats::jdlz::decompress(p).map_err(|e| eprintln!("jdlz: {e}")).ok()
                    } else {
                        nfsu2::formats::huff::decompress(p).map_err(|e| eprintln!("huff: {e}")).ok()
                    }
                }
                0x30203 => Some(c.body.to_vec()),
                _ => None,
            };
            if let Some(raw) = raw {
                // The package's file name follows its header ("…\\X.fng").
                let name = raw.windows(4).position(|w| w == b".fng").and_then(|end| {
                    let start = raw[..end].iter().rposition(|&b| b == 0 || b == b'\\').map_or(0, |s| s + 1);
                    std::str::from_utf8(&raw[start..end + 4]).ok().map(str::to_owned)
                });
                let name = name.unwrap_or_else(|| format!("pkg{n}.fng"));
                let _ = std::fs::write(format!("{dir}/{name}"), &raw);
                n += 1;
            }
        });
        println!("{n} packages");
    }
    let max = args.get(1).and_then(|d| d.parse().ok()).unwrap_or(1);
    for c in chunks(&data, 0) {
        show(c, 0, max);
    }
    Ok(())
}
