//! Regression tests for what the fuzz targets found and what the audit
//! findings listed (docs/fuzzing.md). The
//! inputs are built here from a few header bytes; no game data.

use nfsu2::formats::bundle::{self, Chunk};
use nfsu2::formats::*;

fn chunk(id: u32, body: &[u8]) -> Vec<u8> {
    let mut v = id.to_le_bytes().to_vec();
    v.extend((body.len() as u32).to_le_bytes());
    v.extend_from_slice(body);
    v
}

fn put32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn put16(b: &mut [u8], at: usize, v: u16) {
    b[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

/// A road network of `nodes` nodes in a line and one segment `first..end`
/// holding what the file says.
fn net(nodes: usize, first: u16, end: u16, node_segment: u16) -> Vec<u8> {
    let mut n = vec![0u8; nodes * 0x18];
    for i in 0..nodes {
        n[i * 0x18..i * 0x18 + 4].copy_from_slice(&(i as f32 * 10.0).to_le_bytes());
        put16(&mut n[i * 0x18..], 8, node_segment);
        n[i * 0x18 + 0xA] = 20;
        n[i * 0x18 + 0xB] = 40;
    }
    let mut s = vec![0u8; 0xDC];
    put16(&mut s, 0x1C, first);
    put16(&mut s, 0x1E, end);
    s[0x64..0x68].copy_from_slice(&100.0f32.to_le_bytes());
    [chunk(0x34148, &n), chunk(0x34149, &s)].concat()
}

fn drive(net: &roads::RoadNet) {
    for p in [glam::Vec2::ZERO, glam::Vec2::new(25.0, 3.0)] {
        let _ = net.nearest_node(p);
        for loops in [false, true] {
            if let Some(r) = net.route(&[p, glam::Vec2::new(30.0, 0.0)], loops) {
                let road = net.race_road(&r, loops);
                let mut loc = roads::Locator::default();
                road.locate(&mut loc, p);
                let _ = (road.point_at(5.0), road.direction(5.0), road.segment_at(5.0));
            }
        }
    }
    let _ = net.race_road(&[(0, 0), (7, 1)], false);
}

#[test]
fn road_segment_ranges_past_the_node_table_are_repaired() {
    // end past the nodes, end before first, first past the end, a node of a
    // segment that is not there, a node outside its segment's range.
    for (first, end, seg) in [(0, 60000, 0), (5, 2, 0), (9, 9, 0), (0, 4, 3), (2, 4, 0), (0, 0, 0), (65535, 65535, 0)] {
        let net = roads::RoadNet::parse(&net(4, first, end, seg));
        for s in &net.segments {
            assert!(s.first < s.end && s.end <= net.nodes.len() || (s.first, s.end) == (0, 0), "{first}..{end}: {}..{}", s.first, s.end);
        }
        for (i, n) in net.nodes.iter().enumerate() {
            assert!(n.1 == usize::MAX || (net.segments[n.1].first..net.segments[n.1].end).contains(&i));
        }
        drive(&net);
    }
    // A sound file is not touched.
    let ok = roads::RoadNet::parse(&net(4, 0, 4, 0));
    assert_eq!((ok.repaired, ok.segments[0].first, ok.segments[0].end), (0, 0, 4));
}

#[test]
fn road_route_accessors_without_two_points() {
    for pts in [vec![], vec![glam::Vec2::new(1.0, 2.0)]] {
        let r = roads::RoadRoute::new(pts.clone(), false);
        let _ = (r.point_at(3.0), r.direction(3.0), r.segment_at(3.0), r.project(glam::Vec2::ZERO, Some(1.0), 5.0));
        assert!(r.segment_at(3.0).is_none());
        assert_eq!(r.point_at(3.0), pts.first().copied().unwrap_or(glam::Vec2::ZERO));
    }
    // NaN nodes: the length is NaN and a clamp(0, NaN) panicked.
    let r = roads::RoadRoute::new(vec![glam::Vec2::new(f32::NAN, 0.0), glam::Vec2::ONE], false);
    let _ = (r.point_at(1.0), r.direction(1.0));
}

#[test]
fn point_grid_with_a_stray_far_point_does_not_allocate_the_world() {
    // One record of two control points a million metres apart: 15 625^2
    // cells (5.9 GB) before.
    let mut rec = vec![0u8; 0x8C + 2 * 0x38];
    put16(&mut rec, 0x34, 2);
    put32(&mut rec, 0x0C, 0);
    for (k, x) in [0.0f32, 1.0e6, 3.0e11].into_iter().take(2).enumerate() {
        rec[0x8C + k * 0x38..0x8C + k * 0x38 + 4].copy_from_slice(&x.to_le_bytes());
        rec[0x8C + k * 0x38 + 4..0x8C + k * 0x38 + 8].copy_from_slice(&x.to_le_bytes());
    }
    for far in [1.0e6f32, 3.0e11, -3.0e11] {
        let mut r = rec.clone();
        r[0x8C + 0x38..0x8C + 0x38 + 4].copy_from_slice(&far.to_le_bytes());
        r[0x8C + 0x38 + 4..0x8C + 0x38 + 8].copy_from_slice(&far.to_le_bytes());
        let f = ai_routes::RouteFile::parse(&chunk(0x34121, &r));
        assert_eq!(f.records.len(), 1);
        let grid = ai_routes::PointGrid::new(&f);
        assert_eq!(grid.nearest(&f, glam::Vec3::new(5.0, 5.0, 0.0)).map(|n| n.0), Some(0));
    }
}

#[test]
fn junction_table_is_capped_below_the_none_slot() {
    let mut j = vec![0u8; 4 + 32 * 0x1_0010];
    put32(&mut j, 0, 0);
    let f = ai_routes::RouteFile::parse(&chunk(0x34122, &j));
    assert_eq!(f.junctions.len(), 0xFFFE);
}

#[test]
fn payload_keeps_a_filler_look_alike_past_the_128_boundary() {
    // A chunk at offset 0 (body at 8): 0x11 from file offset 16 for 256
    // bytes. The filler pushes a payload out to offset 128; what follows is
    // data that happens to be 0x11.
    let mut body = vec![0u8; 8];
    body.extend(vec![0x11u8; 256]);
    body.extend([1, 2, 3, 4]);
    let file = chunk(0x1234_5678, &body);
    let c: Chunk = bundle::chunks(&file, 0).next().unwrap();
    let p = c.payload();
    assert_eq!(p.len(), 256 - 112 + 4, "stripped to offset 128 only");
    assert!(p.starts_with(&[0x11; 16]));
}

/// A stored one-entry ZIP with this name and a body.
fn zip(name: &[u8], body: &[u8]) -> Vec<u8> {
    let crc = tpf::zip_crc(body);
    let mut z = Vec::new();
    let le16 = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_le_bytes());
    let le32 = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
    z.extend_from_slice(b"PK\x03\x04");
    for x in [20u16, 0, 0, 0, 0] {
        le16(&mut z, x);
    }
    le32(&mut z, crc);
    le32(&mut z, body.len() as u32);
    le32(&mut z, body.len() as u32);
    le16(&mut z, name.len() as u16);
    le16(&mut z, 0);
    z.extend_from_slice(name);
    z.extend_from_slice(body);
    let cd = z.len();
    z.extend_from_slice(b"PK\x01\x02");
    for x in [20u16, 20, 0, 0, 0, 0] {
        le16(&mut z, x);
    }
    le32(&mut z, crc);
    le32(&mut z, body.len() as u32);
    le32(&mut z, body.len() as u32);
    for x in [name.len() as u16, 0, 0, 0, 0] {
        le16(&mut z, x);
    }
    le32(&mut z, 0);
    le32(&mut z, 0);
    z.extend_from_slice(name);
    let cd_len = z.len() - cd;
    z.extend_from_slice(b"PK\x05\x06");
    for x in [0u16, 0, 1, 1] {
        le16(&mut z, x);
    }
    le32(&mut z, cd_len as u32);
    le32(&mut z, cd as u32);
    le16(&mut z, 0);
    z
}

#[test]
fn tpf_entries_named_outside_the_package_are_dropped() {
    for bad in ["../evil.dds", "a/../../b.dds", "/etc/passwd", "C:\\x.dds", "..\\x.dds", "\\abs.dds"] {
        assert!(!tpf::safe_name(bad), "{bad}");
        assert!(tpf::read(&zip(bad.as_bytes(), b"x")).unwrap().entries.is_empty(), "{bad}");
    }
    for good in ["SPEED2.EXE_0x12345678.dds", "texmod.def", "dir/a..b.dds"] {
        assert!(tpf::safe_name(good), "{good}");
        assert_eq!(tpf::read(&zip(good.as_bytes(), b"x")).unwrap().entries.len(), 1);
    }
}

#[test]
fn gin_with_a_silly_rate_is_refused() {
    let mut g = vec![0u8; 0x40];
    g[..5].copy_from_slice(b"Gnsu2");
    put32(&mut g, 0x1C, 0xFFFF_FFFF);
    assert!(nfsu2::audio::gin::Gin::parse(&g).is_none());
    put32(&mut g, 0x1C, 22050);
    assert!(nfsu2::audio::gin::Gin::parse(&g).is_some());
}

/// A MicroTalk SCHl stream whose block claims 4 G samples from 40 bytes
/// (the decode wrote until the machine ran out of memory).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn schl_block_sample_count_cannot_outgrow_the_stream_limit() {
    let mut h = b"SCHl".to_vec();
    h.extend(0u32.to_le_bytes());
    h.extend(b"PT\0\0");
    h.extend([0xA0, 1, 4, 0x82, 1, 1, 0x80, 1, 2, 0xFF]);
    let hs = h.len();
    put32(&mut h, 4, hs as u32);
    let mut block = b"SCDl".to_vec();
    block.extend(0u32.to_le_bytes());
    block.extend(0u32.to_le_bytes()); // +8: samples, set below
    block.extend(4u32.to_le_bytes()); // channel offset
    block.extend([1u8; 24]);
    let n = block.len();
    put32(&mut block, 4, n as u32);
    put32(&mut block, 8, 0xFFFF_FFF0);
    h.extend(block);
    let t = std::time::Instant::now();
    assert!(nfsu2::audio::schl::decode(&h).is_none());
    assert!(t.elapsed().as_secs() < 5);
}

#[test]
fn movie_audio_block_with_a_huge_sample_count_stays_small() {
    let a = nfsu2::movie::AudioHeader { channels: 2, rate: 22050, samples: 0, codec: 10, big_endian: false };
    let mut p = vec![0u8; 4 + 8 + 20];
    put32(&mut p, 0, 0x7FFF_FFFF);
    put32(&mut p, 4, 0);
    put32(&mut p, 8, 0);
    let out = nfsu2::movie::decode_block(&a, &mut [Default::default(); 2], &p);
    assert!(out.len() <= 2 * (1 << 20));
}

#[test]
fn feng_strings_and_packages_survive_cut_records() {
    // A string table chunk (0x39000) whose count claims 4 G entries.
    let mut body = vec![0u8; 16];
    put32(&mut body, 4, 0xFFFF_FFFF);
    put32(&mut body, 8, 16);
    let f = chunk(0x39000, &body);
    let s = nfsu2::frontend::feng::strings::Strings::parse(&f).unwrap();
    assert!(s.map.len() <= 1);
    // A table shorter than its header.
    let _ = nfsu2::frontend::feng::strings::Strings::parse(&chunk(0x39000, &[1, 2, 3]));
    // A package whose objects hold cut tags and records.
    for body in [b"FEngObjd".to_vec(), [b"FEng".as_slice(), &9u32.to_le_bytes(), b"Obj\0\xff\xff\xff\xff\xff"].concat()] {
        let _ = nfsu2::frontend::feng::parse(&body);
    }
    let _ = nfsu2::frontend::feng::parse_responses(b"Mi\x02\x00\x01");
}

#[test]
fn abk_with_player_offsets_past_the_file_does_not_panic() {
    let mut d = vec![0u8; 0x100];
    d[..4].copy_from_slice(b"ABKC");
    put32(&mut d, 0x1C, 0x40);
    put16(&mut d, 0x0A, 1);
    d[0x40 + 0x24] = 200; // 200 players, the record holds none
    let _ = nfsu2::audio::abk::Bank::parse(d);
}

/// A sound list that claims 4 G entries took 4 G iterations (found by the
/// long run, abk target: 3-5 s on a 13 KB input).
#[test]
fn abk_sound_list_count_is_bounded_by_the_file() {
    let mut d = vec![0u8; 0x200];
    d[..4].copy_from_slice(b"ABKC");
    put32(&mut d, 0x1C, 0x40);
    put16(&mut d, 0x0A, 1);
    d[0x40 + 0x24] = 1;
    put32(&mut d, 0x40 + 0x28, 0x80);
    put32(&mut d, 0x40 + 0x2C, 0x90);
    put32(&mut d, 0x94, 0x100);
    put32(&mut d, 0x100, 0xFFFF_FFFF);
    let b = nfsu2::audio::abk::Bank::parse(d).unwrap();
    let t = std::time::Instant::now();
    assert!(b.sound_list(&b.modules[0]).len() <= 0x200 / 12);
    assert!(t.elapsed().as_millis() < 500);
}
