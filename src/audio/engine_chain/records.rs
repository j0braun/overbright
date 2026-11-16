#[derive(Clone, Copy, Debug)]
pub struct Rec80 {
    pub gin: &'static str,
    pub hybrid: bool,
    pub decel: usize,
    pub rpm_max: f32,
    pub rpm_min: f32,
    pub vol_a: i32,
    pub vol_b: i32,
    pub d0: f32,
    pub a1: f32,
    pub b1: f32,
    pub a0: f32,
    pub b0: f32,
    pub fixed_a: f32,
    pub fixed_b: f32,
    pub cutoff: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct Rec84 {
    pub gin: &'static str,
    pub vol_a: i32,
    pub vol_d: i32,
    pub rpm0: i32,
    pub rpm1: i32,
    pub up: f32,
    pub down: f32,
    pub d0: f32,
    pub a1: f32,
    pub b1: f32,
    pub a0: f32,
    pub b0: f32,
}

/// The engine records (0x7F0DE0, 0x4C bytes): accel GIN (+0x0C), hybrid flag (+0x10),
/// decel record (+0x14), rpm max / min (+0x1C / +0x20), layer volumes (+0x24 / +0x28),
/// +0xD0 normaliser (+0x2C), A / B gains at t = 1 (+0x30 / +0x34) and t = 0 (+0x38 / +0x3C),
/// fixed gains (+0x40 / +0x44), minimum cutoff (+0x48).
pub static ENGINES: crate::exe_tables::Table<[Rec80; 43]> = crate::exe_tables::Table::new(|t| &t.engines);

/// The decel records (0x7F1AA8, 0x34 bytes): the _DCL GIN (+0), layer-A volume off
/// throttle (+4), decel volume (+8), trapezoid rpm ends (+0xC / +0x10), its ramp
/// fractions (+0x14 / +0x18), |+0xD0| normaliser (+0x1C), A / B gains at t2 = 1
/// (+0x24 / +0x28) and 0 (+0x2C / +0x30).
pub static DECELS: crate::exe_tables::Table<[Rec84; 37]> = crate::exe_tables::Table::new(|t| &t.decels);
