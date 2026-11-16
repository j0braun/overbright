use crate::*;

impl Game {
    /// The milestones whose covers the career has earned, magazines or
    /// DVDs (0x523F70: the reward records of kind 1 / 3 or 2 / 4 marked
    /// reached), in record order.
    pub(crate) fn earned_covers(&self, dvd: bool) -> Vec<usize> {
        let Some(p) = self.profile.as_ref() else { return Vec::new() };
        self.game_data.career
            .milestones
            .iter()
            .enumerate()
            .filter(|(_, m)| m.is_dvd() == dvd && matches!(m.kind, 1..=4) && p.milestones.contains(&m.hash))
            .map(|(i, _)| i)
            .collect()
    }

    /// UI_MagazineSelect (screen 0x55B530 → 0x557F30, items 0x523F70,
    /// selection 0x54B160): the earned covers on the icon strip, every one
    /// the magazine or DVD icon, its option title the cover's
    /// `<cover>_TITLE`; the title "Magazines" / "DVD covers", "mag name"
    /// the title again and "mag body" `<cover>_BODY` (how it was earned),
    /// the select button "View Magazine" / "View DVD". MagThumb shows the cover's logo.
    pub(crate) fn covers_menu(&self, dvd: bool, selected: usize) -> Mode {
        use nfsu2::formats::hash::bin_hash_from;
        let icon = if dvd { screens::DVD_ICON } else { screens::MAGAZINE_ICON };
        let view = self.label(if dvd { 0xBB09_5CA1 } else { 0x2465_36EF }, "");
        let mut options = Vec::new();
        let mut extra = Vec::new();
        for i in self.earned_covers(dvd) {
            let cover = self.game_data.career.milestones[i].cover;
            let title = bin_hash_from(cover, "_TITLE");
            options.push(screens::IconOption { icon, label: title, action: Action::Cover(i) });
            extra.push(vec![
                (0x9E30_2291, self.label(title, "")),
                (0x3554_DEB1, self.label(bin_hash_from(cover, "_BODY"), "")),
                (0x010C_BA23, view.clone()),
            ]);
        }
        let title = self.label(if dvd { 0x653B_26C4 } else { 0xF733_3312 }, "");
        let mut menu = IconMenu::new(MenuKind::Covers(dvd).package(), Some(&title), options, self.now());
        menu.extra = extra;
        // MagThumb: the constructor (0x550DE0) hides it; each tick
        // (0xC98356BA, 0x54B2A2) gives it the selected item's cover (the
        // milestone's +0x28, a FrontB texture `MAGAZINE_SETUP_<MAG>`,
        // 128 × 32) and shows it (0x495500). Wine menus113 v2: the
        // "BAYVIEW'S BEST" logo in the right panel.
        let thumb = nfsu2::formats::hash::fe_hash("MagThumb");
        menu.option_state = self
            .earned_covers(dvd)
            .into_iter()
            .map(|i| screens::OptionState {
                shown: vec![thumb],
                images: vec![(thumb, self.game_data.career.milestones[i].cover)],
                ..Default::default()
            })
            .collect();
        menu.select_now(selected);
        Mode::Menu(Menu::Icons(menu, MenuKind::Covers(dvd)))
    }
}

/// The showcase (Show Off) and the magazine cover shots (were `Game`
/// fields). Every field starts empty (`Default`).
#[derive(Default)]
pub(crate) struct Showcase {
    /// MAGAZINES_SHOWCASE.BIN's camera set (0x499EC0 → 0x838574).
    pub(crate) cameras: Option<nfsu2::formats::magazines::Showcase>,
    /// The player car's hood, trunk and doors (0x61F830 on its render info
    /// +0x554), which Show Off moves.
    pub(crate) parts: nfsu2::part_anim::Channels,
    /// Show Off's open flags (+0xE9..+0xEB) the leave (0x499FA0 → 0x499F10)
    /// still has to shut.
    pub(crate) shut: [bool; 3],
    /// The career car posed for UI_MagazineView's shot, its renderer and
    /// the shot's texture.
    pub(crate) cover_car: Option<CoverShot>,
    /// A loaded cover picture (512² RGBA) waiting to be bound under
    /// `CoverView::shot()` (`Game::open_cover`).
    pub(crate) cover_picture: Option<Vec<u8>>,
}
