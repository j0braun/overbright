use crate::formats::hash::bin_hash;
use crate::frontend::Assets;
use crate::frontend::career::{CareerData, Category, Event, EventKind, Opponent, Track};
use crate::frontend::options::{Kind, OFF, ON, Row, RowId, Value};
use crate::frontend::screens::QrMode;

/// The mode records (types 0–11; 12 is the online type's, not kept).
pub const RECORDS: usize = 12;

/// One race type's Quick Race settings (race record +0x10 + 20 × type).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QrRecord {
    /// +0x00: human racers (1; 2 only in split screen, not in the clone).
    pub players: u8,
    /// +0x01 Opponents.
    pub opponents: u8,
    /// +0x02 Difficulty: 0 Easy, 1 Medium, 2 Hard.
    pub difficulty: u8,
    /// +0x04 Traffic Level: 0 None, 1 Minimum, 2 Moderate, 3 Maximum.
    pub traffic: u8,
    /// +0x05 Catch Up.
    pub catch_up: bool,
    /// +0x08: the track (0 before one is picked).
    pub track: u16,
    /// +0x0A Laps.
    pub laps: u16,
    /// +0x0C Track Direction: reverse.
    pub reversed: bool,
    /// +0x10 Lap KO.
    pub lap_ko: bool,
}

impl Default for QrRecord {
    /// 0x52A230's values.
    fn default() -> QrRecord {
        QrRecord {
            players: 1,
            opponents: 3,
            difficulty: 1,
            traffic: 1,
            catch_up: true,
            track: 0,
            laps: 3,
            reversed: false,
            lap_ko: false,
        }
    }
}

impl QrRecord {
    /// A new profile's record for race type `t` (0x515800 on 0x83AAB4,
    /// the profile reset, **verified**): 0x52A230's values with players 1,
    /// opponents 3, difficulty 1, traffic 1 and Catch Up on for every type,
    /// then sprint and drag 1 lap, the circuit track 4021, drift traffic
    /// Moderate, URL 5 opponents, type 5 one opponent on Easy, Free Run no
    /// opponents and no laps, and Street X track 4601, Easy, no traffic and
    /// 10 laps (**observed** under Wine, raceflow, a fresh profile's block
    /// at 0x83AAC4: the same values, the Street X options showing Laps 10,
    /// Opponents 3, Difficulty Easy).
    pub fn new_profile(t: usize) -> QrRecord {
        let mut r = QrRecord { difficulty: 1, ..QrRecord::default() };
        match t {
            0 => r.track = 4021,
            1 | 2 => r.laps = 1,
            3 => r.traffic = 2,
            4 => r.opponents = 5,
            5 => {
                r.opponents = 1;
                r.difficulty = 0;
            }
            9 => {
                r.opponents = 0;
                r.laps = 0;
            }
            10 => {
                r.track = 4601;
                r.difficulty = 0;
                r.traffic = 0;
                r.laps = 10;
            }
            _ => {}
        }
        r
    }
}

/// A new profile's records: the profile reset 0x515AA0 runs 0x515800 on
/// the block (profile +0xD4): every type 1 player, 3 opponents, Medium,
/// traffic 1, catch-up on (loop 0x515810), then Circuit's track 4021
/// (0x515852), Sprint and Drag 1 lap (0x515845 / 0x515849), Drift traffic
/// 2 (0x51585C), URL 5 opponents (0x515894), type 5 1 opponent and Easy
/// (0x51584D / 0x515870), Free Run 0 opponents and 0 laps (0x515869 /
/// 0x515860), Street X Easy, traffic 0, track 4601 and 10 laps (0x515874 …
/// 0x51588B); the laps word the loop leaves is the constructor's 3
/// (0x52A230). **Verified** in memory (menus15, Wine): a fresh profile's
/// block at 0x83AAB4 holds exactly these.
pub fn new_profile_records() -> [QrRecord; RECORDS] {
    let mut r = [QrRecord::default(); RECORDS];
    r[0].track = 4021;
    r[1].laps = 1;
    r[2].laps = 1;
    r[3].traffic = 2;
    r[4].opponents = 5;
    r[5].opponents = 1;
    r[5].difficulty = 0;
    r[9].opponents = 0;
    r[9].laps = 0;
    r[10].difficulty = 0;
    r[10].traffic = 0;
    r[10].track = 4601;
    r[10].laps = 10;
    r
}

/// The race type (g_RaceOptions, race record +0x00) a mode select option
/// sets: 0 Circuit, 1 Sprint, 2 Drag, 3 Drift, 4 URL Race, 9 Free Run, 10
/// Street X.
pub fn race_type(mode: QrMode) -> usize {
    match mode {
        QrMode::Circuit => 0,
        QrMode::Sprint => 1,
        QrMode::Drag => 2,
        QrMode::Drift => 3,
        QrMode::Url => 4,
        QrMode::FreeRun => 9,
        QrMode::StreetX => 10,
    }
}

/// 0x4987C0 (with its second argument 0): a track is listed for a race
/// type when its +0x88 is clear and its +0x94 flags have the type's bit —
/// 0 bit 0, 1 bit 2, 2 bit 1, 3 bit 3, 4 bit 13, 5 bit 12, 8 / 9 bit 4,
/// 10 bit 11, 12 bits 0 or 2.
pub fn listed(track: &Track, race_type: usize) -> bool {
    if track.hidden {
        return false;
    }
    let f = track.flags;
    let bit = |b: u32| f >> b & 1 != 0;
    match race_type {
        0 => bit(0),
        1 => bit(2),
        2 => bit(1),
        3 => bit(3),
        4 => bit(13),
        5 => bit(12),
        8 | 9 => bit(4),
        10 => bit(11),
        12 => f & 5 != 0,
        _ => false,
    }
}

/// Whether a track is open in Quick Race (0x5156F0(1, track, …): the
/// cheat byte 0x838464, else 0x5143C0): some career event (0x4FFCF0 walks
/// the events 0x8383E0, 0x88 bytes each, whose races +0x18 name the
/// track) has, on its first race on the track, the race byte +0x1A bit 1
/// set, or is won (0x514340: the event's +0x08 in the state table
/// 0x862840 with +6 = 1).
pub fn unlocked(data: &CareerData, track: u16, won: &dyn Fn(&Event) -> bool) -> bool {
    data.events.iter().any(|e| e.races.iter().find(|r| r.0 == track).is_some_and(|r| r.1 & 2 != 0 || won(e)))
}

/// A track on the list: its number and whether it is locked (list node
/// +0x08, 0x4B4C10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QrTrack {
    pub number: u16,
    pub locked: bool,
}

/// UI_QRTrackSelect's list (0x4CDED0): every listed track (0x4987C0) in
/// the track table's order (0x5D3E90), then sorted (0x43E650 with
/// 0x4987A0, a stable pass that moves a node ahead of a locked one before
/// it) so the open tracks come first, the locked ones after them, each in
/// table order.
pub fn track_list(data: &CareerData, race_type: usize, won: &dyn Fn(&Event) -> bool) -> Vec<QrTrack> {
    let mut tracks: Vec<&Track> = data.tracks.values().filter(|t| listed(t, race_type)).collect();
    tracks.sort_by_key(|t| t.order);
    let mut list: Vec<QrTrack> =
        tracks.iter().map(|t| QrTrack { number: t.number, locked: !unlocked(data, t.number, won) }).collect();
    list.sort_by_key(|t| t.locked);
    list
}

/// The list's first entry for the record (0x4CDED0): the record's track
/// when it is listed and open, else the first track (whose number the
/// record then takes, with the record's direction kept).
pub fn preselect(list: &[QrTrack], record: &mut QrRecord) -> usize {
    let at = list.iter().position(|t| t.number == record.track && !t.locked);
    let i = at.unwrap_or(0);
    if let Some(t) = list.get(i) {
        record.track = t.number;
    }
    i
}

/// A list entry's icon (0x4984B0; textures by hash): locked 0xF05704AC;
/// by the track's +0x7C, 0 0xB5A633A0, 1 0x79CEAD40, 2 0x4B938E3C, 3
/// 0x4FDE6FEB, 4 0x6BA31820, 5 the mode's own (Track_Icon_Drag,
/// Track_Icon_Drift, Track_Icon_U_Racing_League, Track_Icon_Short_Track for
/// types 2, 3, 4, 10); anything else 0x74CE8C0B, which 0x4CDED0 also puts
/// up when the texture is missing.
pub fn icon(track: &Track, locked: bool, race_type: usize) -> u32 {
    if locked {
        return 0xF057_04AC;
    }
    match track.icon {
        0 => 0xB5A6_33A0,
        1 => 0x79CE_AD40,
        2 => 0x4B93_8E3C,
        3 => 0x4FDE_6FEB,
        4 => 0x6BA3_1820,
        5 => match race_type {
            2 => bin_hash("Track_Icon_Drag"),
            3 => bin_hash("Track_Icon_Drift"),
            4 => bin_hash("Track_Icon_U_Racing_League"),
            10 => bin_hash("Track_Icon_Short_Track"),
            _ => NO_ICON,
        },
        _ => NO_ICON,
    }
}

/// 0x74CE8C0B: the icon put up when a track's has no texture (0x4CDED0).
pub const NO_ICON: u32 = 0x74CE_8C0B;

/// A track's name as the front end shows it (0x516280): the language
/// string TRACK_NAME_<n> ("Resort Loop"), or REVERSE_TRACK_NAME_<n>.
pub fn track_name(assets: &Assets, data: &CareerData, number: u16, reversed: bool) -> String {
    let key = if reversed { format!("REVERSE_TRACK_NAME_{number}") } else { format!("TRACK_NAME_{number}") };
    assets.strings.get(bin_hash(&key)).map_or_else(|| data.track_title(number), str::to_string)
}

/// The drag tracks with no Traffic Level row (0x4CD530: numbers 0x1073 to
/// 0x107F, 4211–4223).
fn drag_without_traffic(number: u16) -> bool {
    (0x1073..=0x107F).contains(&number)
}

/// The highest opponent count (0x4B37F0): 3, 5 for URL races, 2 for drag
/// track 4402 (0x1132) in game mode 2, less the players + 1; the lowest is
/// 0 in game mode 2.
fn max_opponents(race_type: usize, track: u16, r: &QrRecord) -> u8 {
    let max = if race_type == 2 && track == 0x1132 {
        2
    } else if race_type == 4 {
        5
    } else {
        3
    };
    (max + 1u8).saturating_sub(r.players.max(1))
}

/// The Opponents row's constructor (0x4B3750), run whenever
/// UI_QRModeOptions is built (after the track select): in Quick Race (game
/// mode 2) a drag on track 4402 (0x1132) with more than 2 opponents stored
/// gets 2 (0x502960). No other count is clamped.
pub fn clamp_opponents_on_open(race_type: usize, r: &mut QrRecord) {
    if race_type == 2 && r.track == 0x1132 && r.opponents > 2 {
        r.opponents = 2;
    }
}

/// UI_QRModeOptions' rows (0x4E2C70 by type): Circuit 0x4CCF60, Sprint
/// 0x4CD2D0, Drag 0x4CD530, Drift 0x4CD7A0, URL / 5 / Street X 0x4B47B0.
/// Every list starts with Track Direction (0x4B40B0) when the track runs
/// both ways (+0x86 = 1; outside Circuit also open, 0x5156F0); then
/// Circuit: Lap KO (0x4B3FA0), Laps (0x4B3D30), Traffic Level (vtable
/// 0x791458), Opponents (0x4B3750), Difficulty (0x4B3930), Catch Up
/// (vtable 0x791410); Sprint and Drag: Traffic Level (not on drag tracks
/// 4211–4223), Opponents, Difficulty, Catch Up; Drift: Laps and Opponents
/// on its +0x80 = 0 tracks, Difficulty, and Traffic Level on its +0x80 = 1
/// tracks (no opponents there: 0x89E7C4 = 0); URL and Street X: Laps
/// (vtable 0x791530), Opponents, Difficulty, Catch Up. Laps and Opponents
/// are disabled while Lap KO is on (their text methods 0x4B3DD0 /
/// 0x4B38C0 on record +0x10).
pub fn rows(race_type: usize, track: Option<&Track>, open: bool, r: &QrRecord) -> Vec<Row> {
    let row = |id: RowId, label: u32, value: Value, enabled: bool| Row {
        id,
        label,
        kind: Kind::Cycle,
        value: Some(value),
        enabled,
    };
    let on_off = |v: bool| Value::Label(if v { ON } else { OFF });
    let direction =
        row(RowId::QrDirection, 0xA88F_FEB4, Value::Label(if r.reversed { 0x22F2_54AF } else { 0x9E4C_4648 }), true);
    let lap_ko = row(RowId::QrLapKo, 0x7FD7_3849, on_off(r.lap_ko), true);
    let laps = row(RowId::QrLaps, 0x4849_4E83, Value::Owned(r.laps.to_string()), !r.lap_ko);
    let traffic = row(
        RowId::QrTraffic,
        0xEB9D_FC09,
        Value::Label([0x896B_CB4D, 0xB6BD_9C95, 0x907C_555E, 0xB672_F15A][r.traffic.min(3) as usize]),
        true,
    );
    let opponents = row(RowId::QrOpponents, 0x3384_A679, Value::Owned(r.opponents.to_string()), !r.lap_ko);
    let difficulty = row(
        RowId::QrDifficulty,
        0x4D15_6786,
        Value::Label([0xDBBB_4E8F, 0xCAB2_62DE, 0xDBBC_F37C][r.difficulty.min(2) as usize]),
        true,
    );
    let catch_up = row(RowId::QrCatchUp, 0x8B8E_913A, on_off(r.catch_up), true);
    let mut v = Vec::new();
    if track.is_some_and(|t| t.reversible) && (race_type == 0 || open) {
        v.push(direction);
    }
    let number = track.map_or(0, |t| t.number);
    match race_type {
        0 => v.extend([lap_ko, laps, traffic, opponents, difficulty, catch_up]),
        1 => v.extend([traffic, opponents, difficulty, catch_up]),
        2 => {
            if !drag_without_traffic(number) {
                v.push(traffic);
            }
            v.extend([opponents, difficulty, catch_up]);
        }
        3 => {
            let alone = track.is_some_and(|t| t.drift_kind == 1);
            if track.is_some_and(|t| t.drift_kind == 0) {
                v.extend([laps, opponents]);
            }
            v.push(difficulty);
            if alone {
                v.push(traffic);
            }
        }
        4 | 5 | 10 => v.extend([laps, opponents, difficulty, catch_up]),
        _ => {}
    }
    v
}

/// An item's Left / Right (0x9120409E / 0xB5971BF1) on the record:
/// Direction toggles (0x4B40E0, also the race's reverse flag 0x89E7A4);
/// Lap KO toggles and, on, sets 3 laps and 3 opponents (0x4B4020); Laps
/// 1–10 (0x4B3D60), Traffic 0–3 (0x4B3AD0), Difficulty 0–2 (0x4B3960) and
/// Opponents 0…[`max_opponents`] (0x4B37F0) step and wrap; Catch Up
/// toggles.
pub fn change(id: RowId, race_type: usize, r: &mut QrRecord, right: bool) {
    let step = |v: i32, lo: i32, hi: i32| -> i32 {
        let v = if right { v + 1 } else { v - 1 };
        if v < lo {
            hi
        } else if v > hi {
            lo
        } else {
            v
        }
    };
    match id {
        RowId::QrDirection => r.reversed = !r.reversed,
        RowId::QrLapKo => {
            r.lap_ko = !r.lap_ko;
            if r.lap_ko {
                r.laps = 3;
                r.opponents = 3;
            }
        }
        RowId::QrLaps => r.laps = step(r.laps as i32, 1, 10) as u16,
        RowId::QrTraffic => r.traffic = step(r.traffic as i32, 0, 3) as u8,
        RowId::QrDifficulty => r.difficulty = step(r.difficulty as i32, 0, 2) as u8,
        // 0x4B37F0: Left wraps only below the lowest, Right only above the
        // highest, so Left from above the highest steps down (5 → 4).
        RowId::QrOpponents => {
            let max = i32::from(max_opponents(race_type, r.track, r));
            let v = i32::from(r.opponents);
            r.opponents = if right {
                if v + 1 > max { 0 } else { v + 1 }
            } else if v - 1 < 0 {
                max
            } else {
                v - 1
            } as u8;
        }
        RowId::QrCatchUp => r.catch_up = !r.catch_up,
        _ => {}
    }
}

/// The names Quick Race opponents get (0x4FE970: for each, up to 100
/// draws of Rand_Int(53) until an unused one of the table 0x7F6CD0).
pub static NAMES: crate::exe_tables::Table<[&str; 53]> = crate::exe_tables::Table::new(|t| &t.opponent_names);

/// 0x4FE970(names, n): `n` distinct names (a slot whose 100 draws all hit
/// used names stays empty).
pub fn draw_names(rng: &mut crate::opponents::GameRng, n: usize) -> Vec<String> {
    let mut used = [false; 53];
    (0..n)
        .map(|_| {
            for _ in 0..100 {
                let i = rng.below(53) as usize;
                if !used[i] {
                    used[i] = true;
                    return NAMES[i].to_string();
                }
            }
            String::new()
        })
        .collect()
}

/// The race a Quick Race record sets up (0x53F160 for the lapped and
/// point-to-point types): the record's track, direction (0x89E7A4) and
/// laps (0x89E7BC), its opponents (0x89E7C4 = +0x01 with one player),
/// traffic level (0x89E7A8 = +0x04), difficulty (0x89E7F4 = +0x02) and
/// catch-up (0x89E7F8 = +0x05); with Lap KO in a circuit the race type
/// 0x89E7B0 is 3 (knockout). The opponents (0x53EEA0): names from
/// 0x4FE970, each a RANDOM car (0x4FEAF0(record, 100 + i, 2 + i, 1, the
/// player's car, 0, −1): the player's car's pool, the player's own type
/// allowed, against the player's car's rating) with a skill row
/// 0x4FED00(difficulty) draws — `rows` are those rows in order.
///
/// The clone races it as an event: kind and category by the mode, stage
/// 0xFF (in no stage), its unlock hash 0xFFFFFFFF (no career event's).
pub fn event(mode: QrMode, r: &QrRecord, index: usize, suv: bool, names: Vec<String>, rows: &[u8]) -> Event {
    let (kind, category) = match mode {
        QrMode::Sprint => (EventKind::Sprint, Category::Normal),
        QrMode::Drag => (EventKind::Drag, Category::Normal),
        QrMode::Drift => (EventKind::Drift, Category::Normal),
        QrMode::StreetX => (EventKind::StreetX, Category::Normal),
        QrMode::Url => (EventKind::Circuit, Category::Url),
        _ => (EventKind::Circuit, Category::Normal),
    };
    let variant = u8::from(r.reversed);
    let laps = r.laps.clamp(1, 255) as u8;
    let random = bin_hash("RANDOM");
    let opponents = names
        .into_iter()
        .zip(rows)
        .map(|(name, &row)| Opponent { name, mode: 2, skill: 0, car: random, difficulty: [row; 3], block: 1 })
        .collect();
    Event {
        index,
        name: "QUICK_RACE".into(),
        movie: String::new(),
        won_movie: String::new(),
        kind,
        category,
        stage: 0xFF,
        track: r.track,
        variant,
        laps,
        reward: 0,
        marker: 0,
        unlock: 0xFFFF_FFFF,
        tier: 0,
        suv,
        requirement: 0,
        always: false,
        traffic: r.traffic as i8,
        // Not an event byte: the Quick Race set-ups write 0x89E804 their
        // own way (`difficulty::RaceGlobals::quick_race`).
        throttle: -1,
        races: vec![(r.track, variant, laps)],
        opponents,
        goals: Vec::new(),
        sender: 0,
    }
}
