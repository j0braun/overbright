//! `game` — the career: the game's menus, explore mode in Bayview, and
//! races run on the career's events.
//!
//! Menus: arrow keys move, Enter chooses, Esc goes back. Driving: W/S or
//! the arrows, A/D steer, Space handbrake, C camera. In explore mode Esc
//! opens the crib menu and Enter starts the race at an event marker you
//! are parked at; Tab reads the SMS messages. The career is saved to
//! ~/.local/share/overbright.
//!
//! `game --shot SCREEN out.png` renders a screen headless (main, career,
//! crib, carlot, worldmap, sms, sms_rest, sms_tip, smsbox, smsread, finish …) for checking; `NFSU2_SHOT_MS` sets the front
//! end's time (default 5000 ms) and `NFSU2_SHOT_SIZE=WxH` the size.
// The shipped Windows game opens no console window (nfsu2::console).
#![cfg_attr(all(windows, not(feature = "dev-harness")), windows_subsystem = "windows")]

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
#[cfg_attr(not(feature = "dev-harness"), allow(unused_imports))] // used by the dev harness only
use glam::{Quat, Vec2, Vec3};
use nfsu2::formats::bundle::chunks;
use nfsu2::formats::camera_tracks::{CameraGroup, ROUTE_CAMERAS, parse_camera_tracks};
use nfsu2::formats::roads::Locator;
use nfsu2::track_camera::TrackDirector;
use nfsu2::audio::{fe, mixer::Mixer, streams::MusicContext};
use nfsu2::car::{Car, LodDraw};
use nfsu2::chase::{ChaseCamera, VIEW_ORDER};
use nfsu2::drive::{CarState, Controls};

use nfsu2::fog::FogState;
use nfsu2::screen_tint::ScreenTintState;
use nfsu2::sky::TimeOfDay;
use nfsu2::frontend::career::{self, CareerData, Profile, Route};
use nfsu2::frontend::dday;
use nfsu2::frontend::options::{self, OptionsKey};
use nfsu2::frontend::graphics::{self, DecalItem, Setup};
use nfsu2::frontend::profiles;
use nfsu2::frontend::screens::{self, McList, McMessage, McOp, Action, CarChoice, Placeholder, CarLot, Dialog, Engage, IconMenu, InGameWorldMap, Key, LotState, McMain, McOutcome, McStep, PartsBrowser, PostRaceResults, QuickRace, ResultsKind, ShopEngage, WorldMap};
use nfsu2::frontend::{Assets, Quad, ScreenState, hud, hud::Hud, text_quads};
use nfsu2::race::RaceMode;
use nfsu2::render::car::CarRenderer;
use nfsu2::render::display::Display;
use nfsu2::render::lighting::{CameraAxes, CarLighting};
use nfsu2::render::postfx::{Frame as PostFrame, PostFx, Switches};
use nfsu2::render::ui::UiRenderer;
use nfsu2::smackable::Props;
use nfsu2::render::{Camera, FrameParams, Gpu, HeadlightPool, WorldRenderer, create_depth};
use nfsu2::world::{BarrierSetup, World};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};


// `struct Game` (src/bin/game/state.rs).
#[path = "game/state.rs"]
mod state;
pub(crate) use state::Game;

fn main() -> Result<()> {
    shell::startup::main()
}

#[cfg(feature = "dev-harness")]
#[path = "game/dev/mod.rs"]
mod dev;
#[path = "game/enh/mod.rs"]
mod enh;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::{first_drag, first_race, parity, playthrough, post_race, state_hash};
pub(crate) use enh::{mark, photo};

// The dev hooks' facade (stubs without `dev-harness`).
#[path = "game/dev_hooks.rs"]
mod dev_hooks;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::hooks::SHOT_RAIN_CLOCK;

#[cfg(all(test, feature = "dev-harness"))]
#[path = "game/tests/mod.rs"]
mod tests;
#[cfg(all(test, feature = "dev-harness"))]
pub(crate) use tests::test_isolation;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::shot::shot;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::shot::hdr::{fake_hdr, hdr_shot};
#[cfg(feature = "dev-harness")]
pub(crate) use dev::shot::fit::{install_shot_fit, install_shot_hydraulics, install_shot_neon, install_shot_vinyl};
#[cfg(feature = "dev-harness")]
pub(crate) use dev::selftest::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::selftest::free_roam::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::selftest::menus::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::selftest::profile_manager::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::selftest::showcase::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::selftest::customize::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::selftest::drift::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::selftest::garage::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::selftest::audio_lines::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::selftest::dday_paths::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::selftest::resume::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::drivers::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::util::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::flow::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::flow::common::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::flow::outrun::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::flow::rain::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::flow::resume::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::flow::intro_nag::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::flow::soak::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::flow::engine::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::audio_capture::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::bench::{bench, env::*, latency::*, rims::*, scene::*, transitions::*};
#[cfg(feature = "dev-harness")]
pub(crate) use dev::checks::shops::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::headless::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::intro_replay::*;
#[cfg(feature = "dev-harness")]
pub(crate) use dev::misc::*;
#[path = "game/menus/mod.rs"]
mod menus;
#[path = "game/career_glue/mod.rs"]
mod career_glue;
#[path = "game/flow/mod.rs"]
mod flow;
#[path = "game/shops/mod.rs"]
mod shops;
#[path = "game/loads/mod.rs"]
mod loads;
pub(crate) use menus::{dialogs::*, keys::online::*, model::*};
pub(crate) use career_glue::{challenges::*, uniques::*};
pub(crate) use flow::{movies::*, race_end::*};
pub(crate) use shops::{graphics::*, performance::*};
pub(crate) use loads::{loading_screen::*, preload::*};
#[path = "game/sim/mod.rs"]
mod sim;
pub(crate) use render::state::RenderState;
pub(crate) use sim::car_id::{CarId, DrawId};
pub(crate) use sim::drive::*;
pub(crate) use sim::effects::*;
pub(crate) use sim::race::*;
pub(crate) use sim::ai::autopilot::*;
pub(crate) use sim::ai::drag_steer::*;
pub(crate) use sim::ai::mover::*;
pub(crate) use sim::ai::racer::*;
pub(crate) use sim::ai::rival::*;
pub(crate) use sim::event_camera::*;
pub(crate) use sim::race::frame::*;
pub(crate) use sim::race::step::*;
pub(crate) use sim::replay::camera::*;
pub(crate) use sim::replay::playback::*;
pub(crate) use sim::replay::record::*;
pub(crate) use sim::replay::snapshot::*;
pub(crate) use loads::cache::*;
pub(crate) use sim::encounters::*;
pub(crate) use sim::encounters::ai::*;
pub(crate) use sim::encounters::draw::*;
pub(crate) use sim::explore::*;
pub(crate) use sim::traffic::*;
pub(crate) use sim::traffic::place::*;
pub(crate) use sim::traffic::think::*;
#[path = "game/shell/mod.rs"]
mod shell;
pub(crate) use shell::cli::*;
pub(crate) use shell::first_run::*;
pub(crate) use shell::window::*;
pub(crate) use shell::debug_env::*;
#[path = "game/audio/mod.rs"]
mod audio;
pub(crate) use audio::Audio;
pub(crate) use audio::soundfx::*;
pub(crate) use audio::engine::sound_rpm;
#[path = "game/input/mod.rs"]
mod input;
pub(crate) use input::*;
#[path = "game/render/mod.rs"]
mod render;
pub(crate) use render::enh::frame_inputs::*;
pub(crate) use render::enh::post::*;
#[cfg_attr(not(feature = "dev-harness"), allow(unused_imports))] // used by the dev harness only
pub(crate) use render::frame::*;
pub(crate) use render::native::car_lod::*;
pub(crate) use render::native::cars::*;
pub(crate) use render::native::cover::*;
pub(crate) use render::native::flares::*;
pub(crate) use render::native::fog::*;
pub(crate) use render::native::glows::*;
pub(crate) use render::native::markers::*;
pub(crate) use render::native::motion_blur::*;
pub(crate) use render::native::particles::*;
pub(crate) use render::native::rear_view::*;
pub(crate) use render::native::shadows::*;
#[path = "game/garage/mod.rs"]
mod garage;
pub(crate) use garage::{draw::*, request::*, rig::*, scene::*};
#[path = "game/hud/mod.rs"]
mod hud_glue;
#[path = "game/ui/mod.rs"]
mod ui;
#[path = "game/dyno/mod.rs"]
mod dyno;
#[path = "game/car_build.rs"]
mod car_build;
pub(crate) use dyno::consts::*;
#[path = "game/effects_glue.rs"]
mod effects_glue;
#[path = "game/update/mod.rs"]
mod update;

