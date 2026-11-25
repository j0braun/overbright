//! The race mode the handling and the car's paint read (the race type flags 0x89E7D8…).

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RaceMode {
    /// No race: every flag clear.
    #[default]
    FreeRoam,
    /// Race type 4 (0x89E7D8).
    Drag,
    /// Race type 5 (0x89E7D9), the drift events.
    Drift,
    /// Race type 2 (0x89E7E3), the street X events.
    StreetX,
    /// The flag 0x89E7E2 the handling reads (0x5A4FF7, 0x5ABC1D,
    /// 0x5B634D): set offline only by Quick Race type 11 (0x53FC58's
    /// switch → 0x526730), which the mode menu does not offer; the career
    /// never sets it, and its URL events use the plain flags. The name is
    /// the clone's.
    Url,
}

impl RaceMode {
    /// From a name, as in `NFS_RACE=drag`.
    pub fn parse(name: &str) -> Option<RaceMode> {
        Some(match name.to_ascii_lowercase().as_str() {
            "" | "freeroam" | "free" => RaceMode::FreeRoam,
            "drag" => RaceMode::Drag,
            "drift" => RaceMode::Drift,
            "streetx" | "street" => RaceMode::StreetX,
            "url" => RaceMode::Url,
            _ => return None,
        })
    }

    pub fn drag(self) -> bool {
        self == RaceMode::Drag
    }

    /// The game's drift test: a drift flag and not a drag race.
    pub fn drift(self) -> bool {
        self == RaceMode::Drift
    }

    pub fn street_x(self) -> bool {
        self == RaceMode::StreetX
    }

    pub fn url(self) -> bool {
        self == RaceMode::Url
    }
}
