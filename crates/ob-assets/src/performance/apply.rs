use crate::formats::bundle::read_f32;

use super::*;

/// Fit the upgrades to a car-type record (0x890 bytes), as 0x5AC500 does
/// to its copy of the tuning block.
pub fn upgrade(record: &mut [u8], packages: &[Package], fitted: &dyn Fn(u8) -> bool) {
    let base = 0x110;
    let get = |r: &[u8], o: usize| read_f32(r, base + o);
    let set = |r: &mut [u8], o: usize, v: f32| r[base + o..base + o + 4].copy_from_slice(&v.to_le_bytes());
    let lerp = |r: &mut [u8], o: usize, to: usize, f: f32| {
        let v = get(r, o);
        set(r, o, (get(r, to) - v) * f + v);
    };
    let add = |r: &mut [u8], o: usize, from: usize, f: f32| {
        let v = get(r, o) + get(r, from) * f;
        set(r, o, v);
    };
    let round = |x: f32| (x + 0.5) as i32;

    // Weight reduction (0x5A6B60).
    let (f, _) = level(packages, fitted, 0, 1);
    if f != 0.0 {
        add(record, 0, 0x344, f);
        lerp(record, 0x160, 0x348, f);
        let m = get(record, 0x110) * ((get(record, 0x340) - 1.0) * f + 1.0);
        set(record, 0x110, m);
    }
    // Transmission (0x5A6BE0): a gearbox block by level, then the clutch.
    let (f, g) = level(packages, fitted, 1, 3);
    if f != 0.0 {
        let l = round(g[0] * 3.0);
        if (1..=3).contains(&l) {
            let b = 0x310 + l as usize * 0x40;
            let gearbox = |r: &mut [u8], to: usize, from: usize| {
                let v = get(r, from);
                set(r, to, v);
            };
            gearbox(record, 0x1B8, b + 8);
            gearbox(record, 0x1BC, b + 0xC);
            gearbox(record, 0x1C0, b + 0x10);
            gearbox(record, 0x1B0, b);
            gearbox(record, 0x1B4, b + 4);
            for i in 0..8 {
                gearbox(record, 0x1D0 + i * 4, b + 0x20 + i * 4);
            }
            // The forward gears: the ratios after reverse and neutral up to
            // the first zero.
            let gears = (2..8).take_while(|&i| get(record, b + 0x20 + i * 4) != 0.0).count();
            record[base + 0x1C8..base + 0x1CC].copy_from_slice(&(gears as u32).to_le_bytes());
        }
        lerp(record, 0x1CC, 0x3EC, g[0]);
        lerp(record, 0x1C4, 0x3E4, g[0]);
        add(record, 0x1FC, 0x41C, g[1]);
    }
    // Nitrous (inline in 0x5AC500).
    let (f, g) = level(packages, fitted, 2, 1);
    if f != 0.0 {
        lerp(record, 0x2A0, 0x580, g[0]);
        let v = get(record, 0x58C);
        set(record, 0x2AC, v);
    }
    // Engine (0x5A6D10): the torque curve and the three after it.
    let (f, _) = level(packages, fitted, 3, 1);
    if f != 0.0 {
        for i in 0..9 {
            add(record, 0x200 + i * 4, 0x420 + i * 4, f);
        }
        for i in 0..3 {
            add(record, 0x248 + i * 4, 0x444 + i * 4, f);
        }
    }
    // Turbo (0x5A6E40): its curve towards the upgraded one, always added
    // to the torque.
    let (f, _) = level(packages, fitted, 4, 1);
    for i in 0..9 {
        lerp(record, 0x224 + i * 4, 0x510 + i * 4, f);
        let v = get(record, 0x200 + i * 4) + get(record, 0x224 + i * 4);
        set(record, 0x200 + i * 4, v);
    }
    // Suspension (0x5A6FB0), front then rear.
    let (f, g) = level(packages, fitted, 5, 3);
    if f != 0.0 {
        for axle in 0..2 {
            let p = if axle == 0 { 0x5C0 } else { 0x5E0 };
            let o = axle * 0x20;
            lerp(record, 0x18C + o, p + 0x1C, g[0]);
            lerp(record, 0x170 + o, p, g[0]);
            lerp(record, 0x174 + o, p + 4, g[0]);
            lerp(record, 0x184 + o, p + 0x14, g[0]);
            lerp(record, 0x178 + o, p + 8, g[0]);
            lerp(record, 0x188 + o, p + 0x18, g[0]);
            lerp(record, 0x17C + o, p + 0xC, g[0]);
            lerp(record, 0x180 + o, p + 0x10, g[1]);
        }
        add(record, 0x18, 0x604, g[0]);
        add(record, 0x48, 0x604, g[0]);
        add(record, 0x78, 0x608, g[0]);
        add(record, 0xA8, 0x608, g[0]);
        add(record, 0x8, 0x600, -g[0]);
        lerp(record, 0x270, 0x5BC, g[2]);
    }
    // Brakes (0x5A7150).
    let (f, _) = level(packages, fitted, 6, 1);
    if f != 0.0 {
        lerp(record, 0x27C, 0x5B8, f);
        lerp(record, 0x278, 0x5B4, f);
        lerp(record, 0x274, 0x5B0, f);
    }
    // ECU (0x5A71D0): parts 0x2D, 0x2E and 0x33 clear +0x290; the rpm
    // limits by level, then the torque.
    let (f, _) = level(packages, fitted, 7, 1);
    if fitted(0x2D) || fitted(0x2E) || fitted(0x33) {
        set(record, 0x290, 0.0);
    }
    if f != 0.0 {
        let l = round(f * 3.0);
        if (1..=3).contains(&l) {
            let b = 0x410 + l as usize * 0x40;
            add(record, 0x1F0, b, 1.0);
            add(record, 0x1F4, b + 4, 1.0);
            add(record, 0x1F8, b + 8, 1.0);
        }
        for i in 0..9 {
            add(record, 0x200 + i * 4, 0x4E0 + i * 4, f);
        }
        for i in 0..3 {
            add(record, 0x248 + i * 4, 0x504 + i * 4, f);
        }
    }
    // Tires (0x5A7370).
    let (f, _) = level(packages, fitted, 8, 1);
    if f != 0.0 {
        for (o, to) in [
            (0xD8, 0x548),
            (0xD4, 0x544),
            (0xD0, 0x540),
            (0xE0, 0x550),
            (0xDC, 0x54C),
            (0xF8, 0x568),
            (0xF4, 0x564),
            (0xF0, 0x560),
            (0x100, 0x570),
            (0xFC, 0x56C),
        ] {
            lerp(record, o, to, f);
        }
    }
    // Category 9 (0x5A74B0).
    let (f, g) = level(packages, fitted, 9, 2);
    if f != 0.0 {
        let a = get(record, 0x590) + get(record, 0x5A0);
        set(record, 0x264, get(record, 0x264) + a * g[0]);
        let b = get(record, 0x594) + get(record, 0x5A4);
        set(record, 0x268, get(record, 0x268) + b * g[1]);
        let c = get(record, 0x598) + get(record, 0x5A8);
        set(record, 0x26C, get(record, 0x26C) + c * f);
    }
    // The inertia from the (new) mass and the box (+0x114..+0x11C).
    let m = get(record, 0x110);
    let (x, y, z) = (get(record, 0x114), get(record, 0x118), get(record, 0x11C));
    let (x2, y2, z2) = (x * x, y * y, z * z);
    set(record, 0x120, (z2 + y2) * m * (1.0 / 12.0));
    set(record, 0x134, (z2 + x2) * m * (1.0 / 12.0));
    set(record, 0x148, (x2 + y2) * m * (1.0 / 12.0));
    // The performance uniques (0x5997F0, after 0x5AC500 in 0x5B4DD0 and
    // so after the inertia).
    apply_uniques(record, &|i| fitted(UNIQUE_BASE + i));
}

/// `fitted(UNIQUE_BASE + i)` asks for performance unique i (0–9, the
/// upgrade state's flag at +0x28 + 4i; ride +0x318 + 4i, set by 0x521440):
/// part ids end at 0x43, so the two never meet.
pub const UNIQUE_BASE: u8 = 0x80;

/// Apply the performance uniques to a tuning block (0x5997F0; 0x599DA0
/// takes them off again): each adds the car type's unique data, block
/// +0x2B0 on, to the block. Unique i is the category i's own: 0 the mass
/// × (1 − +0x2B0), 1 the rear torque share (+0x1C0) set to +0x2B4, 2 the
/// nitrous +0x2AC plus +0x2B8, 3 (engine) the torque curve +0x200 plus
/// +0x2D0.., 4 (turbo) the torque and the turbo curve +0x224 plus
/// +0x318.., 6 (brakes) +0x274 plus +0x2BC, 7 (ECU) the torque plus
/// +0x2F4.., 8 (tyres) +0xDC / +0xE0 / +0xFC / +0x100 plus +0x2C0 /
/// +0x2C8 / +0x2C4 / +0x2CC. 5 (suspension) and 9 change nothing here.
pub fn apply_uniques(record: &mut [u8], unique: &dyn Fn(u8) -> bool) {
    let base = 0x110;
    let get = |r: &[u8], o: usize| read_f32(r, base + o);
    let set = |r: &mut [u8], o: usize, v: f32| r[base + o..base + o + 4].copy_from_slice(&v.to_le_bytes());
    let add = |r: &mut [u8], o: usize, from: usize| {
        let v = get(r, o) + get(r, from);
        set(r, o, v);
    };
    if unique(0) {
        let m = get(record, 0x110) * (1.0 - get(record, 0x2B0));
        set(record, 0x110, m);
    }
    if unique(1) {
        let v = get(record, 0x2B4);
        set(record, 0x1C0, v);
    }
    if unique(2) {
        add(record, 0x2AC, 0x2B8);
    }
    if unique(6) {
        add(record, 0x274, 0x2BC);
    }
    if unique(8) {
        add(record, 0xDC, 0x2C0);
        add(record, 0xE0, 0x2C8);
        add(record, 0xFC, 0x2C4);
        add(record, 0x100, 0x2CC);
    }
    for i in 0..9 {
        for (u, from) in [(3, 0x2D0), (7, 0x2F4), (4, 0x318)] {
            if unique(u) {
                add(record, 0x200 + i * 4, from + i * 4);
            }
        }
        if unique(4) {
            add(record, 0x224 + i * 4, 0x318 + i * 4);
        }
    }
}

/// Take the performance uniques off a tuning block again (0x599DA0, the
/// inverse of [`apply_uniques`] but for unique 1, which it leaves): mass ÷
/// (1 − +0x2B0), the nitrous, brake and tyre terms subtracted, and each
/// torque point less the engine, ECU and turbo terms (in that order), the
/// turbo curve less its own. The tuning (0x5B4790), its bases (0x5A7DB0),
/// limits (0x5ACA60) and the screens' writer (0x5B3FB0) work on the block
/// with the uniques taken off and put them back after (0x5997F0).
pub fn remove_uniques(record: &mut [u8], unique: &dyn Fn(u8) -> bool) {
    let base = 0x110;
    let get = |r: &[u8], o: usize| read_f32(r, base + o);
    let set = |r: &mut [u8], o: usize, v: f32| r[base + o..base + o + 4].copy_from_slice(&v.to_le_bytes());
    let sub = |r: &mut [u8], o: usize, from: usize| {
        let v = get(r, o) - get(r, from);
        set(r, o, v);
    };
    if unique(0) {
        let m = get(record, 0x110) / (1.0 - get(record, 0x2B0));
        set(record, 0x110, m);
    }
    if unique(2) {
        sub(record, 0x2AC, 0x2B8);
    }
    if unique(6) {
        sub(record, 0x274, 0x2BC);
    }
    if unique(8) {
        sub(record, 0xDC, 0x2C0);
        sub(record, 0xE0, 0x2C8);
        sub(record, 0xFC, 0x2C4);
        sub(record, 0x100, 0x2CC);
    }
    for i in 0..9 {
        let mut v = get(record, 0x200 + i * 4);
        for (u, from) in [(3, 0x2D0), (7, 0x2F4), (4, 0x318)] {
            v -= if unique(u) { get(record, from + i * 4) } else { 0.0 };
        }
        set(record, 0x200 + i * 4, v);
        if unique(4) {
            sub(record, 0x224 + i * 4, 0x318 + i * 4);
        }
    }
}
