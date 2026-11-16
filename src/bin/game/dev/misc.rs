use crate::*;

impl Game {
    /// The nearest stage event that starts in the explore region, and its
    /// start.
    pub(crate) fn nearest_event(&self) -> Option<(usize, Vec3, f32)> {
        let drive = self.sim.drive.as_ref()?;
        let profile = self.profile.as_ref()?;
        let at = drive.state.position;
        let mut best: Option<(usize, Vec3, f32)> = None;
        for e in profile.open_events(&self.game_data.career) {
            let Some(t) = self.game_data.career.tracks.get(&e.track) else { continue };
            if t.region != EXPLORE_REGION {
                continue;
            }
            let Ok(route) = career::load_route(&self.game_data.root, t, e.variant & 1 != 0) else { continue };
            let start =
                route.start_line.or(route.grid.first().map(|g| g.0)).unwrap_or(route.checkpoints[0].extend(at.z));
            let d = start.truncate().distance(at.truncate());
            if best.is_none_or(|b| d < b.2) {
                best = Some((e.index, start, d));
            }
        }
        best
    }
}

/// The mode in a check's message: [`ModeText::Check`].
pub(crate) fn mode_name(m: &Mode) -> String {
    describe_mode(m, ModeText::Check)
}
