use crate::*;

impl Game {
    pub(super) fn paint_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::Paint(p)) = &mut self.mode else { unreachable!() };
        let (slot, finish, vinyl, selected) = (p.slot, p.finish, p.vinyl, p.selected);
        let slots = p.slots();
        let part_slot = slots[slot.min(slots.len() - 1)].0;
        let key_result = p.key(key);
        // The slot strip's arrow that way blinks (0x52FA80).
        if selected.is_none() && p.selected.is_none() && p.slot != slot {
            p.arrows.step(p.slot > slot, now, false);
        }
        let (paint_arrows, paint_opened) = (p.arrows, p.opened);
        match key_result {
            // Back from the swatches to the slots: FadeOut (0x55CF35).
            screens::PaintKey::None if selected.is_some() && p.selected.is_none() => p.swatch_fade = Some((false, now)),
            screens::PaintKey::None => {}
            // Back (0x50E4E0): the vinyl layers from a vinyl's
            // colours, the graphics shop from the paints. Leaving
            // the colours puts the chosen ones in the setup
            // (0x911AB364 → 0x5638F1: 0x5474E0 each, 0x55BE00).
            screens::PaintKey::Leave => {
                self.mode = match vinyl {
                    Some((layer, _)) => {
                        for (slot, hash) in std::mem::take(&mut self.shop.vinyl_pending) {
                            self.shop.setup.put_colour(slot, hash);
                        }
                        self.shop.vinyl = None;
                        self.vinyl_layer_menu((layer - 0x40) as usize)
                    }
                    None => self.graphics_menu(0),
                };
            }
            screens::PaintKey::Refill => {
                let (slot, finish, selected, arrows) = (p.slot, p.finish, p.selected, p.arrows);
                self.mode = self.paint_shop_in(vinyl, slot, Some(finish), selected);
                // The strip's blink carries on (absolute times).
                if let Mode::Menu(Menu::Paint(p)) = &mut self.mode {
                    p.arrows = arrows;
                }
            }
            // A slot (0x406415E3): its swatches when it can be
            // painted (0x50F310), else the reason.
            screens::PaintKey::Enter => match (vinyl, self.unpaintable(part_slot)) {
                (None, Some(reason)) => {
                    let text = self.label(reason, "");
                    let back = self.paint_shop_in(None, slot, Some(finish), None);
                    self.mode = self.dialog(&text, back);
                }
                _ => {
                    // UI_Paint's slot entry (0x55CC60) sets +0x18C,
                    // so its fill (0x548640) takes the finish
                    // (+0x174) from the slot's part — the setup's
                    // item (0x8389C8), else the fitted one
                    // (0x610300) — by its brand 0xEBB03E66, and
                    // opens on that part; then Base Paint with no
                    // swatches, or a spoiler paint not Gloss, refill
                    // on Gloss. Under Wine the finished-career save's red opened on
                    // Metallic with the car still red.
                    if vinyl.is_none() {
                        let part = self.effective(part_slot);
                        let own = part.and_then(|h| self.part_attr(part_slot, h, 0xEBB0_3E66)).unwrap_or(finish);
                        let (arrows, opened) = (paint_arrows, paint_opened);
                        let mut m = self.paint_shop_in(None, slot, Some(own), None);
                        let empty = matches!(&m, Mode::Menu(Menu::Paint(q)) if q.swatches.is_empty());
                        if (part_slot == 0x3F && empty) || (part_slot == 0x45 && own != screens::GLOSS) {
                            m = self.paint_shop_in(None, slot, Some(screens::GLOSS), None);
                        }
                        if let Mode::Menu(Menu::Paint(q)) = &mut m {
                            q.arrows = arrows;
                            q.opened = opened;
                        }
                        self.mode = m;
                    }
                    if let Mode::Menu(Menu::Paint(p)) = &mut self.mode
                        && !p.swatches.is_empty()
                    {
                        p.selected = Some(p.swatches.iter().position(|s| s.4).unwrap_or(0));
                        // FadeIn on the swatch panel (0x55CD45).
                        p.swatch_fade = Some((true, now));
                    }
                }
            },
            // A swatch (0x563420): a question first; in a career
            // the price and the setup must be within the bank
            // ("You do not have enough bank…", 0xD1AC60BE).
            screens::PaintKey::Install(i) => {
                let hash = p.swatches[i].0;
                // The bank check is a career's only (0x850078 = 1 at
                // 0x563799).
                let cash = self.profile.as_ref().filter(|p| !p.scratch).map(|p| p.cash as i32);
                let total = self.graphics_total().unwrap_or(0);
                let back = |g: &Game| g.paint_shop_in(vinyl, slot, Some(finish), selected);
                let (yes, no) = self.yes_no();
                let (question, step, cost) = match vinyl {
                    // "Choose this vinyl color?" (0x2A2C6AEF) with
                    // more than one colour, else "Would you like
                    // to add this vinyl to your setup?" (0x31FBC167,
                    // checked against the bank: the vinyl's price
                    // and the setup's).
                    Some((layer, n)) => {
                        let v = self.shop.vinyl.unwrap_or(0);
                        let cost = (n < 2).then(|| self.setup_price(layer, v) + total);
                        (
                            if n < 2 { 0x31FB_C167 } else { 0x2A2C_6AEF },
                            SetupStep::VinylColour { colour: slot, slot: part_slot, hash },
                            cost,
                        )
                    }
                    // "Add this paint to your current selection?"
                    // (0xC4C5D271).
                    None => (
                        0xC4C5_D271,
                        SetupStep::Paint { slot, finish, part_slot, hash, selected: i },
                        Some(self.setup_price(part_slot, hash) + total),
                    ),
                };
                if let (Some(cash), Some(cost)) = (cash, cost)
                    && cash < cost
                {
                    let text = self.label(0xD1AC_60BE, "You do not have enough bank to purchase this item.");
                    self.mode = self.dialog(&text, back(self));
                    // Over UI_Paint, then 0x53BD30 (0x5637C8):
                    // its DIM as under the question.
                    if let Mode::Menu(Menu::Dialog(d, outcomes)) = &mut self.mode {
                        d.under = outcomes.iter().position(|m| matches!(m, Mode::Menu(Menu::Paint(_))));
                        d.dim_under = Some(("UI_Paint", screens::DIM_PAINT_SLOT));
                    }
                    return Ok(());
                }
                let (a, b) = (back(self), back(self));
                // GenericDialog_SMALL.fng (0x78FA24 at 0x5637F3 and
                // the paints' 0x55D4DF).
                self.mode = self.setup_dialog_in(
                    Some("GenericDialog_SMALL"),
                    question,
                    vec![(yes, Some(step), a), (no, None, b)],
                );
                // 0x53BD30 after the ask (0x563803, the paints'
                // 0x55D420): UI_Paint's StartDim and, in a slot,
                // DIM on its group 0xDB285767.
                if let Mode::Menu(Menu::Dialog(d, _)) = &mut self.mode {
                    d.dim_under = Some(("UI_Paint", screens::DIM_PAINT_SLOT));
                }
                // No focused (0x5587A0's 1 at 0x5637D8; under Wine).
                if let Mode::Menu(Menu::Dialog(d, _)) = &mut self.mode {
                    d.selected = 1;
                }
            }
        }
        Ok(())
    }
}
