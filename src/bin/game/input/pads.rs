use crate::*;

impl Game {
    /// The profile's controller maps given to the attached controllers
    /// with their GUIDs (0x5B9580).
    ///
    /// Then the profile keeps only the attached controllers' maps: the
    /// loader hands each block to the device with its GUID and the writer
    /// 0x5B9430 writes the attached records' blocks, zeros past them
    /// (**verified** under Wine: a community save holding a second
    /// controller's block, resaved by the original with only the keyboard
    /// attached, has zeros there). A controller that is not attached loses
    /// its map at the next save.
    pub(crate) fn apply_pad_maps(&mut self) {
        // Controller Layout Auto: a pad with custom Original bindings
        // in this profile is presented as Original, so they keep working.
        self.platform.pads.set_saved_blocks(&self.settings.pad_blocks);
        for d in &mut self.platform.pads.devices {
            if let Some(b) = self.settings.pad_blocks.iter().find(|b| b[..16] == d.guid) {
                d.map.apply_block(b);
            }
        }
        let mut blocks: Vec<[u8; nfsu2::pad::BLOCK_LEN]> =
            self.platform.pads.devices.iter().map(|d| d.map.to_block(&d.guid)).collect();
        blocks.truncate(nfsu2::pad::RECORDS - 1);
        self.settings.pad_blocks = blocks;
    }

    /// Controller `i`'s map into the profile's blocks (0x5B9430 writes the
    /// attached records' blocks; see `apply_pad_maps`).
    fn store_pad_map(&mut self, i: usize) {
        if i >= self.platform.pads.devices.len() {
            return;
        }
        // The attached records in their order, as 0x5B9430 writes them.
        let mut blocks: Vec<[u8; nfsu2::pad::BLOCK_LEN]> =
            self.platform.pads.devices.iter().map(|d| d.map.to_block(&d.guid)).collect();
        blocks.truncate(nfsu2::pad::RECORDS - 1);
        self.settings.pad_blocks = blocks;
        let _ = self.settings.save();
    }

    /// The Controls page's cells for the device shown.
    pub(crate) fn controls_cells(&self, c: &screens::ControlsScreen) -> ([[String; 2]; 18], screens::ControlsPage) {
        match c.device.checked_sub(1).and_then(|i| self.platform.pads.devices.get(i)) {
            Some(d) => {
                let page = if d.caps.dev_type == nfsu2::pad::DevType::Driving {
                    screens::ControlsPage::Wheel
                } else {
                    screens::ControlsPage::Joypad
                };
                let name = |b: nfsu2::pad::Binding| {
                    if d.layout == nfsu2::pad::host::Layout::Modern {
                        nfsu2::pad::host::modern_binding_name(b, d.family)
                    } else {
                        b.name()
                    }
                };
                (
                    std::array::from_fn(|row| {
                        std::array::from_fn(|col| name(d.map.binding(&d.caps, screens::CONTROL_ROWS[row].1, col as u8)))
                    }),
                    page,
                )
            }
            None => (self.settings.keys.clone(), screens::ControlsPage::Keyboard),
        }
    }

    /// The controllers' frame (0x5C0FA0, 0x5CA6A0, 0x5CFA20, 0x5C1850):
    /// hot-plug, a capture on the Controls screen (0x5CA730), then their
    /// presses: the FE keys in the menus and the replay (the FE map's
    /// actions and the stick rows), the driving actions elsewhere.
    pub(crate) fn pad_frame(&mut self) {
        // The Controller extras (a layout change re-presents the pads,
        // each taking its map for that layout from the profile).
        let relayout = self.platform.pads.set_options(self.settings.enh.pad);
        let changes = self.platform.pads.poll(self.seconds());
        // A button pressed: the log's waits count from it (docs/developing/waits-log.md).
        if self.platform.pads.pressed() != 0 {
            self.loads.waits.key();
        }
        if changes.attached || relayout {
            self.apply_pad_maps();
        }
        // Controller extra (Pause on Disconnect): the pad that drove went while
        // driving → the pause menu, as the Pause key opens it.
        if changes.driver_lost
            && self.settings.enh.pad.pause_on_disconnect
            && (matches!(self.mode, Mode::Explore) || matches!(&self.mode, Mode::Race(r) if r.replay.playback.is_none()))
        {
            let mut acts = [false; 18];
            acts[nfsu2::frontend::options::action::PAUSE] = true;
            let mut exit = false;
            if let Err(e) = self.dispatch(None, None, acts, &mut || exit = true) {
                eprintln!("{e:#}");
            }
        }
        // Controller extra (Quit with Both Sticks): L3 + R3 on the main menu
        // asks the 'Q' key's quit question (0x558B80); off, nothing is
        // tracked. Only the main menu itself, not a question over it.
        let main = self.settings.enh.pad.sticks_quit
            && matches!(self.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main)))
            && self.help_screen().is_some_and(|(mask, _)| mask != 0);
        if self.platform.pads.both_sticks(main) {
            self.quit_question();
        }
        // Restore Defaults' OK (0x4ACD90): the keyboard's map (0x5D0000,
        // the DIACTION defaults 0x7FCAC0) or the controller's (0x5C11B0).
        if let Mode::Menu(Menu::Controls(c)) = &mut self.mode
            && c.restore
        {
            c.restore = false;
            let device = c.device;
            match device.checked_sub(1) {
                Some(i) => {
                    if let Some(d) = self.platform.pads.devices.get_mut(i) {
                        d.restore_defaults();
                        self.store_pad_map(i);
                    }
                }
                None => {
                    self.settings.keys = options::DEFAULT_KEYS.map(|k| k.map(String::from));
                    let _ = self.settings.save();
                }
            }
        }
        if let Mode::Menu(Menu::Controls(c)) = &mut self.mode
            && c.device > self.platform.pads.devices.len()
        {
            c.device = 0;
            c.capturing = false;
        }
        // A capture for a controller's page takes that controller's input.
        if let Mode::Menu(Menu::Controls(c)) = &mut self.mode
            && c.capturing
        {
            let (ticked, safe) = (self.platform.pads.ticked, self.settings.enh.pad.safe_bind);
            if let Some(i) = c.device.checked_sub(1)
                && let Some(d) = self.platform.pads.devices.get_mut(i)
            {
                if d.map.capture == 0 {
                    d.map.begin_capture(screens::CONTROL_ROWS[c.selected].1, c.column as u8);
                    // 0x5B91D3: the axes' base at the capture's start.
                    d.map.axis_base = d.prev.axes;
                    d.capture_prev = d.prev;
                }
                // 0x5CA730 runs once a frame of the original's (60 Hz ticks
                // of the game clock, so the same at any frame rate); a
                // button's edge spans the frames in between.
                if ticked {
                    let (caps, now) = (d.caps.clone(), d.state);
                    let prev = std::mem::replace(&mut d.capture_prev, now);
                    if d.map.capture_step_with(&caps, &prev, &now, safe) {
                        c.capturing = false;
                        c.bound = true;
                        self.store_pad_map(i);
                    }
                }
            }
            return;
        }
        let pressed = self.platform.pads.pressed();
        // The FE stick rows run every frame (their repeat state with them).
        let stick = self.platform.pads.stick_pressed();
        // Controller extra (Modern layout): the footers name the pad used last.
        let modern = |d: &&nfsu2::pad::host::Device| d.layout == nfsu2::pad::host::Layout::Modern;
        if !self.platform.pads.any_modern() {
            nfsu2::pad::extras::set_prompt(None);
        } else if let Some(d) = self.platform.pads.devices.iter().filter(modern).find(|d| !d.report.mask & d.last.mask != 0) {
            nfsu2::pad::extras::set_prompt(Some(d.family));
        } else if stick.iter().any(|&s| s)
            && let Some(d) = self.platform.pads.devices.iter().find(modern)
        {
            nfsu2::pad::extras::set_prompt(Some(d.family));
        }
        let fe = matches!(&self.mode, Mode::Menu(_)) || matches!(&self.mode, Mode::Race(r) if r.replay.playback.is_some());
        let mut exit = false;
        if fe {
            use nfsu2::pad::act::*;
            let keys = [
                (MENU_UP, KeyCode::ArrowUp),
                (MENU_DOWN, KeyCode::ArrowDown),
                (MENU_LEFT, KeyCode::ArrowLeft),
                (MENU_RIGHT, KeyCode::ArrowRight),
                (MENU_SELECT, KeyCode::Enter),
                (MENU_SELECT_ALT, KeyCode::NumpadEnter),
                (MENU_BACK, KeyCode::Escape),
                (MENU_CUSTOMISE, KeyCode::KeyC),
                (MENU_BUTTON2, KeyCode::Comma),
                (MENU_BUTTON3, KeyCode::Period),
                (MENU_L1, KeyCode::BracketLeft),
                (MENU_R1, KeyCode::BracketRight),
                (0x2E, KeyCode::KeyP),
                (0x2F, KeyCode::KeyQ),
            ];
            // Photo mode reads the sticks itself (no menu steps).
            let stick = if self.photo.is_some() { [false; 4] } else { stick };
            let mut codes: Vec<KeyCode> =
                keys.iter().filter(|&&(a, _)| pressed & (1u64 << a) != 0).map(|&(_, k)| k).collect();
            for (on, k) in
                stick.into_iter().zip([KeyCode::ArrowUp, KeyCode::ArrowDown, KeyCode::ArrowLeft, KeyCode::ArrowRight])
            {
                if on && !codes.contains(&k) {
                    codes.push(k);
                }
            }
            // Controller extra (Controller Layout Modern): MC_Main's name editor
            // takes the directions (`McMain::pad_edit`).
            if self.platform.pads.any_modern()
                && let Mode::Menu(Menu::McMain(m)) = &mut self.mode
                && m.step == McStep::Name
            {
                codes.retain(|k| {
                    match [KeyCode::ArrowUp, KeyCode::ArrowDown, KeyCode::ArrowLeft, KeyCode::ArrowRight]
                        .iter()
                        .position(|a| a == k)
                    {
                        Some(dir) => {
                            m.pad_edit(dir);
                            false
                        }
                        None => true,
                    }
                });
            }
            for code in codes {
                if let Err(e) = self.key(code, &mut || exit = true) {
                    eprintln!("{e:#}");
                }
            }
            // BUTTON1's release (0xD871B0A4): the purge test's end.
            if self.platform.pads.released() & (1u64 << MENU_CUSTOMISE) != 0 {
                self.key_released(KeyCode::KeyC);
            }
        } else {
            for (row, &(_, a)) in screens::CONTROL_ROWS.iter().enumerate() {
                if pressed & (1u64 << a) != 0 {
                    let mut acts = [false; 18];
                    acts[row] = true;
                    if let Err(e) = self.dispatch(None, None, acts, &mut || exit = true) {
                        eprintln!("{e:#}");
                    }
                }
            }
        }
        self.platform.exit_requested |= exit;
    }

    /// Controller extra (Rumble, off = native): one world update of the rumble
    /// model from the player's car, sent to the pads. Nothing runs (no
    /// state) while the option is off.
    pub(crate) fn rumble_tick(&mut self) {
        if self.settings.enh.pad.rumble == 0 || self.platform.pads.devices.is_empty() {
            if self.rumble_gear.take().is_some() {
                self.rumble_travel = None;
                self.rumble.reset();
                self.platform.pads.set_rumble(0.0, 0.0);
            }
            return;
        }
        let driving = !self.career_world.dday.holds_input()
            && match &self.mode {
                Mode::Explore => true,
                Mode::Race(r) => r.replay.playback.is_none(),
                _ => false,
            };
        let feed = match self.sim.drive.as_mut().filter(|_| driving) {
            Some(d) => {
                let gear = d.state.drivetrain.as_ref().map(|t| t.gear);
                let shifted = gear.is_some() && self.rumble_gear.is_some_and(|g| Some(g) != gear);
                self.rumble_gear = gear.or(Some(0));
                // The suspension travel since the last tick (0 on the
                // first: no jolt from a fresh car).
                let now = d.state.wheels.each_ref().map(|w| w.compression);
                let bump = self.rumble_travel.map_or(0.0, |was| {
                    now.iter().zip(was).map(|(n, w)| (n - w).abs()).sum::<f32>()
                });
                // The hardest new hit since the last tick (none on the
                // first: a hit from before the rumble ran is dropped).
                let hit = d.state.take_hit();
                let hit = if self.rumble_travel.is_some() { hit } else { 0.0 };
                self.rumble_travel = Some(now);
                nfsu2::pad::extras::RumbleFeed {
                    speed: d.state.velocity.length(),
                    bump,
                    surfaces: d.state.wheels.each_ref().map(|w| w.surface),
                    // Surface 13 (PUDDLE), the one 0x5C9DA0's negative
                    // damper plays on.
                    rough: d.state.wheels.iter().any(|w| w.surface == 0xD),
                    nitrous: d.state.nitrous.is_some_and(|t| t.running.is_some()),
                    shifted,
                    hit,
                    driving: true,
                }
            }
            None => {
                self.rumble_gear = Some(0);
                self.rumble_travel = None;
                nfsu2::pad::extras::RumbleFeed::default()
            }
        };
        let (strong, weak) = self.rumble.step(&feed);
        self.platform.pads.set_rumble(strong, weak);
    }
}
