use crate::frontend::fe_ids;
use super::*;

/// The engage prompt's queue (element +0x60, see [`Slot`]), the slots
/// dropped since the last update (drawn once in state 6), the current
/// marker's kind it last queued, whether the D-Day prompt was queued, and
/// the sounds not yet taken ([`Hud::take_sounds`]).
#[derive(Default)]
pub(super) struct Prompts {
    pub(super) prompts: [Slot; 8],
    pub(super) dropped: Vec<Slot>,
    pub(super) marker_kind: Option<u8>,
    pub(super) phone_queued: bool,
    pub(super) sounds: Vec<(u8, u8)>,
}

impl Prompts {
    /// 0x4DA9A0's step with the element's mask on (see [`Hud::quads`]).
    pub(super) fn update(&mut self, s: &mut ScreenState, sms_kind: u8, held: bool, now: f32) {
        let reset = |s: &mut ScreenState| {
            s.image.insert(ENGAGE_DPAD, fe_ids::OBJ_EA570722);
            s.hidden.extend([ENGAGE_ICON, ENGAGE_MAIL, ENGAGE_COUNT]);
            s.hidden.extend(ENGAGE_HIGHLIGHTS);
        };
        reset(s);
        for i in 0..8 {
            self.run_slot(i, s, sms_kind, held, now);
        }
        let active = self.prompts.iter().position(|x| matches!(x.state, 2 | 3));
        let pending = pending_slot(&self.prompts);
        match (active, pending) {
            (None, Some(p)) => {
                if self.prompts[p].state != 4 {
                    self.flash(p, now);
                }
                self.run_slot(p, s, sms_kind, held, now);
            }
            (None, None) => reset(s),
            (Some(a), Some(p)) if priority(self.prompts[p].kind) > priority(self.prompts[a].kind) => {
                if self.prompts[a].kind == 0x13 {
                    self.prompts[a].state = 4;
                    self.run_slot(a, s, sms_kind, held, now);
                } else {
                    self.prompts[a].state = 6;
                    self.run_slot(a, s, sms_kind, held, now);
                    self.prompts[a] = Slot::default();
                }
                self.flash(p, now);
            }
            (Some(a), _) => self.run_slot(a, s, sms_kind, held, now),
        }
    }

    /// 0x4C9170(kind, value): a used slot of the kind starts flashing again
    /// only from rest (4 → 2: the time, its sound, the value); in states
    /// 1–3 nothing changes; with none, the first free slot takes it queued
    /// (state 1).
    pub(super) fn queue(&mut self, kind: u8, value: u32, now: f32) {
        if let Some(i) = self.prompts.iter().position(|x| x.kind == kind && x.state != 0) {
            if self.prompts[i].state == 4 {
                self.flash(i, now);
                self.prompts[i].value = value;
            }
        } else if let Some(x) = self.prompts.iter_mut().find(|x| x.state == 0) {
            *x = Slot { state: 1, kind, since: x.since, value };
        }
    }

    /// 0x4DAB80(kind): the first slot of the kind drawn in state 6, then
    /// freed.
    pub(super) fn drop_kind(&mut self, kind: u8) {
        if let Some(x) = self.prompts.iter_mut().find(|x| x.kind == kind) {
            self.dropped.push(Slot { state: 6, ..*x });
            *x = Slot::default();
        }
    }

    /// A slot starts flashing: state 2 from now, with its sound.
    pub(super) fn flash(&mut self, i: usize, now: f32) {
        let x = &mut self.prompts[i];
        x.state = 2;
        x.since = now;
        if let Some(c) = slot_sound(x.kind, 2) {
            self.sounds.push((crate::audio::fe::event::FOCUS, c));
        }
    }

    /// 0x4C9030 on slot `i`: a slot with no kind is freed; one in state 2
    /// or more steps (0x4C8AC0: 2 → 3 when more than 1 s — kind 0x14 its
    /// value in seconds — has passed since state 2 began, the SMS's held
    /// while the D-Day script runs, 0x502420; 3 → 4 past 4 s; each with
    /// its sound) and draws.
    pub(super) fn run_slot(&mut self, i: usize, s: &mut ScreenState, sms_kind: u8, held: bool, now: f32) {
        let x = &mut self.prompts[i];
        if x.kind == 0 {
            x.state = 0;
            return;
        }
        if x.state < 2 {
            return;
        }
        let age = now - x.since;
        let first = if x.kind == 0x14 { x.value as f32 * 1000.0 } else { 1000.0 };
        let next = match x.state {
            2 if !(x.kind == 0x13 && held) && age > first => Some(3),
            3 if age > 4000.0 => Some(4),
            _ => None,
        };
        if let Some(n) = next {
            x.state = n;
            if let Some(c) = slot_sound(x.kind, n) {
                self.sounds.push((crate::audio::fe::event::FOCUS, c));
            }
        }
        slot_draw(s, &mut self.prompts[i], sms_kind, now);
    }
}

/// 0x4ABF20: a kind's priority (markers 4, the encounter's 9 / 10 3, the
/// SMS 2, 0x14 1).
pub(super) fn priority(kind: u8) -> u8 {
    match kind {
        1..=8 | 0xB..=0x12 => 4,
        9 | 10 => 3,
        0x13 => 2,
        0x14 => 1,
        _ => 0,
    }
}

/// 0x4C90B0: the first used slot of the highest priority.
pub(super) fn pending_slot(slots: &[Slot; 8]) -> Option<usize> {
    let mut best = (0, None);
    for (i, x) in slots.iter().enumerate() {
        if x.state != 0 && priority(x.kind) > best.0 {
            best = (priority(x.kind), Some(i));
        }
    }
    best.1
}

/// The sound a slot plays as it starts flashing (0x4DA9A0, 0x4C9170) or
/// as it steps (0x4C8AC0 → 0x4AB9F0): 0x51DE40(FOCUS, control), the cell
/// 0x837FE0[11·27 + control] — state 2 FE_SOUND_ENGAGE_PHONE (0xD, FE id
/// 14), the SMS's FE_SOUND_ENGAGE_SMS (0xF, id 16); state 3
/// FE_SOUND_ENGAGE_DPAD (0xE, id 15) whatever the kind; state 4 none;
/// kind 0x14 never (its callers skip it).
pub(super) fn slot_sound(kind: u8, state: u8) -> Option<u8> {
    use crate::audio::fe::control as c;
    match (kind, state) {
        (0x14, _) => None,
        (0x13, 2) => Some(c::ENGAGE_SMS),
        (_, 2) => Some(c::ENGAGE_PHONE),
        (_, 3) => Some(c::ENGAGE_DPAD),
        _ => None,
    }
}

/// 0x4C9030's draw by kind: 0x11 (0x4C8DD0) shares the marker visuals,
/// 0x13 [`sms_draw`] (0x4ABAF0), 0x14 [`phone_draw`] (0x4ABA30),
/// else [`marker_draw`] (0x4C8B80).
pub(super) fn slot_draw(s: &mut ScreenState, slot: &mut Slot, sms_kind: u8, now: f32) {
    match slot.kind {
        0x13 => sms_draw(s, *slot, sms_kind, now),
        0x14 => phone_draw(s, slot.state, now),
        _ => marker_draw(s, slot, now),
    }
}

/// Starts `name` on `o` unless it is its current script (0x51D050 /
/// 0x51CF80(…, 1)).
pub(super) fn start_script(s: &mut ScreenState, o: u32, name: &str, now: f32) {
    if !s.scripts.get(&o).is_some_and(|&(n, _)| n == fe_hash(name)) {
        s.run(o, name, now);
    }
}

/// 0x4C8B80, a marker's or the encounter's prompt: state 2 the D-pad to
/// 0x6FFF0767 and the mode icon to the kind's (0x4ABE50); 3 the D-pad
/// alone; 4 back to 3 (so a marker's prompt never rests); in 2 and 3 the
/// icon and the second highlight (+0x2C) shown and, unless running,
/// "pulse" on the highlight, "Blink" on the icon and "Pulse" on the D-pad.
/// 6: the icon and highlight hidden, the D-pad back to 0xEA570722, "Init"
/// on the three.
pub(super) fn marker_draw(s: &mut ScreenState, slot: &mut Slot, now: f32) {
    if slot.state == 4 {
        slot.state = 3;
    }
    let highlight = ENGAGE_HIGHLIGHTS[1];
    match slot.state {
        2 | 3 => {
            s.image.insert(ENGAGE_DPAD, fe_ids::OBJ_6FFF0767);
            // Native images retain 0x50C990's texture after state 2.
            // Our per-frame image overrides are cleared, so restore it
            // in state 3 too. The photo-spot handler 0x4C8DD0 writes it
            // in both states (0x4C8EB1), with the same scripts below.
            if let Some(icon) = engage_texture(slot.kind) {
                s.image.insert(ENGAGE_ICON, icon);
            }
            s.hidden.remove(&ENGAGE_ICON);
            s.hidden.remove(&highlight);
            start_script(s, highlight, "PULSE", now);
            start_script(s, ENGAGE_ICON, "BLINK", now);
            start_script(s, ENGAGE_DPAD, "PULSE", now);
        }
        6 => {
            s.hidden.insert(ENGAGE_ICON);
            s.hidden.insert(highlight);
            s.image.insert(ENGAGE_DPAD, fe_ids::OBJ_EA570722);
            for o in [ENGAGE_DPAD, highlight, ENGAGE_ICON] {
                start_script(s, o, "INIT", now);
            }
        }
        _ => {}
    }
}

/// 0x4ABA30, the D-Day script's prompt (kind 0x14): state 2 the D-pad to
/// 0x6FFF0767 with "Pulse"; 3 the texture with "Init"; 4 and 6 "Init"
/// alone (each unless running).
pub(super) fn phone_draw(s: &mut ScreenState, state: u8, now: f32) {
    match state {
        2 => {
            s.image.insert(ENGAGE_DPAD, fe_ids::OBJ_6FFF0767);
            start_script(s, ENGAGE_DPAD, "PULSE", now);
        }
        3 => {
            s.image.insert(ENGAGE_DPAD, fe_ids::OBJ_6FFF0767);
            start_script(s, ENGAGE_DPAD, "INIT", now);
        }
        4 | 6 => start_script(s, ENGAGE_DPAD, "INIT", now),
        _ => {}
    }
}

/// The outrun meter's inputs (0x4DABD0).
#[derive(Clone, Copy, Debug, Default)]
pub struct OutrunHud {
    /// 0x4C9450's distance, metres: above 0 the player is behind.
    pub value: f32,
    /// The player leads (racer +0x0A = 1).
    pub leading: bool,
    /// The race status is racing (its state 4), for the race-leader messages.
    pub racing: bool,
}

/// The engage prompt's objects (0x4F83D0, HUD_SingleRace): the D-pad
/// (+0x20), the highlights (+0x28, +0x2C), the mode icon (+0x34), the mail
/// (+0x38) and its count (+0x3C, a string).
pub(super) const ENGAGE_DPAD: u32 = fe_ids::ENGAGE_DPAD;
pub(super) const ENGAGE_HIGHLIGHTS: [u32; 2] = [fe_ids::ENGAGE_HIGHLIGHTS_0, fe_ids::ENGAGE_HIGHLIGHTS_1];
pub(super) const ENGAGE_ICON: u32 = fe_ids::ENGAGE_ICON;
pub(super) const ENGAGE_MAIL: u32 = fe_ids::ENGAGE_MAIL;
pub(super) const ENGAGE_COUNT: u32 = fe_ids::ENGAGE_COUNT;

/// The SMS slot's drawing (0x4ABAF0). States 2 and 3: the D-pad to
/// 0x6FFF0767, the mail to the newest unread message's icon (0x4974A0),
/// the mail and the first highlight shown, the count and the mode icon
/// hidden, and unless running "animation" on the mail, "Hide" on the
/// count, "pulse" on the highlight and "Pulse" on the D-pad. State 4: the
/// mail the envelope 0x4B9B8F5A, the D-pad back to 0xEA570722, the mail,
/// highlight and count shown with "static", "Init" on the D-pad, and the
/// count's text set from the format at 0x783FE5, which is empty. State 6:
/// the D-pad back, the highlight hidden, "Hide" on the mail and count,
/// "Init" on the D-pad. The slots draw in slot order, so with a marker's
/// prompt up the later slot's D-pad stands (0x4DA9A0). The icon's colour
/// (0x4974A0 → 0x535580 on the mail's "animation", markers162): see
/// [`super::super::ScreenState::script_color`].
pub(super) fn sms_draw(s: &mut ScreenState, slot: Slot, kind: u8, now: f32) {
    let highlight = ENGAGE_HIGHLIGHTS[0];
    match slot.state {
        2 | 3 => {
            s.image.insert(ENGAGE_DPAD, fe_ids::OBJ_6FFF0767);
            // The icon (0x4974A0) and its colour: when "animation" is not
            // yet running, 0x535580 moves that script's colour track so
            // its time-0 value is the kind's colour (the envelope's
            // green, (14, 156, 14), for the tips).
            let (icon, [r, g, b, alpha]) = super::super::sms::icon(kind);
            s.image.insert(ENGAGE_MAIL, icon);
            if !s.scripts.get(&ENGAGE_MAIL).is_some_and(|&(n, _)| n == fe_hash("ANIMATION")) {
                s.script_color.insert(ENGAGE_MAIL, (fe_hash("ANIMATION"), [b, g, r, alpha].map(u32::from)));
            }
            s.hidden.remove(&ENGAGE_MAIL);
            s.hidden.insert(ENGAGE_COUNT);
            s.hidden.insert(ENGAGE_ICON);
            s.hidden.remove(&highlight);
            start_script(s, ENGAGE_MAIL, "ANIMATION", now);
            start_script(s, ENGAGE_COUNT, "HIDE", now);
            start_script(s, highlight, "PULSE", now);
            start_script(s, ENGAGE_DPAD, "PULSE", now);
        }
        4 => {
            s.image.insert(ENGAGE_MAIL, super::super::sms::ENVELOPE);
            s.image.insert(ENGAGE_DPAD, fe_ids::OBJ_EA570722);
            for o in [ENGAGE_MAIL, ENGAGE_COUNT, highlight] {
                s.hidden.remove(&o);
                start_script(s, o, "STATIC", now);
            }
            start_script(s, ENGAGE_DPAD, "INIT", now);
            s.text.insert(ENGAGE_COUNT, String::new());
        }
        6 => {
            s.image.insert(ENGAGE_DPAD, fe_ids::OBJ_EA570722);
            s.hidden.insert(highlight);
            start_script(s, ENGAGE_MAIL, "HIDE", now);
            start_script(s, ENGAGE_COUNT, "HIDE", now);
            start_script(s, ENGAGE_DPAD, "INIT", now);
        }
        _ => {}
    }
}

/// The engage prompt's icon by kind (0x4ABE50): events 1–8 (0x4ABF70),
/// shops 0xB–0x10 (0x4ABFE0), 0x11–0x13 the others.
pub fn engage_texture(kind: u8) -> Option<u32> {
    Some(match kind {
        1 => fe_ids::QR_MODE_CIRCUIT_ICON,
        2 => fe_ids::QR_MODE_DRAG_ICON,
        3 => fe_ids::QR_MODE_DRIFT_ICON,
        4 => fe_ids::QR_MODE_SPRINT_ICON,
        5 | 6 => fe_ids::QR_MODE_STREET_X_ICON,
        7 => fe_ids::QR_MODE_URL_ICON,
        8 => 0xBF5A_C5A9,
        9 | 10 => 0x33B5_791A,
        0xB => 0x8244_F164,
        0xC => fe_ids::JUMP_TO_GARAGE_ICON,
        0xD => fe_ids::CUSTOMIZE_CATEGORY_0_ICON,
        0xE => fe_ids::CUSTOMIZE_CATEGORY_1_ICON,
        0xF => fe_ids::CUSTOMIZE_CATEGORY_2_ICON,
        0x10 => fe_ids::CUSTOMIZE_CATEGORY_3_ICON,
        0x11 => 0xD1D9_D8A4,
        0x12 => 0xB0C0_DCB5,
        0x13 => 0xF95D_D41E,
        _ => return None,
    })
}
