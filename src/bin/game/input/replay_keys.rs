//! UI_ReplayControl's keys (`Game::dispatch` in a replay; cut from it,
//! statement order unchanged).

use crate::*;

impl Game {
    /// UI_ReplayControl's keys (0x4D7BD0): Back (0x911AB364 →
    /// 0x4C1220) skips to THE_PRESENT and leaves, Accept
    /// (0x0C407210 → 0x4A4E80) plays or pauses, BUTTON1 (0xC519BFC0
    /// → 0x4A5090, REPLAY_MOVE_CAMERA) changes the camera
    /// (0x5FAFE0), 0xB5AF2461 slow motion (0x4A4FB0), 0x5073EF13 /
    /// 0xD9FEEC59 skip back / forward (0x4A4CA0 / 0x4A4DD0); 0xC519BFBF
    /// hides the bar (0x4D800D → HIDE 0x4A5200; 0xD97360E9 is
    /// compared too, at 0x4D7FC2, but no pad bit sends it). The events are bin
    /// hashes of PAD_START (slow motion), PAD_LTRIGGER / PAD_RTRIGGER
    /// (skips) and PAD_BUTTON1 (camera). The FE keyboard map (the
    /// descriptor table 0x7FDD10, first row per JOY id) gives them
    /// their keys: FENG_START comma (row 4), FENG_LTRIGGER /
    /// _RTRIGGER [ / ] (rows 9, 10), FENG_BUTTON1 C (row 6),
    /// FENG_CANCEL Esc (row 13), FENG_SELECT / _ALTERNATE Enter /
    /// Num Enter (rows 11, 12).
    /// The bar's hide (0xC519BFBF → 0x4A5200) and the Help path
    /// (0xD97360E9) take no PC key: **observed** under Wine
    /// (2026-10-31), P, H and every other key leave the control's
    /// +0x74 at 0 and the bar up, so the clone binds neither.
    pub(super) fn replay_key(&mut self, code: Option<KeyCode>) -> Result<()> {
        let Some(code) = code else { return Ok(()) };
        let Mode::Race(r) = &mut self.mode else { unreachable!() };
        let (Some(p), Some(rec)) = (r.replay.playback.as_mut(), r.replay.recording.as_ref()) else { return Ok(()) };
        let leaving = p.leaving;
        // The time now (0x7FB718), or a waiting skip's target time.
        let now = match p.skip {
            Some((SkipTo::Lap(i), _)) => rec.mark(i).time,
            _ => rec.time_at(p.at),
        };
        let marker_time = |m: usize| rec.mark(m).time;
        let mut leave_now = false;
        match code {
            // Back (0x4C1220, once: +0xD2): a skip to THE_PRESENT
            // when one was taken (the pause's replay), else, with
            // no skip waiting, the replay is left at once
            // (0x4A50E0); a visible bar is hidden (0x4A5200).
            KeyCode::Escape if !leaving => {
                p.leaving = true;
                if p.present.is_some() {
                    p.skip = Some((SkipTo::Present, p.clock));
                    p.paused = true;
                } else if p.skip.is_none() {
                    leave_now = true;
                }
                p.hidden = true;
            }
            // Only with frames left (manager +0x7C over 0x7FB724).
            KeyCode::Enter | KeyCode::NumpadEnter if p.skip.is_none() && p.at < rec.frames.len() => {
                p.paused = !p.paused
            }
            // Not while paused (manager +0x14).
            // Replay Cameras "Extended": smooth ½ / ¼ speed.
            KeyCode::Comma if !p.paused && nfsu2::extras::replay_cameras() => {
                p.slow = false;
                let next = nfsu2::replay_cams::next_speed(if p.smooth > 0.0 { p.smooth } else { 1.0 });
                p.smooth = if next < 1.0 { next } else { 0.0 };
                p.slow_prev = None;
            }
            KeyCode::Comma if !p.paused => p.slow = !p.slow,
            // Back: the last marker over 1 s before now, else the
            // first; forward: the first after now, else none.
            // Either ends slow motion.
            KeyCode::BracketLeft | KeyCode::BracketRight if !leaving => {
                p.slow = false;
                p.smooth = 0.0;
                p.slow_prev = None;
                let to = if code == KeyCode::BracketLeft {
                    p.markers.iter().rev().find(|&&m| marker_time(m) < now - 1.0).or(p.markers.first()).copied()
                } else {
                    p.markers.iter().find(|&&m| marker_time(m) > now).copied()
                };
                if let Some(m) = to {
                    p.skip = Some((SkipTo::Lap(m), p.clock));
                    p.paused = true;
                }
            }
            KeyCode::KeyC => {
                p.camera.cycle();
                if let (Some(v), Some(d)) = (p.camera.view, self.sim.drive.as_mut())
                    && p.camera.index != 0
                {
                    d.chase = ChaseCamera::new(&d.car, v);
                }
            }
            _ => {}
        }
        if leave_now {
            self.end_replay();
        }
        Ok(())
    }
}
