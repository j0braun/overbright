//! The career tables (chunk 0x80034A10) and their loader.

use super::*;

pub struct CareerData {
    pub events: Vec<Event>,
    pub tracks: HashMap<u16, Track>,
    /// Car unlocks (0x34A1F, 12-byte records): car type hash and the
    /// event unlock hash that frees it. The record holds two events (+4,
    /// +8); 0x513C50 reads +8 because 0x570A40 (the static 6 at 0x7FA73C)
    /// is non-zero. 0 frees the car from the start.
    pub unlocks: Vec<(u32, u32)>,
    /// Car type name → manufacturer ("FOCUS" → "FORD").
    pub makes: HashMap<String, String>,
    /// The stage definitions (0x34A18), by stage number.
    pub stages: Vec<StageDef>,
    /// The sponsors (0x34A19), by hash.
    pub sponsors: HashMap<u32, Sponsor>,
    pub brands: Vec<Brand>,
    /// The stage milestones (0x34A16).
    pub milestones: Vec<Milestone>,
    /// The car types that are SUVs (car type +0x88A).
    pub suvs: std::collections::HashSet<String>,
    /// The part cost table (0x34A1C): visual points, prices, unlocks.
    pub costs: crate::formats::parts::CostTable,
    /// The performance packages (0x34A15).
    pub packages: Vec<crate::performance::Package>,
    /// The shops (0x34A12).
    pub shops: Vec<Shop>,
    /// The ranges the shops' stat bars normalise by (0x34A32).
    pub stat_ranges: crate::performance::StatRanges,
    /// What the fitted performance parts open to tuning (0x34A1A).
    pub tuning: Vec<crate::tuning::Unlock>,
    /// The SMS messages (0x34A17).
    pub messages: Vec<crate::frontend::sms::Message>,
    /// The magazine / unique-part message triggers (0x34A1B).
    pub mag_triggers: Vec<crate::frontend::sms::MagTrigger>,
    /// The hidden cash pickups (0x34A1E, see [`Bank`]).
    pub banks: Vec<Bank>,
}

/// A hidden cash pickup ("bank"; career chunk 0x34A1E, 12-byte records at
/// 0x838430, count 0x838434, loaded by 0x512D82). **Verified** (0x5141B0,
/// 0x513890, 0x5283D0, 0x528380): +0x00 (u16) the cash, +0x02 what makes
/// it available (1: the career stage, 0x862838 / career +0x7AF8, above
/// +0x04; 2: the visit-table entry +0x04 in state 1, 0x500B80, an event
/// won; any other: always), +0x03 its collected flag's index in career
/// +0x8768 (0x8634A8, 256 bytes, saved), +0x08 the name hash of its
/// kind-0x13 route trigger.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bank {
    pub cash: u16,
    pub condition: u8,
    pub flag: u8,
    pub value: u32,
    pub trigger: u32,
}

impl Bank {
    pub fn parse(body: &[u8]) -> Vec<Bank> {
        body.chunks_exact(12)
            .map(|r| Bank {
                cash: read_u16(r, 0),
                condition: r[2],
                flag: r[3],
                value: read_u32(r, 4),
                trigger: read_u32(r, 8),
            })
            .collect()
    }
}

/// A brand (career chunk 0x34A14, 0x44 bytes, 0x838400): +0x00 its name
/// (DECAL_ICON_<name>, 0x4FFF40), +0x20 the name shown (0x4FFF00), +0x40
/// its hash.
#[derive(Clone, Debug, Default)]
pub struct Brand {
    pub name: String,
    pub display: String,
    pub hash: u32,
}

/// A sponsor (career chunk 0x34A19, 0x10 bytes; read by the sponsorship
/// screen 0x495D30): +0x00 name (string pool), +0x02 sponsor-race purse,
/// +0x04 four required race types (counted by 0x5137C0), +0x08 hash (the
/// stage definitions list these), +0x0C signing bonus.
#[derive(Clone, Debug, Default)]
pub struct Sponsor {
    pub name: String,
    pub purse: u16,
    pub races: [u8; 4],
    pub hash: u32,
    pub bonus: i16,
    /// +0x0E: the average reputation points a race the stage's wins must
    /// have earned before it is offered (0x500670).
    pub reputation: i16,
}

/// A career stage (career chunk 0x34A18, 0x50 bytes each): +0x00 stage,
/// +0x01 sponsor count, +0x08 five sponsor brand hashes, +0x28 the hash of
/// the event that ends the stage (its last URL race; 0 for stage 1),
/// +0x30.. race counts by type, +0x40 wins required. Winning the final
/// event posts the stage-complete message (0x5242D0, message 7).
#[derive(Clone, Debug, Default)]
pub struct StageDef {
    pub stage: u8,
    /// +0x02 (i16): an outrun's bank, paid for a win and taken for a loss
    /// (0x541330; shown as "Bank Reward:", 0x4C7F00 → 0x500C60): 100 in
    /// stages 0–3, 250 in 4, 400 in 5.
    pub outrun_bank: i16,
    /// +0x04 (i16): what a photo shoot pays per point of the car's visual
    /// rating (0x541220: career stage record +0x100 is this record).
    pub cover_cash: i16,
    pub sponsors: Vec<u32>,
    pub final_event: u32,
    pub required: i8,
    /// +0x1C: per sponsor, the normal wins in the stage before it is
    /// offered (0x500610).
    pub sponsor_wins: [i16; 5],
    /// How many events of each normal slot are open at once (0x527AB0:
    /// slots 0..4 read +0x30, +0x31, +0x33, +0x34, +0x32), and SUV events
    /// (+0x37).
    pub caps: [i8; 6],
    /// The most always-open events open at once (+0x41; −1 no limit).
    pub always_cap: i8,
    /// The opponents' skill range over the stage (0x5132E0): +0x42 the
    /// wins it spreads over, +0x44 the skill at none, +0x48 at all.
    pub skill_wins: i16,
    pub skill: (f32, f32),
}

impl StageDef {
    /// One 0x50-byte record. The sponsor count (+0x01) is clamped to the
    /// five hash slots +0x08…+0x1B (and `sponsor_wins`' five entries): a
    /// damaged or modified count no longer reads past the record.
    pub(super) fn parse(r: &[u8]) -> StageDef {
        StageDef {
            stage: r[0],
            outrun_bank: read_u16(r, 2) as i16,
            cover_cash: read_u16(r, 4) as i16,
            sponsors: (0..(r[1] as usize).min(5)).map(|i| read_u32(r, 8 + i * 4)).collect(),
            final_event: read_u32(r, 0x28),
            required: r[0x40] as i8,
            sponsor_wins: std::array::from_fn(|i| read_u16(r, 0x1C + i * 2) as i16),
            caps: [0x30, 0x31, 0x33, 0x34, 0x32, 0x37].map(|o| r[o] as i8),
            always_cap: r[0x41] as i8,
            skill_wins: read_u16(r, 0x42) as i16,
            skill: (read_f32(r, 0x44), read_f32(r, 0x48)),
        }
    }
}

/// A shop (career chunk 0x34A12, 0xA0 bytes): +0x00 name
/// ("CC_BODYSHOP_1"), +0x20 its scene, +0x38 its hash (the free-roam
/// marker of the same name places it), +0x40 its scene file, +0x50 its
/// kind, +0x51 announced when it opens (0x512FF0), +0x74 and +0x9C what
/// opens it (0x512870: 0 nothing, 1 an event won, 2 URL, 3 normal, 4
/// sponsor wins in the stage ≥ +0x74), +0x9D the stage it opens in.
#[derive(Clone, Debug)]
pub struct Shop {
    pub name: String,
    pub hash: u32,
    /// +0x40: its scene file; GarageMain (0x4EAE37) names the cars' light
    /// setup after it, up to the first '.'.
    pub scene_file: String,
    /// +0x20: the movie its first entry plays (0x499640: when not empty
    /// and the shop's visit-table entry is not in state 5, 0x500460;
    /// queued by 0x557E00 on IG_PlayMovie). Empty for most shops.
    pub movie: String,
    /// 0 garage, 1 graphics, 2 body, 3 performance, 4 car lot, 5 car
    /// specialties, 6 unique parts.
    pub kind: u8,
    pub announced: bool,
    pub requirement: (u8, u32),
    pub stage: u8,
    /// +0x3C: the world trigger that stands for it (0x533320).
    pub trigger: u32,
}

impl Shop {
    /// The engage dialog's title (0x4FFEA0): by kind, Megalow its own.
    pub fn title_label(&self) -> u32 {
        if self.hash == 0x7FC8_D0D0 {
            return 0x6AA6_54C5;
        }
        match self.kind {
            0 => 0x7CE2_1662, // "Garage"
            1 => 0xA5F0_4BD7, // "Graphics Shop"
            2 => 0x0B92_53E9, // "Body Shop"
            3 => 0xAF25_8EE8, // "Performance Shop"
            4 => 0x4DC0_A646, // "Car Lot"
            5 => 0xA4E5_EE6C, // "Car Specialties Shop"
            _ => 0,
        }
    }

    /// The minimap icon (table 0x7F4348: shops at 6 + kind, the garage
    /// its own).
    pub fn icon(&self) -> &'static str {
        match self.kind {
            0 => "CRIBINDICATOR_",
            1 => "PAINTSHOPINDICATOR_",
            3 => "PERFORMANCESHOPINDICATOR_",
            4 => "CARLOTINDICATOR_",
            5 => "ICESHOPINDICATOR_",
            _ => "PARTSSHOPINDICATOR_",
        }
    }
}

/// A stage milestone (career chunk 0x34A16, 0x40 bytes): +0x00 its name
/// ("STAGE_2_DVD_1", "STAGE_3_SHOW_1_PICTURE"), +0x20 its hash, +0x25 the
/// stage, +0x35 set when the stage's last URL race needs it (0x513490: the
/// contract's media requirement).
#[derive(Clone, Debug)]
pub struct Milestone {
    pub name: String,
    pub hash: u32,
    pub stage: u8,
    pub required: bool,
    /// +0x24: 1 / 3 magazine, 2 / 4 DVD.
    pub kind: u8,
    /// +0x28: the cover; `<cover>_TITLE` its title ("Bayview's Best").
    pub cover: u32,
    /// +0x2C: the photo-shoot spot (a route trigger, see [`trigger_spot`]).
    pub spot: u32,
    /// +0x3C: the visual rating a DVD asks for (0x5331D0).
    pub rating: f32,
    /// +0x34: how it is offered — 0 on entering any shop, 1 on entering
    /// the shop whose +0x38 is `offer_at` (0x533760, from 0x54CB00); 2 on
    /// winning the event whose +8 is `offer_at` once its state record's +6
    /// is set (0x5337E0, from the win routine 0x541430).
    pub offer: u8,
    /// +0x38: the shop or event hash `offer` names.
    pub offer_at: u32,
    /// +0x26 (i16): the cash its photo shoot pays (0x541220), before the
    /// stage's per-rating share.
    pub cash: i16,
}

impl Milestone {
    pub fn is_dvd(&self) -> bool {
        matches!(self.kind, 2 | 4)
    }
}

impl CareerData {
    /// The career data of `root`; also sets the game's region from its
    /// CARS folder ([`crate::region::init`]), which the car lists follow.
    pub fn load(root: &Path) -> Result<CareerData> {
        crate::region::init(root);
        let file = crate::game_fs::read(root, "GLOBAL/GlobalB.lzc")?;
        let data = crate::formats::jdlz::decompress(&file)?;
        let career = chunks(&data, 0).find(|c| c.id == 0x8003_4A10).context("no career chunk")?;
        let pool = career.child(0x34A1D).map_or(&[][..], |c| c.body);
        let string = |at: u32| -> String {
            let at = at as usize;
            if at >= pool.len() {
                return String::new();
            }
            let end = pool[at..].iter().position(|&b| b == 0).map_or(pool.len(), |e| at + e);
            String::from_utf8_lossy(&pool[at..end]).into_owned()
        };
        let events_chunk = career.child(0x34A11).context("no events")?;
        let events = events_chunk
            .body
            .chunks_exact(0x88)
            .enumerate()
            .map(|(index, r)| Event {
                index,
                name: string(read_u16(r, 0) as u32),
                movie: string(read_u16(r, 2) as u32),
                won_movie: string(read_u16(r, 4) as u32),
                kind: EventKind::from(r[0x0F]),
                category: match r[0x34] {
                    0 => Category::Scripted,
                    1 => Category::Sponsor,
                    2 => Category::Url,
                    3 => Category::Normal,
                    v => Category::Other(v),
                },
                stage: r[0x37],
                track: read_u16(r, 0x18),
                variant: r[0x1A],
                laps: r[0x1B],
                reward: read_u32(r, 0x30),
                marker: read_u32(r, 0x28),
                unlock: read_u32(r, 0x08),
                tier: r[0x0C],
                suv: r[0x0D] == 1,
                requirement: read_u32(r, 0x10),
                always: r[0x7F] != 0,
                traffic: r[0x36] as i8,
                throttle: r[0x7D] as i8,
                races: (0..(r[0x7E] as usize).min(4))
                    .map(|k| (read_u16(r, 0x18 + k * 4), r[0x1A + k * 4], r[0x1B + k * 4]))
                    .collect(),
                opponents: (0..(r[0x7C] as usize).min(5))
                    .map(|k| {
                        let o = &r[0x40 + k * 0xC..];
                        Opponent {
                            name: string(read_u16(o, 0) as u32),
                            mode: o[2],
                            skill: o[3],
                            car: read_u32(o, 4),
                            difficulty: [o[8], o[9], o[10]],
                            block: o[11] as i8,
                        }
                    })
                    .collect(),
                goals: (0..(r[0x35] as i8).clamp(0, 10) as usize)
                    .map(|k| (r[0x38 + k * 8], read_u32(r, 0x3C + k * 8)))
                    .collect(),
                sender: read_u32(r, 0x84),
            })
            .collect();
        let tracks_chunk = chunks(&data, 0).find(|c| c.id == 0x34201).context("no track table")?;
        let mut tracks = tracks_chunk
            .body
            .chunks_exact(0x128)
            .enumerate()
            .map(|(order, r)| Track {
                number: read_u16(r, 0x8A),
                order,
                hidden: r[0x88] != 0,
                icon: read_u32(r, 0x7C),
                drift_kind: read_u32(r, 0x80),
                reversible: r[0x86] == 1,
                name: read_cstr(r, 0, 0x20),
                path: read_cstr(r, 0x20, 0x20),
                region: read_cstr(r, 0x40, 8),
                flags: read_u32(r, 0x94),
                length: read_u32(r, 0x98),
                default_record: [f32::from_bits(read_u32(r, 0x9C)), f32::from_bits(read_u32(r, 0xA0))],
                map_left: read_f32(r, 0xAC),
                map_top: read_f32(r, 0xB0),
                map_size: read_f32(r, 0xB4),
                map_zoom: read_f32(r, 0xC0),
                map_window: [read_f32(r, 0x118), read_f32(r, 0x11C), read_f32(r, 0x120)],
                map_window_fixed: r[0x124] != 0,
                traffic: std::array::from_fn(|k| r[0xEC + k] as i8),
                traffic_with: std::array::from_fn(|k| read_f32(r, 0x108 + 4 * k)),
                start_kind: r[0xF6] as i8,
                start_gap: std::array::from_fn(|k| (r[0xF4 + k] == 0).then(|| read_f32(r, 0xF8 + 4 * k))),
                drag_fill: std::array::from_fn(|k| read_f32(r, 0x100 + 4 * k)),
            })
            .map(|t| (t.number, t))
            .collect();
        crate::frontend::map_pack::remember_native(&tracks);
        if let Some(pack) = crate::hd_ui_pack::active_root() {
            if let Err(error) = crate::frontend::map_pack::apply(pack, &mut tracks) {
                log::warn!("Ignoring map calibration: {error:#}");
            }
        }
        let unlocks = career
            .child(0x34A1F)
            .map(|c| c.body.chunks_exact(12).map(|r| (read_u32(r, 0), read_u32(r, 8))).collect())
            .unwrap_or_default();
        // Each car type's manufacturer (car record +0xC0, 0x610180): the
        // car lot's logo is CARSELECT_MANUFACTURER_<it> (0x511E60).
        let makes = crate::car::parse_car_types(&data)
            .map(|types| types.into_iter().map(|t| (t.name, t.manufacturer)).collect())
            .unwrap_or_default();
        let stages =
            career.child(0x34A18).map(|c| c.body.chunks_exact(0x50).map(StageDef::parse).collect()).unwrap_or_default();
        let brands = career
            .child(0x34A14)
            .map(|c| {
                c.body
                    .chunks_exact(0x44)
                    .map(|r| Brand {
                        name: read_cstr(r, 0, 0x20),
                        display: read_cstr(r, 0x20, 0x20),
                        hash: read_u32(r, 0x40),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let sponsors = career
            .child(0x34A19)
            .map(|c| {
                c.body
                    .chunks_exact(0x10)
                    .map(|r| Sponsor {
                        name: string(read_u16(r, 0) as u32),
                        purse: read_u16(r, 2),
                        races: [r[4], r[5], r[6], r[7]],
                        hash: read_u32(r, 8),
                        bonus: read_u16(r, 0xC) as i16,
                        reputation: read_u16(r, 0xE) as i16,
                    })
                    .map(|s| (s.hash, s))
                    .collect()
            })
            .unwrap_or_default();
        let milestones = career
            .child(0x34A16)
            .map(|c| {
                c.body
                    .chunks_exact(0x40)
                    .map(|r| Milestone {
                        name: read_cstr(r, 0, 0x20),
                        hash: read_u32(r, 0x20),
                        stage: r[0x25],
                        required: r[0x35] != 0,
                        kind: r[0x24],
                        cover: read_u32(r, 0x28),
                        spot: read_u32(r, 0x2C),
                        rating: read_f32(r, 0x3C),
                        offer: r[0x34],
                        offer_at: read_u32(r, 0x38),
                        cash: read_u16(r, 0x26) as i16,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let suvs = crate::car::parse_car_types(&data)
            .map(|types| types.into_iter().filter(|t| t.suv).map(|t| t.name).collect())
            .unwrap_or_default();
        let costs = career.child(0x34A1C).map(|c| crate::formats::parts::CostTable::parse(c.body)).unwrap_or_default();
        let shops = career
            .child(0x34A12)
            .map(|c| {
                c.body
                    .chunks_exact(0xA0)
                    .map(|r| Shop {
                        name: read_cstr(r, 0, 0x20),
                        hash: read_u32(r, 0x38),
                        scene_file: read_cstr(r, 0x40, 0x10),
                        movie: read_cstr(r, 0x20, 0x18),
                        kind: r[0x50],
                        announced: r[0x51] != 0,
                        requirement: (r[0x9C], read_u32(r, 0x74)),
                        stage: r[0x9D],
                        trigger: read_u32(r, 0x3C),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let packages = career.child(0x34A15).map(|c| crate::performance::parse(c.body)).unwrap_or_default();
        let mut stat_ranges = [0.0; 30];
        crate::formats::bundle::walk(chunks(&data, 0), &mut |c| {
            if c.id == 0x34A32 && c.body.len() >= 0x78 {
                stat_ranges = std::array::from_fn(|i| read_f32(c.body, i * 4));
            }
        });
        let tuning = career.child(0x34A1A).map(|c| crate::tuning::parse(c.body)).unwrap_or_default();
        let messages = career.child(0x34A17).map(|c| crate::frontend::sms::parse(c.body, &string)).unwrap_or_default();
        let mag_triggers =
            career.child(0x34A1B).map(|c| crate::frontend::sms::parse_triggers(c.body, &string)).unwrap_or_default();
        let banks = career.child(0x34A1E).map(|c| Bank::parse(c.body)).unwrap_or_default();
        Ok(CareerData {
            events,
            tracks,
            unlocks,
            makes,
            stages,
            sponsors,
            brands,
            milestones,
            suvs,
            costs,
            shops,
            packages,
            stat_ranges,
            tuning,
            messages,
            mag_triggers,
            banks,
        })
    }

    /// The events a career at `stage` can race: that stage's plain,
    /// sponsor and URL events whose route exists.
    /// A stage's events as the World Map lists them (0x4B1360): those of
    /// that stage, stage-0 events with stage 1, but not the D-Day intro
    /// events (hashes 0xDD60E402 / 0xDD60E403), and only on tracks the clone
    /// has a route for.
    pub fn stage_events(&self, root: &Path, stage: u8) -> Vec<&Event> {
        self.events
            .iter()
            .filter(|e| e.stage == stage || (stage == 1 && e.stage == 0))
            .filter(|e| e.unlock != 0xDD60_E402 && e.unlock != 0xDD60_E403)
            .filter(|e| self.tracks.get(&e.track).is_some_and(|t| route_path(root, t).exists()))
            .collect()
    }

    pub fn track_title(&self, number: u16) -> String {
        self.tracks.get(&number).map_or_else(|| format!("Track {number}"), |t| t.title().to_string())
    }
}
