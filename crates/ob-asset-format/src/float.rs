//! Floats in text files (conventions.md §3.4): written with the shortest digits that round-trip the value at its
//! own width, in a form TOML always reads as a float; read so that a written value comes back with the exact
//! bits; the `bits:` escape for NaNs and for the values a decimal cannot carry through binary64.

use crate::{Refusal, Rule};

/// What a float field allows besides finite values (its field table says so).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Allow {
    pub infinite: bool,
    pub nan: bool,
}

/// The shortest decimal digits of a value and their exponent: value = 0.d₁d₂… × 10^(exp+1), that is
/// d₁.d₂… × 10^exp. `digits` has no leading or trailing zero (except `"0"` for zero).
fn shortest(sci: &str) -> (bool, String, i32) {
    // Rust's `{:e}` prints the shortest digits that round-trip at the value's own width: `-1.5e-7`, `3e38`.
    let (neg, rest) = match sci.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, sci),
    };
    let (mant, exp) = rest.split_once('e').expect("`{:e}` has an exponent");
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0".to_string() } else { digits.to_string() };
    (neg, digits, exp.parse().expect("`{:e}` exponent"))
}

/// The decimal text form of the spec: plain notation with at least one digit after the point when the shortest
/// decimal is zero or its exponent lies in −5..=15 (1e-5 ≤ |x| < 1e16); otherwise `d.ddde±x`.
fn decimal_form(neg: bool, digits: &str, exp: i32) -> String {
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    if digits == "0" {
        out.push_str("0.0");
    } else if (-5..16).contains(&exp) {
        if exp >= 0 {
            let int_len = exp as usize + 1;
            if digits.len() <= int_len {
                out.push_str(digits);
                out.extend(std::iter::repeat_n('0', int_len - digits.len()));
                out.push_str(".0");
            } else {
                out.push_str(&digits[..int_len]);
                out.push('.');
                out.push_str(&digits[int_len..]);
            }
        } else {
            out.push_str("0.");
            out.extend(std::iter::repeat_n('0', (-exp - 1) as usize));
            out.push_str(digits);
        }
    } else {
        out.push_str(&digits[..1]);
        out.push('.');
        out.push_str(if digits.len() > 1 { &digits[1..] } else { "0" });
        out.push('e');
        out.push(if exp < 0 { '-' } else { '+' });
        out.push_str(&exp.unsigned_abs().to_string());
    }
    out
}

/// The `bits:` escape of a binary32 value.
pub fn bits32(x: f32) -> String {
    format!("\"bits:0x{:08X}\"", x.to_bits())
}

/// The `bits:` escape of a binary64 value.
pub fn bits64(x: f64) -> String {
    format!("\"bits:0x{:016X}\"", x.to_bits())
}

/// A binary32 value as the TOML text a writer puts after `=`: the shortest decimal, or the quoted `bits:`
/// escape for NaNs, infinities and the values whose decimal would read back differently through binary64
/// (±7.038531e-26 among all 2³² values).
pub fn write_f32(x: f32) -> String {
    if !x.is_finite() {
        return bits32(x);
    }
    let (neg, digits, exp) = shortest(&format!("{x:e}"));
    let text = decimal_form(neg, &digits, exp);
    match text.parse::<f64>() {
        Ok(v) if (v as f32).to_bits() == x.to_bits() => text,
        _ => bits32(x),
    }
}

/// A binary64 value as TOML text: the shortest decimal, or the `bits:` escape for NaNs and infinities.
pub fn write_f64(x: f64) -> String {
    if !x.is_finite() {
        return bits64(x);
    }
    let (neg, digits, exp) = shortest(&format!("{x:e}"));
    decimal_form(neg, &digits, exp)
}

fn parse_bits(s: &str, hex_len: usize) -> Option<u64> {
    let hex = s.strip_prefix("bits:0x")?;
    if hex.len() != hex_len || !hex.bytes().all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b)) {
        return None;
    }
    u64::from_str_radix(hex, 16).ok()
}

fn check(x: f64, allow: Allow) -> Result<(), Refusal> {
    if x.is_nan() && !allow.nan {
        return Err(Refusal::new(Rule::Conv7, "NaN is not allowed here"));
    }
    if x.is_infinite() && !allow.infinite {
        return Err(Refusal::new(Rule::Conv7, "an infinite value is not allowed here"));
    }
    Ok(())
}

/// An `f32` field's value: a TOML float or integer means round₃₂(round₆₄(x)) (TOML's own binary64 reading, then
/// to binary32); a string must be the `bits:0xXXXXXXXX` escape. `-0.0` keeps its sign.
pub fn read_f32(v: &toml::Value, allow: Allow) -> Result<f32, Refusal> {
    let x = match v {
        toml::Value::Float(f) => {
            let x = *f as f32;
            if f.is_finite() && x.is_infinite() {
                return Err(Refusal::new(Rule::Conv6, format!("{f} is outside the range of a 32-bit float")));
            }
            x
        }
        toml::Value::Integer(i) => *i as f64 as f32,
        toml::Value::String(s) => match parse_bits(s, 8) {
            Some(b) => f32::from_bits(b as u32),
            None => return Err(Refusal::new(Rule::Conv7, format!("{s:?} is not a `bits:0xXXXXXXXX` escape"))),
        },
        other => return Err(Refusal::new(Rule::Conv6, format!("a float was expected, found {}", other.type_str()))),
    };
    check(f64::from(x), allow)?;
    Ok(x)
}

/// An `f64` field's value: a TOML float or integer, or the `bits:0xXXXXXXXXXXXXXXXX` escape.
pub fn read_f64(v: &toml::Value, allow: Allow) -> Result<f64, Refusal> {
    let x = match v {
        toml::Value::Float(f) => *f,
        toml::Value::Integer(i) => *i as f64,
        toml::Value::String(s) => match parse_bits(s, 16) {
            Some(b) => f64::from_bits(b),
            None => {
                return Err(Refusal::new(Rule::Conv7, format!("{s:?} is not a `bits:0xXXXXXXXXXXXXXXXX` escape")));
            }
        },
        other => return Err(Refusal::new(Rule::Conv6, format!("a float was expected, found {}", other.type_str()))),
    };
    check(x, allow)?;
    Ok(x)
}

/// Floats compared the format's way (§3.4): by their bits, so `-0.0` and `0.0` differ and two NaNs are equal
/// only with equal bits.
pub fn same_f64(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read32(text: &str) -> f32 {
        let t: toml::Table = toml::from_str(&format!("x = {text}")).unwrap();
        read_f32(&t["x"], Allow { infinite: true, nan: true }).unwrap()
    }

    #[test]
    fn spec_examples() {
        // conventions.md §3.4's examples of the text form.
        assert_eq!(write_f32(1.0), "1.0");
        assert_eq!(write_f32(-0.0), "-0.0");
        assert_eq!(write_f32(0.25), "0.25");
        assert_eq!(write_f32(146.0), "146.0");
        assert_eq!(write_f32(f32::MAX), "3.4028235e+38");
        assert_eq!(write_f32(f32::from_bits(5)), "7.0e-45");
        assert_eq!(write_f32(1.2345678e15), "1234567800000000.0");
        assert_eq!(write_f64(7.006492e-46), "7.006492e-46");
        assert_eq!(write_f64(1e16), "1.0e+16");
        assert_eq!(write_f64(123456789012345.6), "123456789012345.6");
        assert_eq!(write_f32(1e-5), "0.00001");
        assert_eq!(write_f32(9.9e-6), "9.9e-6");
        assert_eq!(write_f32(83.41), "83.41");
    }

    #[test]
    fn double_rounding_values_take_the_escape() {
        // The two binary32 values a decimal cannot carry through binary64 (§3.4, "Readers").
        for bits in [0x15AE_43FDu32, 0x95AE_43FD] {
            let x = f32::from_bits(bits);
            let text = write_f32(x);
            assert!(text.starts_with("\"bits:0x"), "{text}");
            assert_eq!(read32(&text).to_bits(), bits);
            // The shortest decimal itself reads back one step off through binary64.
            let decimal = format!("{x:e}").parse::<f64>().unwrap() as f32;
            assert_eq!(decimal.to_bits(), bits + 1);
        }
    }

    #[test]
    fn escapes_and_refusals() {
        assert!(read32("\"bits:0x7FC00001\"").is_nan());
        assert_eq!(read32("\"bits:0x7FC00001\"").to_bits(), 0x7FC0_0001);
        assert_eq!(write_f32(f32::from_bits(0x7FC0_0001)), "\"bits:0x7FC00001\"");
        assert_eq!(read32("-0.0").to_bits(), 0x8000_0000);
        assert_eq!(read32("3").to_bits(), 3.0f32.to_bits());
        let t: toml::Table =
            toml::from_str("a = nan\nb = \"bits:0x7fc00000\"\nc = 1e39\nd = \"bits:0x3F800000\"").unwrap();
        assert_eq!(read_f32(&t["a"], Allow::default()).unwrap_err().rule, Rule::Conv7);
        assert_eq!(read_f32(&t["b"], Allow::default()).unwrap_err().rule, Rule::Conv7);
        assert_eq!(read_f32(&t["c"], Allow::default()).unwrap_err().rule, Rule::Conv6);
        assert_eq!(read_f64(&t["d"], Allow::default()).unwrap_err().rule, Rule::Conv7);
        assert!(!same_f64(0.0, -0.0));
        assert!(same_f64(f64::NAN, f64::NAN));
    }

    /// Every TOML text the writer makes is a TOML float and reads back to the same bits.
    fn round_trip(bits: u32) {
        let x = f32::from_bits(bits);
        let text = write_f32(x);
        if !text.starts_with('"') {
            assert!(text.contains('.'), "{text} would read as an integer");
        }
        assert_eq!(read32(&text).to_bits(), bits, "{text}");
    }

    #[test]
    fn round_trip_sampled() {
        let mut bits = 0u32;
        loop {
            round_trip(bits);
            round_trip(bits ^ 0x8000_0000);
            match bits.checked_add(0x0001_0001) {
                Some(b) => bits = b,
                None => break,
            }
        }
        for bits in [0, 1, 0x007F_FFFF, 0x0080_0000, 0x7F7F_FFFF, 0x7F80_0000, 0x3F80_0000, 0x3727_C5AC] {
            round_trip(bits);
        }
    }

    /// Every one of the 2³² binary32 values (about two minutes on 16 threads): `cargo test --release -p
    /// ob-asset-format -- --ignored every_f32`.
    #[test]
    #[ignore = "exhaustive: all 2^32 values"]
    fn every_f32() {
        use std::sync::atomic::{AtomicU64, Ordering};
        let escapes = AtomicU64::new(0);
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get()) as u64;
        std::thread::scope(|s| {
            for t in 0..threads {
                let escapes = &escapes;
                s.spawn(move || {
                    let mut b = t;
                    while b <= u64::from(u32::MAX) {
                        let x = f32::from_bits(b as u32);
                        let text = write_f32(x);
                        if x.is_finite() {
                            if text.starts_with('"') {
                                escapes.fetch_add(1, Ordering::Relaxed);
                                assert!(matches!(b as u32, 0x15AE_43FD | 0x95AE_43FD), "escape for {b:#X}");
                            } else {
                                assert!(text.contains('.'));
                                let back = text.parse::<f64>().unwrap() as f32;
                                assert_eq!(back.to_bits(), b as u32, "{text}");
                            }
                        }
                        b += threads;
                    }
                });
            }
        });
        assert_eq!(escapes.load(Ordering::Relaxed), 2);
    }
}
