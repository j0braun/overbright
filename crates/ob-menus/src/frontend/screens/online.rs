use crate::frontend::fe_ids;
use super::*;

/// UI_OLISPConnect, the first screen of the online flow (UI_Main's Online
/// sends 0xD05FC3A3 to UI_Main, whose handler 0x4F98B0 sets game mode 8
/// and starts the flow 0x4EFFD0 → 0x4E3960 on UI_OLISPConnect.fng; class
/// 0x49DE10). Its constructor puts "Initializing network ..."
/// (0xD5344356) on Header_text; the footer's "$JOY_EVENT_FENG_SELECT$
/// Cancel" (Accept or Back leave, 0x4B97D0 → 0x4A0140 → UI_Main).
pub fn isp_connect_quads(assets: &Assets, opened: f32, now: f32) -> Vec<Quad> {
    let Some(package) = assets.package("UI_OLISPConnect") else { return Vec::new() };
    let mut state = ScreenState::default();
    footer(&mut state, package, "Cancel");
    if let Some(text) = assets.strings.get(0xD534_4356) {
        state.text.insert(fe_hash("Header_text"), text.to_string());
    }
    // The Cancel button's label ("$JOY_EVENT_FENG_SELECT$ Cancel").
    state.text.insert(fe_ids::ONLINE_CANCEL, format!("{}: Cancel", crate::pad::extras::accept_name()));
    layout(assets, package, &state, now - opened)
}

/// UI_PC_LAN_ServerSelect (UI_Main's LAN, 0x4AEDE0: game mode 0x200;
/// class 0x4FB4C0). The server list (0x542CB0) starts empty: its rows'
/// "999999" counts and names stay blank. The constructor's 0x4F2EE0 binds
/// fifteen slots to `scroll_text_%d` / `scroll_racers_%d` / `High_Light_%d`
/// (Highlight / UnHighlight) and 0x4E8A00 fills an empty list with fifteen
/// blank rows and updates the scroll bar (0x538510(1): no handle or arrows
/// for an empty list); the rows' backings (`High_Light_2`..`_15`) go on
/// from their zero-length INIT to HIDE (alpha 0), so only the track and the first
/// row's band show (the original under Wine, frame lan_6: the band at
// prov: inferred frontend.screens.lan_servers_quads
/// `High_Light_1`'s place, no arrows, no handle). **Inferred**: which code
/// leaves `High_Light_1` at its INIT alpha (0x91) — the slot list's first
/// selection is not traced.
pub fn lan_servers_quads(assets: &Assets, opened: f32, now: f32) -> Vec<Quad> {
    let Some(package) = assets.package("UI_PC_LAN_ServerSelect") else { return Vec::new() };
    let mut state = ScreenState::default();
    footer(&mut state, package, "Select");
    for o in package.objects.iter().filter(|o| matches!(o.text.as_str(), "999999\n" | "WWWWWWWWWWWWWWWW")) {
        state.hidden.insert(o.hash);
    }
    for i in 2..=15 {
        state.chained.insert(fe_hash(&format!("High_Light_{i}")));
    }
    // 0x4E8A00 ends with 0x538510(1): the scroll bar of an empty list
    // hides its handle and arrows (`scroll_bar_at`).
    scroll_bar_at(&mut state, package, "ScrollBar", false, 15, 0, 0);
    layout(assets, package, &state, now - opened)
}

/// UI_OLEALogin (screen record 0x7F907C: factory 0x4E3C50, help hash
/// 0xE720F694, mask 0x405; class 0x4D0690, vtable 0x795ED8, handler
/// 0x4F0060), the online flow's first screen once UI_OLISPConnect has the
/// network: "EA Login" (0x3C149013) over two options, "Create New EA
/// Account" (0xED21B47F) and "Use Existing EA Account" (0x036E79B9, option
/// class 0x79475C whose Accept sets the flow's +4, 0x4B9C50), the first
/// selected. Accept (0x0C407210 at 0x4F00C2) puts "Connecting to the EA
/// Server..." (0x144849C1) up through the connect helper 0x4CFCB0 (dialog
/// 0x4B8EE0, its Cancel sending 0xCA4C311B); Back (0x911AB364 at 0x4F0130)
/// asks "Are you sure you wish to disconnect and stop playing Online?"
/// (0xAFFF7C7A; 0x558830: Yes 0xF770 → 0x2C0E485A to UI_Main, No 0x061C
/// nothing, No focused, Back ignored). The connect helper's handler
/// 0x4EFE40: 0xCA4C311B → 0x4B8F40 "If you cancel connecting to the lobby
/// server, …" (0xD857CE14; 0x558760: Yes → 0x2C0E485A, No → 0xCA4C3140,
/// No focused), 0xCA4C3140 → "Connecting to lobby..." (0x0023E142) with
/// Cancel. A failed connection ends in 0x49F780: "You have lost your
/// connection to the EA Server.  You will be returned to the Main Menu."
/// (0x3F920248) with OK (0x0639 → 0x2C0E485A: 0x4A0140, UI_Main). Seen so
/// under Wine with a network (frames on1–on9, on_1–on_8, cn_1–cn_10:
// prov: inferred frontend.screens.distance_takes_three_decimals
/// Create New and Use Existing both connect first). **Inferred**: the list's
/// row layout (rows 37 units apart from the highlight band 0xC128B184's
/// place, as the original drew them) and how long a failing connection
/// takes (`EaLogin::CONNECT_MS`; network-dependent, about 9 s of game time
/// under Wine).
#[derive(Clone, Debug)]
pub struct EaLogin {
    pub selected: usize,
    pub opened: f32,
}

mod ea_login {
    pub const TITLE: u32 = crate::frontend::fe_ids::EA_LOGIN_TITLE;
    pub const OPTIONS: [u32; 2] = [crate::frontend::fe_ids::ONLINE_OPTIONS_0, crate::frontend::fe_ids::ONLINE_OPTIONS_1];
    /// option_name_1 / _2 (templates below the screen until placed).
    pub const NAME: [u32; 2] = [crate::frontend::fe_ids::NAME, crate::frontend::fe_ids::NAMES_11];
    /// The highlight band (at the first row in the package).
    pub const BAND: u32 = crate::frontend::fe_ids::DATA_MASTER;
    pub const ESRB: u32 = crate::frontend::fe_ids::ESRB;
    pub const ROW_GAP: f32 = 36.0;
    /// The first row's text and the band (**inferred**: measured on the
    /// original's frames, on_2).
    pub const ROW: glam::Vec2 = glam::Vec2::new(-7.0, -156.0);
    pub const BACKING_AT: glam::Vec2 = glam::Vec2::new(6.5, -146.0);
    /// option_name_1's backing group (option_name_n's is + n − 1).
    pub const BACKING: u32 = crate::frontend::fe_ids::MC_LIST_BACKING;
}

impl EaLogin {
    /// How long a connection attempt runs before it fails (ms, the screens'
    /// clock; **inferred**).
    pub const CONNECT_MS: f32 = 9000.0;

    pub fn new(now: f32) -> EaLogin {
        EaLogin { selected: 0, opened: now }
    }

    /// Up / Down move between the two options.
    pub fn key(&mut self, key: Key) {
        match key {
            Key::Up => self.selected = 0,
            Key::Down => self.selected = 1,
            _ => {}
        }
    }

    pub fn quads(&self, assets: &Assets, now: f32) -> Vec<Quad> {
        use ea_login::*;
        let Some(package) = assets.package("UI_OLEALogin") else { return Vec::new() };
        let mut state = ScreenState::default();
        footer(&mut state, package, "Select");
        package_init(&mut state, package);
        state.hidden.insert(ESRB);
        // The unused option rows and their backings.
        for o in package.objects.iter().filter(|o| o.text.as_str() == "option_name" && !NAME.contains(&o.hash)) {
            state.hidden.insert(o.hash);
        }
        for h in fe_ids::MC_LIST_BACKING..=0x66E2_9939u32 {
            state.hidden.insert(h);
        }
        state.hidden.insert(fe_ids::TITLE_MASTER);
        if let Some(t) = assets.strings.get(TITLE) {
            state.text.insert(fe_ids::ONLINE_TITLE, t.to_string());
        }
        for (i, (&name, &label)) in NAME.iter().zip(&OPTIONS).enumerate() {
            state.text.insert(name, assets.strings.get(label).unwrap_or_default().to_string());
            state.position.insert(name, glam::Vec2::new(ROW.x, ROW.y + ROW_GAP * i as f32));
            state.shown.insert(name);
            state.run(name, if i == self.selected { "HIGHLIGHT" } else { "UNHIGHLIGHT" }, 0.0);
        }
        // Each row's backing (0x66E29936 + row: HIGHLIGHT alpha 0xC8,
        // UNHIGHLIGHT 0), moved to its row.
        for i in 0..2 {
            let backing = BACKING + i as u32;
            state.hidden.remove(&backing);
            state.position.insert(backing, glam::Vec2::new(BACKING_AT.x, BACKING_AT.y + ROW_GAP * i as f32));
            state.run(backing, if i == self.selected { "HIGHLIGHT" } else { "UNHIGHLIGHT" }, 0.0);
        }
        state.hidden.insert(BAND);
        // The list's scroll bar: the handle at the selected option.
        scroll_bar_at(&mut state, package, "ScrollBar", false, 1, OPTIONS.len(), self.selected + 1);
        layout(assets, package, &state, now - self.opened)
    }
}
