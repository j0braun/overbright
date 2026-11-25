//! The shop drive-in and drive-out scenes (NIS `ShopEnter1` /
//! `ShopExit1`) and the shop doors they open and close; also the career's
//! start scene (`CareerNis1`, see [`SceneKind::Career`]).
//!
//! * Accepting UI_EngageShopDialog (handler 0x499690, Accept_Button
//!   0xD72F002A) runs 0x499640 → 0x499470: the shop is entered (0x54CB00),
//!   then the NIS manager 0x827E88 is told to play a shop entry (0x430570:
//!   +0x50 = 1, +0x54 = the shop's hash +0x38; 0x827ED4 = the marker's
//!   +0x30, the shop's world trigger), the scene is picked and loaded
//!   (0x437270 → 0x436840 case 4: the scene table's entries with flag
//!   +6 & 1 for an entry, & 2 for an exit, +5 & 0x40, in region 4000; the
//!   game ships one of each, `Scene_ShopEnter1` and `Scene_ShopExit1`) and
//!   the flow plays it (0x5F4250(2)). Without a scene 0x4995A0 opens the
//!   shop's front end at once; it is also where the scene's end leads.
//! * The scene file `NIS\Scene_<name>_BundleB.bun`: chunk 0x80037020 holds
//!   the header (sub-chunk 0x37030: +0x00 hash, +0x04 name, +0x18 type —
//!   0 intro, 2 dyno entry, 3 shop entry, 4 shop exit (0x43B000 reads it
//!   at scene +0x10 → +0x18) — and three equal lengths from +0x28, 140
//!   frames for ShopEnter1, 120 for ShopExit1), and 0xE34010 banks with
//!   the car's channels `ZPM_Car1_q` / `_s` / `_t` (and spotlight channels
//!   the scene's lamps use, not drawn).
//! * Its origin (0x43B000 → 0x431990): for a shop entry (type 3) the
//!   marker named by the shop's hash (0x431930 with 0x827EDC), for an exit
//!   (type 4, career +0x713C clear) 0x42FC60 with the start marker
//!   0x86501C; x / y the marker's, z the ground there (0x5E7E80), turned
//!   about z by the marker's heading (0x431930 takes heading − 0x4000 and
//!   0x431990 adds 0x4000 back for these scenes; 0x42FC60 adds 0x4000 and
//!   0x431990 takes it off).
//! * The event track `NIS\Scene_<name>_EvtTrk.txt` (parsed by 0x432D00):
//!   `EVENT: <time> WORLD_MESSAGE DOOR_POSITION <position> <duration>`
//!   (type 0xC; 0x430D00: DOOR_POSITION → 1) posts the door message
//!   0x827F40 ("WAM_NIS_DOOR_CONTROL_MSG") to every world animation whose
//!   +0x20 or +0x24 is 0x827ED4 (0x438CA0), handled by 0x435660 (see
//!   [`Door::message`]). ShopEnter1: closed at 0, opening from 21 over 140;
//!   ShopExit1: open at 0, closing from 40 over 70.

use std::path::Path;

use anyhow::{Context, Result};
use glam::{Mat4, Quat, Vec3};

use crate::formats::bundle::{chunks, read_u32};
use crate::formats::camera_tracks::{CameraGroup, CameraTrack, GENERIC_CAMERAS, parse_camera_tracks};
use crate::formats::eagl::{self, Channel};
use crate::track_camera;

pub mod audio;
pub mod effects;
pub mod wheels;

/// The scene clock, frames a second (the controllers' 30, 0x785BD8).
pub const FPS: f32 = 30.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneKind {
    Enter,
    Exit,
    /// "Career Start NIS" (`Scene_CareerNis1`): the airport scene a new
    /// career opens with (0x437270 → 0x436840 case 4 takes 0x865598 =
    /// BinHash("CareerNis1"), set at 0x77D8E0, while the D-Day script is
    /// idle, 0x500AA0).
    Career,
    /// A race's start scene, `Scene_IntroNis<nn>` (type 0; see
    /// [`IntroTable`]).
    Intro(u8),
    /// A race's ending scene, `Scene_EndingNis<nn>` (0x436840 case 1,
    /// [`IntroTable::ending_candidates`]): it plays behind the post-race
    /// screens.
    Ending(u8),
    /// The car driving onto the dyno, `Scene_DynoEnter` (type 2), when the
    /// tuning session's track is loaded.
    DynoEnter,
    /// The car driving off the dyno, `Scene_DynoExit` (type 2, 180
    /// frames), when the Dyno Shop's Test Run ends the dyno session's race
    /// (race type 8, 0x603EC0 case 1 → 0x5F41F0(4) → 0x436840 case 1 on
    /// the dyno track 4706); the test run's track loads after it.
    DynoExit,
}

impl SceneKind {
    pub fn name(self) -> String {
        match self {
            SceneKind::Enter => "ShopEnter1".into(),
            SceneKind::Exit => "ShopExit1".into(),
            SceneKind::Career => "CareerNis1".into(),
            SceneKind::Intro(n) => format!("IntroNis{n:02}"),
            SceneKind::Ending(n) => format!("EndingNis{n:02}"),
            SceneKind::DynoEnter => "DynoEnter".into(),
            SceneKind::DynoExit => "DynoExit".into(),
        }
    }
}

/// A door message from the event track: at `time` (frames), move to
/// `position` (0 shut, 1 open) over `duration` (0: at once).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DoorEvent {
    pub time: f32,
    pub position: f32,
    pub duration: f32,
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub kind: SceneKind,
    /// The header's hash (+0x00): the key of the scene's camera group.
    pub hash: u32,
    /// The header's type (+0x14).
    pub scene_type: u32,
    /// Frames (header +0x28).
    pub length: f32,
    /// Header +0x24: the scene's car count; +0x2C / +0x30: two frames a
    /// race start scene names (IntroNis08: 260 and 180). With Crowds off
    /// 0x436840 plays no scene and makes a third of the time between
    /// them the countdown's step (race status +0xC, see [`IntroTable`]).
    pub cars: u32,
    pub marks: [u32; 2],
    /// The header's description (+0x34), e.g. "Flag girl runs from side,
    /// looks at cars, starts race".
    pub description: String,
    pub car_rotation: Option<Channel>,
    pub car_translation: Option<Channel>,
    /// `ZPM_Car<n>_q` / `ZPM_Car<n>_t` for n = 1..8 (index n − 1): the
    /// race start scene's cars. 0x43ACF0 binds each slot whose racer is
    /// known (0x827B98 + 0x14·(n − 1), +0x10 the racer's index, set by
    /// 0x432860 from the racer's grid slot, info +0x03 = n) to its two
    /// channels (0x434A80 on the `_t` and `_q` hashes); a slot with no
    /// channel bound is dropped.
    pub racers: [Option<(Option<Channel>, Channel)>; 8],
    pub doors: Vec<DoorEvent>,
    /// Native brake-lock, burnout wheel and body-torque event records.
    /// Body-torque commands are retained for the NIS solver, not yet applied.
    pub wheel_events: Vec<wheels::Event>,
    /// Authored CAR_AUDIO_PATTERN records in native (frame-sorted) order.
    pub audio_events: Vec<audio::Event>,
    /// The car effect records (CAR_TRIGGER_EFFECT, SEQ_CAR_NITRO's flame).
    pub effect_events: Vec<effects::Event>,
    /// The SCREEN_FLASH events' times (scene frames, `screen_flash`).
    pub flashes: Vec<f32>,
}

impl Scene {
    pub fn load(root: &Path, kind: SceneKind) -> Result<Scene> {
        let bun = crate::game_fs::read(root, &format!("NIS/Scene_{}_BundleB.bun", kind.name())).context("the scene bundle")?;
        let track = crate::game_fs::read_to_string(root, &format!("NIS/Scene_{}_EvtTrk.txt", kind.name())).unwrap_or_default();
        Scene::parse(kind, &bun, &track)
    }

    pub fn parse(kind: SceneKind, bun: &[u8], track: &str) -> Result<Scene> {
        let mut scene = Scene { kind, hash: 0, scene_type: 0, length: 0.0, cars: 0, marks: [0; 2], description: String::new(), car_rotation: None, car_translation: None, racers: Default::default(), doors: parse_door_events(track), wheel_events: wheels::parse(track), audio_events: audio::sorted(audio::parse(track)), effect_events: effects::parse(track), flashes: crate::screen_flash::parse(track) };
        let mut rotations: [Option<Channel>; 8] = Default::default();
        for c in chunks(bun, 0) {
            match c.id {
                0x8003_7020 => {
                    if let Some(h) = chunks(c.body, c.offset).find(|h| h.id == 0x0003_7030) {
                        scene.hash = read_u32(h.body, 0);
                        scene.scene_type = read_u32(h.body, 0x18);
                        scene.length = read_u32(h.body, 0x28) as f32;
                        if h.body.len() >= 0x34 {
                            scene.cars = read_u32(h.body, 0x24);
                            scene.marks = [read_u32(h.body, 0x2C), read_u32(h.body, 0x30)];
                            scene.description = h.body[0x34..].iter().take_while(|&&b| b != 0).map(|&b| b as char).collect();
                        }
                    }
                }
                0x00E3_4010 => {
                    for (name, channel) in eagl::parse_bank(c.aligned(16)).unwrap_or_default() {
                        match name.as_str() {
                            "ZPM_Car1_q" => scene.car_rotation = Some(channel.clone()),
                            "ZPM_Car1_t" => scene.car_translation = Some(channel.clone()),
                            _ => {}
                        }
                        let slot = |suffix: &str| name.strip_prefix("ZPM_Car").and_then(|r| r.strip_suffix(suffix)).and_then(|n| n.parse::<usize>().ok()).filter(|n| (1..=8).contains(n));
                        if let Some(n) = slot("_q") {
                            rotations[n - 1] = Some(channel);
                        } else if let Some(n) = slot("_t") {
                            scene.racers[n - 1] = Some((None, channel));
                        }
                    }
                }
                _ => {}
            }
        }
        for (r, q) in scene.racers.iter_mut().zip(rotations) {
            if let Some((rot, _)) = r {
                *rot = q;
            }
        }
        anyhow::ensure!(scene.car_translation.is_some(), "no ZPM_Car1_t");
        Ok(scene)
    }

    /// The car's pose at scene frame `frame` in the scene's space: the
    /// channel's keys spread over the scene's length, two scene frames a
    /// key (71 keys over ShopEnter1's 140 frames, 61 over ShopExit1's 120).
    /// **Inferred** from the data: both lengths are exactly twice the last
    /// key, and so the car meets the door (the scene origin, the slats at
    /// y 0) at frame 96, when the door, opening from 21 at 90 frames per
    /// 140, has its slats up (at one key a frame it would be at frame 48,
    /// through the slats still down). The NIS time base itself is not
    /// traced.
    pub fn car(&self, frame: f32) -> (Vec3, Quat) {
        // Two scene frames a key (the shop scenes' lengths are exactly
        // twice their last key; the race ending scenes' 46 keys bring the
        // car in within ~3 s of their 550 frames, as under Wine the car
        // stands among the crowd when Event Results comes up).
        let key = |c: &Channel| (frame * 0.5).min((c.frames().max(2) - 1) as f32);
        let pos = self.car_translation.as_ref().map_or(Vec3::ZERO, |c| c.vec3(key(c)));
        let rot = self.car_rotation.as_ref().map_or(Quat::IDENTITY, |c| c.quat(key(c)));
        (pos, rot)
    }

    /// A bound scene slot, including the player slot from an ending entry.
    pub fn car_at(&self, slot: usize, frame: f32) -> (Vec3, Quat) {
        self.racer(slot, frame).unwrap_or_else(|| self.car(frame))
    }

    /// Grid slot `n`'s car (1-based, racer info +0x03) at scene frame
    /// `frame`, in the scene's space, two scene frames a key as
    /// [`Scene::car`]; `None` when the scene has no such car. 0x439240
    /// evaluates the bound channels each frame of a race start scene (until
    /// the scene clock passes the cut, header +0x30) and 0x4309D0 puts the
    /// racer there: its x / y and the heading of the transformed matrix,
    /// the car's own z (+0x68); the first time by 0x5FBD70 (a placement),
    /// then through the NIS mover 0x5A40B0 (mode 5), which places it again
    /// when it is over 1.1 m or 15° off and otherwise steers it to the
    /// pose. **Observed** under Wine (Quick Race drift, Stadium Drift 2,
    /// IntroNis12): the four cars at the load on markers 0–3, then from
    /// the scene's start on Car1–Car4's keys, the player (slot 4) rolling
    /// from 15.95 m to 9.09 m behind the start line over 3.7 s (57 keys).
    pub fn racer(&self, n: usize, frame: f32) -> Option<(Vec3, Quat)> {
        let (rot, pos) = self.racers.get(n.checked_sub(1)?)?.as_ref()?;
        let key = |c: &Channel| (frame * 0.5).min((c.frames().max(2) - 1) as f32);
        Some((pos.vec3(key(pos)), rot.as_ref().map_or(Quat::IDENTITY, |c| c.quat(key(c)))))
    }
}

/// The scene cameras: the generic camera groups (chunk 0x8003B201) of
/// `GLOBAL\InGameFreeRoam.bun`, keyed by the scenes' hashes (one track
/// each for the shop scenes, "Track 0").
pub fn load_cameras(root: &Path) -> Vec<CameraGroup> {
    let Ok(raw) = crate::game_fs::read(root, "GLOBAL/InGameFreeRoam.bun") else { return Vec::new() };
    let data = if raw.starts_with(b"JDLZ") { crate::formats::jdlz::decompress(&raw).unwrap_or(raw) } else { raw };
    let mut out = Vec::new();
    let mut stack = vec![(&data[..], 0usize)];
    while let Some((d, base)) = stack.pop() {
        for c in chunks(d, base) {
            if c.id == GENERIC_CAMERAS {
                out.extend(parse_camera_tracks(c).unwrap_or_default());
            } else if c.id & 0x8000_0000 != 0 {
                stack.push((c.body, c.offset));
            }
        }
    }
    out
}

impl Scene {
    /// The scene's camera track (0x44FE80 while a scripted scene runs,
    /// 0x4324D0(1)): in the generic group keyed by the scene's hash
    /// (0x4307C0 → 0x442980), the track named "Track %d" of the scene's
    /// +0x4C (0x827B14, set by the scene's constructor at 0x439014);
    // prov: inferred shop_scene.camera_track
    /// **inferred**: 0 for the shop scenes, whose groups hold only
    /// "Track 0".
    pub fn camera_track<'a>(&self, groups: &'a [CameraGroup]) -> Option<&'a CameraTrack> {
        groups.iter().find(|g| g.key == self.hash)?.tracks.iter().find(|t| t.name == "Track 0")
    }

    /// The scene camera's view at scene frame `frame` (0x44FE80: the
    /// fraction the scene's +0x2C over +0x30, the clone's frame over the
    /// length; the entry at the fraction, 0x442B20, shown when its type
    /// is above 1, 0x5FB46C; its points in the scene's frame, space 3).
    pub fn camera(&self, track: &CameraTrack, frame: f32, origin: Mat4, ground: Option<&dyn Fn(Vec3) -> Option<f32>>) -> Option<crate::draw::camera::Camera> {
        let fraction = frame / self.length.max(1.0);
        let i = track_camera::entry_index(track, fraction)?;
        if track.entries[i].mover <= 1 {
            return None;
        }
        track_camera::camera_in(track, i, fraction, origin, Some(origin), ground)
    }

    /// The camera's shot at scene frame `frame` (`track_camera::shot`).
    pub fn camera_shot(&self, track: &CameraTrack, frame: f32) -> Option<usize> {
        track_camera::shot(track, frame / self.length.max(1.0))
    }

    /// The letterbox bars' share of the screen height at scene frame
    /// `frame` (`track_camera::letterbox`; 0 while the entry shows no
    /// scene camera).
    pub fn letterbox(&self, track: &CameraTrack, frame: f32) -> f32 {
        let fraction = frame / self.length.max(1.0);
        track_camera::entry_index(track, fraction).filter(|&i| track.entries[i].mover > 1).map_or(0.0, |i| track_camera::letterbox(track, i, fraction))
    }
}

/// A race start scene of the NIS table (chunk 0x80037050 → 0x37070 in
/// `GLOBAL\InGameCommon.bun`, kept at 0x827AD4 by 0x431870): the race
/// intro list is the table's fourth (count +0x0C, 0x14-byte entries from
/// +0x2BC).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntroEntry {
    /// +0x00: BinHash("IntroNis<nn>").
    pub hash: u32,
    /// +0x05: flags; 0x40 marks it a race start scene, the low bits the
    /// grid slots (1 << (slot − 1)) a human racer may start from.
    pub flags: u8,
    /// +0x06 / +0x07: the racer counts it is for.
    pub min_racers: u8,
    pub max_racers: u8,
    /// +0x08: the start kind it is for (the track record's +0xF6).
    pub start_kind: u8,
    /// +0x0C: the scene car each grid slot takes (0x436840's car number,
    /// table byte (index·5 + 0xAF)·4 + 0x0B + slot).
    pub slot_cars: [u8; 4],
}

#[derive(Clone, Debug, Default)]
pub struct IntroTable {
    pub entries: Vec<IntroEntry>,
    /// The tracks that name their scenes (0x430160): (track, scene index
    /// mask), the list at +0x37C (count +0x14) when the call's flag is 1,
    /// else +0x91C (count +0x18); 12-byte records. A track not listed may
    /// take any scene.
    pub tracks: [Vec<(u16, u16)>; 2],
    /// The single-player race start list (0x436840 case 0, taken when
    /// 0x5F0100 on 0x89CF48 says no other human races; case 3 and
    /// [`IntroTable::entries`] are the multiplayer path): count +0x00,
    /// 12-byte records from +0x1C.
    pub race_list: Vec<RaceEntry>,
    /// The two track lists' whole records (track, u32 mask of list
    /// entries 0–31 at +4, u16 mask of entries 32.. at +8; 0x430070).
    pub track_masks: [Vec<(u16, u32, u16)>; 2],
    /// The race ending scenes (0x436840 case 1): count +0x04, 8-byte
    /// records from +0x25C (hash, variant +4, flags +5).
    pub ending_list: Vec<EndingEntry>,
    /// The track list 0x4300F0 reads for them (+0x91C, count +0x18, the
    /// call's flag clear): track, u16 mask of ending entries at +10.
    pub ending_masks: Vec<(u16, u16)>,
}

/// A race ending scene (0x436840 case 1; 8 bytes from table +0x25C).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EndingEntry {
    /// +0x00: BinHash("EndingNis<nn>").
    pub hash: u32,
    /// +0x04: the variant, handed out with the hash (the camera track,
    /// "Track %d").
    pub variant: u8,
    /// +0x05: flags (0x40 offline single player, 0x80 see
    /// [`IntroTable::ending_candidates`]).
    pub flags: u8,
    pub rest: [u8; 2],
}

/// A single-player race start scene (0x436840 case 0; 12 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RaceEntry {
    /// +0x00: BinHash("IntroNis<nn>").
    pub hash: u32,
    /// +0x04: the variant (case 0 hands it out with the hash: the scene's
    /// camera track, "Track %d").
    pub variant: u8,
    /// +0x05: flags, 0x40 and the grid slots a human may start from.
    pub flags: u8,
    /// +0x06 / +0x07: the racer counts.
    pub min_racers: u8,
    pub max_racers: u8,
    /// +0x08: the start kind (track record +0xF6).
    pub start_kind: u8,
}

impl IntroTable {
    pub fn load(root: &Path) -> Result<IntroTable> {
        let raw = crate::game_fs::read(root, "GLOBAL/InGameCommon.bun").context("InGameCommon.bun")?;
        let data = if raw.starts_with(b"JDLZ") { crate::formats::jdlz::decompress(&raw).unwrap_or(raw) } else { raw };
        let mut found = None;
        crate::formats::bundle::walk(chunks(&data, 0), &mut |c| {
            if c.id == 0x0003_7070 && found.is_none() {
                found = Some(c.body.to_vec());
            }
        });
        let t = found.context("no NIS table (0x37070)")?;
        Ok(IntroTable::parse(&t))
    }

    pub fn parse(t: &[u8]) -> IntroTable {
        let count = |o: usize| if t.len() >= o + 4 { read_u32(t, o) as usize } else { 0 };
        let entries = (0..count(0x0C))
            .filter_map(|k| {
                let e = t.get(0x2BC + k * 0x14..0x2BC + (k + 1) * 0x14)?;
                Some(IntroEntry { hash: read_u32(e, 0), flags: e[5], min_racers: e[6], max_racers: e[7], start_kind: e[8], slot_cars: [e[0xC], e[0xD], e[0xE], e[0xF]] })
            })
            .collect();
        let list = |at: usize, n: usize| (0..n).filter_map(|k| t.get(at + k * 0xC..at + k * 0xC + 4).map(|r| (u16::from_le_bytes([r[0], r[1]]), u16::from_le_bytes([r[2], r[3]])))).collect();
        let race_list = (0..count(0x00))
            .filter_map(|k| {
                let e = t.get(0x1C + k * 12..0x1C + (k + 1) * 12)?;
                Some(RaceEntry { hash: read_u32(e, 0), variant: e[4], flags: e[5], min_racers: e[6], max_racers: e[7], start_kind: e[8] })
            })
            .collect();
        let masks = |at: usize, n: usize| (0..n).filter_map(|k| t.get(at + k * 0xC..at + k * 0xC + 0xC).map(|r| (u16::from_le_bytes([r[0], r[1]]), read_u32(r, 4), u16::from_le_bytes([r[8], r[9]])))).collect();
        let ending_list = (0..count(0x04))
            .filter_map(|k| {
                let e = t.get(0x25C + k * 8..0x25C + (k + 1) * 8)?;
                Some(EndingEntry { hash: read_u32(e, 0), variant: e[4], flags: e[5], rest: [e[6], e[7]] })
            })
            .collect();
        let ending_masks = (0..count(0x18)).filter_map(|k| t.get(0x91C + k * 0xC..0x91C + k * 0xC + 0xC).map(|r| (u16::from_le_bytes([r[0], r[1]]), u16::from_le_bytes([r[10], r[11]])))).collect();
        IntroTable { entries, tracks: [list(0x37C, count(0x14)), list(0x91C, count(0x18))], race_list, track_masks: [masks(0x37C, count(0x14)), masks(0x91C, count(0x18))], ending_list, ending_masks }
    }

    /// The scenes 0x436840 case 3 may pick for a race: the track's own
    /// mask (0x430160, list 0 when `first_list`), flag 0x40 and the human
    /// racer's grid slot among the flags (0x430020 with 0x431F10's slot
    /// mask), the racer count within the entry's, and the track's start
    /// kind (0x42FFD0, also kind 3 on an entry of kind 2 for two racers or
    /// fewer when the entry is the string 0x785D54's scene, **not ported**).
    pub fn candidates(&self, track: u16, first_list: bool, racers: u32, human_slot: u8, start_kind: u8) -> Vec<usize> {
        let list = &self.tracks[usize::from(!first_list)];
        let slot_bit = 1u32.checked_shl(u32::from(human_slot.max(1)) - 1).unwrap_or(0);
        (0..self.entries.len())
            .filter(|&i| {
                let e = &self.entries[i];
                let allowed = list.iter().find(|r| r.0 == track).is_none_or(|r| r.1 & (1 << i) != 0);
                allowed
                    && e.flags & 0x40 != 0
                    && u32::from(e.flags) & slot_bit == slot_bit
                    && (u32::from(e.min_racers)..=u32::from(e.max_racers)).contains(&racers)
                    && e.start_kind == start_kind
            })
            .collect()
    }

    /// The single-player race's scenes (0x436840 case 0, **verified**
    /// unless marked): Crowds on (0x5BF6E0; off, no scene); the track's
    /// list record (0x430070: list 0 when `first_list`, the entry's index
    /// k a bit of the u32 at +4 below 32, of the u16 at +8 from 32; a track
    /// not listed takes any); 0x42FE00: flag 0x40, the humans' grid-slot
    /// bits within the flags, the racer count within [+6, +7], the start
    /// kind (0x42FCF0: equal; also kind 3 with an entry of kind 2 for
    /// fewer than three racers on IntroNis13 variant 0, and kind 1 with
    /// an entry of kind 2 for two players — split screen, not ported) and
    /// 0x42FD60 (IntroNis15 for a human on grid slot 2 only when the car's
    /// +0x581 > 1: **not ported**, the scene is left out there). Also not
    /// ported: 0x863470 +0x0D = 1 keeping only IntroNis08 / 12, and two
    /// players' 0x42FDA0 (0x8900BC = 2).
    pub fn race_candidates(&self, track: u16, first_list: bool, racers: u32, human_slot: u8, start_kind: u8) -> Vec<usize> {
        let list = &self.track_masks[usize::from(!first_list)];
        let slot_bit = 1u32.checked_shl(u32::from(human_slot.max(1)) - 1).unwrap_or(0);
        let nis15 = crate::formats::hash::bin_hash("IntroNis15");
        let nis13 = crate::formats::hash::bin_hash("IntroNis13");
        (0..self.race_list.len())
            .filter(|&k| {
                let e = &self.race_list[k];
                let allowed = list.iter().find(|r| r.0 == track).is_none_or(|r| if k < 32 { r.1 & (1 << k) != 0 } else { u32::from(r.2) & (1 << (k - 32)) != 0 });
                let kind = e.start_kind == start_kind || (start_kind == 3 && e.start_kind == 2 && racers < 3 && e.hash == nis13 && e.variant == 0);
                allowed
                    && e.flags & 0x40 != 0
                    && u32::from(e.flags) & slot_bit == slot_bit
                    && (u32::from(e.min_racers)..=u32::from(e.max_racers)).contains(&racers)
                    && kind
                    && !(e.hash == nis15 && human_slot == 2)
            })
            .collect()
    }

    /// The ending scenes 0x436840 case 1 may pick (0x865010 clear, no
    /// forced scene 0x86502C, not the dyno track 0x1262 — that takes
    /// DynoExit) for an offline human winner (0x431DF0: +0x1C clear,
    /// +0x1E set when the results winner is player 0):
    /// * 0x4300F0: a track listed at +0x91C takes entry k when bit k of its
    ///   u16 at +10 is set; a track not listed takes any;
    /// * 0x42FED0: EndingNis01 only when the results winner's tuning block
    ///   (car +0x20) has rear torque split +0x1C0 > 0.8;
    /// * 0x42FF50: flag 0x40, and with +0x1E set flag 0x80 too (the
    ///   track numbers 0x104E–0x1053 refuse every scene for one player);
    /// * 0x42FF20: any scene when the NIS memory pool (0x43E8E0(1)) has
    ///   1,600,000 bytes free (0x4306F0), else only EndingNis04 (`room`;
    ///   the pool's size is not traced, but under Wine one race end played
    ///   EndingNis04 and the next EndingNis02, so it passes on the PC).
    ///
    /// One of them is drawn at random (0x43C1C0).
    pub fn ending_candidates(&self, track: u16, rear_split: f32, room: bool) -> Vec<usize> {
        self.ending_candidates_for_result(track, rear_split, room, true)
    }

    /// 0x431DF0 / 0x42FF50: bit 0x80 selects a human winner; without
    /// it the winner is AI. The drivetrain gate also reads that winner.
    pub fn ending_candidates_for_result(&self, track: u16, rear_split: f32, room: bool, player_won: bool) -> Vec<usize> {
        let mask = self.ending_masks.iter().find(|r| r.0 == track).map(|r| r.1);
        let nis01 = crate::formats::hash::bin_hash("EndingNis01");
        let nis04 = crate::formats::hash::bin_hash("EndingNis04");
        if (0x104E..=0x1053).contains(&track) {
            return Vec::new();
        }
        (0..self.ending_list.len())
            .filter(|&k| {
                let e = &self.ending_list[k];
                mask.is_none_or(|m| k < 16 && m & (1 << k) != 0)
                    && (e.hash != nis01 || rear_split > 0.8)
                    && e.flags & 0x40 != 0
                    && (e.flags & 0x80 != 0) == player_won
                    && (room || e.hash == nis04)
            })
            .collect()
    }

    /// 0x436BBA..0x436BD1: consume the shared random stream once when
    /// the eligible list is nonempty. `winner` is the first results car.
    pub fn pick_ending(&self, track: u16, winner: &crate::handling::Handling, room: bool, player_won: bool, rng: &mut crate::opponents::GameRng) -> Option<usize> {
        let candidates = self.ending_candidates_for_result(track, winner.gearbox.rear_split, room, player_won);
        if candidates.is_empty() { return None; }
        Some(candidates[rng.below(candidates.len() as u32) as usize])
    }

    /// An ending scene's number from its hash (BinHash("EndingNis<nn>")).
    pub fn ending_number(hash: u32) -> Option<u8> {
        (1..=4).find(|&n| crate::formats::hash::bin_hash(&format!("EndingNis{n:02}")) == hash)
    }

    /// The scene's number from its hash (BinHash("IntroNis<nn>")).
    pub fn scene_number(hash: u32) -> Option<u8> {
        (1..=28).find(|&n| crate::formats::hash::bin_hash(&format!("IntroNis{n:02}")) == hash)
    }
}

/// The two marks 0x436840 case 3 takes for a race start scene with Crowds
/// off (0x436E5E..0x436F03, by the scene's name; the same as its header's
/// +0x2C / +0x30): IntroNis08 260 / 180, 12 322 / 200, 13 265 / 180, 14 320 /
/// 200, 20 311 / 200, 21 275 / 180, any other 260 / 180. The countdown's
/// step is a third of the gap at 30 frames a second (0x436F08..0x436F4D).
pub fn crowds_off_marks(scene: u8) -> (u32, u32) {
    match scene {
        12 => (322, 200),
        13 => (265, 180),
        14 => (320, 200),
        20 => (311, 200),
        21 => (275, 180),
        _ => (260, 180),
    }
}

/// The DOOR_POSITION world messages of an event track (0x432D00): the
/// time, then the message's two floats (+0x14, +0x18).
pub fn parse_door_events(track: &str) -> Vec<DoorEvent> {
    track
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.starts_with("//") {
                return None;
            }
            let w: Vec<&str> = l.split_whitespace().collect();
            if w.len() < 6 || w[0] != "EVENT:" || w[2] != "WORLD_MESSAGE" || w[3] != "DOOR_POSITION" {
                return None;
            }
            let f = |s: &str| s.trim_end_matches('f').parse::<f32>().ok();
            Some(DoorEvent { time: f(w[1])?, position: f(w[4])?, duration: f(w[5])? })
        })
        .collect()
}

/// The scene's origin (0x431990): the marker's x / y, the ground's z, turned
/// about z by the marker's heading (radians).
pub fn origin(at: Vec3, heading: f32) -> Mat4 {
    Mat4::from_translation(at) * Mat4::from_rotation_z(heading)
}

/// A shop door's animation state (an entity controller: +0x64 the frame,
/// +0x60 the length, +0x40 the rate, +0x10 the state, +0x08 open).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Door {
    pub frame: f32,
    pub length: f32,
    /// Frames a scene frame while moving (0x435660: length ÷ duration).
    pub rate: f32,
    /// Where it is going: 0 or the length.
    pub target: f32,
}

impl Door {
    pub fn new(length: f32) -> Door {
        Door { length, ..Default::default() }
    }

    /// The door message (0x435660 with the position and the duration):
    /// with no duration (under 1e-6) the frame goes to the length when the
    /// position is 1 and to 0 otherwise, at once; with one the controller
    /// plays at length ÷ duration. **Inferred**: the way it plays (0x435660
    /// sets no direction; the clone moves towards the position's end).
    pub fn message(&mut self, position: f32, duration: f32) {
        let open = (position - 1.0).abs() < 1e-6;
        self.target = if open { self.length } else { 0.0 };
        if duration.abs() < 1e-6 {
            self.frame = self.target;
            self.rate = 0.0;
        } else {
            self.rate = self.length / duration;
        }
    }

    /// Advance `dt` seconds (0x435140: 30 × rate frames a second).
    pub fn step(&mut self, dt: f32) {
        let step = FPS * self.rate * dt;
        if self.frame < self.target {
            self.frame = (self.frame + step).min(self.target);
        } else if self.frame > self.target {
            self.frame = (self.frame - step).max(self.target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ending_pick_uses_tuned_rear_split_and_advances_the_shared_rng() {
        let table = IntroTable {
            entries: vec![], tracks: [vec![], vec![]], race_list: vec![],
            track_masks: [vec![], vec![]], ending_masks: vec![],
            ending_list: [1, 2, 3, 4].map(|n| EndingEntry {
                hash: crate::formats::hash::bin_hash(&format!("EndingNis{n:02}")),
                variant: 0, flags: 0xC0, rest: [0; 2],
            }).to_vec(),
        };
        let mut handling = crate::handling::parse_handling(&[0; 0x900]);
        for split in [0.0, 0.199, 0.5, 0.8, 0.801, 1.0] {
            handling.gearbox.rear_split = split;
            let mut rng = crate::opponents::GameRng(0);
            let mut expected = rng.clone();
            expected.below(if split > 0.8 { 4 } else { 3 });
            assert_eq!(table.pick_ending(4014, &handling, true, true, &mut rng), Some(if split > 0.8 { 0 } else { 1 }), "rear split {split}");
            assert_eq!(rng.0, expected.0, "selection must consume the shared RNG");
        }
        let mut rng = crate::opponents::GameRng(3);
        assert_eq!(table.pick_ending(4014, &handling, false, true, &mut rng), Some(3));
        // Losing variants carry 0x40 without 0x80. A RWD player does
        // not unlock the burnout when the first results car is FWD.
        let mut loss_table = table.clone();
        for e in &mut loss_table.ending_list { e.flags = 0x40; }
        handling.gearbox.rear_split = 0.0;
        rng.0 = 0;
        assert_eq!(loss_table.pick_ending(4014, &handling, true, false, &mut rng), Some(1));
        assert_eq!(loss_table.pick_ending(4014, &handling, true, true, &mut rng), None);
        let before = rng.0;
        assert_eq!(table.pick_ending(0x104E, &handling, true, true, &mut rng), None);
        assert_eq!(rng.0, before, "empty candidates do not draw");
    }

    #[test]
    fn the_shop_scenes_open_and_shut_the_door() {
        let enter = "\n// start closed then open it later\nEVENT: 0.0f    WORLD_MESSAGE   DOOR_POSITION  0.0f  0.0f\nEVENT: 21.0f   WORLD_MESSAGE   DOOR_POSITION  1.0f  140.0f\nEVENT: 35.0    CAR_AUDIO_PATTERN     \t1      \t3           1.0f\n";
        let e = parse_door_events(enter);
        assert_eq!(e, vec![DoorEvent { time: 0.0, position: 0.0, duration: 0.0 }, DoorEvent { time: 21.0, position: 1.0, duration: 140.0 }]);
        let mut d = Door::new(90.0);
        d.message(1.0, 0.0);
        assert_eq!(d.frame, 90.0);
        d.message(0.0, 70.0);
        // 70 scene frames shut it.
        for _ in 0..70 {
            d.step(1.0 / FPS);
        }
        assert!(d.frame.abs() < 1e-3);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_shop_entry_scene_loads() {
        let root = crate::testkit::game_root();
        let s = Scene::load(Path::new(&root), SceneKind::Enter).unwrap();
        assert_eq!((s.scene_type, s.length), (3, 140.0));
        let x = Scene::load(Path::new(&root), SceneKind::Exit).unwrap();
        assert_eq!((x.scene_type, x.length), (4, 120.0));
        // Two scene frames a key.
        for sc in [&s, &x] {
            assert_eq!(sc.car_translation.as_ref().unwrap().frames() as f32 - 1.0, sc.length / 2.0);
        }
        // The drive-in ends inside, past the door (the origin, y 0).
        assert!(s.car(0.0).0.y < -5.0 && s.car(140.0).0.y > 6.0);
        assert!(x.car(0.0).0.y > 4.0 && x.car(120.0).0.y < -4.5);
        // The career's start scene: 405 frames of a car standing still,
        // 2.95 m to the right of the start line marker, turned a quarter.
        let c = Scene::load(Path::new(&root), SceneKind::Career).unwrap();
        assert_eq!((c.scene_type, c.length, c.hash), (4, 405.0, 0x346D_604C));
        let (a, b) = (c.car(0.0), c.car(405.0));
        assert!((a.0 - Vec3::new(0.0, -2.95, 0.0)).length() < 0.02 && (a.0 - b.0).length() < 1e-3);
        assert!((a.1.to_euler(glam::EulerRot::ZYX).0.to_degrees() - 89.2).abs() < 0.5);
    }

    /// DynoExit (Wine, played as the Dyno Shop's Test Run left the
    /// dyno, 0x827E1C = 0xB7DEC9B3, mode 5 for ~6 s): type 2 like
    /// DynoEnter, 180 frames, one car, from the rollers off the ramp.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_dyno_exit_scene_loads() {
        let root = crate::testkit::game_root();
        let x = Scene::load(Path::new(&root), SceneKind::DynoExit).unwrap();
        assert_eq!((x.hash, x.scene_type, x.length, x.cars), (0xB7DE_C9B3, 2, 180.0, 1));
        let e = Scene::load(Path::new(&root), SceneKind::DynoEnter).unwrap();
        // It starts where DynoEnter ends (the rollers) and leaves them.
        let (a, b) = (x.car(0.0).0, e.car(e.length).0);
        assert!((a - b).truncate().length() < 0.2, "{a:?} vs {b:?}");
        assert!((x.car(x.length).0 - a).truncate().length() > 10.0);
        assert!(x.camera_track(&load_cameras(Path::new(&root))).is_some());
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn a_single_player_race_picks_from_the_first_list() {
        let root = crate::testkit::game_root();
        let t = IntroTable::load(Path::new(&root)).unwrap();
        assert_eq!(t.race_list.len(), 42);
        // Resort Loop (4014, start kind 1), four racers, the human on slot
        // 1: IntroNis01 / 03 / 06 / 08 (the original played IntroNis03).
        let names: Vec<u8> = t.race_candidates(4014, true, 4, 1, 1).iter().filter_map(|&k| IntroTable::scene_number(t.race_list[k].hash)).collect();
        assert!(names.contains(&3) && names.contains(&8) && names.iter().all(|n| [1, 3, 6, 8].contains(n)), "{names:?}");
        assert_eq!(crowds_off_marks(12), (322, 200));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_shop_scenes_have_their_cameras() {
        let root = crate::testkit::game_root();
        let groups = load_cameras(Path::new(&root));
        let origin = origin(Vec3::new(100.0, 200.0, 10.0), 0.5);
        // The career's start scene flies its camera in world space.
        let c = Scene::load(Path::new(&root), SceneKind::Career).unwrap();
        let t = c.camera_track(&groups).expect("the career scene's camera track");
        assert!(t.entries.len() == 6 && t.entries.iter().all(|e| e.mover == 0x12 && e.eye_space == 1 && e.look_space == 1));
        for kind in [SceneKind::Enter, SceneKind::Exit] {
            let s = Scene::load(Path::new(&root), kind).unwrap();
            let t = s.camera_track(&groups).expect("the scene's camera track");
            assert!(t.entries.iter().all(|e| e.mover == 0x12 && e.eye_space == 3 && e.look_space == 3));
            // Frame 0: the first entry's eye, in the scene's frame.
            let c = s.camera(t, 0.0, origin, None).unwrap();
            let want = origin.transform_point3(Vec3::from(t.entries[0].eye[0]));
            assert!((c.position - want).length() < 1e-3, "{kind:?}: {:?} vs {want:?}", c.position);
        }
    }
}

crate::persist!(Door { frame, length, rate, target });
