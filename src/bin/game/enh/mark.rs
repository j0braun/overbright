//! "Mark This Spot" (docs/playing/mark-spot.md): the Enhancements page's Mark
//! Spot Key collects what the player sees into a `nfsu2::mark_spot::Spot`
//! and writes the report. Clone tooling; nothing native changes.
use crate::*;

/// The Mark Spot Key row's index for a key (`mark_spot::KEYS`), 0 none.
pub(crate) fn key_index(code: KeyCode) -> u8 {
    match code {
        KeyCode::F9 => 1,
        KeyCode::F10 => 2,
        KeyCode::F11 => 3,
        KeyCode::F12 => 4,
        _ => 0,
    }
}

impl Game {
    /// What the player sees now.
    pub(crate) fn mark_spot_now(&self) -> nfsu2::mark_spot::Spot {
        let mut s = nfsu2::mark_spot::Spot {
            mode: match &self.mode {
                Mode::Explore => "explore",
                Mode::Race(_) => "race",
                Mode::Menu(_) => "menu",
            }
            .into(),
            game_time: f64::from(self.now()) / 1000.0,
            size: nfsu2::qol::output_size_or((1280, 960)),
            options: self.settings.options_text(),
            profile_save: profiles::current_save().map(|n| n.to_bytes()),
            game_files: nfsu2::fingerprint::current().map(|f| f.summary()),
            ..Default::default()
        };
        if let Mode::Race(r) = &self.mode {
            s.event = self.game_data.career.events.get(r.event).map(|e| e.name.clone());
        }
        if let Some(d) = self.sim.drive.as_ref() {
            s.region = Some(d.region.clone());
            s.car = Some(d.car.car_type.name.clone());
            let p = d.state.position;
            s.position = Some(p.to_array());
            let fwd = d.state.orientation * Vec3::X;
            s.heading_deg = fwd.y.atan2(fwd.x).to_degrees();
            s.speed_kmh = d.state.velocity.length() * 3.6;
            let cam = &d.camera;
            s.camera = Some(nfsu2::mark_spot::Look {
                eye: cam.position.to_array(),
                target: (cam.position + cam.forward() * 10.0).to_array(),
                fov_deg: cam.fov_y.to_degrees(),
                game_fov_deg: cam.game_fov.map(f32::to_degrees),
                up: cam.up.to_array(),
            });
            s.section = d.world.drawn_sections_at(p, nfsu2::qol::keep_loaded()).map(|(sec, _)| sec);
            s.nearest = d
                .world
                .placements
                .iter()
                .map(|pl| (pl, pl.transform.w_axis.truncate().distance(p)))
                .filter(|(pl, _)| d.world.solids.get(pl.solid).is_some())
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(pl, dist)| {
                    (d.world.solids[pl.solid].name.clone(), dist, pl.transform.w_axis.truncate().to_array())
                });
            s.world_clock = Some(d.clock);
        }
        s
    }

    /// The key pressed: write the report, show where on the HUD line.
    pub(crate) fn mark_spot(&mut self) {
        let spot = self.mark_spot_now();
        let until = self.now() + 3000.0;
        match nfsu2::mark_spot::write(&spot) {
            Ok(path) => {
                log::warn!("mark this spot: report written to {}", path.display());
                let name = path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
                self.hud_state.toast = Some((format!("SPOT MARKED: {name}"), until));
            }
            Err(e) => {
                log::warn!("mark this spot: report not written: {e}");
                self.hud_state.toast = Some(("SPOT NOT MARKED (SEE LOG)".into(), until));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_offered_keys_mark() {
        assert_eq!(key_index(KeyCode::F9), 1);
        assert_eq!(key_index(KeyCode::F12), 4);
        assert_eq!(key_index(KeyCode::F3), 0);
        assert_eq!(nfsu2::mark_spot::KEYS[usize::from(key_index(KeyCode::F10))], "F10");
    }
}
