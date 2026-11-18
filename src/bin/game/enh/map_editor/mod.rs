//! The map editor in the game (docs/map-editor.md; opt-in modding tool,
//! reachable only after a start with `--editor`, then F8 in free roam).
//! Like photo mode it stands over the explore pause menu: the world stays
//! paused under it and is drawn from the editor's free camera with the
//! HUD hidden. The edits and the mod live in `nfsu2::tools::map`.
use crate::*;
use nfsu2::tools::map::edit::{self, Editor, Page, RacePart};
use nfsu2::tools::map::{self as map, CustomRace, MapMod, RaceKind, store};

mod ui;

/// The editor's state in the game (`Game::map_editor`).
pub(crate) struct MapEditor {
    pub ed: Editor,
    /// The panel and the key help shown.
    pub help: bool,
    /// The mouse at the last tick while a button was held (look).
    drag_from: Option<Vec2>,
    /// The base tracks' road networks (by track number) for the route
    /// preview, and the preview of the race as it was last built.
    nets: HashMap<u16, Option<nfsu2::formats::roads::RoadNet>>,
    pub preview: Option<(CustomRace, Result<Vec<Vec2>, String>)>,
}

/// What an editor frame changed for its draw, put back after it.
pub(crate) struct EditorSwap {
    camera: Option<nfsu2::render::Camera>,
    mode: Mode,
}

/// The mod the editor opens: the last enabled one, else `my-map` (read
/// when it exists).
fn open_mod() -> (String, MapMod) {
    let dir = store::maps_dir();
    let folder = store::enabled(&dir).pop().unwrap_or_else(|| "my-map".to_string());
    let m = map::active().iter().find(|m| m.0 == folder).map(|m| m.1.clone()).or_else(|| {
        let path = dir.join(&folder).join(store::FILE);
        path.is_file().then(|| store::read_file(&path).map_err(|e| log::warn!("map editor: {e:#}")).ok()).flatten()
    });
    let m = m.unwrap_or_else(|| MapMod { name: folder.clone(), ..Default::default() });
    (folder, m)
}

impl Game {
    /// F8 in free roam (with `--editor`): the pause menu under the
    /// editor, its camera from the game's.
    pub(crate) fn map_editor_open(&mut self) {
        if !map::editor_enabled()
            || self.map_editor.is_some()
            || !matches!(self.mode, Mode::Explore)
            || self.here().is_none()
        {
            return;
        }
        let Some(d) = self.sim.drive.as_ref() else { return };
        let mut camera = d.camera;
        camera.game_fov = None;
        camera.fov_y = 60f32.to_radians();
        camera.near = 0.1;
        camera.up = Vec3::Z;
        let (folder, m) = open_mod();
        self.mode = self.icon_menu(MenuKind::ExplorePause);
        let mut ed = Editor::new(&folder, m, camera);
        ed.status = Some((format!("Editing map mod `{folder}`"), false));
        self.map_editor = Some(MapEditor { ed, help: true, drag_from: None, nets: HashMap::new(), preview: None });
        log::info!("map editor opened on {folder}");
    }

    /// Esc: the mod saved (when changed), back to driving.
    pub(crate) fn map_editor_close(&mut self) -> Result<()> {
        let Some(me) = self.map_editor.take() else { return Ok(()) };
        if me.ed.dirty
            && let Err(e) = save(&me.ed)
        {
            log::warn!("map editor: not saved: {e:#}");
        }
        if let Some(d) = self.sim.drive.as_mut() {
            // A kept world comes back to the world as edited.
            d.world.retake_loaded();
        }
        log::info!("map editor closed");
        if matches!(self.mode, Mode::Menu(Menu::Icons(_, MenuKind::ExplorePause))) {
            self.resume_from_pause(false)?;
        }
        Ok(())
    }

    /// The editor closes once its pause menu is gone (called with
    /// `photo_close_update`).
    pub(crate) fn map_editor_close_update(&mut self) {
        if self.map_editor.is_some() && !matches!(&self.mode, Mode::Menu(Menu::Icons(_, MenuKind::ExplorePause))) {
            self.map_editor = None;
        }
    }

    /// A key while the editor is up (the held ones are read by
    /// [`Game::map_editor_tick`]).
    pub(crate) fn map_editor_key(&mut self, code: KeyCode) -> Result<()> {
        let ctrl =
            self.platform.keys.contains(&KeyCode::ControlLeft) || self.platform.keys.contains(&KeyCode::ControlRight);
        let shift =
            self.platform.keys.contains(&KeyCode::ShiftLeft) || self.platform.keys.contains(&KeyCode::ShiftRight);
        let Some(me) = self.map_editor.as_mut() else { return Ok(()) };
        let Some(d) = self.sim.drive.as_mut() else { return Ok(()) };
        let region = d.region.clone();
        let ed = &mut me.ed;
        // Steps along the camera's ground directions.
        let f = ed.camera.forward();
        let flat = Vec2::new(f.x, f.y).normalize_or(Vec2::Y);
        let right = Vec2::new(flat.y, -flat.x);
        let step = if shift { 2.0 } else { 0.25 };
        match code {
            KeyCode::Escape | KeyCode::F8 => return self.map_editor_close(),
            KeyCode::Tab => ed.page = if ed.page == Page::Props { Page::Races } else { Page::Props },
            KeyCode::KeyH => me.help = !me.help,
            KeyCode::KeyS if ctrl => match save(ed) {
                Ok(p) => {
                    ed.dirty = false;
                    ed.status = Some((format!("Saved {}", p.display()), false));
                }
                Err(e) => ed.status = Some((format!("Not saved: {e}"), true)),
            },
            KeyCode::F5 => return self.map_editor_reload(),
            KeyCode::KeyG => {
                // The car to the aim point, facing the camera's way.
                let at = ed.aim(&d.world);
                let heading = ed.heading().to_radians();
                d.place(at + Vec3::Z * 0.5, heading);
                return self.map_editor_close();
            }
            _ if ed.page == Page::Props => match code {
                KeyCode::Space | KeyCode::Enter | KeyCode::NumpadEnter => ed.pick(&d.world),
                KeyCode::ArrowUp => ed.nudge(&mut d.world, &region, (flat * step).extend(0.0), 0.0),
                KeyCode::ArrowDown => ed.nudge(&mut d.world, &region, (-flat * step).extend(0.0), 0.0),
                KeyCode::ArrowRight => ed.nudge(&mut d.world, &region, (right * step).extend(0.0), 0.0),
                KeyCode::ArrowLeft => ed.nudge(&mut d.world, &region, (-right * step).extend(0.0), 0.0),
                KeyCode::PageUp => ed.nudge(&mut d.world, &region, Vec3::Z * step, 0.0),
                KeyCode::PageDown => ed.nudge(&mut d.world, &region, -Vec3::Z * step, 0.0),
                KeyCode::KeyQ => ed.nudge(&mut d.world, &region, Vec3::ZERO, if shift { 90.0 } else { 15.0 }),
                KeyCode::KeyE => ed.nudge(&mut d.world, &region, Vec3::ZERO, if shift { -90.0 } else { -15.0 }),
                KeyCode::Delete | KeyCode::Backspace => ed.delete(&mut d.world, &region),
                KeyCode::KeyC => ed.duplicate(&d.world, &region),
                KeyCode::KeyZ => ed.restore(&mut d.world),
                _ => {}
            },
            _ => match code {
                KeyCode::KeyN => {
                    let at = ed.aim(&d.world).truncate();
                    match nearest_track(&self.game_data.root, &self.game_data.career, &region, at) {
                        Some(t) => ed.new_race(&d.world, t, RaceKind::Sprint),
                        None => ed.status = Some(("No race track of this region found".into(), true)),
                    }
                }
                KeyCode::KeyX if ctrl => ed.delete_race(),
                KeyCode::BracketLeft | KeyCode::BracketRight if !ed.map.races.is_empty() => {
                    let n = ed.map.races.len();
                    let k = ed.race.unwrap_or(0);
                    ed.race = Some(if code == KeyCode::BracketLeft { (k + n - 1) % n } else { (k + 1) % n });
                    ed.point = 0;
                }
                KeyCode::KeyP => {
                    let k = RacePart::ALL.iter().position(|p| *p == ed.part).unwrap_or(0);
                    ed.part = RacePart::ALL[(k + 1) % RacePart::ALL.len()];
                    ed.point = 0;
                }
                KeyCode::Comma => ed.point = ed.point.saturating_sub(1),
                KeyCode::Period => ed.point += 1,
                KeyCode::KeyJ => {
                    if let Some(p) = ed.point_position() {
                        // The camera above the point, looking down at it.
                        let back = Vec3::new(ed.camera.forward().x, ed.camera.forward().y, 0.0).normalize_or(Vec3::Y);
                        let z = d.world.grid_ground(p).unwrap_or(p.z);
                        ed.camera.position = p.truncate().extend(z) - back * 12.0 + Vec3::Z * 8.0;
                        ed.camera.pitch = -0.55;
                    }
                }
                KeyCode::Enter | KeyCode::NumpadEnter => ed.place(&d.world),
                KeyCode::Delete | KeyCode::Backspace => ed.remove_point(),
                KeyCode::ArrowUp => ed.move_point(flat * step * 2.0),
                KeyCode::ArrowDown => ed.move_point(-flat * step * 2.0),
                KeyCode::ArrowRight => ed.move_point(right * step * 2.0),
                KeyCode::ArrowLeft => ed.move_point(-right * step * 2.0),
                KeyCode::KeyM => ed.cycle_kind(),
                KeyCode::Equal | KeyCode::NumpadAdd => ed.laps(1),
                KeyCode::Minus | KeyCode::NumpadSubtract => ed.laps(-1),
                KeyCode::KeyT => return self.map_editor_race(),
                _ => {}
            },
        }
        if let Some(r) = me.ed.race.and_then(|k| me.ed.map.races.get(k)) {
            match r.part_len(me.ed.part) {
                0 => me.ed.point = 0,
                n => me.ed.point = me.ed.point.min(n - 1),
            }
        }
        self.map_editor_flush();
        Ok(())
    }

    /// The placements an edit moved or hid, to the renderer (as a load
    /// sends its barriers' in `drive_in_loaded`).
    fn map_editor_flush(&mut self) {
        let Some(d) = self.sim.drive.as_mut() else { return };
        let changed = d.world.take_changed();
        if let (Some(r), Some(w)) = (self.render.world.as_mut(), &self.platform.window) {
            for p in changed {
                let placement = &d.world.placements[p];
                r.update_placement(&w.gpu, p, placement.transform, placement.flags);
            }
        }
    }

    /// A simulation step's share of the editor: the held keys fly the
    /// camera, a mouse drag turns it.
    pub(crate) fn map_editor_tick(&mut self, dt: f32) {
        let Some(me) = self.map_editor.as_mut() else { return };
        let k = |c: KeyCode| if self.platform.keys.contains(&c) { 1.0 } else { 0.0 };
        let mv = Vec3::new(
            k(KeyCode::KeyW) - k(KeyCode::KeyS),
            k(KeyCode::KeyD) - k(KeyCode::KeyA),
            k(KeyCode::KeyR) - k(KeyCode::KeyF),
        );
        // I / K look up and down, J / L turn (on the race page J jumps to
        // the selected point: there the mouse and the numpad turn).
        let mut turn = Vec2::ZERO;
        turn.x = k(KeyCode::Numpad4) - k(KeyCode::Numpad6)
            + if me.ed.page == Page::Props { k(KeyCode::KeyJ) - k(KeyCode::KeyL) } else { 0.0 };
        turn.y = k(KeyCode::KeyI) - k(KeyCode::KeyK) + k(KeyCode::Numpad8) - k(KeyCode::Numpad2);
        let fast =
            self.platform.keys.contains(&KeyCode::ShiftLeft) || self.platform.keys.contains(&KeyCode::ShiftRight);
        let m = self.platform.mouse;
        self.platform.mouse.end_frame();
        let drag = match (m.held || m.pressed, m.at, me.drag_from) {
            (true, Some(at), Some(from)) => at - from,
            _ => Vec2::ZERO,
        };
        me.drag_from = if m.held || m.pressed { m.at } else { None };
        me.ed.camera.yaw += drag.x * 0.004;
        me.ed.camera.pitch = (me.ed.camera.pitch - drag.y * 0.004).clamp(-1.5, 1.5);
        edit::fly(&mut me.ed.camera, mv, turn, dt, fast);
        // The route preview of the race being edited, built again when
        // the race changed.
        let race = me.ed.race.and_then(|k| me.ed.map.races.get(k)).cloned();
        match race {
            Some(r) if me.preview.as_ref().is_none_or(|p| p.0 != r) => {
                let root = &self.game_data.root;
                let career = &self.game_data.career;
                let net = me.nets.entry(r.track).or_insert_with(|| {
                    let t = career.tracks.get(&r.track)?;
                    map::race::road_net(root, t).map_err(|e| log::warn!("map editor: {e:#}")).ok()
                });
                let built = match net {
                    None => Err(format!("track {} has no road network", r.track)),
                    Some(net) => map::race::route(net, &r, &|_| None)
                        .map(|route| route.road.points.clone())
                        .map_err(|e| e.to_string()),
                };
                me.preview = Some((r, built));
            }
            None => me.preview = None,
            _ => {}
        }
    }

    /// Before an editor frame's draw: its camera in, the pause menu's
    /// mode out (the world drawn as when driving). None without the editor.
    pub(crate) fn map_editor_begin(&mut self) -> Option<EditorSwap> {
        let cam = self.map_editor.as_ref()?.ed.camera;
        let camera = self.sim.drive.as_mut().map(|d| std::mem::replace(&mut d.camera, cam));
        let mode = std::mem::replace(&mut self.mode, Mode::Explore);
        Some(EditorSwap { camera, mode })
    }

    /// After it: everything put back.
    pub(crate) fn map_editor_end(&mut self, s: EditorSwap) {
        self.mode = s.mode;
        if let (Some(c), Some(d)) = (s.camera, self.sim.drive.as_mut()) {
            d.camera = c;
        }
    }

    /// F5: the mod saved and enabled, the world loaded again with it (the
    /// added props drawn), the editor open again at the same camera.
    fn map_editor_reload(&mut self) -> Result<()> {
        let Some(me) = self.map_editor.take() else { return Ok(()) };
        if let Err(e) = save(&me.ed) {
            log::warn!("map editor: not saved: {e:#}");
        }
        let Some(at) = self.here() else { return Ok(()) };
        // No kept or preloaded world: the region is read again.
        self.drop_drive();
        self.loads.resident = None;
        self.loads.kept_ahead = None;
        self.loads.preload = None;
        self.render.resident = None;
        self.render.resident_water = None;
        self.explore(Some(at))?;
        let mut ed = me.ed;
        (ed.dirty, ed.needs_reload, ed.selected) = (false, false, None);
        ed.status = Some(("World loaded again with the map mod".into(), false));
        if matches!(self.mode, Mode::Explore) {
            self.mode = self.icon_menu(MenuKind::ExplorePause);
            self.map_editor = Some(MapEditor { ed, ..me });
        }
        Ok(())
    }

    /// T: the race saved and raced (its base track's event set-up with
    /// the custom route, `drive_to_grid`).
    fn map_editor_race(&mut self) -> Result<()> {
        let Some(me) = self.map_editor.as_mut() else { return Ok(()) };
        let Some(race) = me.ed.race.and_then(|k| me.ed.map.races.get(k)).cloned() else {
            me.ed.status = Some(("Make a race first (N)".into(), true));
            return Ok(());
        };
        if let Some((_, Err(why))) = &me.preview {
            me.ed.status = Some((format!("Cannot race: {why}"), true));
            return Ok(());
        }
        let Some(event) = base_event(&self.game_data.career, &race) else {
            me.ed.status = Some((format!("No event of this mode on track {}", race.track), true));
            return Ok(());
        };
        if let Err(e) = save(&me.ed) {
            me.ed.status = Some((format!("Not saved: {e}"), true));
            return Ok(());
        }
        self.map_editor_close()?;
        log::info!("map editor: racing {} on track {} (event {event})", race.id, race.track);
        self.sim.map_race = Some(race);
        let r = self.start_race(event, true, false);
        if r.is_err() {
            self.sim.map_race = None;
        }
        r
    }

    /// A custom race's route in place of the track's own
    /// (`drive_to_grid`), while one is being raced on that track.
    pub(crate) fn map_race_route(&self, track: &career::Track) -> Option<Result<career::Route>> {
        let race = self.sim.map_race.as_ref().filter(|r| r.track == track.number)?;
        Some(map::race::road_net(&self.game_data.root, track).and_then(|net| map::race::route(&net, race, &|_| None)))
    }
}

/// The mod saved and listed as enabled; the game's enabled mods read again.
fn save(ed: &Editor) -> Result<std::path::PathBuf> {
    let dir = store::maps_dir();
    let mut m = ed.map.clone();
    if m.name.is_empty() {
        m.name = ed.folder.clone();
    }
    let path = store::save(&dir, &ed.folder, &m)?;
    store::set_enabled(&dir, &ed.folder, true)?;
    map::reload_active();
    Ok(path)
}

/// The career event a custom race borrows its set-up from: one on its
/// base track of the same race mode (circuit and sprint share theirs).
fn base_event(career: &career::CareerData, race: &CustomRace) -> Option<usize> {
    let want = match race.kind {
        RaceKind::Circuit | RaceKind::Sprint => nfsu2::race::RaceMode::FreeRoam,
        RaceKind::StreetX => nfsu2::race::RaceMode::StreetX,
        RaceKind::Drift => nfsu2::race::RaceMode::Drift,
    };
    // The race's first round is the one raced (`start_round`), on its
    // track and in its direction.
    let fits = |e: &&career::Event| {
        let (track, variant) = e.races.first().map_or((e.track, e.variant), |r| (r.0, r.1));
        track == race.track && (variant & 1 != 0) == race.backward && e.race_mode() == want
    };
    let same_kind = |e: &&career::Event| match race.kind {
        RaceKind::Circuit => e.kind == career::EventKind::Circuit,
        RaceKind::Sprint => e.kind == career::EventKind::Sprint,
        _ => true,
    };
    career.events.iter().filter(fits).find(same_kind).or_else(|| career.events.iter().find(fits)).map(|e| e.index)
}

/// The race track of `region` whose own start lies nearest `at` (the base
/// track a new race starts on).
fn nearest_track(root: &std::path::Path, career: &career::CareerData, region: &str, at: Vec2) -> Option<u16> {
    let markers =
        career::load_markers(&nfsu2::game_fs::path(root, &format!("TRACKS/ROUTES{region}/TrackPosMarkersAll.bin")))
            .ok()?;
    let start = nfsu2::formats::hash::bin_hash("ForwardStartLine");
    markers
        .iter()
        .filter(|m| m.name == start)
        .filter(|m| career.tracks.get(&m.track).is_some_and(|t| t.region == region))
        .filter(|m| career.events.iter().any(|e| e.races.first().map_or(e.track, |r| r.0) == m.track))
        .min_by(|a, b| a.position.truncate().distance(at).total_cmp(&b.position.truncate().distance(at)))
        .map(|m| m.track)
}

/// How many points a race part has.
trait PartLen {
    fn part_len(&self, part: RacePart) -> usize;
}

impl PartLen for CustomRace {
    fn part_len(&self, part: RacePart) -> usize {
        match part {
            RacePart::Checkpoint => self.checkpoints.len(),
            RacePart::Grid => self.grid.len(),
            RacePart::Start => usize::from(self.start.is_some()),
            RacePart::Finish => usize::from(self.finish.is_some()),
        }
    }
}
