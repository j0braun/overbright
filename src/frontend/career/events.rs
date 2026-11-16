//! Career events (chunk 0x34A11 records): kinds, categories, opponents.

use super::*;

/// Race type, event byte +0x0F.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    Circuit,
    Sprint,
    StreetX,
    Special,
    Drag,
    Drift,
    Other(u8),
}

impl EventKind {
    pub(super) fn from(v: u8) -> EventKind {
        match v {
            0 => EventKind::Circuit,
            1 => EventKind::Sprint,
            2 => EventKind::StreetX,
            3 => EventKind::Special,
            4 => EventKind::Drag,
            5 => EventKind::Drift,
            v => EventKind::Other(v),
        }
    }

    /// The language label for the type, as the world map shows it.
    pub fn label(self) -> &'static str {
        match self {
            EventKind::Circuit => "Circuit",
            EventKind::Sprint => "Sprint",
            EventKind::StreetX => "Street X",
            EventKind::Drag => "Drag",
            EventKind::Drift => "Drift",
            _ => "Special",
        }
    }

    /// Whether the race goes round the route more than once.
    pub fn loops(self) -> bool {
        matches!(self, EventKind::Circuit | EventKind::StreetX | EventKind::Drift)
    }
}

/// Event category, byte +0x34: 3 a plain event, 1 sponsor, 2 URL, 0 the
/// scripted day-one events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Scripted,
    Sponsor,
    Url,
    Normal,
    Other(u8),
}

#[derive(Clone, Debug)]
pub struct Event {
    pub index: usize,
    /// From the string pool, e.g. "S2_CIRCUIT_3" (empty for some).
    pub name: String,
    /// +0x02 (0x4FFDD0, into the string pool 0x838428): the movie the
    /// race dialog's Accept plays before the race (0x499B50 → 0x53D640),
    /// when not empty.
    pub movie: String,
    /// +0x04 (0x513A50): the movie Return to Explore Mode plays after the
    /// event is won (0x4A3E30 → 0x53D7B0), when not empty.
    pub won_movie: String,
    pub kind: EventKind,
    pub category: Category,
    /// Career stage 1..5 (+0x37).
    pub stage: u8,
    /// Track number (+0x18), e.g. 4014: TRACKS/ROUTESL4RA/Paths4014.bin.
    pub track: u16,
    /// +0x1A: 0, 1 or 2; bit 0 runs the track backward (0x532EE3 → race
    /// record +4, the reverse flag 0x89E7A4 that picks Routes<n>B and
    /// BackwardStartLine).
    pub variant: u8,
    pub laps: u8,
    /// Cash for winning (+0x30).
    pub reward: u32,
    /// The world marker (+0x28, `bin_hash("EV_nn")`).
    pub marker: u32,
    /// What winning it unlocks (+0x08; matched by the car unlock table).
    pub unlock: u32,
    /// +0x0C: 3 a URL race that opens after a number of wins, 4 the
    /// stage's last URL race (0x513510).
    pub tier: u8,
    /// +0x0D = 1: an SUV event, opened only to an SUV (0x527D40).
    pub suv: bool,
    /// +0x10: a URL race's requirement (0x513510): tier 3 needs
    /// `>> 16` normal wins; tier 4 needs bytes 1, 2 and 3 as normal,
    /// sponsor and URL wins. Stage 5's last race (0x04CCE3E1) keeps the
    /// event it follows here.
    pub requirement: u32,
    /// +0x7F: always open (the H_ events), up to the stage's cap.
    pub always: bool,
    /// +0x36: the race's traffic level (0x5322F0 → race parameters +0x08,
    /// 0x89E7A8; read signed).
    pub traffic: i8,
    /// +0x7D: the AI racers' throttle level (0x89E804, set at 0x5323F5 by
    /// Race_SetupFromEvent; the CarDriver's cap 0x783A28[level], 0x425B52).
    pub throttle: i8,
    /// +0x18: the races, +0x7E of them (at most 4; 0x532EE3), 4 bytes
    /// each (u16 track, u8 variant, u8 laps); more than one makes a
    /// tournament (the game's race list 0x85BAB8, 0xD24-byte records +0
    /// track, +4 reversed, +8 laps; count 0x85EF4C).
    pub races: Vec<(u16, u8, u8)>,
    /// The opponents (+0x40, 0xC bytes each, +0x7C of them; set up by
    /// 0x5260C0).
    pub opponents: Vec<Opponent>,
    /// Places to reach instead of a race (+0x35 of them, 8 bytes each
    /// from +0x38: a kind byte and at +0x3C a hash). Kind 4 is a shop
    /// (its +0x38 hash): entering it wins the event (0x54CB00 with
    /// 0x4FFDE0). The D-Day intro's two events are such: the car lot
    /// CC_CAR_LOT_1, then the garage CRIB_1.
    pub goals: Vec<(u8, u32)>,
    /// +0x84: the race dialog's sender label (0x512820: when non-zero,
    /// else the first opponent's name, +0x40).
    pub sender: u32,
}

/// An event's opponent: driver name (career string, low u16 of +0x00),
/// skill source (+0x02: 0 the fixed skill at +0x03, 3 from the stage,
/// 0x5132E0; otherwise the player's car's rating; see
/// [`crate::opponents`]), car (+0x04: bin_hash("RANDOM") picks one by
/// skill, 0x4FEAF0) and the per-difficulty values (+0x08…+0x0A).
#[derive(Clone, Debug)]
pub struct Opponent {
    pub name: String,
    pub mode: u8,
    pub skill: u8,
    pub car: u32,
    pub difficulty: [u8; 3],
    /// +0x0B: the racer's Block row (racer info +0x902, 0x6373F0's sixth
    /// argument from 0x5260C0; AI tuning +0x934 + 8·row).
    pub block: i8,
}

/// The car types a RANDOM opponent is drawn from (0x4FEAF0): the main pool
/// (0x7F6DA4), the alternate one (0x7F6DC0, taken when 0x570A40 returns
/// non-zero, which it always does), the SUVs (0x7F6DDC) when the event is
/// an SUV event. See [`crate::opponents::pick_car`].
pub static OPPONENT_CARS: crate::exe_tables::Table<[u8; 26]> = crate::exe_tables::Table::new(|t| &t.opponent_cars);
pub const OPPONENT_CARS_ALT: [u8; 26] = [
    0x1B, 0x13, 0x0F, 0x16, 0x0D, 0x0B, 0x10, 0x15, 0x02, 0x0E, 0x1E, 0x1C, 0x1A, 0x04, 0x1D, 0x03, 0x01, 0x00, 0x18,
    0x17, 0x14, 0x11, 0x0C, 0x19, 0x07, 0x06,
];
pub const OPPONENT_SUVS: [u8; 3] = [8, 9, 10];

impl Event {
    /// The open-event slot it fills (0x513600): 0..4 normal circuit,
    /// drag, drift, sprint, street X; 5 SUV; 6..10 the same types as
    /// sponsor races. URL races have none.
    pub fn slot(&self) -> Option<usize> {
        let by_kind = |base: usize| match self.kind {
            EventKind::Circuit => Some(base),
            EventKind::Drag => Some(base + 1),
            EventKind::Drift => Some(base + 2),
            EventKind::Sprint => Some(base + 3),
            EventKind::StreetX => Some(base + 4),
            _ => None,
        };
        match self.category {
            _ if self.suv => Some(5),
            Category::Normal => by_kind(0),
            Category::Sponsor => by_kind(6),
            _ => None,
        }
    }

    /// The race flags the career sets from the event (0x532377): a URL
    /// event (+0x34 = 2) clears them all and sets only 0x89E7E1, which the
    /// handling never reads; else drag (0x89E7D8) for type 4, drift
    /// (0x89E7D9) for type 5, street X (0x89E7E3) for type 2. The career
    /// never sets 0x89E7E2 ([`RaceMode::Url`]).
    pub fn race_mode(&self) -> RaceMode {
        match (self.kind, self.category) {
            (_, Category::Url) => RaceMode::FreeRoam,
            (EventKind::Drag, _) => RaceMode::Drag,
            (EventKind::Drift, _) => RaceMode::Drift,
            (EventKind::StreetX, _) => RaceMode::StreetX,
            _ => RaceMode::FreeRoam,
        }
    }
}
