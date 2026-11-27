/// One trick popup (0x4C7E20 builds it from a trick, 0x2C bytes).
#[derive(Clone, Debug)]
pub(super) struct Popup {
    /// +0x08: the table entry's continuous byte (+6).
    pub(super) continuous: bool,
    /// +0x0C: the run's points; +0x10 the multiplier rounded (trunc(m +
    /// 0.5), 0x784260); +0x14 the table index; +0x1C its name label.
    pub(super) points: f32,
    pub(super) count: i32,
    pub(super) index: usize,
    pub(super) name: u32,
    /// +0x18: when it was (last) put on screen, ms; None for the 0 that
    /// 0x4C7E20 and each refresh (0x4C8A6D) store.
    pub(super) shown_at: Option<f32>,
    /// +0x20: put on screen once (its "animate" ran).
    pub(super) shown: bool,
    /// +0x24: the trick's state (0..4) at the last refresh, 5 once faded
    /// out; None before any refresh (0x4C7E20 does not write it).
    pub(super) state: Option<u8>,
    /// +0x28: the trick was lost (state 4).
    pub(super) lost: bool,
}

/// The style element's popup queue and what 0x4AB780 last wrote to
/// SP_Trick_Type / SP_Trick_Point (FEng keeps an object's text, base
/// colour and visibility until they are written again).
#[derive(Clone, Debug, Default)]
pub(super) struct Popups {
    pub(super) queue: std::collections::VecDeque<Popup>,
    /// +0x7A: set by every 0x4DA710 that found the queue not empty (the
    /// constructor clears it; nothing else writes it).
    pub(super) seen: bool,
    /// The texts and base colour (ARGB) last written.
    pub(super) point_text: String,
    pub(super) type_text: String,
    /// The objects' own visibility (0x50CA50 / 0x50CA00): [point, type].
    pub(super) visible: [bool; 2],
    /// The element was on last frame (0x4C6690 shows every object of an
    /// element when its mask turns on).
    pub(super) on: bool,
    /// The freed popups' 0x2C blocks, the last freed on top, each with the
    /// +0x24 it had (5 after a fade-out). 0x4C7E20 never writes +0x24 and
    /// the CRT heap (0x75C6C8) hands a freed block straight to the next
    /// popup: under Wine (70 constructions in a
    /// career circuit) one block was built 40 times and 15 of 70 new popups
    /// started with 5, inherited from the faded popup that last used the
    /// block. A new popup takes the top block's value (`queue`).
    // prov: inferred frontend.popup_heap
    /// The LIFO model of the allocator is **inferred** from that reuse
    /// pattern (the CRT allocator is not traced); a block another
    /// allocation took in between (the observed garbage values) acts as
    /// "not 5", as a never-used one (None) does.
    pub(super) freed: Vec<Option<u8>>,
}

/// The "DRIFT" label (0x4AB78C): the drift trick's popup shows its points
/// without the multiplier.
pub(super) const DRIFT_LABEL: u32 = crate::frontend::fe_ids::DRIFT_LABEL;

impl Popups {
    /// A popup dropped (0x575640 frees its block): the block goes on top of
    /// `freed` with its +0x24.
    pub(super) fn free(&mut self, at: usize) {
        if let Some(p) = self.queue.remove(at) {
            // A bound for the clone's list only (the heap has no such cap).
            if self.freed.len() == 64 {
                self.freed.remove(0);
            }
            self.freed.push(p.state);
        }
    }

    /// 0x4C88E0 outside drift (in drift the drift trick goes to the drift
    /// element instead and 0x4DA710 never runs, so the clone queues
    /// nothing there): each reported trick refreshes the queued popup of
    /// its index when its points are not 0 (+0x28 lost, +0x24 state, +0x0C
    /// points, +0x10 the rounded multiplier unless banked, +0x18 = 0), or,
    /// with points not 0, a new popup goes to the back (to the front when
    /// the points are negative).
    pub(super) fn queue(&mut self, tricks: &[crate::drift::TrickReport]) {
        use crate::drift::TrickState;
        let code = |s: TrickState| match s {
            TrickState::Running => 0,
            TrickState::Pending => 1,
            TrickState::Idle => 2,
            TrickState::Banked => 3,
            TrickState::Lost => 4,
        };
        for t in tricks {
            if t.points == 0.0 {
                continue;
            }
            let count = (t.multiplier + 0.5) as i32;
            if let Some(p) = self.queue.iter_mut().find(|p| p.index == t.index) {
                p.lost = t.state == TrickState::Lost;
                p.state = Some(code(t.state));
                p.points = t.points;
                if t.state != TrickState::Banked {
                    p.count = count;
                }
                p.shown_at = None;
                continue;
            }
            let p = Popup {
                continuous: t.continuous,
                points: t.points,
                count,
                index: t.index,
                name: t.name,
                shown_at: None,
                shown: false,
                // +0x24 as the reused block left it (`freed`).
                state: self.freed.pop().flatten(),
                lost: t.state == TrickState::Lost,
            };
            if t.points < 0.0 {
                self.queue.push_front(p);
            } else {
                self.queue.push_back(p);
            }
        }
    }
}
