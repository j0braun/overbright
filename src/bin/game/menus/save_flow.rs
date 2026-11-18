use crate::*;

impl Game {
    /// The memory-card library's text `i` (`profiles::text`).
    pub(crate) fn mc_text(&self, i: usize, arg: &str) -> String {
        profiles::text(&self.game_data.root, i, arg)
    }

    pub(crate) fn mc_yes_no(&self, text: &str, yes: McAct, no: McAct) -> Mode {
        let m = McMessage::ask(text, &self.mc_text(2, ""), &self.mc_text(3, ""), self.now());
        Mode::Menu(Menu::McMessage(m, vec![yes, no]))
    }

    pub(crate) fn mc_tell(&self, text: &str, then: McAct) -> Mode {
        let m = McMessage::tell(text, &self.mc_text(4, ""), self.now());
        Mode::Menu(Menu::McMessage(m, vec![then]))
    }

    /// MC_List of the profiles on disk ("No profiles were found.",
    /// 0xAB1F0E92, when there are none).
    pub(crate) fn mc_list(&self, op: McOp) -> Mode {
        let names = profiles::list();
        if names.is_empty() {
            let text = self.label(0xAB1F_0E92, "No profiles were found.");
            return self.mc_tell(&text, McAct::To(MenuKind::Profile));
        }
        Mode::Menu(Menu::McList(McList::new(op, names, self.now())))
    }

    /// A name picked from MC_List: Load asks first when a profile is
    /// current ("Loading a profile will result in the loss of any unsaved
    /// changes…", 0x81B1D73D); Delete asks "Are you sure you want to delete
    /// %1?" (the library's text 11). No is focused; No goes back to the list.
    pub(crate) fn mc_pick(&mut self, op: McOp, name: String) -> Mode {
        match op {
            McOp::Load if self.profile.is_some() || profiles::current_name().is_some() => {
                let text = self.label(0x81B1_D73D, "Are you sure you want to load a profile?");
                self.mc_yes_no(&text, McAct::Load(name), McAct::List(op))
            }
            McOp::Load => self.mc_act_mode(McAct::Load(name)),
            McOp::Delete => {
                let text = self.mc_text(profiles::mc_text::DELETE_ASK, &name);
                self.mc_yes_no(&text, McAct::Delete(name), McAct::List(op))
            }
        }
    }

    /// "Are you sure you want to save^the profile named %s?" (0x2A9BA6F9,
    /// state 5), unasked when the Save option is at Autosave and the
    /// operation lacks 0x200 (0x4E8D30; the Profile Manager's 0x4A51 has
    /// it, so it always asks).
    pub(crate) fn mc_save_question(&self, name: &str, back: MenuKind) -> Mode {
        let text = self.label(0x2A9B_A6F9, "Are you sure you want to save^the profile named %s?").replace("%s", name);
        self.mc_yes_no(&text, McAct::SaveAsk(name.to_string()), McAct::To(back))
    }

    /// 0x52A470: the save-point snapshot of the profile, unless one is
    /// kept; only with save prompts on (profile +0xA108) or `force`.
    pub(crate) fn take_snapshot(&self, force: bool) {
        if self.save_flow.snapshot.borrow().is_some() || !(force || self.settings.save_prompts) {
            return;
        }
        if let Some(p) = self.profile.as_ref().filter(|p| !p.scratch) {
            let _ = p.commit();
            *self.save_flow.snapshot.borrow_mut() = p.native_payload();
        }
    }

    /// 0x5032C0: the snapshot dropped.
    pub(crate) fn drop_snapshot(&self) {
        self.save_flow.snapshot.borrow_mut().take();
    }

    /// 0x52A4D0: whether the profile differs from the snapshot (none: no);
    /// the snapshot is dropped.
    pub(crate) fn changed_since_snapshot(&self) -> bool {
        let Some(snap) = self.save_flow.snapshot.borrow_mut().take() else { return false };
        let Some(p) = self.profile.as_ref() else { return false };
        let _ = p.commit();
        p.native_payload().is_some_and(|now| now != snap)
    }

    /// An MC_Main operation from the icon menu `kind` with mode & 0xF = 1
    /// (UI_ProfileManager's Create 0x861, Save 0x4A51, Load 0xC11, Delete
    /// 0x831): 0x4A3100 sends 0x96DDA9E6 to the menu (0x4A3172: its
    /// StartDim, the Event Handler's DIM) and pushes the MC package over it
    /// (0x555EC0(package, caller, 0xFF, 0)); modes 2 / 3 replace the caller
    /// (0x537990, e.g. the Options' 0x2042). Under Wine (menus13
    /// profile_load_list) the Profile Manager stays under MC_List: its
    /// strip fades out, its title stays (behind the list's header), the
    /// garage scene shows through; no MC_Background.
    pub(crate) fn mc_over(&mut self, kind: MenuKind, now: f32) {
        if self.fe.mc_background {
            return;
        }
        let mut menu = self.icon_menu(kind);
        settle_screen(&mut menu, now);
        self.fe.mc_under =
            Some((Box::new(menu), kind.package(), now, (nfsu2::frontend::DIM_SCRIPT, nfsu2::frontend::UNDIM_SCRIPT)));
    }

    /// A save point (0x4A3100 operation 0x40) with no profile current:
    /// MC_Main's start (0x4F33A0) finds none (0x502FE0) and asks "Would
    /// you like to create a new profile?" (0x4E8D30(2), No focused). For
    /// the modes with low nibble 2 (the Options' and Customize's Back,
    /// 0x2042) MC_Main replaces the caller and its end switches to the
    /// next screen whatever the answer (0x4A31D0 → 0x537990); no
    /// message reaches the caller (no 0x100 / 0x400, 0x4A30A0).
    pub(crate) fn save_point_create(&mut self, then: AfterSave) {
        let now = self.now();
        let mut m = McMain::new(false, now);
        m.autosave = self.settings.autosave;
        m.career = false;
        self.save_flow.mc_create_then = Some(then);
        self.mode = Mode::Menu(Menu::McMain(m));
    }

    /// A save point (0x4A3100 with operation 0x40 or 0x50, MC_Main's
    /// states 0x4E8D30; **verified** under Wine on the Options' Back):
    /// Prompt asks "Are you sure you want to save^the profile named %s?"
    /// (No focused), Yes asks "Do you want to overwrite previously saved
    /// game?" and saves with "Save complete." (Continue); Autosave (and no
    /// 0x200 in the mode) skips the first question and the message (the
    /// first autosave of the session still asked the overwrite question).
    /// Then `then`. Without a profile the game goes on here; the callers
    /// that reach operation 0x40 with no profile ask to create one first
    /// (`save_point_create`, `customize_dyno_yes`).
    pub(crate) fn save_point(&mut self, always_ask: bool, then: AfterSave) -> Result<()> {
        let under = self.fe.save_under.take();
        let Some(name) = self.profile.as_ref().filter(|p| !p.scratch).map(|p| p.name.clone()) else {
            return self.go_on(then);
        };
        // MC_Main over the caller (mode & 0xF = 1, see `save_under`).
        if !self.fe.mc_background
            && let Some((mut m, package, at, scripts)) = under
        {
            settle_screen(&mut m, at);
            self.fe.mc_under = Some((m, package, at, scripts));
        }
        if let Some(p) = self.profile.as_ref() {
            let _ = p.commit();
        }
        if self.settings.autosave && !always_ask {
            if !self.save_flow.autosaved && profiles::exists(&profiles::sanitize(&name)) {
                let text = self.mc_text(profiles::mc_text::OVERWRITE, "");
                self.mode = self.mc_yes_no(&text, McAct::SpSave(then.clone(), false), McAct::SpNo(then, true));
                return Ok(());
            }
            return self.mc_act(McAct::SpSave(then, false));
        }
        self.mc_act(McAct::SpAsk(then))
    }

    /// UI_PostRace's save point is due (0x4F3A09 / 0x4F3A89): game mode 1
    /// (a career), save prompts on, and the race just run won
    /// (0x5287B0(0): the unsettled outcome, `race_result`, is a win), not
    /// mid-tournament.
    pub(crate) fn post_race_save_due(&self) -> bool {
        self.race_flow.game_mode == 1
            && self.settings.save_prompts
            && self.race_flow.next_round.is_none()
            && self.race_flow.result.as_ref().is_some_and(|r| r.won)
    }

    /// Past a save point.
    pub(crate) fn go_on(&mut self, then: AfterSave) -> Result<()> {
        match then {
            AfterSave::Menu(kind) => self.mode = self.icon_menu(kind),
            AfterSave::Explore(at) => {
                self.drop_drive();
                self.explore(at)?;
            }
            AfterSave::LeaveGarage(at) => {
                if let Some(p) = self.profile.as_mut() {
                    p.explore_set = true;
                }
                self.drop_drive();
                self.explore(at)?;
            }
            AfterSave::Act(action, kind) => {
                self.save_flow.post_race_saved = true;
                self.fe.confirmed = true;
                self.act(action, kind)?;
            }
            AfterSave::Roam => self.mode = Mode::Explore,
            AfterSave::CustomizeDyno => {
                if let Some(scratch) = self.dyno.scratch.take() {
                    let real = self.profile.replace(scratch);
                    self.save_flow.customize = Some(real);
                }
                self.mode = self.enter_dyno_scene();
            }
            AfterSave::QuitTuning => {
                self.quit_tuning();
                self.mode = self.after_tuning();
            }
            AfterSave::CarSelect => {
                self.mode = match self.quick.customize.take() {
                    Some(mut lot) => {
                        // The list again (All Cars, 0x4FC1F0 set it), on the
                        // car customized (player 0's, 0x502A40).
                        lot.opened = self.now();
                        lot.filter = 3;
                        self.quick.filter = 3;
                        lot.cars = self.qr_cars(3);
                        let id = self.profile.as_ref().and_then(|p| p.menu_car_id);
                        lot.selected = id
                            .and_then(|id| {
                                lot.cars.iter().position(|c| c.my_car.is_some_and(|k| career::tuned_car_id(k) == id))
                            })
                            .unwrap_or(0);
                        Mode::Menu(Menu::CarLot(lot))
                    }
                    None => self.icon_menu(MenuKind::Main),
                };
            }
        }
        Ok(())
    }

    /// The boot's profile step after the splash (**verified** in the
    /// original): one profile was loaded ("Load complete."), several are
    /// listed to pick from, none asks to create one (MC_Main operation 0x60).
    pub(crate) fn boot_profile_step(&mut self) -> Mode {
        let now = self.now();
        // UG_LS_Splash's end (0x4C4F50) opens MC_Background.fng (0x537990)
        // when the memory-card state 0x836498 +0x20 is 0 or 0xB; the boot's
        // MC_Main / list / message screens are pushed over it.
        self.fe.mc_background = true;
        match self.fe.boot_profiles.take() {
            Some(profiles::Boot::One(name)) if profiles::current_name().as_deref() == Some(name.as_str()) => {
                self.profile = Profile::load();
                self.settings = options::Settings::load();
                // The profile's EA Trax list in force (0x5344E0 / 0x515AA0 →
                // 0x479220): the pools refill, the song goes on.
                if let Some(a) = self.audio.as_mut() {
                    a.streams.reset_playlist();
                }
                self.apply_pad_maps();
                let text = self.mc_text(profiles::mc_text::LOAD_DONE, &name);
                self.mc_tell(&text, McAct::To(MenuKind::Main))
            }
            Some(profiles::Boot::One(name)) => {
                let text = self.mc_text(profiles::mc_text::LOAD_FAILED, &name);
                self.mc_tell(&text, McAct::To(MenuKind::Main))
            }
            Some(profiles::Boot::Pick(names)) => {
                self.fe.from_boot = true;
                Mode::Menu(Menu::McList(McList::new(McOp::Load, names, now)))
            }
            Some(profiles::Boot::None) => {
                self.fe.from_boot = true;
                let mut m = McMain::new(false, now);
                m.autosave = self.settings.autosave;
                m.career = false;
                Mode::Menu(Menu::McMain(m))
            }
            None => self.icon_menu(MenuKind::Main),
        }
    }

    /// The mode a memory-card step leads to (side effects included).
    pub(crate) fn mc_act_mode(&mut self, act: McAct) -> Mode {
        let _ = self.mc_act(act);
        std::mem::replace(&mut self.mode, Mode::Menu(Menu::LotDeal(LotDeal::QuitTuning)))
    }

    pub(crate) fn mc_act(&mut self, act: McAct) -> Result<()> {
        let act = match act {
            McAct::Continue(_)
            | McAct::Cover(..)
            | McAct::SpAsk(_)
            | McAct::SpNo(..)
            | McAct::SaveNo(_)
            | McAct::SpOverwrite(_)
            | McAct::SpSave(..) => return self.mc_act_save_point(act),
            other => other,
        };
        self.mc_act_profile(act)
    }

    /// `mc_act`'s save-point and cover steps (each sets the mode or goes on).
    fn mc_act_save_point(&mut self, act: McAct) -> Result<()> {
        match act {
            McAct::Continue(then) => return self.go_on(then),
            McAct::Cover(milestone, dvd, loaded) => {
                let now = self.now();
                self.mode = if loaded {
                    Mode::Menu(Menu::Cover(screens::CoverView {
                        milestone,
                        dvd,
                        opened: now,
                        camera: None,
                        layers: [0; 4],
                        reward: false,
                        picture: true,
                    }))
                } else {
                    let selected = self.earned_covers(dvd).iter().position(|&i| i == milestone).unwrap_or(0);
                    self.covers_menu(dvd, selected)
                };
                return Ok(());
            }
            McAct::SpAsk(then) => {
                let name = self.profile.as_ref().map(|p| p.name.clone()).unwrap_or_default();
                let text =
                    self.label(0x2A9B_A6F9, "Are you sure you want to save^the profile named %s?").replace("%s", &name);
                self.mode = self.mc_yes_no(&text, McAct::SpOverwrite(then.clone()), McAct::Continue(then));
                return Ok(());
            }
            McAct::SpNo(then, first_autosave) => {
                if first_autosave {
                    self.settings.autosave = false;
                    self.save_flow.autosaved = false;
                    let _ = self.settings.save();
                }
                return self.mc_act(McAct::SpAsk(then));
            }
            McAct::SaveNo(name) => {
                let back = if self.profile.is_some() { MenuKind::Profile } else { MenuKind::Main };
                self.mode = self.mc_save_question(&name, back);
                return Ok(());
            }
            McAct::SpOverwrite(then) => {
                let name = self.profile.as_ref().map(|p| p.name.clone()).unwrap_or_default();
                if profiles::exists(&profiles::sanitize(&name)) {
                    let text = self.mc_text(profiles::mc_text::OVERWRITE, "");
                    self.mode = self.mc_yes_no(&text, McAct::SpSave(then.clone(), true), McAct::SpNo(then, false));
                    return Ok(());
                }
                return self.mc_act(McAct::SpSave(then, true));
            }
            McAct::SpSave(then, tell) => {
                let name = self.profile.as_ref().map(|p| p.name.clone()).unwrap_or_default();
                let saved = self.profile.as_ref().map_or_else(|| Err(anyhow::anyhow!("no profile")), |p| p.save());
                if self.settings.autosave {
                    self.save_flow.autosaved = true;
                }
                match saved {
                    Ok(()) if tell => {
                        let text = self.mc_text(profiles::mc_text::SAVE_DONE, &name);
                        self.mode = self.mc_tell(&text, McAct::Continue(then));
                    }
                    Ok(()) => return self.go_on(then),
                    Err(e) => {
                        eprintln!("save {name}: {e:#}");
                        let text = self.mc_text(profiles::mc_text::SAVE_FAILED, &name);
                        self.mode = self.mc_tell(&text, McAct::Continue(then));
                    }
                }
                return Ok(());
            }
            other => unreachable!("mc_act_save_point: {other:?}"),
        }
    }

    /// `mc_act`'s profile steps: menus, load, delete, save.
    fn mc_act_profile(&mut self, act: McAct) -> Result<()> {
        let back = if self.profile.is_some() { MenuKind::Profile } else { MenuKind::Main };
        self.mode = match act {
            McAct::Continue(_)
            | McAct::SpOverwrite(_)
            | McAct::SpSave(..)
            | McAct::SpAsk(_)
            | McAct::SpNo(..)
            | McAct::SaveNo(_)
            | McAct::Cover(..) => unreachable!("handled above"),
            McAct::To(kind) => self.icon_menu(kind),
            McAct::List(op) => self.mc_list(op),
            McAct::Load(name) => match profiles::load(&name) {
                Ok(_) => {
                    self.save_flow.autosaved = false;
                    self.drop_snapshot();
                    self.profile = Profile::load();
                    if let Some(p) = self.profile.as_mut() {
                        p.upgrade_save(&self.game_data.career);
                        // The open events are not in the save (career +0x71C8 holds
                        // event-record pointers) and the load picks none: the
                        // original makes no slot-event draw at a profile load or at
                        // Resume, its first after the load is the city start's
                        // 0x527D40 (**measured**, the sequence log, docs/rng.md).
                    }
                    self.settings = options::Settings::load();
                    // The profile's EA Trax list in force (0x5344E0 / 0x515AA0 →
                    // 0x479220): the pools refill, the song goes on.
                    if let Some(a) = self.audio.as_mut() {
                        a.streams.reset_playlist();
                    }
                    self.apply_pad_maps();
                    self.race_flow.game_mode = 0;
                    let text = self.mc_text(profiles::mc_text::LOAD_DONE, &name);
                    // From the boot's list on to UI_Main, else back to
                    // the Profile Manager (as the original went).
                    let to = if std::mem::take(&mut self.fe.from_boot) { MenuKind::Main } else { MenuKind::Profile };
                    self.mc_tell(&text, McAct::To(to))
                }
                Err(e) => {
                    eprintln!("load {name}: {e:#}");
                    let text = self.mc_text(profiles::mc_text::LOAD_FAILED, &name);
                    self.mc_tell(&text, McAct::List(McOp::Load))
                }
            },
            McAct::Delete(name) => match profiles::delete(&name) {
                Ok(()) => {
                    if self.profile.as_ref().is_some_and(|p| profiles::sanitize(&p.name).eq_ignore_ascii_case(&name)) {
                        self.profile = None;
                    }
                    let text = self.mc_text(profiles::mc_text::DELETED, &name);
                    self.mc_tell(&text, McAct::To(MenuKind::Profile))
                }
                Err(e) => {
                    eprintln!("delete {name}: {e:#}");
                    let text = self.mc_text(profiles::mc_text::DELETE_FAILED, &name);
                    self.mc_tell(&text, McAct::To(MenuKind::Profile))
                }
            },
            // "Do you want to overwrite previously saved game?" (the
            // library's text 6) when the profile's file exists, Autosave
            // or not (mode 0x4A51 has 0x4000: state 6 → 7 at once, the
            // library's save asks; observed under Wine with Autosave on
            // and the session's card +6 set). No: "Are you sure…" again.
            McAct::SaveAsk(name) => {
                // The library finds the file case-insensitively (Windows; under
                // Wine "name" over "NAME" asked this, `profiles::resolve`).
                if profiles::exists(&profiles::sanitize(&name)) {
                    let text = self.mc_text(profiles::mc_text::OVERWRITE, "");
                    self.mc_yes_no(&text, McAct::Save(name.clone()), McAct::SaveNo(name))
                } else {
                    return self.mc_act(McAct::Save(name));
                }
            }
            McAct::Save(name) => {
                let saved = match self.profile.as_ref() {
                    Some(p) => p.save(),
                    None => Err(anyhow::anyhow!("no profile")),
                };
                let to = if std::mem::take(&mut self.fe.from_boot) { MenuKind::Main } else { back };
                match saved {
                    // With Autosave on no "Save complete." (the Profile
                    // Manager's Save under Wine: straight back on it).
                    Ok(()) if self.settings.autosave => {
                        self.save_flow.autosaved = true;
                        self.icon_menu(to)
                    }
                    Ok(()) => {
                        let text = self.mc_text(profiles::mc_text::SAVE_DONE, &name);
                        self.mc_tell(&text, McAct::To(to))
                    }
                    Err(e) => {
                        eprintln!("save {name}: {e:#}");
                        let text = self.mc_text(profiles::mc_text::SAVE_FAILED, &name);
                        self.mc_tell(&text, McAct::To(to))
                    }
                }
            }
        };
        Ok(())
    }
}

/// The save points, the autosave and the customize session's unsaved
/// state (were `Game` fields). Every field starts empty (`Default`).
#[derive(Default)]
pub(crate) struct SaveFlow {
    /// Main-menu Customize under way: the profile it set aside for the
    /// unsaved one of the car being customized.
    pub(crate) customize: Option<Option<Profile>>,
    /// MC_Main's create question opened by a save point with no profile
    /// current (0x4F33A0: operation 0x40 → 0x4E8D30(2)): where its answers
    /// go (only Customize's dyno entry, 0x55E5D0, is ported).
    pub(crate) mc_create_then: Option<AfterSave>,
    /// Main-menu Customize's My Cars record (its index) and whether the
    /// session made it from a stock car (0x839BC2, 0x552CD0).
    pub(crate) customize_car: Option<(usize, bool)>,
    /// The save-point snapshot (0x83A9D0 +0x16778): the profile's payload
    /// as it was when a screen with a save point opened (0x52A470, only
    /// with save prompts on or forced), compared and dropped by the save
    /// point (0x52A4D0) or dropped (0x5032C0).
    pub(crate) snapshot: std::cell::RefCell<Option<Vec<u8>>>,
    /// 0x838540: a customize session changed the profile (set by its Back
    /// 0x55BE00, cleared by the car select's Back 0x4B22C0).
    pub(crate) customize_changed: bool,
    /// The same for a customize session started with no profile
    /// (0x552CD0): the session car's parts and performance parts as it
    /// started.
    pub(crate) customize_entry: Option<(std::collections::BTreeMap<u8, u32>, std::collections::BTreeSet<u8>)>,
    /// The car a customize session with no profile made (0x52A720 keeps
    /// it in the memory's My Cars record 0): a profile created at the
    /// save point that follows keeps it (observed under Wine, L2).
    pub(crate) pending_car: Option<career::MyCar>,
    /// An autosave has written the profile this session: the first one
    /// still asked "Do you want to overwrite previously saved game?" under
    /// Wine, the later ones wrote at once (**inferred** to be the memory
    /// card library's state +6 that 0x4E8D30's state 6 → 7 test reads).
    pub(crate) autosaved: bool,
    /// UI_PostRace's save point passed (its continuation runs the action).
    pub(crate) post_race_saved: bool,
    /// The quit question came from UI_PostRace.
    pub(crate) quit_post_race: bool,
    /// The front-end line kind of the screen whose INIT ran Rachel's line
    /// (0x5243E0) — once per opening; cleared when the screen is left
    /// (`Game::tick_fe_lines`).
    pub(crate) fe_screen: Option<u32>,
}
