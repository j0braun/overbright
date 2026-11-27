//! The tutorials (docs/modding/studio.md, "The tutorials"): "Your first mod" (make a mod, go to a street, place a
//! prop, drive it, share it) and "Make your first race" (make a mod, choose a base track, place the start, add the
//! race points, test the race, share it), each on real operations. Each step ticks by itself when the mod or the
//! window shows it done, however it was done (the step's button, the menus, the mouse, the pad); the card that
//! shows the step is Home's verdict card while Home is open and floats over the view's corner elsewhere.

use super::words;

/// Which tutorial runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tutorial {
    FirstMod,
    FirstRace,
}

impl Tutorial {
    pub const ALL: [Tutorial; 2] = [Tutorial::FirstMod, Tutorial::FirstRace];

    pub fn steps(self) -> &'static [Step] {
        match self {
            Tutorial::FirstMod => &[Step::MakeMod, Step::GoToStreet, Step::PlaceProp, Step::DriveIt, Step::ShareIt],
            Tutorial::FirstRace => {
                &[Step::MakeMod, Step::BaseTrack, Step::PlaceStart, Step::RacePoints, Step::TestRace, Step::ShareIt]
            }
        }
    }

    /// Its row on Home (START HERE) and the line under it.
    pub fn row(self) -> (&'static str, &'static str) {
        match self {
            Tutorial::FirstMod => (words::TUTORIAL_ROW, words::TUTORIAL_ROW_LINE),
            Tutorial::FirstRace => (words::RACE_TUTORIAL_ROW, words::RACE_TUTORIAL_ROW_LINE),
        }
    }

    /// The tutorial's mod's name, typed into the New mod dialog.
    pub fn mod_name(self) -> &'static str {
        match self {
            Tutorial::FirstMod => MOD_NAME,
            Tutorial::FirstRace => RACE_MOD_NAME,
        }
    }
}

/// One step of a tutorial.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    MakeMod,
    GoToStreet,
    PlaceProp,
    DriveIt,
    BaseTrack,
    PlaceStart,
    RacePoints,
    TestRace,
    ShareIt,
    /// Every step done: the last card says what comes next.
    Done,
}

/// What the step's button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    /// The New mod dialog, with the tutorial's name typed in.
    NewMod,
    /// World, the camera at street height over an open street.
    TakeMeThere,
    /// A cone where the camera looks (the editor's own add, as a drop from the Prop browser).
    PlaceCone,
    /// Drive here (Shift+F5): the game starts at the spot looked at, with the mod.
    DriveHere,
    /// Races, the camera over a dock road, then the editor's New race (on the base track whose roads are nearest).
    NewRace,
    /// The Start tool's click on the road ahead.
    PlaceStart,
    /// The Checkpoint and Finish tools' clicks along the base track's roads ahead of the start.
    AddPoints,
    /// Test race (F5).
    TestRace,
    /// The Mod page, where Export is.
    GoToMod,
    /// Close the card.
    End,
}

impl Step {
    pub fn title(self, t: Tutorial) -> &'static str {
        match self {
            Step::MakeMod => words::MAKE_A_MOD,
            Step::GoToStreet => words::GO_TO_A_STREET,
            Step::PlaceProp => words::PLACE_A_PROP,
            Step::DriveIt => words::DRIVE_IT,
            Step::BaseTrack => words::CHOOSE_A_BASE_TRACK,
            Step::PlaceStart => words::PLACE_THE_START,
            Step::RacePoints => words::ADD_RACE_POINTS,
            Step::TestRace => words::TEST_THE_RACE,
            Step::ShareIt => words::SHARE_IT,
            Step::Done if t == Tutorial::FirstRace => words::YOUR_FIRST_RACE_IS_DONE,
            Step::Done => words::YOUR_FIRST_MOD_IS_DONE,
        }
    }

    /// What to do, in plain words.
    pub fn line(self, t: Tutorial) -> &'static str {
        match self {
            Step::MakeMod if t == Tutorial::FirstRace => {
                "A race lives in a mod, apart from the game's own files. Give it a name; Studio makes its folder \
                 and saves as you go."
            }
            Step::MakeMod => {
                "A mod keeps your changes apart from the game's own files. Give it a name; Studio makes its folder \
                 and saves as you go."
            }
            Step::GoToStreet => {
                "World shows the city. Studio takes you down to street level. To fly yourself: hold the right \
                 mouse button and use W A S D."
            }
            Step::PlaceProp => {
                "Drag a prop from the Prop browser at the bottom onto the road, or let Studio place a cone where you \
                 look. Ctrl+Z takes it back."
            }
            Step::DriveIt => {
                "Drive here starts the game at this spot with your mod on. Drive past your prop, then close the game \
                 to come back."
            }
            Step::BaseTrack => {
                "A race drives on the roads of one of the game's tracks, its base track. Studio takes the one under \
                 you; its roads show faint green."
            }
            Step::PlaceStart => {
                "Click the road with the Start tool. The line sits on the road, the cars line up behind it facing \
                 down the road."
            }
            Step::RacePoints => {
                "Checkpoints mark the way and the finish ends it, all on the green roads. Click the road yourself, \
                 or let Studio add them ahead."
            }
            Step::TestRace => {
                "Test race (F5) starts the game on your grid against the AI. Race it, or close the game to come \
                 back."
            }
            Step::ShareIt => {
                "The Mod page says whether your mod is ready to share. Export makes one .zip that a friend adds in \
                 the launcher."
            }
            Step::Done if t == Tutorial::FirstRace => {
                "You made a race on a base track, tested it and shared it. Next: open the sample race pack on Home \
                 to see more."
            }
            Step::Done => {
                "You made a mod, placed a prop, drove it and shared it. Next: make a race in Races. Help › Tutorial \
                 starts this again."
            }
        }
    }

    /// The step's button.
    pub fn act(self) -> (Act, &'static str) {
        match self {
            Step::MakeMod => (Act::NewMod, words::NEW_MOD),
            Step::GoToStreet => (Act::TakeMeThere, words::TAKE_ME_THERE),
            Step::PlaceProp => (Act::PlaceCone, words::PLACE_A_CONE),
            Step::DriveIt => (Act::DriveHere, words::DRIVE_HERE),
            Step::BaseTrack => (Act::NewRace, words::GO_TO_THE_DOCKS),
            Step::PlaceStart => (Act::PlaceStart, words::PLACE_THE_START),
            Step::RacePoints => (Act::AddPoints, words::ADD_THEM_AHEAD),
            Step::TestRace => (Act::TestRace, words::TEST_RACE),
            Step::ShareIt => (Act::GoToMod, words::GO_TO_MOD),
            Step::Done => (Act::End, words::CLOSE),
        }
    }
}

/// What the window shows this frame, as far as the steps need it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    /// The open mod's folder.
    pub open: Option<String>,
    /// The map editor's camera sees the ground (a drop or Drive here lands there).
    pub ground_in_view: bool,
    /// Props the open mod adds.
    pub added: usize,
    /// Races in the open mod.
    pub races: usize,
    /// The selected race has its start line.
    pub start: bool,
    /// The selected race can be raced: checkpoints, a finish when it is a sprint, no error left.
    pub raceable: bool,
    /// Test runs started in this window so far.
    pub tests: u64,
    /// Exports written in this window so far.
    pub exports: u64,
}

/// The tutorial while it runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Guide {
    pub tutorial: Tutorial,
    pub step: Step,
    /// The mod the tutorial made (its folder): the steps work on it, never on a mod made before.
    pub made: Option<String>,
    /// A step's action that waits for the map editor to have the tutorial's mod open (Take me there, Place a
    /// cone, Drive here, the race steps).
    pub pending: Option<Act>,
    /// The counters when their step began (a test or export from before does not count).
    tests_at: u64,
    exports_at: u64,
}

/// The tutorials' mods' names, typed into the New mod dialog.
pub const MOD_NAME: &str = words::MY_FIRST_MOD;
pub const RACE_MOD_NAME: &str = words::MY_FIRST_RACE;

/// The spot Take me there goes to: an open street in the default region (L4RA), and the camera's place
/// over it (behind and above, looking at it; well within the editor's ground reach).
pub const SPOT: [f32; 3] = [-805.1, -309.9, 11.8];
pub const EYE_BACK: [f32; 3] = [-28.0, -32.0, 14.0];

/// The race tutorial's start: the dock road at the rail crossing in Coal Harbor (base track 4041's roads, the
/// sample's Harbour Line), the race running south; the camera behind it, looking down the road.
pub const RACE_SPOT: [f32; 3] = [-1101.6, -1772.0, 13.6];
pub const RACE_EYE_BACK: [f32; 3] = [0.0, 45.0, 22.0];
pub const RACE_LOOK: [f32; 2] = [0.0, -1.0];
/// The race points Add them ahead places: checkpoints this far along the road from the start (metres), then the
/// finish (a short first sprint).
pub const CHECKPOINTS_AT: [f32; 4] = [40.0, 120.0, 200.0, 280.0];
pub const FINISH_AT: f32 = 350.0;

/// The props the tutorial places, the first the world has.
pub const CONES: [&str; 2] = ["XO_CONE_01", "XC_CONE_01"];

impl Guide {
    pub fn new(tutorial: Tutorial, f: &Facts) -> Guide {
        Guide { tutorial, step: Step::MakeMod, made: None, pending: None, tests_at: f.tests, exports_at: f.exports }
    }

    /// The step's number (1 …); the last card has none.
    pub fn number(&self) -> Option<usize> {
        self.tutorial.steps().iter().position(|s| *s == self.step).map(|i| i + 1)
    }

    /// The step at `n` (1 …; past the last: the last card).
    pub fn step_at(t: Tutorial, n: usize) -> Step {
        n.checked_sub(1).and_then(|i| t.steps().get(i)).copied().unwrap_or(Step::Done)
    }

    fn next(&self) -> Step {
        self.number().map_or(Step::Done, |n| Guide::step_at(self.tutorial, n + 1))
    }

    pub fn title(&self) -> &'static str {
        self.step.title(self.tutorial)
    }

    pub fn line(&self) -> &'static str {
        self.step.line(self.tutorial)
    }

    /// Tick the steps the facts show done (several at once when they are).
    pub fn advance(&mut self, f: &Facts) {
        loop {
            let ours = self.made.is_some() && f.open == self.made;
            let waits = |a| self.pending == Some(a);
            let done = match self.step {
                Step::MakeMod => ours,
                Step::GoToStreet => ours && f.ground_in_view && !waits(Act::TakeMeThere),
                Step::PlaceProp => ours && f.added > 0,
                Step::DriveIt | Step::TestRace => ours && f.tests > self.tests_at,
                Step::BaseTrack => ours && f.races > 0 && !waits(Act::NewRace),
                Step::PlaceStart => ours && f.races > 0 && f.start && !waits(Act::PlaceStart),
                Step::RacePoints => ours && f.raceable && !waits(Act::AddPoints),
                Step::ShareIt => ours && f.exports > self.exports_at,
                Step::Done => false,
            };
            if !done {
                return;
            }
            self.step = self.next();
            self.tests_at = f.tests;
            self.exports_at = f.exports;
        }
    }

    /// "Step 2 of 5" (the last card: "Tutorial").
    pub fn kicker(&self) -> String {
        match self.number() {
            Some(n) => format!("TUTORIAL · STEP {n} OF {}", self.tutorial.steps().len()),
            None => "TUTORIAL".into(),
        }
    }
}

/// The card's second button: End tutorial (the last card: Go to Races, or Open the sample after the race one).
pub fn second_word(g: &Guide) -> &'static str {
    match (g.step, g.tutorial) {
        (Step::Done, Tutorial::FirstMod) => words::GO_TO_RACES,
        (Step::Done, Tutorial::FirstRace) => words::OPEN_SAMPLE,
        _ => words::END_TUTORIAL,
    }
}

// --- the tutorial in Studio's window ----------------------------------------------------------------------------

use egui::{Rect, pos2, vec2};
use glam::{Vec2, Vec3};

use crate::editor::theme::{self as etheme, tokens};
use crate::tools::map::PropEdit;
use crate::tools::map::edit::ray_ground;

use super::Studio;
use super::draw::{self, Card};
use super::layout::Layout;
use super::workspace::Workspace;

impl Studio {
    /// Start a tutorial (Home's START HERE, Help › Tutorial, F1), on Home.
    pub fn start_tutorial(&mut self, t: Tutorial) {
        let f = self.guide_facts();
        self.guide = Some(Guide::new(t, &f));
        self.workspace(Workspace::Home);
        self.home_focus = 0;
    }

    /// The card's main action: the step's button.
    pub fn guide_main(&mut self) {
        let Some(g) = self.guide.as_mut() else { return };
        let name = g.tutorial.mod_name();
        match g.step.act().0 {
            Act::NewMod => {
                self.open_new_mod();
                if let Some(d) = self.new_mod.as_mut().filter(|d| d.name.is_empty()) {
                    d.name = name.into();
                }
            }
            a @ (Act::TakeMeThere | Act::PlaceCone | Act::DriveHere) => {
                g.pending = Some(a);
                if self.nav.workspace != Workspace::World {
                    self.workspace(Workspace::World);
                }
            }
            a @ (Act::NewRace | Act::PlaceStart | Act::AddPoints | Act::TestRace) => {
                g.pending = Some(a);
                if self.nav.workspace != Workspace::Races {
                    self.workspace(Workspace::Races);
                }
            }
            // On the Mod page the button is the page's own Export (the warning first when it has one to give).
            Act::GoToMod if self.nav.workspace == Workspace::Mod => {
                self.mod_act(super::mod_page::Act::Export { derived_ok: false })
            }
            Act::GoToMod => self.workspace(Workspace::Mod),
            Act::End => self.guide = None,
        }
    }

    /// The card's second button: End tutorial; the last card's: Go to Races (Open the sample after the race one).
    pub fn guide_second(&mut self) {
        let done = self.guide.as_ref().filter(|g| g.step == Step::Done).map(|g| g.tutorial);
        self.guide = None;
        match done {
            Some(Tutorial::FirstMod) => self.workspace(Workspace::Races),
            Some(Tutorial::FirstRace) => self.open_sample(),
            None => {}
        }
    }

    /// What the steps read this frame.
    pub fn guide_facts(&self) -> Facts {
        let open = self.open_mod().map(|m| m.folder.clone());
        let st = self
            .editor
            .as_ref()
            .and_then(|h| h.app.state.as_ref())
            .filter(|s| open.as_deref() == Some(s.session.folder.as_str()));
        let (ground_in_view, added) = match st {
            Some(s) => (
                ray_ground(&s.session.world, s.camera.position, s.camera.forward(), 1500.0).is_some(),
                s.session.map.props.iter().filter(|p| matches!(p, PropEdit::Add { .. })).count(),
            ),
            None => (false, self.open_mod().map_or(0, |m| m.added)),
        };
        let races = st.map_or_else(|| self.open_mod().map_or(0, |m| m.races), |s| s.session.map.races.len());
        let (start, raceable) = st.map_or((false, false), |s| {
            let Some((k, r)) = s.race.and_then(|k| s.session.map.races.get(k).map(|r| (k, r))) else {
                return (false, false);
            };
            let error =
                s.session.issues.iter().any(|i| i.race == Some(k) && i.level == crate::editor::state::Level::Error);
            let finish = r.finish.is_some() || r.kind != crate::tools::map::RaceKind::Sprint;
            (r.start.is_some(), r.start.is_some() && r.checkpoints.len() >= 2 && finish && !error)
        });
        Facts {
            open,
            ground_in_view,
            added,
            races,
            start,
            raceable,
            tests: crate::editor::testplay::started(&self.ctx),
            exports: self.mod_page.exports,
        }
    }

    /// Before a frame: a waiting action done once the editor has the tutorial's mod open, then the steps ticked.
    pub(super) fn guide_frame(&mut self) {
        let Some(g) = self.guide.as_ref() else { return };
        if let Some(a) = g.pending {
            let made = g.made.clone();
            let editing = self.editing();
            let ctx = self.ctx.clone();
            if let Some(app) = self.editor.as_mut().map(|h| &mut h.app)
                && let Some(s) = app.state.as_mut()
                && made.as_deref() == Some(s.session.folder.as_str())
                && editing
            {
                match a {
                    Act::TakeMeThere => {
                        let spot = Vec3::from(SPOT);
                        s.camera = crate::draw::camera::Camera::looking_at(spot + Vec3::from(EYE_BACK), spot);
                        app.panels.viewport.set_look(s, "street");
                        s.status = words::HERE_STREET_LEVEL_A_PLACE_TO.into();
                    }
                    Act::PlaceCone => place_cone(s),
                    Act::NewRace => {
                        let spot = Vec3::from(RACE_SPOT);
                        let look = spot + Vec3::from((Vec2::from(RACE_LOOK) * 30.0, 0.0));
                        s.camera = crate::draw::camera::Camera::looking_at(spot + Vec3::from(RACE_EYE_BACK), look);
                        app.panels.viewport.set_look(s, "street");
                        let mut races = app.panels.races.borrow_mut();
                        if s.session.map.races.is_empty() {
                            races.new_race(s);
                        } else {
                            (s.race, s.mode) = (Some(0), crate::editor::state::Mode::Races);
                        }
                    }
                    Act::PlaceStart => place_start(s, &mut app.panels.races.borrow_mut()),
                    Act::AddPoints => add_points(s, &mut app.panels.races.borrow_mut()),
                    Act::TestRace => crate::editor::menu::post(&ctx, crate::editor::menu::Command::TestRace),
                    _ => crate::editor::menu::post(&ctx, crate::editor::menu::Command::DriveHere),
                }
                if let Some(g) = self.guide.as_mut() {
                    g.pending = None;
                }
            }
        }
        // The race steps read the validation: made again for a changed race before the facts.
        if self.guide.as_ref().is_some_and(|g| g.tutorial == Tutorial::FirstRace)
            && let Some(app) = self.editor.as_mut().map(|h| &mut h.app)
            && let Some(s) = app.state.as_mut()
        {
            app.panels.races.borrow_mut().update(s);
        }
        let f = self.guide_facts();
        if let Some(g) = self.guide.as_mut() {
            g.advance(&f);
        }
    }

    /// The tutorial's card over the corner of the view (every workspace but Home, where the card is Home's own).
    pub(super) fn guide_card(&mut self, ui: &mut egui::Ui, l: &Layout) {
        let Some(g) = self.guide.clone() else { return };
        let t = tokens();
        let view = l.view;
        let w = 380.0f32.min(view.width() - 32.0);
        if w < 240.0 {
            return;
        }
        let kicker = if self.nav.input.is_pad() { g.kicker() } else { format!("{} · F1", g.kicker()) };
        let c = Card {
            step: g.number(),
            kicker: Some(&kicker),
            head: g.title(),
            line: g.line(),
            main: if g.step == Step::ShareIt && self.nav.workspace == Workspace::Mod {
                words::EXPORT
            } else {
                g.step.act().1
            },
            second: second_word(&g),
        };
        let h = draw::card_height(ui.painter(), &c, w);
        // Above the hint line under the editor's view.
        let bottom = if self.editing() { view.bottom() - super::world::STRIP_H } else { view.bottom() };
        let min = pos2(view.right() - w - 16.0, (bottom - h - 12.0).max(view.top() + 8.0));
        let mut acts = (false, false);
        egui::Area::new(egui::Id::unique("guide-card"))
            .fixed_pos(min)
            .order(egui::Order::Foreground)
            .fade_in(false)
            .show(ui.ctx(), |ui| {
                let p = ui.painter().clone();
                let fill = etheme::over(t.panel, t.slab.over(t.page));
                p.rect_filled(Rect::from_min_size(min, vec2(w, h)).expand(1.0), 9.0, etheme::over(t.slab_edge, t.page));
                let (r, a, b) = draw::card(ui, &p, min, w, &c, fill);
                ui.allocate_rect(r, egui::Sense::hover());
                acts = (a.clicked(), b.clicked());
            });
        if acts.0 {
            self.nav.mouse();
            self.guide_main();
        } else if acts.1 {
            self.nav.mouse();
            self.guide_second();
        }
        self.guide_rect = Some(Rect::from_min_size(min, vec2(w, h)));
    }
}

/// The race's start line: the Start tool's own click on the road at the tutorial's spot (snapped onto the base
/// track's road, facing down it, its height the road's).
fn place_start(s: &mut crate::editor::state::EditorState, races: &mut crate::editor::races::Races) {
    if s.race.is_none_or(|k| k >= s.session.map.races.len()) {
        s.status = words::MAKE_A_RACE_FIRST_GO_TO.into();
        return;
    }
    s.mode = crate::editor::state::Mode::Races;
    races.tool = crate::editor::races::Tool::Start;
    races.click(s, Vec3::from(RACE_SPOT), false);
    if s.race.and_then(|k| s.session.map.races.get(k)).is_some_and(|r| r.start.is_some()) {
        s.status = words::START_LINE_PLACED_THE_CARS_LINE.into();
    }
}

/// The race points ahead of the start along the base track's roads: the Checkpoint tool's clicks, then the
/// Finish tool's (one undo step each, as clicked by hand).
fn add_points(s: &mut crate::editor::state::EditorState, races: &mut crate::editor::races::Races) {
    use crate::editor::races::{Tool, ahead};
    use crate::editor::validate::ground;
    let Some((track, start)) = s.race.and_then(|k| s.session.map.races.get(k)).map(|r| (r.track, r.start)) else {
        s.status = words::MAKE_A_RACE_FIRST_GO_TO.into();
        return;
    };
    let Some(start) = start else {
        s.status = words::PLACE_THE_START_LINE_FIRST.into();
        return;
    };
    let h = start.heading.to_radians();
    let at: Vec<f32> = CHECKPOINTS_AT.into_iter().chain([FINISH_AT]).collect();
    let pts = match races.net(s, track) {
        Some(net) => ahead(net, start.position.truncate(), Vec2::new(h.cos(), h.sin()), &at),
        None => Vec::new(),
    };
    if pts.len() < 3 {
        s.status = format!("Base track {track} has no road far enough ahead: click the road for the points");
        return;
    }
    s.mode = crate::editor::state::Mode::Races;
    let (cps, finish) = pts.split_at(pts.len().min(CHECKPOINTS_AT.len()));
    for p in cps {
        races.tool = Tool::Checkpoint;
        races.click(s, p.extend(ground(s, *p)), false);
    }
    if let Some(f) = finish.first() {
        races.tool = Tool::Finish;
        races.click(s, f.extend(ground(s, *f)), false);
    }
    races.tool = Tool::Select;
    races.update(s);
    s.status = format!("{} checkpoints and the finish placed on the base track's roads", cps.len());
}

/// Place a cone where the camera looks (the editor's own add, the step a drop from the Prop browser makes).
fn place_cone(s: &mut crate::editor::state::EditorState) {
    let (eye, f) = (s.camera.position, s.camera.forward());
    let Some(at) = ray_ground(&s.session.world, eye, f, 300.0) else {
        s.status = words::NO_GROUND_IN_VIEW_PRESS_TAKE.into();
        return;
    };
    let Some(solid) = CONES.into_iter().find(|c| s.session.world.solids.iter().any(|x| x.name.eq_ignore_ascii_case(c)))
    else {
        s.status = words::NO_CONE_IN_THIS_CITY_DRAG.into();
        return;
    };
    let heading = f.y.atan2(f.x).to_degrees() + 180.0;
    if crate::editor::props::place_new(s, solid, at, heading).is_some() {
        s.status = format!("Placed {solid} · Ctrl+Z takes it back");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(open: Option<&str>) -> Facts {
        Facts { open: open.map(str::to_string), ..Default::default() }
    }

    #[test]
    fn steps_tick_in_order_on_the_tutorials_mod_only() {
        let mut f = facts(Some("old-mod"));
        f.added = 3;
        f.ground_in_view = true;
        let mut g = Guide::new(Tutorial::FirstMod, &f);
        g.advance(&f);
        assert_eq!(g.step, Step::MakeMod, "a mod made before is not the tutorial's");
        g.made = Some("my-first-mod".into());
        let mut f = facts(Some("my-first-mod"));
        g.advance(&f);
        assert_eq!(g.step, Step::GoToStreet);
        f.ground_in_view = true;
        g.pending = Some(Act::TakeMeThere);
        g.advance(&f);
        assert_eq!(g.step, Step::GoToStreet, "the camera has not moved yet");
        g.pending = None;
        g.advance(&f);
        assert_eq!(g.step, Step::PlaceProp);
        f.added = 1;
        f.exports = 1;
        g.advance(&f);
        assert_eq!(g.step, Step::DriveIt, "an export before Share it does not count");
        f.tests = 1;
        g.advance(&f);
        assert_eq!(g.step, Step::ShareIt);
        f.exports = 2;
        g.advance(&f);
        assert_eq!(g.step, Step::Done);
        assert_eq!(g.kicker(), "TUTORIAL");
    }

    #[test]
    fn every_step_has_plain_words_and_a_button() {
        for t in Tutorial::ALL {
            for s in t.steps().iter().copied().chain([Step::Done]) {
                assert!(!s.title(t).is_empty() && s.line(t).len() < 200, "{s:?}");
                assert!(!s.act().1.is_empty());
            }
        }
        assert_eq!(Guide::new(Tutorial::FirstMod, &Facts::default()).kicker(), "TUTORIAL · STEP 1 OF 5");
        assert_eq!(Guide::new(Tutorial::FirstRace, &Facts::default()).kicker(), "TUTORIAL · STEP 1 OF 6");
    }

    #[test]
    fn the_race_steps_tick_from_the_race() {
        let mut f = facts(Some("my-first-race"));
        let mut g = Guide::new(Tutorial::FirstRace, &f);
        g.made = Some("my-first-race".into());
        g.advance(&f);
        assert_eq!(g.step, Step::BaseTrack);
        f.races = 1;
        g.pending = Some(Act::NewRace);
        g.advance(&f);
        assert_eq!(g.step, Step::BaseTrack, "the race is not made yet");
        g.pending = None;
        g.advance(&f);
        assert_eq!(g.step, Step::PlaceStart);
        f.start = true;
        g.advance(&f);
        assert_eq!(g.step, Step::RacePoints);
        f.raceable = true;
        f.tests = 1;
        g.advance(&f);
        assert_eq!(g.step, Step::TestRace, "a test from before does not count");
        f.tests = 2;
        g.advance(&f);
        assert_eq!(g.step, Step::ShareIt);
        f.exports = 1;
        g.advance(&f);
        assert_eq!((g.step, second_word(&g)), (Step::Done, words::OPEN_SAMPLE));
    }
}
