//! Number, time and money text formatters (the game's printf and FormatRaceTime). Moved side by side from their old homes, unchanged;
//! the old paths re-export them.

/// `%$d`: the game's printf groups thousands with ',' when the format has
/// the '$' flag (0x43E950 with the locale bytes at 0x7EB3F0: '.', ',', 3).
pub fn grouped(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// `%.Nf` as the game's printf (0x43E950) makes it: the float goes in as a
/// double, its digits are taken one by one (truncated, `_ftol`) and the last
/// is rounded up when ten times the rest is 5 or more, i.e. ties round away
/// from zero (Wine, a tuning slider at 8.125: "8.13"; Rust's `{:.2}` gives
/// "8.12"). Returns the digits without a sign and whether it was negative.
pub fn bprintf_fixed_parts(v: f64, decimals: usize) -> (bool, String) {
    let neg = v < 0.0;
    // The exact binary value to well past the rounding digit, then cut and
    // rounded half up.
    let s = format!("{:.*}", decimals + 24, v.abs());
    let (int, frac) = s.split_once('.').unwrap_or((&s, ""));
    let mut digits: Vec<u8> = int.bytes().chain(frac.bytes().take(decimals)).collect();
    if frac.as_bytes().get(decimals).is_some_and(|&d| d >= b'5') {
        let mut i = digits.len();
        loop {
            if i == 0 {
                digits.insert(0, b'1');
                break;
            }
            i -= 1;
            if digits[i] == b'9' {
                digits[i] = b'0';
            } else {
                digits[i] += 1;
                break;
            }
        }
    }
    let split = digits.len() - decimals;
    let int = String::from_utf8_lossy(&digits[..split]).to_string();
    let frac = String::from_utf8_lossy(&digits[split..]).to_string();
    // A negative value keeps its sign even when it prints as zero (the
    // sign flag is set before the digits).
    (neg, if decimals == 0 { int } else { format!("{int}.{frac}") })
}

/// `%$.Nf`: [`bprintf_fixed`] with the integer part's thousands grouped
/// (the `$` flag, as `%$d`; Career Statistics "4,325.0").
pub fn grouped_fixed(v: f64, decimals: usize) -> String {
    let (neg, body) = bprintf_fixed_parts(v, decimals);
    let (int, frac) = body.split_once('.').map_or((body.as_str(), None), |(i, f)| (i, Some(f)));
    let int = grouped(int.parse::<i64>().unwrap_or(0));
    let s = match frac {
        Some(f) => format!("{int}.{f}"),
        None => int,
    };
    if neg { format!("-{s}") } else { s }
}

/// `%.Nf` (0x43E950's rounding, `bprintf_fixed_parts`).
pub fn bprintf_fixed(v: f64, decimals: usize) -> String {
    let (neg, body) = bprintf_fixed_parts(v, decimals);
    if neg { format!("-{body}") } else { body }
}

/// `%$2.Nf`: N decimals, the thousands grouped (the `$` flag). The same
/// native path as [`grouped_fixed`] (0x43E950; the width only pads).
pub(crate) fn fixed_grouped(v: f32, decimals: usize) -> String {
    grouped_fixed(f64::from(v), decimals)
}

/// `%$3.1f`: one decimal, thousands grouped. 0x43E950 takes every `%f`
/// through one digit loop whose last digit rounds half up
/// (0x43EF92, `10 × rest ≥ 5`), the `$` flag only grouping the integer
/// digits; the width 3 only pads. So this is [`grouped_fixed`] with one
/// decimal (Rust's `{:.1}` rounds the exact binary value half to even:
/// 0.25 → "0.2" where the game prints "0.3").
pub(crate) fn grouped_1(v: f32) -> String {
    grouped_fixed(f64::from(v), 1)
}

/// A time with a sign (0x577250 with flag 2 → 0x576D40): '-' when
/// negative, else '+'. The sign takes the leading field's blank tens
/// digit, or when the field has two digits goes in front and the last
/// character drops.
pub fn signed_time(ticks: i32) -> String {
    let text = super::hud::time_text(ticks.abs());
    let sign = if ticks < 0 { '-' } else { '+' };
    let whole = ticks.abs() / 4000;
    let lead = if whole >= 3600 {
        whole / 3600
    } else if whole >= 60 {
        whole % 3600 / 60
    } else {
        whole
    };
    // Under a second (template "0.00", no blank) the sign goes in front
    // and, without flag 8, the last digit drops.
    if lead > 9 || whole == 0 {
        let mut t: String = text.chars().collect();
        t.pop();
        format!("{sign}{t}")
    } else {
        format!("{sign}{}", text.chars().skip(1).collect::<String>())
    }
}

/// `%$2.2f`: two decimals, the whole part grouped by thousands.
/// "%$f": the formatter's %f with no precision takes 3 decimals
/// (0x43EDFD), the whole part grouped with ','.
/// Str_VFormat's "%$f" with 3 decimals (0x43E950): the digits by ×10 and
/// truncation, then the last one rounded up when the rest × 10 ≥ 5
/// (0x43EF92–0x43EFD8, 5.0 at 0x786E48): half up, not half to even.
pub fn fixed3(v: f64) -> String {
    let scaled = v.abs() * 1000.0;
    let mut k = scaled.trunc();
    if (scaled - k) * 10.0 >= 5.0 {
        k += 1.0;
    }
    let s = format!("{}{}.{:03}", if v < 0.0 { "-" } else { "" }, (k / 1000.0).trunc() as i64, (k % 1000.0) as i64);
    let (whole, fraction) = s.split_once('.').unwrap_or((&s, "000"));
    let digits = whole.trim_start_matches('-');
    let sign = if whole.starts_with('-') { "-" } else { "" };
    format!("{sign}{}.{fraction}", super::grouped(digits.parse().unwrap_or(0)))
}

pub fn fixed2(v: f32) -> String {
    let s = super::bprintf_fixed(f64::from(v), 2);
    let (whole, fraction) = s.split_once('.').unwrap_or((&s, "00"));
    let digits = whole.trim_start_matches('-');
    let sign = if whole.starts_with('-') { "-" } else { "" };
    format!("{sign}{}.{fraction}", super::grouped(digits.parse().unwrap_or(0)))
}

/// The game's clock ticks 4000 times a second (0x865188).
pub(crate) fn ticks(seconds: f32) -> i32 {
    (seconds * 4000.0) as i32
}

/// A time as the HUD prints it (0x577250 → 0x576D40, English): "SS.CC"
/// under a minute, "MM:SS.CC" under an hour, else "HHh:MM:SS". A
/// leading zero digit of the first field is a space.
///
/// This is THE FormatRaceTime with flags 0: the World Map record
/// (0x496D70), the Quick Race track record, the results' finish times
/// and the HUD all call the one 0x577250 with flags 0, so the former
/// `race_time` (seconds, rounded) and `format_race_time` (seconds × 100,
/// truncated in f32) were merged into it. Ticks → whole seconds
/// `t / 4000`, hundredths `(t % 4000) × 1000 / 4000 / 10`, all integer
/// and truncating (0x5771C0, 0x577250's `/ 10`).
pub fn time_text(ticks: i32) -> String {
    let t = ticks.max(0);
    let whole = t / 4000;
    let (h, m, s) = (whole / 3600, whole % 3600 / 60, whole % 60);
    let c = (t % 4000) * 1000 / 4000 / 10;
    let lead = |v: i32| -> String {
        let tens = if v / 10 % 10 == 0 && v / 10 < 10 { ' ' } else { char::from(b'0' + (v / 10 % 10) as u8) };
        let ones = if v % 10 == 0 && v < 10 { ' ' } else { char::from(b'0' + (v % 10) as u8) };
        format!("{tens}{ones}")
    };
    if h > 0 {
        format!("{}h:{m:02}:{s:02}", lead(h))
    } else if m > 0 {
        format!("{}:{s:02}.{c:02}", lead(m))
    } else if s > 0 {
        format!("{}.{c:02}", lead(s))
    } else {
        // Under a second 0x576D40 starts from its own template "0.00"
        // (0x7A0630; "0.--" 0x7A0628 without flag 4) and writes the
        // hundredths: no blank in front (Wine: "0.64" on the race clock,
        // "0.00" for a blown engine's finish time).
        format!("0.{c:02}")
    }
}

/// A signed time with 0x577250's flags 10 (0x576D40): '-' when
/// negative, else '+', over the first field's leading space; a first
/// field of two digits moves right to make room (flag 8 keeps the last
/// digit).
pub fn signed_time_text(ticks: i32) -> String {
    let t = time_text(ticks.saturating_abs());
    let whole = ticks.saturating_abs() / 4000;
    let first = if whole >= 3600 {
        whole / 3600
    } else if whole >= 60 {
        whole % 3600 / 60
    } else {
        whole % 60
    };
    let sign = if ticks < 0 { '-' } else { '+' };
    // Under a second the template has no blank: the sign goes in front
    // (flag 8 keeps the last digit).
    if first > 9 || whole == 0 { format!("{sign}{t}") } else { format!("{sign}{}", &t[1..]) }
}

/// "%$d": thousands grouped with ','.
pub fn grouped_int(v: i32) -> String {
    let digits = v.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if v < 0 { format!("-{out}") } else { out }
}
