/// What a marker trigger stands for: the marker table entry 0x527810
/// builds (+4 an event, +8 a shop, +0xC a milestone, +0x10 a world
/// challenge, 0x512A10).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MarkerTarget {
    Event(usize),
    Shop(usize),
    /// A milestone's photo shoot, by its hash.
    Shoot(u32),
    /// A world challenge's start, an index into `CareerData::mag_triggers`.
    Challenge(usize),
}

/// The running world challenge (career +0xC00 = 0x85B940; +0xC08, the
/// record, is 0x85B948).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WorldChallenge {
    /// +8: the record, an index into `CareerData::mag_triggers`.
    pub(crate) index: usize,
    /// +4: the time left, s (the record's +0x10 at the start).
    pub(crate) time: f32,
    /// +0x11C: 1 the clock runs (0x5016A0), 2 the goal shop reached
    /// (0x514FD0), 3 the goal milestone's spot reached (0x532B20), 4 in
    /// the goal shop (0x54CB00), 6 out of time (0x501600), 7 given up
    /// (0x501640), 8 retried (0x5150D0).
    pub(crate) state: u8,
    /// +0: the time ran out (0x501600); 0x498D00 then opens the retry.
    pub(crate) failed: bool,
    /// +0x118: the car's heading at the start, where a retry puts it back.
    pub(crate) heading: f32,
}
