//! Times shown to the player: in their local time zone, the date in the
//! system's own short form (Linux: the C library's `%x` of the LC_TIME
//! locale; Windows: GetDateFormatW / GetTimeFormatW with the account's
//! settings). Files and metadata keep UTC (`qol::saves::timestamp`).
//!
//! The renders and the tests use a fixed form in UTC ([`fixed`]), so they do
//! not depend on the machine's time zone or language.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

static FIXED: AtomicBool = AtomicBool::new(false);

/// From now on every time is shown in the fixed form, in UTC (the fake
/// data: renders and tests).
pub fn fixed() {
    FIXED.store(true, Ordering::Relaxed);
}

/// `YYYYMMDD-HHMMSS` (UTC, a backup's stamp) as a time.
pub fn from_stamp(s: &str) -> Option<SystemTime> {
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, se) = (n(0..4)?, n(4..6)?, n(6..8)?, n(9..11)?, n(11..13)?, n(13..15)?);
    // Days from the civil date (Howard Hinnant's algorithm).
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let secs = days * 86_400 + h * 3600 + mi * 60 + se;
    u64::try_from(secs).ok().map(|s| SystemTime::UNIX_EPOCH + Duration::from_secs(s))
}

/// (year, month, day, hour, minute) in UTC.
fn civil(t: SystemTime) -> (i64, usize, i64, u64, u64) {
    let secs = t.duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m as usize, d, secs % 86_400 / 3600, secs % 3600 / 60)
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// The fixed form: "17 Nov 2026" (and "10:15"), UTC.
pub fn fixed_date(t: SystemTime) -> String {
    let (y, m, d, _, _) = civil(t);
    format!("{d} {} {y}", MONTHS[m - 1])
}

/// A date the player reads: local, the system's short form.
pub fn date(t: SystemTime) -> String {
    if FIXED.load(Ordering::Relaxed) {
        return fixed_date(t);
    }
    sys::format(t, false).unwrap_or_else(|| fixed_date(t))
}

/// A date and time the player reads: local, the system's short forms.
pub fn date_time(t: SystemTime) -> String {
    let fixed = || {
        let (_, _, _, h, mi) = civil(t);
        format!("{} {h:02}:{mi:02}", fixed_date(t))
    };
    if FIXED.load(Ordering::Relaxed) {
        return fixed();
    }
    sys::format(t, true).unwrap_or_else(|| format!("{} UTC", fixed()))
}

#[cfg(unix)]
mod sys {
    use std::time::SystemTime;

    pub fn format(t: SystemTime, time: bool) -> Option<String> {
        static LOCALE: std::sync::Once = std::sync::Once::new();
        // The player's date form (LC_TIME only; numbers and text stay as
        // they are).
        LOCALE.call_once(|| unsafe {
            libc::setlocale(libc::LC_TIME, c"".as_ptr());
        });
        let secs = t.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs() as libc::time_t;
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        // SAFETY: both pointers are valid for the call.
        if unsafe { libc::localtime_r(&secs, &mut tm) }.is_null() {
            return None;
        }
        let fmt = if time { c"%x %H:%M" } else { c"%x" };
        let mut buf = [0u8; 128];
        // SAFETY: the buffer's length is passed; tm is filled.
        let n = unsafe { libc::strftime(buf.as_mut_ptr().cast(), buf.len(), fmt.as_ptr(), &tm) };
        (n > 0).then(|| String::from_utf8_lossy(&buf[..n]).into_owned())
    }
}

#[cfg(windows)]
mod sys {
    use std::time::SystemTime;

    #[repr(C)]
    #[derive(Default)]
    struct SysTime {
        year: u16,
        month: u16,
        dow: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        ms: u16,
    }

    #[repr(C)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn FileTimeToSystemTime(ft: *const FileTime, st: *mut SysTime) -> i32;
        fn SystemTimeToTzSpecificLocalTime(tz: *const u8, utc: *const SysTime, local: *mut SysTime) -> i32;
        fn GetDateFormatW(locale: u32, flags: u32, st: *const SysTime, fmt: *const u16, out: *mut u16, n: i32) -> i32;
        fn GetTimeFormatW(locale: u32, flags: u32, st: *const SysTime, fmt: *const u16, out: *mut u16, n: i32) -> i32;
    }
    const LOCALE_USER_DEFAULT: u32 = 0x0400;
    const DATE_SHORTDATE: u32 = 1;
    const TIME_NOSECONDS: u32 = 2;

    pub fn format(t: SystemTime, time: bool) -> Option<String> {
        let d = t.duration_since(SystemTime::UNIX_EPOCH).ok()?;
        let ticks = d.as_secs() * 10_000_000 + u64::from(d.subsec_nanos() / 100) + 116_444_736_000_000_000;
        let ft = FileTime { low: ticks as u32, high: (ticks >> 32) as u32 };
        let (mut utc, mut local) = (SysTime::default(), SysTime::default());
        let mut buf = [0u16; 128];
        // SAFETY: every pointer is valid for its call; the lengths are passed.
        unsafe {
            if FileTimeToSystemTime(&ft, &mut utc) == 0
                || SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) == 0
            {
                return None;
            }
            let n =
                GetDateFormatW(LOCALE_USER_DEFAULT, DATE_SHORTDATE, &local, std::ptr::null(), buf.as_mut_ptr(), 128);
            if n <= 0 {
                return None;
            }
            let mut s = String::from_utf16_lossy(&buf[..n as usize - 1]);
            if time {
                let n = GetTimeFormatW(
                    LOCALE_USER_DEFAULT,
                    TIME_NOSECONDS,
                    &local,
                    std::ptr::null(),
                    buf.as_mut_ptr(),
                    128,
                );
                if n > 0 {
                    s.push(' ');
                    s.push_str(&String::from_utf16_lossy(&buf[..n as usize - 1]));
                }
            }
            Some(s)
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod sys {
    pub fn format(_t: std::time::SystemTime, _time: bool) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_and_the_fixed_form() {
        let t = from_stamp("20261117-101500").unwrap();
        assert_eq!(crate::qol::saves::timestamp(t), "20261117-101500");
        assert_eq!(fixed_date(t), "17 Nov 2026");
        assert_eq!(from_stamp("19700101-000000"), Some(SystemTime::UNIX_EPOCH));
        assert!(from_stamp("bad").is_none());
        // The system's own form: some text with the year in it (whatever the
        // machine's zone and language).
        let local = sys::format(t, true).unwrap_or_default();
        assert!(local.is_empty() || local.contains("26"), "{local}");
    }
}
