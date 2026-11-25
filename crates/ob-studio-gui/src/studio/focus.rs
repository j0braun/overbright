//! Focus inside an area (docs/modding/studio.md, "Keyboard and pad"): the shell's own controls, in reading order,
//! as the keys and the pad reach them. Each area keeps the control it was on; Tab, the arrows and the D-pad move
//! between them, Tab and Shift+Tab on past an area's ends into the next or the previous area.
//!
//! The controls are registered while they are drawn, so a control that is not drawn (no room, folded) is never
//! reached; the keys of a frame act on the controls of the frame before. The focused one wears the launcher's focus:
//! the selection fill (`sel`) under it and the lime ring around it, the same ring as Home's, the Mod page's and the
//! Changes page's rows.

use egui::{Painter, Rect, Stroke, StrokeKind};

use crate::editor::theme::{rgb, tokens};

use super::nav::Area;
use super::workspace::Workspace;

/// The right dock's tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RightTab {
    Inspector,
    History,
    Help,
}

/// One of the shell's own controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spot {
    /// A workspace on the rail.
    Rail(Workspace),
    /// The menu bar's ← and →, the address and Find.
    Back,
    Forward,
    Address,
    Find,
    /// The toolbar's main action.
    Test,
    /// A row of the left panel's mod tree.
    Tree(usize),
    RightTab(RightTab),
    /// A row of the right dock's body.
    RightRow(usize),
    /// The bottom dock's tabs (Problems, Log, Jobs, then the workspace's) and its rows.
    BottomTab(usize),
    BottomRow(usize),
}

impl Spot {
    /// A tab: ◀ ▶ (← →) change it.
    pub fn is_tab(self) -> bool {
        matches!(self, Spot::RightTab(_) | Spot::BottomTab(_))
    }
}

fn slot(a: Area) -> usize {
    Area::ALL.iter().position(|x| *x == a).unwrap_or(0)
}

/// The controls of the shell's areas and where each area's focus is.
#[derive(Clone, Debug, Default)]
pub struct Spots {
    /// The last frame's controls (the keys act on these) and this frame's, as they are drawn.
    last: Vec<(Area, Spot, Rect)>,
    now: Vec<(Area, Spot, Rect)>,
    /// The focused control of each area: an index into its controls.
    at: [usize; 7],
}

impl Spots {
    /// A frame starts: its controls are registered as they are drawn.
    pub fn begin(&mut self) {
        self.now.clear();
    }

    /// The frame is drawn: its controls are the ones the next keys act on.
    pub fn end(&mut self) {
        std::mem::swap(&mut self.last, &mut self.now);
        for a in Area::ALL {
            let n = self.count(a);
            let i = &mut self.at[slot(a)];
            *i = (*i).min(n.saturating_sub(1));
        }
    }

    /// Register `s` of area `a` drawn at `r`; true when it is the area's focused control.
    pub fn add(&mut self, a: Area, s: Spot, r: Rect) -> bool {
        let i = self.now.iter().filter(|x| x.0 == a).count();
        self.now.push((a, s, r));
        i == self.at[slot(a)]
    }

    /// The controls of area `a` (as last drawn), in reading order.
    pub fn of(&self, a: Area) -> Vec<Spot> {
        self.last.iter().filter(|x| x.0 == a).map(|x| x.1).collect()
    }

    pub fn count(&self, a: Area) -> usize {
        self.last.iter().filter(|x| x.0 == a).count()
    }

    pub fn index(&self, a: Area) -> usize {
        self.at[slot(a)]
    }

    /// The focused control of area `a`.
    pub fn focused(&self, a: Area) -> Option<Spot> {
        self.of(a).get(self.index(a)).copied()
    }

    /// Where the focused control of area `a` was drawn (this frame's, else the last frame's).
    pub fn focused_rect(&self, a: Area) -> Option<Rect> {
        let i = self.index(a);
        let pick = |v: &[(Area, Spot, Rect)]| v.iter().filter(|x| x.0 == a).nth(i).map(|x| x.2);
        pick(&self.now).or_else(|| pick(&self.last))
    }

    /// Focus the `i`-th control of area `a` (`usize::MAX`: its last; kept within its controls as it is drawn).
    pub fn set(&mut self, a: Area, i: usize) {
        self.at[slot(a)] = if self.count(a) > 0 { i.min(self.count(a) - 1) } else { i };
    }

    /// Focus `s` in area `a`, when it is there.
    pub fn focus(&mut self, a: Area, s: Spot) -> bool {
        match self.of(a).iter().position(|x| *x == s) {
            Some(i) => {
                self.at[slot(a)] = i;
                true
            }
            None => false,
        }
    }

    /// One control on (`d` = 1) or back (-1) in area `a`; false (nothing moved) past its ends.
    pub fn step(&mut self, a: Area, d: i32) -> bool {
        let n = self.count(a) as i32;
        let j = self.index(a) as i32 + d;
        if n == 0 || j < 0 || j >= n {
            return false;
        }
        self.at[slot(a)] = j as usize;
        true
    }
}

/// The focused row's selection fill (draw it before the row's words).
pub fn fill(p: &Painter, r: Rect) {
    p.rect_filled(r, 6.0, rgb(tokens().sel));
}

/// The focus ring around a focused control (draw it after the control).
pub fn ring(p: &Painter, r: Rect) {
    p.rect_stroke(r, 6.0, Stroke::new(2.0, rgb(tokens().lime)), StrokeKind::Inside);
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};

    fn r(y: f32) -> Rect {
        Rect::from_min_size(pos2(0.0, y), vec2(10.0, 10.0))
    }

    #[test]
    fn steps_stay_inside_an_area_and_say_when_they_run_out() {
        let mut s = Spots::default();
        s.begin();
        assert!(s.add(Area::Rail, Spot::Rail(Workspace::Home), r(0.0)), "the first is focused at first");
        assert!(!s.add(Area::Rail, Spot::Rail(Workspace::World), r(10.0)));
        s.add(Area::Address, Spot::Back, r(0.0));
        s.end();
        assert_eq!(s.of(Area::Rail), [Spot::Rail(Workspace::Home), Spot::Rail(Workspace::World)]);
        assert!(!s.step(Area::Rail, -1));
        assert!(s.step(Area::Rail, 1));
        assert_eq!(s.focused(Area::Rail), Some(Spot::Rail(Workspace::World)));
        assert!(!s.step(Area::Rail, 1), "past the end");
        assert_eq!(s.focused(Area::Address), Some(Spot::Back), "each area keeps its own");
        assert!(!s.step(Area::View, 1), "an area without controls");
        // A control that is no longer drawn: the focus stays on one that is.
        s.begin();
        s.add(Area::Rail, Spot::Rail(Workspace::Home), r(0.0));
        s.end();
        assert_eq!(s.focused(Area::Rail), Some(Spot::Rail(Workspace::Home)));
    }
}
