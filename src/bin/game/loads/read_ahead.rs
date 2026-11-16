//! What the game reads ahead into the system's file cache
//! (`nfsu2::cache::read_ahead`, docs/read-ahead.md): the files of the
//! minutes after the window's first frame, in the order the game reads
//! them. Each list holds only files a load on that way opens and reads;
//! nothing here changes what is loaded.
use crate::*;

impl Game {
    /// The files of the minutes after the first frame that a frame on the
    /// main thread reads, in the order the game reads them: the boot
    /// screens' movies still to come (each read whole when it opens, also
    /// when Enter skips the one before); the main menu's FE car and its
    /// showroom (the main menu's first frame); the career garage's platform
    /// (its first frame); what free roam's first frames read (its routes,
    /// its minimap, the drive's sound banks). Not free roam's city and car:
    /// the menus read them from the first frame on (`Preload`).
    pub(crate) fn read_ahead_files(&self) -> Vec<std::path::PathBuf> {
        let root = &self.game_data.root;
        let mut files = Vec::new();
        if let Mode::Menu(Menu::Movie(m)) = &self.mode
            && let Some(first) = m.boot
        {
            let mut step = Some(first);
            while let Some(s) = step {
                files.extend(s.movie().and_then(|(_, _, name)| nfsu2::movie::path(root, name)));
                step = s.next();
            }
        }
        if let Some(car) = self.main_menu_car().0 {
            files.extend(nfsu2::car::read_ahead_files(root, &car));
        }
        files.extend(nfsu2::garage::platform_file(root, nfsu2::garage::PLATFORMS[1]).ok());
        if self.profile.is_some() {
            files.extend(nfsu2::garage::platform_file(root, nfsu2::garage::PLATFORMS[4]).ok());
        }
        for f in ["RoutesFreeRoam.bin", "PathsFreeRoam.bin"] {
            files.push(nfsu2::game_fs::path(root, &format!("TRACKS/ROUTES{EXPLORE_REGION}/{f}")));
        }
        files.push(nfsu2::game_fs::path(root, "TRACKS/TRACKMAP4000.BIN"));
        files.extend(nfsu2::audio::banks::cache::DRIVE_BANKS.iter().map(|b| nfsu2::game_fs::path(root, b)));
        files
    }

    /// Once, after the window's first frame: [`Game::read_ahead_files`]
    /// queued (`nfsu2::cache::read_ahead`).
    pub(crate) fn read_ahead_after_start(&mut self) {
        if std::mem::replace(&mut self.loads.read_ahead_queued, true) || !nfsu2::cache::read_ahead::enabled() {
            return;
        }
        nfsu2::cache::read_ahead::queue(self.read_ahead_files());
    }
}
