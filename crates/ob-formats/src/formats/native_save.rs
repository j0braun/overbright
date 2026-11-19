//! The original's profile save (SPEED2.EXE's memory-card library on the
//! PC: one file per profile, `<LOCALAPPDATA>\NFS Underground 2\<NAME>\<NAME>`,
//! 0x4A9C40 builds the folder with SHGetFolderPathA(CSIDL_LOCAL_APPDATA |
//! CSIDL_FLAG_CREATE) and the name at 0x7F66A0).
//!
//! **Container** (**verified**: the reader 0x6FD3F0…0x6FD6E8 checks it, the
//! header constructor 0x6FC226 writes the magic, and files the original
//! wrote and read back were checked byte for byte): a 0x1C-byte header —
//! `+0x00` magic "20CM" (0x4D433032), `+0x04` the file's size, `+0x08` and
//! `+0x0C` the sizes of the two sections that follow, `+0x10` / `+0x14`
//! their checksums, `+0x18` the checksum of the header's first 0x18 bytes —
//! then section 1 (8 bytes: the block type 0x10D and the payload size,
//! written at 0x4D9C15 / 0x4D9CBF and checked by 0x4A9850) and section 2,
//! the payload. The checksum is 0x6FE173 (through 0x6FCD1C): a CRC-32 with
//! polynomial 0x04C11DB7 (table 0x8145E0) that loads the first four bytes
//! big-endian and inverted, shifts each further byte in from the right,
//! and returns the register inverted (0 for fewer than four bytes).
//!
//! **Payload** (0x52A450: 0x43E + 0xD254 = 0xD692 bytes, always; written by
//! 0x52A290 and read by 0x5344E0 in the same order, **verified** against
//! the live memory of a running original): the garage (0x5166B0 on
//! 0x83AD90), the career (0x514530 on 0x85AD40), the profile slot (0x83A9E0
//! fields), the 0x850080 object (0x515BF0) and the controller maps
//! (0x5B9430). Offsets below are payload offsets.

use anyhow::{Result, bail, ensure};

use super::reader::Reader;

/// "20CM" read as a little-endian dword.
pub const MAGIC: u32 = 0x4D43_3032;
/// The block type section 1 names (0x4A98F7 compares it).
pub const BLOCK_TYPE: u32 = 0x10D;
pub const HEADER_LEN: usize = 0x1C;
pub const PAYLOAD_LEN: usize = 0xD692;
pub const FILE_LEN: usize = HEADER_LEN + 8 + PAYLOAD_LEN;

// --- garage (0x83AD90, 0x5166B0) ---------------------------------------
// Every record kind in the garage is a car object with a vtable at +0 (not
// saved) and the same head (**verified**: the writers save from +4): `+4`
// a dword (0 in every save seen), `+8` its id hash (0x503510 / 0x503680
// prov: open formats.native_save.GARAGE_LIST
// match +8; a free entry has 0), `+0xC` an f32 (**open**: written when the
// car lot is visited, 0 before), `+0x10` set for a usable record (0x5162D0
// skips a record with +0x10 clear), `+0x14` the Quick Race filter bits (1
// Stock, 2 My Cars, 4 Career, 8 Sponsor, 0x10 Ranked; 0x5162D0 / 0x4E22E0).
// The 91 pointers at garage +0x9BD8 (not saved) index them all.
/// 48 stock car entries (garage + 0x1C·i, the head only: +4..+0x18 saved,
/// 24 bytes): `+0x14` (memory +0x18) their list index, −1 for a free one.
pub const GARAGE_LIST: usize = 0;
pub const LIST_LEN: usize = 0x18;
pub const LIST_MAX: usize = 48;
/// Within a saved head (memory +4): the id, the f32, the usable flag, the
/// filter bits.
pub const HEAD_ID: usize = 4;
pub const HEAD_F32: usize = 8;
pub const HEAD_USED: usize = 0xC;
pub const HEAD_FILTER: usize = 0x10;
/// Garage +0x540: the stock list's count (0x1F in every save seen).
pub const LIST_COUNT: usize = 0x480;
/// 20 "My Cars" records (garage +0x544, stride 0x434; 0x430 bytes saved
/// from +4): the head (5 dwords), the car record (memory +0x18, 0x418
/// bytes) and the dword at memory +0x430. A free one has id 0 (0x5165E0
/// takes the first). Made by the main menu's Customize (filter bit 2).
pub const GARAGE_CARS: usize = 0x484;
pub const GARAGE_CAR_LEN: usize = 0x430;
pub const GARAGE_CAR_MAX: usize = 20;
/// The five career car slots CAREER_SLOT_0..4 (garage +0x5954, stride
/// 0x7F8 in memory, 0x7F2 bytes here: 0x516620).
pub const SLOTS: usize = 0x5844;
pub const SLOT_LEN: usize = 0x7F2;
/// Within a slot: `+0x00` the head (memory +4..+0x14; +4 the slot's name
/// hash, bin_hash("CAREER_SLOT_<n>"), filter bits 4), `+0x14` the car
/// record (memory +0x18, 0x418 bytes), `+0x42C` a dword (memory +0x430),
/// `+0x430` locked (+0x7F4), `+0x431` has its own car (+0x7F5), `+0x432`
/// the 0x3C0 bytes 0x503F00 writes from memory +0x434.
pub const SLOT_ID: usize = 4;
pub const SLOT_RECORD: usize = 0x14;
pub const SLOT_LOCKED: usize = 0x430;
pub const SLOT_HAS_CAR: usize = 0x431;
pub const SLOT_EXTRA: usize = 0x432;
/// Within the extra block (0x503F00 remaps memory slot +0x434 = X…): the
/// owned-part bit sets (0x503D50 sets, 0x503DB0 tests: bit n of a slot's
/// set is the part that 0x61BA90 counts n parts after in its slot; 0x503B10
/// picks the set by slot, table 0x503CAC): X+0 one dword per slot (the
/// slots the table does not name), `+0x15C` the performance parts' bits
/// (0x503E10 / 0x530360), `+0x16C` (0x100 bytes) slots 0x1D / 0x1E (rims),
/// `+0x26C` slot 7, `+0x27C` slot 0xC, `+0x28C` (8 bytes) slot 0x20, then
/// the money spent on the car (memory X+0x3BC, 0x547747), `+0x298` (8
/// bytes, memory X+0x294) slot 9, `+0x2A0` the brand list (memory
/// X+0x2AC, 0x44 hashes, 0x503E10), `+0x3B0` (memory X+0x29C, 16 bytes)
/// slot 0xA2. Slots 0x3F…0x49 (paint and vinyl colours) have their own
/// test (0x503DB0's table 0x503DFC).
/// 0x503F00's remap of the slot's memory block X (+0x434) into the saved
/// extra block: (memory offset, length, saved offset).
pub const OWNED_REMAP: [(usize, usize, usize); 10] = [
    (0, 0x15C, 0),
    (0x15C, 0x10, 0x15C),
    (0x16C, 0x100, 0x16C),
    (0x26C, 0x10, 0x26C),
    (0x27C, 0x10, 0x27C),
    (0x28C, 8, 0x28C),
    (0x3BC, 4, 0x294),
    (0x294, 8, 0x298),
    (0x2AC, 0x110, 0x2A0),
    (0x29C, 0x10, 0x3B0),
];
pub const EXTRA_PERF_OWNED: usize = 0x15C;
pub const EXTRA_SPENT: usize = 0x294;
pub const EXTRA_BRANDS: usize = 0x2A0;
pub const BRANDS: usize = 0x44;
/// The car record (memory object +0x18; 0x503950 fills one from a ride):
/// `+0x00` 1 once filled, `+0x04` a dword (0 in every save seen), `+0x08`
/// the car type's bin hash, `+0x0C + 4·slot` the part in each of the 0xAA
/// part slots (0x503950: the part's two halves, 0 none), `+0x2B4` the
/// 0x94-byte upgrade state (ten category levels, `+0x28 + 4i` the
/// performance-unique flags, `+0x50 + id` the fitted performance parts),
/// `+0x348` the dyno's five tuning settings, 0x29 bytes each (object
/// +0x360; 0x516A60 fills them with 100 through 0x5A7940; 0x516AC0 copies
/// setting n to the ride's +0x398), in the dyno's mode order Circuit, Drag,
/// Drift, Street X, URL Race; 3 bytes of padding.
pub const REC_CAR: usize = 0x08;
pub const REC_PARTS: usize = 0x0C;
pub const PART_SLOTS: usize = 0xAA;
pub const REC_UPGRADE: usize = 0x2B4;
pub const UPGRADE_UNIQUES: usize = 0x28;
pub const UPGRADE_FITTED: usize = 0x50;
pub const REC_TUNING: usize = 0x348;
pub const TUNING_LEN: usize = 0x29;
pub const RECORD_LEN: usize = 0x418;
/// The career slot count (garage +0x812C, 5).
pub const SLOT_COUNT: usize = 0x7FFE;
/// Six more car records (garage +0x8130, stride 0x438; 0x434 bytes saved
/// from +4: the head, the car record, memory +0x430 and +0x434), filter
/// bits 0x10 (Ranked Cars) in every save seen.
pub const RANKED_CARS: usize = 0x8002;
pub const RANKED_CAR_LEN: usize = 0x434;
pub const RANKED_CAR_MAX: usize = 6;
/// Garage +0x9A80 (0 in every save seen; **open**).
pub const GARAGE_TAIL: usize = 0x993A;

// --- career (0x85AD40, 0x514530) -----------------------------------------
pub const CAREER: usize = 0x993E;
/// Eight stage records (career +0x72D8, 0x104 in memory, 0x101 here,
/// 0x500FA0): `+0x00` 35 normal wins, `+0x8C` four sponsor wins, `+0x9C`
/// ten URL wins (each the won event's hash, filed by 0x513390), `+0xDC`
/// outruns won, `+0xE0` reputation (f32) and `+0xE4` races it counts,
/// `+0xE8` five pointers saved as the bin hash of their name (the first
/// the signed sponsor), `+0xFC` unlocked, `+0xFD` the definition's stage.
pub const STAGES: usize = CAREER;
pub const STAGE_LEN: usize = 0x101;
pub const STAGE_COUNT: usize = 8;
pub const ST_NORMAL: usize = 0x00;
pub const ST_SPONSOR: usize = 0x8C;
pub const ST_URL: usize = 0x9C;
/// `+0xC4`: six dwords, the milestones whose photo shoot was done in the
/// stage (0x5410A0, from the reward 0x541220).
pub const ST_COVERS: usize = 0xC4;
pub const ST_OUTRUNS: usize = 0xDC;
pub const ST_REP: usize = 0xE0;
pub const ST_REP_COUNT: usize = 0xE4;
pub const ST_SPONSOR_SIGNED: usize = 0xE8;
pub const ST_UNLOCKED: usize = 0xFC;
const C2: usize = CAREER + STAGE_LEN * STAGE_COUNT;
/// Career +0x7134, the bank.
pub const CASH: usize = C2;
/// Career +0x8868 (0x8635A8): the explore pause menu's Jump to Garage.
pub const JUMP_TO_GARAGE: usize = C2 + 4;
/// Career +0x872C: races entered.
pub const RACES_ENTERED: usize = C2 + 5;
/// Career +0x873E / +0x873F: Rachel's intro races closed / revealed.
pub const INTRO_CLOSED: usize = C2 + 9;
pub const INTRO_RACES: usize = C2 + 10;
/// Career +0x7AF8: the stage.
pub const STAGE: usize = C2 + 0xB;
/// Career +0x7AFC / +0x7B00: the visit table, 0x180 entries of 8 bytes
/// (hash, +4 visited, +6 state: 1 won, 0x500B80).
pub const VISIT_COUNT: usize = C2 + 0xF;
pub const VISITS: usize = C2 + 0x13;
pub const VISIT_MAX: usize = 0x180;
/// Career +0x873C (0x86347C): the career's intro movie has played (set by
/// 0x54AF90's mode 6, cleared by a new career; UI_StartCareer's Resume
/// test, 0x4AF165; `Profile::intro_played`).
pub const UNDER_WAY: usize = C2 + 0xC13;
/// Career +0x873D (0x86347D): the player has a car of their own.
pub const HAS_OWN_CAR: usize = C2 + 0xC14;
/// Career +0x8740: the current slot's name hash.
pub const CURRENT_SLOT: usize = C2 + 0xC15;
/// Career +0x8744 / +0x8750 / +0x8760: the saved place's flag, position
/// (x, y, z, w) and heading (u16, a full turn = 0x10000).
pub const PLACE_SET: usize = C2 + 0xC19;
pub const PLACE_POS: usize = C2 + 0xC1A;
pub const PLACE_HEADING: usize = C2 + 0xC2A;
/// Career +0x8764: the free-roam driving time (s).
pub const DRIVE_TIME: usize = C2 + 0xC2C;
/// Career +0x8768 (0x8634A8): the hidden cash pickups' collected flags,
/// one byte per 0x34A1E record's +0x03 (0x514685 copies 0x40 dwords).
pub const BANK_FLAGS: usize = C2 + 0xC30;
pub const BANK_FLAGS_LEN: usize = 0x100;
/// The SMS inbox (career +0x5D20, 0x5014F0): 256 entries of 0x14 bytes
/// (the message name's bin hash, four flag bytes, body / argument /
/// subject labels), `+0x1400` the count, `+0x1404` the unread count,
/// `+0x1408` the new-mail flag.
pub const INBOX: usize = C2 + 0xD30;
pub const INBOX_LEN: usize = 0x14;
pub const INBOX_MAX: usize = 256;
pub const INBOX_COUNT: usize = INBOX + 0x1400;
pub const INBOX_UNREAD: usize = INBOX + 0x1404;
pub const INBOX_NEW: usize = INBOX + 0x1408;
/// Career +0x8869 … +0x8878 (0x8635A9): the World Map legend's show
/// flags (1 shown), 16 bytes right after the inbox (0x5146A9). 0x496390
/// (group, hide) writes byte [`LEGEND_BYTE`]\[group\] (group 13 also
/// bytes 6 and 8); 0x4CA7B5 tests them.
pub const LEGEND_SHOWN: usize = INBOX + 0x1409;
/// 0x496390's jump table 0x49648C: the byte each legend group (0..13)
/// flips.
pub const LEGEND_BYTE: [usize; 14] = [0, 1, 2, 3, 4, 5, 7, 9, 0xA, 0xC, 0xD, 0xB, 0xE, 0xF];
/// Career +0x887C (0x8635BC, 0x190 bytes, 0x531810 builds it): Rachel's
/// hints object (0x5243E0 / 0x551080; `Profile::hints` in the clone): `+9` /
/// `+10` the once flags of the car lot's and the garage's hints, `+0xA8`
/// eight timed slots {ticks, kind, value} (0x5111F0); the whole layout
/// is in docs/formats.md, **Rachel's speech** (`dday::Hints::to_bytes`).
pub const HINTS: usize = LEGEND_SHOWN + 0x10;
pub const HINTS_LEN: usize = 0x190;
/// The hints' dwords the career loader 0x529240 zeroes after the copy
/// (career +0x8898, +0x8984, +0x898C, +0x8994; **verified**).
pub const HINTS_ZEROED_ON_LOAD: [usize; 4] = [0x1C, 0x108, 0x110, 0x118];
/// Career +0x8738: the D-Day event under way, saved as its hash.
pub const INTRO_EVENT: usize = HINTS + HINTS_LEN;

// --- profile (0x83A9E0, 0x52A290's own copies) ----------------------------
pub const PROFILE: usize = CAREER + 0x2AE5;
/// Profile +0x2D8 (0x83ACB8): the EA Trax playlist, 27 × (index, state).
pub const TRAX: usize = PROFILE;
/// Profile +0x48 … +0x84 (settings block +0x24 …): the seven volumes, then
/// the audio mode at settings +0x4C.
pub const VOLUMES: usize = PROFILE + 0xD8;
pub const AUDIO_MODE: usize = PROFILE + 0x100;
/// Profile +0x38 … +0x48 (settings +0x14 …): rearview, …, metric, race map,
/// explore map, then the jump camera (f32) at settings +0x1C.
pub const SETTINGS_14: usize = PROFILE + 0x114;
/// Profile +0x84 (settings +0x60): the two players' settings, 0x28 each.
pub const PLAYERS: usize = PROFILE + 0x124;
/// Profile +0x28 … +0x38 (settings +4 …, 0x5116E0's defaults): +4 an f32
/// (0.5), +8 and +0xC dwords (0), +0x10 a byte (0; when set, the race HUD
/// constructors 0x4AC76B / 0x4F8C58 load Chyron_IG.fng). What edits them
/// is **not traced**.
pub const SETTINGS_4: usize = PROFILE + 0x174;
/// Settings +0x20 (0x83AA24, f32): the event cameras' second gate (kinds
/// 1..5 start only while it is not 0: 0x6023DA, and the kind-1 timer
/// 0x60293D / the keys 0x7F..0x81 at 0x602FB9, 0x603103, 0x603275). Its
/// writers go through the settings block's base: the constructor 0x5116E0
/// stores 127.5 (0x42FF0000) at +0x1C and +0x20 (0x51170B / 0x51170E,
/// `ecx` = profile +0x24 from 0x5344B6), and the profile load copies the
/// saved value; no option row writes it (**verified**). Live under Wine
/// it read 127.5 before and after loading the finished-career save (whose Jump Camera,
/// +0x1C, read 0.0 once loaded), and every save at hand holds 127.5.
pub const SETTINGS_20: usize = SETTINGS_14 + 0xC;
/// The value 0x5116E0 gives settings +0x1C and +0x20.
pub const SETTINGS_20_DEFAULT: f32 = 127.5;
/// Settings +0x54 (0x83AA58): the EA Trax play mode, 0 Sequential, 1
/// Random (0x524130; toggled by 0x565DA0, the jukebox's update).
pub const TRAX_MODE: usize = VOLUMES + 0x30;
/// Profile +0xD4 (0x83AAB4): the race record (+0x10 + 20 × type, the Quick
/// Race settings).
pub const RACE_RECORD: usize = PROFILE + 0x184;
/// Profile +0x250: career difficulty.
pub const DIFFICULTY: usize = PROFILE + 0x300;
/// Profile +0x254 (0x83AC34): the number Style_P1 shows after its label
/// (0x4EAAB1 "%s %d", 0x4C1BB2 per player); its writer is **not traced**.
pub const STYLE_NUMBER: usize = PROFILE + 0x304;
/// 0x844AD9 / 0x844AD8 (profile +0xA0F9 / +0xA0F8): autosave and save
/// prompts.
pub const AUTOSAVE: usize = PROFILE + 0x308;
pub const SAVE_PROMPTS: usize = PROFILE + 0x309;
/// The controller maps (0x5B9430 / 0x5B9580): six blocks of 0xB5 bytes,
/// one per input record up to the record count 0x870764, zeros past it
/// (`pad::PadMap::to_block`); the end of the payload.
pub const CONTROLLER_MAPS: usize = 0xD254;
/// GUID_SysKeyboard as the keyboard's block holds it (zeros after it).
pub const GUID_SYS_KEYBOARD: [u8; 16] = [0x61, 0x2B, 0x1D, 0x6F, 0xA0, 0xD5, 0xCF, 0x11, 0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00];
/// Profile +0xA0F4 (0x844AD4): a dword (0 in every save seen; the only
/// code use is 0x526AEF's end-of-table compare) **open**.
pub const PROFILE_A0F4: usize = PROFILE + 0x30A;
/// Profile +0xA0FC (0x844ADC, 0xA50 bytes): the records and statistics.
/// `+0x000` 0x140 records of 8 bytes: i16 track (0 free), i8 `+2` and i8
/// `+3` keys, i32 value (0x502A50 finds (track, +3, +2); 0x502DF0 adds or
/// betters one in the first free entry; 0x502AA0 the lowest value of a
/// (track, +3), 0x502C50 the highest; with none, the track's defaults
/// 0x5D3E50 +0x9C / +0xA0 / +0xA4). Then the totals 0x515990 adds after a
/// race: `+0xA00` an f32 sum (racer +0xF8), `+0xA08` a count of firsts
/// (racer +0 = 1), `+0xA0C` races counted (1, 2, 4 in the saves after one,
/// two, four of Rachel's races), `+0xA18 + 12·type` three dwords per race
/// type, `+0xA44` / `+0xA48` / `+0xA4C`. The keys: `+2` the race's laps
/// flag 0x89E7BC, `+3` run backward 0x89E7A4 (0x551440 → 0x515990); the
/// value the best lap in 1/4000 s, or the drift score (`career::Record`).
pub const RECORDS: usize = PROFILE + 0x30E;
pub const RECORDS_LEN: usize = 0xA50;
pub const RECORD_MAX: usize = 0x140;
pub const RECORD_RACES: usize = RECORDS + 0xA0C;
/// Profile +0x258 (0x83AC38, 0x80 bytes, 0 in every save seen; **open**).
pub const PROFILE_258: usize = RECORDS + RECORDS_LEN;
/// Profile +0: the name (32 bytes, NUL padded).
pub const NAME: usize = PROFILE + 0xDDE;
pub const NAME_LEN: usize = 0x20;

// --- 0x850080 (0x515BF0, 0x33 bytes) -------------------------------------
/// Two zero dwords (written as 0, not from the object), then +0x49, +0x4A,
/// +0x4B (bytes; 1, 1, 0 in every save seen), +0x58 and +0x80 (five dwords
/// each, 0). The object's role is **not traced**.
pub const OBJ_850080: usize = NAME + NAME_LEN;

// --- controller maps (0x5B9430 / 0x5B9580) -------------------------------
/// Six blocks of 0xB5 bytes, one per input device (0x874C40 + 0x370·k) up
/// to the device count 0x870764, zeros past it: `+0x00` the device's
/// DirectInput instance GUID (0x5B8120: GetDeviceInfo's guidInstance; the
/// loader matches blocks to devices by it, so the order does not matter;
/// zeros when device +0x230 is −1), `+0x10` device +0x344 (four axis ids),
/// `+0x20` +0x354 (four axis values: signs / ranges), `+0x30` +0x364 (four
/// set flags), `+0x34` +0x368 (the combined-pedals flag; 0x5B8260 assigns
/// an axis to a role by +0x33C), `+0x35` device +0x18 (32 dwords: the
/// action of each of 32 buttons, 0x5B8470 → 0x5B8545). The keyboard
/// (GUID_SysKeyboard 6F1D2B61-D5A0-11CF-BFC7-444553540000) has all zeros
/// past its GUID in every save seen.
pub const CONTROLS: usize = OBJ_850080 + 0x33;
pub const CONTROL_LEN: usize = 0xB5;
pub const CONTROL_DEVICES: usize = 6;

/// 0x6FE173.
pub fn checksum(data: &[u8]) -> u32 {
    if data.len() < 4 {
        return 0;
    }
    let table = table();
    let mut r = !u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    for &b in &data[4..] {
        r = (r << 8 | b as u32) ^ table[(r >> 24) as usize];
    }
    !r
}

fn table() -> [u32; 256] {
    std::array::from_fn(|i| {
        let mut c = (i as u32) << 24;
        for _ in 0..8 {
            c = if c & 0x8000_0000 != 0 { c << 1 ^ 0x04C1_1DB7 } else { c << 1 };
        }
        c
    })
}

/// A memory-card file around `payload`: the header, section 1 (the block
/// type 0x10D and the payload's size) and the payload (see the module
/// notes). Profiles and the cover pictures share it.
pub fn container(payload: &[u8]) -> Vec<u8> {
    let mut s1 = Vec::with_capacity(8);
    s1.extend_from_slice(&BLOCK_TYPE.to_le_bytes());
    s1.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    let mut out = Vec::with_capacity(HEADER_LEN + 8 + payload.len());
    for v in [MAGIC, (HEADER_LEN + 8 + payload.len()) as u32, 8, payload.len() as u32, checksum(&s1), checksum(payload)] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    let h = checksum(&out);
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&s1);
    out.extend_from_slice(payload);
    out
}

/// A saved magazine / DVD cover (**verified** against the original's
/// files, e.g. a community save's `<NAME> DVD 1/<NAME> DVD 1`, 1,048,612
/// bytes): the same container (block type 0x10D) around 0x100000 bytes,
/// the 512 × 512 "ScreenCap" texture the cover capture 0x5C3130 fills
/// (the 320 × 432 shot stretched over it), rows top down, 4 bytes a
/// pixel in B, G, R, A order (D3DFMT_A8R8G8B8 in memory). The alpha bytes
/// are the back buffer's, copied with the colour (0x5C3130's StretchRect
/// into an A8R8G8B8 target), so they vary; the view draws the picture
/// opaque.
pub const PICTURE_SIZE: u32 = 512;
pub const PICTURE_PAYLOAD: usize = (PICTURE_SIZE * PICTURE_SIZE * 4) as usize;
pub const PICTURE_FILE_LEN: usize = HEADER_LEN + 8 + PICTURE_PAYLOAD;

/// The cover file's bytes from a top-down RGBA image of any size (the
/// capture rectangle), stretched to 512 × 512 and stored as BGRA with its
/// alpha. 0x5C3130 (**verified** in the code; the filter's result
/// **likely**): CreateRenderTarget(512, 512, A8R8G8B8, no multisampling,
/// lockable), StretchRect(back buffer, rect, target, NULL,
/// D3DTEXF_NONE), LockRect(READONLY): no filtering, which D3D9 drivers
/// carry out as point sampling — target pixel x takes source pixel
/// floor((x + 0.5) · w / 512) — and the back buffer's alpha goes with it.
pub fn picture(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let n = PICTURE_SIZE as usize;
    let mut out = vec![0u8; PICTURE_PAYLOAD];
    // In usize: `width * height * 4` in u32 wrapped for big sizes and let a
    // short buffer through.
    if width == 0 || height == 0 || rgba.len() < width as usize * height as usize * 4 {
        return container(&out);
    }
    let (w, h) = (width as usize, height as usize);
    let point = |i: usize, size: usize| ((i * 2 + 1) * size / (n * 2)).min(size - 1);
    for y in 0..n {
        let sy = point(y, h);
        for x in 0..n {
            let sx = point(x, w);
            let src = &rgba[(sy * w + sx) * 4..(sy * w + sx) * 4 + 4];
            out[(y * n + x) * 4..(y * n + x) * 4 + 4].copy_from_slice(&[src[2], src[1], src[0], src[3]]);
        }
    }
    container(&out)
}

/// A saved cover's picture as top-down RGBA, 512 × 512 (the capture
/// texture as stored; UI_MagazineView shows it stretched over MagImage):
/// the reverse of [`picture`]. Checked as the profile reader checks (size,
/// magic, header checksum, block type; the sections' checksums do not
/// reject). The alpha bytes are set to 255 (**inferred**: the view draws
/// the picture opaque, as the original's covers with random alpha show).
pub fn read_picture(file: &[u8]) -> Result<Vec<u8>> {
    ensure!(file.len() == PICTURE_FILE_LEN, "picture of {} bytes, the game writes {PICTURE_FILE_LEN}", file.len());
    let r = Reader::new(file);
    let u = |o: usize| r.u32_at(o);
    ensure!(u(0)? == MAGIC && u(4)? as usize == file.len(), "not a picture save");
    ensure!(checksum(r.bytes_at(0, 0x18)?) == u(0x18)?, "header checksum");
    ensure!(u(HEADER_LEN)? == BLOCK_TYPE && u(HEADER_LEN + 4)? as usize == PICTURE_PAYLOAD, "block type / size");
    let mut out = r.bytes_at(HEADER_LEN + 8, file.len() - (HEADER_LEN + 8))?.to_vec();
    for px in out.chunks_exact_mut(4) {
        px.swap(0, 2);
        px[3] = 255;
    }
    Ok(out)
}

/// One profile file's payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeSave {
    pub payload: Vec<u8>,
}

impl NativeSave {
    /// A new profile named `name` from `profile`, the original's own new
    /// profile (created at boot with the default name "NAME", the payload
    /// it writes before any career; the career writes it from its rules and
    /// the player's game files, `frontend::career::fresh_save`).
    pub fn fresh(profile: &NativeSave, name: &str) -> NativeSave {
        let mut s = profile.clone();
        s.set_name(name);
        s
    }

    pub fn parse(file: &[u8]) -> Result<NativeSave> {
        ensure!(file.len() >= HEADER_LEN + 8, "save too short ({} bytes)", file.len());
        let mut r = Reader::new(file);
        let u = |o: usize| r.u32_at(o);
        ensure!(u(0)? == MAGIC, "not a profile save (magic {:#010x})", u(0)?);
        ensure!(u(4)? as usize == file.len(), "size {} but the file has {}", u(4)?, file.len());
        let (n1, n2) = (u(8)? as usize, u(0xC)? as usize);
        ensure!(n1 == 8 && HEADER_LEN.checked_add(n1).and_then(|n| n.checked_add(n2)) == Some(file.len()), "section sizes {n1} {n2}");
        ensure!(checksum(r.bytes_at(0, 0x18)?) == u(0x18)?, "header checksum");
        r.seek(HEADER_LEN)?;
        let mut s1 = r.sub(8)?;
        let s2 = r.rest();
        // The sections' checksums are computed but do not reject the file:
        // the reader 0x6FD3F0 sets error 8 on a mismatch (0x6FD6E8 /
        // 0x6FD718) and then jumps to 0x6FD73C, which clears the error
        // (**verified**; the original under Wine loads community saves
        // edited without fixing the payload checksum, e.g. a community save: "Load
        // complete.", its bank 999999 in memory). Only the size, the magic
        // and the header checksum reject one; a save rewrites them all.
        let t = s1.u32()?;
        let size = s1.u32()? as usize;
        if t != BLOCK_TYPE || size != n2 {
            bail!("block type {t:#x}, size {size}");
        }
        ensure!(n2 == PAYLOAD_LEN, "payload of {n2} bytes, the game writes {PAYLOAD_LEN}");
        Ok(NativeSave { payload: s2.to_vec() })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        container(&self.payload)
    }

    pub fn u8(&self, o: usize) -> u8 {
        self.payload[o]
    }
    pub fn u16(&self, o: usize) -> u16 {
        u16::from_le_bytes(self.payload[o..o + 2].try_into().unwrap())
    }
    pub fn u32(&self, o: usize) -> u32 {
        u32::from_le_bytes(self.payload[o..o + 4].try_into().unwrap())
    }
    pub fn f32(&self, o: usize) -> f32 {
        f32::from_bits(self.u32(o))
    }
    pub fn set_u8(&mut self, o: usize, v: u8) {
        self.payload[o] = v;
    }
    pub fn set_u16(&mut self, o: usize, v: u16) {
        self.payload[o..o + 2].copy_from_slice(&v.to_le_bytes());
    }
    pub fn set_u32(&mut self, o: usize, v: u32) {
        self.payload[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    pub fn set_f32(&mut self, o: usize, v: f32) {
        self.set_u32(o, v.to_bits());
    }

    pub fn name(&self) -> String {
        let b = &self.payload[NAME..NAME + NAME_LEN];
        let n = b.iter().position(|&c| c == 0).unwrap_or(NAME_LEN);
        b[..n].iter().map(|&c| c as char).collect()
    }

    pub fn set_name(&mut self, name: &str) {
        let b = &mut self.payload[NAME..NAME + NAME_LEN];
        b.fill(0);
        // The inverse of `name` (one byte per char): a loaded name keeps
        // its bytes (community saves have CP1251 names such as "Эмиль",
        // DD EC E8 EB FC, which the original's writer copies as they are);
        // only a char past U+00FF has no byte.
        for (d, c) in b.iter_mut().zip(name.chars().take(NAME_LEN - 1)) {
            *d = u8::try_from(c as u32).unwrap_or(b'?');
        }
    }

    pub fn slot(&self, i: usize) -> usize {
        SLOTS + i * SLOT_LEN
    }

    /// The visit table's entries (hash, the four bytes after it).
    pub fn visits(&self) -> Vec<(u32, [u8; 4])> {
        let n = (self.u32(VISIT_COUNT) as usize).min(VISIT_MAX);
        (0..n).map(|i| (self.u32(VISITS + 8 * i), self.payload[VISITS + 8 * i + 4..VISITS + 8 * i + 8].try_into().unwrap())).collect()
    }

    pub fn set_visits(&mut self, v: &[(u32, [u8; 4])]) {
        let n = v.len().min(VISIT_MAX);
        self.payload[VISITS..VISITS + 8 * VISIT_MAX].fill(0);
        for (i, (h, b)) in v.iter().take(n).enumerate() {
            self.set_u32(VISITS + 8 * i, *h);
            self.payload[VISITS + 8 * i + 4..VISITS + 8 * i + 8].copy_from_slice(b);
        }
        self.set_u32(VISIT_COUNT, n as u32);
    }

    pub fn stage(&self, k: usize) -> usize {
        STAGES + k * STAGE_LEN
    }
}

#[cfg(test)]
mod picture_tests {
    use super::*;

    #[test]
    fn a_cover_file_is_the_originals_size_and_bgra() {
        let rgba: Vec<u8> = (0..4 * 3).flat_map(|_| [10u8, 20, 30, 255]).collect();
        let f = picture(&rgba, 4, 3);
        assert_eq!(f.len(), PICTURE_FILE_LEN);
        assert_eq!(f.len(), 1_048_612);
        let u = |o: usize| u32::from_le_bytes(f[o..o + 4].try_into().unwrap());
        assert_eq!((u(0), u(4) as usize, u(8), u(0xC) as usize, u(0x1C), u(0x20) as usize), (MAGIC, f.len(), 8, PICTURE_PAYLOAD, BLOCK_TYPE, PICTURE_PAYLOAD));
        assert_eq!(checksum(&f[..0x18]), u(0x18));
        assert_eq!(&f[0x24..0x28], &[30, 20, 10, 255]);
    }

    /// 0x5C3130: StretchRect with D3DTEXF_NONE (point sampling, no blend
    /// of neighbours) into an A8R8G8B8 target, alpha copied.
    #[test]
    fn a_cover_is_point_sampled_with_its_alpha() {
        // Two columns, black and white: no grey between them.
        let rgba: Vec<u8> =
            (0..2 * 2).flat_map(|i| if i % 2 == 0 { [0u8, 0, 0, 9] } else { [255, 255, 255, 200] }).collect();
        let f = picture(&rgba, 2, 2);
        let px = |x: usize| &f[0x24 + x * 4..0x24 + x * 4 + 4];
        assert_eq!(px(0), &[0, 0, 0, 9]);
        assert_eq!(px(255), &[0, 0, 0, 9]);
        assert_eq!(px(256), &[255, 255, 255, 200]);
        assert_eq!(px(511), &[255, 255, 255, 200]);
        // 320 → 512: target x takes source floor((x + 0.5) · 320 / 512).
        let wide: Vec<u8> = (0..320u32).flat_map(|x| [x as u8, (x >> 8) as u8, 0, 255]).collect();
        let f = picture(&wide, 320, 1);
        for x in [0usize, 1, 2, 3, 100, 511] {
            let want = ((x as f32 + 0.5) * 320.0 / 512.0) as u32;
            assert_eq!(u32::from(f[0x24 + x * 4 + 2]) | u32::from(f[0x24 + x * 4 + 1]) << 8, want, "x {x}");
        }
    }

    #[test]
    fn a_cover_file_reads_back_as_rgba() {
        let rgba: Vec<u8> = (0..4 * 3).flat_map(|_| [10u8, 20, 30, 7]).collect();
        let f = picture(&rgba, 4, 3);
        let back = read_picture(&f).unwrap();
        assert_eq!(back.len(), PICTURE_PAYLOAD);
        assert_eq!(&back[..4], &[10, 20, 30, 255]);
        assert!(read_picture(&f[..f.len() - 1]).is_err());
        let mut bad = f.clone();
        bad[0x18] ^= 1;
        assert!(read_picture(&bad).is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A save file around a payload of zeros named "NAME" (the container
    /// tests need no game data).
    fn blank() -> Vec<u8> {
        let mut s = NativeSave { payload: vec![0; PAYLOAD_LEN] };
        s.set_name("NAME");
        s.to_bytes()
    }

    #[test]
    fn fresh_round_trips() {
        let file = blank();
        let s = NativeSave::parse(&file).unwrap();
        assert_eq!(s.to_bytes(), file);
        assert_eq!(s.name(), "NAME");
        assert_eq!(PROFILE + 0xDFE + 0x33 + 0x43E, PAYLOAD_LEN);
        assert_eq!(CONTROLS + CONTROL_LEN * CONTROL_DEVICES, PAYLOAD_LEN);
        assert_eq!(PROFILE_258 + 0x80, NAME);
        assert_eq!(GARAGE_CARS + GARAGE_CAR_LEN * GARAGE_CAR_MAX, SLOTS);
        assert_eq!(SLOTS + SLOT_LEN * 5, SLOT_COUNT);
        assert_eq!(RANKED_CARS + RANKED_CAR_LEN * RANKED_CAR_MAX + 4, CAREER);
        assert_eq!(REC_TUNING + 5 * TUNING_LEN + 3, RECORD_LEN);
        assert_eq!(INTRO_EVENT + 4, PROFILE);
    }

    #[test]
    fn checksum_matches_header() {
        let file = blank();
        let u = |o: usize| u32::from_le_bytes(file[o..o + 4].try_into().unwrap());
        assert_eq!(checksum(&file[..0x18]), u(0x18));
        assert_eq!(checksum(&[1, 2, 3]), 0);
    }

    /// 0x6FD3F0: a damaged payload still loads (its checksum's error is
    /// cleared at 0x6FD73C); a damaged header, size or magic does not.
    #[test]
    fn rejects_header_damage_only() {
        let fresh = blank();
        let mut b = fresh.clone();
        b[0x1000] ^= 1;
        let s = NativeSave::parse(&b).expect("a payload checksum mismatch loads");
        assert_eq!(s.payload[0x1000 - HEADER_LEN - 8], b[0x1000]);
        for at in [0, 4, 0x18] {
            let mut h = fresh.clone();
            h[at] ^= 1;
            assert!(NativeSave::parse(&h).is_err(), "header byte {at:#x}");
        }
        assert!(NativeSave::parse(&fresh[..fresh.len() - 1]).is_err());
    }
}
