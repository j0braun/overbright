use crate::*;

/// `NFSU2_RAIN_CLOCK=<s>`: the world clock (0x7FB718) a track load starts
/// at (1.0 s in the game), to see the rain at a given time; a `--shot`
/// also steps the weather up to it. The sky cycle (0x60FAB0) starts as
/// it would be that long after a load.
pub(crate) fn rain_clock() -> Option<f32> {
    debug_env().rain_clock.clone().and_then(|v| v.parse::<f32>().ok()).or_else(crate::dev_hooks::rain_clock_override)
}

impl Drive {
    /// A world load's clock: 0x576D00 (from the world object's ctor
    /// 0x5FF4A0 and dtor 0x5FF700) sets the tick 0x865188 to 4000, the
    /// clock 0x7FB718 to 1.0 s, and the unload 0x5815D0 resets the sky
    /// cycle (0x60F750). A drive-in that keeps this drive (same region and
    /// car) is a load in the game all the same (the race's track switch
    /// 0x581830 → 0x5815D0 → 0x60A9B0 → 0x5FF700, then World_Start), so it
    /// starts them as a new drive does. Observed under Wine: resuming a
    /// career read 1.93 s on the first in-world frame.
    pub(crate) fn restart_clock(&mut self) {
        self.clock = rain_clock().unwrap_or(1.0);
        self.world_clock = Default::default();
        // The human controller is rebuilt with the world (node +0x0C 0,
        // way flag 1: 0x40F630).
        self.human_road = Default::default();
        self.time_of_day = rain_clock().map_or_else(TimeOfDay::default, |c| TimeOfDay::default().after(c - 1.0));
        nfsu2::weather::set_clock(&self.world, self.clock);
    }

    /// `rng`: the shared generator (0x7EA624) the rain's gusts and lens
    /// drops draw from (0x6136C0, 0x614100), stepped here on the
    /// fixed update.
    pub(crate) fn tick_clock(&mut self, dt: f32, rng: &mut nfsu2::opponents::GameRng) {
        self.world_clock.finish_update(dt);
        // 0x5728D0 (0x5814C2): the shake steps once a world frame, after
        // the cameras.
        self.impact.shake.step(dt);
        self.clock += dt;
        self.time_of_day.update(self.clock, dt);
        nfsu2::weather::set_clock(&self.world, self.clock);
        let frame = self.weather_frame();
        self.weather.update(&self.world, &frame, dt, rng);
    }

    /// The camera and car as the weather object reads them (the camera's
    /// velocity, camera +0x1E0, taken as the car's: **inferred**).
    pub(crate) fn weather_frame(&self) -> nfsu2::weather::Frame {
        nfsu2::weather::Frame {
            camera: self.camera.position,
            camera_forward: self.camera.forward(),
            camera_velocity: self.state.velocity,
            car: self.state.position,
            forward: self.state.orientation * Vec3::X,
            clock: self.clock,
        }
    }

    /// The falling rain's quads for the particle pass (0x613A80; the batch
    /// 0x5BB440 takes the colour as it is, like the particles'), seen from
    /// the frame's camera with the car as drawn (the frame's own
    /// values; the drive's are not written for the draw). `rain`,
    /// the drops as the frame draws them between the ticks (None: the
    /// tick's).
    pub(crate) fn rain_quads_at(
        &self,
        camera: &Camera,
        car: &CarState,
        rain: Option<&nfsu2::weather::RainSnapshot>,
    ) -> Vec<nfsu2::particles::Quad> {
        let texture = nfsu2::formats::hash::bin_hash("RAINDROP");
        let frame = nfsu2::weather::Frame {
            camera: camera.position,
            camera_forward: camera.forward(),
            camera_velocity: car.velocity,
            car: car.position,
            forward: car.orientation * Vec3::X,
            clock: self.clock,
        };
        rain.map_or_else(|| self.weather.streaks(&frame), |r| r.streaks(&frame))
            .into_iter()
            .map(|q| nfsu2::particles::Quad {
                texture,
                corners: q.corners,
                uv: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                rgba: q.rgba,
            })
            .collect()
    }

    /// The rain's screen passes for this frame. `rain`, the lens
    /// drops as the frame draws them between the ticks (None: the tick's).
    /// `rain_color`, the fog's rain colour as the frame draws it.
    /// `lens_drops`: the Lens Drops row the drops are drawn with.
    pub(crate) fn rain_frame(
        &self,
        rain_color: [f32; 3],
        rain: Option<&nfsu2::weather::RainSnapshot>,
        lens_drops: bool,
    ) -> nfsu2::render::postfx::RainFrame {
        if debug_env().fog_debug {
            eprintln!(
                "fog debug: clock {:.1} fog weight {:.3} rain {:.3} rain colour {:?}",
                self.clock,
                self.time_of_day.fog_weight(),
                self.weather.amount,
                rain_color
            );
        }
        // 0x5C7870 takes slot 0 (the NIS SCREEN_FLASH) over the rain's slot 2.
        let tint = self
            .screen_flash
            .multiplier()
            .or_else(|| nfsu2::weather::tint_multiplier(rain_color, self.weather.amount));
        nfsu2::render::postfx::RainFrame {
            tint,
            drops: rain
                .map_or_else(|| self.weather.lens_drops().collect::<Vec<_>>(), |r| r.lens_drops().collect())
                .into_iter()
                // Lens Drops' extra drops as the last tick left them
                // (stepped on the tick, `WorldRenderer::enh_tick`).
                .chain(nfsu2::render::weather_fx::lens_frame(lens_drops))
                .collect(),
        }
    }
}
