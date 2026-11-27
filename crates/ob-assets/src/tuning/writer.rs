use super::*;

pub fn get(r: &[u8], o: usize) -> f32 {
    read_f32(r, o)
}

pub fn put(r: &mut [u8], o: usize, v: f32) {
    r[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

/// A parameter's value before tuning (0x5A7DB0): the mean of the pair for
/// 1, 5, 19 and 20; the drop from the stock record for the ride heights 3
/// and 7 (always the first wheel pair); the torque less the turbo curve
/// for 23; else the field.
pub fn base(record: &[u8], stock: &[u8], param: u8, i: usize) -> Option<f32> {
    let f = |i| field(param, i);
    Some(match param {
        1 | 5 | 19 | 20 => (get(record, f(0)?) + get(record, f(1)?)) * 0.5,
        3 | 7 => get(stock, f(0)?) - get(record, f(0)?),
        23 => get(record, f(i)?) - get(record, field(24, i)?),
        10..=15 => get(record, f(0)?),
        _ => get(record, f(i)?),
    })
}

/// A tuning-block field (record +0x110 + `o`).
pub fn blk(r: &[u8], o: usize) -> f32 {
    get(r, 0x110 + o)
}

/// The partner a nitrous parameter trades against (0x5B3110: 16 ↔ 17).
pub fn nitrous_partner(param: u8) -> u8 {
    if param == 17 { 16 } else { 17 }
}

/// The tuning object while 0x5B4790 runs: the car's record, the stock
/// record of its type (0x8A1CCC table), its slider bytes (rewritten by
/// 0x5B3EF0 as values are clamped) and the bases (+0x40 one per
/// parameter 0–22, +0x9C / +0xC0 the nine ECU / turbo points), zero for
/// a parameter that is not tunable.
pub struct Tuner<'a> {
    pub rec: &'a mut [u8],
    pub stock: &'a [u8],
    pub table: &'a [Unlock],
    pub fitted: &'a dyn Fn(u8) -> bool,
    pub gears: u32,
    pub s: Settings,
    pub bases: [f32; 23],
    pub curve_bases: [[f32; 9]; 2],
    /// 0x5B2C10's spreading flag (the tuning screens' path).
    pub spread: bool,
}

/// 0x7A035C.
pub const EPS: f32 = 1.0e-7;

impl Tuner<'_> {
    pub fn tunable(&self, p: u8) -> bool {
        tunable(self.table, self.fitted, self.gears, p)
    }

    pub fn base(&self, p: u8, i: usize) -> f32 {
        base(self.rec, self.stock, p, i).unwrap_or(0.0)
    }

    /// The stored base (tuning object +0x40 / +0x9C / +0xC0).
    pub fn saved_base(&self, p: u8, i: usize) -> f32 {
        match p {
            0..=22 => self.bases[p as usize],
            23 | 24 => self.curve_bases[(p - 23) as usize][i.min(8)],
            _ => 0.0,
        }
    }

    /// Widen hard limits by the opening record (0x59B170): kinds 2/3 by
    /// the range times the span, 0/1 by the range, 4 to include it.
    pub fn adjust(&self, p: u8, lo: &mut f32, hi: &mut f32) {
        let Some(u) = opener(self.table, self.fitted, p) else { return };
        let (l, h) = (*lo, *hi);
        match u.kind {
            2 | 3 => {
                *lo += (h - l) * u.range.0;
                *hi += (h - l) * u.range.1;
            }
            0 | 1 => {
                *lo += u.range.0;
                *hi += u.range.1;
            }
            4 => {
                *lo = lo.min(u.range.0);
                *hi = hi.max(u.range.1);
            }
            _ => {}
        }
    }

    /// A gear ratio for a road speed in m/s at the rpm of block +0x1F4
    /// (0x59B5D0 without the flag): the driven wheels' radius (front +0x50
    /// below 0.2 drive bias +0x1C0, rear +0x80 above 0.8, else between)
    /// × rpm × 2π/60 over the final drive +0x1B8 and the speed.
    pub fn ratio_for_speed(&self, v: f32) -> f32 {
        let r = &*self.rec;
        let bias = blk(r, 0x1C0);
        let radius = if bias < 0.2 {
            blk(r, 0x50)
        } else if bias > 0.8 {
            blk(r, 0x80)
        } else {
            blk(r, 0x80) * bias + (1.0 - bias) * blk(r, 0x50)
        };
        if v == 0.0 {
            return 0.0;
        }
        radius / v * (blk(r, 0x1F4) * 0.104_719_67 / blk(r, 0x1B8))
    }

    /// The pairs of 1/5 and 19/20 (0x5A7CC0 / 0x5A7B90): per member the
    /// stock value and its upgrade target, each widened by the opener.
    pub fn pair_limits(&self, p: u8, i: usize) -> (f32, f32) {
        let (lo, hi) = match p {
            1 => (0x178, 0x5C8),
            5 => (0x198, 0x5E8),
            19 => (0xDC, 0x54C),
            _ => (0xFC, 0x56C),
        };
        let (mut l, mut h) = (blk(self.stock, lo + 4 * i), blk(self.stock, hi + 4 * i));
        self.adjust(p, &mut l, &mut h);
        (l, h)
    }

    /// A parameter's hard limits (0x5ACA60), from the stock record: the
    /// min and max over its stock and upgrade values, widened by the
    /// opener (0x59B170). `mean` gives the pair parameters' mean.
    pub fn limits(&self, p: u8, i: usize, mean: bool) -> (f32, f32) {
        let s = self.stock;
        let mut l = 100_000.0f32;
        let mut h = -100_000.0f32;
        let mut take = |v: f32| {
            l = l.min(v);
            h = h.max(v);
        };
        match p {
            0 | 4 => [0x174, 0x194, 0x5C4, 0x5E4].iter().for_each(|&o| take(blk(s, o))),
            2 | 6 => [0x180, 0x1A0, 0x5D0, 0x5F0].iter().for_each(|&o| take(blk(s, o))),
            3 | 7 => {
                take(-blk(s, 0x604));
                take(-blk(s, 0x608));
                take(0.0);
            }
            8 => [0x270, 0x5BC].iter().for_each(|&o| take(blk(s, o))),
            9 => [0x1B8, 0x3D8].iter().for_each(|&o| take(blk(s, o))),
            1 | 5 | 19 | 20 => {
                if !mean {
                    return self.pair_limits(p, i);
                }
                let (a, b) = (self.pair_limits(p, 0), self.pair_limits(p, 1));
                return ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
            }
            10..=15 => return (self.ratio_for_speed(111.756_82), self.ratio_for_speed(1.117_568_1)),
            16 => return (1.0, 11.0),
            17 => (l, h) = (0.1, 0.9),
            18 => (l, h) = (0.0, 1.0),
            21 | 22 => {
                take(blk(s, 0x264));
                take(blk(s, 0x5A0) + blk(s, 0x590) + blk(s, 0x264));
                take(blk(s, 0x268));
                take(blk(s, 0x5A4) + blk(s, 0x594) + blk(s, 0x268));
            }
            23 | 24 => {
                for k in 0..9 {
                    let o = 0x200 + 4 * k;
                    if p == 23 {
                        take(blk(s, o));
                        take(blk(s, 0x4E0 + 4 * k) + blk(s, 0x420 + 4 * k) + blk(s, o));
                    } else {
                        take(blk(s, 0x224 + 4 * k));
                        take(blk(s, 0x510 + 4 * k));
                    }
                }
                self.adjust(p, &mut l, &mut h);
                return (l.max(0.0), h.max(0.0));
            }
            _ => {}
        }
        self.adjust(p, &mut l, &mut h);
        (l, h)
    }

    /// The slider's centre (0x5A7A00): the stored base; for the pairs 0/4,
    /// 2/6 and 21/22 the larger of theirs, for 3/7 the smaller.
    pub fn centre(&self, p: u8, i: usize) -> f32 {
        let members: &[u8] = match p {
            0 | 4 => &[0, 4],
            2 | 6 => &[2, 6],
            3 | 7 => &[3, 7],
            21 | 22 => &[21, 22],
            _ => std::slice::from_ref(&p),
        };
        let low = matches!(p, 3 | 7 | 9);
        members.iter().fold(if low { 1e7 } else { -1e7 }, |c, &m| {
            let v = self.saved_base(m, i);
            if low { c.min(v) } else { c.max(v) }
        })
    }

    /// The ECU / turbo cap (0x5ACE90): no point above the curve's largest
    /// base, nor making more power at its rpm (idle +0x1F0 to max +0x1F8 in
    /// eighths) than the base curve's peak (0x5AC990, 50 rpm steps).
    pub fn curve_cap(&self, p: u8, lo: &mut f32, hi: &mut f32, i: usize) {
        let values = self.curve_bases[(p - 23) as usize];
        *hi = hi.min(values.iter().fold(0.0f32, |m, &v| m.max(v)));
        let (idle, max) = (blk(self.rec, 0x1F0), blk(self.rec, 0x1F8));
        let curve = crate::handling::Curve::<9> { x0: idle, x1: max, values };
        let mut peak = 0.0f32;
        let mut rpm = idle;
        while rpm < max {
            peak = peak.max(curve.at(rpm) * 1000.0 * rpm * 0.000_104_712_04);
            rpm += 50.0;
        }
        let at = (max - idle) * i as f32 * 0.125 + idle;
        let power = *hi * 1000.0 * at * 0.000_104_712_04;
        if peak < power {
            *hi = ((peak - power) / (at * 0.104_712_04) + *hi).max(0.0);
        }
        *lo = lo.max(0.0);
        *hi = hi.max(0.0);
    }

    /// The slider's range before 0x5B2990's last step (unconverted, not
    /// widened) by the opening record's kind; `None` when nothing opens the
    /// parameter or the kind is none that 0x5B2990 writes.
    pub fn range_raw(&self, p: u8, i: usize) -> Option<(f32, f32)> {
        let u = opener(self.table, self.fitted, p)?;
        let (mut l, mut h) = self.limits(p, i, true);
        let c = self.centre(p, i);
        if p == 23 || p == 24 {
            self.curve_cap(p, &mut l, &mut h, i);
        }
        let (r0, r1) = u.range;
        let down = matches!(p, 3 | 7 | 9);
        let (lo, hi) = match u.kind {
            2 if down => ((c - (h - l) * r1).max(l), h),
            2 => (l, (c + (h - l) * r1).min(h)),
            3 => ((c + (h - l) * r0).max(l), (c + (h - l) * r1).min(h)),
            0 if down => ((c - r1).max(l), h),
            0 => (l, (c + r1).min(h)),
            1 => ((c + r0).max(l), (c + r1).min(h)),
            4 => (r0, r1),
            _ => return None,
        };
        Some((lo, hi))
    }

    /// The slider's value range (0x5B2990 with flag 0): the raw range, its
    /// hi widened by 0.01 when the two are equal.
    pub fn range(&self, p: u8, i: usize) -> Option<(f32, f32)> {
        let (lo, hi) = self.range_raw(p, i)?;
        Some((lo, if lo == hi { hi + 0.01 } else { hi }))
    }

    /// A native end on a parameter's display scale (0x5B09B0's switch,
    /// **verified**): a gear ratio (10–15) becomes its road speed first
    /// (0x59B5D0 with the flag: the same radius × rpm × 2π/60 over the final
    /// drive and the value), then km/h (× 3.6) or mph (× 2.237); the ride
    /// heights (3, 7) cm (× 100) or inches (× 39.37); the ECU and turbo
    /// (23, 24) × 1000, then × 0.7375621 unless metric; the rest as they are.
    pub fn display_unit(&self, p: u8, v: f32, metric: bool) -> f32 {
        match p {
            3 | 7 => v * if metric { 100.0 } else { 39.37 },
            10..=15 => self.ratio_for_speed(v) * if metric { 3.6 } else { 2.237 },
            23 | 24 => {
                let v = v * 1000.0;
                if metric { v } else { v * 0.737_562_1 }
            }
            _ => v,
        }
    }

    /// The display limits of a parameter (0x5B09B0 with point 0 and flag 1,
    /// read through the wrappers 0x5B0BE0 / 0x5B0C10; **verified**): the
    /// native limits (0x5ACA60) unless the opening record gives a display
    /// value (+0x10 / +0x14, −99.99 for none); a native end goes through
    /// [`Tuner::display_unit`]; the pair is swapped into order and the hi
    /// widened by 0.01 when equal. Without an opener the native limits come
    /// back as they are.
    pub fn display_limits(&self, p: u8, metric: bool) -> (f32, f32) {
        let (mut lo, mut hi) = self.limits(p, 0, true);
        let Some(u) = opener(self.table, self.fitted, p) else { return (lo, hi) };
        lo = if u.display.0 == -99.99 { self.display_unit(p, lo, metric) } else { u.display.0 };
        hi = if u.display.1 == -99.99 { self.display_unit(p, hi, metric) } else { u.display.1 };
        if hi < lo {
            std::mem::swap(&mut lo, &mut hi);
        }
        if lo == hi {
            hi += 0.01;
        }
        (lo, hi)
    }

    /// A native value on its parameter's display scale (0x5B2810 with flag
    /// 1, **verified**): mapped linearly (0x401790: clamp, then lerp) from
    /// the native limits (0x5ACA60; the hi widened by 0.01 when they are
    /// equal and an opener exists) onto the display limits. A gear (10–15)
    /// maps its road speed (0x59B5D0) from the speeds of the two limits, in
    /// order, instead.
    pub fn to_display(&self, p: u8, x: f32, metric: bool) -> f32 {
        let (n_lo, n_hi) = self.limits(p, 0, true);
        let n_hi = if n_lo == n_hi && opener(self.table, self.fitted, p).is_some() { n_hi + 0.01 } else { n_hi };
        let (d_lo, d_hi) = self.display_limits(p, metric);
        let (x, n_lo, n_hi) = if matches!(p, 10..=15) {
            let (a, b) = (self.ratio_for_speed(n_lo), self.ratio_for_speed(n_hi));
            (self.ratio_for_speed(x), a.min(b), if b < a { a } else { b })
        } else {
            (x, n_lo, n_hi)
        };
        let x = x.max(n_lo).min(n_hi);
        (d_hi - d_lo) * ((x - n_lo) / (n_hi - n_lo)) + d_lo
    }

    /// Clamp a value into the slider's range and store the slider there
    /// (0x5B3EF0).
    pub fn clamp_store(&mut self, v: f32, p: u8, i: usize) -> f32 {
        let Some((lo, hi)) = self.range(p, i) else { return v };
        let x = v.max(lo).min(hi);
        let t = (x - self.saved_base(p, i)) / (hi - lo);
        self.s.set_t(p, i, t);
        x
    }

    /// Write a value into the record (0x5B3FB0).
    pub fn write(&mut self, v: f32, p: u8, i: usize) {
        match p {
            1 | 5 | 19 | 20 => {
                // Spread over the members' own ranges by the position in
                // the mean range (x = clamp(v, L, H), f = (x − L) / (H − L),
                // 0x5B4032–0x5B4098). The game has no guard for a zero span
                // (0x5B407C, fdiv): 0 / 0 is a NaN, so both members take NaN.
                let (l, h) = self.limits(p, 0, true);
                let x = v.max(l).min(h);
                let f = (x - l) / (h - l);
                for k in 0..2 {
                    let (lo, hi) = self.limits(p, k, false);
                    let o = field(p, k).unwrap_or(0);
                    put(self.rec, o, lo + f * (hi - lo));
                }
            }
            3 | 7 => {
                for k in 0..2 {
                    let o = field(p, k).unwrap_or(0);
                    put(self.rec, o, get(self.stock, o) - v);
                }
            }
            9 => {
                put(self.rec, 0x2C8, v);
                put(self.rec, 0x2CC, v);
            }
            10..=15 => self.write_gears(v, p),
            16 | 17 => self.write_nitrous(v, p),
            23 | 24 => self.write_curve(v, p, i, self.spread),
            _ => {
                if let Some(o) = field(p, i) {
                    put(self.rec, o, v);
                }
            }
        }
    }

    /// Move one gear (0x5B2EE0) in `ratios` (index = parameter − 8): a
    /// rise pushes the next lower gear to at least 0.05 above it, a fall
    /// the next higher one to at most 0.05 below; what a gear cannot take
    /// past its own range comes back. Returns that excess.
    pub fn shift_gear(&self, v: f32, p: u8, ratios: &mut [f32; 9]) -> f32 {
        // Without an opener 0x5B2990 writes neither limit (0x59AE50 finds
        // none): the game clamps to the parameter's own bits and an
        // prov: open tuning.shift_gear
        // uninitialised stack slot (0x5B2EE0's local, **open**). The clone
        // leaves the value unclamped.
        let (lo, hi) = self.range(p, 0).unwrap_or((f32::MIN, f32::MAX));
        let x = v.max(lo).min(hi);
        let k = p as usize - 8;
        let d = v - ratios[k];
        if d == 0.0 {
            return 0.0;
        }
        let dir: i32 = if d > 0.0 && p >= 11 {
            -1
        } else if d < 0.0 && p < 15 {
            1
        } else {
            0
        };
        let mut excess = v - x;
        if dir != 0 {
            let q = (p as i32 + dir) as u8;
            let r = ratios[q as usize - 8];
            let target = if q > p && r > x - 0.05 {
                x - 0.05
            } else if q < p && r < x + 0.05 {
                r.max(x + 0.05)
            } else {
                r
            };
            if target != r {
                excess += self.shift_gear(target, q, ratios);
            }
        }
        ratios[k] = v - excess;
        excess
    }

    /// The gear writer (0x5B3070): shift, clear gears past the car's, and
    /// write the six ratios back.
    pub fn write_gears(&mut self, v: f32, p: u8) {
        let mut ratios = [0.0f32; 9];
        for g in 10..16u8 {
            ratios[g as usize - 8] = get(self.rec, field(g, 0).unwrap_or(0));
        }
        self.shift_gear(v, p, &mut ratios);
        for r in ratios.iter_mut().skip(self.gears as usize + 2) {
            *r = 0.0;
        }
        for g in 10..16u8 {
            put(self.rec, field(g, 0).unwrap_or(0), ratios[g as usize - 8]);
        }
    }

    /// The nitrous writer (0x5B3110): the value to its field; the partner
    /// moves the other way by the same share of its own range.
    pub fn write_nitrous(&mut self, v: f32, p: u8) {
        let q = nitrous_partner(p);
        put(self.rec, field(p, 0).unwrap_or(0), v);
        let (Some((lp, hp)), Some((lq, hq))) = (self.range(p, 0), self.range(q, 0)) else { return };
        let f = (v - self.saved_base(p, 0)) / (hp - lp);
        put(self.rec, field(q, 0).unwrap_or(0), self.saved_base(q, 0) - (hq - lq) * f);
    }

    /// The curve writer (0x5B2C10): one point clamped into its range; the
    /// ECU (23) writes torque = point + turbo for all nine, the turbo (24)
    /// moves the torque with its curve. Nothing is written when the point
    /// equals its base. With `spread` (the tuning screens' path, 0x522510
    /// → 0x5B4680(…, 1); the physics set-up passes 0) the move d = v −
    /// base is shared out: neighbours i+1, i−1, i+2, i−2, … i±8 (then
    /// i+1 again), skipping those off the curve, take d × the next entry
    /// of 0x7FC674 {0.25, 0.25, −0.25 × 6, 0} — held so as not to pass the
    /// remainder r on r's side — and after those nine, −r; each clamped
    /// into its own range, r += what it moved; at most 20 moves, while
    /// |r| > 1e−7 (0x7A035C). r starts at d, so the curve keeps its total.
    pub fn write_curve(&mut self, v: f32, p: u8, i: usize, spread: bool) {
        let mut pts: [f32; 9] = std::array::from_fn(|k| self.base(p, k));
        let b = pts[i];
        let Some((lo, hi)) = self.range(p, i) else { return };
        pts[i] = v.max(lo).min(hi);
        let d = v - b;
        if d == 0.0 {
            return;
        }
        if spread && d.abs() > EPS {
            const SHARE: [f32; 9] = [0.25, 0.25, -0.25, -0.25, -0.25, -0.25, -0.25, -0.25, 0.0];
            let (mut r, mut used, mut off) = (d, 0usize, 0i32);
            while used < 20 && r.abs() > EPS {
                let mut delta = -r;
                if used < 9 {
                    delta = d * SHARE[used];
                    if r > 0.0 {
                        if delta >= r {
                            delta = r;
                        }
                    } else if delta <= r {
                        delta = r;
                    }
                }
                off = -off;
                if off >= 0 {
                    off += 1;
                    if off >= 9 {
                        off = 1;
                    }
                }
                let j = i as i32 + off;
                if !(0..9).contains(&j) {
                    continue;
                }
                let j = j as usize;
                let Some((lo, hi)) = self.range(p, j) else { break };
                let x = (delta + pts[j]).max(lo).min(hi);
                r += x - pts[j];
                pts[j] = x;
                used += 1;
            }
        }
        for (k, &pt) in pts.iter().enumerate() {
            let torque = 0x310 + 4 * k;
            let turbo = 0x334 + 4 * k;
            if p == 23 {
                put(self.rec, torque, pt + get(self.rec, turbo));
            } else {
                put(self.rec, torque, get(self.rec, torque) - get(self.rec, turbo) + pt);
                put(self.rec, turbo, pt);
            }
        }
    }

    /// Set a parameter (0x5B4680 without conversion): clamp, store the
    /// slider, write, then re-clamp the sliders the write moved (all
    /// curve points, every tunable gear, the nitrous partner).
    pub fn set(&mut self, v: f32, p: u8, i: usize) {
        let x = self.clamp_store(v, p, i);
        self.write(x, p, i);
        match p {
            23 | 24 => {
                for k in 0..9 {
                    let b = self.base(p, k);
                    self.clamp_store(b, p, k);
                }
            }
            10..=15 => {
                for g in 10..16u8 {
                    if self.tunable(g) {
                        let b = self.base(g, 0);
                        self.clamp_store(b, g, 0);
                    }
                }
            }
            16 | 17 => {
                let q = nitrous_partner(p);
                let b = self.base(q, 0);
                self.clamp_store(b, q, 0);
            }
            _ => {}
        }
    }

    pub fn torque_sum(&self) -> f32 {
        (0..9).map(|k| get(self.rec, 0x310 + 4 * k)).sum()
    }

    /// 0x5B4790.
    pub fn run(&mut self) {
        for p in 0..23u8 {
            if self.tunable(p) {
                self.bases[p as usize] = self.base(p, 0);
            }
        }
        for p in 0..23u8 {
            if !self.tunable(p) {
                continue;
            }
            let Some((lo, hi)) = self.range(p, 0) else { continue };
            let v = (hi - lo) * self.s.t(p, 0) + self.bases[p as usize];
            if (10..16).contains(&p) {
                self.write(v, p, 0);
            } else {
                self.set(v, p, 0);
            }
        }
        for p in 10..16u8 {
            if !self.tunable(p) {
                continue;
            }
            let Some((lo, hi)) = self.range(p, 0) else { continue };
            let v = (hi - lo) * self.s.t(p, 0) + self.bases[p as usize];
            self.set(v, p, 0);
        }
        let before = self.torque_sum();
        for p in [23u8, 24] {
            if !self.tunable(p) {
                continue;
            }
            for k in 0..9 {
                self.curve_bases[(p - 23) as usize][k] = self.base(p, k);
            }
            for k in 0..9 {
                let Some((lo, hi)) = self.range(p, k) else { continue };
                let v = (hi - lo) * self.s.t(p, k) + self.curve_bases[(p - 23) as usize][k];
                self.write_curve(v, p, k, false);
            }
        }
        // 0x59B3D0: a torque total that moved by more than 0.001 is
        // shared back evenly, so tuning reshapes the curve.
        let after = self.torque_sum();
        if (before - after).abs() > 0.001 {
            let d = (before - after) * 0.111_111_11;
            for k in 0..9 {
                let o = 0x310 + 4 * k;
                put(self.rec, o, get(self.rec, o) + d);
            }
        }
    }
}

/// Tune a car-type record (already upgraded by `performance::upgrade`) as
/// the physics setup does (0x5B4DD0 → 0x5B4790, run when 0x59B0F0 counts
/// a tunable parameter), every writer of 0x5B3FB0 included:
/// 1. the bases of the tunable parameters 0–22 (0x5A7DB0);
/// 2. each tunable parameter 0–22 becomes base + (hi − lo) × t, clamped
///    to lo..hi (0x5B2990 by the opening record's kind; 0x5B3EF0 stores
///    the clamped slider back), written (0x5B3FB0) — the gears raw first,
///    then again through the clamp in a second pass;
/// 3. the ECU and turbo curves point by point (0x5B2C10);
/// 4. the torque total restored if it moved (0x59B3D0).
///
/// Returns the slider bytes as the clamps stored them back (the game keeps
/// them on the car's tuning object).
pub fn apply(
    record: &mut [u8],
    stock: &[u8],
    table: &[Unlock],
    fitted: &dyn Fn(u8) -> bool,
    settings: &Settings,
) -> Settings {
    let gears = read_u32(record, 0x2D8);
    // 0x5B4790 (0x5B47BD / 0x5B4BDF): the block is tuned with the
    // performance uniques taken off (0x599DA0) and they go back on after
    // (0x5997F0), so a unique's bonus is neither clamped by the limits nor
    // counted in the bases.
    let unique = |i: u8| fitted(crate::performance::UNIQUE_BASE + i);
    crate::performance::remove_uniques(record, &unique);
    let mut t = Tuner {
        rec: &mut *record,
        stock,
        table,
        fitted,
        gears,
        s: *settings,
        bases: [0.0; 23],
        curve_bases: [[0.0; 9]; 2],
        spread: false,
    };
    t.run();
    let out = t.s;
    crate::performance::apply_uniques(record, &unique);
    out
}

/// One press on a tuning screen, `dir` ±1, as the screens move the dyno
/// car's tuning (the car tuned with `settings` first, as `apply`):
/// * a slider row (0x564F20, parameters 0–22): the row's slider (vtable
///   0x79AE5C, 0x4FF590) adds ±(hi − lo)/30 (0x522570: the 0x5B2990
///   range × 0x786460) to the current value (0x5B3E50);
/// * a curve point (0x522F20 on Up / Down; Left / Right pick the point,
///   0x522EC0, wrapping) or the nitrous slider (0x523220 on Left / Right,
///   parameter 16): ±(hi − lo) × 0.1 (0x549E60 / 0x54A1A0, the point's
///   0x5B2BB0 / 0x5B2BE0 range × 0x784358), held to that range (0x5101E0 /
///   0x510240);
///
/// then 0x5B4680(value, parameter, point, 1): clamp and store the slider,
/// write (a curve with 0x5B2C10's spreading), re-clamp the sliders the
// prov: inferred tuning.nudge
/// write moved. Returns the slider bytes. **Inferred**: the unit
/// conversion of the screens' values (0x5B2810, flag 1) cancels out.
pub fn nudge(
    record: &[u8],
    stock: &[u8],
    table: &[Unlock],
    fitted: &dyn Fn(u8) -> bool,
    settings: &Settings,
    param: u8,
    point: usize,
    dir: f32,
) -> Settings {
    let gears = read_u32(record, 0x2D8);
    let mut rec = record.to_vec();
    // The writer works on the block with the uniques off (0x5B3FDC /
    // 0x5B417C), as the tuning does (see `apply`).
    crate::performance::remove_uniques(&mut rec, &|i| fitted(crate::performance::UNIQUE_BASE + i));
    let mut t = Tuner {
        rec: &mut rec,
        stock,
        table,
        fitted,
        gears,
        s: *settings,
        bases: [0.0; 23],
        curve_bases: [[0.0; 9]; 2],
        spread: false,
    };
    t.run();
    if !t.tunable(param) {
        return t.s;
    }
    let Some((lo, hi)) = t.range(param, point) else { return t.s };
    let step = if matches!(param, 16 | 23 | 24) { (hi - lo) * 0.1 } else { (hi - lo) * (1.0 / 30.0) };
    let mut v = t.base(param, point) + dir * step;
    if matches!(param, 16 | 23 | 24) {
        v = v.max(lo).min(hi);
    }
    t.spread = true;
    t.set(v, param, point);
    t.s
}

/// The slider range of a parameter for a car (0x5B2990, unconverted):
/// what the tuning screens map −1..1 onto. `record` is the upgraded,
/// untuned record. `None` when nothing opens the parameter.
pub fn slider_range(
    record: &[u8],
    stock: &[u8],
    table: &[Unlock],
    fitted: &dyn Fn(u8) -> bool,
    param: u8,
    point: usize,
) -> Option<(f32, f32)> {
    let gears = read_u32(record, 0x2D8);
    let mut rec = record.to_vec();
    let mut t = Tuner {
        rec: &mut rec,
        stock,
        table,
        fitted,
        gears,
        s: Settings::default(),
        bases: [0.0; 23],
        curve_bases: [[0.0; 9]; 2],
        spread: false,
    };
    for p in 0..23u8 {
        if t.tunable(p) {
            t.bases[p as usize] = t.base(p, 0);
        }
    }
    for p in [23u8, 24] {
        for k in 0..9 {
            t.curve_bases[(p - 23) as usize][k] = t.base(p, k);
        }
    }
    t.range(param, point)
}
