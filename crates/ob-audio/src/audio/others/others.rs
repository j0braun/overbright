use super::ai_engine::Ai;
use super::doppler::Doppler;
use super::horn::{Horn, horn_aims};
use super::traffic_ctx::{Context, TRAFFIC_CONTEXTS, assign};
use super::{AiInput, TrafficInput};
use crate::audio::carfx::{CarFxSounds, EventBank, Sink, close_bank, open_bank, run_bank};
use crate::audio::mixmap::Levels;
use crate::audio::stitch::{StitchBanks, StitchVoice};
use glam::Vec3;
use std::collections::HashMap;
use std::path::Path;

/// The other cars' sounds.
pub struct OtherCars {
    pub(super) sounds: CarFxSounds,
    pub(super) gains: HashMap<u32, (f32, f32, f32)>,
    pub(super) stitches: Option<StitchBanks>,
    /// The AI engines' banks by path, run in path order: a hash map's
    /// order differed run to run, so their voices started in either order
    /// (fixed with the audio clock). The original runs each racer's own engine object in the
    // prov: inferred audio.others.bank_order
    /// racer list's order; a fixed order here is **inferred**, not traced.
    pub(super) banks: std::collections::BTreeMap<String, Option<EventBank>>,
    pub(super) traffic_bank: Option<EventBank>,
    pub(super) ai: Vec<Ai>,
    /// The traffic manager's contexts (its list +0x10, in order).
    pub(super) contexts: Vec<Context>,
    pub(super) ai_counter: i32,
    pub(super) traffic_toggle: i32,
    pub(super) whoosh_counter: u32,
    /// 0x82CB6C: the racers' stereo counter (kept for the session).
    pub(super) stereo_counter: u32,
    pub(super) clock: f32,
    /// Whooshes started this frame (for the self-test).
    pub whooshes: usize,
    /// ENV_COMMON_MB.abk for the horns, and the last horn's start (0x82BC50).
    pub(super) env_bank: Option<EventBank>,
    pub(super) last_horn: f32,
    /// The player's car forward (+0x90) and speed (+0x42C), and whether
    /// the race is a drag (0x89E7D8), for the horns.
    pub player_forward: Vec3,
    pub player_speed: f32,
    pub drag: bool,
    /// The traffic cars of the car table (the clone's pool size): the
    /// contexts are min(it, 6) (0x476AF0).
    pub traffic_cars: usize,
    /// The mix map's levels per traffic context (section 3's copies) and
    /// per AI racer (section 2's).
    pub traffic_levels: Vec<Levels>,
    /// Horns started this frame (for the self-test).
    pub horns: usize,
    /// Traffic bottom-out knocks started this frame.
    pub knocks: usize,
    /// Car stereos started this frame (traffic and racers).
    pub stereos: usize,
    /// Racers whose roll gave them a stereo (for the self-test).
    pub racer_stereos: usize,
}

pub(super) const TRAFFIC: &str = "SOUND/IG_GLOBAL/TRAFFIC_MB.abk";
pub(super) const ENV_COMMON: &str = "SOUND/IG_GLOBAL/ENV_COMMON_MB.abk";

impl OtherCars {
    pub fn new(root: &Path) -> OtherCars {
        OtherCars {
            sounds: CarFxSounds::new(root),
            gains: HashMap::new(),
            stitches: StitchBanks::load(root),
            banks: Default::default(),
            traffic_bank: None,
            ai: Vec::new(),
            contexts: Vec::new(),
            ai_counter: 0,
            traffic_toggle: 0,
            whoosh_counter: 0,
            stereo_counter: 0,
            clock: 0.0,
            whooshes: 0,
            env_bank: None,
            last_horn: 0.0,
            player_forward: Vec3::X,
            player_speed: 0.0,
            drag: false,
            traffic_cars: TRAFFIC_CONTEXTS,
            traffic_levels: Vec::new(),
            horns: 0,
            knocks: 0,
            stereos: 0,
            racer_stereos: 0,
        }
    }

    /// The traffic contexts' cars (the mix map's section 3 copies, in
    /// order).
    pub fn traffic_contexts(&self) -> Vec<Option<usize>> {
        self.contexts.iter().map(|c| c.car).collect()
    }

    /// The traffic objects' fields for the mix map: (woosh +0xC, engine
    /// +0x10) of traffic context `k`.
    pub fn traffic_fields(&self, k: usize) -> (i32, i32) {
        (self.contexts.get(k).is_some_and(|c| c.t.whoosh_started) as i32, 0x7FFF)
    }

    pub fn silence(&mut self, sink: &mut dyn Sink) {
        for c in &mut self.contexts {
            if let Some(mut w) = c.t.whoosh.take() {
                w.stop(sink);
            }
            c.t.bottom.stop(sink);
        }
        for b in self.banks.values_mut() {
            close_bank(b, &mut self.sounds, &mut self.gains, sink);
        }
        self.banks.clear();
        close_bank(&mut self.traffic_bank, &mut self.sounds, &mut self.gains, sink);
        close_bank(&mut self.env_bank, &mut self.sounds, &mut self.gains, sink);
        self.ai.clear();
        self.contexts.clear();
    }

    /// One frame: the AI racers, the traffic cars (in the pool's order),
    /// the player's car position and velocity (the listener's car and the
    /// view car).
    /// The AI racers' engine objects alive (one per racer fed to `update`).
    pub fn ai_voices(&self) -> usize {
        self.ai.len()
    }

    pub fn update(
        &mut self,
        sink: &mut dyn Sink,
        ai: &[AiInput],
        traffic: &[TrafficInput],
        player: (Vec3, Vec3),
        dt: f32,
    ) {
        self.clock += dt;
        self.whooshes = 0;
        self.horns = 0;
        self.knocks = 0;
        self.stereos = 0;
        self.ai_update(sink, ai, player.0, dt);
        self.traffic_update(sink, traffic, player, dt);
    }

    /// The context's release (0x469B90: +0x3C = 0, each object's slot 12,
    /// +0x30 = 0).
    pub(super) fn release(&mut self, c: usize, sink: &mut dyn Sink) {
        let now = self.clock;
        let ctx = &mut self.contexts[c];
        ctx.car = None;
        let t = &mut ctx.t;
        // TrafficEngine 0x475790: the FX_TRAFFIC instance stops.
        if let (Some(id), Some(b)) = (t.event.take(), self.traffic_bank.as_mut()) {
            b.events.release(id);
        }
        // TrafficWoosh 0x462650: the whoosh goes.
        if let Some(mut w) = t.whoosh.take() {
            w.stop(sink);
        }
        // TrafficHorn 0x4624E0: the horn stops, +0xC4 = +0xC5 = 0, +0xC0 =
        // the context's clock.
        if let (Some(id), Some(b)) = (t.horn.event.take(), self.env_bank.as_mut()) {
            b.events.release(id);
        }
        t.horn = Horn { stopped: now, refused: t.horn.refused, ..Horn::default() };
        // TrafficStereo 0x463540, TrafficBottomOut 0x462700.
        t.stereo.release(self.env_bank.as_mut().map(|b| &mut b.events));
        t.bottom.stop(sink);
    }

    /// The context's attach (0x469B20: +0x30 = the car, +0x3C = 1, each
    /// object's slot 11).
    pub(super) fn attach(&mut self, c: usize, key: usize, taxi: bool) {
        let ctx = &mut self.contexts[c];
        ctx.car = Some(key);
        let t = &mut ctx.t;
        // The position object follows the new car (**inferred**: its
        // doppler history starts again).
        t.doppler = Doppler::default();
        // TrafficEngine 0x4621C0: FX_TRAFFIC with the next variant
        // (0x82CB38 toggles 0 / 1).
        if let Some(b) = self.traffic_bank.as_mut() {
            let ev = &mut b.events;
            self.traffic_toggle = (self.traffic_toggle + 1) & 1;
            t.variant = self.traffic_toggle;
            t.event = ev.module("FX_TRAFFIC").and_then(|m| ev.play(m, &[t.variant.clamp(0, 15), 0, 0, 0, 0]));
        }
        // TrafficStereo 0x476580.
        t.stereo.attach(taxi);
    }

    pub(super) fn traffic_update(
        &mut self,
        sink: &mut dyn Sink,
        traffic: &[TrafficInput],
        player: (Vec3, Vec3),
        dt: f32,
    ) {
        open_bank(&mut self.traffic_bank, &mut self.sounds, &mut self.gains, sink, TRAFFIC);
        open_bank(&mut self.env_bank, &mut self.sounds, &mut self.gains, sink, ENV_COMMON);
        // The manager's contexts (0x476AF0): min(traffic cars, 6).
        let n = self.traffic_cars.min(TRAFFIC_CONTEXTS);
        if self.contexts.len() != n {
            for c in 0..self.contexts.len() {
                self.release(c, sink);
            }
            self.contexts.clear();
            self.contexts.resize_with(n, Context::default);
            // Each new horn object's ctor clears 0x82BC50.
            self.last_horn = 0.0;
        }
        // 0x463E30: attach / release by range.
        let held: Vec<Option<usize>> = self.contexts.iter().map(|c| c.car).collect();
        let cars: Vec<(usize, f32)> = traffic.iter().map(|i| (i.key, player.0.distance(i.position))).collect();
        let (attach, release) = assign(&held, &cars);
        for c in release {
            self.release(c, sink);
        }
        for (c, key) in attach {
            let taxi = traffic.iter().find(|i| i.key == key).is_some_and(|i| i.taxi);
            self.attach(c, key, taxi);
        }
        // 0x463BD0: each context's objects (they act only while attached).
        let rel = |inp: &TrafficInput| {
            let v = player.1 - inp.velocity;
            (v.x * v.x + v.y * v.y).sqrt()
        };
        for (k, ctx) in self.contexts.iter_mut().enumerate() {
            let t = &mut ctx.t;
            t.whoosh_started = false;
            let Some(inp) = ctx.car.and_then(|key| traffic.iter().find(|i| i.key == key)) else { continue };
            let mix = self.traffic_levels.get(k).copied().unwrap_or(inp.mix);
            let d = player.0.truncate().distance(inp.position.truncate()) as i32;
            t.doppler.update(d, dt);
            // The engine's frame (0x462240).
            if let (Some(b), Some(e)) = (self.traffic_bank.as_mut(), t.event) {
                let mph = inp.velocity.length() * 2.237;
                let v = mph / 140.0 * 1024.0;
                let p4 = if v > 0.0 { v.min(1024.0) as i32 } else { 0 };
                let p2 = (t.doppler.ratio(1) * 16383.0 - 16383.0) as i32;
                b.events.set(
                    e,
                    &[
                        t.variant,
                        mix.level(0, 0).clamp(0, 0x7FFF),
                        p2.clamp(-0x3FFF, 0x3FFF),
                        mix.az(0, 0).clamp(0, 0xFFFF),
                        p4.clamp(0, 0x3FF),
                    ],
                );
            }
            // CARSFX_TrafficBottomOut: BottomOut's update and per-frame
            // (0x47ED80 / 0x47F100) on the traffic car. Its mover is never
            // the physics one (car +0x28 is 2, TrafficMover, or 3, MeloMvr,
            // after a contact; 0x5F46B0), so each knock takes volume 127
            // (0x47EFB3); levels from the class's slots 0 and 2.
            let levels = [(mix.level(4, 0), mix.az(4, 0)), (mix.level(4, 2), mix.az(4, 2))];
            let b = crate::audio::roadwind::BottomIn {
                down: inp.down,
                up_z: inp.up_z,
                volume: 127.0,
                event_kind: inp.event_kind,
                levels,
            };
            self.knocks +=
                crate::audio::roadwind::bottom_out(&mut t.bottom, self.stitches.as_mut(), sink, b, self.clock, dt);
            // The whoosh (0x47E8D0 first: the running one, then 0x475E10).
            let r = rel(inp);
            if let Some(w) = t.whoosh.as_mut() {
                if r <= 15.0 || !w.update(sink, mix.level(1, 2), mix.az(1, 2), dt) {
                    w.stop(sink);
                    t.whoosh = None;
                }
            }
            let near = player.0.distance(inp.position) < 5.0;
            if t.whoosh.is_none() && near && r > 15.0 && t.last_whoosh.is_none_or(|l| l + 3.0 <= self.clock) {
                let n = self.whoosh_counter % 6;
                self.whoosh_counter = n + 1;
                let x = ((r - 15.0) / (60.0 - 15.0)).clamp(0.0, 1.0).clamp(0.0, 0.99);
                let k = ((x * 127.0).clamp(0.0, 127.0) * -0.023_437_5) as i32;
                let index = n as i32 + 9 - 6 * k;
                if let Some(s) = self.stitches.as_mut() {
                    t.whoosh = StitchVoice::start(s, 1, index as usize);
                    self.whooshes += t.whoosh.is_some() as usize;
                }
                t.last_whoosh = Some(self.clock);
                t.whoosh_started = true;
            }
            // The horn (0x47E6E0, then its per-frame 0x462550).
            let now = self.clock;
            let h = &mut t.horn;
            h.step_fade(dt);
            let ev = self.env_bank.as_mut().map(|b| &mut b.events);
            match (h.event, ev) {
                (None, Some(ev)) => {
                    if now > h.stopped + 3.0
                        && now > h.refused + 3.0
                        && self.player_speed > 15.0
                        && horn_aims(inp.position, inp.velocity, player.0, self.player_forward, self.drag)
                    {
                        if crate::audio::collision::rand_float(1.0) < 0.3 || self.last_horn + 5.0 < now {
                            let variant = 5 + crate::audio::collision::rand(3) as i32;
                            h.timed = true;
                            h.length = crate::audio::collision::rand_float(4.0 - 1.2) + 1.2;
                            h.variant = variant.clamp(0, 50);
                            h.event =
                                ev.module("ENV_STATIC").and_then(|m| ev.play(m, &[variant.clamp(0, 50), 0, 0, 0, 0]));
                            self.last_horn = now;
                            h.start = now;
                            h.set_fade(0.001, 1.0, 1.0);
                            h.fading = false;
                            self.horns += h.event.is_some() as usize;
                        } else {
                            h.refused = now;
                        }
                    }
                }
                (Some(e), Some(ev)) => {
                    if (now > h.start + h.length || !h.timed) && !h.fading {
                        h.set_fade(0.15, 1.0, 0.0);
                        h.fading = true;
                    }
                    if h.fading && h.fade.5 {
                        ev.release(e);
                        *h = Horn { stopped: now, refused: h.refused, ..Horn::default() };
                    } else {
                        let p2 = ((t.doppler.ratio(0) * 4096.0) as i32).clamp(0, 0x1FA0);
                        let p1 = ((mix.level(2, 1) as f32 * h.fade.4) as i32).clamp(0, 0x7FFF);
                        ev.set(e, &[h.variant, p1, p2, mix.az(2, 1).clamp(0, 0xFFFF)]);
                    }
                }
                _ => {}
            }
            // The stereo's slots 9 / 10 (0x4635B0, 0x4636E0): the mix map's
            // traffic class 3 slot 0.
            if let Some(ev) = self.env_bank.as_mut().map(|b| &mut b.events) {
                self.stereos += t.stereo.frame(ev, mix.level(3, 0), mix.az(3, 0)) as usize;
            }
        }
        run_bank(&mut self.traffic_bank, &mut self.sounds, &mut self.gains, sink, dt);
        run_bank(&mut self.env_bank, &mut self.sounds, &mut self.gains, sink, dt);
    }
}
