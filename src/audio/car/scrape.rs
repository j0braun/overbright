//! The collision objects' scrape (FX_Scrape, the second event of
//! `SOUND/IG_GLOBAL/FX_Misc_MB.abk`), traced in SPEED2.EXE (verified
//! unless marked).
//!
//! * **The sliding speed.** The track / car impulse (0x5AF0F0) takes the
//!   contact's relative velocity less its part along the normal (0x43D0C0)
//!   and passes that tangent's length — the sliding speed, which also
//!   ramps the friction in below 1 m/s (0x7FBF30) — as the third argument
//!   of the push 0x5A9630, which keeps it in the record's +0x40 (0x5A9667)
//!   (`drive::slide_speed`).
//! * **Arming.** Every push within 50 m of the view and 100 m of the
//!   listener runs, after the impact's first update (0x4814E0), 0x4792D0 →
//!   0x469DE0 on the audio manager: the record's sound object (+0x24 = the
//!   record), when neither scraping (+0x134) nor done (+0x135), asks its
//!   slot +0x18. Track (vtable 0x78BEE0) and car–car (0x78BE90) objects
//!   use 0x459890: record +0x40 ≥ the curve +0xA4's x0 — the curve is
//!   0x82BD40 {y0 0, y1 1, x0 0, x1 40} (static init at 0x469F89 from
//!   0x7884D4..0x7884E0), so any push passes; props (0x78BB08) use 0x404800
//!   (false: no prop scrape). A pass sets +0x22.
//! * **Start.** The object's update (0x481F10, once its first update has
//!   run, +0x18) with +0x22 runs 0x46A460 and clears it: +0x100 = the
//!   point, +0x134 = 1, a voice from the manager's pool (+0xBC; none: back
//!   to not scraping and done), then by the class +0x120 (0x459680: 0
//!   track, 1 car–car, 2 prop): class 1 returns there, so **two cars never
//!   start FX_Scrape** (the object only stays "scraping"); classes 0 / 2
//!   name it "SND_WORLDScrape" / "SND_SmokeableScrape" (0x7EC844, a debug
//!   name), +0xE0 = +0x54, +0xD4 = +0xDC = +0xD8 = 0, and slot +0x2C
//!   (0x4596E0) makes the block (0x458010, record 0x7EDDF0, slot 0x82C5D8)
//!   {+0xD4 (0..0x7FFF), +0xD8 (0..9000), +0xDC (0..0xFFFF), +0xD0 (0..16),
//!   +0xE0 (0..0x7F)}. +0xD0 is 0 from the constructor (0x46A0D2: two
//!   blocks {0, 0x7FFF, 0x1000, 0, 0} at +0xBC and +0xD0, the second
//!   overwritten as above).
//! * **Per frame** (0x479970, while scraping): +0x54 = trunc(127 ·
//!   0x4599E0(curve 0x82BD40, record +0x40)) = trunc(127 · min(slide / 40,
//!   1)) (slot +8, 0x479AD0; 0x78B104 = 127), +0xE0 = +0x54; during the
//!   fade +0xD4 = trunc(fade · +0xD4); 0x459770 sends {+0xD4, +0xD8, +0xDC,
//!   (+0xD0 kept), +0xE0} clamped.
//! * **Stop.** The update marks the object done (0x46A1A0) once the record
//!   is gone or its last push is 0.3 s old (0x4595D0, 0x7FBEE0): a scraping
//!   object starts its fade ramp +0x58 (0x46B100: 0.1 s, 1 → 0, linear).
//!   When the ramp ends 0x46A3A0 stops the event (slot +0x38, 0x46A400) and
//!   clears +0x134.
//!
//! * **Level and azimuth (verified).** +0xD4 / +0xD8 / +0xDC are
//!   written apart from the object's update, by its mix-map client
//!   SFXObj_Collision (id 0x50000, factory 0x47E220, ctor 0x474C60,
//!   vtable 0x78B078). Its slot 0x28 (0x461CE0) takes the sound object
//!   ([client +0xB0] +0x44, attached by 0x461AE0) and writes +0xD4 =
//!   slot 0x38(9) (0x45E390: [+0x58 + 4·9] & 0x7FFF), +0xD8 = 0x1000
//!   (0x45E3D0; 0 while the audio manager's +0x14 pause is set), and,
//!   once (while the client's +0xBC is set), +0xDC = slot 0x3C(9) (the
//!   azimuth word); the impact block gets slot (+0x124 kind 1..8) at
//!   +0xC0 / +0xC8 the same way. Its field update 0x461B60 copies the
//!   sound object's point +0x100 to the client's +0xD0, which its
//!   SFXCTL_3DScrapePos (0x50010, vtable 0x78AB30, slot 0x2C) reads as the
//!   3D position (0x45E100 → 0x45DF00). In the mix map slot 9 of section 5
//!   is master 1: −600 mB + sub channel 0 (section 0's hold and speech
//!   duck, the effects volume) + the 3D control 1 (4..30 m by the distance
//!   to the player's car in view 0). So a scrape by the player's car plays
//!   at −604 mB (16365), −1501 (5812, read under Wine) while a
//!   speech line ducks the world by 900 mB, and fades out by 30 m. The
//!   clone runs one section-5 copy per live object (`Scrapes::points`,
//!   `mixmap::Scene::collisions`).
//!
//! The earlier reading that the scrape is silent (nothing writes +0xD4
//! after 0x46A460) was wrong: memory reads under Wine refuted it and the writer
//! is the mix client above.
//!
// prov: inferred audio.scrape.module
//! **Inferred**: the mix pass reaching a new object from its second frame
//! (the start frame sends the zeros of 0x46A460), the client's +0xBC set
//! once per object (so +0xDC is its first mix azimuth; observed constant
//! per instance under Wine), one sound object per contact record without the
//! manager's 10-object cap and voice pool (as `collision`), +0x54 fresh at
//! the start (0x46A460 runs slot +8 when the impact already has a voice,
//! else +0x54 is still the constructor's 0 for one frame), the 100 m
//! listener gate taken from the impact sounds.

use crate::audio::carfx::{CarFxSounds, EventBank, Sink, close_bank, open_bank, run_bank};
use glam::Vec3;
use std::collections::HashMap;
use std::path::Path;

/// The sound object's class (0x459680).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Track,
    Cars,
}

/// One push on a contact record, as 0x4792D0 sees it.
#[derive(Clone, Copy, Debug)]
pub struct Push {
    /// The record (unique while it lives).
    pub record: u64,
    pub class: Class,
    /// Record +0x40: the sliding speed, m/s.
    pub slide: f32,
    pub at: Vec3,
}

/// The pushes posted by the physics, waiting for the audio (bounded: the
/// oldest drop): a session's own queue (`drive::ContactSounds`),
/// no longer one process-wide queue.
#[derive(Clone, Debug, Default)]
pub struct Pushes(Vec<Push>);

impl Pushes {
    /// A push on a record (0x5A9630 → 0x4792D0).
    pub fn post(&mut self, push: Push) {
        let p = &mut self.0;
        if p.len() >= 256 {
            p.remove(0);
        }
        p.push(push);
    }

    /// Take the posted pushes.
    pub fn drain(&mut self) -> Vec<Push> {
        std::mem::take(&mut self.0)
    }

    /// The pushes posted, oldest first.
    pub fn as_slice(&self) -> &[Push] {
        &self.0
    }
}

/// The level FX_Scrape's p4 takes (0x479AD0): trunc(127 · ramp 0x82BD40).
pub fn level(slide: f32) -> i32 {
    (127.0 * crate::audio::roadwind::ramp(0.0, 1.0, 0.0, 40.0, slide)) as i32
}

/// A record's last push this long ago marks its sound object done
/// (0x4595D0, 0x7FBEE0).
pub const RECORD_LIFE: f32 = 0.3;
/// The fade ramp (0x46A1A0 → +0x58), seconds.
pub const FADE: f32 = 0.1;

#[derive(Clone, Debug)]
struct Object {
    class: Class,
    slide: f32,
    /// Seconds since the record's last push.
    age: f32,
    armed: bool,
    scraping: bool,
    done: bool,
    /// The fade ramp's time (+0x58) once done while scraping.
    fade: Option<f32>,
    event: Option<u32>,
    /// {+0xD4, +0xD8, +0xDC, +0xD0, +0xE0}.
    block: [i32; 5],
    /// The record's point at its last push (+0x30 → +0x100, 0x46A540 /
    /// 0x479970).
    at: Vec3,
    /// SFXObj_Collision's slot 9 this frame (the packed level / azimuth
    /// dword, `Scrapes::set_mix`); none: no mix ran.
    mix: Option<u32>,
    /// +0xDC is taken from the mix once (0x461CE0 with the client's +0xBC).
    az_set: bool,
}

/// The scrapes of the live collision objects.
pub struct Scrapes {
    sounds: CarFxSounds,
    gains: HashMap<u32, (f32, f32, f32)>,
    bank: Option<EventBank>,
    /// The live objects by collision record, in the order they were
    /// made: the manager walks its object list (+0x24, next at +0x10,
    /// 0x469DE0) in list order; that a new object goes to the list's end
    // prov: inferred audio.scrape.list_order
    /// is **inferred**. A hash map's order differed run to run, so two
    /// scrapes starting in one frame took their voices in either order.
    objects: Vec<(u64, Object)>,
    /// FX_Scrape events started this frame (for the self-test).
    pub started: usize,
    /// The FX_Camera event (the manager's +0x9C, 0x45E450).
    camera: Option<u32>,
}

const BANK: &str = "SOUND/IG_GLOBAL/FX_Misc_MB.abk";

impl Scrapes {
    pub fn new(root: &Path) -> Scrapes {
        Scrapes {
            sounds: CarFxSounds::new(root),
            gains: HashMap::new(),
            bank: None,
            objects: Vec::new(),
            started: 0,
            camera: None,
        }
    }

    /// The live objects' records and points, in a fixed order: the
    /// collision section's copies (`mixmap::Scene::collisions`).
    pub fn points(&self) -> Vec<(u64, [f32; 2])> {
        let mut v: Vec<(u64, [f32; 2])> = self.objects.iter().map(|(k, o)| (*k, [o.at.x, o.at.y])).collect();
        v.sort_unstable_by_key(|e| e.0);
        v
    }

    /// Each object's SFXObj_Collision slot 9 (`Mix::packed(5, copy, 0, 9)`).
    pub fn set_mix(&mut self, slots: &[(u64, Option<u32>)]) {
        for &(k, m) in slots {
            if let Some((_, o)) = self.objects.iter_mut().find(|e| e.0 == k) {
                o.mix = m;
            }
        }
    }

    #[cfg(test)]
    fn obj(&self, k: u64) -> &Object {
        &self.objects.iter().find(|e| e.0 == k).expect("no such object").1
    }

    /// Objects scraping now (both classes).
    pub fn scraping(&self) -> usize {
        self.objects.iter().filter(|o| o.1.scraping).count()
    }

    pub fn silence(&mut self, sink: &mut dyn Sink) {
        close_bank(&mut self.bank, &mut self.sounds, &mut self.gains, sink);
        self.objects.clear();
        self.camera = None;
    }

    /// The camera shutter (0x45E450, run by the audio manager while its
    /// +0x08 flag is set; ShowcaseCamera_Shutter 0x501250 sets it through
    /// 0x458880 as Take Picture runs): the flag cleared, the last FX_Camera
    /// event (+0x9C) stopped (its vtable +0x04), then FX_Camera (slot
    /// 0x82C72C, record 0x7EDDE8) started with an all-zero block
    /// (+0x04..+0x1C). Whether it played: for the self-test.
    pub fn shutter(&mut self, sink: &mut dyn Sink) -> bool {
        open_bank(&mut self.bank, &mut self.sounds, &mut self.gains, sink, BANK);
        let Some(bank) = self.bank.as_mut() else { return false };
        if let Some(id) = self.camera.take() {
            bank.events.release(id);
        }
        let module = bank.events.module("FX_Camera");
        self.camera = module.and_then(|m| bank.events.play(m, &[0; 7]));
        self.camera.is_some()
    }

    /// One frame: the pushes since the last (0x469DE0), then each object's
    /// update (0x481F10 → 0x46A460, 0x479970, 0x46A3A0).
    pub fn update(&mut self, sink: &mut dyn Sink, pushes: &[Push], dt: f32) {
        self.started = 0;
        open_bank(&mut self.bank, &mut self.sounds, &mut self.gains, sink, BANK);
        for (_, o) in self.objects.iter_mut() {
            o.age += dt;
        }
        for p in pushes {
            let i = match self.objects.iter().position(|e| e.0 == p.record) {
                Some(i) => i,
                None => {
                    self.objects.push((p.record, Object {
                class: p.class,
                slide: 0.0,
                age: 0.0,
                armed: false,
                scraping: false,
                done: false,
                fade: None,
                event: None,
                block: [0x7FFF, 0x1000, 0, 0, 0],
                at: p.at,
                mix: None,
                az_set: false,
            }));
                    self.objects.len() - 1
                }
            };
            let o = &mut self.objects[i].1;
            o.slide = p.slide;
            o.at = p.at;
            o.age = 0.0;
            // 0x459890: slide ≥ x0 (0).
            if !o.scraping && !o.done && p.slide >= 0.0 {
                o.armed = true;
            }
        }
        let Some(bank) = self.bank.as_mut() else { return };
        let ev = &mut bank.events;
        let module = ev.module("FX_Scrape");
        for (_, o) in self.objects.iter_mut() {
            let starting = std::mem::take(&mut o.armed);
            if starting {
                o.scraping = true;
                if o.class == Class::Track {
                    o.block = [0, 0, 0, o.block[3], level(o.slide)];
                    o.event = module.and_then(|m| ev.play(m, &o.block));
                    self.started += o.event.is_some() as usize;
                }
            }
            if !o.done && o.age >= RECORD_LIFE {
                o.done = true;
                if o.scraping {
                    o.fade = Some(0.0);
                }
            }
            if o.scraping {
                // SFXObj_Collision's update (0x461CE0): +0xD4 = slot 9's
                // level, +0xD8 = 0x1000 (0x45E3D0), +0xDC = slot 9's
                // azimuth once; the mixer's pass runs apart from the
                // object's update, so the start frame (0x46A460 zeroes
                // them) sends zeros.
                if let Some(m) = o.mix.filter(|_| !starting) {
                    o.block[0] = (m & 0x7FFF) as i32;
                    o.block[1] = 0x1000;
                    if !o.az_set {
                        o.block[2] = (m >> 16) as i32;
                        o.az_set = true;
                    }
                }
                o.block[4] = level(o.slide);
                if let Some(t) = o.fade.as_mut() {
                    *t += dt;
                    let f = (1.0 - *t / FADE).max(0.0);
                    o.block[0] = (f * o.block[0] as f32) as i32;
                }
                if let Some(id) = o.event {
                    let b = &o.block;
                    ev.set(
                        id,
                        &[
                            b[0].clamp(0, 0x7FFF),
                            b[1].clamp(0, 9000),
                            b[2].clamp(0, 0xFFFF),
                            b[3].clamp(0, 16),
                            b[4].clamp(0, 0x7F),
                        ],
                    );
                }
                if o.fade.is_some_and(|t| t >= FADE) {
                    if let Some(id) = o.event.take() {
                        ev.release(id);
                    }
                    o.scraping = false;
                }
            }
        }
        self.objects.retain(|(_, o)| !(o.done && !o.scraping));
        run_bank(&mut self.bank, &mut self.sounds, &mut self.gains, sink, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_ramps_to_forty() {
        assert_eq!(level(0.0), 0);
        assert_eq!(level(20.0), 63);
        assert_eq!(level(80.0), 127);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn scrape_starts_and_fades() {
        struct Null;
        impl Sink for Null {
            fn one_shot(&mut self, _: &crate::audio::fe::Sample, _: f32, _: u16) {}
            fn start_loop(&mut self, _: &crate::audio::fe::Sample, _: f32, _: u16) -> u32 {
                1
            }
            fn gain(&mut self, _: u32, _: f32) {}
            fn end(&mut self, _: u32) {}
            fn start_once(&mut self, _: &crate::audio::fe::Sample, _: f32, _: u16) -> u32 {
                1
            }
            fn sounding(&mut self, _: u32) -> bool {
                true
            }
            fn pitch(&mut self, _: u32, _: u16) {}
            fn pan(&mut self, _: u32, _: u16) {}
        }
        let root = crate::testkit::game_root();
        let mut s = Scrapes::new(&root);
        let push = |record, class| Push { record, class, slide: 12.0, at: Vec3::ZERO };
        s.update(&mut Null, &[push(1, Class::Track), push(2, Class::Cars)], 0.016);
        assert_eq!(s.started, 1);
        assert_eq!(s.scraping(), 2);
        // The block as coded: volume 0, the level of 12 m/s.
        let o = &s.obj(1);
        assert_eq!(o.block, [0, 0, 0, 0, 38]);
        // 0.3 s without a push, then the 0.1 s fade.
        for _ in 0..40 {
            s.update(&mut Null, &[], 0.016);
        }
        assert_eq!(s.scraping(), 0);
        assert!(s.objects.is_empty());
    }

    /// The mix client's slot 9 reaches the block from the second frame;
    /// the azimuth is taken once; the fade scales the fresh level.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_mix_gives_the_scrape_its_level() {
        struct Null;
        impl Sink for Null {
            fn one_shot(&mut self, _: &crate::audio::fe::Sample, _: f32, _: u16) {}
            fn start_loop(&mut self, _: &crate::audio::fe::Sample, _: f32, _: u16) -> u32 {
                1
            }
            fn gain(&mut self, _: u32, _: f32) {}
            fn end(&mut self, _: u32) {}
            fn start_once(&mut self, _: &crate::audio::fe::Sample, _: f32, _: u16) -> u32 {
                1
            }
            fn sounding(&mut self, _: u32) -> bool {
                true
            }
            fn pitch(&mut self, _: u32, _: u16) {}
            fn pan(&mut self, _: u32, _: u16) {}
        }
        let root = crate::testkit::game_root();
        let mut s = Scrapes::new(&root);
        let push = Push { record: 7, class: Class::Track, slide: 12.0, at: Vec3::new(3.0, 4.0, 0.0) };
        assert_eq!(s.points(), vec![]);
        s.update(&mut Null, &[push], 1.0 / 60.0);
        assert_eq!(s.points(), vec![(7, [3.0, 4.0])]);
        let packed = |l: u32, az: u32| Some(l | az << 16);
        s.set_mix(&[(7, packed(5812, 64584))]);
        s.update(&mut Null, &[push], 1.0 / 60.0);
        assert_eq!(s.obj(7).block, [5812, 4096, 64584, 0, 38]);
        s.set_mix(&[(7, packed(16365, 1000))]);
        s.update(&mut Null, &[push], 1.0 / 60.0);
        assert_eq!(s.obj(7).block, [16365, 4096, 64584, 0, 38]);
        // 0.3 s without a push starts the 0.1 s fade: the fresh level, scaled.
        for _ in 0..18 {
            s.update(&mut Null, &[], 1.0 / 60.0);
        }
        s.set_mix(&[(7, packed(5812, 64584))]);
        s.update(&mut Null, &[], 0.05);
        let b = s.obj(7).block[0];
        assert!(b > 0 && b < 5812, "{b}");
    }
}
