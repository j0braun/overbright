use crate::formats::hash::bin_hash;

use super::*;

/// Instance flag set only on `XO_TRACKBARRIER*` props: the barriers that
/// close off streets for a race. Nearly all also carry [`HIDDEN_FLAG`] and
/// show only while a race's scenery group lists them; 0x5DB4C0 mirrors
/// them for a race run backward (see [`World::set_up_barriers`]).
pub const RACE_ONLY_FLAG: u16 = 0x0200;
/// Instance flag 0x10: every view's cull flags have it (0x5B9C30 ors 0x10
/// in), so no view draws such an instance. A scenery group's members carry
/// it until the group is switched on (0x5710B0 clears it, 0x5710F0 sets
/// it).
pub const HIDDEN_FLAG: u16 = 0x0010;

/// The names 0x571170 and 0x578070 switch on (the table at 0x800E00, 3
/// entries of {name prefix, flag}, matched in order by 0x5D7410 with the
/// case-blind compare 0x43DD10 over the prefix's length): the flag says
/// whether the like-named barrier group's collision comes on too. A name
/// matching none switches nothing.
pub const SWITCH_PREFIXES: [(&str, bool); 3] =
    [("BARRIERS_", true), ("PLAYER_BARRIERS_", true), ("FREE_ROAM", false)];

/// What a track load sets the barriers up for (0x578070, called by
/// 0x57A660 once the track is in; the race's track record is 0x883DAC).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BarrierSetup {
    /// The track record's number (+0x8A, i16): the race's track, 4000 in
    /// free roam.
    pub track: i32,
    /// No race (race type 0x89E7B0 = 0): the career's groups are switched.
    pub free_roam: bool,
    /// In a career (game mode 0x850078 = 1).
    pub career: bool,
    /// The career's current stage (0x862838).
    pub stage: u8,
    /// The race runs backward (0x89E7A4 = 1).
    pub reversed: bool,
    /// The career has a car of its own (career +0x873D, 0x86347D: set
    /// when a car slot is made current, 0x541550 / 0x500BD0).
    pub owns_car: bool,
}

impl World {
    /// The groups as a load of the region has them (`World::load` is
    /// one): every barrier group off but `NONE` — the region loader's last
    /// step, 0x57A590, runs 0x5D8360 once the track bundle is in (+8 =
    /// (index == 0); every shipped entry has 1 on disk) — and the scenery,
    /// its groups and the instance table as on disk. The game loads the
    /// region again only when the next race or free roam is in another
    /// region or is a test run (0x5818F9..0x581994: 0x57F220 → RegionLoader
    /// 0x57DC70, else straight to the track loader 0x57F2E0 at 0x5819C5).
    pub fn reset_groups(&mut self) {
        for g in 0..self.barriers.len() {
            self.barriers.set(g, g == 0);
        }
        for (g, on) in self.scenery_groups.iter_mut().zip(&self.disk_groups_on) {
            g.on = *on;
        }
        for i in 0..self.links.len() {
            self.set_link_flags(i, self.disk_link_flags[i]);
        }
        self.held_groups.clear();
        self.switches = [0; 8];
        self.mirror_race_only(false);
    }

    /// Take the barrier switches over from `other`, the same region before
    /// the clone loaded it again (for another car; the game keeps the
    /// region loaded then).
    pub fn copy_barriers_from(&self, other: &World) {
        for g in 0..self.barriers.len().min(other.barriers.len()) {
            self.barriers.set(g, other.barriers.is_on(g as u8));
        }
    }

    /// Set the barriers up for a race or free roam on the loaded region:
    /// the last one's scenery groups off (its end, 0x5815D0 → 0x5711E0),
    /// this one's switched on (0x57A660 → 0x578070) and the race-only
    /// scenery mirrored for a backward race (0x57A660 → 0x5DFD10 →
    /// 0x5DB4C0). The barrier groups' collision is not switched off by
    /// either: a racer's own list does that ([`World::racer_barriers`]).
    ///
    /// 0x578070 first sets the groups `Z1`..`Z19` from the race's stream
    /// sections (0x5DB620); no shipped track has groups of those names, so
    /// that is left out. Then `BARRIERS_<n>` and `PLAYER_BARRIERS_<n>` for
    /// the track's number n (0x571170 each); with no race also, in a
    /// career, `FREE_ROAM` (stage above 0; stage 0 takes
    /// `FREE_ROAM_STAGE0` once the career owns a car, byte 0x86347D, else
    /// `FREE_ROAM_STAGE0A`), and `BARRIERS_CAREER<stage>`
    /// in every mode (stage 5 for game modes 0x200 and 8 with 0x850088 =
    /// 8, not single player). The race road's barriers are closed in
    /// [`crate::frontend::career::load_route`].
    pub fn set_up_barriers(&mut self, setup: &BarrierSetup) {
        for g in std::mem::take(&mut self.held_groups) {
            self.show_scenery_group(g, false);
        }
        self.switches = [0; 8];
        let mut held = Vec::new();
        held.extend(self.switch_on(&format!("BARRIERS_{}", setup.track)));
        held.extend(self.switch_on(&format!("PLAYER_BARRIERS_{}", setup.track)));
        if setup.free_roam {
            // 0x57821D..0x57824A: a career's free roam switches FREE_ROAM
            // past stage 0; in stage 0 FREE_ROAM_STAGE0 once the career has
            // a car of its own (0x86347D), FREE_ROAM_STAGE0A before.
            if setup.career {
                let name = match (setup.stage, setup.owns_car) {
                    (0, true) => "FREE_ROAM_STAGE0",
                    (0, false) => "FREE_ROAM_STAGE0A",
                    _ => "FREE_ROAM",
                };
                held.extend(self.switch_on(name));
            }
            held.extend(self.switch_on(&format!("BARRIERS_CAREER{}", setup.stage)));
        }
        self.held_groups = held;
        self.mirror_race_only(setup.reversed);
    }

    /// A racer's own barrier groups (0x5F54B0, from the car set-up 0x604270
    /// for owner kinds 1 and 2): `PLAYER_BARRIERS_<n>` and `BARRIERS_<n>`
    /// for the race road's number n (0x88F2C0: the route record's +0, the
    /// race's track; 0 with no race, 0x5D9A70) and, in career free roam,
    /// `BARRIERS_CAREER<stage>`; each only if the track has it (0x5D8100).
    /// See [`crate::ground::GroundQuery::barriers`].
    pub fn racer_barriers(&self, setup: &BarrierSetup) -> Vec<u8> {
        let road = if setup.free_roam { 0 } else { setup.track };
        let mut names = vec![format!("PLAYER_BARRIERS_{road}"), format!("BARRIERS_{road}")];
        if setup.free_roam && setup.career {
            names.push(format!("BARRIERS_CAREER{}", setup.stage));
        }
        names.iter().filter_map(|n| self.barrier_group(bin_hash(n))).map(|g| g as u8).collect()
    }

    /// The barrier group with this name hash (0x5D3C70: entries 1..).
    fn barrier_group(&self, hash: u32) -> Option<usize> {
        (1..self.barrier_groups.len().min(256)).find(|&g| self.barrier_groups[g].hash == hash)
    }

    /// 0x571170: switch a name on. Unless it starts with one of
    /// [`SWITCH_PREFIXES`] nothing happens; else the like-named barrier
    /// group comes on when the prefix says so, and the like-named scenery
    /// group (found by hash, 0x5DB350) shows its members (0x5710B0). It
    /// first notes the hash in the first free slot of [`World::switches`]
    /// (0x5D7480; with none free it is dropped), which the placed emitters
    /// read (0x5D7450). Returns the scenery group.
    pub fn switch_on(&mut self, name: &str) -> Option<usize> {
        let (_, collision) =
            SWITCH_PREFIXES.iter().find(|(p, _)| name.get(..p.len()).is_some_and(|n| n.eq_ignore_ascii_case(p)))?;
        let hash = bin_hash(name);
        if let Some(slot) = self.switches.iter_mut().find(|h| **h == 0) {
            *slot = hash;
        }
        if *collision && let Some(g) = self.barrier_group(hash) {
            self.barriers.set(g, true);
        }
        let group = self.scenery_groups.iter().position(|g| g.hash == hash)?;
        self.show_scenery_group(group, true);
        Some(group)
    }

    /// 0x5710B0 (`on`) / 0x5710F0: clear or set [`HIDDEN_FLAG`] in each
    /// listed entry of the instance table, which goes to its instance
    /// (0x5DB0F0), and set the group's +0x2C.
    pub fn show_scenery_group(&mut self, group: usize, on: bool) {
        let members = self.scenery_groups[group].members.clone();
        for m in members {
            let m = m as usize;
            let Some(link) = self.links.get(m) else { continue };
            let flags = if on { link.instance_flags & !HIDDEN_FLAG } else { link.instance_flags | HIDDEN_FLAG };
            self.set_link_flags(m, flags);
        }
        self.scenery_groups[group].on = on;
    }

    /// An instance-table entry's flags, copied to its instance (0x5DB0F0).
    fn set_link_flags(&mut self, link: usize, flags: u16) {
        self.links[link].instance_flags = flags;
        self.links[link].flags = flags as u8;
        if let Some(p) = self.link_placements[link]
            && self.placements[p].flags != flags
        {
            self.placements[p].flags = flags;
            self.changed.push(p);
        }
    }

    /// 0x5DB4C0: when the race's direction (0x89E7A4 = 1) differs from the
    /// section's (+9), each instance with [`RACE_ONLY_FLAG`] (+0x1B & 2)
    /// gets its rotation's first row (local x) negated — diag(−1, 1, 1, 1)
    /// × the rotation (0x43C990 → 0x4013F0), the position kept — mirroring
    /// it.
    fn mirror_race_only(&mut self, reversed: bool) {
        if reversed == self.race_only_mirrored {
            return;
        }
        self.race_only_mirrored = reversed;
        for (i, p) in self.placements.iter_mut().enumerate() {
            if p.flags & RACE_ONLY_FLAG != 0 {
                p.transform.x_axis = -p.transform.x_axis;
                self.changed.push(i);
            }
        }
    }

    /// The placement an instance-table (`0x34107`) entry stands for.
    pub fn scenery_placement(&self, link: u16) -> Option<usize> {
        self.link_placements.get(link as usize).copied().flatten()
    }

    /// The placements whose flags or transform changed since the last
    /// call, for the renderer.
    pub fn take_changed(&mut self) -> Vec<usize> {
        let mut changed = std::mem::take(&mut self.changed);
        changed.sort_unstable();
        changed.dedup();
        changed
    }
}
