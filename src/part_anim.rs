//! The part animation channels of a car (the hood, trunk and doors; the
//! records 0x433A80 keeps in the car's "PartsAnimations" list, found by
//! 0x433A30 and the channel): their state, requested by 0x61F830(render
//! info, channel, open, speed) → 0x433E20, and their pose as the car draw
//! evaluates it (0x434190).
//!
//! * A record (0x28 bytes): +0x08 speed, +0x0C channel, +0x10 state
//!   (0 open, 1 shut, 2 opening, 3 closing), +0x14 animation (0x433AD0's
//!   variant), +0x18 the state's start time (the 4000 Hz tick 0x865188 ×
//!   0.00025). Without a record 0x433CD0 answers 1 (shut).
//! * 0x433E20: an open request on an opening / open channel, or a close
//!   on a closing / shut one, does nothing; otherwise it sets the speed and
//!   0x433D50 moves the state: shut → opening and open → closing from now;
//!   opening ⇄ closing with the start moved so the elapsed time mirrors
//!   (now − (L − (now − start)), L the animation's length in seconds,
//!   0x433D10). A channel without a record gets one, opening from now
//!   (0x433A80 sets state 2 whatever was asked).
//! * 0x434190 (each draw): t = L when open, 0 when shut, the elapsed time
//!   while opening (open once it reaches L), L less it while closing
//!   (shut once it reaches L); L = length ÷ speed ÷ 30 s; the frame
//!   (+0x68) = 30 · speed · t, the value 0x434610 = frame ÷ length.
//! * The variant ([`Styles`], picked by 0x61F830 from the fitted parts):
//!   a split hood (2 left / right, 4 front / back) draws each half by its
//!   model's group g (0x623540): 2 + (g ≠ 0), or 4 + (g ≠ 1); its L is
//!   length ÷ (0.75 · speed) ÷ 30 and its frame speed 1.0, or for a
//!   STYLE22 hood 0.75 · speed (g = 0) and 1.25 · speed (g = 1). A door
//!   closing from open keeps the variant it opened with (0x7EA610).
//!   **Not ported** here: the sounds 0x61F830 starts.

use glam::{Mat4, Quat, Vec3};

use crate::formats::zam::PartAnimations;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Open = 0,
    Shut = 1,
    Opening = 2,
    Closing = 3,
}

#[derive(Clone, Copy, Debug)]
struct Record {
    speed: f32,
    state: State,
    variant: usize,
    start: f32,
    /// +0x24: the STYLE22 hood's halves at their own speeds.
    style22: bool,
}

/// The animations a car's fitted parts pick (0x61F830): the hood's
/// variant (0 REG, 2 left / right split, 4 front / back split), whether
/// it is a STYLE22 hood, and the doors' (0 REG, 1 SCISSOR, 2 SUICIDE,
/// 3–6 WIDE1–4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Styles {
    pub hood: usize,
    pub style22: bool,
    pub doors: usize,
}

impl Styles {
    fn variant(&self, channel: usize) -> (usize, bool) {
        match channel {
            0 => (self.hood, self.style22),
            2 | 3 => (self.doors, false),
            _ => (0, false),
        }
    }
}

/// 0x434190: the animation a model of group `group` plays for a record's
/// variant, and whether it is a split hood's half.
fn half_variant(channel: usize, variant: usize, group: usize) -> (usize, bool) {
    match (channel, variant) {
        (0, 2 | 3) => (2 + usize::from(group != 0), true),
        (0, 4 | 5) => (4 + usize::from(group != 1), true),
        _ => (variant, false),
    }
}

/// The four channels: 0 hood, 1 trunk, 2 / 3 left / right door.
#[derive(Clone, Copy, Debug, Default)]
pub struct Channels {
    records: [Option<Record>; 4],
    /// 0x7EA610: each door's variant as it opened.
    door_variant: [Option<usize>; 4],
    /// REG animation instance +0x60: defaults to 1, updated when REG is drawn.
    regular_speed: [Option<f32>; 4],
}

/// A channel's pose at a moment.
#[derive(Clone, Copy, Debug)]
pub struct ChannelPose {
    /// The animation's turn, after the rest matrix's undone where the draw
    /// does that (A · R⁻¹).
    pub rotation: Quat,
    /// a: the animation's translation, else the rest matrix's.
    pub moved: Vec3,
    /// r: the rest matrix's translation.
    pub rest: Vec3,
    /// 0x434610: frame ÷ length, 0 shut … 1 open.
    pub value: f32,
}

impl ChannelPose {
    /// The model's car-space matrix for a solid whose header +0x70 is
    /// `origin` (0x6235A1…0x6243E5, written as row vectors there):
    /// T(a − p) · A · R⁻¹ · T(p − r).
    pub fn part_matrix(&self, origin: Vec3) -> Mat4 {
        Mat4::from_translation(self.moved - origin) * Mat4::from_quat(self.rotation) * Mat4::from_translation(origin - self.rest)
    }
}

impl Channels {
    /// 0x433CD0: the channel's state, shut without a record.
    pub fn state(&self, channel: usize) -> State {
        self.records.get(channel).copied().flatten().map_or(State::Shut, |r| r.state)
    }

    /// 0x61F830 → 0x433E20: open or close `channel` at `speed` (1.0 from
    /// every caller but 0x523DE0's), `now` in seconds.
    pub fn request(&mut self, anims: Option<&PartAnimations>, styles: &Styles, channel: usize, open: bool, speed: f32, now: f32) {
        let (mut variant, style22) = styles.variant(channel);
        let Some(slot) = self.records.get_mut(channel) else { return };
        let Some(r) = slot.as_mut() else {
            self.door_variant[channel] = Some(variant);
            *slot = Some(Record { speed, state: State::Opening, variant, start: now, style22 });
            return;
        };
        match (open, r.state) {
            (true, State::Opening | State::Open) | (false, State::Closing | State::Shut) => return,
            _ => {}
        }
        // 0x433D10 explicitly selects variant 0, even for a styled door.
        // Its instance speed comes from the last REG draw, not this record.
        let length = seconds(anims, channel, self.regular_speed[channel].unwrap_or(1.0));
        if matches!(channel, 2 | 3) {
            match (r.state, open) {
                (State::Shut, true) => self.door_variant[channel] = Some(variant),
                (State::Open, false) => variant = self.door_variant[channel].unwrap_or(variant),
                _ => {}
            }
        }
        r.speed = speed;
        r.style22 = style22;
        r.variant = variant;
        r.state = match (r.state, open) {
            (State::Shut, _) => {
                r.start = now;
                State::Opening
            }
            (State::Open, _) => {
                r.start = now;
                State::Closing
            }
            (State::Opening, _) => {
                r.start = now - (length - (now - r.start));
                State::Closing
            }
            (State::Closing, _) => {
                r.start = now - (length - (now - r.start));
                State::Opening
            }
        };
    }

    /// 0x434190: the channel's pose at `now`, settling an opening or
    /// closing channel that has run its length. None when the car has no
    /// such animation.
    pub fn pose(&mut self, anims: &PartAnimations, channel: usize, group: usize, now: f32) -> Option<ChannelPose> {
        let r = self.records.get_mut(channel)?.as_mut()?;
        let (variant, split) = half_variant(channel, r.variant, group);
        let ((anim, rest), regular) = anims.get(channel, variant).map(|a| (a, variant == 0))
            .or_else(|| anims.get(channel, 0).map(|a| (a, true)))?;
        let (length, frame_speed) = if split {
            let frame_speed = match (r.style22, group) {
                (false, _) => 1.0,
                (true, 0) => r.speed * 0.75,
                (true, _) => r.speed * 1.25,
            };
            (anim.length() * (1.0 / 30.0 / (r.speed * 0.75).max(1e-6)), frame_speed)
        } else {
            ((anim.length() / r.speed.max(1e-6)) / 30.0, r.speed)
        };
        // Cache the instance actually evaluated, including a missing style's
        // REG fallback; keep requested style/rest-transform behavior intact.
        if regular { self.regular_speed[channel] = Some(frame_speed); }
        let elapsed = now - r.start;
        let t = match r.state {
            State::Open => length,
            State::Shut => 0.0,
            State::Opening if elapsed >= length => {
                r.state = State::Open;
                length
            }
            State::Opening => elapsed,
            State::Closing if elapsed >= length => {
                r.state = State::Shut;
                0.0
            }
            State::Closing => length - elapsed,
        };
        let frame = frame_speed * 30.0 * t;
        let (turn, moved) = anim.sample(frame);
        let (_, rest_turn, rest_at) = rest.to_scale_rotation_translation();
        // 0x623A7A: the hood always, the doors' SCISSOR and WIDE1–4
        // (0x434550 = 1, 3…6) undo the rest matrix's turn first.
        let undo = channel == 0 || (matches!(channel, 2 | 3) && matches!(variant, 1 | 3..=6));
        Some(ChannelPose {
            rotation: if undo { turn * rest_turn.inverse() } else { turn },
            moved: moved.unwrap_or(rest_at),
            rest: rest_at,
            value: if anim.length() > 0.0 { frame / anim.length() } else { 0.0 },
        })
    }

    /// The poses of all four channels at `now`: None for a channel shut
    /// when the draw looks (0x6234E2 tests 0x433CD0 before 0x434190), whose
    /// models it leaves at rest.
    /// Per channel, the pose of its group-0 and group-1 models.
    pub fn poses(&mut self, anims: Option<&PartAnimations>, now: f32) -> [[Option<ChannelPose>; 2]; 4] {
        std::array::from_fn(|c| std::array::from_fn(|g| if self.state(c) == State::Shut { None } else { anims.and_then(|a| self.pose(a, c, g, now)) }))
    }
}

/// 0x433D10: REG duration at its cached instance speed, regardless of active style.
fn seconds(anims: Option<&PartAnimations>, channel: usize, speed: f32) -> f32 {
    anims.and_then(|a| a.get(channel, 0)).map_or(0.0, |(a, _)| (1.0 / 30.0) / speed.max(1e-6) * a.length())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::zam::Animation;

    fn anims() -> PartAnimations {
        let turn = |a: f32| Quat::from_rotation_y(a.to_radians());
        let mut a = PartAnimations { pivots: vec![Mat4::from_translation(Vec3::new(1.0, 0.0, 0.5)); 21], ..Default::default() };
        a.animations.insert("_ZAM_HOOD_REG".into(), Animation { frames: 31, rotations: (0..31).map(|f| turn(-2.0 * f as f32)).collect(), translations: Vec::new() });
        a
    }

    #[test]
    fn a_hood_uses_native_duration_and_closes_back() {
        let a = anims();
        let mut c = Channels::default();
        assert_eq!(c.state(0), State::Shut);
        c.request(Some(&a), &Styles::default(), 0, true, 1.0, 10.0);
        assert_eq!(c.state(0), State::Opening);
        let half = c.pose(&a, 0, 0, 10.5).unwrap();
        assert!((half.value - 15.0 / 31.0).abs() < 1e-4);
        assert_eq!(c.pose(&a, 0, 0, 11.0).unwrap().value, 30.0 / 31.0);
        assert_eq!(c.state(0), State::Opening);
        let open = c.pose(&a, 0, 0, 11.2).unwrap();
        assert_eq!((c.state(0), open.value), (State::Open, 1.0));
        // The front (+x of the hinge) is up 60°.
        let tip = open.part_matrix(Vec3::ZERO).transform_point3(Vec3::new(2.0, 0.0, 0.5));
        assert!((tip.z - (0.5 + 60f32.to_radians().sin())).abs() < 1e-3);
        // Closing a quarter of the way in, then opening again mirrors the
        // elapsed time.
        c.request(Some(&a), &Styles::default(), 0, false, 1.0, 20.0);
        assert!((c.pose(&a, 0, 0, 20.25).unwrap().value - (1.0 - 7.5 / 31.0)).abs() < 1e-4);
        c.request(Some(&a), &Styles::default(), 0, true, 1.0, 20.25);
        assert!((c.pose(&a, 0, 0, 20.25).unwrap().value - (1.0 - 7.5 / 31.0)).abs() < 1e-4);
        // The draw settles it open (every frame in the game).
        c.pose(&a, 0, 0, 25.0);
        assert_eq!(c.state(0), State::Open);
        c.request(Some(&a), &Styles::default(), 0, false, 1.0, 30.0);
        c.pose(&a, 0, 0, 31.5);
        assert_eq!(c.state(0), State::Shut);
    }

    #[test]
    fn split_hood_halves_pick_their_own_animation() {
        // 0x434190: left / right by group 0 / 1, front / back by 1 / 0.
        assert_eq!(half_variant(0, 2, 0), (2, true));
        assert_eq!(half_variant(0, 2, 1), (3, true));
        assert_eq!(half_variant(0, 4, 1), (4, true));
        assert_eq!(half_variant(0, 4, 0), (5, true));
        assert_eq!(half_variant(0, 0, 1), (0, false));
        assert_eq!(half_variant(2, 3, 1), (3, false));
    }

    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn door_requests_match_original_authored_duration_oracle() {
        let mut anims=PartAnimations { pivots:vec![Mat4::IDENTITY;21],..Default::default() };
        for channel in [2,3] {
            for (variant,(name,_)) in crate::formats::zam::NAMES[channel].iter().enumerate() {
                // Captured RX8 headers: REG/SUICIDE 30, SCISSOR/WIDE 28.
                anims.animations.insert((*name).into(),Animation { frames:if matches!(variant,0|2) {30} else {28},..Default::default() });
            }
        }
        let bytes=&crate::testkit::private("docs/evidence/fe-door-transitions/transitions.bin");
        for (case,b) in bytes.chunks_exact(52).enumerate() {
            let u=|i:usize| u32::from_le_bytes(b[i*4..i*4+4].try_into().unwrap());
            let f=|i:usize| f32::from_bits(u(i));
            let channel=u(0) as usize;let variant=u(1) as usize;
            let mut c=Channels::default();
            c.request(Some(&anims),&Styles::default(),channel,true,f(4),0.0);
            c.pose(&anims,channel,0,0.1); // Populate the native REG instance speed.
            let r=c.records[channel].as_mut().unwrap();
            r.state=[State::Open,State::Shut,State::Opening,State::Closing][u(2) as usize];
            r.variant=variant;r.speed=f(5);r.start=f(7);
            c.door_variant[channel]=Some(1);
            c.request(Some(&anims),&Styles {doors:variant,..Default::default()},channel,u(3)!=0,f(6),u(8) as f32*0.00025);
            let r=c.records[channel].unwrap();
            assert_eq!(r.state as u32,u(9),"case{case}");
            assert_eq!(r.variant as u32,u(10),"case{case}");
            assert!((r.start-f(11)).abs()<0.000003,"case{case}: start{} native{}",r.start,f(11));
            assert_eq!(c.door_variant[channel],Some(u(12) as usize),"case{case}");
        }
    }

    #[test]
    fn missing_door_style_updates_reg_fallback_speed_before_reversal() {
        let mut anims = PartAnimations { pivots: vec![Mat4::IDENTITY; 21], ..Default::default() };
        anims.animations.insert("_ZAM_LEFT_DOOR_REG".into(), Animation { frames: 30, ..Default::default() });
        let styles = Styles { doors: 3, ..Default::default() };
        let mut channels = Channels::default();
        channels.request(Some(&anims), &styles, 2, true, 2.0, 0.0);
        assert!(channels.pose(&anims, 2, 0, 0.1).is_some(), "missing WIDE falls back to REG");
        channels.request(Some(&anims), &styles, 2, false, 1.5, 0.1);
        let record = channels.records[2].unwrap();
        assert_eq!(record.state, State::Closing);
        assert_eq!(record.variant, 3, "fallback does not replace the requested style");
        // REG was actually evaluated at speed 2: duration .5 seconds.
        assert!((record.start - (-0.3)).abs() < 1e-6,
            "reversal must use the evaluated REG speed: {}", record.start);
    }

    #[test]
    fn a_door_closes_with_the_style_it_opened_with() {
        let mut c = Channels::default();
        let scissor = Styles { doors: 1, ..Default::default() };
        c.request(None, &scissor, 2, true, 1.0, 0.0);
        c.records[2].as_mut().unwrap().state = State::Open;
        c.request(None, &Styles::default(), 2, false, 1.0, 5.0);
        assert_eq!(c.records[2].unwrap().variant, 1);
    }

}
