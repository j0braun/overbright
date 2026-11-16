//! The dyno's slider ranges (0x5B2990) and the tuned fields at both ends of
//! every slider (0x5B4790), every part fitted: a check that the writers
//! stay finite and inside the ranges.
use nfsu2::formats::bundle::{chunks, read_cstr, read_f32};
use nfsu2::tuning::{Settings, apply, field, slider_range};

fn main() -> anyhow::Result<()> {
    let root = nfsu2::game_root();
    let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN"))?;
    let chunk = chunks(&globalb, 0).find(|c| c.id == 0x34600).unwrap();
    let career = nfsu2::formats::jdlz::decompress(&std::fs::read(root.join("GLOBAL/GlobalB.lzc"))?)?;
    let group = chunks(&career, 0).find(|c| c.id == 0x8003_4A10).unwrap();
    let packages = nfsu2::performance::parse(group.child(0x34A15).unwrap().body);
    let table = nfsu2::tuning::parse(group.child(0x34A1A).unwrap().body);
    let only = std::env::args().nth(1);
    let fitted = |p: u8| p != 9 && p != 8 && p != 12 && p != 13 && p != 29 && p != 30 && p != 31 && p != 32 && p != 40 && p != 41;
    for stock in chunk.payload().chunks_exact(0x890).take(31) {
        let name = read_cstr(stock, 0, 0x20);
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        let mut up = stock.to_vec();
        nfsu2::performance::upgrade(&mut up, &packages, &fitted);
        println!("{name}");
        for p in 0..25u8 {
            let Some((lo, hi)) = slider_range(&up, stock, &table, &fitted, p, 0) else { continue };
            let f = field(p, 0).unwrap();
            let mut ends = Vec::new();
            for t in [-1.0, 0.0, 1.0] {
                let mut s = Settings::default();
                for k in 0..9 {
                    s.set_t(p, k, t);
                }
                let mut r = up.clone();
                apply(&mut r, stock, &table, &fitted, &s);
                ends.push(read_f32(&r, f));
            }
            println!("  {p:2} range {lo:9.4} .. {hi:9.4}  field {:9.4} -> {:9.4} / {:9.4} / {:9.4}", read_f32(&up, f), ends[0], ends[1], ends[2]);
        }
    }
    Ok(())
}
