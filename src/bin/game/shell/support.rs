//! Ctrl+F8 in the game (docs/troubleshooting.md): the recent events into
//! the log, then a support file (`nfsu2::diag::support`) written on a
//! thread of its own, its path in the log. Nothing the game draws, hears
//! or simulates changes; the key still reaches the game.

use nfsu2::diag::{events, support};
use std::sync::atomic::{AtomicU8, Ordering};
use winit::keyboard::KeyCode;

/// The Control keys held (bit 0 left, bit 1 right, bit 2 the window's
/// modifier state), from the window's own events (the game's held keys
/// follow its ticks; on Wayland a Control press may come only as a
/// modifier change).
static CTRL: AtomicU8 = AtomicU8::new(0);

/// A key event of the window: Ctrl+F8 (first press, not a repeat) makes
/// the support file.
pub(crate) fn key(code: KeyCode, pressed: bool, repeat: bool) {
    let bit = match code {
        KeyCode::ControlLeft => 1,
        KeyCode::ControlRight => 2,
        _ if is_hotkey(code, pressed, repeat, CTRL.load(Ordering::Relaxed) != 0) => {
            support_hotkey();
            return;
        }
        _ => return,
    };
    if pressed {
        CTRL.fetch_or(bit, Ordering::Relaxed);
    } else {
        CTRL.fetch_and(!bit, Ordering::Relaxed);
    }
}

/// Whether a key event is Ctrl+F8's first press.
fn is_hotkey(code: KeyCode, pressed: bool, repeat: bool, ctrl: bool) -> bool {
    code == KeyCode::F8 && pressed && !repeat && ctrl
}

/// The window's modifiers changed.
pub(crate) fn modifiers(control: bool) {
    if control {
        CTRL.fetch_or(4, Ordering::Relaxed);
    } else {
        CTRL.store(0, Ordering::Relaxed);
    }
}

/// Ctrl+F8 pressed.
fn support_hotkey() {
    events::record(events::Kind::Hotkey, "Ctrl+F8: support file");
    events::dump_to_log("Ctrl+F8");
    let spawned = std::thread::Builder::new().name("support-file".into()).spawn(|| {
        // The lines just sent are in the file before it is copied.
        nfsu2::qol::log_file::flush(std::time::Duration::from_millis(500));
        let req = support::SupportRequest {
            origin: support::Origin::Game,
            reason: Some("Ctrl+F8 pressed in the game".into()),
            ..Default::default()
        };
        match support::write_support_file(&req) {
            Ok(p) => nfsu2::qol::log_file::fact(&format!(
                "support file written: {}",
                nfsu2::qol::crash::scrub(&p.display().to_string())
            )),
            Err(e) => log::warn!("support file not written: {e}"),
        }
    });
    if let Err(e) = spawned {
        log::warn!("support file not started: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_first_press_of_ctrl_f8_is_the_hotkey() {
        assert!(is_hotkey(KeyCode::F8, true, false, true));
        assert!(!is_hotkey(KeyCode::F8, true, false, false), "F8 alone");
        assert!(!is_hotkey(KeyCode::F8, true, true, true), "a repeat");
        assert!(!is_hotkey(KeyCode::F8, false, false, true), "a release");
        assert!(!is_hotkey(KeyCode::F9, true, false, true));
    }
}
