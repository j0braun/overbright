use crate::*;

/// `NFSU2_QR_MODE`: a Quick Race mode for the `qrstart` shot and the
/// `race` bench (default Circuit).
pub(crate) fn qr_mode_env() -> screens::QrMode {
    match std::env::var("NFSU2_QR_MODE").as_deref() {
        Ok("drift") => screens::QrMode::Drift,
        Ok("sprint") => screens::QrMode::Sprint,
        Ok("drag") => screens::QrMode::Drag,
        Ok("streetx") => screens::QrMode::StreetX,
        Ok("url") => screens::QrMode::Url,
        _ => screens::QrMode::Circuit,
    }
}

/// A line of `NFSU2_BENCH_CAMTRACK`: eye (3), forward (3), screen up (3),
/// the field of view as the game's binary angle (camera +0xC4), the car's
/// position (3) and its forward and left axes (3 + 3), whitespace apart.
pub(crate) struct TrackPose {
    eye: Vec3,
    forward: Vec3,
    up: Vec3,
    fov: f32,
    car: Vec3,
    car_axes: [Vec3; 2],
}

impl TrackPose {
    pub(crate) fn parse(line: &str) -> Option<TrackPose> {
        let v: Vec<f32> = line.split_whitespace().filter_map(|f| f.parse().ok()).collect();
        if v.len() < 19 {
            return None;
        }
        let at = |k: usize| Vec3::new(v[k], v[k + 1], v[k + 2]);
        Some(TrackPose {
            eye: at(0),
            forward: at(3).normalize_or(Vec3::Y),
            up: at(6).normalize_or(Vec3::Z),
            fov: v[9] / 65536.0 * std::f32::consts::TAU,
            car: at(10),
            car_axes: [at(13), at(16)],
        })
    }

    pub(crate) fn apply(&self, d: &mut Drive) {
        let [f, l] = self.car_axes;
        let rot =
            glam::Mat3::from_cols(f.normalize_or(Vec3::X), l.normalize_or(Vec3::Y), f.cross(l).normalize_or(Vec3::Z));
        d.state.position = self.car;
        d.state.orientation = Quat::from_mat3(&rot).normalize();
        d.state.velocity = Vec3::ZERO;
        // Explicit synthetic/replay motion initialization; no native placement offset.
        d.state.publish_forward_speed();
        d.camera.position = self.eye;
        d.camera.yaw = self.forward.x.atan2(self.forward.y);
        d.camera.pitch = self.forward.z.clamp(-1.0, 1.0).asin();
        d.camera.up = self.up;
        d.camera.game_fov = Some(self.fov);
    }
}

/// The bench's knobs (`NFSU2_BENCH_*` and the measuring aids it shares
/// with the shots), read once at the start. Each keeps its old
/// parse; the defaults stay at the use (`unwrap_or`) where they were.
pub(crate) struct BenchEnv {
    /// NFSU2_BENCH_SIZE=WxH.
    pub(crate) size: Option<(u32, u32)>,
    /// NFSU2_BENCH_GPU (not "0"): the GPU timestamps.
    pub(crate) gpu_timing: bool,
    /// NFSU2_FLOW_LOAD=name: that saved profile.
    pub(crate) flow_load: Option<String>,
    /// NFSU2_SHOT_SAVE=file: a native save's profile.
    pub(crate) shot_save: Option<std::ffi::OsString>,
    /// NFSU2_BENCH_CAR: the starting car.
    pub(crate) car: Option<String>,
    /// NFSU2_NEON: an underglow fitted.
    pub(crate) neon: bool,
    /// NFSU2_BENCH_DETAIL=1..5.
    pub(crate) detail: Option<u8>,
    /// NFSU2_BENCH_ENH: options.txt lines for the run.
    pub(crate) enh: Option<String>,
    /// NFSU2_BENCH_AUDIO: the sounds on an offline mixer.
    pub(crate) audio: bool,
    /// NFSU2_BENCH_EVENT=n.
    pub(crate) event: Option<usize>,
    /// NFSU2_BENCH_MENU_FIRST and NFSU2_BENCH_MENU_FRAMES.
    pub(crate) menu_first: bool,
    pub(crate) menu_frames: Option<usize>,
    /// NFSU2_VIEW=k: the camera setting.
    pub(crate) view: Option<u8>,
    /// NFSU2_BENCH_SEED=n.
    pub(crate) seed: Option<u32>,
    /// NFSU2_BENCH_KIND=drift|drag.
    pub(crate) kind: Option<String>,
    /// NFSU2_BENCH_ENGAGE=n.
    pub(crate) engage: Option<usize>,
    /// NFSU2_BENCH_INFLIGHT=n.
    pub(crate) inflight: Option<u64>,
    /// NFSU2_BENCH_AT=x,y,z,deg.
    pub(crate) at: Option<[f32; 4]>,
    /// NFSU2_FX_SPEED's first value (km/h) and NFSU2_FX_GEAR.
    pub(crate) fx_speed: Option<f32>,
    pub(crate) fx_gear: Option<usize>,
    /// NFSU2_BENCH_AT_DRIVE=n.
    pub(crate) at_drive: Option<usize>,
    /// NFSU2_REPLAY_AFTER, NFSU2_REPLAY_FROM=post, NFSU2_REPLAY_CAMERA,
    /// NFSU2_REPLAY_SLOW, NFSU2_REPLAY_SMOOTH_PROBE.
    pub(crate) replay_after: Option<usize>,
    pub(crate) replay_from_post: bool,
    pub(crate) replay_camera: Option<usize>,
    pub(crate) replay_slow: Option<f32>,
    pub(crate) replay_smooth_probe: Option<f32>,
    /// NFSU2_BENCH_CAMTRACK=file.
    pub(crate) camtrack: Option<String>,
    /// NFSU2_BENCH_FREEZE=n and NFSU2_BENCH_FREEZE_MOVE=dx,dy,dz.
    pub(crate) freeze: Option<usize>,
    pub(crate) freeze_move: Option<[f32; 3]>,
    /// NFSU2_BENCH_HZ (1 or more).
    pub(crate) hz: Option<f32>,
    /// NFSU2_BENCH_DUMP=dir, NFSU2_BENCH_DUMP_EVERY, NFSU2_BENCH_DUMP_FRAMES.
    pub(crate) dump: Option<std::path::PathBuf>,
    pub(crate) dump_every: Option<usize>,
    pub(crate) dump_frames: Option<String>,
    /// NFSU2_BENCH_NOGAS.
    pub(crate) no_gas: bool,
    /// NFSU2_BENCH_JSON=file.
    pub(crate) json: Option<std::path::PathBuf>,
    /// NFSU2_BENCH_FINISH=s, NFSU2_BENCH_LAPS=n@s, NFSU2_BENCH_THROUGH.
    pub(crate) finish: Option<f32>,
    pub(crate) laps: Option<String>,
    pub(crate) through: bool,
    /// NFSU2_BENCH_MATRIX and NFSU2_BENCH_MATRIX_FRAMES.
    pub(crate) matrix: Option<String>,
    pub(crate) matrix_frames: Option<usize>,
    /// NFSU2_VRAM_REPORT, NFSU2_POP_METER_LOG, NFSU2_POP_LOG: report files.
    pub(crate) vram_report: Option<String>,
    pub(crate) pop_meter_log: Option<String>,
    pub(crate) pop_log: Option<String>,
}

fn var<T: std::str::FromStr>(name: &str) -> Option<T> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}

fn floats(name: &str) -> Option<Vec<f32>> {
    std::env::var(name).ok().map(|s| s.split(',').filter_map(|f| f.trim().parse::<f32>().ok()).collect::<Vec<_>>())
}

impl BenchEnv {
    pub(crate) fn from_env() -> Self {
        Self {
            size: env_size("NFSU2_BENCH_SIZE"),
            gpu_timing: std::env::var("NFSU2_BENCH_GPU").map_or(true, |v| v != "0"),
            flow_load: std::env::var("NFSU2_FLOW_LOAD").ok(),
            shot_save: std::env::var_os("NFSU2_SHOT_SAVE"),
            car: std::env::var("NFSU2_BENCH_CAR").ok(),
            neon: std::env::var_os("NFSU2_NEON").is_some(),
            detail: var::<u8>("NFSU2_BENCH_DETAIL").filter(|d| (1..=5).contains(d)),
            enh: std::env::var("NFSU2_BENCH_ENH").ok(),
            audio: std::env::var_os("NFSU2_BENCH_AUDIO").is_some(),
            event: var("NFSU2_BENCH_EVENT"),
            menu_first: std::env::var_os("NFSU2_BENCH_MENU_FIRST").is_some(),
            menu_frames: var("NFSU2_BENCH_MENU_FRAMES"),
            view: var("NFSU2_VIEW"),
            seed: var("NFSU2_BENCH_SEED"),
            kind: std::env::var("NFSU2_BENCH_KIND").ok(),
            engage: var("NFSU2_BENCH_ENGAGE"),
            inflight: var("NFSU2_BENCH_INFLIGHT"),
            at: floats("NFSU2_BENCH_AT").filter(|v| v.len() == 4).map(|v| [v[0], v[1], v[2], v[3]]),
            fx_speed: std::env::var("NFSU2_FX_SPEED")
                .ok()
                .and_then(|s| s.split(',').next().and_then(|f| f.trim().parse().ok())),
            fx_gear: var("NFSU2_FX_GEAR"),
            at_drive: var("NFSU2_BENCH_AT_DRIVE"),
            replay_after: var("NFSU2_REPLAY_AFTER"),
            replay_from_post: std::env::var("NFSU2_REPLAY_FROM").is_ok_and(|v| v == "post"),
            replay_camera: var("NFSU2_REPLAY_CAMERA"),
            replay_slow: var("NFSU2_REPLAY_SLOW"),
            replay_smooth_probe: var("NFSU2_REPLAY_SMOOTH_PROBE"),
            camtrack: std::env::var("NFSU2_BENCH_CAMTRACK").ok(),
            freeze: var("NFSU2_BENCH_FREEZE"),
            freeze_move: floats("NFSU2_BENCH_FREEZE_MOVE").filter(|v| v.len() == 3).map(|v| [v[0], v[1], v[2]]),
            hz: var("NFSU2_BENCH_HZ").filter(|&hz: &f32| hz >= 1.0),
            dump: std::env::var("NFSU2_BENCH_DUMP").ok().map(std::path::PathBuf::from),
            dump_every: var("NFSU2_BENCH_DUMP_EVERY"),
            dump_frames: std::env::var("NFSU2_BENCH_DUMP_FRAMES").ok(),
            no_gas: std::env::var_os("NFSU2_BENCH_NOGAS").is_some(),
            json: std::env::var_os("NFSU2_BENCH_JSON").map(std::path::PathBuf::from),
            finish: var("NFSU2_BENCH_FINISH"),
            laps: std::env::var("NFSU2_BENCH_LAPS").ok(),
            through: std::env::var_os("NFSU2_BENCH_THROUGH").is_some(),
            matrix: std::env::var("NFSU2_BENCH_MATRIX").ok(),
            matrix_frames: var("NFSU2_BENCH_MATRIX_FRAMES"),
            vram_report: std::env::var("NFSU2_VRAM_REPORT").ok(),
            pop_meter_log: std::env::var("NFSU2_POP_METER_LOG").ok(),
            pop_log: std::env::var("NFSU2_POP_LOG").ok(),
        }
    }
}
