use crate::*;

impl Game {
    /// The parts a body slot lists (0x5449D0): in a shop the stock, owned
    /// and unlocked ones; in the garage the stock and owned ones (mode 2,
    /// 0x515500). Carbon fibre and the rim brands are not in yet.
    pub(crate) fn slot_parts(&self, slot: u8, carbon: bool) -> Vec<&nfsu2::formats::parts::Part> {
        let Some(p) = self.profile.as_ref() else { return Vec::new() };
        if slot == 0x5E {
            return Vec::new();
        }
        p.shop_parts(&self.game_data.career, &self.game_data.parts, slot as usize, carbon)
            .into_iter()
            .filter(|part| !self.shop.garage_parts || part.level == 0 || p.owned.contains(&(slot, part.hash)))
            .collect()
    }

    /// A part as a browser lists it: hash, name, price, owned, installed.
    pub(crate) fn browser_part(&self, slot: u8, part: &nfsu2::formats::parts::Part) -> screens::BrowserPart {
        let profile = self.profile.as_ref();
        let name = self
            .game_data.parts
            .attribute(part, nfsu2::formats::hash::bin_hash("LANGUAGEHASH"))
            .and_then(|h| self.game_data.assets.strings.get(h))
            .map(str::to_string)
            .or_else(|| self.game_data.parts.string(part.name as u32))
            .unwrap_or_default();
        let price = self.game_data.career.costs.price(self.game_data.parts.category(slot as usize, part), part.level);
        let owned = part.level == 0 || profile.is_some_and(|p| p.owned.contains(&(slot, part.hash)));
        let installed =
            profile.map_or(part.level == 0, |p| p.installed.get(&slot).map_or(part.level == 0, |&h| h == part.hash));
        (part.hash, name, price, owned, installed)
    }

    /// The parts of a slot as a browser category, locked when it has no
    /// part beyond the stock one (0x545710), or none at all for carbon.
    pub(crate) fn browser_category(&self, slot: u8, icon: u32, label: u32, carbon: bool) -> screens::BrowserCategory {
        let parts: Vec<screens::BrowserPart> =
            self.slot_parts(slot, carbon).into_iter().map(|p| self.browser_part(slot, p)).collect();
        let locked = parts.len() <= usize::from(!carbon);
        screens::BrowserCategory {
            slot,
            icon,
            label,
            parts,
            locked,
            action: screens::CategoryAction::Parts,
            text: None,
        }
    }

    /// UI_GenericParts_Browser: the body shop's categories (0x545B60), or
    /// in `sub` the rim brands (0x546530) or the carbon fibre ones
    /// (0x545D40); opening on `category` and, when given, in it on `part`.
    /// Titled "Body Shop" in a shop and "Body Parts Storage" in the garage,
    /// "Rims", "Carbon Fiber" (0x545520).
    pub(crate) fn parts_browser_in(&self, sub: Option<u8>, category: usize, part: Option<usize>) -> Mode {
        use screens::{BrowserCategory, CategoryAction};
        let profile = self.profile.as_ref();
        let car = self.sim.drive.as_ref().map(|d| d.car.car_type.clone()).or_else(|| {
            let name = profile.map(|p| p.car.clone())?;
            self.game_data.car_types.iter().find(|t| t.name == name).cloned()
        });
        let unlocked = |category: u8| profile.is_some_and(|p| p.part_unlocked(&self.game_data.career, category, 3));
        let (title, categories): (u32, Vec<BrowserCategory>) = match sub {
            Some(0x5E) => {
                let mut categories: Vec<BrowserCategory> =
                    [(9, 0xF736_3420), (0x0C, 0xC431_76D4), (7, 0x643B_DE3C), (0x20, 0x10B5_E8D1)]
                        .iter()
                        .map(|&(slot, label)| self.browser_category(slot, label, label, true))
                        .collect();
                // The trunk (0x60) and the kits (0x5D): convert while the
                // "none" part (0xC8D313AC) is on, else revert; locked until
                // their cost category's level 3 unlocks (0x52A1B0).
                for (slot, cost, convert, revert) in [
                    (0x60u8, 0x56u8, (0xF811_1A17, 0xF811_1A17), (0x8F5D_C19F, 0x1036_E38E)),
                    (0x5D, 0x55, (0x246D_E0CB, 0xB229_FA7E), (0x2BDB_0653, 0xE170_F115)),
                ] {
                    let none = 0xC8D3_13AC;
                    let on = profile.and_then(|p| p.installed.get(&slot).copied()).unwrap_or(none);
                    let carbon = car
                        .as_ref()
                        .and_then(|c| self.game_data.parts.find(&c.name, slot as usize, None, Some(3)))
                        .map_or(0, |p| p.hash);
                    let ((icon, label), fit) = if on == none { (convert, carbon) } else { (revert, none) };
                    categories.push(BrowserCategory {
                        slot,
                        icon,
                        label,
                        parts: Vec::new(),
                        locked: !unlocked(cost),
                        action: CategoryAction::Toggle(fit),
                        text: None,
                    });
                }
                (0x2491_1E04, categories)
            }
            _ => {
                let mut categories: Vec<BrowserCategory> = screens::BODY_SHOP_CATEGORIES
                    .iter()
                    .map(|&(slot, icon, label)| match slot {
                        // Rims: open with at least two rims to pick from
                        // (0x539C30).
                        0x1D => BrowserCategory {
                            slot,
                            icon,
                            label,
                            parts: Vec::new(),
                            locked: self.slot_parts(0x1D, false).len() < 2,
                            action: CategoryAction::Open,
                            text: None,
                        },
                        // Carbon fibre: open with a carbon part for a slot
                        // or the trunk or kits unlocked (0x544F10).
                        0x5E => {
                            let none = [9u8, 0x0C, 0x20, 7].iter().all(|&s| self.slot_parts(s, true).is_empty());
                            BrowserCategory {
                                slot,
                                icon,
                                label,
                                parts: Vec::new(),
                                locked: none && !unlocked(0x56) && !unlocked(0x55),
                                action: CategoryAction::Open,
                                text: None,
                            }
                        }
                        _ => self.browser_category(slot, icon, label, false),
                    })
                    .collect();
                // The locks are the strip's as it was built (0x545B60 runs
                // only from 0x54FAF0: the constructor and the carbon fibre
                // leave): a purchase does not open a category until the
                // screen is built again (Wine, the original's career at stage 2: Carbon
                // Fiber stayed locked after a spoiler bought in the same
                // visit, orig/70-c*.jpg).
                let mut latch = self.shop.strip_locks.borrow_mut();
                match latch.as_ref() {
                    Some(locks) if locks.len() == categories.len() => {
                        for (c, &l) in categories.iter_mut().zip(locks) {
                            c.locked = l;
                        }
                    }
                    _ => *latch = Some(categories.iter().map(|c| c.locked).collect()),
                }
                (if self.shop.garage_parts { 0x25F0_C994 } else { 0x796D_C449 }, categories)
            }
        };
        Mode::Menu(Menu::Parts(PartsBrowser {
            package: "UI_GenericParts_Browser",
            title,
            sub,
            categories,
            category,
            part,
            opened: self.now(),
            total: None,
            arrows: Default::default(),
            motion: Default::default(),
        }))
    }

    pub(crate) fn parts_browser(&self, category: usize, part: Option<usize>) -> Mode {
        self.parts_browser_in(None, category, part)
    }

    /// UI_GenericParts_Browser built anew (its constructor 0x55E830, or
    /// back from carbon fibre, 0x567830): the strip's locks taken now.
    pub(crate) fn new_parts_browser(&self, category: usize) -> Mode {
        self.shop.strip_locks.replace(None);
        self.parts_browser(category, None)
    }

    /// The career car's type.
    pub(crate) fn career_car_type(&self) -> Option<&nfsu2::car::CarType> {
        let name = self.profile.as_ref()?.car.clone();
        self.game_data.car_types.iter().find(|t| t.name == name)
    }
}
