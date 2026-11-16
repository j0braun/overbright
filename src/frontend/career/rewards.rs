//! Cash and reputation: start cash, race rewards, sponsors, banks.

use super::*;


/// Cash a new career starts with: the sum of the two bonus amounts
/// 0x865128 + 0x86512C (0x53291D), which only the bonus codes regmybank
/// and ordermebaby set ($200 and $1000, 0x5760B0; `cheats`); otherwise 0.
pub fn start_cash() -> u32 {
    crate::frontend::cheats::start_cash()
}

/// A won race's (reputation, bank) (UI_PostRaceReward, 0x4A4240; the win
/// handler 0x541430 pays the same): the bank is 0x532E60 — the event's
/// purse (+0x30), or, for a sponsor race (+0x34 = 1) with a sponsor signed
/// in the current stage (stage record +0xE8), that sponsor's +0x02 —
/// passed in as `bank`; the reputation comes from the winning margin (% ahead
/// of the runner-up, 0x513FE0) through table 0x7F7AC8 by the event's
/// reward class (0x512AE0): margins of 1, 2 and 5% (drift 10, 20 and 30%)
/// give 100, 200 and 300 points.
pub fn race_reward(event: &Event, margin: f32, bank: u32) -> (u32, u32) {
    (reputation_points(event, margin), bank)
}

/// The reward class (0x512AE0): 6 a URL race (+0x34 = 2); a plain (3),
/// sponsor (1) or SUV (+0x0D = 1) event by type (circuit 1, sprint 3,
/// street X 5, drag 2, drift 4, anything else 0); other categories (the
/// scripted 0) 0.
fn reward_class(event: &Event) -> u8 {
    match event.category {
        Category::Url => 6,
        Category::Normal | Category::Sponsor => kind_class(event.kind),
        _ if event.suv => kind_class(event.kind),
        _ => 0,
    }
}

fn kind_class(kind: EventKind) -> u8 {
    match kind {
        EventKind::Circuit => 1,
        EventKind::Sprint => 3,
        EventKind::StreetX => 5,
        EventKind::Drag => 2,
        EventKind::Drift => 4,
        _ => 0,
    }
}

/// 0x512B50: the class's row of table 0x7F7AC8 (7 rows of 0x1C: class,
/// three margins, three points; every row 1/2/5 → 100/200/300 but class 4,
/// drift, 10/20/30); below the first margin, or a class without a row, 0
/// (0x7D7D3C).
pub fn reputation_points(event: &Event, margin: f32) -> u32 {
    let class = reward_class(event);
    let steps: [f32; 3] = if class == 4 { [10.0, 20.0, 30.0] } else { [1.0, 2.0, 5.0] };
    if class == 0 || margin < steps[0] {
        0
    } else if margin < steps[1] {
        100
    } else if margin < steps[2] {
        200
    } else {
        300
    }
}

impl Profile {
    /// A won event's bank (0x532E60): a sponsor race (+0x34 = 1) pays the
    /// sponsor signed in the current stage (0x862838's record +0xE8) its
    /// +0x02; any other race, or a sponsor race with none signed, the
    /// event's purse (+0x30).
    pub fn race_bank(&self, data: &CareerData, event: &Event) -> u32 {
        if event.category == Category::Sponsor
            && let Some(s) = self.sponsors.get(&self.stage).and_then(|h| data.sponsors.get(h))
        {
            return s.purse as u32;
        }
        event.reward
    }

    /// The sponsors offered now (0x500610, 0x500670): none once one is
    /// signed; otherwise 0x500670 counts the stage's sponsors whose win
    /// count (stage def +0x1C) the stage's normal wins reach and whose
    /// reputation (+0x0E) the previous stage's average points a win reach, and
    /// UI_Sponsorship_new (0x4C1596) lists that many of the stage's
    /// sponsors from the first (0x5006D0 by index), not the ones counted.
    pub fn sponsor_offers<'a>(&self, data: &'a CareerData) -> Vec<&'a Sponsor> {
        if self.sponsors.contains_key(&self.stage) {
            return Vec::new();
        }
        let Some(def) = data.stages.get(self.stage as usize) else {
            return Vec::new();
        };
        let wins = self.stage_wins(data, Category::Normal).len() as i32;
        // 0x4A4610 / 0x4C1576: 0x5005E0 on the record before the current
        // one (0x861F14 + 0x104 × stage): the previous stage's average
        // points a win (under Wine: stage 1 at 300 a win offered all five
        // stage-2 sponsors after the first stage-2 win, before its points
        // were filed).
        let previous = self.stage.checked_sub(1).unwrap_or(0);
        let average = self.stage_rep.get(&previous).map_or(0.0, |&(sum, n)| if n > 0 { sum / n as f32 } else { 0.0 });
        let count = def
            .sponsors
            .iter()
            .enumerate()
            .filter(|&(i, _)| def.sponsor_wins.get(i).is_some_and(|&w| w as i32 <= wins))
            .filter_map(|(_, h)| data.sponsors.get(h))
            .filter(|s| s.reputation as f32 <= average)
            .count();
        def.sponsors.iter().take(count).filter_map(|h| data.sponsors.get(h)).collect()
    }

    /// A sponsor is on offer: the post-race chain shows UI_SponsorPopup
    /// and UI_Sponsorship_new (0x4A4610, from stage 2).
    pub fn needs_sponsor(&self, data: &CareerData) -> bool {
        self.stage > 1 && !self.sponsor_offers(data).is_empty()
    }

    /// Sign a sponsor for the current stage: its signing bonus goes to the
    /// bank.
    /// Sign a sponsor (0x532D00): with none signed in the stage, the next
    /// car slot opens (0x5284E0) and the signing bonus is paid.
    pub fn sign(&mut self, sponsor: &Sponsor) {
        if !self.sponsors.contains_key(&self.stage) {
            self.unlock_slot();
        }
        self.sponsors.insert(self.stage, sponsor.hash);
        self.cash = (self.cash as i64 + sponsor.bonus as i64).max(0) as u32;
    }

    /// Whether a cash pickup can be collected (0x513890; 0x5141B0 lists
    /// those at career +0x800 for XEE_BANK the same way): condition 1 the
    /// stage above +0x04, condition 2 the visit-table entry +0x04 in state
    /// 1 (0x500B80: an event won), any other always; and its flag (career
    /// +0x8768 + flag) clear.
    pub fn bank_open(&self, data: &CareerData, b: &Bank) -> bool {
        let ok = match b.condition {
            1 => (self.stage as u32) > b.value,
            2 => self.won(data, b.value),
            _ => true,
        };
        ok && !self.banks.contains(&b.flag)
    }

    /// The car in a cash-pickup trigger (route-trigger kind 0x13; 0x5EB12F
    /// → 0x5283D0 → 0x528380): the first 0x34A1E record naming the
    /// trigger, if open, adds its cash to the bank (career +0x7134) and
    /// sets its flag. Returns the cash collected.
    pub fn collect_bank(&mut self, data: &CareerData, trigger: u32) -> Option<u16> {
        let b = *data.banks.iter().find(|b| b.trigger == trigger)?;
        if !self.bank_open(data, &b) {
            return None;
        }
        self.cash = self.cash.saturating_add(b.cash as u32);
        self.banks.insert(b.flag);
        Some(b.cash)
    }
}
