use crate::*;

impl Game {
    /// The running world challenge's unique reward (0x533D30, from the
    /// fill 0x53ABE0 through 0x539AA0): for each of the record's slots
    /// +0x15..+0x17 (0 and −1 skipped), with the record's +0x14 kind 1
    /// (visual) the slot's unique parts — 0x3E the vinyls (slot 0x40, any
    /// level, part byte +5 = 0x98: kind 0x18, level 4), 0xD the rims
    /// (slot 0x1D level 4 by 0x61BA40(car, 0x1D, 0, 4): one per name,
    /// replaced by a later one of the same name when its 0xCE7D8DB5 is the
    /// car's +0xDC and no less than the kept one's and its 0xEB0101E2 is
    /// larger but no more than the car's +0xDE; then 0x53ABE0 keeps those
    /// of the car's +0xDC), else slot 0x5020C0(n) at level 4; with kind 2
    /// (performance) the unique 0x502240(n) when the car does not have it
    /// yet (car record +0x2F4 + 4n). The parts are free (0x514280 prices
    /// level 4 at 0).
    pub(crate) fn unique_reward(&self) -> Vec<UniqueItem> {
        let (Some(t), Some(p)) = (self.challenge_record(), self.profile.as_ref()) else { return Vec::new() };
        let car = p.car.clone();
        let car_type = self.career_car_type();
        let (size, max) = car_type.map_or((0, 0), |c| (c.rim_size as i8 as i32, c.rim_max as i8 as i32));
        let attr = |part: &nfsu2::formats::parts::Part, key: u32| {
            self.game_data.parts.attribute(part, key).map_or(0, |v| v as u8 as i8 as i32)
        };
        let walk = |slot: usize| {
            let mut out = Vec::new();
            let mut at = self.game_data.parts.find_after(&car, slot, None);
            while let Some(part) = at {
                out.push(part);
                at = self.game_data.parts.find_after(&car, slot, Some(part));
            }
            out
        };
        let mut items = Vec::new();
        for &n in &t.slots {
            let n = n as i8;
            if n == -1 || n == 0 {
                continue;
            }
            match t.kind {
                1 if n == 0x3E => {
                    items.extend(
                        walk(0x40)
                            .into_iter()
                            .filter(|q| q.kind == 0x18 && q.level == 4)
                            .map(|q| UniqueItem::Part { slot: 0x40, hash: q.hash }),
                    );
                }
                1 if n == 0xD => {
                    let mut kept: Vec<&nfsu2::formats::parts::Part> = Vec::new();
                    for q in walk(0x1D).into_iter().filter(|q| q.level == 4) {
                        let name = self.game_data.parts.string(q.name as u32);
                        match kept.iter_mut().find(|k| self.game_data.parts.string(k.name as u32) == name) {
                            None => kept.push(q),
                            Some(k) => {
                                let s = attr(q, 0xCE7D_8DB5);
                                let inch = attr(q, 0xEB01_01E2);
                                if s == size && s >= attr(k, 0xCE7D_8DB5) && attr(k, 0xEB01_01E2) < inch && inch <= max
                                {
                                    *k = q;
                                }
                            }
                        }
                    }
                    // 0x53ABE0: only the car's size shows.
                    items.extend(
                        kept.into_iter()
                            .filter(|q| attr(q, 0xCE7D_8DB5) == size)
                            .map(|q| UniqueItem::Part { slot: 0x1D, hash: q.hash }),
                    );
                }
                1 => {
                    if let Some(slot) = unique_slot(n as u8) {
                        items.extend(
                            walk(slot as usize)
                                .into_iter()
                                .filter(|q| q.level == 4)
                                .map(|q| UniqueItem::Part { slot, hash: q.hash }),
                        );
                    }
                }
                2 => {
                    if let Some(i) = perf_unique_index(n as u8)
                        && !p.perf_uniques.contains(&i)
                    {
                        items.push(UniqueItem::Perf(i));
                    }
                }
                _ => {}
            }
        }
        items
    }

    /// UI_ChooseUniquePart (descriptor 0x7F8730, factory 0x554B30 → 0x553210,
    /// vtable 0x79CAA8, messages 0x561730, fill 0x53ABE0), the screen of a
    /// kind-6 shop (0x50E2E0; 0x520430 sets 0x838990 = 6) — the shop front
    /// end 0x528570 takes THE_UNIQUES_SHOP's in the challenge's state 4,
    /// whichever shop was entered. Its options are `unique_reward`'s, each
    /// on the strip: a part's icon 0x50E640 / label 0x50E6F0 by slot (6 /
    /// 9 / 0xC / 0x1D / 0x40–0x43, else 0x74CE8C0B / none), a performance
    /// unique's icon 0x50E820 / label 0x50E790. The refresh (vtable +0x10,
    /// 0x5618C0) titles it "Unique Part Select" (0x9986A6E3 on 0x42ADB44C)
    /// and puts the selected option's name on +0x134: a performance
    /// unique's label; a part's LANGUAGEHASH (0x4732DA07) string, else its
    /// name (0x610010), rims as "%s %s %d\"" with the brand's name
    /// (0x50E960 of attribute 0xEBB03E66) and 0xEB0101E2. Accept's answer (0xD05FC3A3) fits and
    /// buys the part (0x55C240, 0x528A00) or adds the performance unique
    /// (0x521440), then marks the challenge's reward taken (0x5419D0).
    /// Back (0x911AB364) asks "If you don't choose a unique …"
    /// (0x20D0D219, GenericDialog_SMALL, Yes 0x34DC1BEC → 0x5419D0; No
    /// 0x1FAB5998 stays). Either way +0x150 is set, and the next tick
    /// (0xC98356BA) commits the car (0x54FD50) and leaves for the game
    /// (0x5404B0).
    pub(crate) fn unique_part_screen(&self) -> Mode {
        let categories = self
            .unique_reward()
            .into_iter()
            .map(|item| {
                let (slot, icon, label, hash, text) = match item {
                    UniqueItem::Part { slot, hash } => (
                        slot,
                        unique_part_icon(slot),
                        unique_part_label(slot),
                        hash,
                        Some(self.unique_part_name(slot, hash)),
                    ),
                    UniqueItem::Perf(i) => {
                        (0xFF, PERF_UNIQUE_ICONS[i as usize], PERF_UNIQUE_LABELS[i as usize], u32::from(i), None)
                    }
                };
                screens::BrowserCategory {
                    slot,
                    icon,
                    label,
                    parts: vec![(hash, String::new(), 0, true, false)],
                    locked: false,
                    action: screens::CategoryAction::Parts,
                    text,
                }
            })
            .collect();
        Mode::Menu(Menu::Parts(PartsBrowser {
            package: "UI_ChooseUniquePart",
            title: 0x9986_A6E3,
            sub: None,
            categories,
            category: 0,
            part: None,
            opened: self.now(),
            total: None,
            arrows: Default::default(),
        }))
    }

    /// A unique part's name on UI_ChooseUniquePart (0x5618C0): rims "%s %s
    /// %d\"" of the brand's name, the part's name and its size; others the
    /// LANGUAGEHASH string or the name. The brand's name is 0x50E960's
    /// (`screens::rim_brand_name`, **verified** at run time): the
    /// career brand table's name ("5ZIGEN") first, else the brand label.
    pub(crate) fn unique_part_name(&self, slot: u8, hash: u32) -> String {
        let Some(p) = self.profile.as_ref().and_then(|p| self.game_data.parts.find(&p.car, slot as usize, Some(hash), None))
        else {
            return String::new();
        };
        let lang = self.game_data.parts.attribute(p, 0x4732_DA07).and_then(|h| self.game_data.assets.strings.get(h)).map(str::to_string);
        let name = lang.unwrap_or_else(|| self.game_data.parts.string(p.name as u32).unwrap_or_default());
        if slot == 0x1D {
            let brand = self.game_data.parts.attribute(p, 0xEBB0_3E66).map_or(String::new(), |b| {
                screens::rim_brand_name(&self.game_data.career.brands, &self.game_data.assets.strings, b)
            });
            let size = self.game_data.parts.attribute(p, 0xEB01_01E2).map_or(0, |v| v as u8 as i8 as i32);
            return format!("{brand} {name} {size}\"");
        }
        name
    }

    /// The unique-part screen's answer (0x561730 on 0xD05FC3A3, or Yes
    /// 0x34DC1BEC to its Back's question when `item` is None): the part
    /// fitted and owned (free: level 4), or the performance unique added
    /// (0x521440: car record +0x2F4 + 4n = 1), then 0x5419D0 ends the
    /// challenge (its name's flag +6 = 1, the record cleared, markers
    /// rebuilt) and the next tick leaves for the game (0x5404B0).
    pub(crate) fn take_unique(&mut self, item: Option<UniqueItem>) -> Result<()> {
        if let Some(p) = self.profile.as_mut() {
            match item {
                Some(UniqueItem::Part { slot, hash }) => {
                    p.own_part(slot, hash);
                    // 0x55C240: fitted, the ride stored (0x55C35A).
                    let before = p.installed.clone();
                    p.fit_visual(&self.game_data.parts, slot, hash);
                    p.store_visual(&self.game_data.parts, &self.game_data.career.packages, &before);
                    self.drop_drive();
                }
                Some(UniqueItem::Perf(i)) => {
                    // 0x521440: the flag, then 0x639CF0(1, …) and 0x503950.
                    let before = p.perf.clone();
                    p.perf_uniques.insert(i);
                    p.store_ride(&self.game_data.parts, &self.game_data.career.packages, &before);
                }
                None => {}
            }
        }
        // Rachel's kind 8 / 9 (0x5255E0 / 0x525620), Accept only.
        if let (Some(it), Some(p)) = (item, self.profile.as_mut()) {
            p.hints.unique_taken(matches!(it, UniqueItem::Perf(_)));
        }
        self.finish_challenge();
        if item.is_some() {
            let rating = self.visual_rating();
            if let Some(p) = self.profile.as_mut() {
                p.offer_covers(&self.game_data.career, rating);
            }
        }
        if let Some(p) = self.profile.as_ref() {
            let _ = p.commit();
        }
        self.explore(self.here())
    }
}
