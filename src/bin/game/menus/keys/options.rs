use crate::*;

impl Game {
    pub(super) fn options_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::Options(o)) = &mut self.mode else { unreachable!() };
        match o.key(key, &mut self.settings, self.platform.pads.force_feedback(), self.profile.is_some(), now) {
            OptionsKey::None => {}
            OptionsKey::Changed => {
                let _ = self.settings.save();
            }
            // Back (0x49C690 on the screen's out animation): Gameplay
            // asks before leaving when save prompts went off ("turn off
            // save prompts?") or autosave on ("turn on Autosave?");
            // Yes leaves, No stays.
            OptionsKey::Leave => self.options_leave(now),
            // "Default Settings" asks "Reset to default settings?"
            // (0x49B980); Yes resets the display (0x5CECA0).
            OptionsKey::Button(options::RowId::DefaultDisplay) => {
                let (yes, no) = (Mode::Menu(Menu::Options(o.clone())), Mode::Menu(Menu::Options(o.clone())));
                self.fe.resetting_display = true;
                // Over the page, no StartDim (Wine, menus15: the Display rows stay
                // under the question, unhighlighted; the garage behind).
                self.mode = dialog_over(self.ask_in("GenericDialog_SMALL", 0x2BBC_CA6B, yes, no), false, None);
            }
            // Customize Display Settings (0x49C98A; on the pause's
            // 0x4A82FA): UI_PC_Customize_Options. Wheel Options
            // (0x49C9A8): UI_Wheel_Options for the player shown.
            OptionsKey::Button(options::RowId::CustomizeDisplay) => {
                self.mode = Mode::Menu(Menu::Options(options::OptionsScreen::new_in(
                    options::Category::CustomDisplay,
                    &self.settings,
                    self.platform.pads.force_feedback(),
                    o.pause,
                    now,
                )));
            }
            // The clone's Enhancements page (QoL, clone deviations).
            OptionsKey::Button(options::RowId::Enhancements) => {
                self.mode = Mode::Menu(Menu::Options(options::OptionsScreen::new_in(
                    options::Category::Enhancements,
                    &self.settings,
                    self.platform.pads.force_feedback(),
                    o.pause,
                    now,
                )));
            }
            // The clone's Graphics page (graphics enhancements).
            OptionsKey::Button(options::RowId::HdUiPage) => {
                self.mode = Mode::Menu(Menu::Options(options::OptionsScreen::new_in(
                    options::Category::HdUi,
                    &self.settings,
                    self.platform.pads.force_feedback(),
                    o.pause,
                    now,
                )));
            }
            OptionsKey::Button(options::RowId::Graphics) => {
                self.mode = Mode::Menu(Menu::Options(options::OptionsScreen::new_in(
                    options::Category::Graphics,
                    &self.settings,
                    self.platform.pads.force_feedback(),
                    o.pause,
                    now,
                )));
            }
            // The settings page's Advanced page (from the hub's Advanced row).
            OptionsKey::Button(options::RowId::AdvancedPage) => {
                self.mode = Mode::Menu(Menu::Options(options::OptionsScreen::new_in(
                    options::Category::Advanced,
                    &self.settings,
                    self.platform.pads.force_feedback(),
                    o.pause,
                    now,
                )));
            }
            // A group page of the settings page (from the hub, or
            // the next / previous one with the shoulder keys).
            OptionsKey::Button(options::RowId::UxGroup(g)) => {
                self.mode = Mode::Menu(Menu::Options(options::OptionsScreen::new_in(
                    options::Category::Settings(g),
                    &self.settings,
                    self.platform.pads.force_feedback(),
                    o.pause,
                    now,
                )));
            }
            OptionsKey::Button(options::RowId::GraphicsGroup(g)) => {
                self.mode = Mode::Menu(Menu::Options(options::OptionsScreen::new_in(
                    options::Category::GraphicsGroup(g),
                    &self.settings,
                    self.platform.pads.force_feedback(),
                    o.pause,
                    now,
                )));
            }
            // Restore Native: every deviation off at once.
            OptionsKey::Button(options::RowId::RestoreNative) => {
                self.settings.restore_native();
                let _ = self.settings.save();
            }
            OptionsKey::Button(_) => {
                let mut w = options::OptionsScreen::new(options::Category::Wheel, &self.settings, self.platform.pads.force_feedback(), now);
                w.player = o.player;
                self.mode = Mode::Menu(Menu::Options(w));
            }
        }
        Ok(())
    }

    /// Back on an options page: the page or menu it goes back to, after the
    /// Gameplay page's save-prompt / autosave question.
    fn options_leave(&mut self, now: f32) {
        let Mode::Menu(Menu::Options(o)) = &mut self.mode else { unreachable!() };
        let (prompts, autosave) = o.opened_save;
        let gameplay = o.category == options::Category::Gameplay;
        let stay = Mode::Menu(Menu::Options(o.clone()));
        let back = match (o.category, o.pause) {
            // UI_PC_Customize_Options' Back (0x49C540): in the
            // game (0x8900D8 state 6) UI_PauseOptions, else
            // UI_Options, on Display.
            (options::Category::CustomDisplay, pause) => {
                let mut d = options::OptionsScreen::new_in(
                    options::Category::Display,
                    &self.settings,
                    self.platform.pads.force_feedback(),
                    pause,
                    now,
                );
                d.selected = 2;
                Mode::Menu(Menu::Options(d))
            }
            // The Enhancements page goes back to Display, on
            // its row.
            (options::Category::Enhancements, pause) => {
                // Compare off, the page's session state reset.
                nfsu2::frontend::settings_page::reset_session(&self.settings);
                let mut d = options::OptionsScreen::new_in(
                    options::Category::Display,
                    &self.settings,
                    self.platform.pads.force_feedback(),
                    pause,
                    now,
                );
                let rows = options::rows_in(options::Category::Display, &self.settings, 0, self.platform.pads.force_feedback(), pause);
                d.selected = rows.iter().position(|r| r.id == options::RowId::Enhancements).unwrap_or(0);
                d.top = (d.selected + 1).saturating_sub(options::VISIBLE);
                Mode::Menu(Menu::Options(d))
            }
            // The Graphics page goes back to Enhancements, on
            // its row.
            (options::Category::Graphics | options::Category::HdUi, pause) => {
                let mut d = options::OptionsScreen::new_in(
                    options::Category::Enhancements,
                    &self.settings,
                    self.platform.pads.force_feedback(),
                    pause,
                    now,
                );
                let rows = options::rows_in(options::Category::Enhancements, &self.settings, 0, self.platform.pads.force_feedback(), pause);
                d.selected = rows
                    .iter()
                    .position(|r| {
                        r.id == if o.category == options::Category::HdUi {
                            options::RowId::HdUiPage
                        } else {
                            options::RowId::Graphics
                        }
                    })
                    .unwrap_or(0);
                d.top = (d.selected + 1).saturating_sub(options::VISIBLE);
                Mode::Menu(Menu::Options(d))
            }
            // A settings group page goes back to the page that lists it
            // (the Advanced page for the graphics groups, else the hub),
            // on its group's row; the Advanced page to the hub on its row.
            (options::Category::Settings(_) | options::Category::Advanced, pause) => {
                let (up, on) = match o.category {
                    options::Category::Settings(g)
                        if nfsu2::frontend::settings_page::ADVANCED_GROUPS.contains(&g) =>
                    {
                        (options::Category::Advanced, options::RowId::UxGroup(g))
                    }
                    options::Category::Settings(g) => (options::Category::Enhancements, options::RowId::UxGroup(g)),
                    _ => (options::Category::Enhancements, options::RowId::AdvancedPage),
                };
                let mut d = options::OptionsScreen::new_in(up, &self.settings, self.platform.pads.force_feedback(), pause, now);
                let rows = options::rows_in(up, &self.settings, 0, self.platform.pads.force_feedback(), pause);
                d.selected = rows.iter().position(|r| r.id == on).unwrap_or(0);
                d.top = (d.selected + 1).saturating_sub(options::PAGE_VISIBLE);
                Mode::Menu(Menu::Options(d))
            }
            // A Graphics group page goes back to the Graphics
            // page, on its group's row.
            (options::Category::GraphicsGroup(g), pause) => {
                let mut d = options::OptionsScreen::new_in(
                    options::Category::Graphics,
                    &self.settings,
                    self.platform.pads.force_feedback(),
                    pause,
                    now,
                );
                let rows = options::rows_in(options::Category::Graphics, &self.settings, 0, self.platform.pads.force_feedback(), pause);
                d.selected = rows.iter().position(|r| r.id == options::RowId::GraphicsGroup(g)).unwrap_or(0);
                d.top = (d.selected + 1).saturating_sub(options::VISIBLE);
                Mode::Menu(Menu::Options(d))
            }
            // UI_Wheel_Options' Back (0x49CBB0): UI_Options.
            (options::Category::Wheel, _) => {
                let mut d = options::OptionsScreen::new(options::Category::Player, &self.settings, self.platform.pads.force_feedback(), now);
                d.selected = 9;
                d.top = 2;
                Mode::Menu(Menu::Options(d))
            }
            // UI_PauseOptions' Back (0x4A8010): UI_PauseOptionsMain.
            (_, Some(p)) => self.icon_menu(MenuKind::PauseOptions(p.race)),
            _ => self.icon_menu(MenuKind::Options),
        };
        self.mode = if gameplay && prompts && !self.settings.save_prompts {
            self.ask(0x22BC_94AF, back, stay)
        } else if gameplay && !autosave && self.settings.autosave {
            self.ask(0xF356_8B86, back, stay)
        } else {
            back
        };
    }
}
