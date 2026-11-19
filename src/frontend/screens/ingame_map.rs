use crate::frontend::fe_ids;
use super::*;

/// UI_InGame_WorldMap (screen record 0x7F827C: factory 0x4F9C10,
/// constructor 0x4EE320, messages 0x4E08E0; the map widget 0x4E0440 at
/// screen +0x4C), opened in career free roam by the World Map key (code
/// 0xC0, 0x602BE0 → flow event 0x12 → 0x60B3B0 pauses in mode 6, whose
/// handler 0x606740 opens the package). Back (0x911AB364) closes it the
/// next tick and the game resumes (flow event 8, 0x5F4250).
///
/// The map (TRACK2000_MAP, 0x90FE80F3) shows TRACKMAP<track> through a
/// window from the zoom table 0x7F42C8 ({cx, cy, size, label} in texels
/// of 512): UV (cx, cy) / 512 … (cx + size, cy + size) / 512; `[` / `]`
/// (0x5073EF13 / 0xD9FEEC59, 0x4CBDA0) step it down / up, wrapping, up to
/// the stage's last record (0x7F4328 by 0x862838). Each icon (0x4B09E0)
/// sits at the map's centre + (fx·W, fy·H) with fx = ((x − left)/size −
/// uc)/span, fy = ((top − y)/size − vc)/span, span = size/512 and (uc, vc)
/// the window's centre; it shows only strictly inside the map (0x496760),
/// 11 × 11 at zoom 0 and 15 × 15 zoomed, turned by its heading. Icon
/// objects are `<NAME><n>` from a per-type counter that starts at 1 (AI
/// racers at 0; 0x835F38).
#[derive(Clone, Debug)]
pub struct InGameWorldMap {
    pub opened: f32,
    pub zoom: usize,
    /// The stage's last zoom record (0x7F4328).
    pub max_zoom: usize,
    /// TRACKMAP<n> and the track record's frame: left x, top y, size.
    pub texture: Option<u32>,
    pub frame: [f32; 3],
    /// The widget's items in its order (0x4E0440): the marker table's
    /// entries (0x500EE0) that pass 0x533120, then the active AI cars
    /// (0x4B0740, group 0xE), then the player (group 0x10).
    pub icons: Vec<MapIcon>,
    /// The selected marker (the widget's +0x14, an index into `icons`;
    /// only marker entries are selectable, list +0xC); the selection group
    /// 0xE2848973 pulses on it.
    pub selected: Option<usize>,
    /// The legend rows (0x4EE320): one per group the markers show, in the
    /// order first met, each with the marker type that made it (its
    /// label, 0x4966A0).
    pub legend: Vec<(u8, u8)>,
    /// The focused legend row (screen +0x5C).
    pub focus: Option<usize>,
    /// The groups' hide flags (0x8635A9.., 0x496390 / 0x4964D0).
    pub hidden: [bool; 14],
    /// The GPS is on (0x501100 in 0–3): the help bar offers "Deactivate
    /// GPS" instead of "Activate GPS" (0x4B0FE0).
    pub gps: bool,
    /// The GPS target's hash (0x838590) while it is on.
    pub gps_target: Option<u32>,
}

/// One item on the in-game map (the widget's 0x2C-byte records).
#[derive(Clone, Copy, Debug)]
pub struct MapIcon {
    /// +0x08: the legend group (0x4965F0 of the marker type), 0xE an AI
    /// racer, 0x10 the player.
    pub group: u8,
    /// The marker type (0x500DE0): events 0–5 / 7 (0x5002E0), milestones
    /// 6, shops 9–14 (0x500340), challenges 0xF.
    pub kind: u8,
    /// +0x10 / +0x14: the route trigger box's centre (0x4B0670), a car's
    /// +0x60 / +0x64.
    pub at: Vec2,
    /// A car's heading (radians; car +0x40C).
    pub heading: Option<f32>,
    /// A sponsor event (+0x34 = 1): the icon takes texture 0xB90F50BB.
    pub sponsor: bool,
    /// The marker's hash (0x514160) and route trigger: the GPS target.
    pub target: Option<(u32, crate::frontend::career::Trigger)>,
}

/// The marker type of an event (0x5002E0): +0x0D (SUV) 7, a URL race
/// (+0x34 = 2) 5, else by type (+0x0F): sprint 4, street X 2, drag 1,
/// drift 3, others 0.
pub fn map_event_kind(e: &Event) -> u8 {
    use crate::frontend::career::{Category, EventKind};
    match (e.kind, e.category) {
        _ if e.suv => 7,
        (_, Category::Url) => 5,
        (EventKind::Sprint, _) => 4,
        (EventKind::StreetX, _) => 2,
        (EventKind::Drag, _) => 1,
        (EventKind::Drift, _) => 3,
        _ => 0,
    }
}

/// The marker type of a shop by its kind (+0x50, 0x500340): 1 → 9, 2 →
/// 10, 5 → 11, 3 → 12, 4 → 13, 0 → 14, else 10.
pub fn map_shop_kind(kind: u8) -> u8 {
    match kind {
        1 => 9,
        5 => 11,
        3 => 12,
        4 => 13,
        0 => 14,
        _ => 10,
    }
}

/// The legend group of a marker type (0x4965F0), the index into the icon
/// names 0x7F4348.
pub fn map_group(kind: u8) -> u8 {
    match kind {
        0..=5 => kind,
        6 | 8 | 0xF => 0xD,
        7 => 6,
        9 => 7,
        10 => 8,
        0xB => 0xB,
        0xC => 9,
        0xD => 10,
        0xE => 0xC,
        _ => 0xD,
    }
}

/// The legend label of a marker type (0x4966A0).
pub fn map_label(kind: u8) -> u32 {
    match kind {
        0 => fe_ids::OBJ_268D063C,
        1 => fe_ids::OBJ_40C91BA7,
        2 => fe_ids::OBJ_0FDA6758,
        3 => fe_ids::OBJ_59ECB2C2,
        4 => fe_ids::OBJ_BA613729,
        5 => fe_ids::OBJ_B46377BC,
        6 | 8 | 0xF => 0x2E37_E28B,
        7 => 0xB463_6FA7,
        9 => 0x373D_AD1A,
        10 => 0x40C7_F697,
        0xB => 0xB463_42BA,
        0xC => 0xD6F5_819B,
        0xD => 0x93FF_4A6E,
        0xE => 0x9D53_BD10,
        _ => 0,
    }
}

/// The icon object names by group (0x7F4348).
pub const MAP_ICON_NAMES: [&str; 15] = [
    "CIRCUITEVENTINDICATOR_",
    "DRAGEVENTINDICATOR_",
    "STREETEVENTINDICATOR_",
    "DRIFTEVENTINDICATOR_",
    "SPRINTEVENTINDICATOR_",
    "URLEVENTINDICATOR_",
    "SUVEVENTINDICATOR_",
    "PAINTSHOPINDICATOR_",
    "PARTSSHOPINDICATOR_",
    "PERFORMANCESHOPINDICATOR_",
    "CARLOTINDICATOR_",
    "ICESHOPINDICATOR_",
    "CRIBINDICATOR_",
    "SHOWCASEINDICATOR_",
    "AIRACERINDICATOR_",
];

/// 0x7F42C8: the zoom records (cx, cy, size in texels of the 512 map, the
/// label's string hash: Full Map, City Core, Beacon Hill, Jackson Heights,
/// Coal Harbor East, Coal Harbor West).
pub static WORLD_MAP_ZOOMS: crate::exe_tables::Table<[(f32, f32, f32, u32); 6]> = crate::exe_tables::Table::new(|t| &t.world_map_zooms);

/// 0x7F4328: the last zoom record each stage (0x862838) may use.
pub fn world_map_max_zoom(stage: u8) -> usize {
    [1, 1, 2, 3, 4, 5, 5][usize::from(stage).min(6)]
}

mod in_game_map {
    pub const MAP: u32 = crate::frontend::fe_ids::IN_GAME_MAP_MAP;
    /// The zoom record's label (0x4CBDA0).
    pub const ZOOM_LABEL: u32 = crate::frontend::fe_ids::ZOOM_LABEL;
    /// The selection marker group (0x4B07F0 / 0x4B0850).
    pub const SELECTION: u32 = crate::frontend::fe_ids::SELECTION;
    /// The selection group's two rings (Minimap_GPS_Selector, 50 × 50):
    /// their INIT (length 0) chains to PULSE (1600 ms, looping: size 28.75
    /// → 70, alpha 0 → 255 → 0; the second ring 400 ms behind the first).
    pub const RINGS: [u32; 2] = [0x4F64_9313, 0x4F64_9314];
    /// PLAYERCARINDICATOR: the player's icon (0x4E0440, type 0x10) and the
    /// shown check box's colour (row +0x18, 0x4B0D50).
    pub const PLAYER: u32 = crate::frontend::fe_ids::IN_GAME_MAP_PLAYER;
    /// PAINTSHOPINDICATOR_1: the hidden check box's colour (row +0x1C).
    pub const OFF_COLOUR: u32 = crate::frontend::fe_ids::OFF_COLOUR;
    /// The check box textures (0x496920): shown, hidden; the second is
    /// also a sponsor event's icon (0x4B0670) and the legend's DATA_D.
    pub const BOX_ON: u32 = crate::frontend::fe_ids::BOX_ON;
    pub const BOX_OFF: u32 = crate::frontend::fe_ids::BOX_OFF;
    /// The legend's rows (0x4EE320: 14, 0x496870 / 0x4B0D50).
    pub const ROWS: usize = 14;
}

impl InGameWorldMap {
    /// The screen as its constructor leaves it (0x4EE320 with the widget
    /// 0x4E0440): the legend from the markers; the selection on the GPS
    /// target's marker while the GPS is on (0x501100 in 0–3) and its group
    /// shows, else on the first marker; then the first legend row focused
    /// (0x4B0E70), which with the GPS off and its group shown selects that
    /// group's first marker (0x4B07F0).
    pub fn new(
        opened: f32,
        max_zoom: usize,
        texture: Option<u32>,
        frame: [f32; 3],
        icons: Vec<MapIcon>,
        hidden: [bool; 14],
        gps: Option<u32>,
    ) -> InGameWorldMap {
        let mut legend: Vec<(u8, u8)> = Vec::new();
        // 0x4EE320 makes a row for a group's first marker (0x4B0D50) and
        // then runs 0x496A40 for every marker of the group on its row, so
        // the label (0x4966A0 of the marker's type) is the last marker's.
        for i in icons.iter().filter(|i| i.target.is_some()) {
            if let Some(l) = legend.iter_mut().find(|l| l.0 == i.group) {
                l.1 = i.kind;
            } else if legend.len() < in_game_map::ROWS {
                legend.push((i.group, i.kind));
            }
        }
        let mut m = InGameWorldMap {
            opened,
            zoom: 0,
            max_zoom,
            texture,
            frame,
            icons,
            selected: None,
            legend,
            focus: None,
            hidden,
            gps: gps.is_some(),
            gps_target: gps,
        };
        m.selected = match gps {
            Some(h) => m.marker_with(h).filter(|&i| !m.is_hidden(m.icons[i].group)),
            None => m.markers().next(),
        };
        if !m.legend.is_empty() {
            m.focus_row(0);
        }
        m
    }

    /// The marker entries (the widget's list +0xC), as icon indices.
    fn markers(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.icons.len()).filter(|&i| self.icons[i].target.is_some())
    }

    /// 0x4B07B0: the marker whose hash (0x514160) is `hash`.
    fn marker_with(&self, hash: u32) -> Option<usize> {
        self.markers().find(|&i| self.icons[i].target.is_some_and(|t| t.0 == hash))
    }

    /// 0x4964D0: a group's hide flag (AI racers and the player have none).
    pub fn is_hidden(&self, group: u8) -> bool {
        self.hidden.get(group as usize).copied().unwrap_or(false)
    }

    /// 0x4B0E70: focus a legend row; with the GPS off and the row's group
    /// shown, the selection goes to the group's first marker (0x4B07F0;
    /// none of it: unchanged).
    fn focus_row(&mut self, row: usize) {
        self.focus = Some(row);
        let g = self.legend[row].0;
        let first = self.markers().find(|&i| self.icons[i].group == g);
        if !self.gps && !self.is_hidden(g) && first.is_some() {
            self.selected = first;
        }
    }

    /// Where each icon lands on the canvas (package coordinates), and its
    /// object; None when outside the map (0x496760). Objects are
    /// `<NAME><n>` by a per-group counter from 1 (0x4E0440 sets the 15 of
    /// 0x835F38 to 1; AI racers from 0, 0x835F70 = 0) and the player's
    /// PLAYERCARINDICATOR.
    fn placed(&self, package: &crate::frontend::feng::Package) -> Vec<Option<(u32, Vec2)>> {
        let Some(o) = package.object(in_game_map::MAP) else { return vec![None; self.icons.len()] };
        let (at, size) = (o.attributes.position(), o.attributes.size());
        let (cx, cy, span, _) = WORLD_MAP_ZOOMS[self.zoom];
        let span = span / 512.0;
        let (uc, vc) = (cx / 512.0 + span * 0.5, cy / 512.0 + span * 0.5);
        let [left, top, scale] = self.frame;
        let mut counters = [1u32; 15];
        counters[14] = 0;
        self.icons
            .iter()
            .map(|icon| {
                let h = match MAP_ICON_NAMES.get(icon.group as usize) {
                    Some(name) => {
                        let n = &mut counters[icon.group as usize];
                        *n += 1;
                        fe_hash(&format!("{name}{}", *n - 1))
                    }
                    None => in_game_map::PLAYER,
                };
                let fx = ((icon.at.x - left) / scale - uc) / span;
                let fy = ((top - icon.at.y) / scale - vc) / span;
                (fx.abs() < 0.5 && fy.abs() < 0.5).then(|| (h, Vec2::new(at.x + fx * size.x, at.y + fy * size.y)))
            })
            .collect()
    }

    /// The mouse's zones (0x4CBEE0's list, kind 0): each marker on the
    /// map with its icon object, in the widget's order.
    pub fn mouse_markers(&self, package: &crate::frontend::feng::Package) -> Vec<(usize, u32)> {
        let placed = self.placed(package);
        self.markers().filter_map(|i| placed.get(i).copied().flatten().map(|(h, _)| (i, h))).collect()
    }

    /// 0x4B1170 on a legend row (kind 1): 0x4B0E70, the row focused.
    pub fn hover_row(&mut self, row: usize) {
        if row < self.legend.len() {
            self.focus_row(row);
        }
    }

    /// 0x4CBE70 on a legend row: 0x4B0E70, then 0x4B0EF0 (the group's
    /// hide flag flips).
    pub fn click_row(&mut self, row: usize) {
        if row < self.legend.len() {
            self.focus_row(row);
            self.toggle();
        }
    }

    /// 0x4B1170 on a marker (kind 0): selected with the GPS off and its
    /// group shown (the marker is on the map: `mouse_markers`).
    pub fn hover_marker(&mut self, i: usize) {
        if !self.gps && self.icons.get(i).is_some_and(|m| !self.is_hidden(m.group)) {
            self.selected = Some(i);
        }
    }

    /// 0x4CBE70 on a marker: selected (0x4967F0) when its group is shown;
    /// whether it was (then the GPS button 0x4B10C0 follows).
    pub fn click_marker(&mut self, i: usize) -> bool {
        if self.icons.get(i).is_some_and(|m| !self.is_hidden(m.group)) {
            self.selected = Some(i);
            return true;
        }
        false
    }

    /// 0x4B0850: Left (0x9120409E, 3) steps the selection forward through
    /// the markers, Right (0xB5971BF1, 2) back, wrapping; only markers on
    /// the map count, and only the focused legend row's group when one is
    /// focused (a hidden group: no selection). With none selected the
    /// first that counts; none counts: it stays.
    fn step(&mut self, forward: bool, package: &crate::frontend::feng::Package) {
        let group = self.focus.map(|r| self.legend[r].0);
        if group.is_some_and(|g| self.is_hidden(g)) {
            self.selected = None;
            return;
        }
        let placed = self.placed(package);
        let ok = |i: usize| group.is_none_or(|g| self.icons[i].group == g) && placed[i].is_some();
        let markers: Vec<usize> = self.markers().collect();
        match self.selected.and_then(|s| markers.iter().position(|&m| m == s)) {
            None => self.selected = markers.iter().copied().find(|&i| ok(i)),
            Some(k) => {
                let n = markers.len();
                for d in 1..n {
                    let j = if forward { (k + d) % n } else { (k + n - d) % n };
                    if ok(markers[j]) {
                        self.selected = Some(markers[j]);
                        return;
                    }
                }
            }
        }
    }

    /// The handler's keys (0x4E08E0): `[` / `]` zoom (0x4CBDA0, wrapping
    /// over 0..=max); with the GPS off (0x4AE930) Left / Right step the
    /// selection (0x4B0850); Up (0x72619778, 0x4B1060(0)) / Down
    /// (0x911C0A4B) move the legend focus up / down, wrapping (none
    /// focused: the last row); Accept (0x0C407210 → 0x4B0EF0) toggles the
    /// focused row's group. Back and C are the caller's.
    pub fn key(&mut self, key: Key, package: Option<&crate::frontend::feng::Package>) {
        let n = self.max_zoom + 1;
        let rows = self.legend.len();
        match key {
            Key::LTrigger => self.zoom = (self.zoom + n - 1) % n,
            Key::RTrigger => self.zoom = (self.zoom + 1) % n,
            Key::Left | Key::Right if !self.gps => {
                if let Some(package) = package {
                    self.step(key == Key::Left, package);
                }
            }
            Key::Up if rows > 0 => self.focus_row(match self.focus {
                Some(0) | None => rows - 1,
                Some(k) => k - 1,
            }),
            Key::Down if rows > 0 => self.focus_row(match self.focus {
                None => rows - 1,
                Some(k) => (k + 1) % rows,
            }),
            Key::Accept => self.toggle(),
            _ => {}
        }
    }

    /// 0x4B0EF0: the focused row's group flips (0x496920 / 0x496390);
    /// shown again, the selection goes to its first marker with the GPS
    /// off (0x4B07F0), to the GPS target's marker with it on (0x4B07B0 →
    /// 0x4967F0); then a selection in a hidden group is dropped.
    fn toggle(&mut self) {
        let Some(r) = self.focus else { return };
        let g = self.legend[r].0;
        self.hidden[g as usize] = !self.hidden[g as usize];
        if !self.is_hidden(g) {
            match self.gps_target.filter(|_| self.gps) {
                None => self.focus_row(r),
                Some(h) => {
                    if let Some(i) = self.marker_with(h).filter(|&i| !self.is_hidden(self.icons[i].group)) {
                        self.selected = Some(i);
                    }
                }
            }
        }
        if self.selected.is_some_and(|s| self.is_hidden(self.icons[s].group)) {
            self.selected = None;
        }
    }

    pub fn quads(&self, assets: &Assets, now: f32) -> Vec<Quad> {
        self.quads_with_bar(assets, now, true)
    }

    /// The screen; `bar`: with its help bar. Under the GPS questions
    /// (0x4B10C0's dialogs) the original shows no help bar (Wine:
    /// Back / Activate GPS gone while the question is up).
    pub fn quads_with_bar(&self, assets: &Assets, now: f32, bar: bool) -> Vec<Quad> {
        use in_game_map::*;
        let Some(package) = assets.package("UI_InGame_WorldMap") else { return Vec::new() };
        let mut state = ScreenState::default();
        // The package's init hides its flag-0x40 objects (0x54E4B0): the
        // console footer ("blah 1", "blah", "Back", the $JOY_EVENT_
        // glyphs) and the second "Full Map" (0x632B4FA8).
        package_init(&mut state, package);
        // The rings pulse from the package's start: 0x51CF80's "Pulse" on
        // the group (0x4967F0, 0x4E0440) finds no script of that name on
        // it and does nothing; the rings' own INIT → PULSE chain runs.
        state.chained.extend(RINGS);
        if let Some(t) = self.texture {
            state.image.insert(MAP, t);
        }
        let (cx, cy, span, label) = WORLD_MAP_ZOOMS[self.zoom];
        state.uv.insert(MAP, [cx / 512.0, cy / 512.0, (cx + span) / 512.0, (cy + span) / 512.0]);
        if let Some(s) = assets.strings.get(label) {
            state.text.insert(ZOOM_LABEL, s.to_string());
        }
        let colour = |h: u32| package.object(h).map(|o| o.attributes.color());
        let has = |h: u32| assets.textures.contains_key(&h);
        // 0x4B09E0: 11 × 11 on the full map, 15 × 15 zoomed; shown inside
        // the map when the group is (AI racers and the player always).
        let side = if self.zoom == 0 { 11.0 } else { 15.0 };
        let placed = self.placed(package);
        for (i, p) in placed.iter().enumerate() {
            let Some((h, at)) = p else { continue };
            let icon = &self.icons[i];
            state.position.insert(*h, *at);
            state.size.insert(*h, Vec2::splat(side));
            if let Some(heading) = icon.heading {
                state.angle.insert(*h, (90.0 - heading.to_degrees()).to_radians());
            }
            if icon.sponsor && has(BOX_OFF) {
                state.image.insert(*h, BOX_OFF);
            }
            if self.is_hidden(icon.group) {
                state.hidden.insert(*h);
            } else {
                state.shown.insert(*h);
            }
        }
        match self.selected.and_then(|s| placed.get(s).copied().flatten()) {
            Some((_, at)) => {
                state.position.insert(SELECTION, at);
                state.shown.insert(SELECTION);
            }
            None => {
                state.hidden.insert(SELECTION);
            }
        }
        // The legend (0x496870 hides DATA_A..D of all 14 rows; 0x4B0D50 /
        // 0x496A40 fill one per group): DATA_A the check box (0x496920),
        // DATA_B the group's first icon's texture and colour, DATA_C the
        // type's label (0x4966A0), DATA_D a sponsor event's icon when the
        // group's last marker is one; B and D at alpha 0x3C while hidden.
        for r in 0..ROWS {
            let o = |base: &str| fe_hash(&format!("{base}{}", r + 1));
            let (a, b, c, d) = (o("DATA_A_"), o("DATA_B_"), o("DATA_C_"), o("DATA_D_"));
            let Some(&(group, kind)) = self.legend.get(r) else {
                state.hidden.extend([a, b, c, d]);
                continue;
            };
            let hidden = self.is_hidden(group);
            // 0x496A40, run for each marker of the group in turn, hides
            // DATA_D and shows it again for a sponsor event (event +0x34 =
            // 1): what stays is the group's last marker's.
            let last = self.markers().filter(|&i| self.icons[i].group == group).last().map(|i| self.icons[i]);
            let icon1 = fe_hash(&format!("{}1", MAP_ICON_NAMES[group as usize]));
            let alpha = if hidden { 0x3C as f32 / 255.0 } else { 1.0 };
            state.shown.extend([a, b, c]);
            let (tex, col) = if hidden { (BOX_OFF, colour(OFF_COLOUR)) } else { (BOX_ON, colour(PLAYER)) };
            if has(tex) {
                state.image.insert(a, tex);
            }
            if let Some(col) = col {
                state.color.insert(a, col);
            }
            // DATA_B: the group's indicator's image (0x4B0D50 copies the
            // *current* image of `<GROUP>INDICATOR_1`, 0x50C980 → 0x50C990;
            // **verified**): the cross when that object is a sponsor
            // event's marker (the group's first icon), as the markers get
            // it. Seen under Wine (a community save: the
            // Sprint row shows two crosses).
            let first_sponsor = self.icons.iter().find(|i| i.group == group).is_some_and(|i| i.sponsor);
            let tex1 = if first_sponsor && has(BOX_OFF) {
                Some(BOX_OFF)
            } else {
                package
                    .object(icon1)
                    .and_then(|o| package.resources.get(o.resource as usize))
                    .and_then(|r| assets.texture_for(&r.name))
            };
            if let Some(t) = tex1 {
                state.image.insert(b, t);
            }
            if let Some(mut col) = colour(icon1) {
                col[3] = alpha;
                state.color.insert(b, col);
                if last.is_some_and(|f| f.sponsor) {
                    state.shown.insert(d);
                    if has(BOX_OFF) {
                        state.image.insert(d, BOX_OFF);
                    }
                    state.color.insert(d, col);
                } else {
                    state.hidden.insert(d);
                }
            }
            state.text.insert(c, assets.strings.get(map_label(kind)).unwrap_or_default().to_string());
            // 0x496C50 / 0x496BC0 send the row's text (row +0x14, Data_NN)
            // the *message* Button_Highlight / Button_Disable (0x51CDD0);
            // its MsgR runs Highlight (0x249DB7B7) / Disabled (0x06B55037)
            // on itself and forwards to the row's bar group and DATA_A
            // (**verified**, 2026-10-29; the original under Wine shows the
            // focused row's green bar). The packages have no scripts of
            // the messages' names.
            if self.focus == Some(r) {
                state.focus.push((0.0, c));
            } else if hidden {
                state.scripts.insert(c, (fe_ids::DISABLED, 0.0));
                state.scripts.insert(a, (fe_ids::DISABLED, 0.0));
            }
        }
        let mut quads = layout(assets, package, &state, now - self.opened);
        // The help bar (0x4B0FE0 → 0x54E6F0): the screen record's mask
        // (0x7F827C +0xC = 0x18400: Back) with 0x10000000 "Activate GPS"
        // while the GPS is off, else 0x20000000 "Deactivate GPS".
        if bar {
            let mask = WORLD_MAP_HELP | if self.gps { 0x2000_0000 } else { 0x1000_0000 };
            quads.extend(help_bar(assets, &help_labels(mask), mask, None, now, self.opened));
        }
        quads
    }

    /// The marker selected on the map (the widget's +0x14): what C
    /// (BUTTON1) would aim the GPS at.
    pub fn target(&self) -> Option<(u32, crate::frontend::career::Trigger)> {
        self.selected.and_then(|i| self.icons.get(i).and_then(|i| i.target))
    }
}

/// UI_InGame_WorldMap's help-bar mask (screen record 0x7F827C +0xC).
pub const WORLD_MAP_HELP: u32 = 0x1_8400;

#[cfg(test)]
mod tests {
    use super::*;

    /// The selection's two rings (0x4E0440 / 0x4967F0 show the group on
    /// the selected marker; the rings' INIT → PULSE runs on the FEng clock
    /// from the screen's opening): one 1600-ms package loop (1333 screen
    /// ms at `FE_RATE`), size 28.75 → 70, alpha 0 → 255 → 0, the second
    /// ring 400 package ms behind. The same moment gives the same rings at
    /// 30, 60, 120 and 200 fps.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn selection_rings_pulse() {
        let assets = crate::frontend::Assets::load(&crate::testkit::game_root()).expect("loading the game's front end assets");
        let (cx, cy, span, _) = WORLD_MAP_ZOOMS[0];
        let (uc, vc) = (cx / 512.0 + span / 1024.0, cy / 512.0 + span / 1024.0);
        let trigger = crate::frontend::career::Trigger {
            name: 0,
            position: glam::Vec3::ZERO,
            gps: glam::Vec3::ZERO,
            min: Vec2::ZERO,
            max: Vec2::ZERO,
            polygon: [Vec2::ZERO; 8],
            points: 0,
        };
        let icon = MapIcon {
            group: map_group(9),
            kind: 9,
            at: Vec2::new(uc * 1000.0, -vc * 1000.0),
            heading: None,
            sponsor: false,
            target: Some((0x1234, trigger)),
        };
        let selector = assets.texture_for("Minimap_GPS_Selector.tga").expect("Minimap_GPS_Selector");
        // With the GPS on that marker (Deactivate GPS) and off.
        for gps in [Some(0x1234), None] {
            let map = InGameWorldMap::new(0.0, 1, None, [0.0, 0.0, 1000.0], vec![icon], [false; 14], gps);
            assert_eq!(map.selected, Some(0));
            let rings = |t: f32| -> Vec<(f32, f32)> {
                map.quads_with_bar(&assets, t, false)
                    .iter()
                    .filter(|q| q.texture == Some(selector))
                    .map(|q| ((q.rect[1].x - q.rect[0].x).round(), (q.color[3] * 255.0).round()))
                    .collect()
            };
            let ms = |package: f32| package / crate::frontend::FE_RATE;
            // Package ms 0: both rings clear (not drawn); 200: ring 1
            // 37.8 wide and opaque, ring 2 (400 ms behind) at 28.75 and
            // half clear; 600: ring 1 56 wide at alpha 180.
            let near = |r: &[(f32, f32)], w: f32, a: f32| r.iter().any(|q| (q.0 - w).abs() <= 1.0 && (q.1 - a).abs() <= 2.0);
            let b = rings(ms(200.0));
            let c = rings(ms(600.0));
            assert!(rings(ms(0.0)).is_empty(), "{gps:?}: clear at the start: {:?}", rings(ms(0.0)));
            assert!(b.len() == 2 && near(&b, 38.0, 255.0) && near(&b, 29.0, 128.0), "{gps:?}: 200 ms: {b:?}");
            assert!(c.len() == 2 && near(&c, 56.0, 180.0), "{gps:?}: 600 ms: {c:?}");
            // One loop later the same.
            assert_eq!(rings(ms(200.0 + 1600.0)), b, "{gps:?}: loops every 1600 package ms");
            // The moment, not the frame rate, picks the rings: frames
            // stepped at 30, 60, 120 and 200 fps show the same rings at
            // every 100 ms.
            let at_rate = |fps: u32| -> Vec<Vec<(f32, f32)>> {
                (1..=fps * 2)
                    .filter(|frame| frame % (fps / 10) == 0)
                    .map(|frame| rings(frame as f32 * 1000.0 / fps as f32))
                    .collect()
            };
            let reference = at_rate(60);
            assert_eq!(reference.len(), 20);
            for fps in [30, 120, 200] {
                assert_eq!(at_rate(fps), reference, "{gps:?}: {fps} fps");
            }
        }
    }
}
