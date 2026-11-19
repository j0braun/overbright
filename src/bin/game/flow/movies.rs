use crate::*;

/// The movie object in IG_PlayMovie.fng (0x553950 names its movie).
pub(crate) const MOVIE_OBJECT: u32 = 0x6C65_54EE;

/// The texture the clone binds the frame to (not a game hash).
pub(crate) const MOVIE_TEXTURE: u32 = 0x4D4F_5649;

/// IG_PlayMovie (constructor 0x553950, vtable 0x79CB3C, handler
/// 0x55AFF0) over the movie queue 0x8382A0 (four names of 0x40 bytes,
/// the index at 0x83856C), filled by 0x53D720 (mode 6: "scene01", the
/// career intro), 0x53D640 (mode 1), 0x53D7B0 (mode 4) and 0x557E00
/// (mode 2).
pub(crate) struct MovieScreen {
    pub(crate) queue: Vec<String>,
    pub(crate) current: usize,
    /// 0x838568: what follows the queue (0x54AF90).
    pub(crate) mode: u8,
    pub(crate) player: Option<nfsu2::movie::Player>,
    /// +0x4C: the time on the screen, for the 0.5 s before a skip.
    pub(crate) timer: f32,
    /// +0x50: the queue has run out.
    pub(crate) done: bool,
    /// Whether the mixer streams the current movie's sound.
    pub(crate) streaming: bool,
    /// A boot screen's movie (see [`BootStep`]) instead of IG_PlayMovie.
    pub(crate) boot: Option<BootStep>,
    /// Mode 2's shop (0x83634C): the shop its end opens (0x499560).
    pub(crate) shop: Option<usize>,
    /// Mode 1's event (0x863448): the race its end starts (0x499B10).
    pub(crate) event: Option<usize>,
}

/// The boot screens (0x4D9580 builds the list from 0x7F6620 — the same
/// names as 0x7F65E8 — and pushes its first; each screen's end goes to the
/// next through 0x4C4F50): DiscErrorPC (passed when the disc check
/// 0x5BF550 succeeds), MC_Bootup, LS_BlankMovie, LS_EAlogo, LS_THXMovie,
/// LS_PSAMovie, UG_LS_IntroFMV, UG_LS_Splash, MC_Background (shown only
/// when the memory-card state 0x836498 +0x20 is 0 or 0xB, 0x4A9590) and
/// UI_Main (0x4C4F50 starts a career instead when 0x83648C is set).
/// The clone runs the movies and the splash; MC_Bootup and MC_Background
// prov: open game.unique_part_label
/// are **open** (the memory-card / profile states are not traced).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BootStep {
    /// LS_BlankMovie (0x4A8AD0 → 0x4A8A30): "blank" on 0x58BCF5B6.
    Blank,
    /// LS_EAlogo (0x4A89D0 → 0x4A8930): "ealogo" on 0x58BCF5B6.
    EaLogo,
    /// LS_THXMovie (0x4A8D90 → 0x4A8CC0): "THX_logo_fr" on 0x8BA694A3.
    Thx,
    /// LS_PSAMovie (0x4A8C60 → 0x4A8B30): "PSA" on 0xF2754103.
    Psa,
    /// UG_LS_IntroFMV (0x4C52D0 → 0x4A8860): "FMVOpening" on 0x72CF9F38.
    Intro,
    /// UG_LS_Splash (0x4C5500 → 0x4A90F0).
    Splash,
}

impl BootStep {
    pub(crate) const ALL: [BootStep; 6] =
        [BootStep::Blank, BootStep::EaLogo, BootStep::Thx, BootStep::Psa, BootStep::Intro, BootStep::Splash];

    /// The package, its movie object and the movie's name.
    pub(crate) fn movie(self) -> Option<(&'static str, u32, &'static str)> {
        Some(match self {
            BootStep::Blank => ("LS_BlankMovie", 0x58BC_F5B6, "blank"),
            BootStep::EaLogo => ("LS_EAlogo", 0x58BC_F5B6, "ealogo"),
            BootStep::Thx => ("LS_THXMovie", 0x8BA6_94A3, "THX_logo_fr"),
            BootStep::Psa => ("LS_PSAMovie", 0xF275_4103, "PSA"),
            BootStep::Intro => ("UG_LS_IntroFMV", 0x72CF_9F38, "FMVOpening"),
            BootStep::Splash => return None,
        })
    }

    /// The handlers' skips: the blank, EA and THX screens (0x4C53A0) only
    /// end with their movie (0xC3960EB9); the intro (0x4C5330) also on
    /// Accept (0x406415E3) or Start (0xB5AF2461), without the 0.5 s wait
    /// of IG_PlayMovie; the PSA (0x4C53D0) on them only when the
    /// memory-card state's +5 is set: 0x4A9C40's "the profile folder has
    /// an entry" flag (0x4C622A; Wine: 0 with no profile
    /// on disk, 1 with one, set before blank.vp6 opens). Not modelled here:
    /// the PSA movie is missing from the reference install.
    #[allow(dead_code)] // the native rule; Enter skips every movie (agreed deviation)
    pub(crate) fn skippable(self) -> bool {
        self == BootStep::Intro
    }

    pub(crate) fn next(self) -> Option<BootStep> {
        BootStep::ALL.iter().position(|&s| s == self).and_then(|i| BootStep::ALL.get(i + 1)).copied()
    }
}

impl MovieScreen {
    pub(crate) fn new(root: &std::path::Path, mode: u8, names: &[&str]) -> MovieScreen {
        let mut m = MovieScreen {
            queue: names.iter().map(|n| n.to_string()).collect(),
            current: 0,
            mode,
            player: None,
            timer: 0.0,
            done: false,
            streaming: false,
            boot: None,
            shop: None,
            event: None,
        };
        m.open(root);
        m
    }

    /// Start the current name (0x566250: MOVIES path by 0x51C740; a
    /// missing file ends it at once with 0xC3960EB9). The file is the one
    /// `nfsu2::movie::path` gives (`game_assets::resolve_movie`).
    pub(crate) fn open(&mut self, root: &std::path::Path) {
        self.streaming = false;
        let found = self.queue.get(self.current).map(|n| nfsu2::game_assets::resolve_movie(root, n));
        if let Some(r) = found.as_ref().filter(|r| r.file.is_none()) {
            // Skipped as the original skips it; the names tried, for reports.
            log::info!("movie {}: no file (tried {})", r.want, r.tried.join(", "));
        }
        self.player = found.and_then(|r| r.file).and_then(|p| {
            match nfsu2::movie::Player::open(&p) {
                Ok(p) => Some(p),
                Err(e) => {
                    log::warn!("movie {}: {e}", p.display());
                    None
                }
            }
        });
    }

    /// 0x55AFF0 on 0xC3960EB9 (the movie ended) or a skip: the next name,
    /// or the queue is done (0x8383A8 = 1, +0x50 = 1).
    pub(crate) fn next(&mut self, root: &std::path::Path) {
        self.player = None;
        self.current += 1;
        if self.current < self.queue.len() {
            self.open(root);
        } else {
            self.done = true;
        }
    }
}

/// The car select again (Back from the transmission question,
/// 0x1FAB5998, stays on it).
pub(crate) fn lot_again_qr(lot: &CarLot) -> Mode {
    Mode::Menu(Menu::CarLot(lot.clone()))
}

impl Game {
    /// A boot screen (see [`BootStep`]): a movie screen, or the splash.
    pub(crate) fn boot_screen(&mut self, step: BootStep) -> Mode {
        // Skip Intro Movies / Skip Title Screen (Enhancements, opt-in,
        // independent; Off = native).
        let (skip_movies, skip_title) = (self.settings.enh.skips_movies(), self.settings.enh.skips_title());
        let Some((_, _, name)) = step.movie() else {
            // The title's constructor (0x4A90F0) starts the music with −2
            // (0x480C30): its first song is song 0.
            if let Some(a) = self.audio.as_mut() {
                a.streams.title_music(&a.mixer);
            }
            // The main menu's garage scene made while the title is up.
            self.garage_ahead_start();
            if skip_title {
                // The title's end without it (its Accept, 0x4C4F50): the
                // profile step, the music started as the title starts it.
                log::info!("boot: the title screen skipped (Skip Title Screen)");
                return self.boot_profile_step();
            }
            // One line in the log: the start reached the title screen (the
            // Windows smoke's windowed start waits for it).
            log::info!("boot: the title screen (UG_LS_Splash)");
            return Mode::Menu(Menu::Splash(screens::Splash::new(self.now(), self.real_now(), self.fe.splash_seen)));
        };
        // Skipped as a missing movie file is (0x566250): passed in the same
        // tick, the next step with it.
        let names: &[&str] = if skip_movies {
            log::info!("boot: {name} skipped (Skip Intro Movies)");
            &[]
        } else {
            &[name]
        };
        let mut m = MovieScreen::new(&self.game_data.root, 0, names);
        m.boot = Some(step);
        Mode::Menu(Menu::Movie(m))
    }

    /// The career start past MC_Main or on Resume (0x495A30), before the
    /// D-Day intro: until the career's intro movie has played (0x86347C,
    /// career +0x873C, saved with it), 0x53D720 queues "scene01" in mode 6
    /// and pushes IG_PlayMovie.fng; else D-Day starts at once (0x5410F0,
    /// 0x5404B0), its script ended by the city start (0x5404FE).
    pub(crate) fn career_intro(&mut self) -> Result<()> {
        // 0x495A30: game mode 1.
        self.race_flow.game_mode = 1;
        if self.profile.as_ref().is_some_and(|p| p.intro_played) {
            // 0x495A93: 0x863484 (career +0x8744) = 1, then 0x5404B0.
            if let Some(p) = self.profile.as_mut() {
                p.explore_set = true;
            }
            if let Some(p) = self.profile.as_mut().filter(|p| p.intro == Some(career::DDAY_EVENT_A)) {
                p.hints.dday_a();
            }
            return self.explore(None);
        }
        // A career whose movie has not played has its D-Day script idle:
        // a new career and every load start from the career reset, which
        // zeroes it (0x532A43 in 0x532840; the load 0x5344E0 runs it
        // first), and the script only starts after the movie. The clone
        // keeps the script on the game: it starts again here, and the start
        // scene may play again.
        self.career_world.dday = Default::default();
        self.career_world.dday_prompt = None;
        self.career_world.career_scene_played = false;
        self.mode = Mode::Menu(Menu::Movie(MovieScreen::new(&self.game_data.root, 6, &["scene01"])));
        if let Some(a) = self.audio.as_ref() {
            a.mixer.set_stream(None);
        }
        Ok(())
    }

    /// IG_PlayMovie's tick (0x55AFF0, 0xC98356BA): the timer, the movie,
    /// and once the queue is done what its mode starts (0x54AF90).
    pub(crate) fn movie_tick(&mut self, dt: f32) -> Result<()> {
        let Mode::Menu(Menu::Movie(m)) = &mut self.mode else { return Ok(()) };
        // prov: inferred game.movie_tick
        // **Inferred**: the tick adds 0x865194 (the frame's step); the
        // clone adds the frame's time.
        m.timer += dt;
        if let Some(p) = m.player.as_mut() {
            p.advance(dt);
            if let Some((channels, rate, samples)) = p.audio(0.5) {
                if let Some(a) = self.audio.as_ref() {
                    if !m.streaming {
                        a.mixer.set_stream(Some((channels, rate)));
                        // The movie voice's volume at its first frame
                        // (0x531E00 → 0x525E20): its table byte × master ×
                        // speech ("Speech & Movies").
                        let byte = m.queue.get(m.current).and_then(|n| nfsu2::movie::volume_byte(&self.game_data.root, n));
                        a.mixer.set_stream_gain(nfsu2::audio::mixer::movie_volume(
                            byte,
                            self.settings.master,
                            self.settings.speech,
                        ));
                        m.streaming = true;
                    }
                    a.mixer.push_stream(&samples);
                }
            }
            if p.finished() {
                // The movie's end (0xC3960EB9).
                m.next(&self.game_data.root);
            }
        } else if !m.done {
            m.next(&self.game_data.root);
        }
        if !m.done {
            return Ok(());
        }
        let mode = m.mode;
        let boot = m.boot;
        let shop = m.shop;
        let event = m.event;
        if let Some(a) = self.audio.as_ref() {
            a.mixer.set_stream(None);
        }
        if let Some(step) = boot {
            // A boot screen with no movie file (`nfsu2::movie::path`) ends as
            // it opens (0x566250 sends 0xC3960EB9 at once): the original,
            // missing ealogo_fr, THX_logo_fr and PSA_fr, was on FMVOpening
            // 4 ms after asking for them (Wine's file trace), so such
            // screens pass in the same tick.
            let mut next = step.next().unwrap_or(BootStep::Splash);
            loop {
                self.mode = self.boot_screen(next);
                match &self.mode {
                    Mode::Menu(Menu::Movie(m)) if m.player.is_none() => next = next.next().unwrap_or(BootStep::Splash),
                    _ => break,
                }
            }
            return Ok(());
        }
        // 0x54AF90 case 6: the D-Day event (A, or B once A is won) and
        // the city; 0x86347C = 1. Modes 1, 2 and 4 below.
        if mode == 6 {
            // 0x5410F0 → 0x524250: DDAY_EVENT_A current posts Rachel's
            // reveal and nag; 0x5404B0 runs before 0x86347C is set
            // (0x54AFD5, 0x54AFDA).
            if let Some(p) = self.profile.as_mut().filter(|p| p.intro == Some(career::DDAY_EVENT_A)) {
                p.hints.dday_a();
            }
            self.explore(None)?;
            if let Some(p) = self.profile.as_mut() {
                p.intro_played = true;
            }
        } else if let (2, Some(index)) = (mode, shop) {
            // 0x54AF90 case 2 → 0x499560: the shop's front end.
            self.mode = self.shop_front_end(index);
        } else if let (1, Some(index)) = (mode, event) {
            // 0x54AF90 case 1 → 0x499B10: the race (0x541170).
            self.start_race(index, false, false)?;
        } else if mode == 4 {
            // 0x54AF90 case 4 → 0x5F41F0(0xB): back to the city.
            self.explore_after_race()?;
        } else {
            self.mode = self.icon_menu(MenuKind::Main);
        }
        Ok(())
    }

    /// A key on IG_PlayMovie (0x55AFF0): Accept (0x406415E3, the package's
    /// accept) or Start (PAD_START 0xB5AF2461) skip the movie once the
    /// screen has been up more than 0.5 s (0x784260).
    pub(crate) fn movie_key(&mut self, key: Key) {
        let Mode::Menu(Menu::Movie(m)) = &mut self.mode else { return };
        // Agreed deviation (docs/native-deviations.md "Skipping scenes"): Enter skips every movie, the
        // boot logos and the PSA included (natively only the intro FMV and
        // IG_PlayMovie, after 0.5 s, take the skip; see `skippable`).
        let skip = match m.boot {
            Some(_) => true,
            None => m.timer > 0.5,
        };
        if matches!(key, Key::Accept | Key::Start) && skip {
            if let Some(a) = self.audio.as_ref() {
                a.mixer.set_stream(None);
            }
            m.next(&self.game_data.root);
        }
    }
}
