use crate::*;

impl Game {
    /// The career car's visual rating (0x5234D0) with its fitted parts.
    pub(crate) fn visual_rating(&self) -> f32 {
        self.profile.as_ref().map_or(0.0, |p| self.visual_rating_of(&p.installed))
    }

    /// The visual rating (0x5234D0) of a ride: the car as fitted, or the
    /// shops' preview ride (the FE car's +0x50, `preview_ride`), which
    /// 0x55B1B0 rates after each fit.
    pub(crate) fn visual_rating_of(&self, ride: &std::collections::BTreeMap<u8, u32>) -> f32 {
        let Some(p) = self.profile.as_ref() else { return 0.0 };
        self.visual_rating_car(&p.car, ride)
    }

    /// The visual rating (0x5234D0) of car `car` with parts `ride`
    /// (`car::ride_visual_rating`; matched against the original's 0x528420
    /// / 0x5234D0 for 14 saves and 293 165 swept rides,
    /// docs/visual-rating.md).
    pub(crate) fn visual_rating_car(&self, car: &str, ride: &std::collections::BTreeMap<u8, u32>) -> f32 {
        let stock_paint = self.game_data.car_types.iter().find(|t| t.name == car).map_or(0, |t| t.stock_paint);
        nfsu2::car::ride_visual_rating(&self.game_data.parts, &self.game_data.career.costs, car, stock_paint, ride)
    }
}
