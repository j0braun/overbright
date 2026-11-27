use crate::frontend::fe_ids;
use super::*;

/// UI_CareerWorldMap: the stage's events, one at a time.
#[derive(Clone)]
pub struct WorldMap {
    pub events: Vec<usize>,
    pub selected: usize,
    /// The region entry (0x4E0C30, the list's head) is selected: the
    /// stage's whole map, empty rows, no Select Event. The list opens on it
    /// and on each new stage (Wine, the finished-career save).
    pub region: bool,
    pub opened: f32,
    /// The stage the top scroller shows (+0x224, 1 when opened).
    pub stage: u8,
    /// The highest unlocked stage (stage record +0xFC, 0x500C00).
    pub stages: u8,
    /// The map widget (+0x1F4) and the screen's hand-off to it (+0x22C:
    /// stamped when the selection or the stage changes, 0x4CC140,
    /// 0x4E0E50, and by the constructor), stepped on the tick.
    pub widget: MapWidget,
}

/// A track-map widget (0x4CDD40's object: the World Map's +0x1F4, Quick
/// Race's track select's +0x118) and its screen's hand-off to it, stepped
/// once per tick ([`MapWidget::step`]); the draw only reads it
/// ([`MapWidget::view`]).
///
/// * The screen stamps its field (World Map +0x22C at 0x4CC1D9 / 0x4E0EE4,
///   track select +0x14C at 0x4E32E8) with the real-time clock 0x8651A8
///   on a change. While it is set (0x401820: not 0, not 0x7FFFFFFF) the
///   screen's tick (0x4E12DF, 0x4EFC30) waits for (0x8651A8 − it) ×
///   0.00025 ≥ 0.3 (0x784268, 0x78758C: `fcomp; test ah, 1; jne`), then
///   hands the selected track to the widget (0x498600: +0xC) and clears
///   the stamp; on those ticks the widget's own tick does not run.
/// * Otherwise the widget's tick (0x4CDD40) runs: a handed track not yet
///   shown (+0xC ≠ +0x1C) loads TRACKMAP<n> and, loaded, shows it and
///   starts the zoom (0x4B4B80: +0x28 = _ftol((0.5 + 0.5) × 4000 + 0.5) =
///   4000, 0x78DBCC + 0x78DBD0, 0x784264, 0x784260); in the same tick, and
///   every later one, the countdown +0x28 −= _ftol(dt × 4000) (0x577320
///   with the quantised dt 0x865194; at or below 0 it stays 0 and the
///   uv is no longer set).
///
/// **Inferred**: the TRACKMAP load. The original loads it asynchronously
/// (0x57CF80, its callback 0x4AEBF0 flags +0x18) and shows the track on a
/// later tick; the clone has the maps loaded with the screen and shows it
/// on the widget's first tick after the hand-off.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MapWidget {
    /// The screen's stamp (0x8651A8, 1/4000 s); None once handed off.
    pub changed: Option<u64>,
    /// +0xC: the track handed to the widget.
    pub handed: Option<u16>,
    /// +0x1C: the track the widget shows.
    pub shown: Option<u16>,
    /// +0x28: the zoom's countdown (1/4000 s).
    pub count: i32,
}

impl MapWidget {
    /// 0x78758C: the hand-off 0.3 s after the screen's change.
    pub const HAND_OFF: f32 = 0.3;
    /// 0x4B4B80's countdown: _ftol((0.5 + 0.5) × 4000 + 0.5).
    pub const ZOOM_COUNT: i32 = 4000;

    /// A widget whose screen changed at `real` (0x8651A8), nothing shown.
    pub fn changed_at(real: u64) -> MapWidget {
        MapWidget { changed: Some(real), ..Default::default() }
    }

    /// A widget showing `track` from the start, its zoom about to begin.
    pub fn showing(track: Option<u16>) -> MapWidget {
        MapWidget { changed: None, handed: track, shown: track, count: if track.is_some() { Self::ZOOM_COUNT } else { 0 } }
    }

    /// The screen's change at `real` (+0x22C / +0x14C stamped).
    pub fn touch(&mut self, real: u64) {
        self.changed = Some(real);
    }

    /// One tick: `real` the real-time clock as the tick reads it, `dt` the
    /// tick's quantised dt, `track` the screen's selected track.
    pub fn step(&mut self, real: u64, dt: f32, track: Option<u16>) {
        if let Some(at) = self.changed {
            if crate::game_clock::real_at_least(real, at, Self::HAND_OFF) {
                // 0x498600 with no track leaves the widget as it is.
                if track.is_some() {
                    self.handed = track;
                }
                self.changed = None;
            }
            return;
        }
        if self.handed.is_some() && self.handed != self.shown {
            self.shown = self.handed;
            self.count = Self::ZOOM_COUNT;
        }
        if self.shown.is_some() && self.count > 0 {
            self.count = (self.count - crate::game_clock::countdown_step(dt)).max(0);
        }
    }

    /// The shown track and its zoom countdown, for the draw.
    pub fn view(&self) -> Option<(u16, i32)> {
        self.shown.map(|t| (t, self.count))
    }
}

/// One frame of the World Map widget (screen +0x1F4, 0x4CDD40): the map
/// object's texture TRACKMAP<n> and its UV window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapView {
    pub track: u16,
    pub uv: [Vec2; 2],
}

/// 0x498630(t): the map object's UV corners (0x5179D0 5 / 6) between the
/// whole map (t = 0) and the track's window (t = 1): s = sin(t·π/2)
/// (0x43C350), corners (x·s, y·s)/512 and (x·s + 512 − (512 − size)·s,
/// y·s + …)/512, window (x, y, size) = track record +0x118 / +0x11C /
/// +0x120.
pub fn map_window_uv(window: [f32; 3], t: f32) -> [Vec2; 2] {
    let s = (t * std::f32::consts::FRAC_PI_2).sin();
    let (x, y) = (window[0] * s, window[1] * s);
    let w = 512.0 - (512.0 - window[2]) * s;
    [Vec2::new(x, y) / 512.0, Vec2::new(x + w, y + w) / 512.0]
}

/// The zoom 0x4CDD40 gives a track `elapsed` seconds after it showed:
/// 0x4B4B80 starts a countdown r of ftol(1.0 × 4000 + 0.5) ticks with the
/// whole map, or the window at once for +0x124, or the whole map for a
/// size of 0; while 0 < r ≤ 0.5 s, t = 1 − r / 0.5 (0x784268 0.00025,
/// 0x78DBCC 0.5); at r = 0 the last step stays (the clone takes t = 1).
pub fn map_zoom(window: [f32; 3], fixed: bool, elapsed: f32) -> f32 {
    if window[2] == 0.0 {
        return 0.0;
    }
    if fixed {
        return 1.0;
    }
    let r = 1.0 - elapsed;
    if r > 0.5 {
        0.0
    } else if r > 0.0 {
        1.0 - r / 0.5
    } else {
        1.0
    }
}

/// [`map_zoom`] from the widget's countdown `count` (+0x28, 1/4000 s,
/// [`MapWidget`]): r = count × 0.00025 (0x784268) against 0.5 (0x78DBCC,
/// `fcom; test ah, 0x41; jp`: r > 0.5 leaves the whole map), t = 1 − r /
/// 0.5 (0x784250); a countdown at 0 is t = 1 (the clone's end; the
/// original keeps its last step).
pub fn map_zoom_count(window: [f32; 3], fixed: bool, count: i32) -> f32 {
    if window[2] == 0.0 {
        return 0.0;
    }
    if fixed || count <= 0 {
        return 1.0;
    }
    let r = f64::from(count) * f64::from(0.00025f32);
    if r > 0.5 { 0.0 } else { (1.0 - r / 0.5) as f32 }
}

pub const STATUS: u32 = fe_ids::STATUS;
pub const TYPE: u32 = fe_ids::WORLD_MAP_TYPE;
pub const DISTANCE: u32 = fe_ids::WORLD_MAP_DISTANCE;
pub const TIME: u32 = fe_ids::TIME;
pub const STAGE: u32 = fe_ids::STAGE;
pub const MAP: u32 = fe_ids::IN_GAME_MAP_MAP;

/// The World Map's region entries (0x4E0B50: table 0x7F4388, stride 0x14,
/// by stage): the region's map track (+0xC: TRACKMAP4500 … 4504) and its
/// zoom window (+0, +4, +8: x, y, size in texels of the 512 map).
const WORLD_MAP_REGIONS: [(u16, [f32; 3]); 5] = [
    (4500, [163.0, 154.0, 350.0]),
    (4501, [126.0, 93.0, 387.0]),
    (4502, [17.0, 5.0, 480.0]),
    (4503, [0.0, 0.0, 512.0]),
    (4504, [0.0, 0.0, 512.0]),
];

/// The region entry's map track for `stage` (1–5).
pub fn world_map_region_track(stage: u8) -> Option<u16> {
    WORLD_MAP_REGIONS.get(usize::from(stage).checked_sub(1)?).map(|r| r.0)
}

/// A region map track's zoom window.
pub fn world_map_region(track: u16) -> Option<[f32; 3]> {
    WORLD_MAP_REGIONS.iter().find(|r| r.0 == track).map(|r| r.1)
}

impl WorldMap {
    pub fn key(&mut self, key: Key) -> Option<usize> {
        match key {
            // The stage scroller steps between unlocked stages and stops at
            // its ends (0x4E0E50); the event scroller stops at its head
            // (the region entry) and tail (0x4CC140 returns there).
            Key::Left if self.stage > 1 => self.stage -= 1,
            Key::Right if self.stage < self.stages => self.stage += 1,
            Key::Up if !self.region && self.selected == 0 => self.region = true,
            Key::Up if !self.region => self.selected -= 1,
            Key::Down if self.region && !self.events.is_empty() => {
                self.region = false;
                self.selected = 0;
            }
            Key::Down if !self.region && self.selected + 1 < self.events.len() => self.selected += 1,
            // The region entry is skipped (0x51F7F0 on position 1).
            Key::Accept if !self.region && !self.events.is_empty() => return Some(self.events[self.selected]),
            _ => {}
        }
        None
    }

    /// The selected event, None on the region entry.
    pub fn event(&self) -> Option<usize> {
        if self.region { None } else { self.events.get(self.selected).copied() }
    }

    /// The help mask (0x4CC140 → 0x54E6F0): the screen's, with Select
    /// Event (0x2000000) on an event (item +0x34 set).
    pub fn help_mask(&self) -> Option<u32> {
        help_mask("UI_CareerWorldMap").map(|m| if self.event().is_some() { m | 0x200_0000 } else { m })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn quads(
        &self,
        assets: &Assets,
        data: &CareerData,
        profile: &Profile,
        length: Option<f32>,
        metric: bool,
        view: Option<MapView>,
        now: f32,
    ) -> Vec<Quad> {
        let Some(package) = assets.package("UI_CareerWorldMap") else { return Vec::new() };
        let mut state = ScreenState::default();
        footer(&mut state, package, "Race");
        retitle(&mut state, package, "Region 1", "");
        state.text.insert(ui_main::CASH, crate::frontend::grouped(profile.cash as i64));
        state.hidden.insert(ui_main::PLAYER_NAME);
        let event: Option<&Event> = self.event().map(|i| &data.events[i]);
        let label = |h: u32, fallback: &str| assets.strings.get(h).unwrap_or(fallback).to_string();
        // The region entry's rows (0x496EF0): every label and value "".
        if self.region {
            for h in [
                world_map::STATUS,
                world_map::TYPE,
                world_map::DISTANCE,
                world_map::TIME,
                fe_ids::OBJ_4D2F5C61,
                fe_ids::OBJ_4D2F5C62,
                fe_ids::OBJ_4D2F5C63,
                fe_ids::OBJ_4D2F5C64,
            ] {
                state.text.insert(h, String::new());
            }
        }
        // An event's rows (0x496D70): "Event Status" Completed / Not
        // Completed, "Race Type:" the type's name (0x496D10, no laps),
        // "Distance:" `%$.2f %s` in km or mi (0x83AA1A: × 0.001, × 0.62137
        // for mi), then by type "Best Score:" (drift, `%$d`), "Best Time:"
        // (sprint, drag) or "Best Lap:".
        if let Some(e) = event {
            let won = profile.completed.contains(&e.index);
            state.text.insert(fe_ids::OBJ_4D2F5C61, label(0xED8C_A191, "Event Status"));
            state.text.insert(
                world_map::STATUS,
                if won { label(fe_ids::COMPLETED, "Completed") } else { label(0xD11E_CD79, "Not Completed") },
            );
            state.text.insert(fe_ids::OBJ_4D2F5C62, label(0x7F90_4AE8, "Race Type:"));
            state.text.insert(world_map::TYPE, e.kind.label().to_string());
            state.text.insert(fe_ids::OBJ_4D2F5C63, label(0x7019_ADD7, "Distance:"));
            let (scale, unit) =
                if metric { (0.001, label(fe_ids::TXT_KM, "km")) } else { (0.001 * 0.621_37, label(fe_ids::TXT_MI, "mi")) };
            state.text.insert(
                world_map::DISTANCE,
                length.map_or("--".into(), |m| format!("{} {unit}", fixed2(m.floor() * scale))),
            );
            let best = match e.kind {
                crate::frontend::career::EventKind::Drift => label(0x4E0A_36A0, "Best Score:"),
                crate::frontend::career::EventKind::Sprint | crate::frontend::career::EventKind::Drag => {
                    label(fe_ids::OBJ_DB0C5EFD, "Best Time:")
                }
                _ => label(fe_ids::QR_TRACK_TXT_BEST_LAP_1ECB, "Best Lap:"),
            };
            state.text.insert(fe_ids::OBJ_4D2F5C64, best);
            // The event list's record (0x4B1360): a drift event (type 5)
            // the best score of its track (0x502C50), else the best time
            // (0x502AA0, the track's default with none), in the event's
            // direction.
            let backward = e.variant & 1 != 0;
            let record = match data.tracks.get(&e.track) {
                Some(t) if e.kind == crate::frontend::career::EventKind::Drift => {
                    crate::frontend::grouped(profile.best_score(t.number as i16, backward) as i64)
                }
                // The item's update 0x496D70 formats the ticks with
                // FormatRaceTime 0x577250(flags 0) (0x496E9D): whole
                // seconds and hundredths by integer division, truncated.
                Some(t) => crate::frontend::hud::time_text(profile.best_time(t, backward)),
                None => "--".into(),
            };
            state.text.insert(world_map::TIME, record);
        }
        // 0x4B1590 (after every step, 0x4E0E50 / 0x4CC140, and the list
        // fill 0x4E0CE0): the SideScrollBar bar (screen +0x19C, 0x542AD0
        // "SideScrollBar") by 0x538090(1, +0x228 the entries with the
        // region head, 0x51F7F0 the selection's position from 1); then
        // the arrows by the scrollers' ends: ScrollBar_Arrow_2 (+0x54)
        // hidden on the last unlocked stage, ScrollBar_Arrow_1 (+0x50) on
        // the first; SideScrollBar_Arrow_2 (+0x58) on the list's tail,
        // SideScrollBar_Arrow_1 (+0x5C) on its head (constructor 0x4EE690
        // at 0x4EE798–0x4EE7DB). Wine: Stage 1 shows no left stage
        // arrow and no up arrow on the region entry.
        let entries = 1 + self.events.len();
        let pos = if self.region { 1 } else { self.selected + 2 };
        scroll_bar_at(&mut state, package, "SideScrollBar", false, 1, entries, pos);
        let hide = |state: &mut ScreenState, name: &str, on: bool| {
            let h = fe_hash(name);
            if on {
                state.shown.remove(&h);
                state.hidden.insert(h);
            } else {
                state.hidden.remove(&h);
                state.shown.insert(h);
            }
        };
        hide(&mut state, "ScrollBar_Arrow_2", self.stage >= self.stages);
        hide(&mut state, "ScrollBar_Arrow_1", self.stage <= 1);
        hide(&mut state, "SideScrollBar_Arrow_2", pos == entries);
        hide(&mut state, "SideScrollBar_Arrow_1", pos == 1);
        // 0x4CBF80: the top scroller names each unlocked stage, "Stage %d"
        // (0x0A96B270) on the package's 0xC8E97C9C label.
        let stage = assets.strings.get(0x0A96_B270).unwrap_or("Stage %d").replace("%d", &self.stage.to_string());
        state.text.insert(world_map::STAGE, stage);
        state.shown.insert(world_map::STAGE);
        // The map widget (screen +0x1F4, vtable 0x78F8C8 — only its
        // destructor): the constructor hides the map object 0x90FE80F3
        // (0x50CA00) until a TRACKMAP is loaded; then TRACKMAP<track> with
        // the zoomed UV window (0x4CDD40 → 0x498630).
        match view {
            Some(v) => {
                state.image.insert(world_map::MAP, crate::formats::hash::bin_hash(&format!("TRACKMAP{}", v.track)));
                state.uv.insert(world_map::MAP, [v.uv[0].x, v.uv[0].y, v.uv[1].x, v.uv[1].y]);
                state.shown.insert(world_map::MAP);
            }
            None => {
                state.hidden.insert(world_map::MAP);
            }
        }
        let mut quads = layout(assets, package, &state, now - self.opened);
        // The event list (screen +0x138, 0x5435E0 with "MINI_MASTER" /
        // "EVENT_ICONS"; extent +0x54 = 273, gap +0x60 = 20, vertical,
        // constructor 0x4EE835–0x4EE84C): one U2_MINI_ICON item per entry,
        // the region head first (0x4E0CE0: 0x52ED90 then 0x50C990 with
        // 0x505450("U2_MINI_ICON")), the selected one on MINI_MASTER
        // (0x538770 → 0x52EEB0). Wine: three dots under the region
        // entry's box, one above and three below the first event's.
        let icons = vec![bin_hash("U2_MINI_ICON"); entries];
        // The items' z (EVENT_ICONS, 130) lies behind the selection's
        // U2_mini_icon_backing (z 10) and _highlight (z 1) on MINI_MASTER,
        // so the selected dot shows through the dark backing (Wine:
        // a dim grey dot in the box; the parts browser's backing is at z
        // 990, behind its items).
        let front: Vec<Quad> = {
            let (front, back): (Vec<Quad>, Vec<Quad>) =
                quads.into_iter().partition(|q| matches!(q.texture, Some(fe_ids::U2_MINI_ICON_BACKING | 0xDA3C_4771)));
            quads = back;
            front
        };
        quads.extend(scroller_quads(
            package,
            fe_hash("MINI_MASTER"),
            fe_hash("EVENT_ICONS"),
            &icons,
            pos - 1,
            (pos - 1) as f32,
            false,
            20.0,
            273.0,
        ));
        quads.extend(front);
        quads
    }
}
