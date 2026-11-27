use super::*;

/// A slider row's numbers, in display units (0x530DA0, **verified**): the
/// display limits the slider spans (+0x94 min 0x5B0BE0, +0x90 max 0x5B0C10,
/// 1 when 0), the tunable range (+0xB8 lo 0x5B2BB0, +0xB4 hi 0x5B2BE0, 1
/// when 0) and the value (+0x9C, 0x5B3E50, clamped into min..max). An
/// untunable parameter gives 0, 1, 0, 1, 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderView {
    pub min: f32,
    pub max: f32,
    pub lo: f32,
    pub hi: f32,
    pub value: f32,
}

impl SliderView {
    pub const UNTUNABLE: SliderView = SliderView { min: 0.0, max: 1.0, lo: 0.0, hi: 1.0, value: 0.0 };
}

/// A slider row's numbers for `param` on the car tuned with `settings`
/// (the tuning object's car, which the screens' writer tunes in place, so a
/// gear's speed follows the tuned final drive). `record` is the upgraded,
/// untuned record. The range (0x5B2990 with flag 1) is the raw range with
/// both ends on the display scale, swapped into order and widened by 0.01
/// when equal; the value is the tuned field (0x5B3E50: base + t × span, as
/// the writer stored it) on the display scale.
pub fn slider_view(
    record: &[u8],
    stock: &[u8],
    table: &[Unlock],
    fitted: &dyn Fn(u8) -> bool,
    settings: &Settings,
    param: u8,
    point: usize,
    metric: bool,
) -> SliderView {
    let gears = read_u32(record, 0x2D8);
    let mut rec = record.to_vec();
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
        return SliderView::UNTUNABLE;
    }
    let (min, max) = t.display_limits(param, metric);
    let max = if max == 0.0 { 1.0 } else { max };
    let (lo, hi) = match t.range_raw(param, point) {
        Some((a, b)) => {
            let (mut lo, mut hi) = (t.to_display(param, a, metric), t.to_display(param, b, metric));
            if hi < lo {
                std::mem::swap(&mut lo, &mut hi);
            }
            if lo == hi {
                hi += 0.01;
            }
            (lo, if hi == 0.0 { 1.0 } else { hi })
        }
        None => (0.0, 1.0),
    };
    let raw = match t.range(param, point) {
        Some((a, b)) => t.saved_base(param, point) + (b - a) * t.s.t(param, point),
        None => t.base(param, point),
    };
    let value = t.to_display(param, raw, metric).min(max).max(min);
    SliderView { min, max, lo, hi, value }
}

/// The unit a row's name carries (0x50FFB0, **verified**; the row's text
/// is "%s (%s)", 0x53CD40): the ride heights (3, 7) "cm" 0x5E1F61EE or
/// "in" 0x6382EDE0, the first gear (10) "KM/H" 0xCBBC8BF8 or "MPH"
/// 0x8569AB44, the nitrous (17) and the curves (23, 24) 0x5B81D613 /
/// 0xACB8DFF3; `metric` is the car's +4 (the tuning object's, 1 metric).
pub fn unit_label(param: u8, metric: bool) -> Option<u32> {
    let (m, i) = match param {
        3 | 7 => (0x5E1F_61EE, 0x6382_EDE0),
        10 => (0xCBBC_8BF8, 0x8569_AB44),
        17 | 23 | 24 => (0x5B81_D613, 0xACB8_DFF3),
        _ => return None,
    };
    Some(if metric { m } else { i })
}

/// One point of a curve widget, in display units: its value (the node's
/// +0x0C, 0x549E60), its low end (+0x10) and high end (+0x14).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurvePoint {
    pub value: f32,
    pub low: f32,
    pub high: f32,
}

/// A curve widget's inputs (0x550590, 0x549E60, 0x531050): the axis (the
/// widget's +0x8 / +0xC) and its nine points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveGraph {
    pub axis: (f32, f32),
    pub points: [CurvePoint; 9],
}

/// The axis of a curve widget (0x550590 at 0x550606-0x550671, verified): the
/// display minimum truncated toward zero (0x75BED4), divided by 50 with C
/// truncation (the magic-number sequence with its sign fix), times 50 and
/// clamped at 0; the display maximum (1 when it is 0) truncated the same way,
/// divided by 50, plus 1, times 50.
pub fn curve_axis(lo: f32, hi: f32) -> (f32, f32) {
    let lo = lo as i32;
    let min = (lo / 50 * 50).max(0);
    let hi = (if hi == 0.0 { 1.0 } else { hi }) as i32;
    let max = (hi / 50 + 1) * 50;
    (min as f32, max as f32)
}

/// The inputs of a curve widget for the ECU (23) or turbo (24): the axis
/// and the nine points, all in display units (0x550590's loop). `record` is
/// the upgraded, untuned record, `stock` its type's stock record, `settings`
// prov: inferred tuning.curve_graph
/// the tuning, `metric` the unit flag (the car's +4 in 0x5B09B0; **inferred**
/// to be the Units option). A parameter nothing opens gets 0x550590's
/// fallbacks: axis 0 to 50 and every point (value 0, low 0, high 1). A point
/// whose range 0x5B2990 leaves unwritten (a kind it does not handle) keeps
/// the same fallbacks; the game reads an unset slot there (**verified**: no
/// store on those paths), so that choice is the clone's.
pub fn curve_graph(
    record: &[u8],
    stock: &[u8],
    table: &[Unlock],
    fitted: &dyn Fn(u8) -> bool,
    settings: &Settings,
    param: u8,
    metric: bool,
) -> CurveGraph {
    let fallback = CurvePoint { value: 0.0, low: 0.0, high: 1.0 };
    let gears = read_u32(record, 0x2D8);
    let mut rec = record.to_vec();
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
    if !matches!(param, 23 | 24) || !t.tunable(param) {
        return CurveGraph { axis: (0.0, 50.0), points: [fallback; 9] };
    }
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
    let (d_lo, d_hi) = t.display_limits(param, metric);
    let axis = curve_axis(d_lo, d_hi);
    let mut points = [fallback; 9];
    for (k, point) in points.iter_mut().enumerate() {
        // 0x5B3E50 (flag 1): base + t × (hi − lo) of the widened raw range,
        // then the display scale.
        if let Some((lo, hi)) = t.range(param, k) {
            let base = t.curve_bases[(param - 23) as usize][k];
            point.value = t.to_display(param, (hi - lo) * settings.t(param, k) + base, metric);
        }
        // 0x5B2990 (flag 1): both ends on the display scale, swapped into
        // order, the hi widened by 0.01 when equal (0x550590 keeps a zero hi as 1).
        if let Some((lo, hi)) = t.range_raw(param, k) {
            let mut low = t.to_display(param, lo, metric);
            let mut high = t.to_display(param, hi, metric);
            if high < low {
                std::mem::swap(&mut low, &mut high);
            }
            if low == high {
                high += 0.01;
            }
            point.low = low;
            point.high = if high == 0.0 { 1.0 } else { high };
        }
    }
    CurveGraph { axis, points }
}
