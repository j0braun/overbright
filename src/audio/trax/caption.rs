/// The chyron (Chyron_ShowSong 0x4AC9F0 → 0x4AC950 / 0x4AC880 /
/// 0x4AC7D0 / 0x4CB1D0): Chyron_FE in the front end, else Chyron_IG; the
/// title, artist and album objects (0xF5387816, 0x11DB9E17, 0x48097852);
/// on 0x47FF4E7C the SHORT / MID / LONG script by the longest of the
/// three (up to 17 characters, up to 35, longer), its *_GO 7.2 s
/// (0x78E09C) later, and 1 s (0x78E0A0) after that the caption is gone.
/// Both on the real-time clock 0x8651A8 (**traced**: [`Chyron`]).
pub mod chyron {
    pub const TITLE: u32 = 0xF538_7816;
    pub const ARTIST: u32 = 0x11DB_9E17;
    pub const ALBUM: u32 = 0x4809_7852;
    pub const ROOT: u32 = 0x47FF_4E7C;
    /// 0x78E09C, s.
    pub const GO_S: f32 = 7.2;
    /// 0x78E0A0, s (after the GO).
    pub const OUT_S: f32 = 1.0;

    /// The script for the strings' longest length (0x4AC7D0).
    pub fn script(longest: usize) -> &'static str {
        if longest <= 17 {
            "SHORT"
        } else if longest <= 35 {
            "MID"
        } else {
            "LONG"
        }
    }

    /// The package and the script running: the length's script from the
    /// start, its *_GO from `go` (ms after the start) once it ran.
    pub fn phase(longest: usize, go: Option<f32>, front_end: bool) -> (&'static str, String, f32) {
        let package = if front_end { "Chyron_FE" } else { "Chyron_IG" };
        let s = script(longest);
        match go {
            None => (package, s.to_string(), 0.0),
            Some(at) => (package, format!("{s}_GO"), at),
        }
    }
}

/// A song's caption up (Chyron_SetText 0x4AC880 stamps +0x4C with the
/// real-time clock 0x8651A8 at 0x4AC8E6), stepped by the screen's tick
/// (0x4DB9C0 on 0xC98356BA, **traced**): once (0x8651A8 − +0x4C) ×
/// 0.00025 > 7.2 (0x4DB9CD..0x4DB9F3) Chyron_Go 0x4CB1D0 runs *_GO
/// (+0x55 set); on a later tick, past 7.2 + 1.0 (0x4CB207..0x4CB224), it
/// clears +0x54 and the caption is done. Before: 7200 / 8200 ms of the
/// package clock (1.7 % late). **Not ported**: the start held back while
/// 0x8364DC > 0 (0x4AC8C7, the delayed caption).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chyron {
    pub song: usize,
    /// The start on the screens' clock (ms; the scripts' origin).
    pub at: f32,
    /// +0x4C: 0x8651A8 at the start.
    pub real: u64,
    /// When *_GO ran (ms on the screens' clock), +0x55.
    pub go: Option<f32>,
}

impl Chyron {
    pub fn new(song: usize, at: f32, real: u64) -> Chyron {
        Chyron { song, at, real, go: None }
    }

    /// The tick at `real` (0x8651A8) and `now` (ms): false once the caption
    /// is done.
    pub fn step(&mut self, real: u64, now: f32) -> bool {
        if !crate::game_clock::real_over(real, self.real, chyron::GO_S) {
            return true;
        }
        if self.go.is_none() {
            self.go = Some(now);
            return true;
        }
        // x87: 1.0 + 7.2 (f32) in extended precision, 8.19999981.
        let out = f64::from(chyron::GO_S) + f64::from(chyron::OUT_S);
        crate::game_clock::real_secs(real.saturating_sub(self.real)) <= out
    }
}

/// The caption's quads at `now` (ms, the screens' clock).
pub fn chyron_quads(
    assets: &crate::frontend::Assets,
    c: &Chyron,
    now: f32,
    front_end: bool,
) -> Vec<crate::frontend::Quad> {
    use crate::frontend::screens::EA_TRAX;
    let Some(&(title, artist, album)) = EA_TRAX.get(c.song) else {
        return Vec::new();
    };
    let ms = now - c.at;
    let longest = title.len().max(artist.len()).max(album.len());
    let (name, script, at) = chyron::phase(longest, c.go.map(|g| g - c.at), front_end);
    let Some(package) = assets.package(name) else {
        return Vec::new();
    };
    let mut state = crate::frontend::ScreenState::default();
    state.text.insert(chyron::TITLE, title.to_string());
    state.text.insert(chyron::ARTIST, artist.to_string());
    state.text.insert(chyron::ALBUM, album.to_string());
    state.run(chyron::ROOT, &script, at);
    crate::frontend::layout(assets, package, &state, ms)
}

#[cfg(test)]
mod real_time_clock_tests {
    use super::Chyron;

    /// The caption's *_GO once (0x8651A8 − +0x4C) × 0.00025 > 7.2
    /// (28 800 ticks), gone on a later tick past 8.2 (32 800).
    #[test]
    fn chyron_go_and_end_on_the_real_clock() {
        let mut c = Chyron::new(3, 100.0, 1000);
        assert!(c.step(1000 + 28_799, 7000.0) && c.go.is_none());
        assert!(c.step(1000 + 28_800, 7100.0) && c.go == Some(7100.0));
        assert!(c.step(1000 + 32_799, 8000.0));
        assert!(!c.step(1000 + 32_800, 8010.0));
        // The GO and the end are never the same tick.
        let mut c = Chyron::new(3, 0.0, 0);
        assert!(c.step(40_000, 1.0) && c.go == Some(1.0));
        assert!(!c.step(40_067, 2.0));
    }
}
