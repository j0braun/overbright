//! Enhancement effects on ticks (docs/uncapped-fps.md, "Enhancement effects on ticks"): the
//! world ticks the tick-stepped enhancement effects step on
//! (`nfsu2::render::enh::tick_views`): recorded at the end of each tick
//! (`Game::capture_tick`), published once per drawn frame before the world
//! is planned (`Game::draw_frame`). A pure read of the game.
use crate::*;
use nfsu2::render::enh::tick_views::{self, MAX_VIEWS, TickFrame, TickRecord, TickView};

impl Game {
    /// The end of a tick: its index, the game clock's seconds, the world
    /// clock and the drive's camera kept (the last [`MAX_VIEWS`], the ring
    /// reused).
    pub(crate) fn record_tick_view(&mut self) {
        let r = TickRecord {
            tick: self.pacing.clocks.tick,
            time: self.seconds(),
            clock: self.sim.drive.as_ref().map_or(0.0, |d| d.clock),
            camera: self.sim.drive.as_ref().map(|d| d.camera),
            opts: self.pacing.tick_settings.tick_options(),
        };
        let recent = &mut self.interp.recent;
        if recent.len() >= MAX_VIEWS {
            recent.pop_front();
        }
        recent.push_back(r);
    }

    /// The frame's ticks published (`alpha`: the frame's place between
    /// the last two): the ticks since the last published frame, each with
    /// its camera and the sections drawn from its eye; the newest takes
    /// the drive's camera as the frame finds it (a camera written after
    /// the tick, the harness's frozen one, is the newest tick's), and a
    /// tick taken without a record (the harness's plain steps) the live
    /// values. No drive: nothing published (the effects step per draw).
    pub(crate) fn publish_tick_views(&self, alpha: f32, lag: f32) {
        let Some(d) = self.sim.drive.as_ref() else {
            tick_views::publish(None);
            return;
        };
        let newest = self.pacing.clocks.tick;
        if newest < tick_views::published_through() {
            tick_views::rewind();
        }
        let through = tick_views::published_through();
        let mut views: Vec<TickView> = Vec::new();
        let view = |r: &TickRecord, camera: Camera| {
            let sections: Option<std::sync::Arc<[u32]>> =
                d.world.drawn_sections_at(camera.position, r.opts.keep_loaded).map(|(_, l)| l.as_ref().into());
            let far =
                sections.as_ref().and_then(|l| d.world.far_sections_at(camera.position, l, r.opts.far_draw)).map(Into::into);
            TickView { tick: r.tick, time: r.time, clock: r.clock, camera, sections, far, opts: r.opts }
        };
        if newest > through {
            for r in self.interp.recent.iter().filter(|r| r.tick > through && r.tick < newest) {
                if let Some(c) = r.camera {
                    views.push(view(r, c));
                }
            }
            let last = self.interp.recent.back().filter(|r| r.tick == newest).copied().unwrap_or(TickRecord {
                tick: newest,
                time: self.seconds(),
                clock: d.clock,
                camera: None,
                opts: self.pacing.tick_settings.tick_options(),
            });
            views.push(view(&last, d.camera));
            if views.len() > MAX_VIEWS {
                views.drain(..views.len() - MAX_VIEWS);
            }
        }
        let at = newest as f64 - f64::from(1.0 - alpha.clamp(0.0, 1.0));
        tick_views::publish(Some(TickFrame { views, at, lag }));
    }
}
