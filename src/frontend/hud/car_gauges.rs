//! Pieces of [`Hud::quads`], called in its order (see there).

use super::*;

impl Hud {
    /// The turbo gauge's objects and needle (0x4C6B80).
    pub(super) fn turbo_gauge(&mut self, f: &QuadsFrame<'_>) {
        let QuadsFrame { input, lo, hi, drag, .. } = *f;
        let s = &mut self.state;
        // An element turned on shows its objects (0x4C6690 → FEObject_Show),
        // which clears an object's own invisible flag: RPM_Redline, the drag
        // tach's red band (flags 0x40000001), shows so.
        if drag && lo & DRAG_TACH.lo != 0 {
            s.shown.insert(fe_hash("RPM_Redline"));
        }
        // The turbo gauge's update (0x4C6B80, element 0x4F5A70, its objects
        // +0x20..+0x34): while its mask is on, shown with boost, hidden
        // without (0x50CA50 / 0x50CA00).
        if !input.turbo {
            s.hidden.extend(TURBO.objects.iter().map(|n| fe_hash(n)));
        } else if lo & TURBO.lo != 0 || hi & TURBO.hi != 0 {
            // With boost, FEObject_Show (0x50CA50) on each of the six
            // every frame clears the objects' own invisible flag: HUD_Drag
            // ships the lines, dial and red marks with it set (flags
            // 0x40000001), so a turbo car's drag gauge showed only its fill
            // (Wine round 3A, the stock RX-7: "+ 30 20 10", the needle).
            s.shown.extend(TURBO.objects.iter().map(|n| fe_hash(n)));
        }
        // Its needle (+0x28, 3rdperson_TurboDial) turns by the boost b:
        // 0x4AA160(b, a, c) = (c − a)(b + 1)/2 + a, less 360 over 360 and
        // 360 − it under 0 (sic), with (a, c) = (−14°, 222°), or (158°,
        // 96°) in a drag race (0x89E7D8 / 0x890108); 0x51CEE0 sets it.
        if lo & TURBO.lo != 0 || hi & TURBO.hi != 0 {
            s.angle
                .insert(fe_hash("3rdperson_TurboDial"), turbo_needle(input.boost, input.drag.is_some()).to_radians());
        }
    }

    /// 0x4C68D0: speedometer, gear, needle and shift light.
    pub(super) fn tachometer(&mut self, f: &QuadsFrame<'_>) {
        let QuadsFrame { assets, input, lo, .. } = *f;
        let s = &mut self.state;
        let hide = |s: &mut ScreenState, name: &str| {
            s.hidden.insert(fe_hash(name));
        };
        let text = |s: &mut ScreenState, name: &str, t: String| {
            s.text.insert(fe_hash(name), t);
        };
        let label = |h: u32| assets.strings.get(h).unwrap_or_default().to_string();
        // 0x4C68D0: speedometer, gear, needle and shift light.
        if lo & 0x1000_0060 != 0 {
            // 0x4C7159: km/h (× 3.6, "KM/H") with the metric option, else
            // mph (× 2.237).
            let (units, per) = if input.options.metric { (KMH, 3.6) } else { (MPH, 2.237) };
            text(s, "3rdPersonSpeedUnits", label(units));
            text(s, "3rdPersonSpeed", format!("{:3}", (input.speed.abs() * per) as i32));
            let letter = GEARS.iter().find(|g| g.0 == input.gear as i32).map_or('X', |g| g.1);
            text(s, "3rdPersonGear", letter.to_string());
            // The gear's colour by car +0x448 (the clutch locked): 0x4C6AA6
            // grey 0xFFC3C3C3 locked, 0xFFFF8040 while a shift runs; the
            // drag tach (0x4C7386) its own colour (+0x98) locked, else
            // 0xFFFF8000.
            match (input.drag.is_some(), input.clutch_locked) {
                (false, true) => s.color.insert(fe_hash("3rdPersonGear"), argb(0xFFC3_C3C3)),
                (false, false) => s.color.insert(fe_hash("3rdPersonGear"), argb(0xFFFF_8040)),
                (true, false) => s.color.insert(fe_hash("3rdPersonGear"), argb(0xFFFF_8000)),
                (true, true) => None,
            };
            // The dial's scale (0x60FE20) and the needle's sweep
            // (0x4F5100: 235° at the scale's top; 0x4AA0E0).
            let dial = match (input.max_rpm, input.redline) {
                (m, r) if m >= 9000.0 && r >= 8500.0 => 10_500.0,
                (m, r) if m >= 8000.0 && r >= 7500.0 => 9500.0,
                (m, r) if m >= 7000.0 && r >= 6500.0 => 8500.0,
                _ => 7500.0,
            };
            // The car's HUD textures (0x5ED9C0 with the HUDINDEX
            // attribute; GLOBAL/HUD_CustomTextures): dial lines by scale,
            // the redline mark, fill, needle and turbo gauge.
            let index = input.hud_index;
            let lines = dial as u32;
            let redline = match (input.max_rpm, input.redline) {
                (m, r) if m >= 9000.0 && r >= 8500.0 => {
                    if r >= 9000.0 {
                        9000
                    } else {
                        8500
                    }
                }
                (m, r) if m >= 8000.0 && r >= 7500.0 => {
                    if r >= 8000.0 {
                        8000
                    } else {
                        7500
                    }
                }
                (m, r) if m >= 7000.0 && r >= 6500.0 => {
                    if r >= 7000.0 {
                        7000
                    } else {
                        6500
                    }
                }
                (_, r) => {
                    if r >= 6000.0 {
                        6000
                    } else {
                        5500
                    }
                }
            };
            // The drag tach (0x4F6500 → 0x5ED9C0 with index −1, one local
            // human in a drag race): only its numbers and RPM_Redline take
            // textures, DRAG_RPM_<n>_LINES (10000 / 9000 / 8000 / 7000 by
            // the dial's scale) and DRAG_RPM_REDLINES_<rpm>; with no numbers
            // texture both hide (+0xA4). Its other images stay the
            // package's own.
            let drag_textures = input.drag.is_some().then(|| {
                let numbers = match lines {
                    10_500 => 10_000,
                    9500 => 9000,
                    8500 => 8000,
                    _ => 7000,
                };
                [
                    ("3rdPersonTachRPMNumbers", format!("DRAG_RPM_{numbers}_LINES")),
                    ("RPM_Redline", format!("DRAG_RPM_REDLINES_{redline}")),
                ]
            });
            if let Some(pairs) = &drag_textures {
                if assets.texture_for(&pairs[0].1).is_some() {
                    for (object, texture) in pairs {
                        if let Some(t) = assets.texture_for(texture) {
                            s.image.insert(fe_hash(object), t);
                        }
                    }
                } else {
                    hide(s, "3rdPersonTachRPMNumbers");
                    hide(s, "RPM_Redline");
                }
            }
            for (object, texture) in [
                ("3rdPersonTachRPMNumbers", format!("3RDPERSON_{lines}LINES_CUSTOM_{index:02}")),
                ("Redline", format!("3RDPERSON_REDLINE_{redline}")),
                ("RPM_Redline", format!("3RDPERSON_REDLINE_{redline}")),
                ("tach_fill", format!("TACH_FILL_CUSTOM_{index:02}")),
                ("3rdPersonNeedle", format!("3RDPERSON_RPMDIAL_CUSTOM_{index:02}")),
                ("3rdperson_Turbolines", format!("3RDPERSON_TURBOLINES_CUSTOM_{index:02}")),
                ("3rdperson_Turbofill", format!("3RDPERSON_TURBOFILL_CUSTOM_{index:02}")),
                ("3rdperson_TurboDial", format!("3RDPERSON_TURBODIAL_CUSTOM_{index:02}")),
            ] {
                if drag_textures.is_some() {
                    break;
                }
                if let Some(t) = assets.texture_for(&texture) {
                    s.image.insert(fe_hash(object), t);
                }
            }
            // The custom tach (0x4F5100, the non-drag HUD: 0x4F8D7C) tints
            // its images with the fitted colour parts (0x51D870), white for
            // an empty slot.
            if input.drag.is_none() {
                for (k, objects) in super::super::custom_hud::HUD_TINTS {
                    for o in objects {
                        s.color.insert(fe_hash(o), super::super::custom_hud::hud_color(input.gauge_colors[k]));
                    }
                }
            }
            let degrees = match &input.drag {
                Some(d) => drag_needle_angle(input.rpm, input.max_rpm, d.needle),
                None => input.max_rpm / dial * 235.0 * (input.rpm / input.max_rpm.max(1.0)).clamp(0.0, 1.0),
            };
            s.angle.insert(fe_hash("3rdPersonNeedle"), degrees.to_radians());
            // The drag tachometer's shift light: below (0x4C7090).
            if input.drag.is_none() && !input.shift_light {
                hide(s, "Shift_light");
            }
        }
    }

    /// The drag tachometer, HEAT_Bar and the nitrous gauge.
    pub(super) fn drag_gauges(&mut self, f: &QuadsFrame<'_>) {
        let QuadsFrame { assets, package, input, lo, hi, now, .. } = *f;
        if lo & 0x1000_0060 != 0
            && let Some(d) = &input.drag
        {
            self.drag_tach(assets, package, d, input, now);
        }
        // 0x4C7AA0: HEAT_Bar (0x20000000) filled by 0x4C66F0(bar, heat,
        // 1, 105, 5) with one local human (0x89E7C0 = 1; else 25, 25).
        if lo & 0x2000_0000 != 0
            && let Some(d) = &input.drag
        {
            bar(assets, package, &mut self.state, "HEAT_Bar", d.heat, 105.0, 5.0);
        }
        // 0x4C7B20: the nitrous gauge (0x4F6A90, hi 0x40000), one local
        // human. A drag race fills NOS_Line as a bar (0x4C66F0(…, 1, 45,
        // −9)); elsewhere NOS_GameBreaker shows, coloured 0x96005A7D
        // (element +0x2C; 0x9600B4FA past full), and NOS_Mask turns to
        // −28 − (152 − f·152)°. Empty: both hidden. Past full (the style-point
        // refill, 0x592650) the excess fills the second object.
        if hi & 0x4_0000 != 0
            && let Some(f) = input.nitrous
        {
            let (line, breaker, mask) = (fe_hash("NOS_Line"), fe_hash("NOS_GameBreaker"), fe_hash("NOS_Mask"));
            let st = &mut self.state;
            let mut show = |h: u32, on: bool| {
                if on {
                    st.hidden.remove(&h);
                } else {
                    st.hidden.insert(h);
                }
            };
            if f <= 0.0 {
                show(line, false);
                show(breaker, false);
            } else if input.drag.is_some() {
                show(line, true);
                show(breaker, f > 1.0);
                bar(assets, package, &mut self.state, "NOS_Line", f.min(1.0), 45.0, -9.0);
                if f > 1.0 {
                    bar(assets, package, &mut self.state, "NOS_GameBreaker", f - 1.0, 45.0, -9.0);
                }
            } else {
                // Past full is f > 1 (0x4C7B20's test is strict: a full
                // tank, 1.0, takes the plain colour with the mask at f).
                show(line, f > 1.0);
                show(breaker, true);
                let (fill, colour) = if f > 1.0 { (f - 1.0, 0x9600_B4FA) } else { (f, 0x9600_5A7D) };
                self.state.angle.insert(mask, (-28.0 - (152.0 - fill * 152.0)).to_radians());
                self.state.color.insert(breaker, argb(colour));
            }
        }
    }
}
