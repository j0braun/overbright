//! The career start and the D-Day intro.

use super::*;

/// The D-Day intro's events (bin_hash "DDAY_EVENT_A", "DDAY_EVENT_B").
pub const DDAY_EVENT_A: u32 = 0xDD60_E402;
pub const DDAY_EVENT_B: u32 = 0xDD60_E403;

/// The preset the player drives until the car lot gives them a car
/// (0x637280 with "DDAY_PLAYER_CAR", as 0x4DF140 shows it).
pub const DDAY_PLAYER_CAR: &str = "DDAY_PLAYER_CAR";

impl Profile {
    /// The career's start (0x495A30): with DDAY_EVENT_B won, the garage;
    /// otherwise the D-Day intro goes on with event A, or B once A is won
    /// (0x5410F0 makes it the current event).
    pub fn start_career(&mut self, data: &CareerData) {
        self.intro = if self.won(data, DDAY_EVENT_B) {
            None
        } else if self.won(data, DDAY_EVENT_A) {
            Some(DDAY_EVENT_B)
        } else {
            Some(DDAY_EVENT_A)
        };
    }

    /// The car is driven into a shop (0x54CB00): when the intro's event
    /// has that shop as a goal (0x4FFDE0) the event is won (0x541430: no
    /// purse, no reputation, not filed in the stage's wins) and A hands on
    /// to B, B ends the intro. Returns whether a goal was reached.
    pub fn enter_shop(&mut self, data: &CareerData, root: &Path, shop_hash: u32, rng: &mut crate::opponents::GameRng) -> bool {
        let Some(event) = self.intro.and_then(|h| data.events.iter().find(|e| e.unlock == h)) else {
            return false;
        };
        if !event.goals.iter().any(|&(kind, h)| kind == 4 && h == shop_hash) {
            return false;
        }
        let _ = root;
        // 0x54CB00 → 0x541430: the intro's win sends its messages too
        // (0x529930), then the stage end check (0x532C70).
        self.completed.insert(event.index);
        self.sms_after_win(data, event);
        self.intro = if event.unlock == DDAY_EVENT_A { Some(DDAY_EVENT_B) } else { None };
        self.check_stage(data, event.stage);
        self.refresh_open(data, rng);
        true
    }

    /// Play the D-Day intro through its rules: the car lot's goal, the
    /// stock `car` into the free slot, the garage's goal (tests and
    /// screenshots start from the end of it).
    pub fn skip_intro(&mut self, data: &CareerData, root: &Path, car: &str, rng: &mut crate::opponents::GameRng) {
        self.start_career(data);
        self.intro_played = true;
        while let Some(event) = self.intro.and_then(|h| data.events.iter().find(|e| e.unlock == h)).cloned() {
            let Some(&(_, shop)) = event.goals.iter().find(|g| g.0 == 4) else {
                break;
            };
            if !self.enter_shop(data, root, shop, rng) {
                break;
            }
            if let Some(i) = self.free_slot() {
                self.take_stock_car(i, car);
            }
        }
    }

    /// Saves from before the clone had the intro and car slots: a career
    /// past stage 0 has done the intro, and its car is in the first slot.
    pub fn upgrade_save(&mut self, data: &CareerData) {
        if self.stage >= 1 {
            for e in data.events.iter().filter(|e| e.unlock == DDAY_EVENT_A || e.unlock == DDAY_EVENT_B) {
                self.completed.insert(e.index);
            }
        }
        if self.slots.len() != CAR_SLOTS {
            self.slots.resize(CAR_SLOTS, CarSlot::default());
        }
        if !self.slots.iter().any(|s| s.unlocked) {
            self.slots[0].unlocked = true;
        }
        if !self.car.is_empty() && self.slots[self.slot].car.is_none() {
            self.stash();
        }
        // Saves the clone wrote between the car lot's goal and taking a
        // car (it used to save at the goal): D-Day A won, B not, and no car
        // of its own — a state the game never saves. A goes on again.
        if self.stage == 0
            && self.won(data, DDAY_EVENT_A)
            && !self.won(data, DDAY_EVENT_B)
            && self.car.is_empty()
            && self.slots.iter().all(|s| s.car.is_none() && !s.opaque)
        {
            for e in data.events.iter().filter(|e| e.unlock == DDAY_EVENT_A) {
                self.completed.remove(&e.index);
            }
        }
    }
}
