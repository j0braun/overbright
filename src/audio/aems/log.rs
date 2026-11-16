#[cfg(any(test, feature = "dev-harness"))]
use std::io::Write;
#[cfg(any(test, feature = "dev-harness"))]
use std::sync::Mutex;
#[cfg(any(test, feature = "dev-harness"))]
static LOG: Mutex<Option<(std::fs::File, f64, Vec<(usize, u32, f64, Vec<i32>)>)>> = Mutex::new(None);
#[cfg(any(test, feature = "dev-harness"))]
static INIT: std::sync::Once = std::sync::Once::new();
#[cfg(any(test, feature = "dev-harness"))]
fn with(f: impl FnOnce(&mut (std::fs::File, f64, Vec<(usize, u32, f64, Vec<i32>)>))) {
    INIT.call_once(|| {
        if let Some(p) = std::env::var_os("NFSU2_EVENT_LOG") {
            if let Ok(file) = std::fs::File::create(p) {
                *LOG.lock().unwrap() = Some((file, 0.0, Vec::new()));
            }
        }
    });
    if let Some(l) = LOG.lock().unwrap().as_mut() {
        f(l);
    }
}
/// Whether `NFSU2_EVENT_LOG` is set (read once).
#[cfg(any(test, feature = "dev-harness"))]
pub fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NFSU2_EVENT_LOG").is_some())
}
/// Without the developer harness the log is off.
#[cfg(not(any(test, feature = "dev-harness")))]
pub fn enabled() -> bool {
    false
}
/// The time the next rows carry.
#[cfg(any(test, feature = "dev-harness"))]
pub fn set_log_time(t: f64) {
    if !enabled() {
        return;
    }
    with(|l| l.1 = t);
}
#[cfg(any(test, feature = "dev-harness"))]
pub(super) fn row(bank: usize, name: &str, id: u32, params: &[i32], what: char) {
    with(|l| {
        let t = l.1;
        let k = l.2.iter().position(|e| e.0 == bank && e.1 == id);
        match what {
            's' => {
                if let Some(k) = k {
                    if l.2[k].3 == params && t - l.2[k].2 < 0.1 {
                        return;
                    }
                    l.2[k].2 = t;
                    l.2[k].3 = params.to_vec();
                } else {
                    l.2.push((bank, id, t, params.to_vec()));
                }
            }
            'p' => l.2.push((bank, id, t, params.to_vec())),
            _ => {
                if let Some(k) = k {
                    l.2.swap_remove(k);
                }
            }
        }
        let p: Vec<String> = params.iter().map(|v| v.to_string()).collect();
        let _ = writeln!(l.0, "{t:.3}\t{what}\t{name}\t{bank:x}.{id}\t{}", p.join(" "));
    });
}
/// Without the developer harness: no log (never called: `enabled` is false).
#[cfg(not(any(test, feature = "dev-harness")))]
pub(super) fn row(_bank: usize, _name: &str, _id: u32, _params: &[i32], _what: char) {}
/// A check aid's row in the same log (a value the clone computes outside
/// an event graph, e.g. the player engine's voice bytes as `GIN`), kept
/// as an `s` row of its own name.
#[cfg(any(test, feature = "dev-harness"))]
pub fn check_row(name: &str, params: &[i32]) {
    if enabled() {
        row(usize::MAX, name, 0, params, 's');
    }
}
/// Without the developer harness: no log.
#[cfg(not(any(test, feature = "dev-harness")))]
pub fn check_row(_name: &str, _params: &[i32]) {}
