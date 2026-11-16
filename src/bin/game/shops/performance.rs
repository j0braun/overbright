/// What Yes does on a performance shop question.
#[derive(Clone, Copy, Debug)]
pub(crate) enum PerfPending {
    /// A part (id), bought first unless owned (0x9F00F07 / 0x51123592).
    Part(u8, bool),
    /// A package (level), bought first unless owned (0x5501F0).
    Package(u8, bool),
    /// The category back to stock (0xE75749D4 → 0x521A30).
    Stock,
}
