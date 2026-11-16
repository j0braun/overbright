//! The movie container, the VP6 decoder and the EA-XA audio against the
//! installed Scene01 (ignored without NFSU2_ROOT).
mod common;

use nfsu2::movie::{Movie, XaChannel, decode_block, path};

fn scene01() -> Movie {
    let root = common::game_root();
    let p = path(&root, "scene01").expect("the installed Scene01 movie");
    Movie::parse(std::fs::read(p).expect("reading Scene01")).unwrap()
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn scene01_container() {
    let m = scene01();
    assert_eq!((m.header.width, m.header.height, m.header.frames), (512, 512, 4545));
    assert_eq!((m.header.rate, m.header.scale, m.header.largest), (819175, 32767, 45562));
    assert!((m.header.fps() - 25.0).abs() < 0.001);
    assert_eq!(m.frames.len(), 4545);
    assert_eq!(m.frames.iter().filter(|f| f.0).count(), 44);
    let a = m.audio.unwrap();
    assert_eq!((a.channels, a.rate, a.samples, a.codec, a.big_endian), (2, 32000, 5720277, 10, true));
    assert_eq!(m.blocks.len(), 4469);
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn scene01_audio() {
    let m = scene01();
    let a = m.audio.unwrap();
    let mut st = vec![XaChannel::default(); 2];
    // The first block opens both channels with a raw (0xEE) block.
    let first = decode_block(&a, &mut st, m.payload(&m.blocks[0]));
    assert_eq!(first.len(), 2 * 1288);
    // Left: the raw block's big-endian words 6, 7, 4, 2 after the two
    // history words.
    assert_eq!([first[0], first[2], first[4], first[6]], [6.0, 7.0, 4.0, 2.0]);
    let mut total = first.len() / 2;
    let mut peak = first.iter().fold(0f32, |p, v| p.max(v.abs()));
    for b in &m.blocks[1..] {
        let s = decode_block(&a, &mut st, m.payload(b));
        total += s.len() / 2;
        peak = s.iter().fold(peak, |p, v| p.max(v.abs()));
        assert!(s.iter().all(|v| v.is_finite()));
    }
    assert_eq!(total, 5720277);
    assert!(peak > 1000.0 && peak < 40000.0, "peak {peak}");
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn scene01_frames() {
    let m = scene01();
    let mut d = nfsu2::formats::vp6::Decoder::new();
    let mut hashes = Vec::new();
    for (_, c) in m.frames.iter().take(13) {
        d.decode(m.payload(c)).unwrap();
        // FNV-1a over the visible luma.
        let h = d.frame().plane(0).flatten().fold(0xCBF2_9CE4_8422_2325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x100_0000_01B3));
        hashes.push(h);
    }
    assert_eq!((d.width, d.height), (512, 512));
    println!("luma hashes {hashes:016X?}");
    assert_eq!(hashes[0], FRAME0);
    assert_eq!(hashes[12], FRAME12);
}

// The clone's own output (a regression guard, not a reference from the
// game): the key frame and the twelfth inter frame after it.
const FRAME0: u64 = 0x108F_BD5E_0F88_A8B9;
const FRAME12: u64 = 0x6555_72DF_28DC_FB1D;
