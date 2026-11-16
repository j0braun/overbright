use crate::*;

impl Game {
    /// UI_Sponsorship_new (screen 0x495D30): the stage's five sponsors on
    /// the icon strip, each DECAL_ICON_<name>; for the selected one the
    /// title SPONSOR_<name>, the signing bonus (+0xC), the stage's URL
    /// bonus (the first URL event's purse, 0x500A40), the sponsor-race
    /// purse (+2) and SPONSOR_INFO_<name>.
    pub(crate) fn sponsor_menu(&self) -> Mode {
        // The sponsors on offer (0x500670's test).
        let sponsors: Vec<career::Sponsor> = self
            .profile
            .as_ref()
            .map_or_else(Vec::new, |p| p.sponsor_offers(&self.game_data.career).into_iter().cloned().collect());
        self.sponsor_screen(MenuKind::Sponsor, &sponsors, Action::Sign)
    }

    /// UI_RewardsSponsor (screen 0x4ED840 → 0x4DF6F0): the sponsor signed
    /// in each stage up to the current one, in stage order (0x4AFD00, each
    /// DECAL_ICON_<name>), shown as the sponsorship screen shows its offers
    /// (the same 0x495D30); its options do nothing (0x495C40).
    pub(crate) fn signed_sponsors_menu(&self) -> Mode {
        let sponsors: Vec<career::Sponsor> = self.profile.as_ref().map_or_else(Vec::new, |p| {
            (0..=p.stage)
                .filter_map(|stage| p.sponsors.get(&stage).and_then(|h| self.game_data.career.sponsors.get(h)).cloned())
                .collect()
        });
        self.sponsor_screen(MenuKind::SignedSponsors, &sponsors, |_| Action::Inert)
    }

    /// A sponsor screen (0x495D30): the URL bonus is the current stage's.
    pub(crate) fn sponsor_screen(
        &self,
        kind: MenuKind,
        sponsors: &[career::Sponsor],
        action: impl Fn(usize) -> Action,
    ) -> Mode {
        let stage = self.profile.as_ref().map_or(1, |p| p.stage);
        let url_bonus = self
            .game_data.career
            .events
            .iter()
            .find(|e| e.stage == stage && e.category == career::Category::Url)
            .map_or(0, |e| e.reward);
        let hash = |s: &str| nfsu2::formats::hash::bin_hash(s);
        let mut options = Vec::new();
        let mut extra = Vec::new();
        for (i, s) in sponsors.iter().enumerate() {
            let icon = self.game_data.assets.texture_for(&format!("DECAL_ICON_{}", s.name)).unwrap_or(0);
            options.push(screens::IconOption { icon, label: hash(&format!("SPONSOR_{}", s.name)), action: action(i) });
            extra.push(vec![
                (0xF13F_76B4, self.label(hash(&format!("SPONSOR_{}", s.name)), &s.name)),
                (0xF25B_6BBA, s.bonus.to_string()),
                (0x7D8E_1791, url_bonus.to_string()),
                (0x195D_8D22, s.purse.to_string()),
                (0xF13C_EF3F, self.label(hash(&format!("SPONSOR_INFO_{}", s.name)), "")),
            ]);
        }
        // The contract terms: SPONSOR_DETAILS_<name> broken into the ten
        // TERMS_1..TERMS_10 rows (0x4DF7C9) at the rows' width (0x520080).
        let rows: Vec<u32> = (1..=10).map(|i| nfsu2::formats::hash::fe_hash(&format!("TERMS_{i}"))).collect();
        let package = self.game_data.assets.package(kind.package());
        let row = package.and_then(|p| p.objects.iter().find(|o| o.hash == rows[0]));
        let font =
            row.and_then(|o| package?.resources.get(o.resource as usize)).and_then(|r| self.game_data.assets.font_for(&r.name));
        let width = row.map_or(0.0, |o| o.max_width as f32 - 16.0);
        let terms = sponsors
            .iter()
            .map(|s| {
                let text = self.label(hash(&format!("SPONSOR_DETAILS_{}", s.name)), "");
                match font {
                    Some(f) if width > 0.0 => nfsu2::frontend::wrap(f, &text, width),
                    _ => text.replace('^', "\n"),
                }
                .split('\n')
                .map(str::to_string)
                .collect()
            })
            .collect();
        let mut menu = IconMenu::new(kind.package(), None, options, self.now());
        menu.extra = extra;
        menu.single = true;
        menu.list = Some((rows, terms));
        Mode::Menu(Menu::Icons(menu, kind))
    }
}
