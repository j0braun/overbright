use super::*;

/// SFXCTL_MasterVol's update (0x471970) from the volumes (0x83AA28: +0
/// master, +4 speech, +8 menu music, +0xC game music, +0x10 effects,
/// +0x14 engine, +0x18 tires): with c = curve 0 (0x468160) of 0x7FFF −
/// trunc(master · 32767), / 32767, field f = trunc((1 − v · master · c)
/// · 32767) for v = f0 menu music, f1 game music, f2 speech, f3 effects,
/// f4 tires, f5 engine. Section 0's controls 0..5 read them through curve
/// 8 (0x7FFF − x), so control k = lin_to_db(the volume): master 0 (the
/// EA Trax object's slot 0) takes control 0, master 1 (its slot 1)
/// control 1, master 4 (SFXObj_Ambience, the interactive score, slots 0
/// and 1) control 3, the effects volume (MAPOUTPUT, DFT and DRG alike).
///
/// The same update (0x471A84..0x471B72) then sets f8..f13 (+0x24..+0x38)
/// = trunc(v · master · 32767) for v = menu music, game music, speech,
/// effects, tires, engine: the scales of section 0's event controls (the
/// speech ducks scale by f10, the movie ducks by f11). f6 is the sound
/// hold ([`Scene::sound_hold`]).
///
/// f14 (+0x3C, 0x471B75..0x471BBB; also stored to 0x7ED344): 0x7FFF in a
/// drag (0x89E7D8 / 0x890108) or a drift (0x89E7D9 / 0x890104), or when
/// the EA Trax controller (0x82B9C8, EAXS_PFController) has no song for
/// in game (+0x98, the count of playlist entries in state 2 Racing or 3
/// All that 0x479130.. hands to 0x46BAA0); else f1 (the game-music
/// field: 0 at full game music, 32767 with it off). Read under Wine: 0 /
/// 16384 / 32767 at game music 1 / 0.5 / 0. Section 0's controls 6..8
/// (0x6900000E: curve 9 of f14, boosts 2040 / 839 / 1500 mB) feed the
/// player's wind (class 7), weather wind (class 10) and others: with no
/// music the world's own beds come up (FX_WIND_Weather 205 → 2154 at
/// idle, measured). `in_game_fill` = the drag / drift / no-song case.
///
/// f7 (+0x20, 0x471C50): 1 with race type 8 (0x89E7B0), the race status
/// (0x890118) in state 4 and 0x827EC8 / 0x827EB0 / 0x827E98 all 0; f15
/// (+0x40, 0x471C22): 1 while a movie plays (0x4FE4A0) with [0x8383A4]'s
// prov: open audio.mixmap.player_vol_fields
/// state in 30..34. f15 is 0 here (**open**: not modelled); the caller
/// sets f7 on the dyno's screens (race type 8 in race status 4 after
/// DynoEnter, read under Wine), its only race type 8 case.
/// The player's volume fields (0x45C210, section 1 class 3) from the
/// option volumes and MasterVol's f14 (`fields[14]`, the 0x7ED344 copy).
pub fn player_vol_fields(
    master: f32,
    speech: f32,
    menu_music: f32,
    effects: f32,
    engine: f32,
    tires: f32,
    f14: i32,
) -> [i32; 7] {
    let v = |x: f32| (x * master * 32767.0) as i32;
    [v(speech), v(menu_music), f14, v(effects), v(engine), v(tires), (0x7FFF - f14).clamp(0, 0x7FFF)]
}

pub fn master_vol_fields(master: f32, vols: [f32; 6], in_game_fill: bool) -> [i32; 16] {
    let c = curve(0, 0x7FFF - (master * 32767.0) as i32, false) as f32 * (1.0 / 32767.0);
    let mut f = [0; 16];
    for (k, v) in vols.iter().enumerate() {
        f[k] = ((1.0 - v * master * c) * 32767.0) as i32;
        f[8 + k] = (v * master * 32767.0) as i32;
    }
    f[14] = if in_game_fill { 0x7FFF } else { f[1] };
    f
}
