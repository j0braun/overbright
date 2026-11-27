//! Pieces of [`Hud::quads`], called in its order (see there).

use super::*;

impl Hud {
    /// 0x4DABD0: the outrun meter.
    pub(super) fn outrun_meter(&mut self, f: &QuadsFrame<'_>) {
        let QuadsFrame { assets, package, input, hi, now, .. } = *f;
        let s = &mut self.state;
        let text = |s: &mut ScreenState, name: &str, t: String| {
            s.text.insert(fe_hash(name), t);
        };
        let label = |h: u32| assets.strings.get(h).unwrap_or_default().to_string();
        // 0x4DABD0: the outrun meter (high 0x40).
        if hi & 0x40 != 0
            && let Some(o) = &input.outrun
        {
            // 0x4C9780: the frames shown count to 4; on the fourth
            // Out_Run_Message zooms in with the challenge, on the fifth
            // the 0x8CA76279 group turns ACTIVE.
            let message = fe_hash("Out_Run_Message");
            if self.meter_frames == 3 {
                self.outrun_message = Some((crate::outrun::challenge(o.leading), now));
                s.run(message, "ZOOMIN", now);
            } else if self.meter_frames == 4 && self.groups.get(&fe_ids::OBJ_8CA76279) != Some(&"ACTIVE") {
                s.run(fe_ids::OBJ_8CA76279, "ACTIVE", now);
                self.groups.insert(fe_ids::OBJ_8CA76279, "ACTIVE");
            }
            if self.meter_frames < 4 {
                self.meter_frames += 1;
            }
            let m = crate::outrun::meter(o.value, self.meter_last, o.racing, input.options.metric);
            self.meter_last = o.value;
            if let Some(l) = m.message {
                self.outrun_message = Some((l, now));
                s.run(message, "ZOOMIN", now);
            }
            let number = fe_hash("Distance_Number");
            let script = if m.ahead { "AHEAD" } else { "BEHIND" };
            if self.groups.get(&number) != Some(&script) {
                s.run(number, script, now);
                self.groups.insert(number, script);
            }
            s.color.insert(
                fe_hash("Getaway_Distance_Bar"),
                argb(if m.ahead { crate::outrun::AHEAD_COLOUR } else { crate::outrun::BEHIND_COLOUR }),
            );
            text(s, "Distance_Text", label(m.unit));
            text(s, "Distance_Number", grouped(i64::from(m.number)));
            hbar(assets, package, &mut self.state, "Getaway_Distance_Bar", m.fill);
            // The message shows while its ZoomIn runs.
            let s = &mut self.state;
            match self.outrun_message {
                Some((_, start)) if now - start > script_length(package, message, "ZOOMIN") => {
                    self.outrun_message = None;
                    s.hidden.insert(message);
                }
                Some((l, _)) => {
                    s.text.insert(message, label(l));
                }
                None => {
                    s.hidden.insert(message);
                }
            }
        } else {
            // Hidden: 0x8CA76279 INACTIVE, and outside race state 7 the
            // frame count starts again.
            let s = &mut self.state;
            if self.groups.get(&fe_ids::OBJ_8CA76279) != Some(&"INACTIVE") {
                s.run(fe_ids::OBJ_8CA76279, "INACTIVE", now);
                self.groups.insert(fe_ids::OBJ_8CA76279, "INACTIVE");
            }
            if input.outrun.is_none() {
                self.meter_frames = 0;
            }
            self.outrun_message = None;
        }
    }
}
