use crate::*;

mod camera;
mod clock;
mod step;
mod style;
pub(crate) use camera::{Impact, lift_car, traffic_lift_cars};
pub(crate) use clock::*;
pub(crate) use step::*;
pub(crate) use style::*;

/// A loaded region with the player's car in it.
pub(crate) struct Drive {
    pub(crate) region: String,
    pub(crate) world: World,
    pub(crate) car: Car,
    /// The car's id (its renderer: `RenderState.cars[car_id]`; the
    /// world's renderer is `RenderState.world`).
    pub(crate) car_id: CarId,
    pub(crate) state: CarState,
    pub(crate) chase: ChaseCamera,
    /// The Field Of View offset the chase camera is stepped with (radians):
    /// the tick's (`sim::TickSettings`, set as each tick starts), the
    /// settings' at the drive's load.
    pub(crate) fov_offset: f32,
    pub(crate) camera: Camera,
    pub(crate) fog: FogState,
    /// The screen tint maps, smoothed at the camera (0x5D7800).
    pub(crate) tint: ScreenTintState,
    /// Effect controller slot 0 as the NIS SCREEN_FLASH drives it
    /// (0x5DBD80(1), drawn by the tint pass 0x5C7870).
    pub(crate) screen_flash: nfsu2::screen_flash::ScreenFlash,
    /// The horizon fog's visibility, smoothed at the camera (0x5DB910).
    pub(crate) horizon_fog: nfsu2::horizon_fog::HorizonFog,
    /// Motion blur's level (0x870900) and the camera's velocity (+0x1E0).
    pub(crate) motion: nfsu2::motion_blur::MotionBlur,
    /// The sky cycle (0x60FAB0) and the world clock it reads (0x7FB718:
    /// 1.0 s at track load).
    pub(crate) time_of_day: TimeOfDay,
    pub(crate) clock: f32,
    pub(crate) world_clock: nfsu2::ai::encounter_controller::WorldClock,
    /// The rain (the view's weather object, `nfsu2::weather`).
    pub(crate) weather: nfsu2::weather::Weather,
    /// The human controller's road node and walk (0x42CAA0 → 0x42A190,
    /// traced) and its entry counts (0x411490, 0x40F180) since the drive began.
    pub(crate) human_road: nfsu2::ai::route::HumanRoad,
    /// Free roam's crash / jump camera mover type when its shot is drawn
    /// (Camera_Factory 0x0F / 0x10; 0 none), set by `event_shot`.
    pub(crate) event_mover: u32,
    pub(crate) node_counts: (u32, u32),
    /// The street lamps' probed mirror heights for the road reflection's
    /// flares (each flare record's +0x1C).
    pub(crate) flare_mirrors: nfsu2::glow::MirrorHeights,
    /// The flare pools (0x82CC30) as view 1's scenery cull filled them
    /// this frame, and the world clock they were last emptied at.
    pub(crate) flare_pools: nfsu2::glow::Pools,
    pub(crate) flare_pools_clock: f32,
    /// The native visuals' tick output (S_n) and the tint's and
    /// the flare blink's tick state (`render::visuals`).
    pub(crate) vis: crate::render::visuals::DriveVisuals,
    /// `fill_flare_pools`' list of the frame's light placements (kept for
    /// its allocation).
    pub(crate) flare_lights: Vec<usize>,
    /// Knock-over props, and every knocked one's pose for the renderer
    /// (kept, so a new renderer gets them too).
    pub(crate) props: Props,
    pub(crate) posed: std::collections::HashMap<usize, glam::Mat4>,
    /// The particle effects.
    pub(crate) effects: Effects,
    /// What the track's barriers are set up for (0x578070).
    pub(crate) barriers: BarrierSetup,
    /// The human input's hydraulics (0x407400, 0x4078A0).
    pub(crate) hydro: nfsu2::hydraulics::Controller,
    /// The world tick of the car's last stamped placement (car +0x974;
    /// the hold +0x978 is `racer_reset::PLACE_HOLD`), None when the last
    /// placement was not stamped (0x5FBD70 / 0x5FBEE0 with the flag clear
    /// write +0x974 = 0).
    pub(crate) placed_stamp: Option<u32>,
    /// The free-roam racer's saved places (racer +0xBC / +0xC0, saved each
    /// update by 0x602AEE as in a race; `Progress::save_place`); a new
    /// drive-in starts them empty.
    pub(crate) roam: Progress,
    /// The player's impact shake (0x865360 slot 0) and what the cameras
    /// read of the world with it (`camera::Impact`).
    pub(crate) impact: Impact,
}

impl Drive {
    pub(crate) fn place(&mut self, at: Vec3, heading: f32) {
        let road = self.world.road_height_below(at + Vec3::Z * 3.0).unwrap_or(at.z);
        // The car keeps its own barrier groups (its collision context's
        // list, set by the car set-up 0x604270).
        let barriers = std::mem::take(&mut self.state.ground.barriers);
        // The racer's settings stay: its transmission (racer +0x20, the
        // input's mode +0x2C0) and the race start's hold (+0x2B4) belong to
        // the driver, not the car's placement.
        let (manual, held, stability_off) = (self.state.manual, self.state.held, self.state.stability_off);
        // Set on its four wheels' ground as the racer set-up does
        // (0x604270 → 0x5F5020 → 0x5F4A00).
        self.state = CarState::placed(&self.car.car_type, &self.world, Vec3::new(at.x, at.y, road), heading);
        self.state.ground.barriers = barriers;
        self.state.manual = manual;
        self.state.held = held;
        self.state.stability_off = stability_off;
        // A placement without the stamp flag clears +0x974 (0x5FBDF2).
        self.placed_stamp = None;
        // A new chase mover (0x5FA830 with its reset flag) clears the
        // shake slots (0x5729F0).
        self.impact.shake = Default::default();
        self.chase = ChaseCamera::new(&self.car, self.chase.kind());
        self.chase_view(0.0);
    }

    /// A placement with the stamp flag (0x5FBD70 at 0x5FBDD9 / 0x5FBEE0
    /// → 0x5EBF80): car +0x974 = the sim clock, the reset key's 2 s hold.
    pub(crate) fn stamp_placement(&mut self) {
        self.placed_stamp = Some(self.world_clock.tick);
    }

    pub(crate) fn step(
        &mut self,
        controls: Controls,
        dt: f32,
        rng: &mut nfsu2::opponents::GameRng,
        sounds: &mut nfsu2::drive::ContactSounds,
    ) {
        let world_sample = self.state.begin_world_update(dt);
        let steps = world_steps(dt);
        for _ in 0..steps {
            self.posed.extend(self.state.step_with_props(
                &self.car,
                &self.world,
                &mut self.props,
                controls,
                dt / steps as f32,
                sounds,
            ));
        }
        self.state.finish_world_update(world_sample);
        self.impact_trigger();
        self.chase_view(dt);
        self.tick_clock(dt, rng);
    }

    /// The car's footprint for a free view of the car (held still).
    pub(crate) fn hold(&mut self, dt: f32, rng: &mut nfsu2::opponents::GameRng) {
        self.chase_view(dt);
        self.tick_clock(dt, rng);
    }
}
