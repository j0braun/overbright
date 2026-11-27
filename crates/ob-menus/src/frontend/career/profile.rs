//! The player's profile: My Cars records and the `Profile` struct.

use super::*;

/// The number of "My Cars" records (0x5165E0 / 0x5035C0 walk 0x14).
pub const MY_CARS: usize = 20;

/// A "My Cars" record (garage +0x544 + 0x434·i; **verified** by the
/// readers 0x5035C0 / 0x5165E0, the maker 0x52A720 and the fixture
/// mycar.sav): head +8 the id, bin_hash("TUNED_CAR_<i>") with i the
/// record's index (0x52A720 through 0x50B990), +0xC the rating (0 until
/// the car object is built), +0x10 usable (1), +0x14 the Quick Race filter
/// bit 2 (My Cars; a free record keeps 2, 0x521110); +0x18 the car record
/// (the car type, the part in every slot, the upgrade state).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MyCar {
    pub id: u32,
    /// The car type ("PEUGOT"); empty when the type is not one the clone
    /// knows (the record is then left as the file has it).
    pub car: String,
    /// The parts that are not the stock ones.
    pub installed: BTreeMap<u8, u32>,
    /// The fitted performance part ids (upgrade state +0x50).
    pub perf: BTreeSet<u8>,
}

/// The id of My Cars record `i` (0x52A720: "TUNED_CAR_%d").
pub fn tuned_car_id(i: usize) -> u32 {
    bin_hash(&format!("TUNED_CAR_{i}"))
}

/// A stock car's garage id: bin_hash("STOCK_<car>") (the stock entries'
/// +8 in every fixture: "STOCK_PEUGOT" = 0x959B1E16 is the 206's entry;
// prov: inferred frontend.career.stock_id
/// the maker of the list, 0x5162D0, is not traced further — **inferred**
/// from the values).
pub fn stock_id(car: &str) -> u32 {
    bin_hash(&format!("STOCK_{car}"))
}

/// The player's career: saved as `key value` lines.
#[derive(Clone, Debug)]
pub struct Profile {
    /// Rachel's hints (career +0x887C, saved with the career).
    pub hints: crate::frontend::dday::Hints,
    pub name: String,
    pub cash: u32,
    pub car: String,
    pub stage: u8,
    pub completed: BTreeSet<usize>,
    /// Wins filed in their stage record's win lists (0x513390: the race's
    /// end 0x54CA70 files a win before the win handler runs on the
    /// post-race exit) whose visit entry is not won: a Restart after the
    /// win (0x5F41F0(9)) leaves the outcome unsettled, so the stage record
    /// keeps the entry (stage win counts, the sponsor's races, messages)
    /// while the event stays open and unwon (0x500B80). Saved in the
    /// stage records' lists.
    pub filed: BTreeSet<usize>,
    /// Events raced or declined but not won, by index: their visit-table
    /// entry's +6 ([`visit`]: 2 current, 3 declined, 4 lost). Nothing in
    /// the game reads these states (only +6 = 1 won, 0x500B80, and 5 for a
    /// shop, 0x500460); they are kept for the native save.
    pub tried: BTreeMap<usize, u8>,
    /// Reputation points.
    pub reputation: u32,
    /// Best times per event, seconds.
    pub best: HashMap<usize, f32>,
    /// The sponsor signed for each stage (sponsor hash).
    pub sponsors: HashMap<u8, u32>,
    /// The events open to race (career +0x71C8, 64 slots, by event hash):
    /// kept until won, refilled by [`Profile::refresh_open`].
    pub open: Vec<u32>,
    /// Stage milestones reached (0x34A16 hashes; the game keeps them with
    /// the won events, 0x500B80).
    pub milestones: BTreeSet<u32>,
    /// Milestones offered (state 2): their photo shoot waits at its spot.
    pub offered: BTreeSet<u32>,
    /// The shops driven into (the visit table, career +0x7AFC count and
    /// +0x7B00 entries of a hash and a flag byte, keyed by the shop's
    /// +0x38; 0x512FF0 sets the flag, 0x5003B0 looks it up).
    pub visited: BTreeSet<u32>,
    /// The milestones whose photo spot was driven into (their visit-table
    /// entry's +5 bit 0, set by 0x5011C0 the first time, which then
    /// engages the spot at once).
    pub spotted: BTreeSet<u32>,
    /// The shops entered (their visit-table entry's +6 = 5, set by 0x54CB00
    /// on entering; 0x500460 tests it): a shop with a movie (+0x20) plays it
    /// only while it is not in here (0x499640).
    pub entered: BTreeSet<u32>,
    /// The order the visit-table entries were first touched (0x512F10
    /// appends a new entry at the end of the table when it is first
    /// written: under Wine the stage-1 save had CRIB_1, entered first,
    /// before the Street X raced next). Not saved itself: the native
    /// table keeps the order; this orders the entries a write appends.
    pub visit_seq: Vec<u32>,
    /// The world challenges done (the same flag table keyed by the
    /// challenge's name hash, +6 = 1: completed, 0x541970 / 0x5419D0); a
    /// done one is never on offer again (0x527980).
    pub challenges_done: BTreeSet<u32>,
    /// Parts bought for the career car, by (slot, part hash) (0x503DB0);
    /// owned parts list in the shops whatever their unlocks.
    pub owned: BTreeSet<(u8, u32)>,
    /// The part fitted in each slot where it is not the stock one.
    pub installed: BTreeMap<u8, u32>,
    /// Per stage, the reputation points its wins earned and how many
    /// (stage record +0xE0 / +0xE4, 0x513430).
    pub stage_rep: BTreeMap<u8, (f32, u32)>,
    /// Per stage, the outruns won (stage record +0xDC, counted by 0x541330;
    /// 0x513D30 offers outruns while it is under the stage definition's
    /// +0x40).
    pub outrun_wins: BTreeMap<u8, u32>,
    /// The stage records whose stage is completed (stage record +0xFC,
    /// latched by the stage end check 0x532C70, never cleared but by the
    /// record reset 0x500700; **verified**). Completing
    /// the current stage's record also advances the career (0x528330).
    pub stage_done: BTreeSet<u8>,
    /// The outrun encounters' wait (career +0x4230 → +0x1AE4 = career
    /// +0x5D14), s: 30.0 from the career reset 0x532840 (0x53289A; a new
    /// career and every profile load, 0x5344E0), less each free-roam frame
    /// (0x513DB0), 0 when an outrun's opponent is gone (0x500CA0). Not
    /// saved.
    pub outrun_timer: f32,
    /// The fitted performance part ids (0–0x43; the upgrade state +0x50).
    pub perf: BTreeSet<u8>,
    /// The performance part ids bought for the car (the career car's bit
    /// set at +0x590, 0x530360; set by 0x503E10 on buying): fitting them
    /// again is free.
    pub perf_owned: BTreeSet<u8>,
    /// The performance uniques a world challenge gave the car (0x521440:
    /// car record +0x2F4 + 4n, n 0..9). What they change in the handling
    /// is **open**.
    pub perf_uniques: BTreeSet<u8>,
    /// The car's brand list (career car slot +0x6E0 = car record +0x434 +
    /// 0x2AC): 0x503E10 appends the brand picked on
    /// UI_PerformanceBrandSelect (0x8389B4…; 0xFFFFFFFF when none was
    /// picked) for each performance part bought, into the first empty of
    /// 0x44 entries, never removing one.
    pub brands: Vec<u32>,
    /// The dyno's slider settings for the current car, one per tuning
    /// mode ([`TUNING_MODES`]): the car object (vtable 0x79B134, built by
    /// 0x516B30) keeps five 0x29-byte settings at +0x360 (0x516A60 fills
    /// them with 100 through 0x5A7940); the dyno copies them to its working
    /// set (+0x904) and Mode Select loads one into the tuning object +0x14
    /// (0x50FC60). That this car object is the career slot's is
    /// **inferred**.
    pub tuning: [crate::tuning::Settings; TUNING_MODES],
    /// The career car slots (CAREER_SLOT_0..4): `car`, `installed`,
    /// `owned` and `perf` above are the current slot's, kept here while
    /// another slot is current.
    pub slots: Vec<CarSlot>,
    /// The current slot (career +0x8740, the slot's name).
    pub slot: usize,
    /// The World Map legend's hidden groups (career +0x8869, saved; 0x496390
    /// flips them; the In-game World Map's `hidden`).
    pub map_hidden: [bool; 14],
    /// The D-Day intro event under way (career +0x8738; 0x5410F0), by its
    /// hash.
    pub intro: Option<u32>,
    /// Career +0xC08 (0x85B948) set: a world challenge runs (0x533BF0 sets
    /// it, 0x541970 / 0x5419D0 / 0x533D00's give-up and 0x5139C0 /
    /// 0x513B30 clear it). While set, 0x527D40 empties the open events and
    /// returns (`refresh_open`). Not saved (0x514530 does not write
    /// +0xC00..); the game sets it with its `WorldChallenge`.
    pub challenge_running: bool,
    /// Main-menu Customize's car (game mode 0x20): never saved.
    pub scratch: bool,
    /// The current car's fitted parts when the shop being visited was
    /// entered: the career ride's aerodynamics stay theirs until the shop
    /// is left ([`Profile::build_shop_ride`], [`Profile::store_visual`]). Not
    /// saved.
    pub shop_ride: Option<BTreeMap<u8, u32>>,
    /// The car the main menu shows once the career has a car of its own
    /// (player 0's car name at 0x83AAB8, in the saved block 0x5344E0
    /// copies): set only by 0x521110 when a customize session backs out to
    /// the Quick Race or online car select (0x55BE00), never by the career's
    /// shops (they go back to UI_CareerCrib). The game keeps the car in its
    /// "my cars" list and the name here; the clone keeps the car itself.
    pub menu_car: Option<(String, BTreeMap<u8, u32>)>,
    /// Player 0's car id (race record +4, 0x83AAB8; 0x502A40 writes it):
    /// what `menu_car` is, by its garage id — a My Cars record's
    /// "TUNED_CAR_n" hash or a stock entry's "STOCK_<car>" (`stock_id`).
    /// None leaves the file's.
    pub menu_car_id: Option<u32>,
    /// "My Cars" (garage 0x83AD90 +0x544, 20 records of 0x434, saved
    /// 0x430 each from +4): the cars main-menu Customize keeps (see
    /// [`MyCar`]); None a free record (+8 = 0, 0x5165E0).
    pub my_cars: Vec<Option<MyCar>>,
    /// The SMS inbox (career +0x5D20, 0x860A60).
    pub inbox: crate::frontend::sms::Inbox,
    /// Free-roam driving time, s (career +0x8764; 0x551330 adds the
    /// frame time; saved, 0x51465D).
    pub drive_time: f32,
    /// Races entered (career +0x872C): 0x541170 counts each career event
    /// started from its marker (0x499B27), the World Map (0x4E1284) or
    /// Next Race (0x5416D1); a new career zeroes it (0x5329CB). Saved
    /// (0x514579). Career Statistics' "Races Entered:".
    pub races_entered: u32,
    /// The current slot's "Total Spent On Car" (the garage record's
    /// +0x7F0): the part purchase 0x547580 adds each bought part's price
    /// (0x547747); nothing else writes it (the car lot's 0x516B90 leaves
    /// it), so a slot keeps its total across cars. Career Statistics reads
    /// it (0x4E1526).
    pub spent: u32,
    /// The upgrade state's ten category levels (+0x00, 3 × each category's
    /// level) as the car record got them when the shop last stored the
    /// ride into it (0x503950, [`Profile::store_ride`]); `None` until then,
    /// the record keeping the loaded floats (the original's resave without
    /// a purchase leaves them).
    pub record_levels: Option<[f32; 10]>,
    /// The magazine (0x864210) and unique-part (0x86420C) counters the
    /// trigger messages cycle their texts by (0x5338D0).
    pub trigger_counters: crate::frontend::sms::TriggerCounters,
    /// Where the city start puts the car (career +0x8744 the flag, +0x8750
    /// the position, +0x8760 the heading; saved by 0x5145F3 and loaded by
    /// 0x52930D with the career). With the flag set the city starts
    /// (0x5404B0 / 0x540790 → racer record +0x91C / +0x920 / +0x930) hand
    /// it to the car's placement (0x606666), else the track's
    /// ForwardStartingPosition (0x5EB5E0). A new career clears the flag
    /// (0x532840); see [`Profile::explore_at`].
    pub explore_set: bool,
    /// Career +0x873F (0x86347F): stage 0's races revealed (Rachel's
    /// hint kind 2, 0x53E0C0 case 2), and +0x873E (0x86347E): closed
    /// again after her third complaint (kind 1). Saved with the career
    /// (0x514585 / 0x529299); a new career clears both (0x532A30).
    pub intro_races: bool,
    pub intro_closed: bool,
    /// Career +0x873C (0x86347C): the career's intro movie has played. Set
    /// once that movie's D-Day start has run the city start (0x54AF90
    /// mode 6, 0x54AFDA), cleared by a new career (0x53292D), saved and
    /// loaded with the career (0x5145F3 / 0x529307). With it set the career
    /// start goes straight to the city (0x495A30), whose start ends the
    /// D-Day script (0x5404FE: state 9), and UI_StartCareer offers "Resume
    /// Career" beside a fresh "Start New Career" (0x4AF165).
    pub intro_played: bool,
    /// The saved position and heading (`None`: never written, zero in the
    /// game).
    pub explore_pos: Option<(Vec3, f32)>,
    /// The hidden cash pickups collected: the [`Bank`] flag indices set in
    /// career +0x8768 (0x8634A8, 256 bytes; set by 0x528380, cleared by a
    /// new career at 0x532A21, saved by 0x514685 and loaded by 0x52938E).
    pub banks: BTreeSet<u8>,
    /// The records table (profile +0xA0FC, 0x140 entries of 8 bytes;
    /// [`Record`]): the best lap of every race and the best drift score,
    /// kept by (track, lapped, backward). Saved with the profile; Quick
    /// Race's track list and the career's event list show them.
    pub records: Vec<Record>,
    /// The race totals after the records (profile +0xA00..+0xA50, saved
    /// with them; see [`RaceTotals`]).
    pub totals: RaceTotals,
}

impl Profile {
    /// A new career (0x532840): cash from the bonus codes, five locked
    /// car slots with the first opened, stage 0 (the D-Day intro).
    pub fn new(name: &str) -> Profile {
        let mut slots = vec![CarSlot::default(); CAR_SLOTS];
        slots[0].unlocked = true;
        Profile {
            name: name.into(),
            cash: start_cash(),
            car: String::new(),
            stage: 0,
            completed: BTreeSet::new(),
            filed: BTreeSet::new(),
            reputation: 0,
            best: HashMap::new(),
            sponsors: HashMap::new(),
            open: Vec::new(),
            milestones: BTreeSet::new(),
            offered: BTreeSet::new(),
            challenges_done: BTreeSet::new(),
            hints: crate::frontend::dday::Hints::default(),
            visited: BTreeSet::new(),
            spotted: BTreeSet::new(),
            tried: BTreeMap::new(),
            entered: BTreeSet::new(),
            visit_seq: Vec::new(),
            owned: BTreeSet::new(),
            installed: BTreeMap::new(),
            stage_rep: BTreeMap::new(),
            outrun_wins: BTreeMap::new(),
            stage_done: BTreeSet::new(),
            outrun_timer: 30.0,
            perf: BTreeSet::new(),
            perf_owned: BTreeSet::new(),
            perf_uniques: BTreeSet::new(),
            brands: Vec::new(),
            tuning: Default::default(),
            slots,
            slot: 0,
            map_hidden: [false; 14],
            intro: None,
            challenge_running: false,
            scratch: false,
            shop_ride: None,
            menu_car: None,
            menu_car_id: None,
            my_cars: vec![None; MY_CARS],
            records: vec![Record::default(); RECORD_SLOTS],
            totals: RaceTotals::default(),
            inbox: Default::default(),
            drive_time: 0.0,
            races_entered: 0,
            spent: 0,
            record_levels: None,
            trigger_counters: Default::default(),
            explore_set: false,
            intro_races: false,
            intro_closed: false,
            intro_played: false,
            explore_pos: None,
            banks: BTreeSet::new(),
        }
    }

    /// The city start's place for the car: the saved one when its flag is
    /// set (0x5404B0 / 0x540790 read 0x863484 = career +0x8744).
    pub fn explore_at(&self) -> Option<(Vec3, f32)> {
        self.explore_pos.filter(|_| self.explore_set)
    }

    /// Keep the player's free-roam place, the flag set (0x500AB0,
    /// 0x513B30, 0x5139C0: car +0x60 / +0x64 / +0x68 and heading +0x40C).
    pub fn keep_explore_at(&mut self, at: (Vec3, f32)) {
        self.explore_set = true;
        self.explore_pos = Some(at);
    }

    /// 0x5165E0 (**verified**, dis 0x5165E0-0x516612): the first free My
    /// Cars record (garage +0x54C + 0x434·i zero, i < 0x14). None: all 20
    /// are used, and Customize's Accept (0x4FC308) or Quick Race's
    /// Customize on a stock car (0x4FCB07) shows the "over the limit"
    /// dialog (0x91921A8D) instead.
    pub fn my_cars_free(&self) -> Option<usize> {
        self.my_cars.iter().take(MY_CARS).position(Option::is_none)
    }
}

/// Whether the bank is short of a price, as 0x563D10 tests it at
/// 0x56420B-0x56421F (**verified**): the cash [0x861E74] against the price
/// (0x514280), a signed compare (`jge` buys), so strictly less is short
/// and equal buys.
pub fn short_of_bank(cash: u32, price: i32) -> bool {
    (cash as i32) < price
}

#[cfg(test)]
mod b5_tests {
    use super::*;

    #[test]
    fn twenty_used_my_cars_records_leave_none_free() {
        let mut p = Profile::new("T");
        assert_eq!(p.my_cars_free(), Some(0));
        for k in 0..MY_CARS - 1 {
            p.my_cars[k] = Some(MyCar::default());
        }
        assert_eq!(p.my_cars_free(), Some(MY_CARS - 1), "19 used: the last free");
        p.my_cars[MY_CARS - 1] = Some(MyCar::default());
        assert_eq!(p.my_cars_free(), None, "20 used: over the limit");
        p.my_cars[3] = None;
        assert_eq!(p.my_cars_free(), Some(3), "the first free one");
    }

    #[test]
    fn the_bank_is_short_only_strictly_below_the_price() {
        assert!(short_of_bank(999, 1000));
        assert!(!short_of_bank(1000, 1000), "equal buys (jge)");
        assert!(!short_of_bank(1001, 1000));
        // Signed as natively: a bank above 2^31 reads negative.
        assert!(short_of_bank(0x8000_0000, 1));
    }
}
