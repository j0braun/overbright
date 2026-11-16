/// What a graphics-shop dialog button does (the screens' message
/// handlers: 0x562C60, 0x56AB10, 0x563420, 0x56B3C0).
#[derive(Clone, Copy, Debug)]
pub(crate) enum SetupStep {
    /// Put a paint in the setup ("Add this paint to your current
    /// selection?", 0xD05FC3A3 → 0x53B0D0) and stay on its swatch.
    Paint { slot: usize, finish: u32, part_slot: u8, hash: u32, selected: usize },
    /// A vinyl colour: with one colour into the setup (0x5474E0) and on
    /// to the layers (0x55BD70); with more it waits for Back (0x53C170).
    VinylColour { colour: usize, slot: u8, hash: u32 },
    /// A vinyl from the browser into the setup (Yes 0x51123592 →
    /// 0x5670F9), then its colours or the layers.
    Vinyl { layer: u8, hash: u32 },
    /// Change: the vinyl browser for the layer (0x51123592).
    ChangeVinyl(u8),
    /// Remove: take the layer's vinyl off (0xE75749D4 → 0x547800).
    RemoveVinyl(u8),
    /// Buy the setup (0x547580); then leave the shop, or stay.
    Purchase { leave: bool },
    /// Drop the setup and leave (0x562F5DAC).
    Discard,
    /// Cancel Changes of a kind (0xE75749D4 → 0x561A50).
    Cancel(u8),
    /// A decal layout (0xD05FC3A3 → 0x55FEA0), then into it.
    Layout(u32),
    /// Take every decal of a node off (0xE75749D4 → 0x55FF80).
    RemoveDecals(u32),
    /// Leave the shop: with a setup, ask to buy it first (0x51123592).
    Exit,
}
