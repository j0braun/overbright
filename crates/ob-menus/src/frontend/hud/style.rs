//! Pieces of [`Hud::quads`], called in its order (see there).

use super::*;

impl Hud {
    /// The style element's trick popups (0x4F7AD0, update 0x4DA900).
    pub(super) fn style_popups(&mut self, f: &QuadsFrame<'_>) {
        let QuadsFrame { assets, package, input, lo, now, .. } = *f;
        let s = &mut self.state;
        let label = |h: u32| assets.strings.get(h).unwrap_or_default().to_string();
        // The style element (0x4F7AD0; update 0x4DA900, its mask on: the
        // Score option, 0x5EA8D0). Its SP_DATA / Style_Point_Backing /
        // Style_Points_Title (the total, 0x4AB700) exist in no HUD
        // package, so only the popups and Tach_NOS_Gamebreaker show.
        let style_on = lo & STYLE != 0;
        if style_on && !self.popups.on {
            self.popups.visible = [true; 2];
        }
        self.popups.on = style_on;
        if style_on && input.drift.is_none() {
            let (point, kind, breaker) =
                (fe_hash("SP_Trick_Point"), fe_hash("SP_Trick_Type"), fe_hash("Tach_NOS_Gamebreaker"));
            let has = |o: u32, n: &str| {
                let name = fe_hash(n);
                package.objects.iter().any(|x| x.hash == o && x.scripts.iter().any(|sc| sc.name == name))
            };
            // 0x51D050: the object's current script is `n` and its time
            // has not passed its length.
            let running = |s: &ScreenState, o: u32, n: &str| {
                s.scripts
                    .get(&o)
                    .is_some_and(|&(h, start)| h == fe_hash(n) && now - start <= script_length(package, o, n))
            };
            // 0x51CF80(…, 1): a script the object lacks is not run.
            let run = |s: &mut ScreenState, o: u32, n: &str| {
                if has(o, n) {
                    s.run(o, n, now);
                }
            };
            let pp = &mut self.popups;
            pp.queue(&input.tricks);
            if !pp.queue.is_empty() {
                // 0x4DA710: the first one-off popup, else the first.
                let head = pp.queue.iter().position(|p| !p.continuous).unwrap_or(0);
                // Not while SP_Trick_Type fades out (or Tach_NOS_Gamebreaker
                // runs GLOW / GLOW_2, which no code starts: their names,
                // 0x796580 / 0x796578, are only read here).
                if !running(s, kind, "fadeout") && !running(s, breaker, "GLOW") && !running(s, breaker, "GLOW_2") {
                    // 0x4AB780(popup, SP_Trick_Type, SP_Trick_Point).
                    let p = &mut pp.queue[head];
                    let per = p.name != DRIFT_LABEL;
                    // 0x4AB7A3: `fild count; fdivr points` then _ftol, so a
                    // count of 0 divides to ±inf / NaN and prints 0.
                    let v = ftol(if per { p.points / p.count as f32 } else { p.points });
                    let crash = p.lost || p.points < 0.0;
                    // Element +0x68 (0xFFCF3030) for a loss, else +0x60
                    // (0xFFCAF48A; +0x64, the same, is the drift race's).
                    let colour = if crash { 0xFFCF_3030 } else { 0xFFCA_F48A };
                    let sign = if crash { "" } else { "+" };
                    pp.point_text = if p.count > 1 && per {
                        format!("{sign}{}x{}", grouped_int(v), grouped_int(p.count))
                    } else {
                        format!("{sign}{}", grouped_int(v))
                    };
                    pp.type_text = label(p.name);
                    s.base_color.insert(point, colour);
                    s.base_color.insert(kind, colour);
                    if !p.shown {
                        if !running(s, point, "fadeout") {
                            run(s, point, "animate");
                            pp.visible[0] = input.style_points;
                        }
                        if !running(s, kind, "fadeout") {
                            run(s, kind, "animate");
                            pp.visible[1] = true;
                        }
                    } else if p.state == Some(3) || (!p.continuous && p.state != Some(5)) {
                        // Banked, or a one-off on its second showing: it
                        // fades out unless +0x24 is 5 — a new popup's +0x24
                        // is what its reused block held (`Popups::freed`).
                        run(s, kind, "fadeout");
                        if input.style_points {
                            run(s, point, if p.points < 0.0 { "fadeoutCrash" } else { "fadeout" });
                        }
                        p.state = Some(5);
                    }
                    if p.shown_at.is_none() {
                        p.shown = true;
                        p.shown_at = Some(now);
                    }
                }
                // Dropped 0.5 s (with others waiting) or 1 s after it was
                // put on screen; one never put there (+0x18 = 0) at once.
                let limit = if pp.queue.len() > 1 { 500.0 } else { 1000.0 };
                if now - pp.queue[head].shown_at.unwrap_or(f32::MIN) > limit {
                    pp.free(head);
                }
                pp.seen = true;
            } else if !running(s, kind, "animate") && !running(s, kind, "fadeout") {
                pp.visible = [false; 2];
            }
            // 0x4DA85D: the popups hide while GLOW / GLOW_2 run, or once
            // one has run with +0x7A clear (never: nothing runs them).
            if !input.options.gauges {
                s.hidden.insert(breaker);
            }
            if !pp.visible[0] {
                s.hidden.insert(point);
            }
            if !pp.visible[1] {
                s.hidden.insert(kind);
            }
            s.text.insert(point, pp.point_text.clone());
            s.text.insert(kind, pp.type_text.clone());
        }
    }
}
