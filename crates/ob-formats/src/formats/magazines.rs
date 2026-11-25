//! FRONTEND/MAGAZINES_FRONTEND.BIN chunk 0x30230 (loaded by 0x5108A0 into
//! 0x838570 = the chunk's data): the photo-shoot cameras and the magazine
//! setups UI_MagazineView builds its cover from (0x523D10).
//!
//! * +0x00 u32 camera count (37), +0x04 u32 environment-map count (3),
//!   +0x08 u32 setup count (16); cameras from +0x0C, 0x38 bytes each:
//!   u32 name hash, u8 flags +0x04 (bits 1 / 2 / 4 / 8 switch the FE car
//!   lights 0, 2, 3, 1 on, 0x4C1860), f32 +0x08 / +0x0C to the FE car
//!   object (0x4C2070: +0x274 / +0x254 and +0x25C), then ten floats to
//!   the FE camera mover (0x4A5B70 → 0x451200): +0x10 → mover +0x100,
//!   +0x14..+0x1C → +0x110 (the point looked at), +0x20..+0x28 → +0x120
//!   (the eye), +0x2C / +0x30 → +0x130 / +0x134, +0x34 → +0x138 (the
//!   roll). The mover blends from the current values (0x44CE00, state 0,
//!   a cosine ease) and its view (0x444DE0) looks from the eye at the
//!   point with up −z negated (forward = normalize(point − eye), right
//!   from (0, 0, −1) × forward), rolled about forward by the roll in
//!   degrees when it is over 1e-5.
//! * Setups from +0x110C, 0x68 bytes each: u32 hash (bin_hash
//!   "MAGAZINE_SETUP_<MAG>", compared with the cover 0x837FD8 by 0x510790;
//!   the first is the fallback), +0x08..+0x1C six counts, then the choice
//!   lists (0x5105C0 picks one of each, at random over the count when it
//!   is 2 or more, 0x43C1C0): +0x20 backgrounds ×3, +0x2C mastheads ×2,
//!   +0x34 ×5, +0x48 ×3, +0x54 cameras ×3 (by name hash), +0x60
//!   environment maps ×2 (by hash). The first four are
//!   texture hashes in FRONTEND/MAGAZINES_STREAMING_*.bin: `<MAG>_BG_nn`,
//!   `<MAG>_MASTHEAD_nn`, the model (`<MAG>_<NAME>_nn`) and `<MAG>_XT_nn`.
//! * Environment maps from +0xE0C (after 64 camera slots), 0x18 bytes
//!   each (0x510750 finds one by hash): u32 hash, u32 a light material
//!   hash (0x4A7990 sets it on FE car 0, render info +0x904), then the
//!   name (16 bytes) of FRONTEND\ENVMAPS\<name>.BIN: Showroom (twice,
//!   with different light materials) and ElaborareGT.

use glam::Vec3;

use crate::formats::reader::{FormatResult, Reader};

pub const CHUNK: u32 = 0x30230;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhotoCamera {
    pub hash: u32,
    pub flags: u8,
    /// +0x08 / +0x0C, set on the FE car object (0x4C2070).
    pub car: [f32; 2],
    /// +0x10 → mover +0x100.
    pub lens: f32,
    /// +0x14: the point looked at, car space.
    pub target: Vec3,
    /// +0x20: the eye, car space.
    pub eye: Vec3,
    /// +0x2C / +0x30 → mover +0x130 / +0x134.
    pub extra: [f32; 2],
    /// +0x34: roll about the view direction, degrees.
    pub roll: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MagazineSetup {
    pub hash: u32,
    /// The layer choices: backgrounds (+0x20 ×3), mastheads (+0x2C ×2),
    /// models (+0x34 ×5), extras (+0x48 ×3), as stored (unused slots 0).
    pub layers: [Vec<u32>; 4],
    /// The camera choices (+0x54, `counts[4]` of them).
    pub cameras: Vec<u32>,
    /// The six counts at +0x08.
    pub counts: [u32; 6],
    /// The environment map choices (+0x60, `counts[5]` of them).
    pub envmaps: Vec<u32>,
}

/// An environment map entry (+0xE0C, 0x18 bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct EnvMap {
    pub hash: u32,
    /// +0x04: the FE car's light material (0x4A7990 → 0x48A2D0).
    pub light: u32,
    /// +0x08: the FRONTEND\ENVMAPS file name.
    pub name: String,
}

/// The cube faces' texture names in face order (0x5B9920: D3D faces
/// +X, −X, +Y, −Y, +Z, −Z of the static cube 0x87088C).
pub const ENVMAP_FACES: [&str; 6] = ["XPOS", "XNEG", "YPOS", "YNEG", "ZPOS", "ZNEG"];

/// The six faces of FRONTEND\ENVMAPS\<name>.BIN (a texture pack
/// `...\EnvironmentMaps\<name>.tpk`), in [`ENVMAP_FACES`] order; `None`
/// unless all six are there.
pub fn load_envmap(root: &std::path::Path, name: &str) -> Option<Vec<crate::formats::texture::Texture>> {
    // The game opens "%s.BIN"; the files are "<name>.bin" (Windows paths
    // ignore case).
    let dir = crate::game_fs::path(root, "FRONTEND/ENVMAPS");
    let want = format!("{name}.bin");
    let file = std::fs::read_dir(&dir).ok()?.flatten().map(|e| e.path()).find(|p| p.file_name().is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case(&want)))?;
    let data = crate::game_fs::read_file(&file).ok()?;
    let mut faces: [Option<crate::formats::texture::Texture>; 6] = Default::default();
    crate::formats::bundle::walk(crate::formats::bundle::chunks(&data, 0), &mut |c| {
        if c.id == 0xB330_0000
            && let Ok(ts) = crate::formats::texture::parse_texture_pack(c)
        {
            for t in ts {
                if let Some(i) = ENVMAP_FACES.iter().position(|f| crate::formats::hash::bin_hash(f) == t.hash) {
                    faces[i].get_or_insert(t);
                }
            }
        }
    });
    faces.into_iter().collect()
}

#[derive(Clone, Debug, Default)]
pub struct Magazines {
    pub cameras: Vec<PhotoCamera>,
    pub setups: Vec<MagazineSetup>,
    pub envmaps: Vec<EnvMap>,
}

impl Magazines {
    /// Parses the chunk's data (after its 8-byte header).
    pub fn parse(data: &[u8]) -> Option<Magazines> {
        let data = Reader::new(data);
        if data.len() < 12 {
            return None;
        }
        let count = data.u32_at(0).ok()? as usize;
        let envmaps = data.u32_at(4).ok()? as usize;
        let setups = data.u32_at(8).ok()? as usize;
        if count > 64 || envmaps > 32 || data.len() < 0x110C + setups * 0x68 {
            return None;
        }
        let v = |o: usize| data.vec3_at(o).map(Vec3::from);
        let cameras = (0..count)
            .map(|i| {
                let o = 0x0C + i * 0x38;
                Ok(PhotoCamera {
                    hash: data.u32_at(o)?,
                    flags: data.u8_at(o + 4)?,
                    car: [data.f32_at(o + 0x08)?, data.f32_at(o + 0x0C)?],
                    lens: data.f32_at(o + 0x10)?,
                    target: v(o + 0x14)?,
                    eye: v(o + 0x20)?,
                    extra: [data.f32_at(o + 0x2C)?, data.f32_at(o + 0x30)?],
                    roll: data.f32_at(o + 0x34)?,
                })
            })
            .collect::<FormatResult<_>>()
            .ok()?;
        let setups = (0..setups)
            .map(|i| {
                let o = 0x110C + i * 0x68;
                let mut counts = [0u32; 6];
                for (k, c) in counts.iter_mut().enumerate() {
                    *c = data.u32_at(o + 8 + k * 4)?;
                }
                let list = |at: usize, n: usize| (0..n).map(|k| data.u32_at(o + at + k * 4)).collect::<FormatResult<Vec<u32>>>();
                let cameras = list(0x54, counts[4].min(3) as usize)?;
                let layers = [list(0x20, 3)?, list(0x2C, 2)?, list(0x34, 5)?, list(0x48, 3)?];
                let envmaps = list(0x60, counts[5].min(2) as usize)?;
                Ok(MagazineSetup { hash: data.u32_at(o)?, layers, cameras, counts, envmaps })
            })
            .collect::<FormatResult<_>>()
            .ok()?;
        let envmaps = (0..envmaps)
            .map(|i| {
                let o = 0xE0C + i * 0x18;
                let name = data.bytes_at(o + 8, 0x10)?;
                let name = String::from_utf8_lossy(&name[..name.iter().position(|&b| b == 0).unwrap_or(16)]).into_owned();
                Ok(EnvMap { hash: data.u32_at(o)?, light: data.u32_at(o + 4)?, name })
            })
            .collect::<FormatResult<_>>()
            .ok()?;
        Some(Magazines { cameras, setups, envmaps })
    }

    /// Reads the chunk from FRONTEND/MAGAZINES_FRONTEND.BIN.
    pub fn load(root: &std::path::Path) -> Option<Magazines> {
        let file = crate::game_fs::read(root, "FRONTEND/MAGAZINES_FRONTEND.BIN").ok()?;
        let r = Reader::new(&file);
        if r.u32_at(0).ok()? != CHUNK {
            return None;
        }
        let len = r.u32_at(4).ok()? as usize;
        Magazines::parse(r.bytes_at(8, len).ok()?)
    }

    /// The setup for a cover (0x510790), else the first.
    pub fn setup(&self, cover: u32) -> Option<&MagazineSetup> {
        self.setups.iter().find(|s| s.hash == cover).or(self.setups.first())
    }

    /// The setup's camera choice `pick` (0x5105C0: index 0 with fewer
    /// than two, else the random pick over the count).
    pub fn camera(&self, cover: u32, pick: u32) -> Option<&PhotoCamera> {
        let setup = self.setup(cover)?;
        let n = setup.cameras.len() as u32;
        let i = if n < 2 { 0 } else { pick % n };
        let hash = *setup.cameras.get(i as usize)?;
        self.cameras.iter().find(|c| c.hash == hash)
    }

    /// What 0x5105C0 picks for `cover` with the game's generator: each of
    /// the four layer lists draws only with a count of 2 or more (else its
    /// first slot, which may be 0 = none), the camera list whenever its
    /// count is not 0, then the environment map list likewise.
    pub fn pick(&self, cover: u32, rng: &mut crate::game_rng::GameRng) -> Option<CoverPicks> {
        let setup = self.setup(cover)?;
        let mut layers = [0; 4];
        for (k, layer) in layers.iter_mut().enumerate() {
            let n = setup.counts[k];
            let i = if n >= 2 { rng.below(n) } else { 0 };
            *layer = setup.layers[k].get(i as usize).copied().unwrap_or(0);
        }
        let n = setup.counts[4];
        let pick = if n == 0 { 0 } else { rng.below(n) };
        let envmap = if setup.counts[5] != 0 { rng.below(setup.counts[5]) } else { 0 };
        let envmap = setup.envmaps.get(envmap as usize).copied().unwrap_or(0);
        let hash = *setup.cameras.get(pick as usize).or(setup.cameras.first())?;
        let camera = *self.cameras.iter().find(|c| c.hash == hash)?;
        Some(CoverPicks { layers, camera, envmap })
    }

    /// The environment map entry of a hash (0x510750; none for 0 or an
    /// unknown hash, when 0x523D10 loads nothing).
    pub fn envmap(&self, hash: u32) -> Option<&EnvMap> {
        self.envmaps.iter().find(|e| e.hash == hash)
    }
}

/// FRONTEND/MAGAZINES_SHOWCASE.BIN chunk 0x30231 (loaded by 0x499EC0 when
/// UI_ShowCase_Preview opens, 0x5108E0 → 0x838574 = the chunk's data): the
/// in-game photo shoot's camera set.
///
/// * +0x00 u32 camera count (42), +0x04 u32 count of the +0xB0C list (1),
///   +0x08 u32 setup count (14).
/// * Cameras from +0x0C, 0x2C bytes (0x5107D0 finds one by hash): u32 name
///   hash, u32 0, then nine floats 0x4B5730 hands the showcase camera
///   mover (0x4463C0 → mover +0x2C, +0x30, +0x34, +0x40, +0x44, +0x48,
///   +0x50, +0x54, +0x58; see `nfsu2::chase::showcase_view`).
/// * +0xB0C, 0xC bytes each (0x510800 by hash; 0x4B57B0 drops its answer):
///   one entry, 0xC2082D5F.
/// * Setups from +0xC8C, 0x38 bytes (0x510840 by the cover 0x837FD8, else
///   the first, 0x510880(0)): u32 the cover (bin_hash
///   `MAGAZINE_SETUP_<MAG>`), u32 0, four counts at +0x08, then the lists
///   0x510680 picks from: +0x18 mastheads ×2, +0x20 extras ×1, +0x24
///   cameras ×3 (by name hash), +0x30 ×2 (the +0xB0C hashes).
pub const SHOWCASE_CHUNK: u32 = 0x30231;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShowcaseCamera {
    pub hash: u32,
    /// +0x08..+0x28 → mover +0x2C (yaw, degrees), +0x30 (pitch, degrees),
    /// +0x34 (distance), +0x40..+0x48 (the point looked at, in the car's
    /// ground frame), +0x50 (field of view, degrees), +0x54 / +0x58 (the
    /// camera's focus +0xB4 / +0xB8).
    pub params: [f32; 9],
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShowcaseSetup {
    pub cover: u32,
    /// +0x08..+0x14: the counts of the four lists.
    pub counts: [u32; 4],
    pub mastheads: [u32; 2],
    pub extras: [u32; 1],
    pub cameras: [u32; 3],
    pub lights: [u32; 2],
}

/// What 0x510680 picks into the screen's +0x80..+0x90.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShowcasePicks {
    /// +0x80: the masthead texture (0x523BD0's second layer).
    pub masthead: u32,
    /// +0x84: the extra texture (its fourth).
    pub extra: u32,
    /// +0x88 / +0x8C: the camera's hash and its index in the setup.
    pub camera: u32,
    pub index: u32,
    /// +0x90: the +0x30 pick.
    pub light: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Showcase {
    pub cameras: Vec<ShowcaseCamera>,
    pub setups: Vec<ShowcaseSetup>,
}

impl Showcase {
    /// Parses the chunk's data (after its 8-byte header).
    pub fn parse(data: &[u8]) -> Option<Showcase> {
        let data = Reader::new(data);
        if data.len() < 0xC8C {
            return None;
        }
        let (n, setups) = (data.u32_at(0).ok()? as usize, data.u32_at(8).ok()? as usize);
        if n > 64 || data.len() < 0xC8C + setups * 0x38 {
            return None;
        }
        let cameras = (0..n)
            .map(|i| {
                let o = 0xC + i * 0x2C;
                let mut params = [0f32; 9];
                for (k, p) in params.iter_mut().enumerate() {
                    *p = data.f32_at(o + 8 + k * 4)?;
                }
                Ok(ShowcaseCamera { hash: data.u32_at(o)?, params })
            })
            .collect::<FormatResult<_>>()
            .ok()?;
        let setups = (0..setups)
            .map(|i| {
                let o = 0xC8C + i * 0x38;
                let u = |at: usize| data.u32_at(o + at);
                Ok(ShowcaseSetup {
                    cover: u(0)?,
                    counts: [u(8)?, u(12)?, u(16)?, u(20)?],
                    mastheads: [u(0x18)?, u(0x1C)?],
                    extras: [u(0x20)?],
                    cameras: [u(0x24)?, u(0x28)?, u(0x2C)?],
                    lights: [u(0x30)?, u(0x34)?],
                })
            })
            .collect::<FormatResult<_>>()
            .ok()?;
        Some(Showcase { cameras, setups })
    }

    pub fn load(root: &std::path::Path) -> Option<Showcase> {
        let file = crate::game_fs::read(root, "FRONTEND/MAGAZINES_SHOWCASE.BIN").ok()?;
        let r = Reader::new(&file);
        if r.u32_at(0).ok()? != SHOWCASE_CHUNK {
            return None;
        }
        let len = r.u32_at(4).ok()? as usize;
        Showcase::parse(r.bytes_at(8, len).ok()?)
    }

    /// 0x4B57B0's set: the cover's (0x510840), else the first (0x510880(0)).
    pub fn setup(&self, cover: u32) -> Option<usize> {
        self.setups.iter().position(|s| s.cover == cover).or(if self.setups.is_empty() { None } else { Some(0) })
    }

    /// 0x510680 with the game's generator: each list draws only with a
    /// count of 2 or more (the cameras' and +0x30's with any count not 0),
    /// else its first slot.
    pub fn pick(&self, setup: usize, rng: &mut crate::game_rng::GameRng) -> ShowcasePicks {
        let Some(s) = self.setups.get(setup) else { return ShowcasePicks::default() };
        let masthead = s.mastheads[if s.counts[0] < 2 { 0 } else { rng.below(s.counts[0]) as usize }.min(1)];
        let extra = s.extras[if s.counts[1] < 2 { 0 } else { rng.below(s.counts[1]) as usize }.min(0)];
        let index = if s.counts[2] == 0 { 0 } else { rng.below(s.counts[2]) };
        let camera = s.cameras[(index as usize).min(2)];
        let light = s.lights[if s.counts[3] == 0 { 0 } else { rng.below(s.counts[3]) as usize }.min(1)];
        ShowcasePicks { masthead, extra, camera, index, light }
    }

    /// Cycle Cameras (0x510710): the setup's next camera, wrapping to the
    /// first after the last (count +0x10).
    pub fn next_camera(&self, setup: usize, picks: &mut ShowcasePicks) {
        let Some(s) = self.setups.get(setup) else { return };
        let last = s.counts[2] as i64 - 1;
        let i = picks.index as i64;
        picks.index = if i < last { (i + 1) as u32 } else if i == last { 0 } else { picks.index };
        picks.camera = s.cameras[(picks.index as usize).min(2)];
    }

    /// 0x5107D0: a camera by hash.
    pub fn camera(&self, hash: u32) -> Option<&ShowcaseCamera> {
        self.cameras.iter().find(|c| c.hash == hash)
    }
}

/// The cover layers' textures, by hash, from the streaming pack
/// FRONTEND/MAGAZINES_STREAMING_HIGH.bin (0x523BD0 streams each non-zero
/// pick by hash with 0x494D30 through the stream object 0x863780, whose
/// pack is table 0x7FA294[+0]: HIGH, then LOW; its only constructor call,
/// 0x565888, passes 0 — the LOW one, index 1, is 0x8364F4's, 0x499E6C).
pub fn load_layers(root: &std::path::Path, hashes: &[u32]) -> Vec<crate::formats::texture::Texture> {
    load_layers_from(root, "MAGAZINES_STREAMING_HIGH.bin", hashes)
}

/// [`load_layers`] from one pack of table 0x7FA294: HIGH (index 0) or LOW
/// (index 1, the in-game showcase's: 0x499E60 → 0x510940(1, 1) on the
/// stream object 0x8364F4, whose mastheads only LOW holds).
pub fn load_layers_from(root: &std::path::Path, pack: &str, hashes: &[u32]) -> Vec<crate::formats::texture::Texture> {
    let mut found = Vec::new();
    for file in [pack] {
        let Ok(data) = crate::game_fs::read(root, &format!("FRONTEND/{file}")) else { continue };
        crate::formats::bundle::walk(crate::formats::bundle::chunks(&data, 0), &mut |c| {
            if c.id == 0xB330_0000
                && let Ok(ts) = crate::formats::texture::parse_texture_pack(c)
            {
                for t in ts {
                    if t.hash != 0 && hashes.contains(&t.hash) && !found.iter().any(|f: &crate::formats::texture::Texture| f.hash == t.hash) {
                        found.push(t);
                    }
                }
            }
        });
    }
    found
}

/// A cover's picks (0x5105C0 into UI_MagazineView +0x54..+0x68): the
/// background, masthead, model and extra textures (0 for none) and the
/// photo-shoot camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoverPicks {
    pub layers: [u32; 4],
    pub camera: PhotoCamera,
    /// The environment map's hash (+0x68).
    pub envmap: u32,
}

impl PhotoCamera {
    /// The view's up vector (0x444DE0): right = (0, 0, −1) × forward, up =
    /// forward × right, so the image's up is world +z; the roll turns it
    /// about forward.
    pub fn up(&self) -> Vec3 {
        let forward = (self.target - self.eye).normalize_or(Vec3::X);
        let right = Vec3::new(0.0, 0.0, -1.0).cross(forward).normalize_or(Vec3::Y);
        let down = forward.cross(right).normalize_or(-Vec3::Z);
        let up = -down;
        if self.roll > 1e-5 {
            glam::Quat::from_axis_angle(forward, self.roll.to_radians()) * up
        } else {
            up
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_rng_steps_from_the_games_seed() {
        // 0x43C1C0 from 0x12345678: 0x12345678 % 3, then the xor-shift.
        let mut r = crate::game_rng::GameRng::default();
        assert_eq!([r.below(3), r.below(5), r.below(7)], [0, 1, 3]);
        assert_eq!(r.0, 0x938B_47F6);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn magazines_showcase_has_42_cameras_and_14_setups() {
        let s = Showcase::load(&crate::testkit::game_root()).expect("the showcase");
        assert_eq!((s.cameras.len(), s.setups.len()), (42, 14));
        // MAGAZINE_SETUP_SC: two mastheads, no extra, three cameras.
        let sc = s.setup(crate::formats::hash::bin_hash("MAGAZINE_SETUP_SC")).unwrap();
        assert_eq!(sc, 6);
        let st = &s.setups[sc];
        assert_eq!((st.counts, st.mastheads), ([2, 0, 3, 1], [0x3C52_66FB, 0x3C52_66FC]));
        for setup in &s.setups {
            assert!(setup.cameras.iter().all(|h| s.camera(*h).is_some()));
        }
        // Their mastheads are in the LOW pack.
        let t = load_layers_from(&crate::game_root(), "MAGAZINES_STREAMING_LOW.bin", &st.mastheads);
        assert_eq!(t.len(), 2);
        // Cycle Cameras wraps.
        let mut p = ShowcasePicks { index: 2, ..Default::default() };
        s.next_camera(sc, &mut p);
        assert_eq!((p.index, p.camera), (0, st.cameras[0]));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn magazines_frontend_has_37_cameras_and_16_setups() {
        let m = Magazines::load(&crate::testkit::game_root()).expect("the magazines");
        assert_eq!(m.cameras.len(), 37);
        assert_eq!(m.setups.len(), 16);
        // The first camera: 0x557ECB15, flag 8, eye (−0.06, −4, 2.33).
        let c = m.cameras[0];
        assert_eq!((c.hash, c.flags), (0x557E_CB15, 8));
        assert!((c.eye - Vec3::new(-0.06, -4.0, 2.332)).length() < 0.01);
        // Every setup's camera choices are cameras of the table.
        for s in &m.setups {
            assert!(!s.cameras.is_empty());
            assert!(s.cameras.iter().all(|h| m.cameras.iter().any(|c| c.hash == *h)), "{:08X}", s.hash);
        }
        // Every stored layer is a texture of the streaming packs (CAE: three
        // backgrounds, two mastheads, three models, one extra).
        let cae = m.setups.iter().find(|s| s.hash == crate::formats::hash::bin_hash("MAGAZINE_SETUP_CAE")).unwrap();
        assert_eq!(cae.counts, [3, 2, 3, 3, 2, 1]);
        // Three environment maps; every setup names one of them, and the
        // files hold the six faces.
        let names: Vec<&str> = m.envmaps.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Showroom", "ElaborareGT", "Showroom"]);
        assert!(m.setups.iter().all(|s| s.envmaps.len() == 1 && m.envmap(s.envmaps[0]).is_some()));
        assert_eq!(m.envmap(cae.envmaps[0]).map(|e| e.name.as_str()), Some("Showroom"));
        for e in &m.envmaps {
            let faces = load_envmap(&crate::game_root(), &e.name).expect("six faces");
            assert!(faces.iter().all(|f| f.width == f.height && f.width == faces[0].width), "{}", e.name);
        }
        assert_eq!(cae.layers[0][0], crate::formats::hash::bin_hash("CAE_BG_01"));
        assert_eq!(cae.layers[1][1], crate::formats::hash::bin_hash("CAE_MASTHEAD_02"));
        assert_eq!(cae.layers[3][0], crate::formats::hash::bin_hash("CAE_XT_01"));
        let picked = load_layers(&crate::game_root(), &cae.layers[0]);
        assert_eq!(picked.len(), 3);
        // An unknown cover falls back to the first setup.
        assert_eq!(m.setup(0).map(|s| s.hash), m.setups.first().map(|s| s.hash));
    }
}
