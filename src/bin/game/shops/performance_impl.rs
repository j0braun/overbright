use crate::*;

impl Game {
    /// A performance package (category, level 1–3), when there is one.
    pub(crate) fn package(&self, category: u8, level: u8) -> Option<&nfsu2::performance::Package> {
        self.game_data.career.packages.iter().find(|p| p.category == category && p.level == level)
    }

    /// A performance level unlocked in a shop (0x515630 with flag 4: the
    /// cost table's record for the category, 0x5021C0, at the level).
    /// In main-menu Customize (`scratch`, flag 1: 0x529E40 / 0x52A120)
    /// also a level a performance code opened (0x579DA0: needperformance1
    /// / 2).
    pub(crate) fn perf_unlocked(&self, category: u8, level: u8) -> bool {
        self.profile.as_ref().is_some_and(|p| {
            (p.scratch && nfsu2::frontend::cheats::performance(level))
                || p.part_unlocked(&self.game_data.career, nfsu2::performance::cost_category(category), level)
        })
    }

    /// UI_ChoosePerformanceCategory (screen 0x559C40 → 0x556C90; options
    /// 0x530500, each added by 0x530410): locked until one of its levels
    /// unlocks.
    pub(crate) fn perf_category_menu(&self, selected: usize) -> Mode {
        let mut options: Vec<screens::IconOption> = nfsu2::performance::CATEGORIES
            .iter()
            .map(|&(category, icon, label)| {
                let open = (1..4).any(|l| self.perf_unlocked(category, l));
                screens::IconOption {
                    icon,
                    label,
                    action: if open { Action::PerfCategory(category) } else { Action::Unavailable },
                }
            })
            .collect();
        // 0x530500 adds "Performance Tuning / Dyno" (0x7A9B397A /
        // 0x8513F2C0, an option of vtable 0x79B580) after Turbo in a career
        // (0x850078 = 1) with 0x83898C = 0, i.e. in the shops, not the
        // garage's storage (Wine, IND_PERF_SHOP_1: the tenth entry, asking
        // "Do you want to enter the Performance Tuning / Dyno track?").
        if self.save_flow.customize.is_none() && !self.shop.garage_parts && self.profile.as_ref().is_some_and(|p| !p.scratch) {
            options.push(screens::IconOption { icon: 0x7A9B_397A, label: 0x8513_F2C0, action: Action::Tuning });
        }
        // In the garage's storage (0x83898C = 1, 0x556D81): "Performance
        // Part Storage" (0xDCCD75F8) on 0x42ADB44C.
        let title = self.shop.garage_parts.then(|| self.label(0xDCCD_75F8, "Performance Part Storage"));
        let mut menu = IconMenu::new(MenuKind::PerfCategory.package(), title.as_deref(), options, self.now());
        menu.select_now(selected);
        Mode::Menu(Menu::Icons(menu, MenuKind::PerfCategory))
    }

    /// UI_ChoosePerformancePackage (screen 0x55C8E0 → 0x559CA0; options
    /// 0x556E70): Choose Individual Parts, the Street / Pro / Extreme
    /// packages (PERFORMANCE_PACKAGE_n, icon PERFORMANCE_ICON_PACKAGE_n;
    /// locked until unlocked), Return To Stock. A package lists its parts
    /// under Package Contents (0x547910 → 0x53B7C0): "Installed" when
    /// fitted, "Owned" when bought (both running Purchased on the price,
    /// which shows PURCHASED_ICON_n), else the price `%$d` (running Init);
    /// rows past its parts hide (0x547B70, which also hides the icons).
    /// The total (0x5620E0 with 0x5415F0) is what the parts neither fitted
    /// nor bought cost (0x540FF0), 10% off when there are more than one
    /// (0x541050), or "n/a" when nothing is due. The contents show only for
    /// an open package: Choose Individual Parts and Return To Stock fade
    /// them out (PackageFadeOut) and a locked option hides them (0x5620E0).
    /// The acceleration, top speed and handling bars (0x547A10, 0x54FFA0)
    /// set the car as it is against the car with the option applied: the
    /// highlighted part fitted (UI_BuyPerformanceParts), the package's
    /// parts fitted (0x521350), or the category's parts off (0x521A30); a
    /// locked option shows the car as it is.
    /// One past the last package level offered: the rows run from 1 while
    /// below 4, or below 3 at the shop CC_MEGALOW ("Megalow Parts",
    /// 0x6AA654C5) — 0x5627AD / 0x556F13 with a visited shop (0x863444,
    /// 0x86344C +0x38 = 0x7FC8D0D0) — so that shop has no Extreme package.
    /// It is also Return To Stock's row.
    pub(crate) fn perf_levels(&self) -> u8 {
        if self.shop.visit == Some(MEGALOW) { 3 } else { 4 }
    }

    pub(crate) fn perf_package_menu(&self, selected: usize) -> Mode {
        const ROWS: [u32; 4] = [0xFB71_7AA7, 0xFB71_7AA8, 0xFB71_7AA9, 0xFB71_7AAA];
        const PRICES: [u32; 4] = [0x2AF3_1879, 0x2AF3_187A, 0x2AF3_187B, 0x2AF3_187C];
        const ICONS: [u32; 4] = [0x8C69_E4B6, 0x8C69_E4B7, 0x8C69_E4B8, 0x8C69_E4B9];
        const TOTAL: u32 = 0x0380_B632;
        const CONTENTS: u32 = 0x3D3F_9F04;
        let category = self.shop.perf_category;
        let fitted = self.perf_fitted();
        let owned = |id: u8| self.profile.as_ref().is_some_and(|p| p.perf_owned.contains(&id));
        let career = self.profile.as_ref().is_some_and(|p| !p.scratch);
        let current = self.perf_ratings(&fitted);
        let mut options =
            vec![screens::IconOption { icon: 0xC7D3_F6A7, label: 0xDF3F_D1D6, action: Action::PerfParts }];
        let mut extra = vec![Vec::new()];
        // UI_BuyPerformanceParts takes the focus on Choose Individual Parts
        // (0x556E70 pushes it): the strip's glow (CATEGORY_ONLY) is off
        // (Wine, the finished-career save: no green frame on its icon).
        let mut states = vec![screens::OptionState { hidden: vec![CONTENTS, 0xDF42_2345], ..Default::default() }];
        let mut previews = vec![
            self.shop.perf_parts
                .as_ref()
                .and_then(|l| l.row())
                .map_or(current, |r| self.perf_ratings(&self.perf_with(r.id))),
        ];
        // The packages are left out in a career's storage (0x556EF2: game
        // mode 1 and 0x83898C = 1; Wine, the finished-career save: Choose Individual
        // Parts and Return To Stock only).
        let levels = if self.shop.garage_parts && career { 1 } else { self.perf_levels() };
        for level in 1..levels {
            let label = nfsu2::formats::hash::bin_hash(&format!("PERFORMANCE_PACKAGE_{level}"));
            let icon = nfsu2::formats::hash::bin_hash(&format!("PERFORMANCE_ICON_PACKAGE_{level}"));
            let open = self.package(category, level).is_some() && self.perf_unlocked(category, level);
            options.push(screens::IconOption {
                icon,
                label,
                action: if open { Action::PerfPackage(level) } else { Action::Unavailable },
            });
            let mut texts = Vec::new();
            let mut state = screens::OptionState { hidden: ICONS.to_vec(), ..Default::default() };
            let mut preview = current;
            if let Some(p) = self.package(category, level) {
                for (i, part) in p.parts.iter().enumerate().take(4) {
                    let name = self.game_data.assets.strings.get(part.name).unwrap_or("").to_string();
                    texts.push((ROWS[i], name));
                    let purchased = if fitted.contains(&part.id) {
                        Some(self.label(0xED56_BE1F, "Installed"))
                    } else if owned(part.id) {
                        Some(self.label(0x0371_711C, "Owned"))
                    } else {
                        None
                    };
                    match purchased {
                        Some(text) => {
                            texts.push((PRICES[i], text));
                            state.scripts.push((PRICES[i], "PURCHASED"));
                            state.hidden.retain(|&o| o != ICONS[i]);
                            state.shown.push(ICONS[i]);
                            state.scripts.push((ICONS[i], "PURCHASED"));
                        }
                        // Outside a career "Not Installed" (under Wine in
                        // prov: inferred game.perf_package_menu
                        // Customize; **inferred** the rows' data comes from
                        // the part row's 0x53B7C0, which says so when
                        // 0x850078 ≠ 1, as UI_BuyPerformanceParts' do).
                        None if !career => {
                            texts.push((PRICES[i], self.label(0x4287_206F, "Not Installed")));
                            state.scripts.push((PRICES[i], "INIT"));
                        }
                        None => {
                            texts.push((PRICES[i], nfsu2::frontend::grouped(i64::from(part.price))));
                            state.scripts.push((PRICES[i], "INIT"));
                        }
                    }
                }
                for i in p.parts.len()..4 {
                    state.hidden.push(ROWS[i]);
                    state.hidden.push(PRICES[i]);
                }
                // The total (0x0380B632) is set only in a career (0x850078 = 1,
                // 0x5623ED) and while 0x83898C is not 1 (0x5623F9); else
                // 0x4954E0 hides it (0x5624ED). Only level 1's path is traced.
                // With it the discount line (0x562405…0x562485): its label
                // 0x06E0C9A8 "20% Discount:" (0x66559034) at the shop
                // CC_MEGALOW (the visited shop's +0x38, 0x458340), else "10%
                // Discount:" (0x6DA17A25); its amount 0xBE8693E1 the
                // discount 0x5415F0 returns, `%$d` (grouped), or "n/a"
                // (0xA4C02484) when it is 0 (Wine, a fresh 206's Street
                // Package: "10% Discount: 150", "Total Cost: 1,350").
                if career {
                    let (price, discount) = self.package_price(p);
                    let n_a = || self.label(0xA4C0_2484, "n/a");
                    // The label is the format (0x537B90 with no argument):
                    // its "%%" prints "%".
                    let label = if self.shop.visit == Some(MEGALOW) {
                        self.label(0x6655_9034, "20% Discount:")
                    } else {
                        self.label(0x6DA1_7A25, "10% Discount:")
                    }
                    .replace("%%", "%");
                    texts.push((0x06E0_C9A8, label));
                    texts.push((
                        0xBE86_93E1,
                        if discount > 0 { nfsu2::frontend::grouped(i64::from(discount)) } else { n_a() },
                    ));
                    texts.push((TOTAL, if price > 0 { nfsu2::frontend::grouped(i64::from(price)) } else { n_a() }));
                } else {
                    // 0x5624F0…0x562520: 0x4954E0 on 0x06E0C9A8, 0xBE8693E1,
                    // 0x0380B632 and 0x89CA4FDB (under Wine no "Total Cost:"
                    // nor value in Customize).
                    state.hidden.extend([0x06E0_C9A8, 0xBE86_93E1, TOTAL, 0x89CA_4FDB]);
                }
                if open {
                    let mut with = fitted.clone();
                    for part in &p.parts {
                        nfsu2::performance::install(&mut with, &self.game_data.career.packages, part.id);
                    }
                    preview = self.perf_ratings(&with);
                }
            }
            if !open {
                state.hidden.push(CONTENTS);
            }
            extra.push(texts);
            states.push(state);
            previews.push(preview);
        }
        options.push(screens::IconOption { icon: 0x6A93_275F, label: 0xED55_C635, action: Action::PerfStock });
        extra.push(Vec::new());
        states.push(screens::OptionState { hidden: vec![CONTENTS], ..Default::default() });
        let mut stock = fitted.clone();
        for p in self.game_data.career.packages.iter().filter(|p| p.category == category) {
            for part in &p.parts {
                stock.remove(&part.id);
            }
        }
        previews.push(self.perf_ratings(&stock));
        // This package names the option in 0x0666176B.
        for (o, texts) in options.iter().zip(extra.iter_mut()) {
            texts.push((0x0666_176B, self.game_data.assets.strings.get(o.label).unwrap_or("").to_string()));
        }
        let title = nfsu2::performance::CATEGORIES
            .iter()
            .find(|c| c.0 == category)
            .and_then(|c| self.game_data.assets.strings.get(c.2))
            .map(str::to_string);
        let mut menu = IconMenu::new(MenuKind::PerfPackage.package(), title.as_deref(), options, self.now());
        menu.extra = extra;
        menu.option_state = states;
        menu.bars = screens::PACKAGE_BARS
            .iter()
            .enumerate()
            .map(|(k, bar)| {
                let pick = |r: &nfsu2::performance::Ratings| [r.acceleration, r.top_speed, r.handling][k];
                (*bar, previews.iter().map(|r| (pick(&current), pick(r))).collect())
            })
            .collect();
        menu.select_now(selected);
        Mode::Menu(Menu::Icons(menu, MenuKind::PerfPackage))
    }

    /// The performance parts fitted to the career car.
    pub(crate) fn perf_fitted(&self) -> std::collections::BTreeSet<u8> {
        self.profile.as_ref().map(|p| p.perf.clone()).unwrap_or_default()
    }

    /// The fitted parts with one more fitted as 0x5A6330 would (when it
    /// can go on).
    pub(crate) fn perf_with(&self, id: u8) -> std::collections::BTreeSet<u8> {
        let mut fitted = self.perf_fitted();
        nfsu2::performance::install(&mut fitted, &self.game_data.career.packages, id);
        fitted
    }

    /// The dyno results' bars (0x5103D0): the shops' ratings of the car as
    /// fitted.
    pub(crate) fn dyno_ratings(&self) -> [f32; 3] {
        let r = self.perf_ratings(&self.perf_fitted());
        [r.acceleration, r.top_speed, r.handling]
    }

    /// The shops' three ratings for the career car with these parts.
    pub(crate) fn perf_ratings(&self, fitted: &std::collections::BTreeSet<u8>) -> nfsu2::performance::Ratings {
        self.perf_record(fitted).map(|r| nfsu2::performance::ratings(&r, &self.game_data.career.stat_ranges)).unwrap_or_default()
    }

    /// What a package costs (0x5415F0): its parts neither fitted nor bought
    /// (0x540FF0), 10% off when there are more than one (0x541050); and
    /// the discount.
    pub(crate) fn package_price(&self, package: &nfsu2::performance::Package) -> (u32, u32) {
        let p = self.profile.as_ref();
        let due: Vec<u32> = package
            .parts
            .iter()
            .filter(|x| !p.is_some_and(|p| p.perf.contains(&x.id) || p.perf_owned.contains(&x.id)))
            .map(|x| x.price)
            .collect();
        let cost: u32 = due.iter().sum();
        if due.len() > 1 {
            // 80% at the shop CC_MEGALOW (0x5415F0 tests the visited
            // shop's +0x38 hash).
            let percent = if self.shop.visit == Some(MEGALOW) { 80 } else { 90 };
            let price = cost * percent / 100;
            (price, cost - price)
        } else {
            (cost, 0)
        }
    }

    /// UI_BuyPerformanceParts' list and readouts for the shop's category,
    /// row `selected` highlighted (0x5627A0, 0x562540): the parts of the
    /// category's level 1–3 packages in order, each when bought or when
    /// its level is unlocked in the shop (0x52A0A0 with 0x50F0D0's flags
    /// 6: 0x5155A0); the peaks of the car as it is (a copy, 0x413440) and
    /// with the highlighted part fitted (0x5213E0).
    pub(crate) fn perf_parts_list(&self, selected: usize) -> screens::PerfParts {
        let category = self.shop.perf_category;
        let fitted = self.perf_fitted();
        let owned = |id: u8| self.profile.as_ref().is_some_and(|p| p.perf_owned.contains(&id));
        // Outside a career (0x850078 ≠ 1) nothing reads as bought: a row
        // not fitted shows "Not Installed" (0x53B859, in 0x53B7C0).
        let career = self.profile.as_ref().is_some_and(|p| !p.scratch);
        let mut rows = Vec::new();
        for level in 1..4u8 {
            let Some(package) = self.package(category, level) else { continue };
            // 0x50F0D0's flags: 2 in the garage's storage (only what is
            // owned), 6 in a shop.
            let unlocked = !self.shop.garage_parts && self.perf_unlocked(category, level);
            for part in package.parts.iter().take(4) {
                if owned(part.id) || unlocked {
                    let bought = career && owned(part.id);
                    rows.push(screens::PerfRow {
                        id: part.id,
                        name: part.name,
                        price: part.price,
                        installed: fitted.contains(&part.id),
                        owned: bought,
                        career,
                    });
                }
            }
        }
        let selected = selected.min(rows.len().saturating_sub(1));
        let peaks = |f: &std::collections::BTreeSet<u8>| {
            self.perf_record(f).map_or((0.0, 0.0), |r| nfsu2::performance::peaks(&r))
        };
        let current = peaks(&fitted);
        let preview = rows.get(selected).map_or(current, |r| peaks(&self.perf_with(r.id)));
        let ranges = &self.game_data.career.stat_ranges;
        let bars = |(t, p): (f32, f32)| (nfsu2::performance::bar(ranges, 4, t), nfsu2::performance::bar(ranges, 6, p));
        let mut list = screens::PerfParts {
            rows,
            selected,
            current,
            preview,
            current_bars: bars(current),
            preview_bars: bars(preview),
            metric: self.settings.metric,
            top: 0,
        };
        list.keep_in_view();
        list
    }

    /// Accept on UI_BuyPerformanceParts (the row's 0x55A240): a fitted part
    /// says so (0xDA15D7E5); one not bought asks to buy it (0x44B7167F,
    /// or 0x3949ABEE when it is worse than a part it replaces) unless the
    /// bank is short (0xD1AC60BE); a bought one asks to fit it
    /// (0xF5AC2B2A / 0xF1BD1A8A). "Worse" (0x521C20): the first fitted
    /// part it stands in for is of a higher package level.
    pub(crate) fn perf_part_accept(&mut self) -> Mode {
        let menu = std::mem::replace(&mut self.mode, Mode::Explore);
        let Some(row) = self.shop.perf_parts.as_ref().and_then(|l| l.row()).cloned() else { return menu };
        let worse = self.perf_worse(row.id);
        // Every answer here (notices and questions) ends in "Dim" on
        // UI_ChoosePerformancePackage's handler (0x55A3DE; no event: the
        // screens stay as they are under it).
        if row.installed {
            return dialog_over(
                self.dialog(&self.label(0xDA15_D7E5, "The part you are trying to apply is already installed!"), menu),
                true,
                None,
            );
        }
        // Outside a career (0x55A2AE: 0x850078 ≠ 1) a part is never
        // bought: no bank test, the install question, 0x51123592.
        let career = self.profile.as_ref().is_some_and(|p| !p.scratch);
        let owned = row.owned || !career;
        let cash = self.profile.as_ref().map_or(0, |p| p.cash);
        if !owned && cash < row.price {
            return dialog_over(
                self.dialog(&self.label(0xD1AC_60BE, "You do not have enough bank to purchase this item."), menu),
                true,
                None,
            );
        }
        self.shop.pending_perf = Some(PerfPending::Part(row.id, !owned));
        let question = match (owned, worse) {
            (false, false) => 0x44B7_167F,
            (false, true) => 0x3949_ABEE,
            (true, false) => 0xF5AC_2B2A,
            (true, true) => 0xF1BD_1A8A,
        };
        let yes = self.perf_package_menu(0);
        dialog_over(self.ask(question, yes, menu), true, None)
    }

    /// Accept on a package (0x559F90): one fully fitted says so
    /// (0xB84A37FA); one not fully bought asks to buy and fit it
    /// (0x94F4C140, or 0xFDDD0C23 when a part of it is worse than one it
    /// replaces, 0x5303D0) unless the bank is short of its price
    /// (0xD1AC60BE); a bought one asks to fit it (0xF2D7FFAB / 0x0217453F).
    pub(crate) fn perf_package_accept(&mut self, level: u8) -> Mode {
        let back = self.perf_package_menu(level as usize);
        let Some(package) = self.package(self.shop.perf_category, level).cloned() else { return back };
        let fitted = self.perf_fitted();
        // StartDim after each (0x55A139; no event: unchanged under it).
        if package.parts.iter().all(|x| fitted.contains(&x.id)) {
            return dialog_over(
                self.dialog(&self.label(0xB84A_37FA, "This performance package is already installed!"), back),
                true,
                None,
            );
        }
        // Outside a career (0x55A03F) a package is never bought either.
        let owned = package
            .parts
            .iter()
            .all(|x| self.profile.as_ref().is_some_and(|p| p.scratch || p.perf_owned.contains(&x.id)));
        let worse = package.parts.iter().any(|x| self.perf_worse(x.id));
        if !owned {
            let (price, _) = self.package_price(&package);
            if self.profile.as_ref().map_or(0, |p| p.cash) < price {
                return dialog_over(
                    self.dialog(&self.label(0xD1AC_60BE, "You do not have enough bank to purchase this item."), back),
                    true,
                    None,
                );
            }
        }
        self.shop.pending_perf = Some(PerfPending::Package(level, !owned));
        let question = match (owned, worse) {
            (false, false) => 0x94F4_C140,
            (false, true) => 0xFDDD_0C23,
            (true, false) => 0xF2D7_FFAB,
            (true, true) => 0x0217_453F,
        };
        let yes = self.perf_package_menu(level as usize);
        dialog_over(self.ask(question, yes, back), true, None)
    }

    /// Accept on Return To Stock (0x55A150): with nothing of the category
    /// fitted, "already stock" (0x89D4E03F); else "Re-Install Stock
    /// Parts?" (0x5D336F96).
    pub(crate) fn perf_stock_accept(&mut self) -> Mode {
        let back = self.perf_package_menu(self.perf_levels() as usize);
        let category = self.shop.perf_category;
        let fitted = self.perf_fitted();
        let any = self
            .game_data.career
            .packages
            .iter()
            .filter(|p| p.category == category)
            .flat_map(|p| p.parts.iter())
            .any(|x| fitted.contains(&x.id));
        // StartDim after each (0x55A17F; no event).
        if !any {
            return dialog_over(
                self.dialog(&self.label(0x89D4_E03F, "This performance category is already stock!"), back),
                true,
                None,
            );
        }
        self.shop.pending_perf = Some(PerfPending::Stock);
        let yes = self.perf_package_menu(self.perf_levels() as usize);
        dialog_over(self.ask(0x5D33_6F96, yes, back), true, None)
    }

    /// Whether a part is worse than the part it replaces (0x521C20): the
    /// first of the parts it stands in for that is fitted is of a higher
    /// package level.
    pub(crate) fn perf_worse(&self, id: u8) -> bool {
        let packages = &self.game_data.career.packages;
        let level = |id: u8| nfsu2::performance::part(packages, id).map_or(0, |(p, _)| p.level);
        let fitted = self.perf_fitted();
        nfsu2::performance::part(packages, id)
            .and_then(|(_, part)| part.replaces.iter().find(|r| fitted.contains(r)))
            .is_some_and(|&r| level(id) < level(r))
    }

    /// Yes on a performance shop question. A part (0x9F00F07 / 0x51123592):
    /// bought first unless owned (0x521B10: the price from the bank, marked
    /// bought by 0x503E10), then fitted (0x5213E0 → 0x5A6330). A package
    /// (0x54FE60, 0x5501F0): bought first unless owned (its price, 0x5415F0,
    /// from the bank and its parts marked bought), then each part fitted in
    /// turn (0x521350). Stock (0xE75749D4 → 0x521A30): the category's parts
    /// come off. Parts and packages with brands go through
    /// UI_PerformanceBrandSelect first (0x422F7587 → 0x520930); that screen
    /// is not in the clone.
    /// Yes on buying (0x9F00F07 for a part, 0x5501F0 for a package) opens
    /// UI_PerformanceBrandSelect (0x55DDB0 with 0x422F7587) when a part
    /// has brands (+0x0C), else buys at once (0x521B10 + 0x5213E0); see
    // prov: inferred game.brand_select
    /// [`options::BrandSelect`]. **Inferred**: a package without any
    /// branded part buys at once too.
    pub(crate) fn brand_select(&self, pending: PerfPending, now: f32) -> Option<options::BrandSelect> {
        let packages = &self.game_data.career.packages;
        let (parts, single): (Vec<&nfsu2::performance::PerfPart>, bool) = match pending {
            PerfPending::Part(id, true) => {
                (nfsu2::performance::part(packages, id).map(|(_, x)| x).into_iter().collect(), true)
            }
            PerfPending::Package(level, true) => (
                self.package(self.shop.perf_category, level).map(|p| p.parts.iter().take(4).collect()).unwrap_or_default(),
                false,
            ),
            _ => return None,
        };
        let rows: Vec<(u32, Vec<u32>, usize)> =
            parts.into_iter().filter(|x| !x.brands.is_empty()).map(|x| (x.name, x.brands.clone(), 0)).collect();
        (!rows.is_empty()).then(|| options::BrandSelect::new(rows, single, now))
    }

    /// `from_brand`: settled by UI_PerformanceBrandSelect's Select
    /// (0xE1FDE1D1 → 0x55BD70, back to a new package screen: Choose
    /// Individual Parts and the parts list's first row; Wine, a part and a
    /// package); else straight from the question, the screen kept as it
    /// was (Wine: the bought unbranded part's row stays highlighted).
    pub(crate) fn perf_confirm(&mut self, pending: PerfPending, from_brand: bool) -> Mode {
        let packages = self.game_data.career.packages.clone();
        let category = self.shop.perf_category;
        let price = match pending {
            PerfPending::Package(level, true) => self.package(category, level).map_or(0, |p| self.package_price(p).0),
            _ => 0,
        };
        let selected = match pending {
            _ if from_brand => 0,
            PerfPending::Part(..) => 0,
            PerfPending::Package(level, _) => level as usize,
            PerfPending::Stock => self.perf_levels() as usize,
        };
        let slots = &mut self.shop.perf_brand_slots;
        let db = &self.game_data.parts;
        if let Some(p) = self.profile.as_mut() {
            let before = p.perf.clone();
            match pending {
                // 0x521B10: the price from the bank, the part marked bought
                // with slot 0's brand (0x503E10), its price added to the
                // car's spend (+0x7F0), slot 0 then 0xFFFFFFFF.
                PerfPending::Part(id, buy) => {
                    if buy {
                        let cost = nfsu2::performance::part(&packages, id).map_or(0, |(_, x)| x.price);
                        p.cash = p.cash.saturating_sub(cost);
                        p.perf_owned.insert(id);
                        p.add_brand(slots[0]);
                        p.spent = p.spent.wrapping_add(cost);
                        slots[0] = u32::MAX;
                    }
                    nfsu2::performance::install(&mut p.perf, &packages, id);
                }
                PerfPending::Package(level, buy) => {
                    let parts: Vec<u8> = packages
                        .iter()
                        .find(|x| x.category == category && x.level == level)
                        .map(|x| x.parts.iter().map(|y| y.id).collect())
                        .unwrap_or_default();
                    if buy {
                        p.cash = p.cash.saturating_sub(price);
                        // 0x54FE60: each unowned part (0x530360) marked
                        // bought with slot i's brand and its full price
                        // added to the spend; every slot read is reset.
                        for (i, &id) in parts.iter().enumerate() {
                            if p.perf_owned.insert(id) {
                                p.add_brand(slots.get(i).copied().unwrap_or(0));
                                p.spent = p
                                    .spent
                                    .wrapping_add(nfsu2::performance::part(&packages, id).map_or(0, |(_, x)| x.price));
                            }
                            if let Some(s) = slots.get_mut(i) {
                                *s = u32::MAX;
                            }
                        }
                    }
                    for id in parts {
                        nfsu2::performance::install(&mut p.perf, &packages, id);
                    }
                }
                PerfPending::Stock => {
                    for x in packages.iter().filter(|x| x.category == category) {
                        for part in &x.parts {
                            p.perf.remove(&part.id);
                        }
                    }
                }
            }
            // 0x5213E0 / 0x521350 / 0x521A30 end in 0x639CF0(1, …) and
            // 0x503950: the ride refreshed and stored into the car record.
            p.store_ride(db, &packages, &before);
            let _ = p.commit();
        }
        self.drop_drive();
        let (at, top) = self.shop.perf_parts.as_ref().map_or((0, 0), |l| (l.selected, l.top));
        let mut list = if from_brand { self.perf_parts_list(0) } else { self.perf_parts_list(at) };
        if !from_brand {
            list.top = top;
            list.keep_in_view();
        }
        self.shop.perf_parts = Some(list);
        self.perf_package_menu(selected)
    }
}
