use super::*;

/// The tuning categories of UI_PerformanceTuning_Master (0x557690 adds
/// them in this order through 0x522A80): icon, label and the parameters
/// whose opening opens the category (0x5225D0).
pub const CATEGORIES: [(u32, u32, std::ops::RangeInclusive<u8>); 7] = [
    (0x1784_75E7, 0xEF37_72C1, 0..=8),   // Suspension
    (0x01A2_9FFA, 0xE8DA_9E62, 9..=15),  // Drivetrain
    (0x214D_B7CF, 0x7ED5_54D1, 21..=22), // Aerodynamics
    (0xD6DC_23EE, 0x6741_BF48, 18..=20), // Tires & Brakes
    (0x502A_BEED, 0x135B_DD07, 23..=23), // ECU
    (0x06EF_789C, 0x5AE0_8536, 24..=24), // Turbo
    (0x9701_BDE4, 0x135C_04DA, 16..=17), // Nitrous
];

/// The record that opens `param` for a car with these parts fitted
/// (0x59AE50): of the fitted parts' records, the highest level.
pub fn opener<'a>(table: &'a [Unlock], fitted: &dyn Fn(u8) -> bool, param: u8) -> Option<&'a Unlock> {
    table.iter().filter(|u| u.param == param && fitted(u.part)).fold(None, |best: Option<&Unlock>, u| match best {
        Some(b) if b.level >= u.level => Some(b),
        _ => Some(u),
    })
}

/// Whether `param` can be tuned (0x59B090): opened by a fitted part, and
/// for the gear ratios 10–15 a gear the car has (`gears` forward gears).
pub fn tunable(table: &[Unlock], fitted: &dyn Fn(u8) -> bool, gears: u32, param: u8) -> bool {
    let open = opener(table, fitted, param).is_some();
    if (10..16).contains(&param) {
        return open && (param as i32 - 8) < gears as i32 + 2;
    }
    open
}

/// Whether a category of [`CATEGORIES`] has a parameter to tune (0x5225D0).
pub fn category_open(table: &[Unlock], fitted: &dyn Fn(u8) -> bool, gears: u32, category: usize) -> bool {
    CATEGORIES.get(category).is_some_and(|c| c.2.clone().any(|p| tunable(table, fitted, gears, p)))
}

/// The 41 slider bytes of a car's tuning (tuning object +0x14, the bytes
/// Test Run copies from 0x83A540 +0x14; compared by 0x59AEA0): one per
/// parameter 0–22, then nine points each for the ECU (23) at +0x17 and
/// the turbo (24) at +0x20. A byte is 0–200; it reads as
/// t = clamp((b − 100) × 0.01, −1, 1) (0x59AEF0) and is written as
/// round(t × 100 + 100.5) clamped to 0–200 (0x59AF90, truncating). New
/// settings are all 100 (t = 0: 0x5A7940, the tuning object's 0x5A7980);
/// 0x59AFF0 resets one parameter, or all with −1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings(pub [u8; 41]);

impl Default for Settings {
    fn default() -> Self {
        Settings([100; 41])
    }
}

impl Settings {
    fn index(param: u8, point: usize) -> Option<usize> {
        match param {
            0..=22 => Some(param as usize),
            23 => Some(0x17 + point.min(8)),
            24 => Some(0x20 + point.min(8)),
            _ => None,
        }
    }

    /// The slider position −1..1 of `param` (0x59AEF0); 0 for a parameter
    /// past 24.
    pub fn t(&self, param: u8, point: usize) -> f32 {
        Settings::index(param, point).map_or(0.0, |i| ((self.0[i] as f32 - 100.0) * 0.01).clamp(-1.0, 1.0))
    }

    /// Store a slider position (0x59AF90).
    pub fn set_t(&mut self, param: u8, point: usize, t: f32) {
        if let Some(i) = Settings::index(param, point) {
            self.0[i] = ((t * 100.0 + 100.5) as i32).clamp(0, 200) as u8;
        }
    }

    /// The 41 bytes as hex, for the profile file.
    pub fn hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Read [`Settings::hex`] back.
    pub fn from_hex(hex: &str) -> Option<Settings> {
        if hex.len() != 82 {
            return None;
        }
        let mut s = Settings::default();
        for (i, b) in s.0.iter_mut().enumerate() {
            *b = u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()?;
        }
        Some(s)
    }

    /// Reset a parameter's sliders to the middle (0x59AFF0 on the tuning
    /// object +0x14 through 0x59B160): 0–22 one byte (0x59AF90(0, p)), the
    /// ECU (23) and turbo (24) all nine points, −1 everything.
    pub fn reset(&mut self, param: i32) {
        match param {
            -1 => *self = Settings::default(),
            0..=22 => self.set_t(param as u8, 0, 0.0),
            23 | 24 => (0..9).for_each(|i| self.set_t(param as u8, i, 0.0)),
            _ => {}
        }
    }

    /// The tuning screens' BUTTON1 (0xC519BFC0, "Default"; 0x5650D0 for
    /// the sliders, 0x5654E0 for the graph / nitrous widget): 0x50F9B0
    /// resets the parameter, and with 17 (the nitrous partner) 16 as well.
    pub fn restore(&mut self, param: u8) {
        self.reset(param as i32);
        if param == 17 {
            self.reset(16);
        }
    }

    /// Whether every slider is at its middle (nothing tuned).
    pub fn is_default(&self) -> bool {
        *self == Settings::default()
    }
}

/// The car-type record offset of a parameter's field (0x59B210, which
/// works on the tuning block = record +0x110; offsets here are the
/// record's). `i` is the second index some fields take: the pair's member,
/// a wheel pair, or a curve point. Suspension blocks are at +0x280 (front)
/// / +0x2A0 (rear), the wheels 0x30 apart from +0x120, gear ratios from
/// +0x2E0 (reverse, neutral, 1st…), the torque curve at +0x310, the turbo
/// curve at +0x334.
pub fn field(param: u8, i: usize) -> Option<usize> {
    Some(match param {
        0 => 0x284,
        1 => 0x288 + 4 * i,
        2 => 0x290,
        3 => 0x128 + 0x30 * i,
        4 => 0x2A4,
        5 => 0x2A8 + 4 * i,
        6 => 0x2B0,
        7 => 0x188 + 0x30 * i,
        8 => 0x380,
        9 => 0x2C8 + 4 * i,
        10..=15 => 0x2C0 + 4 * param as usize,
        16 => 0x3B0,
        17 => 0x3BC,
        18 => 0x38C,
        19 => 0x1EC + 4 * i,
        20 => 0x20C + 4 * i,
        21 => 0x374,
        22 => 0x378,
        23 => 0x310 + 4 * i,
        24 => 0x334 + 4 * i,
        _ => return None,
    })
}
