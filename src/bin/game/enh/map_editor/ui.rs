use super::*;
use nfsu2::tools::overlay::{Input, Panel, colour};

/// The keys, by page.
const KEYS_COMMON: &str =
    "WASD/RF fly  Shift fast  drag/IJKL look  Tab page  Ctrl+S save  F5 reload  G car here  H help  Esc close";
const KEYS_PROPS: &str = "Space pick  arrows/PgUp/PgDn move  Q/E turn  Del delete  C copy to aim  Z restore";
const KEYS_RACES: &str = "N new  [ ] race  P part  , . point  J jump  Enter place  Del remove  arrows move  M mode  +/- laps  T race  Ctrl+X delete race";

impl Game {
    /// The editor's quads: the markers of the race being edited and the
    /// picked prop in the view, the crosshair, and the panel at the left.
    pub(crate) fn map_editor_quads(&self) -> Option<Vec<Quad>> {
        let me = self.map_editor.as_ref()?;
        let mut out = Vec::new();
        let assets = &self.game_data.assets;
        let Some(font) = assets
            .fonts
            .get(&nfsu2::formats::hash::bin_hash("CONDUITMDITC_TT21I"))
            .or_else(|| assets.fonts.values().next())
        else {
            return Some(out);
        };
        let ed = &me.ed;
        let aspect =
            self.platform.window.as_ref().map_or(4.0 / 3.0, |w| w.config.width as f32 / w.config.height.max(1) as f32);
        let view = ed.camera.view_projection(aspect);
        // A world point on the 640 × 480 canvas (centred, scaled by height).
        let canvas = |p: Vec3| -> Option<Vec2> {
            let c = view * p.extend(1.0);
            if c.w < 0.2 {
                return None;
            }
            let ndc = c.truncate().truncate() / c.w;
            (ndc.x.abs() < 1.2 && ndc.y.abs() < 1.2)
                .then(|| Vec2::new(320.0 + ndc.x * 240.0 * aspect, 240.0 - ndc.y * 240.0))
        };
        let dot = |out: &mut Vec<Quad>, at: Vec2, size: f32, color: [f32; 4]| {
            out.push(Quad {
                texture: None,
                rect: [at - Vec2::splat(size), at + Vec2::splat(size)],
                uv: [Vec2::ZERO, Vec2::ONE],
                color,
                z: 0.5,
                angle: 0.0,
                place: nfsu2::render::display::Place::CENTRE,
            });
        };
        let label = |out: &mut Vec<Quad>, at: Vec2, text: &str, color: [f32; 4]| {
            nfsu2::frontend::text_quads(
                assets,
                font,
                text,
                1,
                Vec2::splat(0.32),
                at + Vec2::new(0.0, -14.0),
                color,
                0.5,
                out,
            );
        };
        let start = out.len();
        let world = self.sim.drive.as_ref().map(|d| &d.world);
        let ground = |p: Vec2, z: f32| world.and_then(|w| w.grid_ground(p.extend(z + 2.0))).unwrap_or(z);
        if ed.page == Page::Races
            && let Some(r) = ed.race.and_then(|k| ed.map.races.get(k))
        {
            let z0 = r.start.map_or(ed.camera.position.z - 2.0, |s| s.position.z);
            // The road the race runs, as dots.
            if let Some((_, Ok(points))) = &me.preview {
                let mut last: Option<Vec2> = None;
                for p in points {
                    if last.is_some_and(|l| l.distance(*p) < 8.0) {
                        continue;
                    }
                    last = Some(*p);
                    if let Some(c) = canvas(p.extend(ground(*p, z0) + 0.3)) {
                        dot(&mut out, c, 1.2, [0.3, 0.8, 1.0, 0.8]);
                    }
                }
            }
            for (k, c) in r.checkpoints.iter().enumerate() {
                let sel = ed.part == RacePart::Checkpoint && ed.point == k;
                if let Some(s) = canvas(c.extend(ground(*c, z0) + 1.0)) {
                    let col = if sel { colour::FOCUS } else { colour::TITLE };
                    dot(&mut out, s, if sel { 4.0 } else { 3.0 }, col);
                    label(&mut out, s, &format!("{}", k + 1), col);
                }
            }
            for (k, g) in r.grid.iter().enumerate() {
                let sel = ed.part == RacePart::Grid && ed.point == k;
                if let Some(s) = canvas(g.position + Vec3::Z) {
                    let col = if sel { colour::FOCUS } else { colour::OK };
                    dot(&mut out, s, 2.5, col);
                    label(&mut out, s, &format!("G{}", k + 1), col);
                }
            }
            for (spot, name, part) in [(r.start, "START", RacePart::Start), (r.finish, "FINISH", RacePart::Finish)] {
                if let Some(sp) = spot
                    && let Some(s) = canvas(sp.position + Vec3::Z * 1.5)
                {
                    let col = if ed.part == part { colour::FOCUS } else { colour::TEXT };
                    dot(&mut out, s, 3.0, col);
                    label(&mut out, s, name, col);
                }
            }
        }
        if ed.page == Page::Props
            && let (Some(i), Some(w)) = (ed.selected, world)
            && let Some((c, _)) = edit::placement_sphere(w, i)
            && let Some(s) = canvas(c)
        {
            dot(&mut out, s, 3.0, colour::FOCUS);
            label(&mut out, s, &w.solids[w.placements[i].solid].name, colour::FOCUS);
        }
        // The crosshair: what Space picks and where Enter / C place.
        dot(&mut out, Vec2::new(320.0, 240.0), 1.5, [1.0, 1.0, 1.0, 0.9]);
        for q in &mut out[start..] {
            q.place = nfsu2::render::display::Place::CENTRE;
        }
        if !me.help {
            return Some(out);
        }
        // The panel.
        let input = Input::default();
        let mut p = Panel::new(font, &input, Vec2::new(6.0, 6.0), 250.0);
        p.title(&format!("MAP EDITOR - {}{}", ed.folder, if ed.dirty { " *" } else { "" }));
        let page = match ed.page {
            Page::Props => "Page: Props (Tab: Races)",
            Page::Races => "Page: Races (Tab: Props)",
        };
        p.label(page, colour::TEXT);
        let props = ed.map.props.len();
        p.label(&format!("{props} prop edit(s), {} race(s)", ed.map.races.len()), colour::DIM);
        match ed.page {
            Page::Props => {
                match (ed.selected, world) {
                    (Some(i), Some(w)) => {
                        let pl = &w.placements[i];
                        let at = pl.transform.w_axis.truncate();
                        p.label(&format!("{} (section {})", w.solids[pl.solid].name, pl.section), colour::TEXT);
                        p.label(&format!("at {:.2} {:.2} {:.2}", at.x, at.y, at.z), colour::DIM);
                    }
                    _ => p.label("Nothing picked: aim and press Space", colour::DIM),
                }
                if ed.needs_reload {
                    p.label("Added props show after F5 (reload)", colour::WARN);
                }
                p.label("Collision of static scenery stays as the original", colour::DIM);
            }
            Page::Races => match ed.race.and_then(|k| ed.map.races.get(k)) {
                None => p.label("No race: aim at a road and press N", colour::DIM),
                Some(r) => {
                    let k = ed.race.unwrap_or(0);
                    p.label(&format!("Race {}/{}: {} ({})", k + 1, ed.map.races.len(), r.name, r.id), colour::TEXT);
                    let laps = if r.kind.loops() { format!(", {} laps", r.laps) } else { String::new() };
                    p.label(&format!("Track {} {}{laps}", r.track, r.kind.name()), colour::TEXT);
                    p.label(
                        &format!(
                            "{} checkpoints, grid {}, editing {} #{}",
                            r.checkpoints.len(),
                            r.grid.len(),
                            ed.part.name(),
                            ed.point + 1
                        ),
                        colour::DIM,
                    );
                    match &me.preview {
                        Some((_, Ok(points))) => {
                            let len: f32 = points.windows(2).map(|w| w[0].distance(w[1])).sum();
                            p.label(&format!("Route OK: {:.2} km", len / 1000.0), colour::OK)
                        }
                        Some((_, Err(why))) => p.label(&format!("Route: {why}"), colour::WARN),
                        None => {}
                    }
                }
            },
        }
        if let Some((text, warn)) = &ed.status {
            p.label(text, if *warn { colour::WARN } else { colour::OK });
        }
        p.space(4.0);
        for line in [KEYS_COMMON, if ed.page == Page::Props { KEYS_PROPS } else { KEYS_RACES }] {
            for chunk in wrap(line, 56) {
                p.label(&chunk, colour::DIM);
            }
        }
        p.finish().quads(assets, font, &mut out);
        Some(out)
    }
}

/// `text` broken at spaces into lines of at most `width` characters.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in text.split("  ") {
        let last = lines.last_mut().unwrap();
        if !last.is_empty() && last.len() + 2 + word.len() > width {
            lines.push(word.to_string());
        } else {
            if !last.is_empty() {
                last.push_str("  ");
            }
            last.push_str(word);
        }
    }
    lines
}
