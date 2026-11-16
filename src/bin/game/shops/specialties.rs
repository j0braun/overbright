use crate::*;

impl Game {
    /// UI_ICEMAIN, the car specialties shop (0x5593F0, mode 0): its part
    /// categories across OPTION_MASTER, each listing the slot's parts as
    /// the body shop does (0x5449D0; the split hoods are the kind-5 hoods),
    /// locked with nothing beyond stock.
    pub(crate) fn specialties_browser(&self, category: usize, part: Option<usize>) -> Mode {
        let profile = self.profile.as_ref();
        let categories: Vec<screens::BrowserCategory> = screens::SPECIALTIES
            .iter()
            .map(|&(slot, icon, label, kind)| {
                // Neon opens its own slots (0x539FC0): locked while all
                // three are (0x539D10).
                if slot == 0x22 {
                    // 0x539D10: locked with no layout beyond stock.
                    let locked = self.slot_parts(0x22, false).len() <= 1;
                    return screens::BrowserCategory {
                        slot,
                        icon,
                        label,
                        parts: Vec::new(),
                        locked,
                        action: screens::CategoryAction::Open,
                        text: None,
                    };
                }
                // Spinners open UI_ChooseSpinner (0x568750); the item's
                // lock is 0x539D10(0x1D), as the other kinds' (0x53A9E0).
                if slot == 0x1D {
                    let locked = self.slot_parts(0x1D, false).len() <= 1;
                    return screens::BrowserCategory {
                        slot,
                        icon,
                        label,
                        parts: Vec::new(),
                        locked,
                        action: screens::CategoryAction::Open,
                        text: None,
                    };
                }
                if slot == 0x9E {
                    let locked = screens::NEON.iter().all(|n| self.neon_slot_locked(n.0));
                    return screens::BrowserCategory {
                        slot,
                        icon,
                        label,
                        parts: Vec::new(),
                        locked,
                        action: screens::CategoryAction::Open,
                        text: None,
                    };
                }
                let listed: Vec<&nfsu2::formats::parts::Part> = match (kind, profile) {
                    (Some(k), Some(p)) => {
                        let mut out = Vec::new();
                        let mut after = None;
                        while let Some(x) = self.game_data.parts.find_after(&p.car, slot as usize, after) {
                            after = Some(x);
                            if x.kind == k
                                && (p.owned.contains(&(slot, x.hash))
                                    || p.part_listed(
                                        &self.game_data.career,
                                        self.game_data.parts.category(slot as usize, x),
                                        x.level,
                                        x.hash,
                                    ))
                            {
                                out.push(x);
                            }
                        }
                        out
                    }
                    _ => self.slot_parts(slot, false),
                };
                let parts: Vec<screens::BrowserPart> = listed.into_iter().map(|x| self.browser_part(slot, x)).collect();
                let locked = parts.len() <= usize::from(kind.is_none());
                screens::BrowserCategory {
                    slot,
                    icon,
                    label,
                    parts,
                    locked,
                    action: screens::CategoryAction::Parts,
                    text: None,
                }
            })
            .collect();
        // The garage's storage (0x83898C = 1): "Car Specialties Storage"
        // (0xF4B3B4FC, 0x54724B) and no Custom Gauges (0x559737 skips slot
        // 0xA3).
        let (title, categories): (u32, Vec<screens::BrowserCategory>) = if self.shop.garage_parts {
            (0xF4B3_B4FC, categories.into_iter().filter(|c: &screens::BrowserCategory| c.slot != 0xA3).collect())
        } else {
            (0xDF86_227A, categories)
        };
        let category = category.min(categories.len().saturating_sub(1));
        Mode::Menu(Menu::Parts(PartsBrowser {
            package: "UI_ICEMAIN",
            title,
            sub: None,
            categories,
            category,
            part,
            opened: self.now(),
            total: None,
            arrows: Default::default(),
        }))
    }

    /// The fitted trunk layout's level (slot 0x22; 0 with none or stock). The
    /// part is FUN_00610300(0x22); 0x55BFC0 tests its byte +5 & 0xE0.
    pub(crate) fn trunk_layout_level(&self) -> u8 {
        let p = self.profile.as_ref();
        p.and_then(|p| p.installed.get(&0x22).and_then(|&h| self.game_data.parts.find(&p.car, 0x22, Some(h), None)))
            .map_or(0, |x| x.level)
    }

    /// The trunk audio item (slot 0x22, 0x53AAD0) in the car parts list: its
    /// select (event 0xC407210, 0x55BFC0) opens mode 2, the Trunk Layout
    /// screen alone under "TRUNK AUDIO" with its overlay open, when no layout
    /// is fitted or the fitted one is stock; otherwise mode 3.
    pub(crate) fn trunk_audio(&self) -> Mode {
        if self.trunk_layout_level() != 0 {
            return self.audio_browser(false, 0, None);
        }
        let categories = vec![self.browser_category(0x22, 0xF7E5_B04A, 0xC8D2_3393, false)];
        // Its overlay open on the fitted layout (Wine, the finished-career save's
        // stock trunk: the list on STOCK with the column; the clone showed
        // the strip alone).
        let part = categories[0].parts.iter().position(|x| x.4).or((!categories[0].parts.is_empty()).then_some(0));
        Mode::Menu(Menu::Parts(PartsBrowser {
            package: "UI_ICEMAIN",
            title: 0x02AA_DAC5,
            sub: Some(0x22),
            categories,
            category: 0,
            part,
            opened: self.now(),
            total: None,
            arrows: Default::default(),
        }))
    }

    /// The trunk audio (UI_ICEMAIN modes 3 and 4, 0x5593F0): first Audio
    /// Components (icon 0x76167B7E, label 0x28BF66F6, opening its slots)
    /// and Trunk Layout (slot 0x22) under "TRUNK AUDIO"; in `components`,
    /// the slots the fitted layout opens (0x610400), "Trunk Slot %d" each.
    pub(crate) fn audio_browser(&self, components: bool, category: usize, part: Option<usize>) -> Mode {
        use screens::{BrowserCategory, CategoryAction};
        let p = self.profile.as_ref();
        let layout_level = self.trunk_layout_level();
        let car = p.and_then(|p| self.game_data.car_types.iter().position(|t| t.name == p.car)).unwrap_or(0);
        let slots = screens::trunk_slots(layout_level, car);
        let categories: Vec<BrowserCategory> = if components {
            let format = self.game_data.assets.strings.get(0x3F4B_9E43).unwrap_or("Trunk Slot %d").to_string();
            slots
                .map(|slot| {
                    let mut c = self.browser_category(slot, 0xE803_E1CD, 0x3F4B_9E43, false);
                    c.text = Some(format.replace("%d", &(slot - 0x22).to_string()));
                    c
                })
                .collect()
        } else {
            vec![
                BrowserCategory {
                    slot: 0x23,
                    icon: 0x7616_7B7E,
                    label: 0x28BF_66F6,
                    parts: Vec::new(),
                    locked: slots.is_empty(),
                    action: CategoryAction::Open,
                    text: None,
                },
                self.browser_category(0x22, 0xF7E5_B04A, 0xC8D2_3393, false),
            ]
        };
        let sub = Some(if components { 0x23 } else { 0x22 });
        Mode::Menu(Menu::Parts(PartsBrowser {
            package: "UI_ICEMAIN",
            title: 0x02AA_DAC5,
            sub,
            categories,
            category,
            part,
            opened: self.now(),
            total: None,
            arrows: Default::default(),
        }))
    }

    /// A neon slot is locked (0x539D10) with nothing beyond stock in it
    /// (0x539970); engine neon (0x9B) also until the level-3 engine
    /// (performance category 3) unlocks in a shop (0x52A120 → 0x515630);
    /// trunk neon (0x9D) also without a level-3 trunk neon part (0x50ED20)
    /// that passes the trunk layout's (slot 0x22) unlock (0x52A020 →
    /// 0x515500).
    pub(crate) fn neon_slot_locked(&self, slot: u8) -> bool {
        let Some(p) = self.profile.as_ref() else { return self.slot_parts(slot, false).len() <= 1 };
        let gate = match slot {
            0x9B => self.perf_unlocked(3, 3),
            0x9D => {
                let mut after = None;
                let mut level3 = None;
                while let Some(x) = self.game_data.parts.find_after(&p.car, 0x9D, after) {
                    after = Some(x);
                    if x.level == 3 {
                        level3 = Some(x);
                        break;
                    }
                }
                level3.is_some_and(|x| p.part_listed(&self.game_data.career, self.game_data.parts.category(0x22, x), x.level, x.hash))
            }
            _ => true,
        };
        !gate || self.slot_parts(slot, false).len() <= 1
    }

    /// UI_CustomNeonMain (0x554C50 → 0x553500, slots 0x548020): the neon
    /// slots across OPTION_MASTER, their parts down UI_NeonPartsOverlay.
    pub(crate) fn neon_browser(&self, category: usize, part: Option<usize>) -> Mode {
        let categories = screens::NEON
            .iter()
            .map(|&(slot, icon, label)| {
                let mut c = self.browser_category(slot, icon, label, false);
                c.locked = self.neon_slot_locked(slot);
                c
            })
            .collect();
        Mode::Menu(Menu::Parts(PartsBrowser {
            package: "UI_CustomNeonMain",
            title: 0xDF86_227A,
            sub: None,
            categories,
            category,
            part,
            opened: self.now(),
            total: None,
            arrows: Default::default(),
        }))
    }
}
