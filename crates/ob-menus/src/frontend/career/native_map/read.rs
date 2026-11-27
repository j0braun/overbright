//! Native save → profile (`Profile::from_native`).

use crate::frontend::career::*;

impl Profile {
    /// The clone's profile from a native save (the fields the original's
    /// loader 0x5344E0 restores and the clone models; see docs/formats.md
    /// "Native profile save").
    pub fn from_native(s: &crate::formats::native_save::NativeSave, ctx: &NativeContext) -> Profile {
        use crate::formats::native_save::*;
        let mut p = Profile::new(&s.name());
        p.cash = s.u32(CASH);
        p.stage = s.u32(STAGE).min(255) as u8;
        p.races_entered = s.u32(RACES_ENTERED);
        p.intro_closed = s.u8(INTRO_CLOSED) != 0;
        p.intro_races = s.u8(INTRO_RACES) != 0;
        p.intro_played = s.u8(UNDER_WAY) != 0;
        p.drive_time = s.f32(DRIVE_TIME).max(0.0);
        p.banks = (0..BANK_FLAGS_LEN).filter(|&k| s.u8(BANK_FLAGS + k) != 0).map(|k| k as u8).collect();
        for (h, b) in s.visits() {
            p.visit_seq.push(h);
            if let Some(&i) = ctx.event_index.get(&h) {
                match b[2] {
                    visit::WON => {
                        p.completed.insert(i);
                    }
                    st @ (visit::CURRENT | visit::DECLINED | visit::LOST) => {
                        p.tried.insert(i, st);
                    }
                    _ => {}
                }
            } else if ctx.milestones.contains(&h) {
                if b[1] & 1 != 0 {
                    p.spotted.insert(h);
                }
                if b[2] == visit::WON {
                    p.milestones.insert(h);
                } else if b[0] == visit::OFFERED {
                    p.offered.insert(h);
                }
            } else if ctx.challenges.contains(&h) && b[2] == 1 {
                p.challenges_done.insert(h);
            }
            if ctx.shops.contains(&h) && b[0] == 1 {
                p.visited.insert(h);
            }
            if ctx.shops.contains(&h) && b[2] == 5 {
                p.entered.insert(h);
            }
        }
        for k in 0..STAGE_COUNT {
            let o = s.stage(k);
            if s.u8(o + ST_UNLOCKED) != 0 {
                p.stage_done.insert(k as u8);
            }
            let outruns = s.u32(o + ST_OUTRUNS);
            if outruns > 0 {
                p.outrun_wins.insert(k as u8, outruns);
            }
            let (rep, n) = (s.f32(o + ST_REP), s.u32(o + ST_REP_COUNT));
            if rep != 0.0 || n != 0 {
                p.stage_rep.insert(k as u8, (rep, n));
            }
            if let Some(&h) = ctx.sponsor_by_name.get(&s.u32(o + ST_SPONSOR_SIGNED)) {
                p.sponsors.insert(k as u8, h);
            }
            // A win in the lists whose visit entry is not won (a Restart
            // after it): kept as filed.
            for (at, cap) in [(ST_NORMAL, 35), (ST_SPONSOR, 4), (ST_URL, 10)] {
                for j in 0..cap {
                    if let Some(&i) = ctx.event_index.get(&s.u32(o + at + 4 * j))
                        && !p.completed.contains(&i)
                    {
                        p.filed.insert(i);
                    }
                }
            }
        }
        // The clone's running total; the game keeps only the stage records'.
        p.reputation = p.stage_rep.values().map(|r| r.0.max(0.0) as u32).sum();
        let intro = s.u32(INTRO_EVENT);
        p.intro = (intro != 0).then_some(intro);
        // The career loader 0x529240 copies the 0x190 bytes to career
        // +0x887C, then zeroes career +0x8898, +0x8984, +0x898C and +0x8994
        // (**verified**): the hints' +0x1C (zone), +0x108, +0x110 (drive
        // ticks) and +0x118. The original's own load-and-resave of a save
        // with them set writes zeros there.
        let mut hints: Vec<u8> = (0..HINTS_LEN).map(|i| s.u8(HINTS + i)).collect();
        for o in HINTS_ZEROED_ON_LOAD {
            hints[o..o + 4].fill(0);
        }
        p.hints = crate::frontend::dday::Hints::from_bytes(&hints);
        let pos = Vec3::new(s.f32(PLACE_POS), s.f32(PLACE_POS + 4), s.f32(PLACE_POS + 8));
        p.explore_set = s.u8(PLACE_SET) != 0;
        if p.explore_set || pos != Vec3::ZERO {
            p.explore_pos = Some((pos, s.u16(PLACE_HEADING) as f32 * std::f32::consts::TAU / 65536.0));
        }
        let current = s.u32(CURRENT_SLOT);
        let mut slot = 0;
        for (i, cs) in p.slots.iter_mut().enumerate() {
            let o = s.slot(i);
            let r = o + SLOT_RECORD;
            let e = o + SLOT_EXTRA;
            if s.u32(o + SLOT_ID) == current {
                slot = i;
            }
            *cs = CarSlot { unlocked: s.u8(o + SLOT_LOCKED) == 0, ..CarSlot::default() };
            if s.u8(o + SLOT_HAS_CAR) == 0 {
                continue;
            }
            let Some(car) = ctx.cars.get(&s.u32(r + REC_CAR)).cloned() else {
                // A car type the context does not know: kept as the file
                // has it.
                cs.opaque = true;
                continue;
            };
            for k in 0..PART_SLOTS {
                let h = s.u32(r + REC_PARTS + 4 * k);
                if h > 1 && h != ctx.stock_part(&car, k) {
                    cs.installed.insert(k as u8, h);
                }
            }
            let up = r + REC_UPGRADE;
            cs.perf = (0..0x44u8).filter(|&id| s.u8(up + UPGRADE_FITTED + id as usize) != 0).collect();
            cs.perf_uniques = (0..10u8).filter(|&i| s.u32(up + UPGRADE_UNIQUES + 4 * i as usize) != 0).collect();
            cs.perf_owned =
                (0..0x80u8).filter(|&id| s.u8(e + EXTRA_PERF_OWNED + id as usize / 8) >> (id % 8) & 1 != 0).collect();
            cs.brands = (0..BRANDS).map(|k| s.u32(e + EXTRA_BRANDS + 4 * k)).take_while(|&b| b != 0).collect();
            cs.spent = s.u32(e + EXTRA_SPENT);
            for &(slot, hash, at, mask) in ctx.owned_bits(&car).iter() {
                if s.u8(e + at) & mask != 0 {
                    cs.owned.insert((slot, hash));
                }
            }
            for (m, t) in cs.tuning.iter_mut().enumerate() {
                let o = r + REC_TUNING + m * TUNING_LEN;
                t.0.copy_from_slice(&s.payload[o..o + TUNING_LEN]);
            }
            cs.car = Some(car);
        }
        p.load_slot(slot);
        p.map_hidden = std::array::from_fn(|g| s.u8(LEGEND_SHOWN + LEGEND_BYTE[g]) == 0);
        let n = (s.u32(INBOX_COUNT) as usize).min(INBOX_MAX);
        for i in 0..n {
            let o = INBOX + i * INBOX_LEN;
            p.inbox.entries.push(crate::frontend::sms::Entry {
                message: s.u32(o),
                read: s.u8(o + 4) != 0,
                shown: s.u8(o + 5) != 0,
                deleted: s.u8(o + 6) != 0,
                filed: s.u8(o + 7) != 0,
                body: s.u32(o + 8),
                extra: s.u32(o + 0xC),
                subject: s.u32(o + 0x10),
            });
        }
        for (i, m) in p.my_cars.iter_mut().enumerate() {
            let o = GARAGE_CARS + i * GARAGE_CAR_LEN;
            let id = s.u32(o + 4);
            if id == 0 {
                continue;
            }
            let r = o + 0x14;
            let car = ctx.cars.get(&s.u32(r + REC_CAR)).cloned().unwrap_or_default();
            let mut mc = MyCar { id, car, ..MyCar::default() };
            if !mc.car.is_empty() {
                for k in 0..PART_SLOTS {
                    let h = s.u32(r + REC_PARTS + 4 * k);
                    if h > 1 && h != ctx.stock_part(&mc.car, k) {
                        mc.installed.insert(k as u8, h);
                    }
                }
                mc.perf = (0..0x44u8).filter(|&id| s.u8(r + REC_UPGRADE + UPGRADE_FITTED + id as usize) != 0).collect();
            }
            *m = Some(mc);
        }
        p.totals.0.copy_from_slice(&s.payload[RECORDS + 0xA00..RECORDS + 0xA50]);
        p.records = (0..RECORD_MAX)
            .map(|i| {
                let o = RECORDS + 8 * i;
                Record {
                    track: s.u16(o) as i16,
                    lapped: s.u8(o + 2) as i8,
                    backward: s.u8(o + 3) as i8,
                    value: s.u32(o + 4) as i32,
                }
            })
            .collect();
        // Player 0's car (race record +4): a My Cars record or a stock car.
        let id = s.u32(RACE_RECORD + 4);
        if id != 0 {
            if let Some(m) = p.my_cars.iter().flatten().find(|m| m.id == id && !m.car.is_empty()) {
                p.menu_car = Some((m.car.clone(), m.installed.clone()));
                p.menu_car_id = Some(id);
            } else if let Some(car) = ctx.cars.values().find(|c| stock_id(c) == id) {
                p.menu_car = Some((car.clone(), BTreeMap::new()));
                p.menu_car_id = Some(id);
            }
        }
        p
    }
}
