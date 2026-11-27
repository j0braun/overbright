//! An edit from Studio applied to the running world in place (`--studio-link`,
//! `shell::studio_link`). The world of a linked run is made with the mod's
//! own first re-apply (`tools::map::load_world`), so it keeps the original
//! placements beside the modded state; an edit then re-derives the region's
//! placements from original and edited mod (`World::reapply_map_mod`) and the
//! drive takes the result: renderer instances, scenery collision (written by
//! the same collision writer a load uses), the knock-over props, the lamps'
//! emitters. No world is loaded again and the car stays where it is.
//!
//! Props and collision are applied this way, and a race edit that is not the run's own race. A new or removed added
//! placement needs instance slots the renderer was not built with: the world
//! renderer is made again then (as Studio's viewport does), which takes a
//! moment but no load. Anything else (the run's own race, the car or clock) starts the run over
//! as before, and the status says so.
use crate::*;
use nfsu2::tools::map::MapMod;

/// What the renderer's slots were made from for the added placements
/// (everything after the originals): the solids and the section.
type Tail = Vec<(usize, Option<usize>, Option<usize>, u32, u16, u32)>;

fn tail(world: &World) -> Tail {
    let n = world.map_overlay.first_added.unwrap_or(world.placements.len());
    world.placements[n.min(world.placements.len())..]
        .iter()
        .map(|p| (p.solid, p.mid_solid, p.low_solid, p.section, p.flags & 0x0100, p.cull_radius.to_bits()))
        .collect()
}

/// The knock-over props as the physics sees them.
fn props_of(world: &World) -> Vec<(usize, [u32; 16])> {
    world.smackables.iter().map(|p| (p.placement, p.rest.to_cols_array().map(f32::to_bits))).collect()
}

/// Why an edit cannot be applied in place (None: it can). The run's own race (`race`, a race run's id) is
/// the start, route and grid of the run: a change there starts the run over. The other races are not part
/// of the world, so a free-roam run or another race's run takes their edit as it is.
pub(crate) fn needs_restart(old: &MapMod, new: &MapMod, race: Option<&str>) -> Option<&'static str> {
    let of = |m: &MapMod| race.and_then(|id| m.races.iter().find(|r| r.id == id)).cloned();
    (old.races != new.races && of(old) != of(new)).then_some("the race under test changed")
}

impl Game {
    /// Apply `new` (the mod as Studio now has it; `old` the mod the world
    /// was last made with) to the running drive in place. Ok: what was done
    /// (a short phrase); Err: why it cannot be done in place, with nothing
    /// changed.
    pub(crate) fn apply_map_live(&mut self, old: &MapMod, new: &MapMod, folder: &str, race: Option<&str>) -> Result<&'static str, String> {
        if let Some(why) = needs_restart(old, new, race) {
            return Err(why.into());
        }
        let Some(d) = self.sim.drive.as_mut() else { return Err("no run".into()) };
        if !d.world.has_live_base() {
            return Err("this world does not keep its original placements".into());
        }
        let (tail_before, props_before) = (tail(&d.world), props_of(&d.world));
        if !d.world.reapply_map_mod(&d.region, folder, new) {
            return Ok("nothing changed in the world");
        }
        let rebuilt = tail(&d.world) != tail_before;
        let changed = d.world.take_changed();
        if props_of(&d.world) != props_before {
            d.props = Props::new(&d.world);
        }
        if rebuilt {
            // The renderer's instance slots are made from the placements:
            // the frame makes it again (`render::frame`, "freshly loaded region").
            self.render.world = None;
        } else if let (Some(r), Some(w)) = (self.render.world.as_mut(), self.platform.window.as_ref()) {
            for p in changed {
                if let Some(p) = d.world.placements.get(p).map(|q| (p, q.transform, q.flags)) {
                    r.update_placement(&w.gpu, p.0, p.1, p.2);
                }
            }
        }
        nfsu2::render::modern_light_bold::load_emitters(&d.world);
        Ok(if rebuilt { "applied live (scenery rebuilt)" } else { "applied live" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::race_setup::map_test_tests::sprint_mod;
    use nfsu2::tools::map::{CollisionEdit, PropEdit, store};
    use glam::{Mat3, Mat4};

    /// The races are the run's start, not its world: only a change of the run's own race starts it over.
    #[test]
    fn only_the_runs_own_race_starts_it_over() {
        let race = |id: &str, laps: u32| {
            let mut r = nfsu2::tools::map::CustomRace::new(id, 1, nfsu2::tools::map::RaceKind::Circuit);
            r.laps = laps;
            r
        };
        let m = |a: u32, b: u32| MapMod { races: vec![race("a", a), race("b", b)], ..Default::default() };
        assert!(needs_restart(&m(1, 1), &m(1, 1), Some("a")).is_none(), "nothing changed");
        assert!(needs_restart(&m(1, 1), &m(2, 1), Some("a")).is_some(), "the run's own race");
        assert!(needs_restart(&m(1, 1), &m(1, 2), Some("a")).is_none(), "another race");
        assert!(needs_restart(&m(1, 1), &m(2, 2), None).is_none(), "a free-roam run");
        let gone = MapMod { races: vec![race("b", 1)], ..Default::default() };
        assert!(needs_restart(&m(1, 1), &gone, Some("a")).is_some(), "the run's race removed");
        assert!(needs_restart(&gone, &m(1, 1), Some("a")).is_some(), "the run's race added");
    }

    const FOLDER: &str = "test-live";
    /// The link switch and the run's one mod are process-wide: these tests take turns.
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// The nearest original placement to `at` that is plain scenery (no
    /// knock-over prop): its reference and position.
    fn scenery_near(world: &World, region: &str, at: Vec3) -> (nfsu2::tools::map::PropRef, Mat4, String) {
        let n = world.map_overlay.first_added.unwrap_or(world.placements.len());
        let i = (0..n)
            .filter(|&i| {
                let p = &world.placements[i];
                p.flags & nfsu2::world::HIDDEN_FLAG == 0
                    && !world.smackables.iter().any(|s| s.placement == i)
                    && world.solids.get(p.solid).is_some_and(|s| !s.name.is_empty())
            })
            .min_by(|&a, &b| {
                let d = |i: usize| world.placements[i].transform.w_axis.truncate().distance(at);
                d(a).total_cmp(&d(b))
            })
            .expect("scenery");
        let p = &world.placements[i];
        (world.placement_ref(region, i).expect("a name"), p.transform, world.solids[p.solid].name.clone())
    }

    fn save(m: &MapMod) {
        store::save(&store::maps_dir(), FOLDER, m).unwrap();
        nfsu2::tools::map::reload_active();
    }

    /// What a frame would draw and the car would hit: every placement (but
    /// for the scenery a drive's set-up hides, which a plain load has not
    /// done), the
    /// knock-over props' rest poses and the static collision's size.
    fn world_of(w: &World) -> (Vec<(usize, [u32; 16], u16)>, Vec<(usize, [u32; 16])>, usize) {
        (
            w.placements.iter().map(|p| (p.solid, p.transform.to_cols_array().map(f32::to_bits), p.flags & !nfsu2::world::HIDDEN_FLAG)).collect(),
            props_of(w),
            w.collision().triangle_count(),
        )
    }

    /// The world a load of `m` makes (the plain way, no link).
    fn fresh(game: &Game, region: &str, m: &MapMod) -> World {
        save(m);
        nfsu2::tools::map::set_live(false);
        let w = nfsu2::tools::map::load_world(&game.game_data.root, region).unwrap();
        assert!(!w.has_live_base(), "without the link a world keeps nothing for live edits");
        nfsu2::tools::map::set_live(true);
        assert_eq!(nfsu2::tools::map::active().len(), 1, "the edited mod is the run's mod (it passed its checks)");
        w
    }

    /// An edit from Studio moves, adds and takes back props in the running
    /// world without a load: the world is, at each step, the world a fresh
    /// load of the edited mod makes, and the car does not move.
    #[test]
    #[cfg_attr(
        any(not(ob_game), not(feature = "dev-harness")),
        ignore = "needs NFSU2_ROOT (the player's own game files) and the dev-harness test sandbox (it writes a map mod)"
    )]
    fn an_edit_applies_in_place_and_matches_a_fresh_load() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        nfsu2::tools::map::set_live(true);
        let mut game = Game::new().unwrap();
        let (race, route) = sprint_mod(&game, FOLDER);
        game.profile = None;
        let at = route.grid.first().map(|g| g.0).unwrap_or(route.checkpoints[0].extend(0.0));
        let t = crate::shell::cli::MapTest { folder: FOLDER.into(), at: Some((at, 90.0)), ..Default::default() };
        game.start_map_test(&t).unwrap();
        let d = game.sim.drive.as_ref().unwrap();
        let region = d.region.clone();
        assert!(d.world.has_live_base(), "a linked run's world keeps its originals");
        let (target, was, solid) = scenery_near(&d.world, &region, at);
        let car = d.state.position;
        let base = MapMod { name: "Test".into(), races: vec![race.clone()], ..Default::default() };
        let moved = |dx: f32| PropEdit::Move {
            target: target.clone(),
            position: was.w_axis.truncate() + Vec3::new(dx, 0.0, 0.0),
            rotation: Mat3::from_rotation_z(0.3),
        };
        let added = PropEdit::Add {
            id: "a1".into(),
            region: region.clone(),
            solid,
            position: at + Vec3::new(0.0, 8.0, 0.0),
            rotation: Mat3::IDENTITY,
        };
        let steps = [
            MapMod { props: vec![moved(4.0)], ..base.clone() },
            MapMod { props: vec![moved(-3.0)], ..base.clone() },
            MapMod { props: vec![moved(-3.0), added.clone()], ..base.clone() },
            MapMod { props: vec![PropEdit::Delete { target: target.clone() }, added], ..base.clone() },
            // A drawn wall in the road ahead of the car.
            MapMod {
                collision: vec![CollisionEdit::Wall {
                    id: "w1".into(),
                    region: region.clone(),
                    from: at + Vec3::new(5.0, 10.0, 0.0),
                    to: at + Vec3::new(12.0, 10.0, 0.0),
                    height: 3.0,
                    surface: 0,
                }],
                ..base.clone()
            },
            base.clone(),
        ];
        let mut old = base.clone();
        let mut tris = 0;
        for (k, m) in steps.iter().enumerate() {
            let hid_before = game.sim.drive.as_ref().unwrap().world.placements.iter().filter(|p| p.flags & nfsu2::world::HIDDEN_FLAG != 0).count();
            save(m);
            let how = game.apply_map_live(&old, m, FOLDER, Some(&race.id)).unwrap_or_else(|e| panic!("step {k}: {e}"));
            let expect_rebuilt = k == 2 || k == 4;
            assert_eq!(how.contains("rebuilt"), expect_rebuilt, "step {k}: {how}");
            let wf = fresh(&game, &region, m);
            let want = world_of(&wf);
            let d = game.sim.drive.as_mut().unwrap();
            let got = world_of(&d.world);
            assert_eq!(got.0.len(), want.0.len(), "step {k}: placements");
            let diff: Vec<usize> = (0..got.0.len()).filter(|&i| got.0[i] != want.0[i]).collect();
            if let Some(&i) = diff.first() {
                let w = &d.world.placements[i].transform;
                eprintln!("target placement {:?} got {w:?} want {:?}", d.world.find_placement(&target), wf.placements[i].transform);
            }
            assert!(diff.is_empty(), "step {k}: placements differ at {diff:?} ({:?} vs {:?})", got.0.get(diff[0]), want.0.get(diff[0]));
            assert!(got.1 == want.1, "step {k}: knock-over props");
            assert_eq!(got.2, want.2, "step {k}: collision triangles");
            if k == 4 {
                assert!(got.2 != tris, "the drawn wall is in the collision");
            }
            tris = got.2;
            assert_eq!(d.state.position, car, "step {k}: the car stays");
            let hidden = |w: &World| w.placements.iter().filter(|p| p.flags & nfsu2::world::HIDDEN_FLAG != 0).count();
            let deleted = m.props.iter().any(|e| matches!(e, PropEdit::Delete { .. }));
            if deleted {
                assert_eq!(hidden(&d.world), hid_before + 1, "step {k}: the deleted prop is hidden");
            }
            let _ = d.world.take_changed();
            old = m.clone();
        }
        // A changed race is not applied in place.
        let mut other = base.clone();
        other.races[0].checkpoints.pop();
        assert!(game.apply_map_live(&base, &other, FOLDER, Some(&race.id)).is_err(), "the run's own race");
        // A free-roam run, or another race's run, takes it as it is.
        assert!(game.apply_map_live(&base, &other, FOLDER, None).is_ok());
        assert!(game.apply_map_live(&other, &base, FOLDER, Some("another")).is_ok());
        nfsu2::tools::map::set_live(false);
    }

    /// The same drive over a second at each frame rate: an edit applied in
    /// place at the start, and a fresh start with the edit, end at the same
    /// car state.
    #[test]
    #[cfg_attr(
        any(not(ob_game), not(feature = "dev-harness")),
        ignore = "needs NFSU2_ROOT (the player's own game files) and the dev-harness test sandbox (it writes a map mod)"
    )]
    fn an_applied_edit_drives_as_a_fresh_start_at_any_frame_rate() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        nfsu2::tools::map::set_live(true);
        for fps in [30.0f32, 60.0, 120.0, 200.0] {
            let run = |in_place: bool| {
                let mut game = Game::new().unwrap();
                let (race, route) = sprint_mod(&game, FOLDER);
                game.profile = None;
                let at = route.grid.first().map(|g| g.0).unwrap_or(route.checkpoints[0].extend(0.0));
                let t = crate::shell::cli::MapTest { folder: FOLDER.into(), at: Some((at, 90.0)), ..Default::default() };
                let base = MapMod { name: "Test".into(), races: vec![race], ..Default::default() };
                let probe = {
                    game.start_map_test(&t).unwrap();
                    let d = game.sim.drive.as_ref().unwrap();
                    scenery_near(&d.world, &d.region, at)
                };
                let edit = MapMod {
                    props: vec![PropEdit::Move {
                        target: probe.0.clone(),
                        position: probe.1.w_axis.truncate() + Vec3::new(2.0, 0.0, 0.0),
                        rotation: Mat3::from_rotation_z(0.3),
                    }],
                    ..base.clone()
                };
                save(&base);
                game.start_map_test(&t).unwrap();
                save(&edit);
                if in_place {
                    game.apply_map_live(&base, &edit, FOLDER, None).unwrap();
                } else {
                    game.restart_map_test(&t).unwrap();
                }
                for _ in 0..fps as usize {
                    game.advance(1.0 / fps).unwrap();
                }
                let d = game.sim.drive.as_ref().unwrap();
                (d.state.position.to_array().map(f32::to_bits), d.state.heading().to_bits(), world_of(&d.world))
            };
            assert!(run(true) == run(false), "{fps} fps: in place and fresh differ");
        }
        nfsu2::tools::map::set_live(false);
    }

    /// What the game does with an edit of the races, or with a car or clock, over the link (`studio_link::apply`):
    /// another race's edit goes in place; the run's own race, the car and the clock start the run over and say
    /// why; and the run after a start over drives as a fresh start at every frame rate.
    #[test]
    #[cfg_attr(
        any(not(ob_game), not(feature = "dev-harness")),
        ignore = "needs NFSU2_ROOT (the player's own game files) and the dev-harness test sandbox (it writes a map mod)"
    )]
    fn races_apply_in_place_or_start_over_with_a_reason() {
        use crate::shell::studio_link::apply;
        use nfsu2::tools::map::link::{Command, Focus, Op};
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        nfsu2::tools::map::set_live(true);
        let mut game = Game::new().unwrap();
        let (race, _) = sprint_mod(&game, FOLDER);
        game.profile = None;
        let known_now = || nfsu2::tools::map::active().first().map(|m| m.1.clone());
        let cmd = |focus: Option<Focus>| Command { seq: 1, op: Op::Apply, aim: None, focus };
        let mut other = race.clone();
        other.id = "second".into();
        let base = MapMod { name: "Test".into(), races: vec![race.clone(), other.clone()], ..Default::default() };
        // A race run: the run's own race starts it over, another race's edit does not.
        save(&base);
        let mut t = crate::shell::cli::MapTest { folder: FOLDER.into(), race: Some(race.id.clone()), ..Default::default() };
        game.start_map_test(&t).unwrap();
        let (mut known, mut done) = (known_now(), String::new());
        let mut other2 = other.clone();
        other2.name = "Edited".into();
        save(&MapMod { races: vec![race.clone(), other2], ..base.clone() });
        assert_eq!(apply(&mut game, &mut t, &cmd(None), &mut known, &mut done), Ok(false), "another race: in place");
        assert!(done.starts_with("applied live") || done.starts_with("nothing changed"), "{done}");
        let mut own = race.clone();
        own.name = "Edited".into();
        let edited = MapMod { races: vec![own, other.clone()], ..base.clone() };
        save(&edited);
        assert_eq!(apply(&mut game, &mut t, &cmd(None), &mut known, &mut done), Ok(true), "the run's own race: start over");
        assert_eq!(done, "reloaded: the race under test changed");
        // A clock: start over, and say so.
        let clock = Some(Focus { car: None, clock: Some(600.0) });
        assert_eq!(apply(&mut game, &mut t, &cmd(clock), &mut known, &mut done), Ok(true));
        assert_eq!(done, "reloaded: the car or clock changed");
        // The run after the start over is the fresh start's, at every frame rate.
        for fps in [30.0f32, 60.0, 120.0, 200.0] {
            let drive = |over_link: bool| {
                let mut game = Game::new().unwrap();
                game.profile = None;
                save(&base);
                let mut t = crate::shell::cli::MapTest { folder: FOLDER.into(), race: Some(race.id.clone()), ..Default::default() };
                game.start_map_test(&t).unwrap();
                save(&edited);
                if over_link {
                    let (mut known, mut done) = (Some(base.clone()), String::new());
                    assert_eq!(apply(&mut game, &mut t, &cmd(None), &mut known, &mut done), Ok(true));
                } else {
                    game.restart_map_test(&t).unwrap();
                }
                for _ in 0..fps as usize {
                    game.advance(1.0 / fps).unwrap();
                }
                let d = game.sim.drive.as_ref().unwrap();
                (d.state.position.to_array().map(f32::to_bits), d.state.heading().to_bits())
            };
            assert!(drive(true) == drive(false), "{fps} fps: after the start over the run differs");
        }
        nfsu2::tools::map::set_live(false);
    }
}
