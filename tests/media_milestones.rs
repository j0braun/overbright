//! The stage 4–5 magazine and DVD milestones as the clone loads them
//! (career chunk 0x34A16; the maintainer's playthrough notes):
//! STAGE_4_DVD_1..3 and STAGE_5_DVD_1..4 are DVD covers (+0x24 = 2 / 4)
//! asking visual ratings that rise through the stage (0x5331D0 offers a
//! cover once the car's rating reaches +0x3C), and every stage-4 / -5
//! magazine or DVD has a photo-shoot spot and its cash (+0x26, 0x541220).

mod common;

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn stage_4_and_5_media_milestones_load() {
    let root = common::game_root();
    let data = nfsu2::frontend::career::CareerData::load(&root).unwrap();
    let mut dump = String::new();
    for stage in [4u8, 5] {
        let media: Vec<_> = data.milestones.iter().filter(|m| m.stage == stage && matches!(m.kind, 1..=4)).collect();
        for m in &media {
            dump += &format!("{} {:08x} kind {} rating {} offer {} at {:08x} cash {} spot {:08x} required {}\n", m.name, m.hash, m.kind, m.rating, m.offer, m.offer_at, m.cash, m.spot, m.required);
        }
        let dvds: Vec<_> = media.iter().filter(|m| m.is_dvd()).collect();
        let want = if stage == 4 { 3 } else { 4 };
        assert_eq!(dvds.len(), want, "stage {stage} DVDs:\n{dump}");
        for (k, m) in dvds.iter().enumerate() {
            assert_eq!(m.name, format!("STAGE_{stage}_DVD_{}", k + 1), "{dump}");
            assert!(m.spot != 0, "{dump}");
        }
        // Ratings rise through the stage's covers.
        assert!(dvds.windows(2).all(|w| w[0].rating <= w[1].rating), "{dump}");
    }
    if let Ok(dir) = std::env::var("NFSU2_DUMP_MEDIA") {
        std::fs::write(dir, &dump).unwrap();
    }
}
