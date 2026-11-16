use crate::*;

impl Game {
    pub(super) fn status_key(&mut self, key: Key) -> Result<()> {
        let Mode::Menu(Menu::Status(st)) = &mut self.mode else { unreachable!() };
        let rows = st.rows.len();
        if st.screen.key(key, rows) {
            // 0x4B1F20: Select / Accept / Back play out, then
            // 0xC98356BA resumes the game (0x5F4250(1)).
            self.mode = Mode::Explore;
        }
        Ok(())
    }

    pub(super) fn in_game_map_key(&mut self, key: Key) -> Result<()> {
        let Mode::Menu(Menu::InGameMap(map)) = &mut self.mode else { unreachable!() };
        // Back (0x911AB364) closes it; the game resumes the mode
        // 0x60B3B0 saved (free roam).
        self.career_world.map_hidden = map.hidden;
        if let Some(p) = self.profile.as_mut() {
            p.map_hidden = map.hidden;
        }
        if key == Key::Back {
            self.mode = Mode::Explore;
            return Ok(());
        }
        // C (BUTTON1, 0xC519BFC0 → 0x4B10C0): with the GPS off
        // and an icon selected (widget +0x14), "Are you sure you
        // want to engage the GPS system?" (0x63477036, Yes 0xF770
        // / No 0x061C, 0x5587A0); with it on, "Deactivating GPS"
        // (0x12C23B2C, OK 0x0639, 0x558620).
        if key == Key::Button1 {
            let map = map.clone();
            // Both requests name no package (0x4B1151, 0x4B1110):
            // 0x526C50 sizes them, in game too.
            let now = self.now();
            if map.gps {
                let mut off = map.clone();
                off.gps = false;
                let text = self.label(0x12C2_3B2C, "Deactivating GPS");
                let ok = self.label(0x0639, "OK");
                let mut d = Dialog::new(screens::dialog_package(&self.game_data.assets, &text), &text, &[&ok], now);
                // 0x558620 leaves the dialog's Back off (+0x384).
                d.back = false;
                // Over the map (arg 1: the screen's package, +4;
                // Wine: the map stays drawn under it).
                d.under = Some(1);
                self.mode = Mode::Menu(Menu::Dialog(
                    d,
                    vec![
                        Mode::Menu(Menu::LotDeal(LotDeal::GpsOff(Box::new(off)))),
                        Mode::Menu(Menu::InGameMap(map.clone())),
                    ],
                ));
            } else if let Some((target, trigger)) = map.target() {
                let back = Mode::Menu(Menu::InGameMap(map.clone()));
                let text = self.label(0x6347_7036, "Are you sure you want to engage the GPS system?");
                let (yes, no) = (self.label(0xF770, "Yes"), self.label(0x061C, "No"));
                let mut d = Dialog::new(screens::dialog_package(&self.game_data.assets, &text), &text, &[&yes, &no], now);
                // 0x5587A0's eighth argument 1 (0x4B1151): No
                // focused, so C then Enter does not engage (Wine
                // "No" lit); over the map (arg 1, +4).
                d.selected = ask_focus(0x6347_7036);
                d.under = Some(1);
                self.mode = Mode::Menu(Menu::Dialog(
                    d,
                    vec![Mode::Menu(Menu::LotDeal(LotDeal::Gps { target, trigger })), back],
                ));
            }
            return Ok(());
        }
        map.key(key, self.game_data.assets.package("UI_InGame_WorldMap"));
        self.career_world.map_hidden = map.hidden;
        if let Some(p) = self.profile.as_mut() {
            p.map_hidden = map.hidden;
        }
        Ok(())
    }

    pub(super) fn world_map_key(&mut self, key: Key, _key_time: f32) -> Result<()> {
        let Mode::Menu(Menu::WorldMap(map)) = &mut self.mode else { unreachable!() };
        if key == Key::Back {
            self.mode = self.icon_menu(MenuKind::Crib);
            return Ok(());
        }
        let (stage, selected, region) = (map.stage, map.selected, map.region);
        let picked = map.key(key);
        let real = self.real_now();
        if let Mode::Menu(Menu::WorldMap(map)) = &self.mode
            && map.stage != stage
        {
            // 0x4E0E50: a new stage rebuilds the event list
            // (0x4E0CE0) and sets +0x22C; the widget keeps its map.
            let (new_stage, widget) = (map.stage, map.widget);
            self.mode = self.world_map_stage(new_stage, Some(widget));
            return Ok(());
        }
        if let Mode::Menu(Menu::WorldMap(map)) = &mut self.mode
            && (map.selected != selected || map.region != region)
        {
            // 0x4CC140: a new selection sets +0x22C.
            map.widget.touch(real);
        }
        // Accept (0x0C407210 / 0x406415E3, handler 0x4E0F60) on any
        // event — the list (0x4B1360) is not filtered by lock or won
        // flag, only the region entry at position 1 (0x51F7F0) is
        // skipped — asks "Would you like to race this event?"
        // (0xEC9488BB); Yes (0x51123592) races the item's event
        // (+0x34, 0x541170 / 0x5325F0), No (0xB4623F67) stays.
        if let Some(e) = picked {
            let Mode::Menu(Menu::WorldMap(map)) = &self.mode else { unreachable!() };
            let stay = Mode::Menu(Menu::WorldMap(map.clone()));
            self.mode = dialog_over(
                self.ask(0xEC94_88BB, Mode::Menu(Menu::LotDeal(LotDeal::Race { event: e })), stay),
                true,
                None,
            );
        }
        Ok(())
    }
}
