use crate::frontend::fe_ids;
use super::*;

/// UI_EngageRaceDialog: the event's invitation when the player stops at
/// its marker (0x4B5390): subject and race type by the event's category
/// and type (jump tables 0x4B56D8 / 0x4B56F0), the distance (the track's
/// +0x98 metres × 0.000625, "%$f %s" miles), the purse (0x500C30: the
/// signed sponsor's for a sponsor race, else the event's +0x30) and the message (0x512620: "ENGAGE_<event>" for stage 0,
/// else a random "ENGAGE_<TYPE>_RANDOM_n").
pub struct Engage {
    pub event: usize,
    /// Which random message (1-based), chosen when the dialog opens.
    pub message: u32,
    pub opened: f32,
    /// The player's last focus move: (Accept, when ms); see `DialogFocus`.
    pub focus: DialogFocus,
    /// The purse shown (0x500C30, `Profile::race_bank`): a sponsor race
    /// with a sponsor signed in the current stage shows that sponsor's
    /// +0x02, else the event's +0x30 (under Wine: S2_SPON_DRAG_3 with
    /// EDELBROCK signed, "875").
    pub purse: u32,
}

/// The engage dialogs' two buttons (UI_EngageRaceDialog's handler
/// 0x499C00, UI_EngageShopDialog's 0x499690): when the package's CALL
/// sends 0x35F8620B to the screen (180 ms) the handler focuses
/// Cancel_Button (0x63857DE0, 0x52CEB0 → 0x52CE50 → 0x52B3B0); left
/// (0x9120409E) focuses Accept_Button (0xD72F002A), right (0xB5971BF1)
/// Cancel_Button; accept (0x0C407210) acts on the focused one: Accept
/// starts the race / enters the shop (0x499B50 / 0x499640), Cancel declines
/// (0x499BB0 / back to the world), as back (0x911AB364) does. Before the
/// first focus accept does nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct DialogFocus(pub Option<(bool, f32)>);

const DIALOG_READY: u32 = fe_ids::DIALOG_READY;

const DIALOG_ACCEPT: u32 = fe_ids::ENGAGE_ACCEPT;

const DIALOG_CANCEL: u32 = fe_ids::ENGAGE_CANCEL;

impl DialogFocus {
    /// The focus changes from the dialog's opening, (ms, Accept?), in order.
    fn changes(&self, package: &crate::frontend::feng::Package, opened: f32) -> Vec<(f32, bool)> {
        let mut v: Vec<(f32, bool)> =
            crate::frontend::host_time(package, DIALOG_READY).map(|t| (t, false)).into_iter().collect();
        if let Some((accept, at)) = self.0 {
            v.push(((at - opened).max(0.0), accept));
        }
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        v
    }

    /// Whether Accept (true) or Cancel (false) has the focus at `now`.
    pub fn accept_focused(&self, assets: &Assets, package: &str, opened: f32, now: f32) -> Option<bool> {
        let package = assets.package(package)?;
        self.changes(package, opened).into_iter().rev().find(|c| c.0 <= now - opened).map(|c| c.1)
    }

    fn apply(&self, package: &crate::frontend::feng::Package, opened: f32, state: &mut ScreenState) {
        state.focus = self
            .changes(package, opened)
            .into_iter()
            .map(|(t, a)| (t, if a { DIALOG_ACCEPT } else { DIALOG_CANCEL }))
            .collect();
    }
}

pub const SUBJECT: u32 = fe_ids::SUBJECT;
pub const TYPE: u32 = fe_ids::ENGAGE_TYPE;
pub const DISTANCE: u32 = fe_ids::ENGAGE_DISTANCE;
pub const PURSE: u32 = fe_ids::PURSE;
pub const MESSAGE: u32 = fe_ids::ENGAGE_MESSAGE;
pub const ACCEPT: u32 = fe_ids::ENGAGE_ACCEPT;
pub const DECLINE: u32 = fe_ids::ENGAGE_CANCEL;
pub const MAP: u32 = fe_ids::ENGAGE_MAP;

/// 0x512620's set and its size n (**verified**, A-frontend-2 4o-009 /
/// A-game-2 2-014; jump tables 0x5127E4 / 0x5127FC): None (no draw) for
/// an event of stage 0 (+0x37), else in this order the SUV set (+0x0D = 1,
/// 9), the sponsor sets (+0x34 = 1, by kind +0x0F), URL (+0x34 = 2, 12),
/// the plain sets by kind. The number is `0x43C1C0(n) + 1` on the game's
/// generator, drawn once when the dialog is built (0x4B551B). Kind 3 is
/// dead in the binary (a null format / a division by zero); the clone
/// keeps its CIRCUIT fallback for it.
pub fn message_set(e: &Event) -> Option<(&'static str, u32)> {
    use crate::frontend::career::{Category, EventKind};
    if e.stage == 0 {
        return None;
    }
    if e.suv {
        return Some(("SUV", 9));
    }
    Some(match (e.category, e.kind) {
        (Category::Sponsor, EventKind::Circuit) => ("SPON_CIRCUIT", 11),
        (Category::Sponsor, EventKind::Sprint) => ("SPON_SPRINT", 10),
        (Category::Sponsor, EventKind::StreetX) => ("SPON_STREET", 10),
        (Category::Sponsor, EventKind::Drag) => ("SPON_DRAG", 8),
        (Category::Sponsor, EventKind::Drift) => ("SPON_DRIFT", 11),
        (Category::Url, _) => ("URL", 12),
        (_, EventKind::Circuit) => ("CIRCUIT", 18),
        (_, EventKind::Sprint) => ("SPRINT", 15),
        (_, EventKind::StreetX) => ("STREET", 14),
        (_, EventKind::Drag) => ("DRAG", 12),
        (_, EventKind::Drift) => ("DRIFT", 13),
        _ => ("CIRCUIT", 18),
    })
}

impl Engage {
    /// The message label (0x512620): "ENGAGE_<name>" for an event of stage
    /// 0 (+0x37), else the set's label with the number drawn at the
    /// dialog's set-up (`message_set`).
    fn message_label(&self, e: &Event) -> String {
        match message_set(e) {
            None => format!("ENGAGE_{}", e.name),
            Some((set, _)) => format!("ENGAGE_{set}_RANDOM_{}", self.message),
        }
    }

    /// Subject and race type labels (0x4B5555..0x4B5638).
    fn type_labels(e: &Event) -> (u32, u32) {
        use crate::frontend::career::{Category, EventKind};
        let sponsor = e.category == Category::Sponsor;
        match (e.category, e.kind) {
            (Category::Url, _) => (0x80B8_267C, fe_ids::OBJ_B46377BC), // "URL", "U.R.L."
            (_, EventKind::Circuit) => (if sponsor { 0x0F03_7A7B } else { fe_ids::OBJ_9B39C122 }, fe_ids::OBJ_268D063C),
            (_, EventKind::Sprint) => (if sponsor { 0x0721_6A68 } else { 0x9EC5_F82F }, fe_ids::OBJ_BA613729),
            (_, EventKind::StreetX) => (if sponsor { 0xC5F7_902C } else { 0x57F8_33AD }, fe_ids::OBJ_0FDA6758),
            (_, EventKind::Drag) => (if sponsor { 0x3D6A_70E6 } else { 0xE45A_7387 }, fe_ids::OBJ_40C91BA7),
            (_, EventKind::Drift) => (if sponsor { 0xB520_5101 } else { 0x3A10_A7C2 }, fe_ids::OBJ_59ECB2C2),
            _ => (fe_ids::OBJ_9B39C122, fe_ids::OBJ_268D063C),
        }
    }

    pub fn quads(&self, assets: &Assets, data: &CareerData, metric: bool, now: f32) -> Vec<Quad> {
        let Some(package) = assets.package("UI_EngageRaceDialog") else { return Vec::new() };
        let e = &data.events[self.event];
        let mut state = ScreenState::default();
        footer(&mut state, package, "Accept");
        let (subject, kind) = Engage::type_labels(e);
        let label = |h: u32| assets.strings.get(h).unwrap_or("").to_string();
        let subject_fmt = assets.strings.get(0x17EA_64F8).unwrap_or("Subject: %s");
        state.text.insert(engage::SUBJECT, subject_fmt.replace("%s", &label(subject)));
        state.text.insert(engage::TYPE, label(kind));
        // 0x4B5410–0x4B5467: "%$f %s" of the track's +0x98 × 0.000625
        // (miles whatever the units) and "mi" (0xAC3A4F4C) or, with metric
        // units (0x83AA1A), "km" (0xB5D640B1); the formatter's %f without a
        // precision takes 3 (0x43EDFD; under Wine "1.376 mi"). The purse
        // "%$d" (0x78F744) of 0x500C30, no currency sign (Wine: "250").
        // The product is a single-precision one (the FPU runs in 24-bit
        // prov: inferred frontend.screens.quads-8
        // precision under Direct3D 9, **inferred** as the cause): track
        // 4004's 5300 m gives exactly 3.3125, which the formatter rounds
        // up to "3.313" (under Wine, S2_SPON_CIRCUIT_2); in extended
        // precision it would be 3.3124999 → "3.312".
        let miles = data.tracks.get(&e.track).map_or(0.0, |t| f64::from(t.length as f32 * f32::from_bits(0x3A23_D70A)));
        let unit = assets.strings.get(if metric { fe_ids::TXT_KM } else { fe_ids::TXT_MI }).unwrap_or("");
        state.text.insert(engage::DISTANCE, format!("{} {unit}", fixed3(miles)));
        state.text.insert(engage::PURSE, crate::frontend::hud::grouped_int(self.purse as i32));
        let message = assets.strings.label(&self.message_label(e));
        state.text.insert(engage::MESSAGE, message);
        // The sender (0x4B569C): "From: %s" (0x9F29D70D) of 0x512820 —
        // the event's +0x84 label when set, else its first opponent's name
        // (+0x40 in the career strings). Under Wine: "From: Ali"
        // (S2_URL_3), "From: Patrick" (S2_H_CIRCUIT_6).
        let sender = match e.sender {
            0 => e.opponents.first().map_or(String::new(), |o| o.name.clone()),
            h => assets.strings.get(h).or_else(|| assets.strings.get(fe_ids::OBJ_C01A6F63)).unwrap_or("").to_string(),
        };
        state.text.insert(
            fe_ids::FROM,
            assets.strings.get(crate::frontend::sms::FROM_FORMAT).unwrap_or("From: %s").replacen("%s", &sender, 1),
        );
        state.shown.insert(engage::ACCEPT);
        state.shown.insert(engage::DECLINE);
        // The race's own map: the constructor hides the map object
        // 0x0F365871 (0x4B5648 → 0x50CA00) and loads TRACKS\TRACKMAP<track>.bin
        // ("TrackMapMemory", 0x5E8680 / 0x57CF00; its callback 0x4B53A0
        // sets +0x50); each tick (0xC98356BA, 0x499D13) with it loaded the
        // object takes the texture TRACKMAP<track> (+0x58, 0x537A70 →
        // 0x50C990) and shows (0x50CA50), else stays hidden. The package's
        // own texture on it is never drawn.
        let map = crate::formats::hash::bin_hash(&format!("TRACKMAP{}", e.track));
        if assets.textures.contains_key(&map) {
            state.image.insert(engage::MAP, map);
            state.shown.insert(engage::MAP);
        } else {
            state.hidden.insert(engage::MAP);
        }
        self.focus.apply(package, self.opened, &mut state);
        layout(assets, package, &state, now - self.opened)
    }
}

/// UI_EngageShopDialog (screen 0x4CE6B0, set up by 0x4998E0): the shop's
/// kind as the title (0x4FFEA0), its `<NAME>_BODY` message (0x512920;
/// the garage asks 0x987EECDA), and "Enter Shop" / "Enter Garage" on the
/// accept button.
pub struct ShopEngage {
    pub shop: usize,
    pub opened: f32,
    pub focus: DialogFocus,
}

mod shop_engage {
    pub const TITLE: u32 = crate::frontend::fe_ids::SHOP_ENGAGE_TITLE;
    pub const MESSAGE: u32 = crate::frontend::fe_ids::SHOP_ENGAGE_MESSAGE;
    pub const ACCEPT: u32 = crate::frontend::fe_ids::ENGAGE_ACCEPT;
    pub const CANCEL: u32 = crate::frontend::fe_ids::ENGAGE_CANCEL;
}

impl ShopEngage {
    /// The dialog's message (0x512920): the garage (kind 0) 0x987EECDA; a
    /// shop whose visit entry (table 0x862840, 0x86283C entries) is in
    /// state 5 — entered before, +6 = 5 from the first Accept (0x54CB00) —
    /// the generic 0x15D6B438 ("Would you like to enter the Shop?");
    /// otherwise its `<NAME>_BODY`, the first-visit text, or 0x15D6B438 when
    /// that label is missing. Under Wine with the finished-career save
    /// (every shop's entry in state 5) the body, specialties and graphics
    /// shops all asked "Would you like to enter the Shop?".
    pub fn message(assets: &Assets, shop: &crate::frontend::career::Shop, entered: bool) -> String {
        let label = |h: u32| assets.strings.get(h).unwrap_or("").to_string();
        if shop.kind == 0 {
            label(0x987E_ECDA)
        } else if entered {
            label(fe_ids::OBJ_15D6B438)
        } else {
            assets
                .strings
                .get(bin_hash(&format!("{}_BODY", shop.name)))
                .map_or_else(|| label(fe_ids::OBJ_15D6B438), str::to_string)
        }
    }

    /// `entered`: the shop's visit-table entry is in state 5 (+6 = 5, set
    /// on the first Accept, 0x54CB00).
    pub fn quads(&self, assets: &Assets, data: &CareerData, entered: bool, now: f32) -> Vec<Quad> {
        let Some(package) = assets.package("UI_EngageShopDialog") else { return Vec::new() };
        let Some(shop) = data.shops.get(self.shop) else { return Vec::new() };
        let mut state = ScreenState::default();
        footer(&mut state, package, "Accept");
        let label = |h: u32| assets.strings.get(h).unwrap_or("").to_string();
        state.text.insert(shop_engage::TITLE, label(shop.title_label()));
        let message = Self::message(assets, shop, entered);
        state.text.insert(shop_engage::MESSAGE, message);
        state.text.insert(shop_engage::ACCEPT, label(if shop.kind == 0 { 0x67F5_5A89 } else { 0x054B_17DC }));
        state.shown.insert(shop_engage::ACCEPT);
        state.shown.insert(shop_engage::CANCEL);
        // The package's third button (group B1000CC8, "Yes" B8A7C6CE, both
        // INIT → HIDE with no FADEIN response) is for other dialogs: the
        // constructor (0x4B5300 → 0x4998E0) sets only the message, title
        // and Accept_Button's label, so it stays hidden.
        state.chain_init = true;
        self.focus.apply(package, self.opened, &mut state);
        layout(assets, package, &state, now - self.opened)
    }
}

/// A world challenge's dialogs. `Offer`: UI_EngageEventDialog at the
/// challenge's start marker (0x498D00 with marker +0x10; set up by
/// 0x4991B0): the message 0xDE075B62 is 0x515160's — by the kind (+0x14)
/// ENGAGE_VISUAL_GENERIC_%d_BODY (1 to 5), ENGAGE_PERF_GENERIC_%d_BODY (1
/// to 3), ENGAGE_SHOWCASE_GENERIC_%d_BODY (1 to 5; kind 4; kind 3 none),
/// the number drawn from the game's generator (0x43C1C0) + 1, filled with
/// the time limit (+0x10 s as ticks, ftol(t × 4000 + 0.5), printed by
/// 0x577250 with flags 0); the subject 0x2C167533 is "%s" of 0x515250's
/// label (kind 1 0x48157D57, 2 0xA517FD7A, 3 0xBF178F39, 4 0xE5B2C887,
/// "Subject: Magazine Cover Opportunity"). Missing labels read "FENG:
/// Default string error." (0xC01A6F63). The buttons are the package's.
/// `Retry`: UI_EngageShopDialog after the time ran out (0x498D00 with the
/// challenge's failed flag, 0x85B940; 0x4998E0): message 0xC4DF3B44 ("You
/// did not get to the target zone on time. ^Do you wish to retry?"),
/// Accept's label 0x039DF715 "Retry", an empty title.
pub struct ChallengeDialog {
    pub kind: ChallengeDialogKind,
    pub opened: f32,
    pub focus: DialogFocus,
    /// UI_EngageEventDialog's sender name (0x499160), drawn at opening.
    pub from: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChallengeDialogKind {
    /// The challenge (an index into `CareerData::mag_triggers`) and the
    /// drawn message number.
    Offer {
        challenge: usize,
        message: u32,
    },
    Retry,
    /// A photo shoot's spot (0x498D00 with marker +0x0C; 0x4991B0's
    /// milestone branch): the milestone's hash.
    Milestone {
        milestone: u32,
    },
}

/// The message number 0x515160 draws for a kind: the generator's range
/// (`None` for kind 3, which has no message).
pub fn challenge_message_range(kind: u8) -> Option<u32> {
    match kind {
        1 | 4 => Some(5),
        2 => Some(3),
        _ => None,
    }
}

mod event_engage {
    pub const MESSAGE: u32 = crate::frontend::fe_ids::ENGAGE_MESSAGE;
    pub const SUBJECT: u32 = crate::frontend::fe_ids::SUBJECT;
    pub const ACCEPT: u32 = crate::frontend::fe_ids::ENGAGE_ACCEPT;
    pub const CANCEL: u32 = crate::frontend::fe_ids::ENGAGE_CANCEL;
    /// The third button (label and group) a milestone's dialog shows
    /// alone; the two others' groups are hidden.
    pub const BUTTON: u32 = crate::frontend::fe_ids::BUTTON;
    pub const BUTTON_GROUP: u32 = crate::frontend::fe_ids::BUTTON_GROUP;
    pub const PAIR_GROUPS: [u32; 2] = [crate::frontend::fe_ids::PAIR_GROUPS_0, crate::frontend::fe_ids::PAIR_GROUPS_1];
    pub const FROM: u32 = crate::frontend::fe_ids::FROM;
}

impl ChallengeDialog {
    /// "From: %s" (0x9F29D70D) of the drawn name (0x499160).
    fn from_line(&self, assets: &Assets) -> String {
        assets.strings.get(crate::frontend::sms::FROM_FORMAT).unwrap_or("From: %s").replacen("%s", &self.from, 1)
    }

    pub fn package(&self) -> &'static str {
        match self.kind {
            ChallengeDialogKind::Offer { .. } | ChallengeDialogKind::Milestone { .. } => "UI_EngageEventDialog",
            ChallengeDialogKind::Retry => "UI_EngageShopDialog",
        }
    }

    /// The texts the constructor sets: (object, text).
    pub fn texts(&self, assets: &Assets, data: &CareerData) -> Vec<(u32, String)> {
        let label =
            |h: u32| assets.strings.get(h).or_else(|| assets.strings.get(fe_ids::OBJ_C01A6F63)).unwrap_or("").to_string();
        match self.kind {
            ChallengeDialogKind::Offer { challenge, message } => {
                let Some(t) = data.mag_triggers.get(challenge) else { return Vec::new() };
                let body = match t.kind {
                    1 => format!("ENGAGE_VISUAL_GENERIC_{message}_BODY"),
                    2 => format!("ENGAGE_PERF_GENERIC_{message}_BODY"),
                    4 => format!("ENGAGE_SHOWCASE_GENERIC_{message}_BODY"),
                    _ => String::new(),
                };
                let ticks = (t.time as f32 * 4000.0 + 0.5) as i32;
                let time = crate::frontend::hud::time_text(ticks);
                let message = label(bin_hash(&body)).replacen("%s", &time, 1);
                let subject = match t.kind {
                    1 => 0x4815_7D57,
                    2 => 0xA517_FD7A,
                    3 => 0xBF17_8F39,
                    4 => 0xE5B2_C887,
                    _ => 0,
                };
                let mut v = vec![(event_engage::MESSAGE, message), (event_engage::FROM, self.from_line(assets))];
                if subject != 0 {
                    v.push((event_engage::SUBJECT, label(subject)));
                }
                v
            }
            ChallengeDialogKind::Retry => vec![
                (shop_engage::TITLE, String::new()),
                (shop_engage::MESSAGE, label(0xC4DF_3B44)),
                (shop_engage::ACCEPT, label(fe_ids::RETRY)),
            ],
            // 0x514860 (message) and 0x514910 (subject) by the milestone's
            // kind (+0x24, 1–4), and "Continue" (0xDE02CF84) on the third
            // button; seen under Wine for a DVD: "Subject: Take Photo For
            // DVD Cover", "You can now take a photo of your car and get on
            // the cover of a DVD.", Continue.
            ChallengeDialogKind::Milestone { milestone } => {
                let kind = data.milestones.iter().find(|m| m.hash == milestone).map_or(0, |m| m.kind);
                let (body, subject) = match kind {
                    1 => (0x8FF3_F225, 0x1E4C_BC67),
                    2 => (0x1DEF_B577, 0x9982_9679),
                    3 => (0x74B2_A894, 0x1654_9076),
                    4 => (0xA747_EA86, 0xE0F2_E528),
                    _ => (fe_ids::OBJ_C01A6F63, fe_ids::OBJ_C01A6F63),
                };
                vec![
                    (event_engage::MESSAGE, label(body)),
                    (event_engage::SUBJECT, label(subject)),
                    (event_engage::BUTTON, label(fe_ids::CONTINUE)),
                    (event_engage::FROM, self.from_line(assets)),
                ]
            }
        }
    }

    pub fn quads(&self, assets: &Assets, data: &CareerData, now: f32) -> Vec<Quad> {
        let Some(package) = assets.package(self.package()) else { return Vec::new() };
        let mut state = ScreenState::default();
        footer(&mut state, package, "Accept");
        for (object, text) in self.texts(assets, data) {
            state.text.insert(object, text);
        }
        if let ChallengeDialogKind::Milestone { .. } = self.kind {
            // 0x4991B0: the pair's groups and labels hidden, the third
            // button's group and label shown (FadeIn).
            state.hidden.extend(event_engage::PAIR_GROUPS);
            state.hidden.insert(event_engage::ACCEPT);
            state.hidden.insert(event_engage::CANCEL);
            state.shown.insert(event_engage::BUTTON_GROUP);
            state.shown.insert(event_engage::BUTTON);
            // 0x537C10: "FadeIn" on the group and the label.
            state.run(event_engage::BUTTON_GROUP, "FadeIn", 0.0);
            state.run(event_engage::BUTTON, "FadeIn", 0.0);
        } else {
            state.shown.insert(event_engage::ACCEPT);
            state.shown.insert(event_engage::CANCEL);
        }
        state.chain_init = true;
        self.focus.apply(package, self.opened, &mut state);
        layout(assets, package, &state, now - self.opened)
    }
}

/// UI_SponsorPopup (screen 0x4C1280, 0x4A5250): "SELECT A SPONSOR!" with
/// Select and Back, as the package has it.
pub fn sponsor_popup_quads(assets: &Assets, opened: f32, now: f32) -> Vec<Quad> {
    let Some(package) = assets.package("UI_SponsorPopup") else { return Vec::new() };
    let mut state = ScreenState::default();
    footer(&mut state, package, "Select");
    layout(assets, package, &state, now - opened)
}

/// What UI_ReplayControl shows of a replay playing.
#[derive(Clone, Debug, Default)]
pub struct ReplayBar {
    /// The replay's time from its start and its length, s.
    pub time: f32,
    pub length: f32,
    /// The player's lap under way (laps done + 1) and the race's laps.
    pub lap: u32,
    pub laps: u32,
    pub paused: bool,
    /// The markers' times from the start, s (REPLAY_TIME_1.., at most 9).
    pub markers: Vec<f32>,
    /// A drag race (0x89E7D8 / 0x890108): the constructor hides the lap
    /// label 0x788C05E8, LapNumber and NumberLaps.
    pub drag: bool,
}

mod replay_control {
    pub const PLAY: u32 = crate::frontend::fe_ids::PLAY;
    pub const PAUSE: u32 = crate::frontend::fe_ids::PAUSE;
    pub const CURRENT: u32 = crate::frontend::fe_ids::CURRENT;
    pub const FILL: u32 = crate::frontend::fe_ids::FILL;
    pub const BACKING: u32 = crate::frontend::fe_ids::REPLAY_CONTROL_BACKING;
    pub const LAP_NUMBER: u32 = crate::frontend::fe_ids::LAP_NUMBER;
    pub const NUMBER_LAPS: u32 = crate::frontend::fe_ids::NUMBER_LAPS;
    /// The "Lap" label (its text from 0x90038D25, 0x4C0AA0).
    pub const LAP_LABEL: u32 = crate::frontend::fe_ids::LAP_LABEL;
    pub const LAP_TIME: u32 = crate::frontend::fe_ids::REPLAY_CONTROL_LAP_TIME;
}

/// UI_ReplayControl (the post-race menu's Replay, 0x4BED90; class vtable
/// 0x79055C, handler 0x4D7BD0). Each tick (0xC98356BA) the time's
/// fraction f = (now − start) / (end − start) of the replay puts
/// REPLAY_CURRENT's centre at the ScrollBar_Backing's left + f × its width
/// and makes ScrollBar_fill f × that width from the left; LapNumber is
/// "%2d" of the player's lap + 1 (no more than the laps), NumberLaps
/// "/%2d" of the laps, CurrentLapTime the time since the start (0x577250).
/// Play/pause (0x4A4E80) shows REPLAY_PAUSE while playing and REPLAY_PLAY
/// while paused (one hides the other: both sit at (−191, 159)). The
/// constructor (0x4C0AA0) puts marker n's REPLAY_TIME_n at the backing's
/// left + f × its width less half the marker's width, at the backing's
/// height, f the marker's fraction of the replay; the rest stay hidden.
pub fn replay_control_quads(assets: &Assets, bar: &ReplayBar, opened: f32, now: f32) -> Vec<Quad> {
    use replay_control::*;
    let Some(package) = assets.package("UI_ReplayControl") else { return Vec::new() };
    let mut state = ScreenState::default();
    footer(&mut state, package, "Pause");
    for o in package.objects.iter().filter(|o| (fe_ids::OBJ_5382B2CA..=0x5382_B2D2).contains(&o.hash) || o.hash == 0xC3D9_0C3A) {
        state.hidden.insert(o.hash);
    }
    let (on, off) = if bar.paused { (PLAY, PAUSE) } else { (PAUSE, PLAY) };
    state.hidden.insert(off);
    state.shown.insert(on);
    let f = if bar.length > 0.0 { (bar.time / bar.length).clamp(0.0, 1.0) } else { 0.0 };
    if let (Some((left, size)), Some(fill)) = (master_box(package, BACKING), package.object(FILL)) {
        let fill_size = fill.attributes.size();
        let at = |x: f32, y: f32| Vec2::new(x, y);
        let width = f * size.x;
        state.size.insert(FILL, at(width, fill_size.y));
        state.position.insert(FILL, at(left.x + width * 0.5, fill.attributes.position().y));
        if let Some(c) = package.object(CURRENT) {
            state.position.insert(CURRENT, at(left.x + width, c.attributes.position().y));
        }
        let backing_y = package.object(BACKING).map_or(0.0, |b| b.attributes.position().y);
        for (n, &t) in bar.markers.iter().take(9).enumerate() {
            let hash = fe_ids::OBJ_5382B2CA + n as u32;
            let Some(m) = package.object(hash) else { continue };
            let f = if bar.length > 0.0 { (t / bar.length).clamp(0.0, 1.0) } else { 0.0 };
            state.hidden.remove(&hash);
            state.shown.insert(hash);
            state.position.insert(hash, at(left.x + f * size.x - m.attributes.size().x * 0.5, backing_y));
        }
    }
    // A drag race (**verified**, 0x4C0AA0: with 0x89E7D8 or 0x890108 set
    // it sends HIDE to 0x788C05E8, 0x57F0E3A5 and 0xF2685238).
    if bar.drag {
        for h in [LAP_LABEL, LAP_NUMBER, NUMBER_LAPS] {
            state.hidden.insert(h);
            state.shown.remove(&h);
        }
    }
    let laps = bar.laps.max(1);
    state.text.insert(LAP_NUMBER, format!("{:2}", bar.lap.clamp(1, laps)));
    state.text.insert(NUMBER_LAPS, format!("/{laps:2}"));
    state.text.insert(LAP_TIME, crate::frontend::hud::time_text((bar.time * 4000.0) as i32));
    layout(assets, package, &state, now - opened)
}

/// UI_PostRaceReward (0x4A4240): 0x18D97A23 (the "Reputation Points
/// Won" row) from 0x5330E0, 0x18D97A24 ("Bank Won") from 0x532E60.
pub fn reward_quads(assets: &Assets, name: &str, reputation: u32, bank: u32, opened: f32, now: f32) -> Vec<Quad> {
    let Some(package) = assets.package("UI_PostRaceReward") else { return Vec::new() };
    let mut state = ScreenState::default();
    footer(&mut state, package, "Next");
    state.text.insert(0x18D9_7A23, reputation.to_string());
    state.text.insert(0x18D9_7A24, bank.to_string());
    state.text.insert(fe_ids::PLAYER_NAME, name.to_string());
    layout(assets, package, &state, now - opened)
}
