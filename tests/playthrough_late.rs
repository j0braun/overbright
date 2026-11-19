//! Stages 4, 5 and the ending against the original (the maintainer's
//! playthrough notes). The fixtures were saved by
//! SPEED2.EXE under Wine (tests/fixtures/native_saves):
//! - stage4_start.sav: stage 4 just begun (the original won S3_URL_8 on a
//!   clone-written stage-3 save, its "s14");
//! - stage4_win1.sav: that save after the hidden race S4_H_SPRINT_13 won
//!   (teleported) and KENWOOD signed at the sponsor popup;
//! - stage5_start.sav: stage 5 just begun (the original won S4_URL_7);
//! - stage5_win1.sav: that save after S5_H_SPRINT_20 won and KONIG signed;
//! - clone_s5_final.sav: a clone-written stage-5 save with only the final
//!   race S5_CIRCUIT_36 open, loaded and raced by the original:
//! - career_end_won.sav: the original's save after that final race, written
//!   at the save questions before Scene15.
//! Each holds only the game's numbers, hashes and the profile name "NAME".

mod common;

use nfsu2::formats::native_save::NativeSave;
use nfsu2::frontend::career::{self, Category, CareerData, NativeContext, Profile, visit};

/// One of the original's saves (the private set), kept for the test's run.
fn save(name: &str) -> &'static [u8] {
    Box::leak(common::private(&format!("tests/fixtures/native_saves/{name}.sav")).into_boxed_slice())
}

fn context() -> (std::path::PathBuf, CareerData, NativeContext) {
    let root = common::game_root();
    let data = CareerData::load(&root).expect("loading the career data");
    let ctx = NativeContext::new(&root, &data);
    (root, data, ctx)
}

fn load(bytes: &[u8], ctx: &NativeContext) -> Profile {
    Profile::from_native(&NativeSave::parse(bytes).unwrap(), ctx)
}

fn names(p: &Profile, data: &CareerData) -> Vec<String> {
    p.open_events(data).iter().map(|e| e.name.clone()).collect()
}

fn message(data: &CareerData, hash: u32) -> String {
    data.messages.iter().find(|m| m.hash == hash).map_or(format!("{hash:08X}"), |m| m.name.clone())
}

/// Settle a won event as UI_PostRace's Return to Explore does (0x541430):
/// returns the messages it sent.
fn win(p: &mut Profile, root: &std::path::Path, data: &CareerData, name: &str) -> Vec<String> {
    let e = data.events.iter().find(|e| e.name == name).cloned().unwrap();
    let bank = p.race_bank(data, &e);
    let before = p.inbox.entries.len();
    p.filed.insert(e.index);
    p.win_paid(data, root, &e, 60.0, Some(career::race_reward(&e, 66.0, bank)));
    p.inbox.entries[before..].iter().map(|x| message(data, x.message)).collect()
}

#[test]
#[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
fn late_saves_rewrite_byte_for_byte() {
    for (name, bytes) in [("s4", save("stage4_start")), ("s4w", save("stage4_win1")), ("s5", save("stage5_start")), ("s5w", save("stage5_win1")), ("final", save("clone_s5_final")), ("end", save("career_end_won"))] {
        let s = NativeSave::parse(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(s.to_bytes(), bytes, "{name}");
    }
}

/// A stage's city start (0x527D40): four of each normal kind (drawn) and
/// the stage's ten always-open hidden races in the original's order
/// (read live: career +0x71C8).
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn stage_starts_open_what_the_original_opens() {
    let (_, data, ctx) = context();
    for (bytes, stage, hidden) in [
        (save("stage4_start"), 4, ["S4_H_CIRCUIT_14", "S4_H_CIRCUIT_23", "S4_H_DRAG_15", "S4_H_DRAG_17", "S4_H_SPRINT_13", "S4_H_SPRINT_19", "S4_H_DRIFT_13", "S4_H_DRIFT_17", "S4_H_STREET_13", "S4_H_STREET_17"]),
        (save("stage5_start"), 5, ["S5_H_DRIFT_20", "S5_H_DRIFT_24", "S5_H_CIRCUIT_32", "S5_H_CIRCUIT_35", "S5_H_STREET_20", "S5_H_STREET_26", "S5_H_SPRINT_20", "S5_H_SPRINT_28", "S5_H_DRAG_21", "S5_H_DRAG_24"]),
    ] {
        let mut p = load(bytes, &ctx);
        assert_eq!(p.stage, stage);
        p.refresh_open(&data, &mut nfsu2::opponents::GameRng::default());
        let open = names(&p, &data);
        assert_eq!(open.len(), 30, "{open:?}");
        let h: Vec<&str> = open.iter().map(String::as_str).filter(|n| n.contains("_H_")).collect();
        assert_eq!(h, hidden);
        for kind in ["CIRCUIT", "DRAG", "DRIFT", "SPRINT", "STREET"] {
            let n = open.iter().filter(|e| !e.contains("_H_") && e.contains(&format!("_{kind}_"))).count();
            assert_eq!(n, 4, "stage {stage} {kind}");
        }
        // No sponsor signed yet: no sponsor race, no URL race.
        assert!(open.iter().all(|e| !e.contains("SPON") && !e.contains("URL")), "{open:?}");
    }
}

/// The first win of stages 4 and 5 (a hidden sprint in the original): the
/// purse (2200 / 4000) and 300 points, SMS_UNLOCK_30 only in stage 4, then
/// SELECT A SPONSOR! with all five of the stage's sponsors (0x500670); the
/// signing bonus is paid at once (0x532D00). The original's saves after
/// signing hold the same bank.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn first_wins_offer_all_five_sponsors() {
    let (root, data, ctx) = context();
    for (start, after, event, msgs, sponsors) in [
        (save("stage4_start"), save("stage4_win1"), "S4_H_SPRINT_13", vec!["SMS_UNLOCK_30"], ["KENWOOD", "SPARCO", "NOS", "ENKEI", "BREMBO"]),
        (save("stage5_start"), save("stage5_win1"), "S5_H_SPRINT_20", vec![], ["KONIG", "BILSTEIN", "STILLEN", "NEUSPEED", "GREDDY"]),
    ] {
        let mut p = load(start, &ctx);
        p.refresh_open(&data, &mut nfsu2::opponents::GameRng::default());
        assert_eq!(win(&mut p, &root, &data, event), msgs, "{event}");
        let offers: Vec<_> = p.sponsor_offers(&data).into_iter().cloned().collect();
        assert_eq!(offers.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), sponsors);
        assert!(p.needs_sponsor(&data));
        p.sign(&offers[0]);
        let orig = load(after, &ctx);
        assert_eq!(p.cash, orig.cash, "{event}");
        assert_eq!(p.stage_rep.get(&p.stage), orig.stage_rep.get(&orig.stage));
        assert_eq!(orig.sponsors.get(&orig.stage), Some(&offers[0].hash));
    }
}

/// With the sponsor signed, the city start opens one sponsor race of each
/// kind the sponsor asks for and the stage's first URL race (the original
/// read live: 33 open in stage 4, with S4_SPON_CIRCUIT_6, _DRIFT_7,
/// _SPRINT_5 and S4_URL_1; stage 5 S5_SPON_DRAG_10, _SPRINT_6,
/// _STREET_10 and S5_URL_1). The sponsor race of a kind is a random pick.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn signed_sponsor_opens_its_races_and_the_first_url() {
    let (_, data, ctx) = context();
    for (bytes, kinds, url, total) in [(save("stage4_win1"), ["CIRCUIT", "DRIFT", "SPRINT"], "S4_URL_1", 33), (save("stage5_win1"), ["DRAG", "SPRINT", "STREET"], "S5_URL_1", 33)] {
        let mut p = load(bytes, &ctx);
        p.refresh_open(&data, &mut nfsu2::opponents::GameRng::default());
        let open = p.open_events(&data);
        assert_eq!(open.len(), total);
        let spon: Vec<String> = open.iter().filter(|e| e.category == Category::Sponsor).map(|e| e.name.clone()).collect();
        assert_eq!(spon.len(), 3, "{spon:?}");
        for k in kinds {
            assert!(spon.iter().any(|n| n.contains(&format!("_SPON_{k}_"))), "{spon:?}");
        }
        assert_eq!(open.iter().filter(|e| e.category == Category::Url).map(|e| e.name.as_str()).collect::<Vec<_>>(), [url]);
    }
}

/// The final race won (0x541430 → 0x528330): bank +10000 and 300 points,
/// stage 6, nothing open, SMS_END_OF_GAME_MESSAGE newest — the original's
/// own save after that race holds the same.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn the_final_race_ends_the_career_as_the_original() {
    let (root, data, ctx) = context();
    let mut p = load(save("clone_s5_final"), &ctx);
    p.refresh_open(&data, &mut nfsu2::opponents::GameRng::default());
    assert_eq!(names(&p, &data), ["S5_CIRCUIT_36"]);
    assert_eq!(win(&mut p, &root, &data, "S5_CIRCUIT_36"), ["SMS_END_OF_GAME_MESSAGE"]);
    p.refresh_open(&data, &mut nfsu2::opponents::GameRng::default());
    let orig = load(save("career_end_won"), &ctx);
    assert_eq!((p.stage, p.cash), (6, 351_950));
    assert_eq!((orig.stage, orig.cash), (p.stage, p.cash));
    assert!(p.open_events(&data).is_empty());
    let last = |q: &Profile| q.inbox.entries.last().map(|e| message(&data, e.message));
    assert_eq!(last(&orig).as_deref(), Some("SMS_END_OF_GAME_MESSAGE"));
    assert_eq!(last(&p), last(&orig));
    assert_eq!(p.stage_rep, orig.stage_rep);
}

/// The final race lost (S4 career compare, the maintainer's private notes
/// "Losing the final race"; under Wine on clone_s5_final.sav: Caleb
/// finished, the player last): no reward, Post Race → Return to Explore,
/// Scene16, no save questions, then free roam at the engage spot with
/// stage 5, the bank unchanged, no new message, S5_CIRCUIT_36 still the
/// one open event and its visit entry +6 = 4 (the loss handler 0x528510).
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn losing_the_final_race_keeps_the_career_at_stage_five() {
    let (_, data, ctx) = context();
    let mut p = load(save("clone_s5_final"), &ctx);
    p.refresh_open(&data, &mut nfsu2::opponents::GameRng::default());
    let e = data.events.iter().find(|e| e.name == "S5_CIRCUIT_36").cloned().unwrap();
    let inbox = p.inbox.entries.len();
    p.race_started(&e);
    p.lost(&e);
    p.refresh_open(&data, &mut nfsu2::opponents::GameRng::default());
    assert_eq!((p.stage, p.cash), (5, 341_950));
    assert_eq!(p.inbox.entries.len(), inbox);
    assert_eq!(names(&p, &data), ["S5_CIRCUIT_36"]);
    assert_eq!(p.tried.get(&e.index), Some(&visit::LOST));
    // Written back, the visit entry holds +6 = 4 as the original's memory
    // read (career +0x7B00: hash, then 1, 0, 4, 0).
    let base = NativeSave::parse(save("clone_s5_final")).unwrap();
    let out = p.to_native(&base, &ctx);
    let v = out.visits().into_iter().find(|(h, _)| *h == e.unlock).map(|(_, b)| b);
    assert_eq!(v, Some([1, 0, visit::LOST, 0]));
}

/// The cash pickups of stages 4 and 5 (career chunk 0x34A1E): under Wine
/// (stage 5, after the lost final race) the pickup at (499, -1807)
/// (condition 1, stage above 3) paid 200 and set flag 39, the one at
/// (481, -1837) (stage above 4) paid 400 and set flag 56, and driving
/// into the first again paid nothing.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn stage_five_cash_pickups_pay_as_the_original() {
    let (_, data, ctx) = context();
    let mut p = load(save("clone_s5_final"), &ctx);
    let by_flag = |f: u8| *data.banks.iter().find(|b| b.flag == f).unwrap();
    let (a, b) = (by_flag(39), by_flag(56));
    assert_eq!((a.cash, a.condition, a.value), (200, 1, 3));
    assert_eq!((b.cash, b.condition, b.value), (400, 1, 4));
    assert_eq!(p.collect_bank(&data, a.trigger), Some(200));
    assert_eq!(p.collect_bank(&data, b.trigger), Some(400));
    assert_eq!(p.collect_bank(&data, a.trigger), None);
    assert_eq!(p.cash, 341_950 + 600);
    assert_eq!(p.banks.iter().copied().collect::<Vec<_>>(), [39, 56]);
}

/// A stage-5 outrun lost (Wine, clone_s5_final after the final race was
/// lost): the first engage sent SMS_OUTRUN_INTRO, the loss took 400 off
/// the bank (the stage definition's +0x02) and sent SMS_OUTRUN_DEFEAT.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn stage_five_outrun_loss_costs_the_stage_bank() {
    let (_, data, ctx) = context();
    let mut p = load(save("clone_s5_final"), &ctx);
    assert_eq!(data.stages[5].outrun_bank, 400);
    let before = p.inbox.entries.len();
    p.outrun_result(&data, false, &mut nfsu2::opponents::GameRng::default());
    assert_eq!(p.cash, 341_950 - 400);
    let new: Vec<String> = p.inbox.entries[before..].iter().map(|x| message(&data, x.message)).collect();
    assert_eq!(new, ["SMS_OUTRUN_DEFEAT"]);
}

/// After the career's end (career_end_won.sav loaded in the original): stage
/// 6, nothing open, no message arrived in free roam, and the encounter
/// manager stayed in TRYING_TO_ENGAGE with opponents loaded (0x513D30:
/// past stage 5 outruns are always offered).
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn stage_six_world_offers_outruns_and_nothing_else() {
    let (_, data, ctx) = context();
    let mut p = load(save("career_end_won"), &ctx);
    p.refresh_open(&data, &mut nfsu2::opponents::GameRng::default());
    assert_eq!(p.stage, 6);
    assert!(p.open_events(&data).is_empty());
    assert!(p.outruns_offered(&data, false, -1.0));
}

/// Career Statistics on clone_s5_final against the original's block
/// 0x835C00 (0x528A70) read on the same save under Wine
/// (the maintainer's private capture): the stage's normal wins are
/// the record's 35 slots (0x5004D0: 35/35 although 44 are won), so the
/// region completion is 100 %; the Win Percent and reputation rows print
/// `%$.1f` grouped ("4,325.0", "15,300.0").
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn career_statistics_rows_match_the_original_block() {
    use nfsu2::frontend::status::{CareerStats, Row, career_rows};
    let (_, data, ctx) = context();
    let p = load(save("clone_s5_final"), &ctx);
    let s = CareerStats::new(&p, &data);
    assert_eq!((s.wins, s.by_type), (173, [40, 31, 26, 25, 27, 24]));
    assert_eq!((s.magazines, s.sponsors, s.dvds, s.stage_dvds), ((0, 20), 4, (10, 10), (4, 4)));
    assert_eq!((s.normal, s.sponsor, s.url), ((35, 35), (3, 3), (9, 9)));
    assert_eq!(s.region, 1.0);
    assert!((s.completion - 0.7812).abs() < 1e-4 && (s.parts - 0.4172).abs() < 1e-4);
    assert_eq!(&s.reputation[1..6], &[1500.0, 7200.0, 11100.0, 15300.0, 16800.0]);
    let rows = career_rows(&s, &|h| format!("{h:08X}"), 0);
    let value = |label: &str| rows.iter().find_map(|r| match r { Row::Value(l, v) if l.trim() == label => Some(v.clone()), _ => None });
    // Races entered 3 in this save (the original's 4 counts the race it ran).
    assert_eq!(value("0D817A4A").as_deref(), Some("5,766.7"));
    assert!(rows.iter().any(|r| matches!(r, Row::Value(_, v) if v == "15,300.0")));
}

/// docs/original/outrun.md, "Stages 4 and 5": an outrun won at stage 4 with 3
/// wins before, and at stage 5 with 5, as the original gave it under Wine
/// (the maintainer's private captures; the sandbox saves: a community save at stage
/// 4 and `stage5_start`, the stage record's +0xDC set to 3 / 5): the
/// stage's bank (+250 / +400, 0x541330), +0xDC + 1, SMS_OUTRUN_VICTORY only
/// when not in the inbox yet (0x529BE0 → 0x514CB0), and the condition-2
/// trigger whose count is reached (0x533AA0 → 0x5338D0) with the generic
/// labels by counter 0x86420C: stage 4 STAGE_4_VISUAL_3 (SMS_VISUAL_GENERIC_1,
/// Spoilers 0xBB034EA6), stage 5 STAGE_5_PERF_3 (SMS_PERF_GENERIC_1, no
/// argument); the counter goes 0 → 1.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn outrun_wins_at_stages_4_and_5_give_what_the_original_gave() {
    let (_root, data, ctx) = context();
    let community: &[u8] = &common::private("tests/fixtures/native_saves/community_perf_control.sav");
    let by = |name: &str| data.messages.iter().find(|m| m.name == name).unwrap_or_else(|| panic!("{name}")).hash;
    let trigger = |name: &str| {
        let t = data.mag_triggers.iter().find(|t| t.name == nfsu2::formats::hash::bin_hash(name)).unwrap();
        t.message
    };
    // The stage definitions (0x34A18): the outrun bank +0x02 and the
    // cap on the stage's outrun wins +0x40 (0x513D30; −1 none).
    let defs: Vec<(i16, i8)> = (0..7).map(|s| data.stages.get(s).map_or((0, 0), |d| (d.outrun_bank, d.required))).collect();
    // Stages 0–3 cap at 3 / 5 / 10 / 15 (the community save holds exactly
    // these), 4 and 5 at 20 (the finished-career save's stage-4 record holds 20).
    assert_eq!(defs[..6], [(100, 3), (100, 5), (100, 10), (100, 15), (250, 20), (400, 20)]);
    for (bytes, stage, before, bank, sent, body, subject, extra) in [
        (community, 4u8, 3u32, 250u32, vec![trigger("STAGE_4_VISUAL_3")], "SMS_VISUAL_GENERIC_1_BODY", "SMS_VISUAL_GENERIC_1_SUBJECT", 0xBB03_4EA6u32),
        // The stage-5 save had never engaged: the start's SMS_OUTRUN_INTRO
        // (condition 4, game.rs) is in the inbox before the end, as there.
        (save("stage5_start"), 5, 5, 400, vec![by("SMS_OUTRUN_VICTORY"), trigger("STAGE_5_PERF_3")], "SMS_PERF_GENERIC_1_BODY", "SMS_PERF_GENERIC_1_SUBJECT", 0),
    ] {
        let mut p = load(bytes, &ctx);
        assert_eq!(p.stage, stage);
        p.outrun_wins.insert(stage, before);
        if stage == 5 {
            assert!(!p.inbox.contains(by("SMS_OUTRUN_INTRO")) && !p.inbox.contains(by("SMS_OUTRUN_VICTORY")));
            p.inbox.add(by("SMS_OUTRUN_INTRO"));
        } else {
            assert!(p.inbox.contains(by("SMS_OUTRUN_VICTORY")));
        }
        let (cash, n) = (p.cash, p.inbox.entries.len());
        assert_eq!(p.trigger_counters.unique, 0);
        let paid = p.outrun_result(&data, true, &mut nfsu2::opponents::GameRng::default());
        assert_eq!((paid as u32, p.cash - cash), (bank, bank), "stage {stage}");
        assert_eq!(p.outrun_wins[&stage], before + 1);
        let new: Vec<u32> = p.inbox.entries[n..].iter().map(|e| e.message).collect();
        assert_eq!(new, sent, "stage {stage}");
        let e = p.inbox.entries.last().unwrap();
        assert_eq!((e.body, e.subject, e.extra), (nfsu2::formats::hash::bin_hash(body), nfsu2::formats::hash::bin_hash(subject), extra), "stage {stage}");
        assert_eq!(p.trigger_counters.unique, 1);
        // Under Wine the trigger's message was urgent (entry +5 set: its
        // mailbox opened in free roam right after the end, 0x529760 →
        // 0x529710), SMS_OUTRUN_VICTORY not.
        let urgent = |h: u32| data.messages.iter().find(|m| m.hash == h).unwrap().urgent;
        assert!(urgent(*sent.last().unwrap()) && !urgent(by("SMS_OUTRUN_VICTORY")));
        assert!(p.inbox.urgent_pending(&data.messages));
    }
}
