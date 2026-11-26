//! Every collision pack of the base game, read and written back, gives the same bytes. Runs against the
//! installed game: ignored (with the reason) unless `NFSU2_ROOT` is set (docs/developing/testing.md).

mod common;

use nfsu2::formats::bundle;
use nfsu2::formats::collision::{COLLISION_PACK, CollisionPack};

const REGIONS: [&str; 8] = ["L4RA", "L4RB", "L4RC", "L4RD", "L4RF", "L4RG", "L4RH", "L4RR"];

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn every_original_collision_pack_writes_back_byte_for_byte() {
    let root = common::game_root();
    let (mut total, mut same, mut bad) = (0usize, 0usize, Vec::new());
    for region in REGIONS {
        let path = nfsu2::game_fs::path(&root, &format!("TRACKS/STREAM{region}.BUN"));
        let Ok(data) = std::fs::read(&path) else { continue };
        for chunk in bundle::chunks(&data, 0).filter(|c| c.id == COLLISION_PACK) {
            total += 1;
            let start = chunk.offset - 8;
            let original = &data[start..chunk.offset + chunk.body.len()];
            match CollisionPack::parse(chunk) {
                Ok(p) if p.write() == original && start % 16 == 0 => same += 1,
                Ok(p) => bad.push(format!("{region} section {}: bytes differ (offset {start})", p.section)),
                Err(e) => bad.push(format!("{region} at {start}: {e}")),
            }
        }
    }
    println!("collision packs round-tripped: {same}/{total}");
    assert!(total > 0, "no collision packs found");
    assert!(bad.is_empty(), "{same}/{total} packs round-trip; the rest: {bad:#?}");
}

fn packs_of_region(root: &std::path::Path, region: &str) -> Vec<CollisionPack> {
    let data = std::fs::read(nfsu2::game_fs::path(root, &format!("TRACKS/STREAM{region}.BUN"))).unwrap();
    bundle::chunks(&data, 0).filter(|c| c.id == COLLISION_PACK).map(|c| CollisionPack::parse(c).unwrap()).collect()
}

/// A wall across the edge of one pack's grid is written into every pack whose grid it reaches; every
/// pack the save touches reads back as written, each section is written once, and no edit writes nothing.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_face_on_a_section_edge_is_written_into_every_pack_that_holds_it() {
    use nfsu2::formats::collision::{CollisionFace, face_record};
    use nfsu2::world::{LocalFace, packs_of, world_face, write_edited_packs};
    let root = common::game_root();
    let base = packs_of_region(&root, "L4RA");
    assert!(write_edited_packs(&base, &Default::default(), &[]).unwrap().is_empty(), "no edit, nothing written");
    // The east edge of a grid, inside another pack's grid.
    let inside = |p: &CollisionPack, x: f32, y: f32| {
        x > p.origin[0]
            && x < p.origin[0] + p.width as f32 * 32.0
            && y > p.origin[1]
            && y < p.origin[1] + p.height as f32 * 32.0
    };
    let (a, x, y) = base
        .iter()
        .find_map(|a| {
            let (x, y) = (a.origin[0] + a.width as f32 * 32.0, a.origin[1] + a.height as f32 * 16.0);
            let held = base.iter().filter(|b| inside(b, x - 1.0, y) && inside(b, x + 1.0, y)).count();
            (held >= 1 && inside(a, x - 1.0, y)).then_some((a, x, y))
        })
        .expect("a grid edge that lies inside another pack's grid");
    let wall = LocalFace {
        corners: [
            glam::Vec3::new(x - 1.5, y, 0.0),
            glam::Vec3::new(x + 1.5, y, 0.0),
            glam::Vec3::new(x + 1.5, y, 6.0),
            glam::Vec3::new(x - 1.5, y, 6.0),
        ],
        quad: true,
        surface: 0,
        flags: 0x02,
    };
    let face: CollisionFace = world_face(&wall, glam::Mat4::IDENTITY);
    let grids: Vec<_> = base.iter().map(|p| (p.origin, p.width, p.height)).collect();
    let holders: Vec<usize> = packs_of(&face, &grids).into_iter().map(|(i, _)| i).collect();
    assert!(holders.len() >= 2, "the edge face reaches {} packs", holders.len());
    let written = write_edited_packs(&base, &Default::default(), &[face]).unwrap();
    assert_eq!(written.len(), holders.len(), "one write per pack that holds the face");
    let mut sections: Vec<u16> = written.iter().map(|(s, _)| *s).collect();
    sections.sort_unstable();
    sections.dedup();
    assert_eq!(sections.len(), written.len(), "no two packs are the same section");
    assert!(written.iter().any(|(s, _)| *s == a.section));
    for (section, bytes) in &written {
        let p = CollisionPack::parse(bundle::chunks(bytes, 0).next().unwrap()).unwrap();
        let orig = base.iter().find(|b| b.section == *section).unwrap();
        assert_eq!(p.faces.len(), orig.faces.len() + 1, "pack {section}: one face added");
        assert_eq!(p.faces.last(), Some(&face_record(&face)), "pack {section}: the face's record");
        let new = (p.faces.len() - 1) as u16;
        assert!(p.refs.iter().any(|&(f, m)| f == new && m != 0), "pack {section}: a cell lists the face");
        assert_eq!(
            p.cells.iter().map(|c| c.1 as usize).sum::<usize>(),
            p.refs.len(),
            "pack {section}: cells cover the references"
        );
    }
}
