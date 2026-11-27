use super::graphics_shop::*;

/// The pending graphics setup (the list at the shop state +0x48, head
/// 0x8389C8): what the graphics screens chose, previewed on the car and
/// bought together by "Purchase Setup" (0x547580). An item is a slot and
/// the part to fit there, 0 to take its part off.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Setup {
    pub items: Vec<(u8, u32)>,
}

impl Setup {
    /// The setup's part for a slot, if it has one (0x5214E0: the first
    /// item for the slot; 0 is a removal).
    pub fn get(&self, slot: u8) -> Option<u32> {
        self.items.iter().find(|i| i.0 == slot).map(|i| i.1)
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Put a part on a slot (0x53AF30, the paints, decals and layouts):
    /// the slot's items go and the new one goes on the end.
    pub fn put(&mut self, slot: u8, part: u32) {
        self.items.retain(|i| i.0 != slot);
        self.items.push((slot, part));
    }

    /// A vinyl colour (0x5474E0; slots 0x4D–0x5C only).
    pub fn put_colour(&mut self, slot: u8, part: u32) {
        if part != 0 && (0x4D..=0x5C).contains(&slot) {
            self.put(slot, part);
        }
    }

    /// A layer's items and its colours' go (0x53B0F0).
    pub fn drop_layer(&mut self, layer: u8) {
        if (0x40..=0x43).contains(&layer) {
            self.items.retain(|i| i.0 != layer && !is_layer_colour(layer, i.0));
        }
    }

    /// Every layer's items go (0x547550).
    pub fn drop_vinyls(&mut self) {
        for layer in VINYL_LAYERS {
            self.drop_layer(layer);
        }
    }

    /// Put a vinyl on a layer (0x54FDC0): a unique one, or one while the
    /// setup holds a unique one, clears all the layers first; otherwise
    /// the layer's own items go.
    pub fn put_vinyl(&mut self, layer: u8, part: u32, unique: bool, setup_unique: bool) {
        if part == 0 || !(0x40..=0x43).contains(&layer) {
            return;
        }
        if unique || setup_unique {
            self.drop_vinyls();
        } else {
            self.drop_layer(layer);
        }
        self.items.push((layer, part));
    }

    /// Take a layer's vinyl off (0x547800): with a unique vinyl fitted
    /// (`fitted_unique`) or in the setup (`setup_unique`) all four layers
    /// come off, else the one; each layer that has a vinyl fitted gets a
    /// removal (`fitted` tells which).
    pub fn remove_vinyl(&mut self, layer: u8, fitted_unique: bool, setup_unique: bool, fitted: &dyn Fn(u8) -> bool) {
        if !(0x40..=0x43).contains(&layer) {
            return;
        }
        if fitted_unique || setup_unique {
            self.drop_vinyls();
            for l in VINYL_LAYERS {
                if fitted(l) {
                    self.items.push((l, 0));
                }
            }
        } else {
            self.drop_layer(layer);
            if fitted(layer) {
                self.items.push((layer, 0));
            }
        }
    }

    /// Take a decal off (the decal shop's removal: the slot's item goes,
    /// and a removal stands in for a fitted one).
    pub fn remove(&mut self, slot: u8, fitted: bool) {
        self.items.retain(|i| i.0 != slot);
        if fitted {
            self.items.push((slot, 0));
        }
    }

    /// A layer's items taken out of the setup (0x5216D0): the setup's
    /// for the layer and its colours when it has any (a removal counts);
    /// else the car's vinyl and colours, or a removal without one. (The
    /// game's colour loop stops before 0x5C, the top layer's fourth.)
    fn take_layer(&mut self, layer: u8, fitted: &dyn Fn(u8) -> Option<u32>) -> Vec<(u8, u32)> {
        if self.items.iter().any(|i| i.0 == layer) {
            let (taken, kept) = self.items.iter().partition(|i| i.0 == layer || is_layer_colour(layer, i.0));
            self.items = kept;
            return taken;
        }
        match fitted(layer) {
            Some(v) => {
                let mut out = vec![(layer, v)];
                out.extend(
                    (0x4D..0x5C).filter(|&s| is_layer_colour(layer, s)).filter_map(|s| fitted(s).map(|c| (s, c))),
                );
                out
            }
            None => vec![(layer, 0)],
        }
    }

    /// Swap two layers' vinyls and colours (0x561AE0, the Reorder
    /// Layers triggers): each layer's items (0x5216D0) move to the other
    /// (a colour keeps its index, 0x50EF50) and go on the end.
    pub fn swap_layers(&mut self, a: u8, b: u8, fitted: &dyn Fn(u8) -> Option<u32>) {
        if !(0x40..=0x43).contains(&a) || !(0x40..=0x43).contains(&b) {
            return;
        }
        let retarget = |(slot, part): (u8, u32), to: u8| {
            if (0x40..=0x43).contains(&slot) {
                (to, part)
            } else {
                (vinyl_colour_slot(to, (slot - 0x4D) % 4 + 1), part)
            }
        };
        let from_a = self.take_layer(a, fitted);
        let from_b = self.take_layer(b, fitted);
        self.items.extend(from_a.into_iter().map(|i| retarget(i, b)));
        self.items.extend(from_b.into_iter().map(|i| retarget(i, a)));
    }

    /// "Cancel Changes" (0x561A50) of a kind: 0 the vinyls (0x547550),
    /// 1 the decals and layouts (0x530250: slots 0x61–0x98, 0x34–0x3E),
    /// 2 the paints (0x3F, 0x44–0x4A).
    pub fn cancel(&mut self, kind: u8) {
        match kind {
            0 => self.drop_vinyls(),
            1 => self.items.retain(|i| !is_decal_slot(i.0)),
            2 => self.items.retain(|i| !is_paint_slot(i.0)),
            _ => {}
        }
    }

    /// The setup's cost (0x521540): the price (0x514280) of each item
    /// with a part, outside the free location slots 0x34–0x3E.
    pub fn cost(&self, price: &dyn Fn(u8, u32) -> i32) -> i32 {
        self.items.iter().filter(|i| i.1 != 0 && !(0x34..=0x3E).contains(&i.0)).map(|i| price(i.0, i.1)).sum()
    }

    /// Apply the setup to a car's fitted parts (the preview, and the
    /// purchase 0x547580): each item fits its part, or takes the slot's
    /// off; a vinyl over a fitted unique one first clears all four
    /// layers.
    pub fn apply(&self, installed: &mut std::collections::BTreeMap<u8, u32>, is_unique: &dyn Fn(u32) -> bool) {
        for &(slot, part) in &self.items {
            if (0x40..=0x43).contains(&slot)
                && VINYL_LAYERS.iter().any(|l| installed.get(l).is_some_and(|&h| is_unique(h)))
            {
                for l in VINYL_LAYERS {
                    installed.remove(&l);
                }
            }
            if part == 0 {
                installed.remove(&slot);
            } else {
                installed.insert(slot, part);
            }
        }
    }
}

/// The paint slots bought paints are not marked owned in (0x547580
/// skips 0x3F, 0x44–0x47 and 0x49).
pub fn marks_owned(slot: u8) -> bool {
    !matches!(slot, 0x3F | 0x44 | 0x45 | 0x46 | 0x47 | 0x49)
}
