//! Pieces of [`Hud::quads`], called in its order (see there).

use super::*;

impl Hud {
    /// 0x4A9EF0 / 0x4A9FF0: the bank and Cash_Reward.
    pub(super) fn bank_and_cash_reward(&mut self, f: &QuadsFrame<'_>) {
        let QuadsFrame { assets, input, hi, now, .. } = *f;
        let s = &mut self.state;
        let hide = |s: &mut ScreenState, name: &str| {
            s.hidden.insert(fe_hash(name));
        };
        let text = |s: &mut ScreenState, name: &str, t: String| {
            s.text.insert(fe_hash(name), t);
        };
        // 0x4A9EF0: the bank; while a world challenge's clock runs
        // (0x4A9F0F: 0x501680, challenge state 1) Cash_Bank, Cash_data and
        // Cash_header are hidden (0x50CA00), as with its mask off.
        if hi & 0x8_0000 != 0 && input.challenge.is_none() {
            // A pickup waiting (+0x28): Cash_Bank prints "%s %$d" with
            // 0x5D10F6AF ("Bank Reward") and the cash, runs Cash_Animate
            // (0x537DC0) and the amount clears.
            if self.bank_pending != 0 {
                self.bank_text = Some(format!(
                    "{} {}",
                    assets.strings.get(crate::outrun::BANK_REWARD).unwrap_or(""),
                    grouped_int(self.bank_pending as i32)
                ));
                s.run(fe_hash("Cash_Bank"), "Cash_Animate", now);
                self.bank_pending = 0;
            }
            if let Some(t) = &self.bank_text {
                text(s, "Cash_Bank", t.clone());
            }
            text(s, "Cash_data", grouped(input.cash));
        } else {
            for n in ["Cash_Bank", "Cash_data", "Cash_header"] {
                hide(s, n);
            }
        }
        // 0x4A9FF0 (element 0x4F5060, HUD +0x78, "Cash_Reward", high
        // 0x80): a popup due prints "%s%s: %$d" — "Bank Reward for
        // Magazine" (0xABA1E56F, kinds 1 and 3) or "for DVD" (0x2AB10D61,
        // kinds 2 and 4), " " in French (language 1, 0x4FF970) else "", and
        // the cash — shows it and runs ZoomIn. A kind outside 1–4 skips
        // only the text (0x4AA05C → 0x4AA0A4): the object is still shown
        // (0x50CA50) with its old text and ZoomIn (0x790968) still runs
        // (read in assembly).
        if let Some((kind, cash)) = input.cash_reward.filter(|_| hi & 0x80 != 0) {
            let label = match kind {
                1 | 3 => Some(0xABA1_E56F),
                2 | 4 => Some(0x2AB1_0D61),
                _ => None,
            };
            if let Some(l) = label {
                self.cash_reward = Some(format!("{}: {}", assets.strings.get(l).unwrap_or(""), grouped_int(cash)));
            }
            s.run(fe_hash("Cash_Reward"), "ZoomIn", now);
        }
        if let Some(t) = &self.cash_reward {
            text(s, "Cash_Reward", t.clone());
        }
    }
}
