//! The Browser's index on the real game's region: what it finds and where it says things are used agrees with the
//! world's own placements and materials (needs the player's game files).

use std::path::Path;

use nfsu2::studio_api::world::World;
use ob_studio::browser::{Hit, Index, Kind, Model, Query};

mod testkit {
    #![allow(dead_code)]
    ob_testkit::bind!();
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_real_regions_index_agrees_with_its_world() {
    let root = testkit::game_root();
    let world = World::load(Path::new(&root), "L4RA").unwrap();
    let t0 = std::time::Instant::now();
    let i = Index::build("L4RA", &world);
    eprintln!("indexed {} models, {} textures in {:?}", i.models.len(), i.textures.len(), t0.elapsed());
    assert_eq!(i.models.len(), world.solids.len());
    assert_eq!(i.textures.len(), world.textures.len());
    let placed: usize = i.models.iter().map(Model::placed).sum();
    assert_eq!(placed, world.placements.len());
    // Signs are found, and their placements stand that solid.
    let cones = i.search(&Query { text: "sign".into(), kind: Kind::Models, ..Default::default() });
    assert!(!cones.is_empty());
    for h in cones {
        let Hit::Model(m) = h else { unreachable!() };
        let m = &i.models[m];
        assert!(m.name.to_ascii_uppercase().contains("SIGN"));
        for &p in &m.placements {
            assert_eq!(world.placements[p as usize].solid, m.solid);
        }
        let n: usize = m.sections.iter().map(|s| s.1).sum();
        assert_eq!(n, m.placed());
    }
    // Every texture a model draws lists that model among its users, and a texture found by hash is the one.
    for (mi, m) in i.models.iter().enumerate() {
        for &(h, n) in &m.textures {
            if let Some(t) = i.texture_of_hash(h) {
                assert!(i.textures[t].used_by.contains(&(mi, n)), "{} / {h:#x}", m.name);
            }
        }
    }
    let t = &i.textures[i.textures.len() / 2];
    let found = i.search(&Query { text: format!("{:#x}", t.hash), kind: Kind::Textures, ..Default::default() });
    assert!(found.contains(&Hit::Texture(i.texture_of_hash(t.hash).unwrap())));
    // Some texture is drawn by a placed model, and a section search finds what is in it.
    let sec = i.models.iter().find_map(|m| m.sections.first()).unwrap().0;
    let in_sec = i.search(&Query { text: format!("section:{sec}"), ..Default::default() });
    assert!(in_sec.iter().any(|h| matches!(h, Hit::Texture(_))) && in_sec.iter().any(|h| matches!(h, Hit::Model(_))));
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_real_games_sounds_and_strings_list_and_decode() {
    use ob_studio::browser::media::{GameMedia, Media, find_lines, find_sounds, wav};
    let root = testkit::game_root();
    let m = GameMedia::new(Path::new(&root));
    let t0 = std::time::Instant::now();
    let sounds = m.sounds();
    eprintln!("{} sounds in {:?}", sounds.len(), t0.elapsed());
    assert!(sounds.len() > 100);
    // A sound found by name decodes to a WAV of the length the bank says.
    let s = sounds.iter().find(|s| s.playable && s.frames > 0).expect("a playable sound");
    let pcm = m.pcm(&s.bank, s.samples[0]).expect("decodes");
    assert_eq!(pcm.samples.len(), s.frames as usize);
    assert_eq!(wav(&pcm).len(), 44 + 2 * pcm.samples.len());
    assert!(find_sounds(&sounds, &s.name).iter().any(|&i| sounds[i] == *s));
    // The language tables: English reads, its lines are found by words and by label.
    let langs = m.languages();
    assert!(langs.iter().any(|l| l == "English"), "{langs:?}");
    assert!(!langs.iter().any(|l| l.eq_ignore_ascii_case("labels")));
    let en = m.strings("English").unwrap();
    assert!(en.len() > 1000);
    assert!(en.windows(2).all(|w| w[0].hash < w[1].hash));
    let default = find_lines(&en, "IND_GRAPHICS_BODY");
    assert_eq!(default.len(), 1);
    assert!(m.strings("Klingon").is_none());
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_real_regions_tracks_say_their_modes_and_area() {
    use nfsu2::studio_api::map::assets::{GameAssets, MapAssets};
    let root = testkit::game_root();
    let tracks = GameAssets::new(root.into()).tracks("L4RA");
    assert!(tracks.len() > 30);
    let t = |n: u16| tracks.iter().find(|t| t.number == n).unwrap_or_else(|| panic!("track {n}"));
    assert_eq!(t(4001).modes, ["Circuit"]);
    assert!(t(4101).modes.contains(&"Sprint".to_string()));
    // Each neighbourhood of the world map is named by some track, and a track says it in its list line.
    for place in ["City Core", "Beacon Hill", "Jackson Heights", "Coal Harbor East"] {
        assert!(tracks.iter().any(|t| t.area == place), "{place}");
    }
    assert_eq!(t(4001).detail(), format!("Circuit \u{b7} {}", t(4001).area));
}
