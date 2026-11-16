use crate::*;

impl Game {
    /// UI_CareerWorldMap opens on stage 1 (the constructor 0x4EE690 sets
    /// +0x224 to 1).
    pub(crate) fn world_map(&mut self) -> Mode {
        self.world_map_stage(1, None)
    }

    /// The World Map on `stage`: every event of that stage in career
    /// record order (0x4E0CE0 → 0x4B1360: stage-0 events with stage 1, the
    /// D-Day pair left out, each marked won by 0x500B80), none for a stage
    /// not yet unlocked (0x500C00). `widget`: the map widget's state kept
    /// from the stage shown before (0x4E0E50 rebuilds only the list and
    /// sets +0x22C at the real-time clock).
    pub(crate) fn world_map_stage(&mut self, stage: u8, widget: Option<screens::MapWidget>) -> Mode {
        let profile = self.profile.as_ref().expect("career");
        // The top scroller lists Stage 1… while 0x500C00 finds the stage
        // record latched (0x4CBF80); there are five career stages, so a
        // finished career (the clone's stage past 5) stops at Stage 5 (Wine,
        // the finished-career save: no right arrow on Stage 5).
        let last = self.game_data.career.stages.iter().map(|d| d.stage).max().unwrap_or(5).max(1);
        let unlocked = profile.stage.clamp(1, last);
        let events: Vec<usize> = if stage <= unlocked {
            self.game_data.career.stage_events(&self.game_data.root, stage).iter().map(|e| e.index).collect()
        } else {
            Vec::new()
        };
        // The widget loads TRACKMAP<n>.BIN for the selected event's track
        // (0x4CDD40); the clone loads the stage's at once.
        for &i in &events {
            self.game_data.assets.track_map(&self.game_data.root, self.game_data.career.events[i].track);
        }
        if let Some(t) = screens::world_map_region_track(stage) {
            self.game_data.assets.track_map(&self.game_data.root, t);
        }
        let now = self.now();
        Mode::Menu(Menu::WorldMap(WorldMap {
            events,
            selected: 0,
            region: true,
            opened: now,
            stage,
            stages: unlocked,
            widget: screens::MapWidget { changed: Some(self.real_now()), ..widget.unwrap_or_default() },
        }))
    }

    /// The World Map's selected event's track (0x498600: track record
    /// +0x8A).
    pub(crate) fn world_map_track(&self, map: &WorldMap) -> Option<u16> {
        if map.region {
            return screens::world_map_region_track(map.stage);
        }
        map.event().map(|i| self.game_data.career.events[i].track)
    }
}
