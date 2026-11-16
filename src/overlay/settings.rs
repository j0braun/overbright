//! The overlay's options: layouts, corner, scale, CSV rate (options.txt `overlay.*` lines).

/// The layouts F3 cycles through.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layout {
    Minimal = 1,
    Standard = 2,
    Full = 3,
}

impl Layout {
    pub fn from_index(i: u8) -> Option<Layout> {
        match i {
            1 => Some(Layout::Minimal),
            2 => Some(Layout::Standard),
            3 => Some(Layout::Full),
            _ => None,
        }
    }

    /// F3's next layout after `shown` (None: hidden).
    pub fn cycle(shown: Option<Layout>) -> Option<Layout> {
        match shown {
            None => Some(Layout::Minimal),
            Some(Layout::Minimal) => Some(Layout::Standard),
            Some(Layout::Standard) => Some(Layout::Full),
            Some(Layout::Full) => None,
        }
    }
}

/// The Enhancements rows' names for each value.
pub const LAYOUT_NAMES: [&str; 4] = ["Off", "Minimal", "Standard", "Full"];
pub const POSITION_NAMES: [&str; 4] = ["Top Left", "Top Right", "Bottom Left", "Bottom Right"];
/// The Overlay Scale row's steps, percent.
pub const SCALES: [u8; 5] = [75, 100, 125, 150, 200];

/// The overlay's options (options.txt `overlay.*` lines, written only
/// when not the default).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// The layout shown at start: 0 none (F3 shows it, as before), else
    /// [`Layout`].
    pub layout: u8,
    /// The corner: [`POSITION_NAMES`].
    pub position: u8,
    /// Percent ([`SCALES`]).
    pub scale: u8,
    /// Log every metric to a CSV file while the game runs.
    pub csv: bool,
    /// Samples per second (2–10; options.txt only).
    pub hz: u8,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings { layout: 0, position: 0, scale: 100, csv: false, hz: 4 }
    }
}

impl Settings {
    pub fn write(&self, out: &mut String) {
        let d = Settings::default();
        if self.layout != d.layout {
            out.push_str(&format!("overlay.layout {}\n", self.layout));
        }
        if self.position != d.position {
            out.push_str(&format!("overlay.position {}\n", self.position));
        }
        if self.scale != d.scale {
            out.push_str(&format!("overlay.scale {}\n", self.scale));
        }
        if self.csv != d.csv {
            out.push_str("overlay.csv 1\n");
        }
        if self.hz != d.hz {
            out.push_str(&format!("overlay.hz {}\n", self.hz));
        }
    }

    /// An options.txt line; false if not an overlay key.
    pub fn parse(&mut self, key: &str, value: &str) -> bool {
        let n = value.trim().parse::<u8>().ok();
        match key {
            "overlay.layout" => self.layout = n.map_or(self.layout, |v| v.min(3)),
            "overlay.position" => self.position = n.map_or(self.position, |v| v.min(3)),
            "overlay.scale" => {
                self.scale = n.map_or(self.scale, |v| {
                    *SCALES.iter().min_by_key(|s| (i32::from(**s) - i32::from(v)).abs()).unwrap()
                })
            }
            "overlay.csv" => self.csv = value.trim() != "0",
            "overlay.hz" => self.hz = n.map_or(self.hz, |v| v.clamp(2, 10)),
            _ => return false,
        }
        true
    }

    /// The next (`right`) or previous scale step.
    pub fn step_scale(&mut self, right: bool) {
        let i = SCALES.iter().position(|&s| s == self.scale).unwrap_or(1);
        self.scale = SCALES[if right { (i + 1) % SCALES.len() } else { (i + SCALES.len() - 1) % SCALES.len() }];
    }
}
