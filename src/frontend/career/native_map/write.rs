//! Profile → native save (`Profile::to_native`).

use crate::frontend::career::*;

impl Profile {
    /// Write the profile into `base` (the payload it was loaded from, or
    /// the fresh one): the fields the clone models change, every other
    /// byte stays as the original wrote it.
    pub fn to_native(
        &self,
        base: &crate::formats::native_save::NativeSave,
        ctx: &NativeContext,
    ) -> crate::formats::native_save::NativeSave {
        use crate::formats::native_save::*;
        let mut s = base.clone();
        s.set_name(&self.name);
        s.set_u32(CASH, self.cash);
        s.set_u32(STAGE, self.stage as u32);
        s.set_u32(RACES_ENTERED, self.races_entered);
        s.set_u8(INTRO_CLOSED, self.intro_closed as u8);
        s.set_u8(INTRO_RACES, self.intro_races as u8);
        // A new career's −1 (the clone counts from 0) stays until driven.
        if !(self.drive_time == 0.0 && s.f32(DRIVE_TIME) < 0.0) {
            s.set_f32(DRIVE_TIME, self.drive_time);
        }
        for k in 0..BANK_FLAGS_LEN {
            s.set_u8(BANK_FLAGS + k, self.banks.contains(&(k as u8)) as u8);
        }
        // A current slot holding a car the clone does not know keeps the
        // file's own-car flag.
        if !(self.car.is_empty() && self.slots.get(self.slot).is_some_and(|s| s.opaque)) {
            s.set_u8(HAS_OWN_CAR, (!self.car.is_empty()) as u8);
        }
        s.set_u8(UNDER_WAY, self.intro_played as u8);
        if self.won_hash(ctx, DDAY_EVENT_B) {
            s.set_u8(JUMP_TO_GARAGE, 1);
        }
        // The visit table: the original's entries in their order, states
        // brought to the clone's; new ones appended as 0x512F10 does.
        let mut visits = s.visits();
        let known = visits.len();
        let mut set = |h: u32, f: &dyn Fn(&mut [u8; 4])| {
            if let Some(v) = visits.iter_mut().find(|v| v.0 == h) {
                f(&mut v.1);
            } else {
                let mut b = [0u8; 4];
                f(&mut b);
                if b != [0; 4] {
                    visits.push((h, b));
                }
            }
        };
        for (i, &(h, ..)) in ctx.events.iter().enumerate() {
            let st = if self.completed.contains(&i) { visit::WON } else { self.tried.get(&i).copied().unwrap_or(0) };
            // The D-Day events are won in shops, never through a marker of
            // their own (under Wine both keep +4 = 0).
            let marker = h != DDAY_EVENT_A && h != DDAY_EVENT_B;
            set(h, &|b| {
                // The marker's trigger entered (0x512FF0: +4 = 1; seen
                // under Wine on every event raced or declined). **Inferred**
                // for an event started from the World Map's list. Only for
                // a state the clone changed: an entry the file already
                // holds in that state keeps its +4 (community saves have
                // won events with +4 = 0; the finished-career save's 48, the original keeps
                // them on its load-and-resave).
                if st != 0 && marker && b[2] != st {
                    b[0] = 1;
                }
                if st != 0 || matches!(b[2], visit::WON | visit::CURRENT | visit::DECLINED | visit::LOST) {
                    b[2] = st;
                }
            });
        }
        for &h in &ctx.milestones {
            let done = self.milestones.contains(&h);
            let offered = done || self.offered.contains(&h);
            let spotted = self.spotted.contains(&h);
            set(h, &|b| {
                // The spot driven into (0x5011C0: +5 |= 1).
                if spotted {
                    b[1] |= 1;
                }
                // On offer: +4 = 2; a photo shoot done: +4 = 2 and +6 = 1
                // (0x533690).
                // A milestone the file already holds as won keeps its +4
                // (community saves hold won ones with +4 = 0, kept by the
                // original's load-and-resave).
                let kept = done && b[2] == visit::WON;
                if done {
                    b[2] = visit::WON;
                } else if b[2] == visit::WON {
                    b[2] = 0;
                }
                if kept {
                } else if offered {
                    b[0] = visit::OFFERED;
                } else if b[0] == visit::OFFERED {
                    b[0] = 0;
                }
            });
        }
        for &h in &self.challenges_done {
            set(h, &|b| b[2] = 1);
        }
        for &h in &self.visited {
            set(h, &|b| b[0] = 1);
        }
        for &h in &self.entered {
            set(h, &|b| b[2] = 5);
        }
        // The new entries in the order they were first touched (0x512F10
        // appends), those never noted after them as gathered.
        visits[known..].sort_by_key(|v| self.visit_seq.iter().position(|&h| h == v.0).unwrap_or(usize::MAX));
        s.set_visits(&visits);
        for k in 0..STAGE_COUNT {
            let o = s.stage(k);
            // +0xFC: completed (latched, `stage_done`); every stage below
            // the current one was completed to get there.
            s.set_u8(o + ST_UNLOCKED, (k < self.stage as usize || self.stage_done.contains(&(k as u8))) as u8);
            s.set_u32(o + ST_OUTRUNS, self.outrun_wins.get(&(k as u8)).copied().unwrap_or(0));
            let (rep, n) = self.stage_rep.get(&(k as u8)).copied().unwrap_or((0.0, 0));
            s.set_f32(o + ST_REP, rep);
            s.set_u32(o + ST_REP_COUNT, n);
            let sponsor = self.sponsors.get(&(k as u8)).and_then(|h| ctx.sponsor_name.get(h)).copied().unwrap_or(0);
            s.set_u32(o + ST_SPONSOR_SIGNED, sponsor);
            // +0xC4: the six covers shot in the stage (0x541220 → 0x5410A0
            // files the milestone's hash in the current stage record's
            // first free or matching slot; none past six). The base's
            // prov: inferred frontend.career.to_native
            // order is kept; new ones follow in hash order (**inferred**:
            // the clone does not keep the order shot).
            // A hash the context does not know stays as the file has it.
            let mut covers: Vec<u32> = (0..6)
                .map(|j| s.u32(o + ST_COVERS + 4 * j))
                .filter(|&h| h != 0 && (!ctx.milestones.contains(&h) || self.milestones.contains(&h)))
                .collect();
            for &h in &self.milestones {
                if ctx.milestone_stage.get(&h) == Some(&(k as u8)) && !covers.contains(&h) && covers.len() < 6 {
                    covers.push(h);
                }
            }
            for j in 0..6 {
                s.set_u32(o + ST_COVERS + 4 * j, covers.get(j).copied().unwrap_or(0));
            }
            // The win lists (0x513390): category 3 not SUV, 1, 2.
            for (at, cap, cat) in
                [(ST_NORMAL, 35, Category::Normal), (ST_SPONSOR, 4, Category::Sponsor), (ST_URL, 10, Category::Url)]
            {
                let filed = |i: &usize| self.completed.contains(i) || self.filed.contains(i);
                // An event hash the context does not know stays as the
                // file has it.
                let mut won: Vec<u32> = (0..cap)
                    .map(|j| s.u32(o + at + 4 * j))
                    .filter(|&h| h != 0 && ctx.event_index.get(&h).is_none_or(filed))
                    .collect();
                for (i, &(h, st, c, suv)) in ctx.events.iter().enumerate() {
                    if st as usize == k
                        && c == cat
                        && !(cat == Category::Normal && suv)
                        && filed(&i)
                        && !won.contains(&h)
                    {
                        won.push(h);
                    }
                }
                for j in 0..cap {
                    s.set_u32(o + at + 4 * j, won.get(j).copied().unwrap_or(0));
                }
            }
        }
        s.set_u32(INTRO_EVENT, self.intro.unwrap_or(0));
        for (i, &b) in self.hints.to_bytes().iter().enumerate() {
            s.set_u8(HINTS + i, b);
        }
        s.set_u8(PLACE_SET, self.explore_set as u8);
        if let Some((pos, heading)) = self.explore_pos {
            s.set_f32(PLACE_POS, pos.x);
            s.set_f32(PLACE_POS + 4, pos.y);
            s.set_f32(PLACE_POS + 8, pos.z);
            s.set_u16(
                PLACE_HEADING,
                (heading.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU * 65536.0).round() as u32 as u16,
            );
        }
        for i in 0..self.slots.len().min(5) {
            // The slot as a stash leaves it (no profile copy).
            let cs = self.stashed_slot(i).expect("a slot below len");
            let cs = &*cs;
            let o = s.slot(i);
            let (r, e) = (o + SLOT_RECORD, o + SLOT_EXTRA);
            s.set_u8(o + SLOT_LOCKED, (!cs.unlocked) as u8);
            if cs.car.is_none() && cs.opaque {
                // The file's car of a type the clone does not know: its
                // flag and record stay as loaded.
                continue;
            }
            s.set_u8(o + SLOT_HAS_CAR, cs.car.is_some() as u8);
            // The current slot (career +0x8740) names a career slot once
            // the player has a car of their own; before, it is the D-Day
            // car's and stays.
            if i == self.slot && cs.car.is_some() {
                s.set_u32(CURRENT_SLOT, s.u32(o + SLOT_ID));
            }
            let Some(car) = &cs.car else { continue };
            let hash = bin_hash(car);
            if s.u32(r + REC_CAR) != hash {
                // A car the original has not seen in this slot: a record
                // of its stock parts (the rest of a record, paint and
                // vinyl data included, is **not traced** and left zero).
                s.payload[r..r + RECORD_LEN].fill(0);
                s.set_u32(r, 1);
                s.set_u32(r + REC_CAR, hash);
            }
            for k in 0..PART_SLOTS {
                let stock = ctx.stock_part(car, k);
                let h = cs.installed.get(&(k as u8)).copied().unwrap_or_else(|| ctx.new_record_part(car, k));
                let cur = s.u32(r + REC_PARTS + 4 * k);
                if cs.installed.contains_key(&(k as u8)) || (cur > 1 && cur != stock) || cur == 0 {
                    s.set_u32(r + REC_PARTS + 4 * k, h);
                }
            }
            let up = r + REC_UPGRADE;
            if let Some(levels) = cs.record_levels {
                for (c, v) in levels.iter().enumerate() {
                    s.set_f32(up + 4 * c, *v);
                }
            }
            for id in 0..0x44usize {
                s.set_u8(up + UPGRADE_FITTED + id, cs.perf.contains(&(id as u8)) as u8);
            }
            for i in 0..10usize {
                s.set_u32(up + UPGRADE_UNIQUES + 4 * i, cs.perf_uniques.contains(&(i as u8)) as u32);
            }
            for b in 0..16usize {
                let bits =
                    (0..8).filter(|&k| cs.perf_owned.contains(&((b * 8 + k) as u8))).fold(0u8, |a, k| a | 1 << k);
                s.set_u8(e + EXTRA_PERF_OWNED + b, bits);
            }
            for k in 0..BRANDS {
                s.set_u32(e + EXTRA_BRANDS + 4 * k, cs.brands.get(k).copied().unwrap_or(0));
            }
            s.set_u32(e + EXTRA_SPENT, cs.spent);
            // The owned visual parts (bits the clone's parts do not name
            // keep the file's).
            // Parts that share a bit (0x503B10's sets wrap at 32) own it
            // together: clear every bit a part names, then set the owned.
            let bits = ctx.owned_bits(car);
            for &(_, _, at, mask) in bits.iter() {
                s.set_u8(e + at, s.u8(e + at) & !mask);
            }
            for &(slot, hash, at, mask) in bits.iter() {
                if cs.owned.contains(&(slot, hash)) {
                    s.set_u8(e + at, s.u8(e + at) | mask);
                }
            }
            for (m, t) in cs.tuning.iter().enumerate() {
                let o = r + REC_TUNING + m * TUNING_LEN;
                s.payload[o..o + TUNING_LEN].copy_from_slice(&t.0);
            }
        }
        // The legend's show flags (0x496390: group 13 also sets bytes 6
        // and 8).
        for (g, &hidden) in self.map_hidden.iter().enumerate() {
            s.set_u8(LEGEND_SHOWN + LEGEND_BYTE[g], (!hidden) as u8);
            if g == 13 {
                s.set_u8(LEGEND_SHOWN + 6, (!hidden) as u8);
                s.set_u8(LEGEND_SHOWN + 8, (!hidden) as u8);
            }
        }
        let n = self.inbox.entries.len().min(INBOX_MAX);
        s.payload[INBOX..INBOX + INBOX_LEN * INBOX_MAX].fill(0);
        for (i, m) in self.inbox.entries.iter().take(n).enumerate() {
            let o = INBOX + i * INBOX_LEN;
            s.set_u32(o, m.message);
            s.set_u8(o + 4, m.read as u8);
            s.set_u8(o + 5, m.shown as u8);
            s.set_u8(o + 6, m.deleted as u8);
            s.set_u8(o + 7, m.filed as u8);
            s.set_u32(o + 8, m.body);
            s.set_u32(o + 0xC, m.extra);
            s.set_u32(o + 0x10, m.subject);
        }
        // My Cars: a record the clone made or changed gets the car record
        // 0x52A720 / 0x516B90 would build (the rest of a record, paint
        // included, is **not traced** and left zero); a freed one is
        // cleared as 0x521110 clears it (+4…+0x10 0, filter 2, the car
        // record zeroed).
        for (i, m) in self.my_cars.iter().enumerate().take(GARAGE_CAR_MAX) {
            let o = GARAGE_CARS + i * GARAGE_CAR_LEN;
            let r = o + 0x14;
            match m {
                None => {
                    if s.u32(o + 4) != 0 {
                        s.payload[o..o + 0x10].fill(0);
                        s.set_u32(o + 0x10, 2);
                        s.payload[r..r + RECORD_LEN].fill(0);
                    }
                }
                Some(m) if m.car.is_empty() => {}
                Some(m) => {
                    let hash = bin_hash(&m.car);
                    if s.u32(o + 4) != m.id || s.u32(r + REC_CAR) != hash {
                        s.payload[o..o + 0x14].fill(0);
                        s.payload[r..r + RECORD_LEN].fill(0);
                        s.set_u32(o + 4, m.id);
                        s.set_u32(o + 0xC, 1);
                        s.set_u32(o + 0x10, 2);
                        s.set_u32(r, 1);
                        s.set_u32(r + REC_CAR, hash);
                    }
                    for k in 0..PART_SLOTS {
                        let stock = ctx.stock_part(&m.car, k);
                        let h = m.installed.get(&(k as u8)).copied().unwrap_or_else(|| ctx.new_record_part(&m.car, k));
                        let cur = s.u32(r + REC_PARTS + 4 * k);
                        if m.installed.contains_key(&(k as u8)) || (cur > 1 && cur != stock) || cur == 0 {
                            s.set_u32(r + REC_PARTS + 4 * k, h);
                        }
                    }
                    for id in 0..0x44usize {
                        s.set_u8(r + REC_UPGRADE + UPGRADE_FITTED + id, m.perf.contains(&(id as u8)) as u8);
                    }
                }
            }
        }
        if let Some(id) = self.menu_car_id {
            s.set_u32(RACE_RECORD + 4, id);
        }
        for (i, r) in self.records.iter().take(RECORD_MAX).enumerate() {
            let o = RECORDS + 8 * i;
            s.set_u16(o, r.track as u16);
            s.set_u8(o + 2, r.lapped as u8);
            s.set_u8(o + 3, r.backward as u8);
            s.set_u32(o + 4, r.value as u32);
        }
        s.payload[RECORDS + 0xA00..RECORDS + 0xA50].copy_from_slice(&self.totals.0);
        let unread = self.inbox.unread() as u32;
        if s.u32(INBOX_COUNT) != n as u32 || s.u32(INBOX_UNREAD) != unread {
            s.set_u8(INBOX_NEW, (unread > 0) as u8);
        }
        s.set_u32(INBOX_COUNT, n as u32);
        s.set_u32(INBOX_UNREAD, unread);
        s
    }

    fn won_hash(&self, ctx: &NativeContext, hash: u32) -> bool {
        ctx.event_index.get(&hash).is_some_and(|i| self.completed.contains(i))
    }
}
