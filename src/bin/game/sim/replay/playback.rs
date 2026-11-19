use crate::*;

/// Where a skip goes: a marker (index into the recording's LAP_SNAPSHOTs)
/// or THE_PRESENT, the state the replay was entered from (0x4C10B0 takes
/// it on entry; Back skips to it, 0x4C1220, then leaves, 0x4A50E0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum SkipTo {
    Lap(usize),
    Present,
}

/// A replay playing (UI_ReplayControl.fng over RCSTATE_INSTANT_REPLAY):
/// the next recorded frame, the real time not yet played, paused (manager
/// +0x14), slow motion (+0x18), the bar hidden (control +0x74), when the
/// control bar opened (ms), the real time since (s, the tick 0x8651A8),
/// the render frames counted (0x865148), the markers, a skip waiting
/// (control +0xCC, from +0xD4) and the replay camera.
#[derive(Clone, Default)]
pub(crate) struct Playback {
    pub(crate) at: usize,
    pub(crate) carry: f32,
    pub(crate) paused: bool,
    pub(crate) slow: bool,
    pub(crate) hidden: bool,
    pub(crate) opened: f32,
    pub(crate) clock: f32,
    pub(crate) frames: u32,
    pub(crate) markers: Vec<usize>,
    pub(crate) skip: Option<(SkipTo, f32)>,
    pub(crate) present: Option<Box<Snapshot>>,
    /// Entered from the race's pause (UI_pause's Replay, the flow object's
    /// +0x40 clear): THE_PRESENT taken, the last marker, and on leaving
    /// back to the pause (0x4A50E0 → 0x5F43E0(6)). Else from the post-race
    /// menu (+0x40 set by the race's end, 0x60AE55): the first marker, no
    /// THE_PRESENT, and Back leaves at once (0x4C1220).
    pub(crate) pause: bool,
    /// Back was taken (control +0xD2, 0x4C1220): a skip that completes
    /// leaves instead of playing (0x4C10B0).
    pub(crate) leaving: bool,
    pub(crate) camera: ReplayCamera,
    /// The track's route cameras (TRACKS/<region>.BUN, 0x8003B200) and the
    /// director's held shot (0x44F9A0).
    pub(crate) tracks: Vec<CameraGroup>,
    pub(crate) track: TrackDirector,
    /// The view's eye last frame (its velocity, camera +0x1E0).
    pub(crate) last_eye: Option<Vec3>,
    /// The typed track cameras (GLOBAL/InGameCommon.bun, 0x8003B202; the
    /// camera manager +0x14), the held typed shot, the last three tracks
    /// drawn (the ring 0x828774: route or typed) and the car's velocity at
    /// the last played frame (the Corner weight's curvature, **inferred**).
    pub(crate) typed: std::sync::Arc<Vec<CameraGroup>>,
    pub(crate) typed_shot: Option<nfsu2::track_camera::TypedShot>,
    pub(crate) recent: Vec<(usize, usize)>,
    pub(crate) last_velocity: Option<Vec3>,
    /// [car +0x10] +0x20 for the Speed weight (0x441C22): the racer info's
    /// +0x30, the gearing top speed (`gearing_top_speed`).
    pub(crate) typed_top: f32,
    /// Replay Cameras "Extended" (docs/playing/modern-extras.md §3; clone
    /// deviation): smooth slow motion's speed (0: none; ½ or ¼), the
    /// extra cameras' state, and the drawn poses and camera before the last
    /// recorded frame played (the slow frames are drawn between them).
    pub(crate) smooth: f32,
    pub(crate) extra: nfsu2::replay_cams::State,
    pub(crate) slow_prev: Option<(Vec<nfsu2::interpolate::CarPose>, Option<nfsu2::render::camera::Camera>)>,
}

/// A skip waits this long (s, 0x78E000 at 0x4C112A) and for the track
/// streamer before the snapshot is restored.
pub(crate) const SKIP_WAIT: f32 = 0.5;
