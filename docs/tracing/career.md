# Tracing round: career

What a career editor and a later mission / career graph need from `SPEED2.EXE`, with the status of each
piece. The detail lives in [docs/formats/career-1.md](../formats/career-1.md),
[career-2.md](../formats/career-2.md) and [career-3.md](../formats/career-3.md); this page is the map over
them. Tags as in [docs/formats.md](../formats.md): **verified** (traced), **inferred**, **open**.

## Data: GlobalB chunk 0x80034A10

The game reads only the first career chunk (0x512C00, guarded by 0x8641F4). Its jump table 0x512DDC
dispatches the sub-chunks (**verified**):

| Sub-chunk | Record | Table / count | Contents | Field coverage |
|---|---|---|---|---|
| 0x34A11 | 0x88 | 0x8383E0 / 0x8383E4 | events (224) | all non-zero fields known but +0x14 (**open**); see below |
| 0x34A12 | 0xA0 | 0x8383F0 / 0x8383F4 | shops (24) | name, scene, hash +0x38, scene file +0x40, kind +0x50, announced +0x51, unlock +0x74 / +0x9C, stage +0x9D; +0x78..+0x9B vary and are **open** |
| 0x34A14 | 0x44 | 0x838400 / 0x838404 | brands | name, display name, hash +0x40 |
| 0x34A15 | own loader 0x500000 | | performance packages | docs/formats/cars-1.md |
| 0x34A16 | 0x40 | 0x83840C / 0x838408 | magazine / DVD milestones (30) | complete but +0x30..+0x33 (0) |
| 0x34A17 | own loader 0x5012F0 | 0x838454 / 0x838450 | SMS messages (167) | complete |
| 0x34A18 | own loader 0x512A70 | 0x83841C / 0x838418 | stage definitions (6) | readers found for +0x00..+0x04, +0x08, +0x1C, +0x28, +0x30..+0x34, +0x37, +0x40..+0x48; the rest **open** (no reader found) |
| 0x34A19 | 0x10 | 0x838414 / 0x838410 | sponsors (20) | complete |
| 0x34A1A | own loader 0x59AE30 | | tuning parameters | `src/tuning` |
| 0x34A1B | 0x18 | 0x838424 / 0x838420 | world challenges = magazine / unique-part triggers (28) | complete |
| 0x34A1C | 0x28 | 0x83845C / 0x838460 | part cost table | `formats::parts` |
| 0x34A1D | – | 0x838428 / 0x83842C | string pool | – |
| 0x34A1E | 0x0C | 0x838430 / 0x838434 | cash pickups (59) | career-3 "Hidden races, engage tips and cash pickups" |
| 0x34A1F | 0x0C | 0x838438 / 0x83843C | car unlocks (24) | complete |

Sub-chunk 0x34A13 is skipped by the loader.

### Event record (0x88), the editor's main object

| Offset | Field | Status |
|---|---|---|
| +0x00 u16 | name (pool) | verified; +0x08 = `bin_hash(name)` in all 224 |
| +0x02 / +0x04 u16 | movie before the race / after the win (pool) | verified (0x4FFDD0, 0x513A50) |
| +0x06 u16 | marker name (pool); `bin_hash` = +0x28 in all 224 | verified against the data |
| +0x08 u32 | event hash (car unlocks, SMS, stage final event) | verified |
| +0x0C / +0x0D | URL tier (3 / 4) / SUV only | verified (0x513510, 0x527D40) |
| +0x0F | type (0 circuit … 5 drift) | verified |
| +0x10 u32 | URL requirement | verified |
| +0x14 | 100, the D-Day pair 0 | **open** |
| +0x18 | up to 4 races × {u16 track, u8 variant, u8 laps}, count +0x7E | verified (0x532EE3) |
| +0x28 | marker hash | verified |
| +0x2C | forced player car preset (DDAY_EVENT_A: DDAY_PLAYER_CAR) | verified (0x528610) |
| +0x30 | purse | verified |
| +0x34 / +0x35 / +0x36 / +0x37 | category / goal count / traffic / stage | verified |
| +0x38 | goals, 8 bytes each (kind, hash at +0x3C), read only under +0x35 (0x4FFDE0) | verified; stray bytes there in events without goals must be kept |
| +0x40 | up to 5 opponents × 0xC, count +0x7C | verified (0x5260C0) |
| +0x7D / +0x7F | AI throttle level / always open | verified |
| +0x84 | race dialog sender label | verified (0x512820) |
| +0x0E, +0x15..+0x17, +0x32, +0x33, +0x80..+0x83 | 0 in every record | **open** (no reader looked for beyond the career code) |

Editor constraints that follow: more than one goal overlaps the opponents; the marker name and its hash
must change together; the name's hash is the event's identity for unlocks, messages and the stage end.

## Rules (all verified, ported in `src/frontend/career/`)

| Rule | Where | Docs |
|---|---|---|
| Open events: 11 slots, caps from the stage definition, shuffled draw with the game's RNG (0x43C1C0), always-open H_ events, URL requirements | 0x527D40, 0x527AB0, 0x513600, 0x5137C0, 0x513510 | career-1 "Open events" |
| Win filing per stage (normal / sponsor / URL), outrun wins and bank | 0x541430 → 0x513390, 0x541330 | career-1 "Open events", "Outrun" |
| Stage end: final event won + media milestones, or every stage-1 event | 0x532C70, 0x5130A0, 0x513490, 0x528330 | career-1 "Stages" |
| Car unlocks by event hash | 0x34A1F, 0x513C50 | career-1 "Car unlocks" |
| Shop / part unlocks by event, wins of the part's own stage | 0x512870, 0x515360 | career-1 "Shops", "Body shop" |
| Sponsors: offer by stage wins and average reputation, signing bonus, the four required races | 0x500610, 0x500670, 0x495D30 | career-1 "Sponsors" |
| Reputation per win: 0x513430 adds 0x512B50(value, event) to stage record +0xE0 (+0xE4 counts), not for the D-Day pair; 0x512B50 picks a 0x1C-byte row of table 0x7F7AC8 by a key from the event (0x512AE0) and returns its +0x10 when the value lies in its +0x04..+0x08 range (**inferred**: the value is the winning margin, 0x513FE0) | 0x513430, 0x512B50 | career-1 "Sponsors", "First car, World Map, margin" |
| SMS: 15 conditions, inbox of 256, urgent pop-up, mailboxes | 0x529930, 0x529870, 0x529760, 0x529BE0, 0x5299C0, 0x529C50 | career-1 "Messages / SMS" |
| Magazines / DVDs: offered by visual rating or a world challenge, photo shoot, reward | 0x5331D0, 0x533BF0, 0x541220 | career-1 "Milestones", "World challenges"; career-3 "Rewards gallery" |
| World challenges: offer, clock, retry, goal | 0x527980, 0x533BF0, 0x5016A0, 0x541970 | career-1 "World challenges" |
| D-Day intro: goals instead of races, the forced car | 0x532840, 0x495A30, 0x528610 | career-3 "D-Day intro" |
| Hidden races, engage tips, cash pickups | 0x5283D0, 0x529C50 | career-3 |

## Open for the editor

* Event +0x14 and the all-zero event bytes; the stage definition's unread bytes (career-1 "Stages").
* Shop record +0x78..+0x9B.
* 0x527980's special case for STAGE_5_VISUAL_5 and 0x527D40's early returns on career +0x873F / +0x8738 /
  +0x873E (career-1 "World challenges").
* Drag races' fixed lanes (needed before an editor may place drag events on new routes).
* The second career container in GlobalB is never loaded (0x8641F4): an editor that writes a new chunk
  must replace the first one.
