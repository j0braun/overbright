//! The legacy text profile (career.txt) reader.

use super::*;

impl Profile {
    /// The clone's old text save (career.txt), read once to migrate it.
    pub fn load_text(path: &Path) -> Option<Profile> {
        let text = std::fs::read_to_string(path).ok()?;
        let mut p = Profile::new("Player");
        if text.lines().any(|l| l.starts_with("slot ")) {
            p.slots[0].unlocked = false;
        }
        for line in text.lines() {
            let (key, value) = line.split_once(' ').unwrap_or((line, ""));
            match key {
                "name" => p.name = value.into(),
                "cash" => p.cash = value.parse().unwrap_or(p.cash),
                "car" => p.car = value.into(),
                "stage" => p.stage = value.parse().unwrap_or(0),
                "reputation" => p.reputation = value.parse().unwrap_or(0),
                "won" => {
                    if let Ok(e) = value.parse() {
                        p.completed.insert(e);
                    }
                }
                "sponsor" => {
                    if let Some((st, h)) = value.split_once(' ')
                        && let (Ok(st), Ok(h)) = (st.parse(), h.parse())
                    {
                        p.sponsors.insert(st, h);
                    }
                }
                "open" => p.open.extend(value.parse::<u32>()),
                "milestone" => p.milestones.extend(value.parse::<u32>()),
                "offered" => p.offered.extend(value.parse::<u32>()),
                "visited" => p.visited.extend(value.parse::<u32>()),
                "challengedone" => p.challenges_done.extend(value.parse::<u32>()),
                // The old career generator (gone: the career draws
                // on the game's shared generator).
                "rng" => {}
                "menucar" => p.menu_car = Some((value.into(), BTreeMap::new())),
                "introflags" => {
                    let v: Vec<u8> = value.split(' ').filter_map(|w| w.parse().ok()).collect();
                    if let [closed, races] = v[..] {
                        p.intro_closed = closed != 0;
                        p.intro_races = races != 0;
                    }
                }
                "explore" => {
                    let v: Vec<f32> = value.split(' ').filter_map(|w| w.parse().ok()).collect();
                    if let [set, x, y, z, h] = v[..] {
                        p.explore_set = set != 0.0;
                        p.explore_pos = Some((Vec3::new(x, y, z), h));
                    }
                }
                "menupart" => {
                    if let Some((s, h)) = value.split_once(' ')
                        && let (Ok(s), Ok(h)) = (s.parse::<u8>(), h.parse::<u32>())
                        && let Some((_, installed)) = p.menu_car.as_mut()
                    {
                        installed.insert(s, h);
                    }
                }
                "slot" => {
                    if let Some(s) = value.parse::<usize>().ok().and_then(|i| p.slots.get_mut(i)) {
                        s.unlocked = true;
                    }
                }
                "slotcar" => {
                    if let Some((i, car)) = value.split_once(' ')
                        && let Some(s) = i.parse::<usize>().ok().and_then(|i| p.slots.get_mut(i))
                    {
                        s.car = Some(car.into());
                    }
                }
                "slotpart" | "slotowned" => {
                    let v: Vec<&str> = value.split(' ').collect();
                    if let [i, slot, h] = v[..]
                        && let (Ok(i), Ok(slot), Ok(h)) = (i.parse::<usize>(), slot.parse::<u8>(), h.parse::<u32>())
                        && let Some(s) = p.slots.get_mut(i)
                    {
                        if key == "slotpart" {
                            s.installed.insert(slot, h);
                        } else {
                            s.owned.insert((slot, h));
                        }
                    }
                }
                "slotperf" => {
                    if let Some((i, id)) = value.split_once(' ')
                        && let (Ok(i), Ok(id)) = (i.parse::<usize>(), id.parse::<u8>())
                        && let Some(s) = p.slots.get_mut(i)
                    {
                        s.perf.insert(id);
                    }
                }
                "slotperfunique" => {
                    if let Some((i, id)) = value.split_once(' ')
                        && let (Ok(i), Ok(id)) = (i.parse::<usize>(), id.parse::<u8>())
                        && let Some(s) = p.slots.get_mut(i)
                    {
                        s.perf_uniques.insert(id);
                    }
                }
                "slotperfowned" => {
                    if let Some((i, id)) = value.split_once(' ')
                        && let (Ok(i), Ok(id)) = (i.parse::<usize>(), id.parse::<u8>())
                        && let Some(s) = p.slots.get_mut(i)
                    {
                        s.perf_owned.insert(id);
                    }
                }
                "slotbrand" => {
                    if let Some((i, b)) = value.split_once(' ')
                        && let (Ok(i), Ok(b)) = (i.parse::<usize>(), b.parse::<u32>())
                        && let Some(s) = p.slots.get_mut(i)
                    {
                        s.brands.push(b);
                    }
                }
                "brand" => p.brands.extend(value.parse::<u32>()),
                "current" => p.slot = value.parse::<usize>().unwrap_or(0).min(CAR_SLOTS - 1),
                "intro" => p.intro = value.parse().ok(),
                "sms" => p.inbox.load_line(value),
                "drivetime" => p.drive_time = value.parse().unwrap_or(0.0),
                "entered" => p.races_entered = value.parse().unwrap_or(0),
                "spent" => p.spent = value.parse().unwrap_or(0),
                "slotspent" => {
                    if let Some((i, n)) = value.split_once(' ')
                        && let (Ok(i), Ok(n)) = (i.parse::<usize>(), n.parse::<u32>())
                        && let Some(s) = p.slots.get_mut(i)
                    {
                        s.spent = n;
                    }
                }
                "smscounters" => {
                    let v: Vec<i32> = value.split(' ').filter_map(|s| s.parse().ok()).collect();
                    if let [m, u] = v[..] {
                        p.trigger_counters = crate::frontend::sms::TriggerCounters { magazine: m, unique: u };
                    }
                }
                "perf" => {
                    if let Ok(id) = value.parse() {
                        p.perf.insert(id);
                    }
                }
                "perfowned" => p.perf_owned.extend(value.parse::<u8>()),
                "perfunique" => p.perf_uniques.extend(value.parse::<u8>()),
                // "tuning <mode> <hex>"; an older bare "tuning <hex>" is
                // the first mode's.
                "tuning" => {
                    let (m, hex) = value.split_once(' ').unwrap_or(("0", value));
                    if let (Ok(m), Some(t)) = (m.parse::<usize>(), crate::tuning::Settings::from_hex(hex))
                        && m < TUNING_MODES
                    {
                        p.tuning[m] = t;
                    }
                }
                "slottuning" => {
                    let v: Vec<&str> = value.split(' ').collect();
                    if let [i, m, hex] = v[..]
                        && let (Ok(i), Ok(m), Some(t)) =
                            (i.parse::<usize>(), m.parse::<usize>(), crate::tuning::Settings::from_hex(hex))
                        && m < TUNING_MODES
                        && let Some(s) = p.slots.get_mut(i)
                    {
                        s.tuning[m] = t;
                    }
                }
                "outrunwins" => {
                    if let Some((st, n)) = value.split_once(' ')
                        && let (Ok(st), Ok(n)) = (st.parse(), n.parse())
                    {
                        p.outrun_wins.insert(st, n);
                    }
                }
                "stagerep" => {
                    let v: Vec<&str> = value.split(' ').collect();
                    if let [st, sum, n] = v[..]
                        && let (Ok(st), Ok(sum), Ok(n)) = (st.parse(), sum.parse(), n.parse())
                    {
                        p.stage_rep.insert(st, (sum, n));
                    }
                }
                "owned" | "installed" => {
                    if let Some((slot, h)) = value.split_once(' ')
                        && let (Ok(slot), Ok(h)) = (slot.parse::<u8>(), h.parse::<u32>())
                    {
                        if key == "owned" {
                            p.owned.insert((slot, h));
                        } else {
                            p.installed.insert(slot, h);
                        }
                    }
                }
                "best" => {
                    if let Some((e, t)) = value.split_once(' ')
                        && let (Ok(e), Ok(t)) = (e.parse(), t.parse())
                    {
                        p.best.insert(e, t);
                    }
                }
                _ => {}
            }
        }
        Some(p)
    }
}
