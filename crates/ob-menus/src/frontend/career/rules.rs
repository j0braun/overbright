//! Career rules: stage progress, wins, open events, outruns.

use super::*;

impl Profile {
    /// Whether the event (or milestone) with this hash is won (0x500B80).
    pub fn won(&self, data: &CareerData, hash: u32) -> bool {
        self.milestones.contains(&hash)
            || data.events.iter().any(|e| e.unlock == hash && self.completed.contains(&e.index))
    }

    /// The current stage's wins by category, as the stage record keeps
    /// them (0x862018 + n × 0x104): normal races at +0x00 (0x5004D0),
    /// sponsor races at +0x8C (0x5005A0), URL races at +0x9C (0x500520).
    /// The win handler files them so (0x513390): SUV events are in none.
    pub fn stage_wins<'a>(&self, data: &'a CareerData, category: Category) -> Vec<&'a Event> {
        self.stage_wins_in(data, self.stage, category)
    }

    /// [`Self::stage_wins`] of stage record `stage`.
    pub fn stage_wins_in<'a>(&self, data: &'a CareerData, stage: u8, category: Category) -> Vec<&'a Event> {
        data.events
            .iter()
            .filter(|e| {
                e.stage == stage
                    && e.category == category
                    && !(category == Category::Normal && e.suv)
                    && (self.completed.contains(&e.index) || self.filed.contains(&e.index))
            })
            .collect()
    }

    /// The current stage's wins as the stage record counts them for the
    /// opponents' skill (0x5132E0): normal (0x5004D0), sponsor (its four
    /// +0x8C entries) and URL races (0x500520).
    pub fn stage_win_count(&self, data: &CareerData) -> u32 {
        // 0x5004D0: the record's 35 normal slots.
        let normal = self.stage_wins(data, Category::Normal).len().min(35);
        let sponsor = self.stage_wins(data, Category::Sponsor).len().min(4);
        let url = self.stage_wins(data, Category::Url).len();
        (normal + sponsor + url) as u32
    }

    /// The stage's required milestones are reached (0x513490).
    fn media_done(&self, data: &CareerData) -> bool {
        data.milestones
            .iter()
            .filter(|m| m.stage == self.stage && m.required)
            .all(|m| self.milestones.contains(&m.hash))
    }

    /// The most open events of `slot` (0x527AB0): the stage's caps, an SUV
    /// cap only when driving an SUV, and for sponsor slots the races the
    /// signed sponsor asks for of that type less those won (0x5137C0).
    fn slot_cap(&self, data: &CareerData, def: &StageDef, slot: usize) -> Option<usize> {
        match slot {
            0..=4 => Some(def.caps[slot].max(0) as usize),
            5 => Some(if data.suvs.contains(&self.car) { def.caps[5].max(0) as usize } else { 0 }),
            6..=10 => {
                // Sponsor race codes 1 circuit, 3 drag, 2 drift, 4 sprint, 5 street X.
                let code = [1, 3, 2, 4, 5][slot - 6];
                let Some(sponsor) = self.sponsors.get(&self.stage).and_then(|h| data.sponsors.get(h)) else {
                    return Some(0);
                };
                let asked = sponsor.races.iter().filter(|&&r| r == code).count();
                let won = self.stage_wins(data, Category::Sponsor).iter().filter(|e| e.slot() == Some(slot)).count();
                Some(asked.saturating_sub(won))
            }
            _ => None,
        }
    }

    /// Whether a URL race is open (0x513510).
    fn url_open(&self, data: &CareerData, e: &Event) -> bool {
        let normal = self.stage_wins(data, Category::Normal).len() as u32;
        let r = e.requirement;
        match e.tier {
            3 => r >> 16 <= normal,
            4 => {
                self.media_done(data)
                    && (r >> 8 & 0xFF) <= normal
                    && (r >> 24) as usize <= self.stage_wins(data, Category::Url).len()
                    && (r >> 16 & 0xFF) as usize <= self.stage_wins(data, Category::Sponsor).len()
            }
            _ => false,
        }
    }

    /// Rebuild the open events (0x527D40): none once the stage's last race
    /// is won; the open events not yet won stay; each slot fills to its cap
    /// with a random pick among the stage's other unwon events (0x527BC0);
    /// the always-open events follow up to the stage's cap; and the URL
    /// races whose requirements are met.
    /// The slot picks draw on the game's shared generator `rng` (0x527BC0's
    /// 0x43C1C0 on 0x7EA624; the original has no career generator).
    pub fn refresh_open(&mut self, data: &CareerData, rng: &mut crate::opponents::GameRng) {
        let previous = std::mem::take(&mut self.open);
        // 0x527D40's gates: stage 0 opens nothing until its races are
        // revealed (+0x873F), nor while DDAY_EVENT_B is current or once
        // they are closed again (+0x873E); none either while a world
        // challenge runs (+0xC08 at 0x527DA3: the list, copied aside and
        // cleared at 0x527D4B..0x527D84, stays empty, and the copy is
        // dropped, so the rebuild after the challenge draws every slot
        // anew).
        if self.stage == 0 && (!self.intro_races || self.intro == Some(DDAY_EVENT_B) || self.intro_closed) {
            return;
        }
        if self.challenge_running {
            return;
        }
        let Some(def) = data.stages.get(self.stage as usize).cloned() else {
            return;
        };
        let by_hash = |h: u32| data.events.iter().find(|e| e.unlock == h);
        // Stage 5's last race opens once the race it follows is won.
        if self.stage == 5
            && let Some(last) = by_hash(0x04CC_E3E1)
            && self.won(data, last.requirement)
            && !self.won(data, last.unlock)
        {
            self.open.push(last.unlock);
            return;
        }
        if def.final_event != 0 && self.won(data, def.final_event) {
            return;
        }
        for h in previous {
            if by_hash(h).is_some_and(|e| e.stage == self.stage && (!e.suv || data.suvs.contains(&self.car)))
                && !self.won(data, h)
            {
                self.open.push(h);
            }
        }
        let suv = data.suvs.contains(&self.car);
        for slot in 0..11 {
            let cap = self.slot_cap(data, &def, slot);
            loop {
                let count = self.open.iter().filter(|&&h| by_hash(h).and_then(Event::slot) == Some(slot)).count();
                if cap.is_some_and(|c| count >= c) {
                    break;
                }
                let mut pool: Vec<u32> = data
                    .events
                    .iter()
                    .filter(|e| {
                        e.stage == self.stage && e.category != Category::Url && !e.always && e.unlock != 0x04CC_E3E1
                    })
                    .filter(|e| e.slot() == Some(slot) && (!e.suv || suv))
                    .filter(|e| !self.won(data, e.unlock) && !self.open.contains(&e.unlock))
                    .map(|e| e.unlock)
                    .collect();
                if pool.is_empty() {
                    break;
                }
                let n = pool.len() as u32;
                for i in 0..pool.len() {
                    let j = rng.below(n) as usize;
                    pool.swap(i, j);
                }
                self.open.push(pool[0]);
            }
        }
        for e in data.events.iter().filter(|e| e.stage == self.stage && e.always) {
            let open_always = self.open.iter().filter(|&&h| by_hash(h).is_some_and(|e| e.always)).count();
            if def.always_cap >= 0 && open_always >= def.always_cap as usize {
                break;
            }
            if !self.won(data, e.unlock) && !self.open.contains(&e.unlock) {
                self.open.push(e.unlock);
            }
        }
        for e in data.events.iter().filter(|e| e.stage == self.stage && e.category == Category::Url) {
            if !self.won(data, e.unlock) && !self.open.contains(&e.unlock) && self.url_open(data, e) {
                self.open.push(e.unlock);
            }
        }
        self.open.truncate(64);
    }

    /// Note a visit-table entry's first write (see [`Self::visit_seq`]).
    pub fn touch(&mut self, hash: u32) {
        if !self.visit_seq.contains(&hash) {
            self.visit_seq.push(hash);
        }
    }

    /// A career race lost, settled on leaving UI_PostRace (0x528510):
    /// the event's visit entry +6 = 4 unless it is won.
    pub fn lost(&mut self, event: &Event) {
        self.touch(event.unlock);
        if !self.completed.contains(&event.index) {
            self.tried.insert(event.index, visit::LOST);
        }
    }

    /// A career race starts (0x541170): the event's visit entry +6 = 2
    /// (the current event) unless it is won.
    pub fn race_started(&mut self, event: &Event) {
        self.touch(event.unlock);
        if !self.completed.contains(&event.index) {
            self.tried.insert(event.index, visit::CURRENT);
        }
    }

    /// The race dialog declined (0x499C00, 0x911AB364 → 0x499BB0 →
    /// 0x5138F0): the marker's event's visit entry +6 = 3.
    pub fn race_declined(&mut self, event: &Event) {
        self.touch(event.unlock);
        if !self.completed.contains(&event.index) {
            self.tried.insert(event.index, visit::DECLINED);
        }
    }

    /// The open events, as events.
    pub fn open_events<'a>(&self, data: &'a CareerData) -> Vec<&'a Event> {
        self.open.iter().filter_map(|&h| data.events.iter().find(|e| e.unlock == h)).collect()
    }

    /// Record a win (0x541430): the event's visit entry is marked won; the
    /// bank goes to the cash and the reputation points to the stage record
    /// of the event's own stage (+0x37; 0x513430, not for the D-Day
    /// events); then the win's messages (0x529930; the stage record has
    /// the win already: the race's end 0x54CA70, the shop goal 0x54CB00
    /// and 0x541690 file it, 0x513390, before calling 0x541430); then the
    /// stage end check on the event's stage record (0x532C70), Rachel's
    /// hints (0x5242D0, by the caller) and the world-challenge triggers
    /// (0x533B50).
    ///
    /// Then the open events are rebuilt as the next city start does
    /// (0x60A690 → 0x5412E0 → 0x533320 → 0x527D40; 0x541430 itself does
    /// not): a helper for walking a career outside the game.
    pub fn win(&mut self, data: &CareerData, root: &Path, event: &Event, time: f32, reward: (u32, u32), rng: &mut crate::opponents::GameRng) {
        self.win_paid(data, root, event, time, Some(reward));
        self.refresh_open(data, rng);
    }

    /// [`Self::win`], paying `reward` only when given: 0x541430 pays the
    /// bank and files the reputation (0x513430: +0xE0 points, +0xE4 one
    /// race) only with career +0xD26 set and +0xD27 clear (0x500DB0), not
    /// for a race started from the World Map's event list (0x5325F0 sets
    /// +0xD27). The rest of the handler runs either way.
    pub fn win_paid(&mut self, data: &CareerData, root: &Path, event: &Event, time: f32, reward: Option<(u32, u32)>) {
        let _ = root;
        self.touch(event.unlock);
        self.completed.insert(event.index);
        self.filed.remove(&event.index);
        self.tried.remove(&event.index);
        if let Some((reputation, bank)) = reward {
            self.cash += bank;
            self.reputation += reputation;
            if event.unlock != DDAY_EVENT_A && event.unlock != DDAY_EVENT_B {
                let r = self.stage_rep.entry(event.stage).or_insert((0.0, 0));
                r.0 += reputation as f32;
                r.1 += 1;
            }
        }
        let best = self.best.entry(event.index).or_insert(time);
        *best = best.min(time);
        self.sms_after_win(data, event);
        self.check_stage(data, event.stage);
        // 0x541430 ends with 0x533B50: the stage's condition-0 triggers
        // (normal wins equal to +0x07) on the stage as the end check left
        // it.
        let normal = self.stage_wins(data, Category::Normal).len();
        self.fire_triggers(data, 0, normal);
        // The open events are not rebuilt here: 0x527D40's callers are
        // 0x533320 (the marker table), 0x5291F0 and the debug 0x541690,
        // none of them in 0x541430. The won event stays in the list (and
        // in a save made at the save question) until the city's start
        // rebuilds it (`Game::explore`), as seen under Wine.
    }

    /// An outrun's end in the career (0x541330, from 0x5F84A0 with whether
    /// the player's racer status is 6): a win adds the stage definition's
    /// +0x02 to the bank (career +0x7134) and counts in the stage record's
    /// +0xDC, a loss takes it off, at least 0; then 0x529BE0's messages.
    /// Then 0x533AA0 (below), Rachel's line 0x5110C0 (the caller's
    /// `hints.outrun_end`), 0x533320 (below) and the HUD's marker rebuild
    /// 0x4F92A0. 0x529BE0 sends each message once (0x514CB0). Returns the
    /// bank the race-over message shows.
    pub fn outrun_result(&mut self, data: &CareerData, won: bool, rng: &mut crate::opponents::GameRng) -> i16 {
        let bank = data.stages.get(self.stage as usize).map_or(0, |d| d.outrun_bank);
        self.cash = crate::outrun::settle(self.cash, won, bank);
        if won {
            *self.outrun_wins.entry(self.stage).or_insert(0) += 1;
        }
        // 0x529BE0: SMS_OUTRUN_VICTORY (condition 5) on a win,
        // SMS_OUTRUN_DEFEAT (6) on a loss.
        let wanted = if won {
            crate::frontend::sms::condition::OUTRUN_WON
        } else {
            crate::frontend::sms::condition::OUTRUN_LOST
        };
        for m in data.messages.iter().filter(|m| m.condition == wanted) {
            self.inbox.add(m.hash);
        }
        // 0x533AA0(won): on a win, the stage's condition-2 triggers whose
        // count equals its outrun wins (stage record +0xDC).
        if won {
            let wins = self.outrun_wins.get(&self.stage).copied().unwrap_or(0) as usize;
            self.fire_triggers(data, 2, wins);
        }
        // Then 0x533320: the open events and the marker table rebuilt
        // (0x527D40; with the slots full this draws nothing).
        self.refresh_open(data, rng);
        bank
    }

    /// Whether the career's encounter side may ask for opponents now
    /// (0x513D30 with [`crate::outrun::offered`]): the stage's outrun wins
    /// against its definition's +0x40.
    pub fn outruns_offered(&self, data: &CareerData, challenge: bool, timer: f32) -> bool {
        // Stage 0: only with no car of the player's own yet (0x86347D,
        // the D-Day intro in Rachel's car) and its races not closed
        // (0x86347E); `challenge`: a world challenge runs (career +0xC08 =
        // 0x85B948, set by 0x533BF0).
        let cap = data.stages.get(self.stage as usize).map_or(0, |d| d.required);
        crate::outrun::offered(
            self.stage,
            (!self.car.is_empty(), self.intro_closed),
            challenge,
            self.outrun_wins.get(&self.stage).copied().unwrap_or(0),
            cap,
            timer,
        )
    }

    /// The stage end check (0x532C70) on stage record `stage` (the won
    /// event's +0x37): unless the record is already complete (+0xFC), a
    /// stage with a final event (def +0x28) completes once it is won
    /// (0x500B80) and the stage's required milestones are reached
    /// (0x513490); one without (stage 1) once every event of the stage
    /// (+0x37) is won (0x5130A0). Completing it advances the career only
    /// when it is the current stage (0x528330).
    pub(super) fn check_stage(&mut self, data: &CareerData, stage: u8) {
        // 0x532C70: a record already completed (+0xFC) checks nothing
        // more, so a save holding the current stage's record completed
        // stays at that stage (a community save, RaZOR, holds stage 5
        // with record 5 completed).
        // A record other than the current one is checked by the original
        // too (and latched), but the clone checks only the current one:
        // prov: open frontend.career.check_stage
        // which stage 0x513490 tests for another record is **open**.
        if stage != self.stage || self.stage_done.contains(&stage) {
            return;
        }
        // Stage 5 too: its definition's +0x28 is the last race, and once
        // it is won (with the stage's media milestones) 0x528330 moves the
        // career to stage 6 (0x7AF8), which has no definition: no events
        // open (0x527D40) and outruns are always offered (0x513D30:
        // stage > 5). The clone used to stop at 5.
        if let Some(def) = data.stages.get(stage as usize) {
            let done = if def.final_event != 0 {
                self.won(data, def.final_event) && self.media_done(data)
            } else {
                data.events.iter().filter(|e| e.stage == stage).all(|e| self.completed.contains(&e.index))
            };
            if done {
                self.stage_done.insert(stage);
                // 0x528330: the current stage's record advances the career.
                self.stage += 1;
            }
        }
    }
}
