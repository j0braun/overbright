//! SMS and magazine rules: messages after wins, covers, challenges, tips.

use super::*;

impl Profile {
    /// Whether message `m`'s win condition holds after `event` is won
    /// (0x501370): 1 the event's hash is +0x08 or +0x0C; 9, 10 and 12 the
    /// sponsor (its four slots, +0x8C), URL (ten, +0x9C) or normal wins
    /// (0x5004D0) of stage record +0x08 >> 16 reach +0x08 & 0xFFFF.
    fn sms_win_condition(&self, data: &CareerData, m: &crate::frontend::sms::Message, event: &Event) -> bool {
        use crate::frontend::sms::condition::*;
        let (stage, count) = ((m.value[0] >> 16) as u8, (m.value[0] & 0xFFFF) as usize);
        match m.condition {
            EVENT_WON => m.value.contains(&event.unlock),
            SPONSOR_WINS => self.stage_wins_in(data, stage, Category::Sponsor).len().min(4) >= count,
            URL_WINS => self.stage_wins_in(data, stage, Category::Url).len().min(10) >= count,
            NORMAL_WINS => self.stage_wins_in(data, stage, Category::Normal).len().min(35) >= count,
            _ => false,
        }
    }

    /// A win's messages (0x541430 → 0x529930, after the stage record has
    /// the win, 0x513390, and before the stage can end, 0x532C70): every
    /// message not yet in the inbox whose condition holds arrives.
    pub fn sms_after_win(&mut self, data: &CareerData, event: &Event) {
        for m in &data.messages {
            if !self.inbox.contains(m.hash) && self.sms_win_condition(data, m, event) {
                self.inbox.add(m.hash);
            }
        }
    }

    /// Entering a shop (0x54CB00 → 0x529870 with the shop): condition 8
    /// arrives at any shop, 2 at a shop whose hash (+0x38) is the
    /// message's +0x08 or +0x0C.
    pub fn sms_shop_entered(&mut self, data: &CareerData, shop: u32) {
        use crate::frontend::sms::condition::*;
        for m in &data.messages {
            if m.condition == ANY_SHOP || (m.condition == SHOP && m.value.contains(&shop)) {
                self.inbox.add(m.hash);
            }
        }
    }

    /// Free roam's frame (0x551330, career, in game, not online): in
    /// race state 4 the driving time (+0x8764) grows by the frame time,
    /// then (0x529760) every condition-3 message whose +0x08 seconds it
    /// passes arrives. Returns whether an urgent message waits to open
    /// the mailbox (an entry not yet shown, 0x529760's second half).
    pub fn sms_drive(&mut self, data: &CareerData, dt: f32) -> bool {
        self.drive_time += dt;
        for m in &data.messages {
            if m.condition == crate::frontend::sms::condition::DRIVE_TIME && (m.value[0] as f32) < self.drive_time {
                self.inbox.add(m.hash);
            }
        }
        self.inbox.urgent_pending(&data.messages)
    }

    /// Offer the stage's DVD covers the car's visual rating reaches, and
    /// withdraw those it no longer does (0x5331D0).
    pub fn offer_covers(&mut self, data: &CareerData, rating: f32) {
        for m in
            data.milestones.iter().filter(|m| m.is_dvd() && m.stage == self.stage && !self.milestones.contains(&m.hash))
        {
            if rating >= m.rating {
                // A cover offered anew (no state record yet, 0x533277):
                // the condition-14 messages whose +0x08 is its rating
                // (ftol of +0x3C, 0x5299C0).
                // Every reached DVD goes to Rachel's hints (0x5332A0 →
                // 0x525400, which drops a cover it already holds).
                self.hints.milestone(m.kind, m.cover, m.rating);
                if !self.visit_seq.contains(&m.hash) {
                    self.visit_seq.push(m.hash);
                }
                if self.offered.insert(m.hash) {
                    let want = m.rating as i32 as u32;
                    for msg in data
                        .messages
                        .iter()
                        .filter(|x| x.condition == crate::frontend::sms::condition::DVD_OFFERED && x.value[0] == want)
                    {
                        self.inbox.add(msg.hash);
                    }
                }
            } else {
                self.offered.remove(&m.hash);
            }
        }
        // Then career +0x8895 (the hints' +0x19, 0x5332F6 / 0x533309):
        // ftol of the highest +0x3C among the stage's DVD covers the
        // rating does not reach (the loop keeps a record when none is kept
        // or the kept one's +0x3C is lower, 0x5332C0), else 0.
        let due = data
            .milestones
            .iter()
            .filter(|m| m.is_dvd() && m.stage == self.stage && m.rating > rating)
            .map(|m| m.rating)
            .fold(None, |best: Option<f32>, r| Some(best.map_or(r, |b| if b < r { r } else { b })));
        self.hints.rating_due = due.map_or(0, |r| r as i32 as u8);
    }

    /// 0x525660 after `Hints::zone_changed`: 0x500970 on career +0x7148
    /// counts the open events (+0x80, 64 slots) with +0x7F set and, with
    /// any, draws Rand_Int(count) — then takes that many-th *open* event
    /// counting every filled slot, +0x7F or not (as the game does). Its
    /// marker (+0x28, 0x4FFC50), the kind-0x11 zone holding the marker's
    /// point (+4) and that zone's `NEIGHBOURHOOD_` hash: when it is not
    /// the zone just entered and is one of the thirteen, Rachel's
    /// Intro_Events (0x10, bit, 8 or 0x10 by Rand_Int(2), 0x510F70; drawn
    /// only with a speech object, `speech`) and +0x20 = the driving time.
    /// (**Inferred**: `open`'s order stands for the slots'.)
    pub fn neighbourhood_line(
        &mut self,
        data: &CareerData,
        triggers: &RouteTriggers,
        speech: bool,
        rng: &mut crate::opponents::GameRng,
    ) -> Option<Vec<u32>> {
        let always = |h: &u32| data.events.iter().find(|e| e.unlock == *h).is_some_and(|e| e.always);
        let count = self.open.iter().filter(|h| always(h)).count();
        if count == 0 {
            return None;
        }
        let k = rng.below(count as u32) as usize;
        let event = data.events.iter().find(|e| Some(&e.unlock) == self.open.get(k))?;
        let marker = triggers.find(event.marker)?;
        let zone = triggers.zone_at(marker.position.truncate())?;
        let hash = neighbourhood_hash(triggers.zone_name(zone));
        if hash == 0 || hash == self.hints.zone {
            return None;
        }
        let bit = NEIGHBOURHOODS
            .iter()
            .find(|n| crate::formats::hash::bin_hash(&format!("NEIGHBOURHOOD_{}", n.0)) == hash)?
            .1;
        let line = speech.then(|| crate::audio::speech::lines::intro_events(bit, [8, 0x10][rng.below(2) as usize]));
        self.hints.zone_time = self.hints.drive_sum;
        line
    }

    /// The world challenges on offer (0x527D40 → 0x527980, the list at
    /// career +0x7148, 32 slots, in table order): none while one runs
    /// (career +0xC08, the caller's `running`) or in stage 0; each record
    /// of the current stage (+0x04 = career +0x7AF8) not done (its name's
    /// flag +6 ≠ 1) whose condition's count reaches +0x07: 0 the stage's
    /// normal wins (0x5004D0), 1 its URL wins (0x500520), 2 its outrun wins
    /// (stage record +0xDC), 3 its sponsor wins (0x5005A0). Indices into
    /// `data.mag_triggers`.
    pub fn challenges_on_offer(&self, data: &CareerData, running: bool) -> Vec<usize> {
        if running || self.stage == 0 {
            return Vec::new();
        }
        data.mag_triggers
            .iter()
            .enumerate()
            .filter(|(_, t)| t.stage == self.stage as i16 && !self.challenges_done.contains(&t.name))
            .filter(|(_, t)| {
                let have = match t.condition {
                    0 => self.stage_wins(data, Category::Normal).len(),
                    1 => self.stage_wins(data, Category::Url).len(),
                    2 => self.outrun_wins.get(&self.stage).copied().unwrap_or(0) as usize,
                    3 => self.stage_wins(data, Category::Sponsor).len(),
                    _ => return false,
                };
                have >= t.count as usize
            })
            .map(|(i, _)| i)
            .take(32)
            .collect()
    }

    /// A world challenge starts (0x533BF0): its name's flag +6 = 2 (not
    /// kept by the clone: only "done" matters to 0x527980) and its goal
    /// milestone, if any, offered (state +4 = 2) — the magazine offers.
    pub fn start_challenge(&mut self, data: &CareerData, t: &crate::frontend::sms::MagTrigger) {
        if data.milestones.iter().any(|m| m.hash == t.milestone) {
            self.touch(t.milestone);
            self.offered.insert(t.milestone);
        }
    }

    /// A world challenge given up after its time ran out (0x501640): its
    /// goal milestone's offer is withdrawn (state +4 = 0).
    pub fn abandon_challenge(&mut self, t: &crate::frontend::sms::MagTrigger) {
        self.offered.remove(&t.milestone);
    }

    /// What a photo shoot pays (0x541940 → 0x541220): the milestone's
    /// +0x26 plus ftol(the stage record's +4 × the car's visual rating
    /// (0x528420) + 0.005), added to the bank (career +0x7134).
    pub fn cover_reward(&mut self, data: &CareerData, milestone: u32, rating: f32) -> i32 {
        let Some(m) = data.milestones.iter().find(|m| m.hash == milestone) else {
            return 0;
        };
        let per = data.stages.get(self.stage as usize).map_or(0, |s| s.cover_cash);
        let share = (per as f32 * rating + 0.005) as i32;
        let cash = m.cash as i32 + share;
        self.cash = (self.cash as i64 + cash as i64).max(0) as u32;
        cash
    }

    /// The photo shoot of an offered milestone is done (0x533690).
    pub fn shoot(&mut self, data: &CareerData, milestone: u32, rng: &mut crate::opponents::GameRng) {
        // 0x541940 → 0x541220 → 0x5410A0: filed in the stage record's
        // covers (+0xC4, see `to_native`), then the stage end check
        // 0x532C70 on the current stage record — before 0x533690 marks
        // the milestone done (+6 = 1), so 0x513490 does not count it yet.
        self.check_stage(data, self.stage);
        self.offered.remove(&milestone);
        self.touch(milestone);
        self.milestones.insert(milestone);
        self.refresh_open(data, rng);
    }

    /// 0x533B50 / 0x533AA0: every 0x34A1B record of the current stage
    /// (0x862838, +0x04) with this condition (+0x06) whose count (+0x07)
    /// equals `count` sends its message (0x5338D0; once, 0x514CB0, though
    /// the counters step regardless). Kinds 3 / 4 also post Rachel's
    /// milestone hint (0x525400, speech and marker: **not ported**).
    pub fn fire_triggers(&mut self, data: &CareerData, condition: u8, count: usize) {
        for t in data
            .mag_triggers
            .iter()
            .filter(|t| t.condition == condition && t.stage == self.stage as i16 && t.count as usize == count)
        {
            // 0x533B50 / 0x533AA0: kinds 3 and 4 also go to Rachel's
            // hints (0x525400 with 0x4FFF80's milestone), message or not.
            if matches!(t.kind, 3 | 4)
                && let Some(m) = data.milestones.iter().find(|m| m.hash == t.milestone)
            {
                self.hints.milestone(m.kind, m.cover, m.rating);
            }
            if crate::frontend::sms::find(&data.messages, t.message).is_none() {
                continue;
            }
            let cover = data.milestones.iter().find(|m| m.hash == t.milestone).map(|m| m.cover);
            let (body, subject, extra) = crate::frontend::sms::trigger_labels(t, cover, &mut self.trigger_counters);
            SESSION_TRIGGER_COUNTERS.set(self.trigger_counters);
            self.inbox.add_custom(t.message, body, subject, extra);
        }
    }

    /// The car in an engage-tip trigger (route-trigger kind 0x12; 0x5EB0F8
    /// → 0x532D50 → 0x529C50): the condition-15 message whose +0x08 is the
    /// trigger's hash arrives once, and its +0x0C goes to the bank (career
    /// +0x7134); then 0x528990 lists the tip triggers still unsent.
    /// Returns whether one arrived.
    pub fn sms_tip(&mut self, data: &CareerData, trigger: u32) -> bool {
        let Some(m) =
            data.messages.iter().find(|m| m.condition == crate::frontend::sms::condition::TIP && m.value[0] == trigger)
        else {
            return false;
        };
        if !self.inbox.add(m.hash) {
            return false;
        }
        self.cash = self.cash.saturating_add(m.value[1]);
        true
    }

    /// Whether an engage-tip trigger is listed at career +0x600 (0x528990
    /// with 0x514E10): a condition-15 message names it (+0x08) and is not
    /// in the inbox yet. The list is what XEE_GAME_TIP stands on (0x605790).
    pub fn tip_pending(&self, data: &CareerData, trigger: u32) -> bool {
        data.messages
            .iter()
            .find(|m| m.condition == crate::frontend::sms::condition::TIP && m.value[0] == trigger)
            .is_some_and(|m| !self.inbox.contains(m.hash))
    }
}
