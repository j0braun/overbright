use crate::*;

impl Game {
    /// NFSU2_DEV_SHOP=<shop name> (a test aid, not the game; used by
    /// the maintainer's preview_xtest.sh): the first time career free roam is up
    /// with no scene running, the car is put in that shop's route trigger,
    /// mid-box, so the Enter Race / Shop key engages it through the
    /// window's own loop and keys.
    pub(crate) fn dev_teleport(&mut self) {
        // NFSU2_PREVIEW_LOG: each new screen (its package, or the mode), for
        // the script to wait on.
        if nfsu2::env_set!("NFSU2_PREVIEW_LOG") {
            static LAST: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
            let now = match &self.mode {
                Mode::Menu(Menu::Dialog(d, _)) => format!("{} {:?}", d.package, d.text),
                Mode::Menu(Menu::Parts(b)) if b.part.is_some() => format!("{} list", b.package),
                Mode::Menu(m) => {
                    menu_package(m).map_or_else(|| format!("{:?}", std::mem::discriminant(m)), str::to_string)
                }
                Mode::Explore => if self.scenes.shop.is_some() { "explore-scene" } else { "explore" }.to_string(),
                _ => "other".to_string(),
            };
            let mut last = LAST.lock().unwrap();
            if *last != now {
                log::info!("screen: {now}");
                *last = now;
            }
        }
        static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if DONE.load(std::sync::atomic::Ordering::Relaxed)
            || !matches!(self.mode, Mode::Explore)
            || self.scenes.shop.is_some()
            || self.profile.is_none()
        {
            return;
        }
        let Some(name) = nfsu2::env_value!("NFSU2_DEV_SHOP") else { return };
        let Some(t) = self.game_data.career.shops.iter().find(|s| s.name == name).and_then(|s| self.sim.route_triggers.find(s.trigger))
        else {
            return;
        };
        let at = ((t.min + t.max) * 0.5).extend(t.position.z);
        let Some(d) = self.sim.drive.as_mut() else { return };
        d.state.position = at;
        self.career_world.marker_trigger = None;
        DONE.store(true, std::sync::atomic::Ordering::Relaxed);
        log::info!("NFSU2_DEV_SHOP: the car put at {name}'s trigger");
    }
}
