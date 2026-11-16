//! Pieces of [`Hud::quads`], called in its order (see there).

use super::*;

impl Hud {
    /// The engage prompt's queue and the SMS element (0x4F83D0, 0x4DB8C0).
    pub(super) fn prompt_queue(&mut self, f: &QuadsFrame<'_>) {
        let QuadsFrame { input, hi, now, .. } = *f;
        let s = &mut self.state;
        // The engage prompt's queue (element 0x4F83D0, HUD +0x58): eight
        // slots of 0x14 bytes at +0x60 ([`Slot`]). Its callers queue and
        // drop kinds whatever the element's mask: 0x4C9170 / 0x4DAB80.
        // * A marker's: 0x5EAFC0 (the car enters a valid marker's trigger)
        //   → 0x4CA020 → 0x4C9170(kind, 0); 0x5EAF40 (it leaves) and
        //   0x533BF0 (a world challenge starts) → 0x4DB820 → 0x4DAB80(kind).
        //   The clone queues on the current marker's change.
        // * The random encounter's: 0x413030 every frame of the encounter
        //   manager's TRYING_TO_ENGAGE (state 3) → 0x500D20 → 0x4DB890: a
        //   car to challenge queues kind 9, none drops it; the outrun's
        //   engage (0x602E79) drops it too.
        // * The D-Day script's: 0x5342C9 → 0x4CA670 → 0x4C9170(0x14, 2),
        //   once; nothing drops it.
        // * The SMS element's (0x4DB8C0, below, after this update).
        if input.engage != self.prompt.marker_kind {
            if let Some(k) = self.prompt.marker_kind {
                self.prompt.drop_kind(k);
            }
            if let Some(k) = input.engage {
                self.prompt.queue(k, 0, now);
            }
            self.prompt.marker_kind = input.engage;
        }
        if input.encounter {
            self.prompt.queue(9, 0, now);
        } else {
            self.prompt.drop_kind(9);
        }
        if input.phone.is_some() && !self.prompt.phone_queued {
            self.prompt.queue(0x14, 2, now);
        }
        self.prompt.phone_queued = input.phone.is_some();
        // A dropped slot is drawn once in state 6 at the drop (0x4DAB80 →
        // 0x4C9030(0)), before this update's reset.
        let sms_kind = input.sms.map_or(8, |m| m.kind);
        for mut slot in std::mem::take(&mut self.prompt.dropped) {
            slot_draw(s, &mut slot, sms_kind, now);
        }
        // 0x4DA9A0: each frame the D-pad takes texture 0xEA570722 and the
        // highlights, the mode icon, the mail and its count are hidden.
        // With its mask on (0, 2): every slot steps and draws in slot order
        // (0x4C9030: 0x4C8AC0, then by kind 0x4C8DD0 / 0x4ABAF0 / 0x4ABA30 /
        // 0x4C8B80; a later slot's draw overrides an earlier one's), then
        // the first slot in state 2 or 3 is the active one and the pending
        // pick (0x4C90B0) the first of the highest priority (0x4ABF20:
        // markers 4, encounter 9 / 10 3, SMS 2, 0x14 1) among the used
        // slots. No active: the pending one, unless resting (4), starts
        // flashing (state 2, now, its sound) and steps and draws again;
        // none at all: the reset again. Active: a pending one of a higher
        // priority takes over — the active one rests (SMS, state 4) or ends
        // (state 6, drawn, cleared) — and starts flashing; else the active
        // one steps and draws again.
        if hi & 0x2 != 0 {
            self.prompt.update(s, sms_kind, input.script.is_some(), now);
        }
        // 0x4DB8C0, the SMS element (HUD +0x7C; the HUD's update runs it
        // after the prompt's, 0x4EC673 / 0x4EC6CD, whatever the mask): no
        // unread message drops the SMS slot (0x4DAB80(0x13)) and forgets
        // the count; more unread than it last saw queues one with the
        // count (0x4C9170(0x13, n)).
        if let Some(sms) = input.sms {
            if sms.unread == 0 {
                self.prompt.drop_kind(0x13);
                self.sms_last = 0;
            } else if self.sms_last < sms.unread {
                self.prompt.queue(0x13, sms.unread, now);
                self.sms_last = sms.unread;
            }
        }
    }
}
