//! The graphics shop's pieces (UI_ChoosePaintCategory, 0x55A410 →
//! 0x5571C0): the pending "graphics setup" its screens fill and
//! "Purchase Setup" buys, the vinyl layers and groups, and the decal
//! locations. See docs/formats.md, "Graphics shop".

pub mod graphics_shop;
pub mod setup;

pub use graphics_shop::*;
pub use setup::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_slots_follow_the_layers() {
        assert_eq!(vinyl_colour_slot(0x40, 1), 0x4D);
        assert_eq!(vinyl_colour_slot(0x40, 4), 0x50);
        assert_eq!(vinyl_colour_slot(0x41, 1), 0x51);
        assert_eq!(vinyl_colour_slot(0x43, 4), 0x5C);
        assert!(is_layer_colour(0x42, 0x55) && !is_layer_colour(0x42, 0x54));
    }

    #[test]
    fn a_vinyl_replaces_its_layer_and_colours() {
        let mut s = Setup::default();
        s.put_vinyl(0x40, 1, false, false);
        s.put_colour(0x4D, 7);
        s.put_vinyl(0x41, 2, false, false);
        s.put_vinyl(0x40, 3, false, false);
        assert_eq!(s.items, vec![(0x41, 2), (0x40, 3)]);
        // A unique vinyl clears every layer.
        s.put_vinyl(0x42, 4, true, false);
        assert_eq!(s.items, vec![(0x42, 4)]);
        // Removing with a unique one fitted takes the fitted layers off.
        s.remove_vinyl(0x42, true, false, &|l| l == 0x40 || l == 0x43);
        assert_eq!(s.items, vec![(0x40, 0), (0x43, 0)]);
    }

    #[test]
    fn layers_swap_with_their_colours() {
        let mut s = Setup::default();
        s.put_vinyl(0x41, 5, false, false);
        s.put_colour(0x52, 9);
        // Layer 0x40 has a fitted vinyl with a colour.
        let fitted = |slot: u8| match slot {
            0x40 => Some(1),
            0x4E => Some(2),
            _ => None,
        };
        s.swap_layers(0x41, 0x40, &fitted);
        assert_eq!(s.items, vec![(0x40, 5), (0x4E, 9), (0x41, 1), (0x52, 2)]);
        // An empty layer swaps as a removal.
        let mut t = Setup::default();
        t.swap_layers(0x40, 0x41, &fitted);
        assert_eq!(t.items, vec![(0x41, 1), (0x52, 2), (0x40, 0)]);
    }

    #[test]
    fn cancel_and_cost_by_kind() {
        let mut s = Setup::default();
        s.put(0x3F, 10);
        s.put(0x39, 11);
        s.put(0x89, 12);
        s.put_vinyl(0x40, 13, false, false);
        let price = |slot: u8, _: u32| i32::from(slot);
        // The location part (0x39) is free.
        assert_eq!(s.cost(&price), 0x3F + 0x89 + 0x40);
        s.cancel(1);
        assert_eq!(s.items, vec![(0x3F, 10), (0x40, 13)]);
        s.cancel(2);
        s.cancel(0);
        assert!(s.is_empty());
    }

    #[test]
    fn decal_tree() {
        let top = decal_options(0, false);
        assert_eq!(top.len(), 6);
        assert_eq!(top[0].item, DecalItem::Slot(0x69));
        assert_eq!(top[2].item, DecalItem::Node(0x10001));
        assert_eq!(decal_options(0x10001, false).len(), 6);
        assert_eq!(decal_options(0x10003, true).len(), 1);
        assert_eq!(
            decal_options(0x20007, false).iter().map(|o| o.item).collect::<Vec<_>>(),
            vec![DecalItem::Slot(0x89), DecalItem::Slot(0x8A)]
        );
        assert_eq!(decal_parent(0x20009), Some(0x10004));
        assert_eq!(decal_layout(0x2000B), Some((0x34, 0x61, 2)));
    }
}
