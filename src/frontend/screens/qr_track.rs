use crate::frontend::fe_ids;
use super::*;

/// UI_QRTrackSelect (constructor 0x4E2F30, vtable 0x7974F8, handler
/// 0x4EFA10): the mode's track list (0x4CDED0, [`quick_race::track_list`])
/// on an IconScroller across ICON_MASTER (TRACK_ICON clones, gap 40,
/// screen +0xBC), the map widget (0x90FE80F3, screen +0x118) and the
/// selected track's facts (0x4CE170).
#[derive(Clone)]
pub struct QuickRace {
    pub mode: QrMode,
    pub tracks: Vec<quick_race::QrTrack>,
    pub selected: usize,
    pub opened: f32,
    /// The map widget (+0x118) and the screen's hand-off to it (+0x14C,
    /// stamped by a move, 0x4E32E8; [`MapWidget`]).
    pub widget: MapWidget,
    /// The track strip's arrows' blink ([`ArrowBlink`]: restarted at each
    /// step, as under Wine).
    pub arrows: ArrowBlink,
}

impl QuickRace {
    /// The screen for `mode` on the record's track (0x4CDED0; the record
    /// takes the first track when its own is not open). On opening, Lap KO
    /// is cleared (0x4E303A: record +0x10 = 0). None without a track.
    pub fn for_mode(
        data: &CareerData,
        mode: QrMode,
        record: &mut quick_race::QrRecord,
        won: &dyn Fn(&Event) -> bool,
        now: f32,
    ) -> Option<QuickRace> {
        let tracks = quick_race::track_list(data, quick_race::race_type(mode), won);
        if tracks.is_empty() {
            return None;
        }
        record.lap_ko = false;
        let selected = quick_race::preselect(&tracks, record);
        // **Inferred**: the widget has the first track from the opening (the
        // original's first frame shows its TRACKMAP unzoomed; the screen's
        // own hand-off, 0x4EFC72, waits on a move's stamp +0x14C), its zoom
        // counting from the first tick.
        let widget = MapWidget::showing(tracks.get(selected).map(|t| t.number));
        Some(QuickRace { mode, tracks, selected, opened: now, widget, arrows: ArrowBlink::default() })
    }

    /// The selected track.
    pub fn track(&self) -> Option<quick_race::QrTrack> {
        self.tracks.get(self.selected).copied()
    }

    /// Left / Right (0x9120409E / 0xB5971BF1) step the list and stop at its
    /// ends; a move puts the track in the record with the forward
    /// direction (0x498740: record +0x08 = the track, +0x0C = list node
    /// +0x10, 0). Accept (0x0C407210) on an open track (node +0x08 clear)
    /// returns it.
    /// `real`: the real-time clock 0x8651A8 a move stamps (+0x14C).
    pub fn key(&mut self, key: Key, record: &mut quick_race::QrRecord, now: f32, real: u64) -> Option<u16> {
        let n = self.tracks.len();
        let moved = match key {
            Key::Left if self.selected > 0 => {
                self.selected -= 1;
                self.arrows.step(false, now, true);
                true
            }
            Key::Right if self.selected + 1 < n => {
                self.selected += 1;
                self.arrows.step(true, now, true);
                true
            }
            Key::Accept => {
                let picked = self.track().filter(|t| !t.locked).map(|t| t.number);
                // UI_QRModeOptions is built next: its Opponents row's
                // constructor (0x4B3750) clamps a drag on 4402.
                if picked.is_some() {
                    quick_race::clamp_opponents_on_open(quick_race::race_type(self.mode), record);
                }
                return picked;
            }
            _ => false,
        };
        if moved && let Some(t) = self.track() {
            record.track = t.number;
            record.reversed = false;
            self.widget.touch(real);
        }
        None
    }

    /// UI_QRTrackSelect as 0x4E2F30 / 0x4CE170 fill it: "Track Number
    /// DEBUG" (0x53C956C4) hidden; the option title (0xF601F2D4) the
    /// track's name (0x516280: TRACK_NAME_<n>); the race type (0x15E1351B,
    /// 0x4988C0) the mode's label alone (g_RaceOptions 0 Circuit, 1 Sprint,
    /// 2 Drag, 3 Drift, 4 URL Race, 9 Free Run, 10 Street X); the distance
    /// (0x080C9DAA) "%$.2f %s" of the track's +0x98 metres (an int) ×
    /// 0.001, × 0.62137 with "mi" (0xAC3A4F4C) or "km" (0xB5D640B1) by the
    /// units (0x83AA1A); the record label (0x36227EE9) "Best Time:"
    /// (0xDB0C5EFD) for sprint and drag, "Best Lap" (0x3B7B9567) for drift,
    /// else "Best Lap:" (0x3B4A1ECB), and the record (0x0555EECA)
    /// FormatRaceTime(best, 0) of the profile's best (0x502AA0; none → the
    /// track's default, 0 for the tracks seen) or, for drift, "%$d" of the
    /// best score (0x502C50). The arrows (0x444969FD / FE) hide at the
    /// list's ends (0x4CE170, 0x4EFA62). The strip: each track's icon
    /// ([`quick_race::icon`], 0x74CE8C0B when its texture is missing) on
    /// the scroller; the map shows TRACKMAP<n> through the widget's window.
    pub fn quads(
        &self,
        assets: &Assets,
        data: &CareerData,
        metric: bool,
        best: Option<f32>,
        view: Option<MapView>,
        now: f32,
    ) -> Vec<Quad> {
        let Some(package) = assets.package("UI_QRTrackSelect") else { return Vec::new() };
        let mut state = ScreenState::default();
        footer(&mut state, package, "Race");
        for hidden in ["EA Messenger", "$JOY_EVENT_FENG_BUTTON1$", "PLAYER NAME", "$999999999"] {
            for o in package.objects.iter().filter(|o| o.text == hidden) {
                state.hidden.insert(o.hash);
            }
        }
        retitle(&mut state, package, "Continue", &format!("{}: Race", crate::pad::extras::accept_name()));
        state.hidden.insert(0x53C9_56C4);
        // The track strip's scroller (0x5435E0, built by 0x4E2F30 with
        // "ICON_MASTER" / "TRACK_ICON") hides its master (0x50CA00 on it):
        // ICON_MASTER has no texture and drew as a flat white box under the
        // selected icon (Wine 1280 × 960: the box's inside 69,72,64, the
        // clone's 140,144,135).
        state.hidden.insert(fe_hash("ICON_MASTER"));
        // ScrollBar (0x538090(1, count +0x150, position)); the list's
        // ScrollBar_Arrow_1 hides on the first track, _2 on the last.
        let n = self.tracks.len();
        scroll_bar_at(&mut state, package, "ScrollBar", true, 1, n, if n == 0 { 0 } else { self.selected + 1 });
        if self.selected == 0 {
            state.hidden.insert(fe_hash("ScrollBar_Arrow_1"));
        } else if self.selected + 1 == n {
            state.hidden.insert(fe_hash("ScrollBar_Arrow_2"));
        }
        self.arrows.apply(&mut state, SCROLLBAR_ARROWS, self.opened);
        match view {
            Some(v) if assets.textures.contains_key(&bin_hash(&format!("TRACKMAP{}", v.track))) => {
                state.image.insert(world_map::MAP, bin_hash(&format!("TRACKMAP{}", v.track)));
                state.uv.insert(world_map::MAP, [v.uv[0].x, v.uv[0].y, v.uv[1].x, v.uv[1].y]);
            }
            _ => {}
        }
        let label = |h: u32| assets.strings.get(h).unwrap_or("").to_string();
        let ty = quick_race::race_type(self.mode);
        if let Some(t) = self.track().and_then(|t| data.tracks.get(&t.number)) {
            retitle(&mut state, package, "Option title", &quick_race::track_name(assets, data, t.number, false));
            let kind = match ty {
                1 => fe_ids::QR_MODE_SPRINT_LABEL,
                2 => fe_ids::QR_MODE_DRAG_LABEL,
                3 => fe_ids::QR_MODE_DRIFT_LABEL,
                4 => 0x75F9_03F4,
                10 => fe_ids::QR_MODE_STREET_X_LABEL,
                _ => fe_ids::QR_MODE_CIRCUIT_LABEL,
            };
            state.text.insert(fe_ids::ENGAGE_TYPE, label(kind));
            let mut d = t.length as i32 as f32 * 0.001;
            if !metric {
                d *= 0.62137;
            }
            state.text.insert(
                fe_ids::ENGAGE_DISTANCE,
                format!("{} {}", fixed2(d), label(if metric { fe_ids::TXT_KM } else { fe_ids::TXT_MI })),
            );
            // FormatRaceTime(flags 0) works on the record's ticks (0x502AA0);
            // the caller hands the time over as ticks / 4000, so the ticks
            // are taken back to the nearest (not truncated: 4040 ticks are
            // 1.00999 s in f32, which would print "1.00", not "1.01").
            let best_ticks = (best.unwrap_or(0.0) * 4000.0).round() as i32;
            let (record_label, record) = match ty {
                3 => (fe_ids::QR_TRACK_TXT_BEST_LAP_9567, crate::frontend::grouped(best.unwrap_or(0.0) as i64)),
                1 | 2 => (fe_ids::OBJ_DB0C5EFD, crate::frontend::hud::time_text(best_ticks)),
                _ => (fe_ids::QR_TRACK_TXT_BEST_LAP_1ECB, crate::frontend::hud::time_text(best_ticks)),
            };
            state.text.insert(0x3622_7EE9, label(record_label));
            state.text.insert(0x0555_EECA, record);
        }
        let mut quads = layout(assets, package, &state, now - self.opened);
        let icons: Vec<u32> = self
            .tracks
            .iter()
            .map(|q| {
                let icon =
                    data.tracks.get(&q.number).map_or(quick_race::NO_ICON, |t| quick_race::icon(t, q.locked, ty));
                if assets.textures.contains_key(&icon) { icon } else { quick_race::NO_ICON }
            })
            .collect();
        quads.extend(scroller_quads(
            package,
            fe_hash("ICON_MASTER"),
            fe_hash("TRACK_ICON"),
            &icons,
            self.selected,
            self.selected as f32,
            true,
            40.0,
            480.0,
        ));
        quads
    }
}
