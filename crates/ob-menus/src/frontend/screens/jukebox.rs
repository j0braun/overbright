use crate::frontend::fe_ids;
use super::*;

/// The EA Trax table (0x7ECD80, 16 bytes a track: title, artist, album and
/// where it plays, "FE" or "IG"); the 27 well-formed entries before the
/// French variant strings.
pub static EA_TRAX: crate::exe_tables::Table<[(&str, &str, &str); 27]> = crate::exe_tables::Table::new(|t| &t.ea_trax);

/// The chyron's caption (`audio::music::Chyron`) as quads: the front end lays out the caption screen it names.
/// Its quads at `now` (ms, the screens' clock).
pub fn chyron_quads(
    assets: &crate::frontend::assets::Assets,
    c: &crate::audio::music::Chyron,
    now: f32,
    front_end: bool,
) -> Vec<crate::draw::quad::Quad> {
    use crate::audio::music::chyron;
    let Some(&(title, artist, album)) = EA_TRAX.get(c.song) else {
        return Vec::new();
    };
    let ms = now - c.at;
    let longest = title.len().max(artist.len()).max(album.len());
    let (name, script, at) = chyron::phase(longest, c.go.map(|g| g - c.at), front_end);
    let Some(package) = assets.package(name) else {
        return Vec::new();
    };
    let mut state = crate::frontend::feng::state::ScreenState::default();
    state.text.insert(chyron::TITLE, title.to_string());
    state.text.insert(chyron::ARTIST, artist.to_string());
    state.text.insert(chyron::ALBUM, album.to_string());
    state.run(chyron::ROOT, &script, at);
    crate::frontend::layout(assets, package, &state, ms)
}

/// EA_Trax_Jukebox (Options' EA GAMES TRAX, category 5; screen 0x553C80):
/// four rows of number, title, artist, album and the track's state (Off,
/// Menu, Racing, All). The rows follow the profile's playlist (0x83ACB8:
/// index and state per entry; 0x479220 turns states 1 / 3 into the menu
/// mask and 2 / 3 into the race mask); the clone lists the table in order,
// prov: inferred frontend.screens.Jukebox
/// every track All (**inferred**: the profile's default playlist is not
/// traced). Its "RANDOM" play mode stays as the package has it.
pub struct Jukebox {
    pub top: usize,
    pub opened: f32,
    /// The selected track (the list's current item, +0x70).
    pub selected: usize,
    /// When the selection last moved, ms.
    pub moved: f32,
    /// The arrow that last blinked (right?) and when.
    pub blink: Option<(bool, f32)>,
}

pub const ROWS: usize = 4;
pub const NUMBER: u32 = fe_ids::NUMBER;
pub const TITLE: u32 = fe_ids::JUKEBOX_TITLE;
pub const ARTIST: u32 = fe_ids::ARTIST;
pub const ALBUM: u32 = fe_ids::ALBUM;
pub const STATE: u32 = fe_ids::STATE;
/// Each row's left / right arrows (+0x34 / +0x38 of the row).
pub const LEFT: u32 = fe_ids::JUKEBOX_LEFT;
pub const RIGHT: u32 = fe_ids::JUKEBOX_RIGHT;
/// Each row's backing groups.
pub const GROUPS: [u32; 2] = [fe_ids::JUKEBOX_GROUPS_0, fe_ids::JUKEBOX_GROUPS_1];
/// The states' names (0x53DEC0's switch): Off, Menu, Racing, All.
pub const STATES: [(u32, &str); 4] =
    [(fe_ids::STATES_0, "Off"), (fe_ids::STATES_1, "Menu"), (fe_ids::STATES_2, "Racing"), (fe_ids::STATES_3, "All")];

impl Jukebox {
    pub fn new(now: f32) -> Jukebox {
        Jukebox { top: 0, opened: now, selected: 0, moved: now, blink: None }
    }

    /// Up / Down move through the tracks (the list scrolls to keep the
    /// selection in its four rows); Left / Right step the selected track's
    /// state (0x53DEC0: 0x9120409E down, wrapping 0 → 3; 0xB5971BF1 up,
    /// 3 → 0; the arrow blinks). Returns whether the playlist changed.
    pub fn key(&mut self, key: Key, trax: &mut [u8; 27], now: f32) -> bool {
        let n = EA_TRAX.len();
        match key {
            Key::Up if self.selected > 0 => self.selected -= 1,
            Key::Down if self.selected + 1 < n => self.selected += 1,
            Key::Left | Key::Right => {
                let right = key == Key::Right;
                let s = &mut trax[self.selected];
                *s = if right { (*s + 1) % 4 } else { (*s + 3) % 4 };
                self.blink = Some((right, now));
                return true;
            }
            _ => return false,
        }
        self.moved = now;
        if self.selected < self.top {
            self.top = self.selected;
        } else if self.selected >= self.top + jukebox::ROWS {
            self.top = self.selected + 1 - jukebox::ROWS;
        }
        false
    }

    pub fn quads(&self, assets: &Assets, trax: &[u8; 27], random: bool, now: f32) -> Vec<Quad> {
        use jukebox::*;
        let Some(package) = assets.package("EA_TRAX_Jukebox") else { return Vec::new() };
        let mut state = ScreenState::default();
        footer(&mut state, package, "Select");
        for o in package
            .objects
            .iter()
            .filter(|o| matches!(o.text.as_str(), "PLAYER NAME" | "$999999999" | "Preview " | "Grab"))
        {
            state.hidden.insert(o.hash);
        }
        for row in 0..ROWS {
            let i = self.top + row;
            let r = row as u32;
            let Some((title, artist, album)) = EA_TRAX.get(i) else { continue };
            state.text.insert(NUMBER + r, format!("{:02}", i + 1));
            state.text.insert(TITLE + r, title.to_string());
            state.text.insert(ARTIST + r, artist.to_string());
            state.text.insert(ALBUM + r, album.to_string());
            let (h, fallback) = STATES[trax[i].min(3) as usize];
            state.text.insert(STATE + r, assets.strings.get(h).unwrap_or(fallback).to_string());
            state.shown.insert(STATE + r);
            // The rows (0x54B320, vtable 0x795CB0: +0x2C "Highlight", +0x30
            // "UnHighlight") run them on the number, artist, title, album
            // and state strings, the row backing (0xDE635F7C + row, item
            // +0x14) and the row's Left_arrow_ / Right_arrow_ (+0x34 /
            // +0x38): the selected one Highlight, the others UnHighlight.
            // The constructor 0x553C80 runs "static" on the row groups
            // 0x8065767B + row and "Static" on the arrows; the groups get
            // nothing else (their scripts would send the rows' grab arrows
            // GRAB, which only grabbing a track does).
            // Under Wine the rows not selected keep their arrows dim
            // (STATIC, alpha 100), so the clone runs the highlight scripts
            // on the selected row's arrows only (which of the row's objects
            // its UnHighlight reaches is **not traced**).
            let script = if i == self.selected { "HIGHLIGHT" } else { "UNHIGHLIGHT" };
            state.run(GROUPS[1] + r, "STATIC", 0.0);
            for o in [LEFT, RIGHT].map(|o| o + r) {
                state.run(o, if i == self.selected { "HIGHLIGHT" } else { "STATIC" }, 0.0);
            }
            // The rows' grab arrows (Grab_Arrow_1…4, 0x07CD00B5 + row):
            // INIT → HIDE; only the row groups' GRAB (grabbing a track to
            // move it) shows them.
            state.hidden.insert(fe_ids::GRAB_ARROW_1 + r);
            for o in [NUMBER, TITLE, ARTIST, ALBUM, STATE, GROUPS[0]].map(|o| o + r) {
                if package
                    .object(o)
                    .is_some_and(|x| x.scripts.iter().any(|sc| sc.name == crate::formats::hash::fe_hash(script)))
                {
                    state.run(o, script, self.moved - self.opened);
                }
            }
            if i == self.selected
                && let Some((right, at)) = self.blink
            {
                state.run(if right { RIGHT + r } else { LEFT + r }, "BLINK", at - self.opened);
            }
        }
        // The play mode (0xCA74A2FA, 0x553C80): "Sequential" (0x4CA36B89)
        // for 0x83AA58 = 0, "Random" (0xA9C9C8F7) for 1 (settings +0x54,
        // saved in the profile; a fresh profile has 1, 0x5116E0).
        let (mode, fallback) = if random { (fe_ids::TXT_RANDOM, "Random") } else { (fe_ids::TXT_SEQUENTIAL, "Sequential") };
        state.text.insert(0xCA74_A2FA, assets.strings.get(mode).unwrap_or(fallback).to_string());
        // The scroll bar ("Scroll_Area" / "ScrollBar", the 0x542CB0 list at
        // +0x4C): 0x54B320 → 0x538090(4 rows +0x64, 27 tracks +0x68, the
        // window top's index from 1 +0x6C) — the (1, count, current) branch
        // needs list +0xA0 (+0xEC), which 0x542CB0 only clears — then
        // Arrow_1 (+0xF8) hides on the first track and Arrow_2 (+0xFC) on
        // the last (the current item +0x70).
        scroll_bar_at(&mut state, package, "ScrollBar", false, ROWS, EA_TRAX.len(), self.top + 1);
        if self.selected == 0 {
            state.hidden.insert(fe_ids::POST_RACE_STATS_LEFT);
        }
        if self.selected + 1 == EA_TRAX.len() {
            state.hidden.insert(fe_ids::POST_RACE_STATS_RIGHT);
        }
        layout(assets, package, &state, now - self.opened)
    }
}
