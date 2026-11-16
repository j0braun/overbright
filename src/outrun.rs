//! Outrun races in career free roam (see docs/formats.md, "Outrun").
//!
//! The race status's random encounter manager (race status +0xC40, run by
//! 0x42EDF0 each frame of free roam and of an outrun, states NONE,
//! POLL_CAREER, LOADING_OPPONENTS, TRYING_TO_ENGAGE, RACING and
//! DEACTIVATE_OPPONENT, names at 0x7E9B78) keeps three AI racers that the
//! career free-roam set-up builds parked (0x540790: 0x89E7C4 = 3, then
//! 0x53F010(3)). Once the career's side (career +0x4230, 0x513DB0) asks for
//! opponents, it places them on the road ahead of the player (0x42A5E0)
//! and each frame looks for one the player may challenge (0x413030 →
//! 0x40C030), which shows the encounter icon on the HUD. The Enter Race /
//! Shop key (JOY_EVENT_REQUEST_ENGAGE_INFO, 0x602BE0 → 0x602E19) starts the
//! outrun the next frame (race status +0xC30 → 0x607190: race state
//! 0x89E7B0 = 7). The leader is whoever is ahead on the road (0x5FC520,
//! sorted by 0x5EEC10); when the chaser is more than 300 m (feet: 304.9 m)
//! behind, the leader wins (0x6076E0). A win pays the stage's bank, a loss
//! takes it (0x541330).

use glam::{Vec2, Vec3};

/// The encounter cars career free roam builds (0x5407E0: 0x89E7C4 = 3,
/// passed to 0x53F010 at 0x540976).
pub const CARS: usize = 3;

/// Their racer numbers (0x53F010: 100 + i, passed to 0x4FEAF0 and
/// 0x6373F0).
pub const FIRST_RACER: u32 = 100;

/// Seconds between placements (0x42A5E0: none while the newest slot stamp
/// is 5 s old or less, 0x783D38).
pub const SPAWN_GAP: f32 = 5.0;
/// 0x42A5E0's road walk from the player's road node: first 250 m
/// (0x783D08), then 50 m (0x783D0C) a try while under 500 m (0x783D10).
pub const SPAWN_START: f32 = 250.0;
pub const SPAWN_STEP: f32 = 50.0;
pub const SPAWN_END: f32 = 500.0;
/// A point is taken at least 250 m from the player (0x5F0FC0 against
/// 0x78429C), with control point flag bit 11 (+0x0C & 0x800) and no car
/// within 0x410FC0's 30 / 300 m (0x783D14, 0x783D18).
pub const SPAWN_CLEAR: f32 = 250.0;
pub const SPAWN_FLAG: u32 = 0x800;
pub const SPAWN_SPACING: (f32, f32) = (30.0, 300.0);
/// The placed car moves at its racing line's speed byte (control point
/// +0x2C, lane 0 forward / +0x2D lane 1 backward, × 65536 × 1/65536,
/// 0x784278) plus 0.1 (0x5F52D0 → 0x43CF90).
pub const SPAWN_SPEED_ADD: f32 = 0.1;
/// 0x40BEE0: a placed car over 30 s old and 250 m or more from the
/// player is parked again (its flag cleared, 0x5F6E30).
pub const PARK_AGE: f32 = 30.0;
pub const PARK_DISTANCE: f32 = 250.0;

/// The outrun's distance: 300 m (0x7A56B4) with the metric option
/// (0x83AA1A = 1), else 300 + 4.9 m (0x7A5F50, about 1000 ft), added on the
/// x87 stack (0x607DCC). 0x5EEEB0 gives the HUD the same.
pub fn limit(metric: bool) -> f64 {
    if metric { 300.0 } else { 300.0 + f64::from(4.9f32) }
}

/// 0x607DBB–0x607DE5: the chaser whose distance behind the leader
/// (+0x24 after the leader's is taken off) is over the limit has lost.
pub fn lost(gap: f32, metric: bool) -> bool {
    f64::from(gap) > limit(metric)
}

/// A car as these tests read it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Car {
    /// Position (+0x60) and velocity (+0x70).
    pub position: Vec3,
    pub velocity: Vec3,
    /// The heading's direction (+0x90, +0x94).
    pub forward: Vec2,
    /// Speed, |v| (+0x430), and forward speed (+0x42C), m/s (the car
    /// fields as the AI route layer's notes name them).
    pub speed: f32,
    pub forward_speed: f32,
}

impl Car {
    /// 0x40C030's direction: the velocity's x, y while they make 1 m/s
    /// or more (0x784250), else the heading's; normalised (0x43CE50).
    fn direction(&self) -> Vec2 {
        let v = self.velocity.truncate();
        let d = if v.length() < 1.0 { self.forward } else { v };
        d.normalize_or_zero()
    }
}

/// 0x40C030's test of one encounter car: it is ahead of the player along
/// its own direction (> 0), at most 30 m away in the plane (≥ 0), within 5 m
/// in height, doing 30 mph or more (+0x430 × 2.237, 0x78426C), within
/// 10 mph of the player, and its direction within 10° of the player's
/// (0x4000 − 0x43C490 of the directions' dot, × 360/65536 under 10). A
/// slot that passed on the pass before (`held`, slot +0x08) widens the
/// distance to −10 … 40 m, the speeds to ±15 mph and the angle to 20°.
pub fn engage_ok(player: &Car, opponent: &Car, held: bool) -> bool {
    let (pd, od) = (player.direction(), opponent.direction());
    let to = (opponent.position - player.position).truncate();
    if to.dot(od) <= 0.0 {
        return false;
    }
    let distance = to.length();
    let (near, far) = if held { (0.0 - 10.0, 10.0 + 30.0) } else { (0.0, 30.0) };
    if !(near..=far).contains(&distance) {
        return false;
    }
    // 0x40C21D: the height test passes at 5 m too (test ah, 0x41; jp).
    if (opponent.position.z - player.position.z).abs() > 5.0 {
        return false;
    }
    let mph = opponent.speed * 2.237;
    if mph < 30.0 {
        return false;
    }
    let (slow, fast) = if held { (-10.0 - 5.0, 5.0 + 10.0) } else { (-10.0, 10.0) };
    if !(slow..=fast).contains(&(mph - player.speed * 2.237)) {
        return false;
    }
    let angle = 0x4000u16.wrapping_sub(crate::ai::driver::asin16(od.dot(pd))) as f32 * (360.0 / 65536.0);
    angle < if held { 10.0 + 10.0 } else { 10.0 }
}

/// 0x40C030 over the slots (+0x1B18, 0x10 each: +0x00 the held flag,
/// +0x04 the car): three passes over the three slots, each visit clearing
/// the slot's flag and, while none is chosen, testing an active car with
/// the flag it had; the first to pass is the one and its flag is set. The
/// later passes clear the chosen slot's flag again, so every flag ends
/// clear (as the game's loop leaves them). `cars[i]` is None for an empty
/// or parked slot (0x5ED440).
pub fn pick(player: &Car, cars: &[Option<Car>], held: &mut [bool]) -> Option<usize> {
    let mut chosen = None;
    for _ in 0..3 {
        for (i, car) in cars.iter().enumerate().take(held.len()) {
            let was = std::mem::replace(&mut held[i], false);
            if let Some(c) = car
                && chosen.is_none()
                && engage_ok(player, c, was)
            {
                chosen = Some(i);
                held[i] = true;
            }
        }
    }
    chosen
}

/// The leader's road node as 0x5FC520 reads it: its controller's control
/// point (+0x0C; x, y and the i16 z / 8, 0x401A00) and the direction the
/// controller travels it (+0x10 = 1 its own, else the previous point's
/// reversed, 0x401B20 / 0x40CBC0 → 0x401AA0).
#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub position: Vec3,
    pub direction: Vec2,
}

/// 0x5FC520: how far `chaser` is behind `leader` (racer +0x24; the leader
/// itself has 0): their 3D distance (0x43CE10), unless the chaser is over
/// 10 m/s or the leader at most that (forward speed +0x42C, 0x7A56B0), the
/// leader has a
/// road node and the chaser is within 30 m of it (0x7A56B8): then, the
/// leader faster than 10 m/s, with the chaser moving along the node's
/// direction (> 0) it is how far ahead along that direction the leader is
/// (0x40CBC0 of each, 2D); the leader at 10 m/s or less, with the chaser
/// moving away from the node (the node less the chaser's position, dotted
/// with its velocity, < 0) it is minus its distance to the node. Below 0
/// the chaser sorts first and leads (0x5EEC10).
pub fn behind(chaser: &Car, leader: &Car, node: Option<&Node>) -> f32 {
    let mut out = chaser.position.distance(leader.position);
    if !(chaser.forward_speed > 10.0 || leader.forward_speed <= 10.0) {
        return out;
    }
    let Some(n) = node else { return out };
    let to = chaser.position.distance(n.position);
    if !(to < 30.0) {
        return out;
    }
    if leader.forward_speed > 10.0 {
        if n.direction.dot(chaser.velocity.truncate()) > 0.0 {
            let along = |p: Vec3| (p.truncate() - n.position.truncate()).dot(n.direction);
            out = along(leader.position) - along(chaser.position);
        }
    } else if (n.position - chaser.position).dot(chaser.velocity) < 0.0 {
        out = -to;
    }
    out
}

/// 0x5EEC10 in race state 7: of two racers not yet placed, the one with
/// the smaller +0x24 goes first (a swap only when the first's is larger).
/// Returns whether the second leads now.
pub fn second_leads(first: f32, second: f32) -> bool {
    second < first
}

/// The HUD's distance (0x4C9450 over 0x5F82E0's rows): each other racer's
/// row holds trunc((its +0x24 − the player's) × 4000 + 0.5) (0x5F82E0 in
/// state 7), read back × −0.00025; from −`limit` (0x5EEEB0) the value takes a
/// positive row above it, or a negative one above it while it is at most 0.
/// Above 0 the player is behind.
pub fn hud_value(player: f32, others: &[f32], metric: bool) -> f32 {
    let mut v = -limit(metric);
    for &o in others {
        let row = ((f64::from(o) - f64::from(player)) * 4000.0 + 0.5) as i32;
        let r = f64::from(row as f32 * -0.00025f32);
        if (r > 0.0 && v < r) || (r < 0.0 && v <= 0.0 && v < r) {
            v = r;
        }
    }
    v as f32
}

/// What the outrun meter (0x4DABD0) shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Meter {
    /// Ahead (the value at most 0): Distance_Number runs "Ahead" and the
    /// bar is 0xFF30CF30; behind, "Behind" and 0xFF950000.
    pub ahead: bool,
    /// Distance_Number: "%$d" of minus the value (metres), × 3.28
    /// (0x790AE4) in feet; Distance_Text "Meters" 0x6BB2D6AF or "Feet"
    /// 0x001578C3.
    pub number: i32,
    pub unit: u32,
    /// The bar's fill, |value| / the limit, for 0x4C67E0(bar, f, 1, 99, 1).
    pub fill: f32,
    /// Out_Run_Message's lead change while racing (race status state 4):
    /// "You have the Lead!" 0xBFD1936F after being behind (the last value
    /// over 0), "You Lost the Lead!" 0x9B2AF3C7 after being ahead (under 0).
    pub message: Option<u32>,
}

pub const AHEAD_COLOUR: u32 = 0xFF30_CF30;
pub const BEHIND_COLOUR: u32 = 0xFF95_0000;
pub const HAVE_LEAD: u32 = 0xBFD1_936F;
pub const LOST_LEAD: u32 = 0x9B2A_F3C7;

/// 0x4DABD0 outside the split-screen outrun: the value (0x4C9450) and the
/// last frame's (element +0x24, 0 when built).
pub fn meter(value: f32, last: f32, racing: bool, metric: bool) -> Meter {
    let ahead = value <= 0.0;
    let message = if !racing {
        None
    } else if ahead && last > 0.0 {
        Some(HAVE_LEAD)
    } else if !ahead && last < 0.0 {
        Some(LOST_LEAD)
    } else {
        None
    };
    let m = limit(metric);
    let fill = if value > 0.0 {
        (f64::from(value) / m) as f32
    } else if -value > 0.0 {
        (f64::from(-value) / m) as f32
    } else {
        0.0
    };
    // On the x87 stack: minus the value, times the float 3.28 in feet, then
    // truncated (0x75BED4).
    let number = if metric { -f64::from(value) } else { -f64::from(value) * f64::from(3.28f32) };
    Meter { ahead, number: number as i32, unit: if metric { 0x6BB2_D6AF } else { 0x0015_78C3 }, fill, message }
}

/// 0x4C9780: on the meter's fourth frame shown (element +0x44 = 3)
/// Out_Run_Message zooms in with "Outrun Challenge Accepted^Lose Them!"
/// (0x783202B0) for the leader (racer +0x0A = 1), else "…^Catch Them!"
/// (0x18B97380); on the fifth the 0x8CA76279 object runs "Active".
pub fn challenge(leading: bool) -> u32 {
    if leading { 0x7832_02B0 } else { 0x18B9_7380 }
}

/// "You Won the Outrun!" (0xDC089C64, racer status 6) and "You Lost the
/// Outrun!" (0x5D1621B2, status 7), with "Your time:" (0xC2878EBC) and in
/// a career win "Bank Reward:" (0x5D10F6AF) and the stage's bank
/// (0x4C7F00 cases 6 and 7).
pub const WON: u32 = 0xDC08_9C64;
pub const LOST: u32 = 0x5D16_21B2;
pub const BANK_REWARD: u32 = 0x5D10_F6AF;

/// The HUD icon of a random encounter (0x4ABE50 for types 9 and 10).
pub const ICON: u32 = 0x33B5_791A;

/// The career's side (career +0x4230): whether it may ask for opponents
/// (0x513D30), past stage 5 always; otherwise not in stage 0 with its
/// intro flags (0x86347D, 0x86347E), not with a world challenge running (career
/// +0xC08, 0x85B948), only while the stage's outrun wins (stage record
/// +0xDC) are under its definition's +0x40 (−1 no limit), and once its
/// timer (+0x1AE4, less the frame's dt each frame of free roam) is under 0.
pub fn offered(stage: u8, intro: (bool, bool), challenge: bool, wins: u32, cap: i8, timer: f32) -> bool {
    if stage > 5 {
        return true;
    }
    let ok = !(stage == 0 && (intro.0 || intro.1));
    !challenge && ok && (cap == -1 || (wins as i32) < i32::from(cap)) && timer < 0.0
}

/// 0x513E00 / 0x500CB0's gate: not in stage 0 of a career with the second
/// intro flag (0x86347E) set, and not with a world challenge running (0x513E00 only).
pub fn allowed(stage: u8, intro_b: bool, challenge: bool) -> bool {
    !(stage == 0 && intro_b) && !challenge
}

/// 0x541330: a win adds the stage definition's +0x02 to the bank, a loss
/// takes it off, at least 0; a win also counts (stage record +0xDC).
pub fn settle(cash: u32, won: bool, bank: i16) -> u32 {
    let c = cash as i64;
    let b = i64::from(bank);
    if won { (c + b).max(0) as u32 } else { (c - b).max(0) as u32 }
}

/// AIGoalOffsetPursuit's speed cap (goal type 9, vtable 0x784BF8 slot
/// +0x4C = 0x40DC00; the encounter cars' goal before a challenge, 0x42A860
/// → 0x428630): the driver gets the least of its line speed and this. With
/// no race coordinates (0x88F2BE clear, free roam) it is the pursuit table
/// at 0x81E378 (points at 0x7E9BC8: 0 m 1.1, 20 m 1.0, 50 m 0.8, 100 m
/// 0.5, 150 m 0.1; 0x5767A0 interpolates and holds the ends) at the 2D
/// distance to the target (the player) times the target's speed (+0x430),
/// but at least 30 m/s, or for an AI car (owner class 2) 60 mph
/// (0x41D692B6) within 100 m of the target and 40 mph (0x418F0C79) beyond.
/// **Verified** (code; observed under Wine: 17.881 m/s far, 26.822 near).
pub fn pursuit_cap(distance: f32, target_speed: f32) -> f32 {
    const TABLE: [(f32, f32); 5] = [(0.0, 1.1), (20.0, 1.0), (50.0, 0.8), (100.0, 0.5), (150.0, 0.1)];
    let floor = if distance <= 100.0 { f32::from_bits(0x41D6_92B6) } else { f32::from_bits(0x418F_0C79) };
    let scale = if distance <= TABLE[0].0 {
        TABLE[0].1
    } else if distance >= TABLE[4].0 {
        TABLE[4].1
    } else {
        let i = TABLE.windows(2).position(|w| distance >= w[0].0 && distance < w[1].0).unwrap_or(3);
        let ((x0, y0), (x1, y1)) = (TABLE[i], TABLE[i + 1]);
        (distance - x0) / (x1 - x0) * (y1 - y0) + y0
    };
    (scale * target_speed).max(floor)
}

/// A road path for the live outrun comparison's driving bot (check aid,
/// not native; docs/outrun.md): from the control point nearest `at`,
/// travelled along `dir`, the racer search (0x415130, `racer_search`)
/// stepped until it moves, `seed` drawing its junctions, for `metres`.
pub fn road_path(f: &crate::formats::ai_routes::RouteFile, at: glam::Vec3, dir: glam::Vec2, metres: f32, seed: u32) -> Vec<glam::Vec3> {
    let grid = crate::formats::ai_routes::PointGrid::new(f);
    let Some((r, c)) = grid.nearest(f, at) else { return Vec::new() };
    let backward = f.direction(r, c, false).dot(dir) < 0.0;
    let mut rng = crate::opponents::GameRng(seed);
    let mut node = (r, c, backward);
    let mut out = vec![f.records[r].points[c].position];
    let mut walked = 0.0;
    while walked < metres {
        // The search stops on the point it reaches, so step until it moves.
        let mut step = 8.0;
        let n = loop {
            match crate::ai::traffic::racer_search(f, node.0, node.1, node.2, step, false, &mut rng) {
                Some(n) if (n.0, n.1) != (node.0, node.1) => break Some(n),
                Some(_) if step < 400.0 => step += 8.0,
                _ => break None,
            }
        };
        let Some(n) = n else { break };
        node = n;
        out.push(f.records[n.0].points[n.1].position);
        walked += step;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn car(x: f32, y: f32, vx: f32) -> Car {
        Car { position: Vec3::new(x, y, 0.0), velocity: Vec3::new(vx, 0.0, 0.0), forward: Vec2::X, speed: vx.abs(), forward_speed: vx }
    }

    #[test]
    fn the_leader_wins_past_the_distance() {
        // 300 m in metres, 304.9 m (about 1000 ft) in feet; at the limit the
        // race goes on.
        assert!(!lost(300.0, true));
        assert!(lost(300.01, true));
        assert!(!lost(304.8, false));
        assert!(lost(304.91, false));
        assert!(lost(301.0, true) && !lost(301.0, false));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn engaging_needs_a_matched_car_close_ahead() {
        let me = car(0.0, 0.0, 20.0);
        // 20 m ahead, same direction, same speed (44.7 mph).
        assert!(engage_ok(&me, &car(20.0, 0.0, 20.0), false));
        // Behind the player, too far, or slower than 30 mph.
        assert!(!engage_ok(&me, &car(-20.0, 0.0, 20.0), false));
        assert!(!engage_ok(&me, &car(35.0, 0.0, 20.0), false));
        assert!(engage_ok(&me, &car(35.0, 0.0, 20.0), true));
        let slow = car(0.0, 0.0, 12.0);
        assert!(!engage_ok(&slow, &car(20.0, 0.0, 12.0), false));
        // 10 mph apart passes, 12 mph only with the wider band.
        assert!(engage_ok(&me, &car(20.0, 0.0, 20.0 + 10.0 / 2.237 - 0.01), false));
        assert!(!engage_ok(&me, &car(20.0, 0.0, 20.0 + 12.0 / 2.237), false));
        assert!(engage_ok(&me, &car(20.0, 0.0, 20.0 + 12.0 / 2.237), true));
        // 15° apart fails, unless held.
        let mut turned = car(20.0, 0.0, 0.0);
        turned.velocity = Vec3::new(15f32.to_radians().cos(), 15f32.to_radians().sin(), 0.0) * 20.0;
        turned.speed = 20.0;
        assert!(!engage_ok(&me, &turned, false));
        assert!(engage_ok(&me, &turned, true));
    }

    #[test]
    fn the_pick_takes_the_first_and_leaves_the_flags_clear() {
        let me = car(0.0, 0.0, 20.0);
        let cars = [None, Some(car(25.0, 0.0, 20.0)), Some(car(10.0, 0.0, 20.0))];
        let mut held = [true, true, false];
        assert_eq!(pick(&me, &cars, &mut held), Some(1));
        assert_eq!(held, [false, false, false]);
    }

    #[test]
    fn the_chaser_takes_the_lead_past_the_leader_on_its_road() {
        let node = Node { position: Vec3::ZERO, direction: Vec2::X };
        let leader = car(10.0, 0.0, 20.0);
        // Behind on the road: positive; level: 0; past it: negative.
        assert_eq!(behind(&car(4.0, 1.0, 22.0), &leader, Some(&node)), 6.0);
        assert!(behind(&car(14.0, 1.0, 22.0), &leader, Some(&node)) < 0.0);
        assert!(second_leads(0.0, behind(&car(14.0, 1.0, 22.0), &leader, Some(&node))));
        // Away from the node, or moving against it: the straight distance.
        assert_eq!(behind(&car(-50.0, 0.0, 22.0), &leader, Some(&node)), 60.0);
        assert_eq!(behind(&car(4.0, 0.0, -22.0), &leader, Some(&node)), 6.0);
        assert_eq!(behind(&car(4.0, 0.0, 22.0), &leader, None), 6.0);
    }

    #[test]
    fn the_meter_reads_the_gap() {
        // The player 120 m behind the opponent: the row rounds to
        // −479999 ticks (+0.5 then truncated), read back as 119.99975, so
        // red and "-119".
        let v = hud_value(120.0, &[0.0], true);
        assert!((v - 120.0).abs() < 1e-3);
        let m = meter(v, -5.0, true, true);
        assert!(!m.ahead && m.number == -119 && m.message == Some(LOST_LEAD));
        assert!((m.fill - 0.4).abs() < 1e-4);
        // Leading by 60 m: −60, shown as 196 ft in feet.
        let v = hud_value(0.0, &[60.0], false);
        assert!((v + 60.0).abs() < 1e-3);
        let m = meter(v, 3.0, true, false);
        assert!(m.ahead && m.number == 196 && m.unit == 0x0015_78C3 && m.message == Some(HAVE_LEAD));
        // No lead change while not racing.
        assert_eq!(meter(v, 3.0, false, false).message, None);
    }

    #[test]
    fn the_career_offers_outruns_by_stage() {
        assert!(offered(1, (false, false), false, 4, 5, -0.1));
        assert!(!offered(1, (false, false), false, 5, 5, -0.1));
        assert!(!offered(1, (false, false), false, 0, 5, 0.0));
        assert!(!offered(1, (false, false), true, 0, 5, -0.1));
        assert!(!offered(0, (true, false), false, 0, 3, -0.1));
        assert!(offered(6, (true, true), true, 99, 0, 1.0));
        assert!(offered(2, (false, false), false, 99, -1, -0.1));
        assert_eq!(settle(50, false, 100), 0);
        assert_eq!(settle(50, true, 100), 150);
    }

    #[test]
    fn pursuit_cap_matches_the_native_table_and_floors() {
        // Observed under Wine (docs/evidence/outrun-live): a far car's
        // target 17.881, one within 100 m of a standing player 26.822.
        assert_eq!(super::pursuit_cap(300.0, 0.0).to_bits(), 0x418F_0C79);
        assert_eq!(super::pursuit_cap(60.0, 0.0).to_bits(), 0x41D6_92B6);
        assert!((super::pursuit_cap(10.0, 40.0) - 42.0).abs() < 1e-4);
        assert!((super::pursuit_cap(75.0, 60.0) - 39.0).abs() < 1e-4);
        assert!((super::pursuit_cap(0.0, 30.0) - 33.0).abs() < 1e-4);
    }
}
