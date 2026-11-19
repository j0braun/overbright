//! The launcher's window: winit for the window and input, softbuffer for
//! its pixels (no GPU), gilrs for gamepads (the d-pad / left stick move,
//! A confirms, B goes back, LB / RB step through the controls). Play
//! hides the window and starts the game ([`super::run`]); the window
//! comes back on the failure screen when the game fails, and closes when
//! it ends normally. `game --probe` runs in the background meanwhile.

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as WKey, ModifiersState, NamedKey};
use winit::window::{CursorIcon, Window, WindowId};

use super::draw::Canvas;
use super::font::Fonts;
use super::model::{Env, Failure, ProbeState};
use super::platform_deck::{self as deck, Dedup, InputKind, Source};
use super::probe::Probe;
use super::run::{Ended, Running};
use super::screens::{App, Outcome, Screen};
use super::ui::{Input, Key, Kind, render_on};

type Surface = softbuffer::Surface<Rc<Window>, Rc<Window>>;

/// A held pad direction: the first repeat after this, then one every
/// [`REPEAT_EVERY`] (about a keyboard's).
const REPEAT_DELAY: Duration = Duration::from_millis(400);
const REPEAT_EVERY: Duration = Duration::from_millis(110);

struct Run<'e> {
    app: App<'e>,
    fonts: Fonts,
    window: Option<Rc<Window>>,
    surface: Option<Surface>,
    canvas: Canvas,
    mods: ModifiersState,
    mouse: (f32, f32),
    pad: Option<gilrs::Gilrs>,
    /// The left stick's last direction (−1, 0, 1 per axis) for edges.
    stick: (i8, i8),
    /// A direction held on the pad (d-pad or stick) and when it repeats
    /// next, as a held arrow key does.
    held: Option<(Key, Instant)>,
    outcome: Outcome,
    error: Option<String>,
    game: GameStart,
    running: Option<Running>,
    probe: Option<std::sync::mpsc::Receiver<Result<Probe, String>>>,
    /// The game folder the last probe was run with (a new folder: run
    /// again, so the probe's result belongs to the chosen folder).
    probed_root: Option<std::path::PathBuf>,
    /// Run `game --probe` at all (the real machine).
    probing: bool,
    /// The prepare step's run (`game --prepare`), while it runs.
    prepare: Option<super::prepare::Job>,
    /// A texture-package import running on its worker thread.
    import: Option<std::sync::mpsc::Receiver<ImportMsg>>,
    /// The HUD mod's conversion running on its worker thread.
    hud_job: Option<std::sync::mpsc::Receiver<HudMsg>>,
    /// The system folder dialog open on a worker (its answer).
    dialog: Option<std::sync::mpsc::Receiver<super::model::Picked>>,
    /// The backdrop: the picture from the game folder (read on a worker
    /// thread), when it arrived (it fades in), the folder it is for, and
    /// the composed page for the canvas's size and fade step.
    picture: Option<super::backdrop::Picture>,
    /// The backdrop's scene the page was composed for.
    scene: super::backdrop::Scene,
    picture_rx: Option<std::sync::mpsc::Receiver<Option<super::backdrop::Picture>>>,
    picture_at: Option<Instant>,
    picture_root: Option<std::path::PathBuf>,
    page: (u32, u32, u8, Vec<u8>),
    /// The last screen written to the log (`launcher::screen` at debug
    /// level: what the start tests read).
    traced: String,
    /// Files dropped on the window, handed on together once the drop's
    /// events are in (winit sends one per file).
    dropped: Vec<std::path::PathBuf>,
    /// The pointer shape set last.
    pointer: super::ui::Pointer,
    /// The first picture was shown (logged once).
    drawn: bool,
    /// One press counted once from the keyboard and a pad together
    /// (Steam Input's desktop layout).
    dup: Dedup,
    /// The pointer is hidden (the last input was a key or a pad).
    cursor_hidden: bool,
    /// A text field had focus after the last input (gamescope: Steam's
    /// keyboard opens when one gets it).
    field_focused: bool,
}

/// How long the game's picture takes to fade in.
const FADE: Duration = Duration::from_millis(300);

/// What the import worker reports.
enum ImportMsg {
    Progress(String),
    Done(Vec<crate::texture_mods::Outcome>),
}

/// A widget's texts as the trace has them: what is drawn, and after a
/// shortened one its whole value (`TextBox::full`) in `⟨…⟩`.
pub fn trace_texts(w: &super::ui::Widget) -> Vec<String> {
    w.texts
        .iter()
        .filter(|x| !x.text.is_empty())
        .map(|x| match &x.full {
            Some(full) => format!("{} ⟨{full}⟩", x.text),
            None => x.text.clone(),
        })
        .collect()
}

/// How long "Shaders ready" stays on the main screen.
pub const SHADERS_READY_FOR: Duration = Duration::from_secs(6);

/// What the HUD mod's conversion worker reports.
enum HudMsg {
    Progress(String),
    Done(super::model::HudOutcome),
}

/// How Play starts the game.
#[derive(Clone, Debug)]
pub struct GameStart {
    pub exe: std::path::PathBuf,
    pub args: Vec<String>,
    /// Whether the launcher writes its log of the game, into the log folder
    /// (`dirs::logs`, looked up at each start: portable mode may have been
    /// turned on since).
    pub logs: bool,
    /// The game folder to play (given to the game as `NFSU2_ROOT`).
    pub root: Option<std::path::PathBuf>,
}

impl GameStart {
    pub fn start(&self) -> Result<Running, String> {
        let logs = self.logs.then(super::dirs::logs);
        super::run::start(&self.exe, &self.args, logs.as_deref(), self.root.as_deref())
    }
}

/// The failure screen's data for an ended run.
pub fn failure_of(e: &Ended) -> Failure {
    Failure { kind: e.kind(), during_start: e.during_start(), how: e.how(), tail: e.tail.clone(), log: e.log.clone() }
}

impl Run<'_> {
    fn send(&mut self, input: Input) {
        let (focus0, screen0) = (self.app.ui.focused(&self.app.st), self.app.screen);
        let out = self.app.input(&input, &self.fonts);
        // The launcher's own sounds (off unless the player turned them on).
        if super::sounds::allowed() && !matches!(input, Input::MouseMove(..)) {
            if out == Outcome::Play || self.app.screen != screen0 {
                super::sounds::play(super::sounds::Cue::Press);
            } else if self.app.ui.focused(&self.app.st) != focus0 {
                super::sounds::play(super::sounds::Cue::Move);
            }
        }
        match out {
            Outcome::Play => self.play(),
            Outcome::Stay => {}
            Outcome::Restart => self.outcome = Outcome::Restart,
            o => self.outcome = o,
        }
        // Gamescope has no keyboard of its own: Steam's shows when a text
        // field gets focus by key or pad (a click opens nothing: a mouse
        // there means a keyboard is near).
        let field = self.focused_is_field();
        if field && !self.field_focused && deck::is_gamescope() && deck::last_input() != InputKind::Mouse {
            deck::open_keyboard();
        }
        self.field_focused = field;
        if let Some(w) = &self.window {
            // The pointer's shape for what is under it now.
            let (x, y) = self.mouse;
            let p = self.app.ui.pointer_at(x, y, &self.app.st);
            if p != self.pointer {
                self.pointer = p;
                w.set_cursor(match p {
                    super::ui::Pointer::Default => CursorIcon::Default,
                    super::ui::Pointer::Hand => CursorIcon::Pointer,
                    super::ui::Pointer::Text => CursorIcon::Text,
                });
            }
            w.request_redraw();
        }
    }

    /// Hides the window and starts the game with the folder shown. A
    /// prepare run still going is stopped first: what it made so far stays
    /// (each cache file whole, the driver's programs as made), and the game
    /// makes the rest as it does without the step (docs/developing/prepare.md).
    fn play(&mut self) {
        let m = &mut self.app.model.prepare;
        m.wanted = false;
        if let Some(job) = self.prepare.take() {
            log::info!("prepare: stopped (Play)");
            job.stop();
            m.state = super::prepare::State::Idle;
        }
        m.background = false;
        self.game.root = self.app.model.root.clone();
        // The Dev page's start with the editor tools: those arguments this once.
        let extra = std::mem::take(&mut self.app.model.play_args);
        let mut game = self.game.clone();
        game.args.extend(extra);
        match game.start() {
            Ok(r) => {
                log::info!("game started (pid {})", r.id());
                if let Some(w) = &self.window {
                    w.set_visible(false);
                }
                self.running = Some(r);
            }
            Err(e) => self.show_failure(Failure::not_started(e)),
        }
    }

    fn show_failure(&mut self, f: Failure) {
        self.app.motion.play_reset();
        self.app.model.starting = false;
        self.app.model.failure = Some(f);
        self.app.model.failure_details = false;
        self.app.go(Screen::Failure);
        self.app.rebuild(&self.fonts);
        if let Some(w) = &self.window {
            w.set_visible(true);
            w.focus_window();
            w.request_redraw();
        }
    }

    /// The running game: its end judged.
    fn poll_game(&mut self) {
        let Some(r) = self.running.as_mut() else { return };
        let Some(e) = r.poll() else { return };
        self.running = None;
        if e.failed() {
            log::warn!("the game ended with {} after {:.1} s", e.how(), e.secs);
            eprintln!("overbright: showing the failure screen ({})", e.how());
            self.show_failure(failure_of(&e));
        } else {
            self.outcome = Outcome::Quit;
        }
    }

    /// Runs the probe again when the game folder changed (its result
    /// belongs to the folder it was run with).
    fn reprobe(&mut self) {
        let again = std::mem::take(&mut self.app.model.probe_again);
        if !self.probing || (!again && (self.app.model.root.is_none() || self.app.model.root == self.probed_root)) {
            return;
        }
        let root = self.app.model.root.clone();
        log::info!("probe: the game folder changed; checking this computer again with it");
        self.probe = Some(super::probe::spawn(self.game.exe.clone(), root.clone()));
        self.probed_root = root;
        self.app.model.probe = ProbeState::Pending;
        self.app.rebuild(&self.fonts);
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// The prepare step (`game --prepare`): offered by itself once the
    /// hardware check says nothing is prepared for this build, adapter and
    /// driver; started and stopped as the screens ask; its lines shown.
    fn poll_prepare(&mut self) {
        // `OVERBRIGHT_NO_PREPARE` (set at all): never offered by itself
        // ("Prepare again" still works).
        let offer = self.probing && std::env::var_os("OVERBRIGHT_NO_PREPARE").is_none();
        let mut changed = offer && self.prepare.is_none() && self.running.is_none() && self.app.offer_prepare();
        let m = &mut self.app.model.prepare;
        if std::mem::take(&mut m.stop)
            && let Some(job) = self.prepare.take()
        {
            log::info!("prepare: stopped");
            job.stop();
        }
        if std::mem::take(&mut m.wanted) && self.prepare.is_none() {
            match super::prepare::start(&self.game.exe, self.app.model.root.as_deref()) {
                Ok(job) => {
                    log::info!("prepare: started");
                    m.started = Some(std::time::Instant::now());
                    m.done_at = None;
                    self.prepare = Some(job);
                }
                Err(e) => m.state = super::prepare::State::Failed(e),
            }
            changed = true;
        }
        if let Some(job) = self.prepare.as_mut() {
            let (state, ended) = job.poll(m.state.clone());
            if state != m.state {
                log::info!("prepare: {state:?}");
                if state == super::prepare::State::Done {
                    m.done_at = Some(std::time::Instant::now());
                }
                m.state = state;
                changed = true;
            }
            if ended {
                self.prepare = None;
            }
        }
        if changed {
            self.app.rebuild(&self.fonts);
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }

    fn poll_probe(&mut self) {
        let Some(rx) = &self.probe else { return };
        let got = match rx.try_recv() {
            Ok(r) => r,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err("the check stopped".into()),
        };
        self.probe = None;
        let m = &mut self.app.model;
        m.probe = match got {
            Ok(p) => {
                log::info!(
                    "probe: {}; monitor {:?}, {} modes{}; first-run values {}",
                    p.gpu_line().unwrap_or_else(|| p.error.clone().unwrap_or_default()),
                    p.monitor,
                    p.monitor_modes.len(),
                    if p.exclusive { "" } else { ", no exclusive full screen" },
                    match &p.first_run {
                        Some(f) => format!("{}x{} detail {}", f.resolution.0, f.resolution.1, f.detail),
                        None => "none (no usable game folder)".into(),
                    }
                );
                // The monitor as the game sees it (the Resolution row's
                // lists per display path).
                if let Some(info) = p.monitor_info() {
                    m.monitor = Some(info);
                }
                // Before the game's first run: the display values its
                // first start would choose (nothing is written yet).
                if let (false, Some(f)) = (m.settings_file, &p.first_run)
                    && m.choices == m.saved
                {
                    m.choices.resolution = f.resolution;
                    m.choices.fullscreen = f.fullscreen;
                    m.saved = m.choices.clone();
                }
                let fitted = crate::render::display::fit_mode(&m.modes(), m.choices.resolution);
                if fitted != m.choices.resolution {
                    log::info!(
                        "resolution {}x{} is not offered on this monitor; {}x{} shown",
                        m.choices.resolution.0,
                        m.choices.resolution.1,
                        fitted.0,
                        fitted.1
                    );
                    m.choices.resolution = fitted;
                }
                ProbeState::Ready(p)
            }
            Err(e) => {
                log::warn!("probe: {e}");
                ProbeState::Failed(e)
            }
        };
        self.app.rebuild(&self.fonts);
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Reads the game's picture for the backdrop when the game folder is
    /// known or changed (a worker thread); fades it in when it arrives.
    fn poll_backdrop(&mut self) {
        if !self.probing || !super::backdrop::FROM_THE_GAME {
            return;
        }
        if self.app.model.root != self.picture_root {
            self.picture_root = self.app.model.root.clone();
            self.picture_rx = self.picture_root.clone().map(|root| {
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let t = Instant::now();
                    let p = super::backdrop::load(&root);
                    log::info!(
                        "backdrop: {} ({:.0} ms)",
                        if p.is_some() { "the game's title picture" } else { "Overbright's own (no picture)" },
                        t.elapsed().as_secs_f32() * 1000.0
                    );
                    let _ = tx.send(p);
                });
                rx
            });
            if self.picture_root.is_none() {
                self.picture = None;
                self.picture_at = None;
                self.page.3.clear();
            }
        }
        if let Some(rx) = &self.picture_rx {
            match rx.try_recv() {
                Ok(p) => {
                    self.picture_at = p.as_ref().map(|_| Instant::now());
                    self.picture = p;
                    self.picture_rx = None;
                    self.page.3.clear();
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(_) => self.picture_rx = None,
            }
        }
        if self.picture_at.is_some_and(|t| t.elapsed() < FADE + Duration::from_millis(40))
            && let Some(w) = &self.window
        {
            w.request_redraw();
        }
    }

    /// Opens the system folder dialog asked for on a worker thread (the
    /// portal's and zenity's calls block until the player answers; the
    /// window keeps drawing meanwhile) and hands its answer to the screen.
    fn poll_dialog(&mut self) {
        if self.dialog.is_none()
            && let Some(ask) = self.app.model.dialog.as_mut().filter(|a| !a.open)
        {
            ask.open = true;
            let (title, start) = (ask.title.clone(), ask.start.clone());
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(super::model::system_folder_dialog(&title, start.as_deref()));
            });
            self.dialog = Some(rx);
        }
        let Some(rx) = &self.dialog else { return };
        let picked = match rx.try_recv() {
            Ok(p) => p,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => super::model::Picked::NoDialog,
        };
        self.dialog = None;
        if let Some(ask) = self.app.model.dialog.clone() {
            log::info!("folder dialog: {picked:?}");
            self.app.dialog_done(ask.purpose, ask.start, picked);
        }
        self.app.rebuild(&self.fonts);
        if let Some(w) = &self.window {
            w.focus_window();
            w.request_redraw();
        }
    }

    /// Starts the HUD mod's queued conversion on a worker thread; reports
    /// its progress and its end to the Mods page.
    fn poll_hud(&mut self) {
        if self.hud_job.is_none() && self.app.model.mods.hud_queued && self.app.env.background_import() {
            self.app.model.mods.hud_queued = false;
            let parts = self.app.model.mods.hud_parts.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let out = super::model::convert_hud(&parts, &|t| {
                    let _ = tx.send(HudMsg::Progress(t));
                });
                let _ = tx.send(HudMsg::Done(out));
            });
            self.hud_job = Some(rx);
        }
        let Some(rx) = &self.hud_job else { return };
        let mut changed = false;
        loop {
            match rx.try_recv() {
                Ok(HudMsg::Progress(t)) => {
                    self.app.model.mods.hud_busy = Some(t);
                    changed = true;
                }
                Ok(HudMsg::Done(out)) => {
                    for (_, l) in &out.lines {
                        log::info!("mods: {l}");
                    }
                    self.app.finish_hud(out);
                    self.hud_job = None;
                    changed = true;
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.app.finish_hud(super::model::HudOutcome {
                        lines: vec![(super::ui::Mark::Error, "The conversion stopped unexpectedly.".into())],
                        dir: None,
                    });
                    self.hud_job = None;
                    changed = true;
                    break;
                }
            }
        }
        if changed {
            self.app.rebuild(&self.fonts);
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }

    /// Starts a queued import on a worker thread; reports its progress
    /// and its end to the Mods page.
    fn poll_import(&mut self) {
        if self.import.is_none() && !self.app.model.mods.queued.is_empty() && self.app.env.background_import() {
            let paths = std::mem::take(&mut self.app.model.mods.queued);
            let root = self.app.model.root.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let out = super::model::import_paths(&paths, root.as_deref(), &|t| {
                    let _ = tx.send(ImportMsg::Progress(t));
                });
                let _ = tx.send(ImportMsg::Done(out));
            });
            self.import = Some(rx);
        }
        let Some(rx) = &self.import else { return };
        let mut changed = false;
        loop {
            match rx.try_recv() {
                Ok(ImportMsg::Progress(t)) => {
                    self.app.model.mods.busy = Some(t);
                    changed = true;
                }
                Ok(ImportMsg::Done(out)) => {
                    for o in &out {
                        log::info!("mods: {}", o.line);
                    }
                    self.app.finish_import(out);
                    self.import = None;
                    changed = true;
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.app.finish_import(Vec::new());
                    self.app.model.mods.results =
                        vec![(super::ui::Mark::Error, "The import stopped unexpectedly.".into())];
                    self.import = None;
                    changed = true;
                    break;
                }
            }
        }
        if changed {
            self.app.rebuild(&self.fonts);
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }

    /// The device just used: the legends follow it, the pointer hides on
    /// a key or a pad and shows on a mouse movement.
    fn used(&mut self, kind: InputKind) {
        let changed = deck::note_input(kind);
        self.app.st.input = kind;
        let hide = kind != InputKind::Mouse;
        if let Some(w) = &self.window {
            if hide != self.cursor_hidden {
                w.set_cursor_visible(!hide);
            }
            if changed {
                w.request_redraw();
            }
        }
        self.cursor_hidden = hide;
    }

    fn focused_is_field(&self) -> bool {
        let ui = &self.app.ui;
        ui.focused(&self.app.st).and_then(|f| ui.get(f)).is_some_and(|w| matches!(w.kind, Kind::TextField { .. }))
    }

    /// The screen's texts at debug level (`RUST_LOG=launcher::screen=debug`)
    /// when they changed: one line per widget with its id and whether it
    /// can be used; the start tests read the screens from it.
    fn trace(&mut self) {
        if !log::log_enabled!(target: "launcher::screen", log::Level::Debug) {
            return;
        }
        let mut t = format!("screen {}\n", self.app.screen.name());
        for w in &self.app.ui.widgets {
            // The legends: each one's box (where a test clicks it), as a button
            // named `legend:<button>` with its words.
            if let Kind::Legends { actions } = &w.kind {
                let u = self.app.ui.metrics.map_or(1.0, |m| m.u);
                let (_, items) = super::legends::items(&self.fonts, w.rect, self.app.st.input, actions, u);
                for (btn, r) in items {
                    let label = actions.iter().find(|a| a.btn == btn).map_or("", |a| a.label.as_str());
                    t += &format!(
                        "  [button legend:{} @{:.0},{:.0} {:.0}x{:.0}] {label}\n",
                        format!("{btn:?}").to_lowercase(),
                        r.x,
                        r.y - self.app.st.scroll,
                        r.w,
                        r.h
                    );
                }
                continue;
            }
            let texts = trace_texts(w);
            if texts.is_empty() {
                continue;
            }
            let kind = match w.kind {
                Kind::Label => "label",
                Kind::Rule => "rule",
                Kind::Panel => "panel",
                Kind::Card | Kind::Logo | Kind::Accent | Kind::Bar { .. } => "card",
                Kind::Status { mark, .. } => match mark {
                    super::ui::Mark::Ok => "ok",
                    super::ui::Mark::Warn => "warn",
                    super::ui::Mark::Info => "info",
                    super::ui::Mark::Busy => "busy",
                    super::ui::Mark::Error => "error",
                },
                Kind::Button { .. } => "button",
                Kind::Cycle { .. } => "cycle",
                Kind::Checkbox { .. } => "checkbox",
                Kind::List { .. } => "list",
                Kind::TextField { .. } => "field",
                Kind::Link => "link",
                Kind::Play => "play",
                Kind::SumRow { .. } => "row",
                Kind::PageTab { .. } => "page",
                Kind::Rail { .. } => "rail",
                Kind::ValueRow { .. } => "value",
                Kind::Toggle { .. } => "toggle",
                Kind::Message { .. } => "message",
                Kind::Describe => "describe",
                Kind::Scrim => "scrim",
                Kind::Dialog => "dialog",
                Kind::Legends { .. } => "legends",
                Kind::Glyph { .. } => "glyph",
                Kind::Picture { .. } => "picture",
            };
            // A control: its id, "(off)" when it cannot be used, and its
            // box in window pixels (where a test clicks it).
            let id = w.id.map_or(String::new(), |i| {
                let r = w.rect;
                format!(
                    " {i}{} @{:.0},{:.0} {:.0}x{:.0}",
                    if w.enabled { "" } else { " (off)" },
                    r.x,
                    r.y - self.app.st.scroll,
                    r.w,
                    r.h
                )
            });
            t += &format!("  [{kind}{id}] {}\n", texts.join(" "));
            // A list's rows: where it stands and the rows shown, in order
            // (a test clicks row i at its box's top + pad + (i + ½) × row).
            if let Kind::List { len, selected, first, row_h, pad } = &w.kind {
                let rows: Vec<&str> = w
                    .texts
                    .iter()
                    .filter(|x| x.color != super::draw::theme::DIM)
                    .map(|x| x.full.as_deref().unwrap_or(&x.text))
                    .collect();
                let u = self.app.ui.metrics.map_or(1.0, |m| m.u);
                let sc = self.app.st.scroll;
                let bar = super::ui::list_bar(w, u).map_or(String::new(), |(tr, th)| {
                    format!(
                        " bar={:.0},{:.0},{:.0},{:.0} thumb={:.0},{:.0}",
                        tr.x,
                        tr.y - sc,
                        tr.w,
                        tr.h,
                        th.y - sc,
                        th.h
                    )
                });
                t += &format!(
                    "    rows first={first} shown={} sel={selected} len={len} row_h={row_h:.0} pad={pad:.0}{bar}: {}\n",
                    super::ui::list_rows(w),
                    rows.join(" | ")
                );
            }
        }
        if t != self.traced {
            log::debug!(target: "launcher::screen", "{t}");
            self.traced = t;
        }
    }

    fn draw(&mut self) {
        self.trace();
        let (Some(w), Some(surface)) = (&self.window, &mut self.surface) else { return };
        let size = w.inner_size();
        let (Some(nw), Some(nh)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else { return };
        if self.canvas.width() != size.width || self.canvas.height() != size.height {
            self.canvas = Canvas::new(size.width, size.height);
        }
        if let Err(e) = surface.resize(nw, nh) {
            self.error = Some(format!("window buffer: {e}"));
            return;
        }
        // The page: Overbright's own at once, the game's picture faded in
        // (in 10 steps) once it was read.
        let fade = self.picture_at.map_or(0.0, |t| (t.elapsed().as_secs_f32() / FADE.as_secs_f32()).min(1.0));
        let step = (fade * 10.0).round() as u8;
        let (w, h) = (self.canvas.width(), self.canvas.height());
        if (self.page.0, self.page.1, self.page.2) != (w, h, step) || self.page.3.is_empty() {
            let pic = self.picture.as_ref().filter(|_| step > 0);
            let scene = self.app.ui.scene;
            self.page = (w, h, step, super::backdrop::backdrop_a(w, h, scene, pic).as_ref().clone());
            self.scene = scene;
        }
        if self.scene != self.app.ui.scene {
            let pic = self.picture.as_ref().filter(|_| step > 0);
            self.scene = self.app.ui.scene;
            self.page.3 = super::backdrop::backdrop_a(w, h, self.scene, pic).as_ref().clone();
        }
        // The motion of this frame (the page slide, Play's glow).
        let now = Instant::now();
        if let Some(m) = self.app.ui.metrics {
            self.app.motion.set_unit(m.u * 0.9375);
        }
        self.app.st.motion = self.app.motion.frame(now);
        render_on(&self.app.ui, &self.app.st, &self.fonts, &mut self.canvas, &self.page.3);
        match surface.buffer_mut() {
            Ok(mut buf) => {
                self.canvas.to_xrgb(&mut buf);
                if let Err(e) = buf.present() {
                    self.error = Some(format!("window present: {e}"));
                } else if !self.drawn {
                    self.drawn = true;
                    log::info!("window: first picture shown");
                    // The checks' way to see a panic reported (the log, the
                    // message box, exit code 101; docs/launcher.md).
                    if std::env::var_os("OB_LAUNCHER_PANIC_TEST").is_some() {
                        panic!("a test panic after the first picture (OB_LAUNCHER_PANIC_TEST)");
                    }
                }
            }
            Err(e) => self.error = Some(format!("window buffer: {e}")),
        }
    }

    fn poll_pad(&mut self) {
        let Some(g) = &mut self.pad else { return };
        let name = g.gamepads().find(|(_, p)| p.is_connected()).map(|(_, p)| p.name().to_string());
        if name != self.app.model.controller {
            self.app.model.controller = name;
            self.app.rebuild(&self.fonts);
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
        let Some(g) = &mut self.pad else { return };
        let mut inputs = Vec::new();
        let now = Instant::now();
        let is_move = |k: Key| matches!(k, Key::Up | Key::Down | Key::Left | Key::Right);
        let mut kind = None;
        while let Some(ev) = g.next_event() {
            if matches!(ev.event, gilrs::EventType::ButtonPressed(..) | gilrs::EventType::AxisChanged(..)) {
                let p = g.gamepad(ev.id);
                kind = Some(deck::pad_kind(p.name(), p.vendor_id()));
            }
            match ev.event {
                gilrs::EventType::ButtonReleased(b, _) => {
                    let k = match b {
                        gilrs::Button::DPadUp => Key::Up,
                        gilrs::Button::DPadDown => Key::Down,
                        gilrs::Button::DPadLeft => Key::Left,
                        gilrs::Button::DPadRight => Key::Right,
                        _ => continue,
                    };
                    if self.held.is_some_and(|(h, _)| h == k) {
                        self.held = None;
                    }
                }
                gilrs::EventType::ButtonPressed(b, _) => {
                    let k = match b {
                        gilrs::Button::DPadUp => Key::Up,
                        gilrs::Button::DPadDown => Key::Down,
                        gilrs::Button::DPadLeft => Key::Left,
                        gilrs::Button::DPadRight => Key::Right,
                        gilrs::Button::South | gilrs::Button::Start => Key::Enter,
                        gilrs::Button::East => Key::Escape,
                        gilrs::Button::LeftTrigger => Key::PagePrev,
                        gilrs::Button::RightTrigger => Key::PageNext,
                        gilrs::Button::West => Key::ActionX,
                        gilrs::Button::North => Key::ActionY,
                        _ => continue,
                    };
                    if is_move(k) {
                        self.held = Some((k, now + REPEAT_DELAY));
                    }
                    inputs.push(Input::Key(k));
                }
                gilrs::EventType::AxisChanged(axis, v, _) => {
                    let dir = if v > 0.6 {
                        1
                    } else if v < -0.6 {
                        -1
                    } else {
                        0
                    };
                    match axis {
                        gilrs::Axis::LeftStickX if dir != self.stick.0 => {
                            self.stick.0 = dir;
                            if self.held.is_some_and(|(h, _)| matches!(h, Key::Left | Key::Right)) {
                                self.held = None;
                            }
                            if dir != 0 {
                                let k = if dir > 0 { Key::Right } else { Key::Left };
                                self.held = Some((k, now + REPEAT_DELAY));
                                inputs.push(Input::Key(k));
                            }
                        }
                        gilrs::Axis::LeftStickY if dir != self.stick.1 => {
                            self.stick.1 = dir;
                            if self.held.is_some_and(|(h, _)| matches!(h, Key::Up | Key::Down)) {
                                self.held = None;
                            }
                            if dir != 0 {
                                // gilrs: up is positive.
                                let k = if dir > 0 { Key::Up } else { Key::Down };
                                self.held = Some((k, now + REPEAT_DELAY));
                                inputs.push(Input::Key(k));
                            }
                        }
                        _ => {}
                    }
                }
                gilrs::EventType::Disconnected => self.held = None,
                _ => {}
            }
        }
        // A held direction repeats (lists, the resolution row).
        if let Some((k, at)) = self.held
            && now >= at
        {
            self.held = Some((k, now + REPEAT_EVERY));
            inputs.push(Input::Key(k));
        }
        inputs.retain(|i| match i {
            Input::Key(k) => self.dup.accept(*k, Source::Pad, now),
            _ => true,
        });
        if !inputs.is_empty() {
            self.app.model.pad_input = true;
            let kind = kind.unwrap_or_else(|| {
                let k = deck::last_input();
                if k.is_pad() { k } else { deck::pad_kind("", None) }
            });
            self.used(kind);
        }
        for i in inputs {
            self.send(i);
        }
    }
}

impl ApplicationHandler for Run<'_> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        // 960 × 720 (logical), smaller on a monitor where that would not
        // fit beside a task bar and the title bar (a 1366 × 768 laptop at
        // 100 %, any small screen at 150 % or more).
        // The monitor read through `platform::monitor` (an unknown size:
        // 960 × 720, never a panic).
        let primary = el.primary_monitor().map(|m| {
            let f = crate::platform::monitor::facts(&m);
            (m, f)
        });
        // Under gamescope (Deck Game Mode) the whole screen instead.
        let gamescope = deck::is_gamescope();
        let size = deck::window_size(
            gamescope,
            primary.as_ref().map(|(_, f)| f.size),
            primary.as_ref().map_or(1.0, |(m, _)| m.scale_factor()),
        );
        let (lw, lh) = match size {
            deck::WindowSize::Logical(w, h) => (w, h),
            deck::WindowSize::Screen(..) => (960.0, 720.0),
        };
        let attrs = Window::default_attributes()
            .with_title(super::screens::NAME)
            .with_window_icon(crate::platform::icon::window_icon());
        let attrs = match size {
            deck::WindowSize::Screen(w, h) => {
                log::info!("window: gamescope, the whole screen {w}x{h}");
                attrs.with_inner_size(winit::dpi::PhysicalSize::new(w, h)).with_fullscreen(Some(
                    winit::window::Fullscreen::Borderless(primary.as_ref().map(|(m, _)| m.clone())),
                ))
            }
            deck::WindowSize::Logical(..) => attrs
                .with_inner_size(LogicalSize::new(lw, lh))
                .with_min_inner_size(LogicalSize::new(lw.min(640.0), lh.min(480.0))),
        };
        // Centred on the primary monitor.
        let attrs = match &primary {
            _ if gamescope => attrs,
            Some((m, f)) if f.size.0 > 0 && f.size.1 > 0 => {
                let (scale, pos, size) = (m.scale_factor(), m.position(), f.size);
                let (w, h) = ((lw * scale) as i32, (lh * scale) as i32);
                let x = pos.x + ((size.0 as i32 - w) / 2).max(0);
                let y = pos.y + ((size.1 as i32 - h) / 2).max(0);
                attrs.with_position(winit::dpi::PhysicalPosition::new(x, y))
            }
            _ => attrs,
        };
        let made = el.create_window(attrs).map_err(|e| e.to_string()).and_then(|w| {
            let w = Rc::new(w);
            let ctx = softbuffer::Context::new(w.clone()).map_err(|e| e.to_string())?;
            let s = Surface::new(&ctx, w.clone()).map_err(|e| e.to_string())?;
            Ok((w, s))
        });
        match made {
            Ok((w, s)) => {
                let size = w.inner_size();
                log::info!(
                    "window: open, {}x{} at scale {}, {} monitor {:?}",
                    size.width,
                    size.height,
                    w.scale_factor(),
                    if is_wayland(el) { "Wayland" } else { "the system's" },
                    primary.as_ref().map(|(_, f)| (f.size, f.modes.len()))
                );
                // The window's monitor until the probe tells the game's view.
                if let Some(mon) = w.current_monitor()
                    && self.app.model.monitor.is_none()
                {
                    self.app.model.monitor =
                        Some(crate::render::display::MonitorInfo::from_winit(&mon, is_wayland(el)));
                }
                let size = w.inner_size();
                self.app.resize((size.width.max(1), size.height.max(1)), w.scale_factor() as f32, &self.fonts);
                w.request_redraw();
                self.window = Some(w);
                self.surface = Some(s);
            }
            Err(e) => {
                log::error!("window: could not open: {e}");
                self.error = Some(format!("the launcher window could not open: {e}"));
                el.exit();
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // The window is hidden while the game runs: its input waits.
        if self.running.is_some() && !matches!(event, WindowEvent::RedrawRequested | WindowEvent::Resized(_)) {
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                // As Quit: changed choices saved first; the window closes
                // either way.
                let _ = self.app.event(&super::ui::Event::Activate("quit"));
                self.outcome = Outcome::Quit;
            }
            WindowEvent::Resized(s) => {
                let dpi = self.window.as_ref().map_or(1.0, |w| w.scale_factor() as f32);
                self.app.resize((s.width.max(1), s.height.max(1)), dpi, &self.fonts);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let size = self.app.size;
                self.app.resize(size, scale_factor as f32, &self.fonts);
            }
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.app.model.pad_input = false;
                let shift = self.mods.shift_key();
                let key = match &event.logical_key {
                    WKey::Named(n) => match n {
                        NamedKey::ArrowUp => Some(Key::Up),
                        NamedKey::ArrowDown => Some(Key::Down),
                        NamedKey::ArrowLeft => Some(Key::Left),
                        NamedKey::ArrowRight => Some(Key::Right),
                        NamedKey::Tab if self.mods.control_key() => {
                            Some(if shift { Key::PagePrev } else { Key::PageNext })
                        }
                        NamedKey::Tab => Some(if shift { Key::BackTab } else { Key::Tab }),
                        NamedKey::Enter => Some(Key::Enter),
                        NamedKey::Escape => Some(Key::Escape),
                        NamedKey::Backspace => Some(Key::Backspace),
                        NamedKey::Delete => Some(Key::Delete),
                        NamedKey::F2 => Some(Key::Rename),
                        NamedKey::Home => Some(Key::Home),
                        NamedKey::End => Some(Key::End),
                        NamedKey::PageUp => Some(Key::PageUp),
                        NamedKey::PageDown => Some(Key::PageDown),
                        NamedKey::Space if !self.focused_is_field() => Some(Key::Space),
                        _ => None,
                    },
                    // Ctrl+B / Ctrl+E / Ctrl+Z: the Saves page's back up,
                    // export and undo.
                    WKey::Character(c) if self.mods.control_key() && !shift => {
                        match c.to_ascii_lowercase().as_str() {
                            "b" => Some(Key::BackUp),
                            "e" => Some(Key::Export),
                            "z" => Some(Key::Undo),
                            _ => None,
                        }
                    }
                    // Q / E: the pages; R: a page's second action (not
                    // while a text field takes letters).
                    WKey::Character(c) if !self.focused_is_field() && !self.mods.control_key() => {
                        match c.to_ascii_lowercase().as_str() {
                            "q" => Some(Key::PagePrev),
                            "e" => Some(Key::PageNext),
                            "r" => Some(Key::ActionY),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                if let Some(k) = key {
                    if self.dup.accept(k, Source::Keyboard, Instant::now()) {
                        self.used(InputKind::Keyboard);
                        self.send(Input::Key(k));
                    }
                } else if let Some(t) = event.text.as_ref().filter(|t| t.chars().all(|c| !c.is_control())) {
                    if !self.mods.control_key() {
                        self.used(InputKind::Keyboard);
                        self.send(Input::Text(t.to_string()));
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let moved = (position.x as f32, position.y as f32) != self.mouse;
                self.mouse = (position.x as f32, position.y as f32);
                if moved {
                    self.used(InputKind::Mouse);
                }
                log::debug!(target: "launcher::input", "cursor {:.1},{:.1} scale {}", position.x, position.y,
                    self.window.as_ref().map_or(1.0, |w| w.scale_factor()));
                self.send(Input::MouseMove(self.mouse.0, self.mouse.1));
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                self.app.model.pad_input = false;
                self.used(InputKind::Mouse);
                let (x, y) = self.mouse;
                self.send(if state == ElementState::Pressed { Input::MouseDown(x, y) } else { Input::MouseUp(x, y) });
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Back, .. } => {
                self.send(Input::MouseBack)
            }
            WindowEvent::MouseWheel { delta, .. } => {
                log::debug!(target: "launcher::input", "wheel {delta:?}");
                self.send(wheel_input(delta));
            }
            WindowEvent::DroppedFile(p) => {
                log::debug!(target: "launcher::input", "dropped {}", p.display());
                self.dropped.push(p);
            }
            _ => {}
        }
        if self.outcome != Outcome::Stay || self.error.is_some() {
            el.exit();
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if !self.dropped.is_empty() && self.running.is_none() {
            let paths = std::mem::take(&mut self.dropped);
            self.send(Input::Drop(paths));
        }
        self.reprobe();
        self.poll_probe();
        self.poll_import();
        self.poll_hud();
        self.poll_dialog();
        self.poll_backdrop();
        self.poll_game();
        self.poll_prepare();
        if self.running.is_none() {
            self.poll_pad();
        }
        if self.outcome != Outcome::Stay {
            el.exit();
            return;
        }
        let now = Instant::now();
        if self.app.poll_jobs(&self.fonts)
            && let Some(w) = &self.window
        {
            w.request_redraw();
        }
        if self.app.motion.busy(now)
            && let Some(w) = &self.window
        {
            w.request_redraw();
            let at = self.app.motion.next_frame(now).unwrap_or(now + Duration::from_millis(16));
            el.set_control_flow(ControlFlow::WaitUntil(at));
            return;
        }
        // A busy spinner turns (about 30 frames a second); a support file
        // being written is polled.
        // "Shaders ready" goes away by itself.
        if self.app.shaders_ready_shown() && self.app.model.prepare.done_at.is_some_and(|t| t.elapsed() > SHADERS_READY_FOR) {
            self.app.model.prepare.done_at = None;
            self.app.rebuild(&self.fonts);
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
        if self.app.shaders_ready_shown() {
            el.set_control_flow(ControlFlow::WaitUntil(now + Duration::from_millis(250)));
            return;
        }
        let spinning = super::ui::spinning(&self.app.ui) && !super::anim::reduced();
        if (spinning || self.app.model.support_busy) && self.running.is_none() {
            if spinning && let Some(w) = &self.window {
                w.request_redraw();
            }
            el.set_control_flow(ControlFlow::WaitUntil(now + Duration::from_millis(33)));
            return;
        }
        let fading = self.picture_rx.is_some()
            || self.picture_at.is_some_and(|t| t.elapsed() < FADE + Duration::from_millis(40));
        if fading {
            el.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(16)));
        } else if self.running.is_some()
            || self.probe.is_some()
            || self.import.is_some()
            || self.app.saves_rx.is_some()
            || self.hud_job.is_some()
            || self.dialog.is_some()
            || self.app.model.dialog.is_some()
            || self.prepare.is_some()
        {
            el.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(50)));
        } else if self.pad.is_some() {
            el.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(16)));
        } else if let Some(t) = self.app.saves_wake() {
            // The Saves page's Undo goes away by itself.
            el.set_control_flow(ControlFlow::WaitUntil(t));
        } else {
            el.set_control_flow(ControlFlow::Wait);
        }
    }
}

/// A wheel event as the launcher's input: a mouse wheel's notches as
/// lines, a touchpad's smooth scrolling as pixels (both up positive;
/// winit gives both in the window's physical pixels' terms, as the
/// layout is: no scale factor to apply).
pub fn wheel_input(delta: MouseScrollDelta) -> Input {
    match delta {
        MouseScrollDelta::LineDelta(_, y) => Input::Wheel(y),
        MouseScrollDelta::PixelDelta(p) => Input::WheelPx(p.y as f32),
    }
}

/// The window's first inner size (logical): 960 × 720, at most 92 % of the
/// monitor's width and 85 % of its height (room for a task bar and the
/// title bar), never under 480 × 360.
pub fn initial_size(mon_w: u32, mon_h: u32, scale: f64) -> (f64, f64) {
    let s = if scale > 0.0 { scale } else { 1.0 };
    let (w, h) = (f64::from(mon_w) / s, f64::from(mon_h) / s);
    if w < 1.0 || h < 1.0 {
        return (960.0, 720.0);
    }
    ((w * 0.92).floor().clamp(480.0, 960.0), (h * 0.85).floor().clamp(360.0, 720.0))
}

/// Whether the launcher (and so the game) runs on Wayland: no exclusive
/// full screen there.
fn is_wayland(el: &ActiveEventLoop) -> bool {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use winit::platform::wayland::ActiveEventLoopExtWayland;
        el.is_wayland()
    }
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    {
        let _ = el;
        false
    }
}

/// What the window starts with.
pub struct Start {
    pub screen: Screen,
    /// The game folder in use, if known.
    pub root: Option<std::path::PathBuf>,
    /// A failed start to show (the skip rule's start failed).
    pub failure: Option<Failure>,
    /// The full check of the game folder, made before the window opens
    /// (the main screen's state line says what it found).
    pub check: Option<super::model::CheckResult>,
    /// Run `game --probe` in the background (the real machine).
    pub probe: bool,
    pub game: GameStart,
}

/// Opens the window and runs it until the player quits, or the game
/// started from it ends normally.
pub fn run_with_root(env: &dyn Env, start: Start) -> Result<Outcome, String> {
    let fonts = Fonts::new();
    let mut model = env.load();
    if let Some(r) = start.root {
        model.languages = env.languages(&r);
        model.root = Some(r);
    }
    model.failure = start.failure;
    if start.check.is_some() {
        model.check = start.check;
    }
    // Found on this computer at this start (nothing kept from before): the
    // main screen says where, and Play keeps it.
    model.just_found = model.root.is_some() && crate::qol::setup::saved_root().is_none();
    let model_root = model.root.clone();
    let mut app = App::new(env, model, start.screen, (960, 720), 1.0, &fonts);
    app.apply_prefs();
    app.st.input = deck::default_input();
    let el = EventLoop::new().map_err(|e| format!("no display for the launcher window: {e}"))?;
    let pad = if std::env::var_os("NFSU2_NO_PADS").is_none() {
        match gilrs::Gilrs::new() {
            Ok(g) => Some(g),
            Err(e) => {
                log::warn!("gamepads: not available ({e})");
                None
            }
        }
    } else {
        None
    };
    log::info!("start: gamepads {}", if pad.is_some() { "on" } else { "off" });
    let probed_root = model_root.clone();
    let probe = start.probe.then(|| super::probe::spawn(start.game.exe.clone(), probed_root.clone()));
    let mut run = Run {
        app,
        fonts,
        window: None,
        surface: None,
        canvas: Canvas::new(1, 1),
        mods: ModifiersState::empty(),
        mouse: (0.0, 0.0),
        pad,
        stick: (0, 0),
        held: None,
        outcome: Outcome::Stay,
        error: None,
        game: start.game,
        running: None,
        probe,
        probed_root,
        probing: start.probe,
        prepare: None,
        import: None,
        hud_job: None,
        dialog: None,
        picture: None,
        scene: Default::default(),
        picture_rx: None,
        picture_at: None,
        picture_root: None,
        page: (0, 0, 0, Vec::new()),
        traced: String::new(),
        dropped: Vec::new(),
        pointer: super::ui::Pointer::Default,
        drawn: false,
        dup: Dedup::default(),
        cursor_hidden: false,
        field_focused: false,
    };
    el.run_app(&mut run).map_err(|e| e.to_string())?;
    // A game still running when the window was closed: wait for it, so
    // its output is kept.
    if let Some(r) = run.running.take() {
        let _ = r.wait();
    }
    // A prepare run still going: stopped with the launcher (its cache
    // files are whole; the rest is made later).
    if let Some(job) = run.prepare.take() {
        job.stop();
    }
    match run.error {
        Some(e) => Err(e),
        None => Ok(if run.outcome == Outcome::Stay { Outcome::Quit } else { run.outcome }),
    }
}
