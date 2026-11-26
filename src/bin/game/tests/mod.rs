use crate::*;

// The test kit for the harness's own test modules (src/bin/game/dev/).
#[path = "../../../../tests/common/mod.rs"]
pub(crate) mod testkit;
mod basics;
mod dyno_audio;
mod encounters;
// Free roam's encounter racers loaded ahead in the menus.
#[cfg(test)]
mod encounters_ahead;
// The Enhanced effects on ticks.
#[cfg(test)]
mod enh_ticks;
mod fe_real_time_clock;
mod fe_ticks;
mod frontend;
#[cfg(test)]
mod game_side_fixes;
mod hud_texture_list;
#[cfg(test)]
mod impact_shake;
#[cfg(test)]
mod native_dialogs;
#[cfg(test)]
mod native_fe_part2;
#[cfg(test)]
mod intro_skip_grid;
#[cfg(test)]
mod pad_navigation;
#[cfg(test)]
mod countdown_brake;
#[cfg(test)]
mod human_road_path;
#[cfg(test)]
mod outrun_leader_goal;
#[cfg(test)]
mod race_status_clock;
#[cfg(test)]
mod career_playtest;
#[cfg(test)]
mod career_stages_1_2;
mod part_group_owned_bits;
// Photo mode's picture in every window format.
mod photo_formats;
mod physics_ai;
mod race_route;
// The crash camera in free roam.
#[cfg(test)]
mod roam_crash_camera;
#[cfg(test)]
mod rival_pace;
mod rivals_ahead;
// Restart and Next Race: the same field raced again.
#[cfg(test)]
mod race_again;
#[cfg(test)]
mod rumble_feed;
#[cfg(test)]
mod rate;
mod replay;
mod replay_steps;
mod rng;
mod saves_shops;
mod scene_audio;
mod scenes;
// A tick's settings and a frame's (docs/developing/frame-settings.md).
#[cfg(test)]
mod settings_moments;
// The tick snapshots.
mod snapshots;
// The draw's clocks.
mod draw_clocks;
// The garage's own scene between ticks.
mod garage_between;
mod garage_drag;
mod garage_lot_turn;
// The garage scenes draw no weather.
mod garage_drive_ticks;
mod garage_weather;
// The track-map widgets on the tick.
mod map_widget;
// The environment cube on ticks.
mod env_ticks;
// The enhancement effects on ticks.
mod tick_effects;
// The cars' ground effects (tyre smoke, spray, bottom-out).
#[cfg(test)]
mod ground_effects;
// The fog's and the rain's faster passes against their plain forms.
#[cfg(test)]
mod fog_rain_paths;
// The streak buffer on ticks.
mod streak_ticks;
// No program made in a drawn frame after a load or a settings change.
#[cfg(test)]
mod first_use;
// A world kept for the next drive draws what a fresh load draws.
#[cfg(test)]
mod kept_world;
// Light Probes' arrival burst: one wait for the GPU, the same field.
#[cfg(test)]
mod probe_burst;
#[cfg(test)]
mod race_dialog_movies;
// The waits log's hooks (docs/developing/waits-log.md).
#[cfg(test)]
mod waits_log;
// The start's and the menus' waits (docs/original/start-and-menus.md).
#[cfg(test)]
mod start_and_menus;
#[cfg(test)]
pub(crate) mod test_isolation;
mod traffic;
mod traffic_paused_coast;
// The traffic parity harness (docs/traffic-harness.md).
mod traffic_harness;
#[cfg(test)]
mod visuals;
#[cfg(test)]
mod reported_issues;
#[cfg(test)]
mod dyno_entry;
// The manual gearbox through the player's keys and controller.
#[cfg(test)]
mod manual_gears;
// The map editor and custom races (docs/modding/map-editor.md).
#[cfg(test)]
mod map_editor;
