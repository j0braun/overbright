use crate::*;

impl Game {
    /// The shops' Visual Rating widget over the screens 0x55BCA0 shows it
    /// on (`status::shop_rating_quads`): the car's rating and the
    /// preview's, the car with the highlighted part or colour (the shop
    /// screens fit it on the preview ride, 0x55C180 / 0x55CC60, and refresh
    /// the widget, 0x55B1B0).
    pub(crate) fn shop_rating_quads(&self, now: f32) -> Vec<Quad> {
        let Mode::Menu(menu) = &self.mode else { return Vec::new() };
        self.shop_rating_quads_of(menu, now)
    }

    /// The widget over `menu` (also the screen under a dialog: its own
    /// package stays up, Wine menus103 vinyl_ask / vinyl_colour_ask).
    pub(crate) fn shop_rating_quads_of(&self, menu: &Menu, now: f32) -> Vec<Quad> {
        if !self.rating_up_update(menu) {
            return Vec::new();
        }
        let preview = self.profile.as_ref().map_or(0.0, |p| self.visual_rating_of(&self.preview_ride(&p.installed)));
        nfsu2::frontend::status::shop_rating_quads(&self.game_data.assets, self.visual_rating(), preview, now)
    }

    /// Whether the widget is up over `menu`, and
    /// UI_Menu_Asset_Reputation pushed (`FeState::rating_up`) when it is.
    /// Called from `shop_rating_quads_of`.
    pub(crate) fn rating_up_update(&self, menu: &Menu) -> bool {
        // 0x55BCA0 pushes the widget for UI_GenericParts_Browser,
        // UI_ChoosePaintCategory, UI_Paint, UI_DecalMain, UI_ChooseSpinner,
        // UI_Rims_Browser, UI_DecalsOverlay; under Wine none on UI_ICEMAIN
        // (Custom Gauges). The neon / ICE parts lists push it on a step
        // (`overlay_rating`).
        // The trunk layout's own screen (mode 2, opened with its overlay
        // up) shows it at once (Wine, the finished-career save's stock trunk: 2.74;
        // prov: inferred game.shop_rating_quads_of
        // **inferred**: the overlay's opening refresh pushes it).
        // UI_IcePartsOverlay's lists (the specialties' and the trunk
        // audio's, modes 2–5; not Custom Gauges' UI_CustomHUDOverlay) show
        // it from their opening on and drop it as they close (Wine, the
        // finished-career save: Window Tint on DARK BLACK, the trunk layout list in
        // modes 2 and 3, a component slot's list; none on the strips after
        // Back).
        let trunk = matches!(menu, Menu::Parts(b) if b.package == "UI_ICEMAIN" && b.part.is_some() && b.categories.get(b.category).is_some_and(|c| c.slot != 0xA3));
        let parts = trunk
            || matches!(menu, Menu::Parts(b) if matches!(b.package, "UI_GenericParts_Browser" | "UI_DecalMain" | "UI_ChooseSpinner" | "UI_Rims_Browser") || Some(b.package) == self.fe.overlay_rating);
        if !(parts || matches!(menu, Menu::Paint(_) | Menu::Icons(_, MenuKind::Graphics))) {
            return false;
        }
        // The body shop's strip (no list open) is a package 0x55BCA0 keeps
        // the widget on, but nothing pushes it there.
        // The graphics shop's categories the same (Wine, IND_GRAPHICS: no
        // widget on entering).
        // Only the shop's own first screen: any other screen is reached by
        // a screen change, which pushes it (0x55BD70 / 0x55BE00 → 0x55BCA0;
        // Wine: up on the vinyl groups after Choose Vinyl Layer).
        let strip = matches!(menu, Menu::Parts(b) if b.package == "UI_GenericParts_Browser" && b.part.is_none() && b.sub.is_none())
            || matches!(menu, Menu::Icons(_, MenuKind::Graphics));
        if strip && !self.fe.rating_up.get() {
            return false;
        }
        self.fe.rating_up.set(true);
        true
    }

    /// The parts the FE car shows (its shown ride +0x50) on the shop
    /// screens: a parts browser in a category, and the rims' style strip,
    /// show a copy of the car as fitted (+0x940, 0x413440) with the
    /// highlighted part (0x55C180: slot 0x1D also fits 0x1E; 0x55F350);
    /// elsewhere the ride is the car with the graphics setup (0x55C470,
    /// called on leaving a category and the paint slots: the copy, then
    /// each setup item fitted, a vinyl over a fitted unique clearing the
    /// four layers first), UI_Paint adding its highlighted swatch
    /// (0x55C200, on the ride as it is). The trunk layout also empties the
    // prov: open game.preview_ride
    /// component slots (0x547480). **Open**: the vinyl colours' swatches.
    pub(crate) fn preview_ride(
        &self,
        installed: &std::collections::BTreeMap<u8, u32>,
    ) -> std::collections::BTreeMap<u8, u32> {
        self.preview_ride_with(installed, self.shop_preview_item())
    }

    /// [`Self::preview_ride`] with `item` highlighted (the garage's
    /// prefetch of the list's neighbours, `shop_preview_neighbours`).
    pub(crate) fn preview_ride_with(
        &self,
        installed: &std::collections::BTreeMap<u8, u32>,
        item: Option<(u8, u32)>,
    ) -> std::collections::BTreeMap<u8, u32> {
        // Every part the preview fits goes through the ride's set-part
        // 0x631D30, which runs 0x61BCE0 again (0x631D64): the derived
        // slots follow the previewed part — a split hood's HOODUNDER
        // (slot 0x19, the black HOOD_UNDER_LEFT / _RIGHT halves posed with
        // it), a wide body's doors.
        let car = self.profile.as_ref().map(|p| p.car.clone()).unwrap_or_default();
        let ride = self.preview_ride_parts(installed, item);
        if car.is_empty() {
            return ride;
        }
        nfsu2::car::with_derived_slots(&ride, &self.game_data.parts, &car)
    }

    pub(crate) fn preview_ride_parts(
        &self,
        installed: &std::collections::BTreeMap<u8, u32>,
        item: Option<(u8, u32)>,
    ) -> std::collections::BTreeMap<u8, u32> {
        let mut ride = installed.clone();
        // The graphics shop's vinyl lists (the browser's +0x1D7: 0x55EDE0
        // and 0x55EBB0 call 0x55C380(layer 0x8389A8, part), not 0x55C180)
        // fit the vinyl on the ride as it is — the car with the setup
        // (0x55C470 on leaving a category) — after clearing the four
        // layers when a vinyl on the ride, a setup vinyl or the highlighted
        // one is a unique (attribute 0x564B8CB6 = 0x40: 0x50EF10, 0x521590,
        // 0x61B510), else fitting the setup's vinyls and their colours
        // prov: inferred game.preview_ride_parts
        // (0x40–0x43, 0x4D–0x5C) again. **Inferred**: the third test's
        // part (0x61B510's this) is the highlighted vinyl.
        let vinyl = matches!(self.preview_menu(), Some(Menu::Parts(b)) if b.package == "UI_GenericParts_Browser" && b.sub.is_some_and(|l| graphics::VINYL_LAYERS.contains(&l)));
        let browsing = matches!(self.preview_menu(), Some(Menu::Parts(_))) && item.is_some() && !vinyl;
        if !browsing {
            let uniques: HashSet<u32> = self
                .shop.setup
                .items
                .iter()
                .map(|i| i.1)
                .chain(graphics::VINYL_LAYERS.iter().filter_map(|&l| installed.get(&l).copied()))
                .filter(|&h| self.unique_vinyl(h))
                .collect();
            self.shop.setup.apply(&mut ride, &|h| uniques.contains(&h));
        }
        if vinyl && let Some((_, hash)) = item {
            let unique = graphics::VINYL_LAYERS.iter().filter_map(|l| ride.get(l)).any(|&h| self.unique_vinyl(h))
                || self
                    .shop.setup
                    .items
                    .iter()
                    .any(|&(s, h)| graphics::VINYL_LAYERS.contains(&s) && h != 0 && self.unique_vinyl(h))
                || (hash != 0 && self.unique_vinyl(hash));
            if unique {
                for l in graphics::VINYL_LAYERS {
                    ride.remove(&l);
                }
            }
        }
        if let Some((slot, hash)) = item {
            if hash == 0 {
                ride.remove(&slot);
                return ride;
            }
            ride.insert(slot, hash);
            if slot == 0x1D {
                ride.insert(0x1E, hash);
            }
            // 0x55C180 → 0x547480 for the trunk layout: each component slot
            // 0x23–0x2E takes 0x53AEB0's part, the slot's first part of
            // prov: inferred game.preview_ride_parts-2
            // level 0 that 0x52A020 passes (**inferred**: every level-0
            // part passes), or none (0x631D30(slot, 0)). Under Wine (the
            // finished-career save's 350Z) the layout list shows the trunk without
            // its components, also on the fitted layout.
            if slot == 0x22 {
                let car = self.profile.as_ref().map(|p| p.car.clone()).unwrap_or_default();
                for s in 0x23u8..=0x2E {
                    match nfsu2::car::trunk_component(&self.game_data.parts, &car, s as usize) {
                        Some(h) => ride.insert(s, h),
                        None => ride.remove(&s),
                    };
                }
            }
        }
        ride
    }

    /// The highlighted item a shop screen fits on the preview ride (the FE
    /// car's +0x50): a parts browser's part in a category (0x55C180), the
    /// rim style on UI_Rims_Browser's strip (0x55F350), UI_Paint's
    /// highlighted swatch on its slot (0x55C200).
    /// The shop screen the preview ride follows: the mode's menu, or the
    /// screen a dialog was pushed over (it stays up under it: under Wine,
    /// menus14, the vinyl question keeps the list's preview, 0.04 on the
    /// Tear Set vinyl as in the list).
    pub(crate) fn preview_menu(&self) -> Option<&Menu> {
        match &self.mode {
            Mode::Menu(Menu::Dialog(d, outcomes)) => match d.under.and_then(|i| outcomes.get(i)) {
                Some(Mode::Menu(m)) => Some(m),
                _ => None,
            },
            Mode::Menu(m) => Some(m),
            _ => None,
        }
    }

    pub(crate) fn shop_preview_item(&self) -> Option<(u8, u32)> {
        let menu = self.preview_menu()?;
        match menu {
            // The neon list previews its highlighted colour on the car
            // prov: inferred game.shop_preview_item
            // (under Wine RED's underglow; **inferred** that it is
            // 0x55C180 from 0x560790 / 0x560930).
            // UI_IcePartsOverlay (the specialties' and trunk audio's
            // lists on UI_ICEMAIN) fits the highlighted part too: on
            // entering a list (0x5606F0, when the item has a part) and on
            // each step (PartsOverlay_Step 0x560930), both 0x55C180. Not
            // the Custom Gauges list (UI_CustomHUDOverlay, slot 0xA3).
            Menu::Parts(b)
                if b.package == "UI_ICEMAIN" && b.categories.get(b.category).is_some_and(|c| c.slot == 0xA3) =>
            {
                None
            }
            Menu::Parts(b)
                if matches!(
                    b.package,
                    "UI_GenericParts_Browser"
                        | "UI_Rims_Browser"
                        | "UI_ChooseSpinner"
                        | "UI_DecalMain"
                        | "UI_CustomNeonMain"
                        | "UI_ICEMAIN"
                ) =>
            {
                let category = b.categories.get(b.category).filter(|c| c.action == screens::CategoryAction::Parts);
                // UI_Rims_Browser previews the highlighted style already
                // on its strip (0x55F350 fits a rim on the preview ride,
                // 0x631D30, and refreshes the widget; under Wine "0.15" in
                // green on 5Zigen's first style). UI_ChooseSpinner's strip
                // does the same (its handler 0x569680 on a style change,
                // +0x214 clear: the fitted rim, +0x20C, when the style is
                // its name's (0x43DB50(0x610010)), else the style's first
                // size; 0x631D30 on 0x1D and 0x1E of the car's copy, then
                // prov: inferred game.shop_preview_item-2
                // 0x4C2100(0x1D)). **Inferred**: the rims' strip picks the
                // size the same way.
                let strip = matches!(b.package, "UI_Rims_Browser" | "UI_ChooseSpinner");
                let part = b
                    .part
                    .or_else(|| strip.then(|| category.map_or(0, |c| c.parts.iter().position(|p| p.4).unwrap_or(0))));
                part.zip(category).and_then(|(i, c)| c.parts.get(i).map(|p| (c.slot, p.0)))
            }
            // Both modes: the paints (0x55CC60 / 0x55D2E0) and the vinyl
            // colours (0x55D2E0 → 0x55C200 on the ride with the setup).
            Menu::Paint(p) => {
                let part_slot = p.slots().get(p.slot).map(|s| s.0);
                p.selected.and_then(|i| p.swatches.get(i)).zip(part_slot).map(|(s, slot)| (slot, s.0))
            }
            _ => None,
        }
    }

    /// The items a step up or down the shop list would highlight
    /// next ([`Self::shop_preview_item`]'s neighbours: a parts browser's
    /// list, UI_Paint's swatches), for the garage to build ahead of the
    /// step (`draw_garage`'s prefetch). Nothing for the strips and lists
    /// without a highlighted entry.
    pub(crate) fn shop_preview_neighbours(&self) -> Vec<(u8, u32)> {
        let around = |i: usize, n: usize| -> Vec<usize> {
            if n < 2 {
                return Vec::new();
            }
            let mut v = vec![(i + 1) % n, (i + n - 1) % n];
            v.dedup();
            v.retain(|&j| j != i);
            v
        };
        match self.preview_menu() {
            Some(Menu::Parts(b))
                if b.package == "UI_ICEMAIN" && b.categories.get(b.category).is_some_and(|c| c.slot == 0xA3) =>
            {
                Vec::new()
            }
            Some(Menu::Parts(b))
                if matches!(
                    b.package,
                    "UI_GenericParts_Browser" | "UI_DecalMain" | "UI_CustomNeonMain" | "UI_ICEMAIN"
                ) =>
            {
                let Some(c) = b.categories.get(b.category).filter(|c| c.action == screens::CategoryAction::Parts)
                else {
                    return Vec::new();
                };
                let Some(i) = b.part else { return Vec::new() };
                around(i, c.parts.len()).into_iter().map(|j| (c.slot, c.parts[j].0)).collect()
            }
            Some(Menu::Paint(p)) => {
                let (Some(i), Some(slot)) = (p.selected, p.slots().get(p.slot).map(|s| s.0)) else { return Vec::new() };
                around(i, p.swatches.len()).into_iter().map(|j| (slot, p.swatches[j].0)).collect()
            }
            _ => Vec::new(),
        }
    }
}
