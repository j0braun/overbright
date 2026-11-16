//! The shot harness's car fitting (`--shot` with NFSU2_FIT, `vinyl_car`,
//! the `hydro` shots, `neon_car`): parts put on the profile's car before a
//! shot.
use crate::*;

/// `NFSU2_FIT=slot[=name],…` (slots in hex): fit the player's car with
/// each slot's part of that name, or its first part above level 0, before
/// a shot (`--shot decal_door` with `NFSU2_FIT=0x79,0x7A,0x5D`: two left
/// door decals and the carbon-fibre kit).
pub(crate) fn install_shot_fit(game: &mut Game) {
    let want = std::env::var("NFSU2_FIT").unwrap_or_default();
    let (car, installed, perf) = game.player_car();
    if let Some(p) = game.profile.as_mut() {
        (p.car, p.installed, p.perf) = (car.clone(), installed, perf);
    }
    for item in want.split(',').filter(|i| !i.is_empty()) {
        let (slot, name) = item.split_once('=').map_or((item, None), |(a, b)| (a, Some(b)));
        let Ok(slot) = u8::from_str_radix(slot.trim_start_matches("0x"), 16) else { continue };
        let mut after = None;
        while let Some(p) = game.game_data.parts.find_after(&car, slot as usize, after) {
            after = Some(p);
            let hit = match name {
                Some(n) => game.game_data.parts.string(p.name as u32).as_deref() == Some(n),
                None => p.level > 0,
            };
            if hit {
                let hash = p.hash;
                if let Some(pr) = game.profile.as_mut() {
                    pr.installed.insert(slot, hash);
                }
                break;
            }
        }
    }
}

/// `--shot vinyl_car`'s look: a flames vinyl on the bottom layer in a
/// green vinyl colour on the profile's car.
pub(crate) fn install_shot_vinyl(game: &mut Game) {
    let car = game.profile.as_ref().map(|p| p.car.clone()).unwrap_or_default();
    let mut after = None;
    let mut vinyl = None;
    while let Some(p) = game.game_data.parts.find_after(&car, 0x40, after) {
        after = Some(p);
        if p.kind == 8 {
            vinyl = Some(p.hash);
            break;
        }
    }
    let colour = game
        .game_data.parts
        .find_after(&car, 0x4D, None)
        .and_then(|p| game.game_data.parts.find_after(&car, 0x4D, Some(p)))
        .map(|p| p.hash);
    if let (Some(p), Some(v), Some(c)) = (game.profile.as_mut(), vinyl, colour) {
        p.installed.insert(0x40, v);
        p.installed.insert(0x4D, c);
    }
}

/// The `hydro` shots: the player's car with a level-2 hydraulics part
/// (slot 0xA1; `NFSU2_HYDRAULICS_LEVEL` picks another level).
pub(crate) fn install_shot_hydraulics(game: &mut Game) {
    let level: u8 = std::env::var("NFSU2_HYDRAULICS_LEVEL").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
    // `NFSU2_HYDRO_CAR`: a stock car of that type instead (the original's
    // career free roam drives a 350Z).
    let (car, installed, perf) = match std::env::var("NFSU2_HYDRO_CAR") {
        Ok(c) => (c, Default::default(), Default::default()),
        Err(_) => game.player_car(),
    };
    let hash = game.game_data.parts.slot_parts(&car, 0xA1).into_iter().find(|p| p.level == level).map(|p| p.hash);
    if let Some(p) = game.profile.as_mut() {
        (p.car, p.installed, p.perf) = (car, installed, perf);
        if let Some(h) = hash {
            p.installed.insert(0xA1, h);
        }
    }
}

/// `--shot neon_car` (and `neon` with NFSU2_NEON set): an underglow (slot
/// 0x9A) by name, NFSU2_NEON (default MAGENTA; "RED PULSE" etc. for the
/// pulsing ones), on Rachel's car (the D-Day preset, slot 0x9A NO NEON)
/// made the profile's own so the part sticks.
pub(crate) fn install_shot_neon(game: &mut Game) {
    let want = std::env::var("NFSU2_NEON").unwrap_or("MAGENTA".into());
    let (car, installed, perf) = game.player_car();
    if let Some(p) = game.profile.as_mut() {
        (p.car, p.installed, p.perf) = (car.clone(), installed, perf);
    }
    let mut after = None;
    while let Some(p) = game.game_data.parts.find_after(&car, 0x9A, after) {
        after = Some(p);
        if game.game_data.parts.string(p.name as u32).as_deref() == Some(want.as_str()) {
            let hash = p.hash;
            if let Some(pr) = game.profile.as_mut() {
                pr.installed.insert(0x9A, hash);
            }
            break;
        }
    }
}
