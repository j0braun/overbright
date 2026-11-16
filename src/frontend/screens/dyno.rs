use crate::frontend::fe_ids;
use super::*;

/// UI_PerformanceDyno_Chart (Dyno Run 0x53C840; screen 0x55AF30 → 0x557A20,
/// messages 0x54A810): the torque and power curves of the car's engine,
/// drawn by the graph 0x5202C0 (at x 79, y 73 — 0x7FA3C0 holds 79.0 then 73.0 —, 486 × 312) that 0x53D0B0 fills.
/// Each curve is 32 points from idle to max rpm ((max − idle) / 31 apart),
/// torque `curve × 1000` N·m (× 0.7375621 ft-lbs), power `torque × rpm ×
/// 0.00010471204` kW (× 1.3410221 bhp); axes 0..10000 rpm and 0..600
/// imperial (813.49 Nm, 447.42 kW). Torque 0xFFAA8000, power 0xFF1A95AF;
/// after the first run the first run's curves (record 0x83A734) come too,
/// alpha 0x82. The graph's draw (0x50DE90) reveals the curves up to the
/// sweep rpm `first + (last − first) × progress` with a yellow line there
/// while progress < 1. Labels: 0x22B57EB7 + "1".."6" torque, 0xEEC8134F +
/// "1".."6" power, `%$d` of 0, ⅕ max … max; titles "Power ( %s )" /
/// "Torque ( %s )". When the pull ends (0x54A630): MAX_TORQUE_DATA /
/// MAX_HP_DATA "%$3.1f %s^@%$d rpm" of the peaks (0x5B5820 / 0x5B56F0:
/// idle..max in 50 rpm steps), group 0x11BF9934 "FULL", Chart_Legend "Show".
pub struct DynoChart {
    pub engine: crate::handling::Engine,
    /// The first dyno run's engine (0x83A734), drawn after it.
    pub first: Option<crate::handling::Engine>,
    pub metric: bool,
    pub opened: f32,
    /// The pull has been recorded (0x523340 runs once, at its end).
    pub recorded: bool,
    /// The dyno manager's pull ([`crate::dyno::DynoPull`]); none shows the
    /// pull finished.
    pub pull: Option<crate::dyno::DynoPull>,
    /// When the progress reached 1, ms after opening.
    pub finished: Option<f32>,
}

impl DynoChart {
    pub const GROUP: u32 = fe_ids::GROUP;
    const AT: Vec2 = Vec2::new(79.0, 73.0);
    const SIZE: Vec2 = Vec2::new(486.0, 312.0);
    const MAX_RPM: f32 = 10000.0;
    /// The pull's progress (0x56D450 on the dyno manager; 1 without one).
    pub fn progress(&self) -> f32 {
        if self.finished.is_some() {
            return 1.0;
        }
        self.pull.as_ref().map_or(1.0, |p| p.progress())
    }

    /// Run the pull `dt` seconds on; note when it reaches the top.
    pub fn step(&mut self, dt: f32, now: f32) {
        if let Some(p) = self.pull.as_mut() {
            p.step(dt);
        }
        if self.finished.is_none() && self.progress() >= 1.0 {
            self.finished = Some(now - self.opened);
        }
    }

    /// Torque (N·m or ft-lbs) and power (kW or bhp) at `rpm`.
    fn values(e: &crate::handling::Engine, rpm: f32, metric: bool) -> (f32, f32) {
        let t = e.torque.at(rpm) * 1000.0;
        let p = t * rpm * 0.000_104_712_04;
        if metric { (t, p) } else { (t * 0.737_562_1, p * 1.341_022_1) }
    }

    /// The axes' tops: torque, power (0x53D0B0).
    fn tops(metric: bool) -> (f32, f32) {
        if metric { (f32::from_bits(0x444B_5F69), f32::from_bits(0x43DF_B5C0)) } else { (600.0, 600.0) }
    }

    /// Peak (value, rpm) over idle..max in 50 rpm steps (0x5B5820 torque,
    /// 0x5B56F0 power; a later rpm wins only when strictly higher).
    pub fn peaks(e: &crate::handling::Engine, metric: bool) -> [(f32, f32); 2] {
        let mut best = [(0.0, 0.0); 2];
        let mut rpm = e.idle_rpm;
        while rpm < e.max_rpm {
            let (t, p) = Self::values(e, rpm, true);
            for (b, v) in best.iter_mut().zip([t, p]) {
                if v > b.0 {
                    *b = (v, rpm);
                }
            }
            rpm += 50.0;
        }
        if !metric {
            best[0].0 *= 0.737_562_1;
            best[1].0 *= 1.341_022_1;
        }
        best
    }

    pub fn quads(&self, assets: &Assets, now: f32) -> Vec<Quad> {
        let Some(package) = assets.package("UI_PerformanceDyno_Chart") else { return Vec::new() };
        let progress = self.progress();
        let mut state = ScreenState::default();
        footer(&mut state, package, "Results");
        // Chart_Legend's INIT chains to HIDE; only the pull's end shows it
        // (**verified**: no legend during the pull under Wine, menus114
        // ch1–ch20).
        state.chained.insert(fe_hash("Chart_Legend"));
        state.run(Self::GROUP, if progress < 1.0 { "FadeIn" } else { "FULL" }, 0.0);
        let (torque_top, power_top) = Self::tops(self.metric);
        let units = if self.metric { ("Nm", "kW") } else { ("ft-lbs", "bhp") };
        state.text.insert(0x2345_F69E, format!("Torque ( {} )", units.0));
        state.text.insert(0xE2A8_254C, format!("Power ( {} )", units.1));
        // Each label 1..6: (i − 1) × ⅕ of the top, truncated (0x75BED4).
        let (power_step, torque_step) = ((power_top * 0.2) as i64, (torque_top * 0.2) as i64);
        for i in 1..=6i64 {
            let n = i.to_string();
            state.text.insert(
                crate::formats::hash::bin_hash_from(0xEEC8_134F, &n),
                crate::frontend::grouped((i - 1) * power_step),
            );
            state.text.insert(
                crate::formats::hash::bin_hash_from(0x22B5_7EB7, &n),
                crate::frontend::grouped((i - 1) * torque_step),
            );
        }
        if progress >= 1.0 {
            let peaks = Self::peaks(&self.engine, self.metric);
            for ((name, unit), (v, rpm)) in
                [("MAX_TORQUE_DATA", units.0), ("MAX_HP_DATA", units.1)].into_iter().zip(peaks)
            {
                // A peak not above 0 reads "n/a" (0xA4C02484).
                let text = if v > 0.0 {
                    format!("{} {unit}^@{} rpm", grouped_1(v), crate::frontend::grouped((rpm + 0.5) as i64))
                } else {
                    "n/a".into()
                };
                state.text.insert(fe_hash(name), text);
            }
            state.run(fe_hash("Chart_Legend"), "Show", self.finished.unwrap_or(0.0));
        }
        let mut quads = layout(assets, package, &state, now - self.opened);
        // The lines: this car's torque and power, then the first run's.
        let mut lines = vec![(&self.engine, 0xFFu32)];
        lines.extend(self.first.as_ref().map(|e| (e, 0x82)));
        let curves: Vec<(Vec<Vec2>, Vec<Vec2>, u32)> = lines
            .into_iter()
            .map(|(e, alpha)| {
                let step = (e.max_rpm - e.idle_rpm) / 31.0;
                let (mut t, mut p, mut rpm) = (Vec::new(), Vec::new(), e.idle_rpm);
                while rpm <= e.max_rpm {
                    let (tv, pv) = Self::values(e, rpm, self.metric);
                    t.push(Vec2::new(rpm, tv));
                    p.push(Vec2::new(rpm, pv));
                    rpm += step;
                }
                (t, p, alpha)
            })
            .collect();
        // The sweep follows the line reaching the highest rpm.
        let widest = curves
            .iter()
            .map(|c| &c.0)
            .filter(|c| !c.is_empty())
            .max_by(|a, b| a[a.len() - 1].x.total_cmp(&b[b.len() - 1].x));
        let Some(widest) = widest else { return quads };
        let (first, last) = (widest[0].x, widest[widest.len() - 1].x);
        let sweep = (first + (last - first) * progress).clamp(0.0, Self::MAX_RPM);
        let to_screen = |v: Vec2, top: f32| {
            let f = Vec2::new(v.x.clamp(0.0, Self::MAX_RPM) / Self::MAX_RPM, v.y.clamp(0.0, top) / top);
            Vec2::new(Self::AT.x + f.x * Self::SIZE.x, Self::AT.y + (1.0 - f.y) * Self::SIZE.y)
        };
        for (t, p, alpha) in &curves {
            for (points, top, rgb) in [(t, torque_top, 0xAA_8000u32), (p, power_top, 0x1A_95AF)] {
                let color = crate::frontend::argb(alpha << 24 | rgb);
                for w in points.windows(2) {
                    if sweep < w[0].x {
                        break;
                    }
                    let b = if sweep < w[1].x { w[0].lerp(w[1], (sweep - w[0].x) / (w[1].x - w[0].x)) } else { w[1] };
                    quads.push(hairline_quad(to_screen(w[0], top), to_screen(b, top), color));
                }
            }
        }
        if progress < 1.0 {
            let x = to_screen(Vec2::new(sweep, 0.0), 1.0).x;
            quads.push(hairline_quad(
                Vec2::new(x, Self::AT.y + Self::SIZE.y),
                Vec2::new(x, Self::AT.y),
                crate::frontend::argb(0xFFFF_FF00),
            ));
        }
        quads
    }
}

/// A dyno run's record (0x523340 at the pull's end, 0x6C bytes): +0x00
/// the computed top speed (0x5B5940, m/s), +0x08 / +0x0C the 0-60 and
/// 0-100 times (ticks of 1/4000 s, from the dyno car's +0x24 / +0x28 ×
/// 4000 + 0.5), +0x34 the 9 torque values, +0x58 / +0x5C the peak torque
/// and power (0x5B5820 / 0x5B56F0) and +0x60 / +0x64 their rpm, +0x68
/// the max rpm. The dyno car's +0x24 / +0x28 are its set-up object's
/// +0x14 / +0x18 (the set-up sits at +0x10), the launch times 0x5B06B0
/// works out (`handling::launch_times`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DynoRun {
    pub top_speed: f32,
    /// 0-60 and 0-100 mph, ticks of 1/4000 s.
    pub zero_60: i32,
    pub zero_100: i32,
    /// Peak torque (N·m) and its rpm.
    pub torque: (f32, f32),
    /// Peak power (kW) and its rpm.
    pub power: (f32, f32),
}

impl DynoRun {
    pub fn of(h: &crate::handling::Handling) -> DynoRun {
        let [torque, power] = DynoChart::peaks(&h.engine, true);
        let times = crate::handling::launch_times(h, h.type_index);
        // × 4000 + 0.5, truncated (0x75BED4).
        let ticks = |t: f32| (t * 4000.0 + 0.5) as i32;
        DynoRun {
            top_speed: h.computed_top_speed,
            zero_60: ticks(times.zero_60),
            zero_100: ticks(times.zero_100),
            torque,
            power,
        }
    }
}

/// The dyno's run records (0x523340): the first run (0x83A734), the one
/// before the current (0x83A6C8) and the current (0x83A65C). The first
/// pull fills the first and the current; a later one moves the current to
/// the previous (unless its peak power is 0) and takes its place.
#[derive(Clone, Copy, Debug, Default)]
pub struct DynoRuns {
    pub current: Option<DynoRun>,
    pub previous: Option<DynoRun>,
    pub first: Option<DynoRun>,
}

impl DynoRuns {
    pub fn record(&mut self, run: DynoRun) {
        if self.first.is_none() {
            self.first = Some(run);
        } else if self.current.is_some_and(|c| c.power.0 != 0.0) {
            self.previous = self.current;
        }
        self.current = Some(run);
    }
}

/// UI_PerformanceDyno_Results (screen 0x55AF90 → 0x557BE0, filled by
/// 0x54A980): for CURRENT, PREV and FR (first run) the fields
/// `<row>_DATA_1`..`_5`: the 0-60 and 0-100 times ("%s %s", the time and
/// "Sec." 0x29F4BF03), the top speed ("%$3.1f %s": × 3.6 "KM/H"
/// 0xCBBC8BF8 or × 2.237 "MPH" 0x8569AB44), the peak torque
/// (MAX_TORQUE_DATA 0x76F2BC60: × 1000, × 0.7375621 "ft-lbs" 0xACB8DFF3
/// or "Nm" 0x5B81D613, at round(rpm + 0.5)) and power (MAX_HP_DATA
/// 0x43AEEF78: × 1.3410221 "bhp" or "kW"). A missing value (a time of 0
/// or 0x7FFFFFFF, a figure not above 0) reads "n/a" (0xA4C02484).
/// The three rating bars (**verified**, Wine menus114 r1): the
/// constructor's 0x4A22A0 binds accel_ / top_speed_ / handling_ backing,
/// handle and cur as the car lot's (range 0..1, end pads 46), and
/// 0x54A980 ends with 0x5103D0, which sets them to the shops' ratings
/// (0x5B53B0, 0x5B6A30, 0x5B54A0). The car's record is the shops' one for
// prov: inferred frontend.screens.DynoResults
/// the fitted parts (**inferred**: the values match the stock 206 under
/// Wine).
pub struct DynoResults {
    pub runs: DynoRuns,
    pub metric: bool,
    pub opened: f32,
    /// Acceleration, top speed and handling, 0..1.
    pub ratings: Option<[f32; 3]>,
}

impl DynoResults {
    /// The five texts of a run's column.
    pub fn column(run: Option<&DynoRun>, metric: bool) -> [String; 5] {
        let na = || "n/a".to_string();
        let Some(r) = run else { return [na(), na(), na(), na(), na()] };
        let speed = if r.top_speed > 0.0 {
            format!(
                "{} {}",
                grouped_1(r.top_speed * if metric { 3.6 } else { 2.237 }),
                if metric { "KM/H" } else { "MPH" }
            )
        } else {
            na()
        };
        let peak = |(v, rpm): (f32, f32), unit: &str| {
            if v > 0.0 {
                format!("{} {unit}^@{} rpm", grouped_1(v), crate::frontend::grouped((rpm + 0.5) as i64))
            } else {
                na()
            }
        };
        let (t, p) = if metric { (r.torque.0, r.power.0) } else { (r.torque.0 * 0.737_562_1, r.power.0 * 1.341_022_1) };
        let units = if metric { ("Nm", "kW") } else { ("ft-lbs", "bhp") };
        // "%s %s": the time (0x577250 flags 0 → 0x576D40) and "Sec.".
        let time = |ticks: i32| {
            if ticks == 0 || ticks == 0x7FFF_FFFF {
                na()
            } else {
                format!("{} Sec.", crate::frontend::hud::time_text(ticks))
            }
        };
        [time(r.zero_60), time(r.zero_100), speed, peak((t, r.torque.1), units.0), peak((p, r.power.1), units.1)]
    }

    pub fn quads(&self, assets: &Assets, now: f32) -> Vec<Quad> {
        let Some(package) = assets.package("UI_PerformanceDyno_Results") else { return Vec::new() };
        let mut state = ScreenState::default();
        footer(&mut state, package, "Back");
        for (row, run) in [("CURRENT", self.runs.current), ("PREV", self.runs.previous), ("FR", self.runs.first)] {
            for (i, text) in Self::column(run.as_ref(), self.metric).into_iter().enumerate() {
                state.text.insert(fe_hash(&format!("{row}_DATA_{}", i + 1)), text);
            }
        }
        if let Some(ratings) = self.ratings {
            for (prefix, value) in ["accel", "top_speed", "handling"].into_iter().zip(ratings) {
                lot_bar(&mut state, package, prefix, value);
            }
        }
        layout(assets, package, &state, now - self.opened)
    }
}
