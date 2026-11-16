use crate::frontend::fe_ids;
use super::*;

/// How long `object`'s `script` runs, in the real ms the HUD's script
/// starts are kept in (0 when absent). 0x51D050 compares the script's time
/// (+0x10) with its length (+0xC), both package ms, and the package manager
/// 0x558C10 steps package time at 1.2 × real time (`FE_RATE`, verified), as
/// `layout` replays the HUD's scripts; so a script of length L is over after
/// L / 1.2 real ms (audit 2o-002: the 600 ms Shift_light pulse was
/// restarted only after 600 real ms though drawn over at 500, a dark gap
/// every cycle, and ShiftMessage / Out_Run_Message stayed up 20 % long).
pub(super) fn script_length(package: &super::super::feng::Package, object: u32, script: &str) -> f32 {
    let name = crate::formats::hash::fe_hash(script);
    package
        .objects
        .iter()
        .find(|o| o.hash == object)
        .and_then(|o| o.scripts.iter().find(|s| s.name == name))
        .map_or(0.0, |s| s.length as f32 / super::super::FE_RATE)
}

impl Hud {
    /// The drag tachometer's gear of its last update (+0x80, set from the
    /// car's +0x434 at 0x4C7A72): the gear the shift light and the needle
    /// tint are computed for (0x4C7780, 0x4C77C5, 0x4C77F6).
    pub fn drag_last_gear(&self) -> usize {
        self.drag_gear
    }

    /// The drag ShiftMessage's string label while it shows (checks: the drag
    /// rate comparison logs when each message comes up).
    pub fn drag_shift_message(&self) -> Option<u32> {
        self.shift_message.map(|(_, label)| label)
    }

    /// 0x4C7090's drag parts: the shift rating message, RPM_Redline's
    /// blink and the shift light's pulse.
    pub(super) fn drag_tach(
        &mut self,
        assets: &Assets,
        package: &super::super::feng::Package,
        d: &DragHud,
        input: &Input,
        now: f32,
    ) {
        let s = &mut self.state;
        let message = fe_hash("ShiftMessage");
        let q = d.shift_quality;
        // RPM_Redline blinks past the redline after a missed (negative)
        // up-shift, unless "Over Rev!" is up; else back to Init.
        let over_rev = matches!(self.shift_message, Some((_, fe_ids::TXT_OVER_REV)));
        if q < 0.0 && !over_rev && input.rpm > input.redline {
            if !self.redline_blink {
                s.run(fe_hash("RPM_Redline"), "Blink", now);
                self.redline_blink = true;
            }
        } else if self.redline_blink {
            s.run(fe_hash("RPM_Redline"), "Init", now);
            self.redline_blink = false;
        }
        // ShiftMessage zooms in once per gear change: "Perfect Shift!"
        // (0x0598B065) over 0.5, else "Good Shift!" (0x27D2DD45) for a
        // rating in (0, 1]; going up from a gear with no rating, "Short
        // Shift!" (0x2303D76C) at 0, "Over Rev!" (0x27D75CD8) under 0.
        // It hides when ZoomIn ends.
        if let Some((start, _)) = self.shift_message
            && now - start > script_length(package, message, "ZoomIn")
        {
            self.shift_message = None;
        }
        let gear = input.gear;
        if self.shift_message.is_none() && gear != self.drag_gear {
            let label = if q > 0.0 && q <= 1.0 {
                Some(if q > 0.5 { 0x0598_B065 } else { fe_ids::TXT_GOOD_SHIFT })
            } else if gear > self.drag_gear && self.drag_gear != 0 {
                (q <= 0.0).then_some(if q == 0.0 { 0x2303_D76C } else { fe_ids::TXT_OVER_REV })
            } else {
                None
            };
            if let Some(label) = label {
                s.run(message, "ZoomIn", now);
                self.shift_message = Some((now, label));
            }
        }
        match self.shift_message {
            Some((_, label)) => {
                s.text.insert(message, assets.strings.get(label).unwrap_or_default().to_string());
            }
            None => {
                s.hidden.insert(message);
            }
        }
        self.drag_gear = gear;
        // The shift light pulses red past the shift point, blue below a
        // 0.5 rating (not in reverse), green above.
        let light = fe_hash("Shift_light");
        // 3rdPersonNeedle's tint with one local human (0x89E7C0 = 1):
        // its own colour (kept at +0x9C) while the light is off, 0xFF800000
        // at −1, 0xFF00FF00 over 0.5, and in a forward or neutral gear
        // (+0x434 ≠ 0) up to 0.5 its colour blended toward (A 255, R 0,
        // G 128, B 192) by `needle_blend`, each channel clamped to 0..255
        // (0x4AB0B0, 0x4A9E30).
        let needle = fe_hash("3rdPersonNeedle");
        match d.shift_light {
            Some(r) if r == -1.0 => {
                let c = argb(0xFF80_0000);
                s.color.insert(needle, c);
                self.needle_color = Some(c);
            }
            Some(r) if r > 0.5 => {
                let c = argb(0xFF00_FF00);
                s.color.insert(needle, c);
                self.needle_color = Some(c);
            }
            Some(_) if gear != 0 => {
                if let Some(o) = package.objects.iter().find(|o| o.hash == needle) {
                    let base = o.attributes.color();
                    let blue = [0.0, 128.0 / 255.0, 192.0 / 255.0, 1.0];
                    let t = d.needle_blend;
                    let c: [f32; 4] = std::array::from_fn(|i| {
                        ((base[i] + (blue[i] - base[i]) * t) * 255.0).clamp(0.0, 255.0).trunc() / 255.0
                    });
                    s.color.insert(needle, c);
                    self.needle_color = Some(c);
                }
            }
            // In reverse (+0x434 = 0) up to 0.5 the game writes no tint, so
            // the last one written stays rather than the own colour.
            Some(_) => {
                if let Some(c) = self.needle_color {
                    s.color.insert(needle, c);
                }
            }
            // Light off: its own colour.
            None => self.needle_color = None,
        }
        match d.shift_light {
            None => {
                s.hidden.insert(light);
                self.groups.remove(&light);
            }
            Some(r) => {
                let script = if r == -1.0 {
                    "PulseRed"
                } else if r <= 0.5 {
                    "PulseBlue"
                } else {
                    "PulseGreen"
                };
                // 0x4C7090 runs the script unless it is the object's
                // current one and not yet past its length (0x51D050: script
                // +0x30 is the name and time +0x10 ≤ length +0xC), so each
                // 600 ms pulse (no chain, flags 0: it holds at alpha 0)
                // starts over once done. It leaves a blue light alone in
                // reverse.
                let running = self.groups.get(&light) == Some(&script)
                    && s.scripts.get(&light).is_some_and(|&(name, start)| {
                        name == fe_hash(script) && now - start <= script_length(package, light, script)
                    });
                if r > -1.0 && r <= 0.5 && gear == 0 {
                } else if !running {
                    s.run(light, script, now);
                    self.groups.insert(light, script);
                }
            }
        }
    }
}
