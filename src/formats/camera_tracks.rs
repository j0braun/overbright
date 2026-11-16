//! Camera tracks: the replay director's track cameras (see docs/formats.md,
//! Replays). Loaded by 0x44A6C0 into the camera manager 0x828770.
//!
//! ```text
//! 0x8003B200 route cameras    (track bundle, e.g. L4RA.BUN) — kind 1
//! 0x8003B201 generic cameras  (InGameRace/Drag/Drift/FreeRoam.bun) — kind 0
//! 0x8003B202 typed cameras    (InGameCommon.bun) — kind 2
//! └─ 0x0003B210 group: u32 key, u32 track count, then the tracks back to back
//!    track: 0x20-byte header {link pointers 0x00..0x0B (runtime), f32
//!           duration +0x0C, u8 entry count +0x10, u8 +0x11, char name[14]
//!           +0x12 ("Track %d")}, then count × 0x88-byte entries
//! ```
//!
//! The key (0x44A620): route groups {u16 road section, u16 direction bit}
//! (matched by 0x4429C0, at most 3000), generic groups an id (0x442980,
//! at most 256), typed groups a camera type matched in the type table
//! 0x7EC1F8 (at most 100).

use anyhow::{Result, bail};

use super::bundle::Chunk;
use super::reader::{FormatResult, Reader};

pub const ROUTE_CAMERAS: u32 = 0x8003_B200;
pub const GENERIC_CAMERAS: u32 = 0x8003_B201;
pub const TYPED_CAMERAS: u32 = 0x8003_B202;
const GROUP: u32 = 0x0003_B210;
const TRACK_HEADER: usize = 0x20;
const ENTRY: usize = 0x88;

/// Which manager list a chunk fills (0x44A6C0's kind).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraKind {
    /// 0x8003B201, kind 0: manager +0x10, keyed by an id.
    Generic,
    /// 0x8003B200, kind 1: manager +0x18, keyed by road section and direction.
    Route,
    /// 0x8003B202, kind 2: manager +0x14, keyed by camera type.
    Typed,
}

impl CameraKind {
    pub fn of_chunk(id: u32) -> Option<Self> {
        match id {
            ROUTE_CAMERAS => Some(Self::Route),
            GENERIC_CAMERAS => Some(Self::Generic),
            TYPED_CAMERAS => Some(Self::Typed),
            _ => None,
        }
    }
}

/// One 0x88-byte camera entry: the shot from its start fraction to the next
/// entry's (the last one's to 1).
#[derive(Clone, Debug)]
pub struct CameraEntry {
    /// +0: the camera type (0, 0x12 or 0x13 in the files: L4RA's route
    /// entries 31 × 0, 40 × 0x12, 12 × 0x13); the director (0x5FB060) makes
    /// the track camera mover 0x12 only when it is above 1, else it keeps a
    /// chase view.
    pub mover: u8,
    /// +1: blend from the previous shot (0x451B70).
    pub blend: bool,
    /// +2: Hermite eye / look paths (0x446180 / 0x446210); else straight
    /// lerps between the two points.
    pub spline: bool,
    /// +4 / +5: the space of the eye / look points (0x453F50): 0 the car's
    /// frame (matrix car +0x90, position +0x60), 1 world, 2 world axes about
    /// the car's position, 3 the scripted-scene frame (+0xE0 of 0x4324D0(1)).
    /// With space 3 the point also gets the ground height (0x4427C0).
    pub eye_space: u8,
    pub look_space: u8,
    /// +6 / +7: the collision pushes 0x44B040 / 0x444530.
    pub avoid: [bool; 2],
    /// +8: hold the car frame taken when the shot began (mover +0x90).
    pub frozen_frame: bool,
    /// +9: shake (0x446F10 on car +0x80).
    pub shake: bool,
    /// +0x0C: start fraction along the track.
    pub start: f32,
    /// +0x10 / +0x14: the tangent scales at the two ends (0x450040).
    pub tangent: [f32; 2],
    /// +0x18 / +0x24: eye at the start and end; +0x30 / +0x3C: look-at.
    pub eye: [[f32; 3]; 2],
    pub look: [[f32; 3]; 2],
    /// +0x48 / +0x4C: the lens's focal length (mm), start and end
    /// (`track_camera::lens_angle`; measured, not degrees).
    pub fov: [f32; 2],
    /// +0x50..+0x6C: four more start / end scalar pairs (the view's
    /// depth-of-field and shake fields, 0x453F50).
    pub params: [[f32; 2]; 4],
    /// +0x70..+0x88: the effect fields, not yet named (see docs).
    pub tail: [u8; 0x18],
}

#[derive(Clone, Debug)]
pub struct CameraTrack {
    pub name: String,
    /// +0x0C: seconds a typed track runs (0x442B70: elapsed / duration).
    pub duration: f32,
    pub entries: Vec<CameraEntry>,
}

#[derive(Clone, Debug)]
pub struct CameraGroup {
    pub kind: CameraKind,
    pub key: u32,
    pub tracks: Vec<CameraTrack>,
}

impl CameraGroup {
    /// Route groups: (road section, direction bit) — the pair 0x4429C0 matches.
    pub fn route_key(&self) -> (u16, u16) {
        (self.key as u16, (self.key >> 16) as u16)
    }
}

fn parse_entry(e: Reader<'_>) -> FormatResult<CameraEntry> {
    let f = |o| e.f32_at(o);
    let flag = |o| e.u8_at(o).map(|v| v != 0);
    Ok(CameraEntry {
        mover: e.u8_at(0)?,
        blend: flag(1)?,
        spline: flag(2)?,
        eye_space: e.u8_at(4)?,
        look_space: e.u8_at(5)?,
        avoid: [flag(6)?, flag(7)?],
        frozen_frame: flag(8)?,
        shake: flag(9)?,
        start: f(0x0C)?,
        tangent: [f(0x10)?, f(0x14)?],
        eye: [e.vec3_at(0x18)?, e.vec3_at(0x24)?],
        look: [e.vec3_at(0x30)?, e.vec3_at(0x3C)?],
        fov: [f(0x48)?, f(0x4C)?],
        params: [[f(0x50)?, f(0x54)?], [f(0x58)?, f(0x5C)?], [f(0x60)?, f(0x64)?], [f(0x68)?, f(0x6C)?]],
        tail: e.array_at(0x70)?,
    })
}

/// The groups of one 0x8003B20x chunk.
pub fn parse_camera_tracks(chunk: Chunk<'_>) -> Result<Vec<CameraGroup>> {
    let Some(kind) = CameraKind::of_chunk(chunk.id) else { bail!("not a camera chunk: {:#x}", chunk.id) };
    let mut out = Vec::new();
    for g in chunk.children().filter(|c| c.id == GROUP) {
        let mut b = Reader::new(g.body);
        if b.len() < 8 {
            bail!("short camera group");
        }
        let (key, count) = (b.u32()?, b.u32()? as usize);
        // The count from the file reserves no more than the bytes can
        // hold (fuzzing: a lying count asked for gigabytes).
        let mut tracks = Vec::with_capacity(count.min(g.body.len() / TRACK_HEADER));
        for _ in 0..count {
            let Ok(h) = b.sub(TRACK_HEADER) else { bail!("truncated camera track") };
            let n = h.u8_at(0x10)? as usize;
            let Ok(mut body) = b.sub(n * ENTRY) else { bail!("truncated camera entries") };
            tracks.push(CameraTrack {
                name: h.cstr_at(0x12, 0x0E)?,
                duration: h.f32_at(0x0C)?,
                entries: body.records("camera entries", n, ENTRY, usize::MAX)?.map(|e| parse_entry(Reader::new(e))).collect::<FormatResult<_>>()?,
            });
        }
        out.push(CameraGroup { kind, key, tracks });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_built_group() {
        let mut entry = [0u8; ENTRY];
        entry[0] = 0x13;
        entry[2] = 1;
        entry[0x0C..0x10].copy_from_slice(&0.25f32.to_le_bytes());
        entry[0x48..0x4C].copy_from_slice(&50f32.to_le_bytes());
        let mut header = [0u8; TRACK_HEADER];
        header[0x0C..0x10].copy_from_slice(&1f32.to_le_bytes());
        header[0x10] = 1;
        header[0x12..0x19].copy_from_slice(b"Track 0");
        let mut group = Vec::new();
        group.extend_from_slice(&0x0001_00D1u32.to_le_bytes());
        group.extend_from_slice(&1u32.to_le_bytes());
        group.extend_from_slice(&header);
        group.extend_from_slice(&entry);
        let mut body = Vec::new();
        body.extend_from_slice(&GROUP.to_le_bytes());
        body.extend_from_slice(&(group.len() as u32).to_le_bytes());
        body.extend_from_slice(&group);
        let groups = parse_camera_tracks(Chunk { id: ROUTE_CAMERAS, body: &body, offset: 8 }).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].route_key(), (0xD1, 1));
        let t = &groups[0].tracks[0];
        assert_eq!((t.name.as_str(), t.duration, t.entries.len()), ("Track 0", 1.0, 1));
        let e = &t.entries[0];
        assert!(e.mover == 0x13 && e.spline && !e.blend);
        assert_eq!((e.start, e.fov[0]), (0.25, 50.0));
    }
}
