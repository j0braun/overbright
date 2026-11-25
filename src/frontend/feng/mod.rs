//! FEng packages: the game's front-end screens (`*.fng`, chunk 0x30210
//! compressed, 0x30203 plain; see docs/formats.md, "Front end").
//!
//! A package is a tree of four-character tags, each `tag u32, size u32`:
//!
//! ```text
//! FEn\xE7  package
//!   PkHd   header: name, file path
//!   TypS   type sizes
//!   Res\xCC  resources: RsNm (names, NUL-separated), RsRq (requests)
//!   Obj\xCC  objects: Butn, then FOb\xEA per object:
//!            ObjD (two-character tags), Scrp* (animations), MsgR (responses)
//! ```
//!
//! Inside `ObjD` and `Scrp` the tags are two characters with a 16-bit size.

mod anim;
pub mod dim;
pub mod font;
pub mod layout;
pub mod model;
pub mod package_run;
pub mod parse;
pub mod script;
pub mod state;
pub mod strings;
pub mod text;

pub use anim::*;
pub use model::*;
pub use parse::*;

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Quat;

    fn track(param: u8, flags: u8, length: u32, base: Vec<u32>, keys: Vec<(u32, Vec<u32>)>) -> Track {
        Track { param, size: (base.len() * 4) as u8, interpolation: 1, flags, length, offset: 0, base, keys }
    }

    /// 0x519B00 / 0x4FE6B0: a rotation track's keys are quaternion deltas
    /// composed with the base, so an identity delta keeps the base's angle
    /// and a 90° delta adds 90° (adding them would halve the angle).
    #[test]
    fn rotation_deltas_compose() {
        let q = |r: Quat| vec![r.x.to_bits(), r.y.to_bits(), r.z.to_bits(), r.w.to_bits()];
        let base = Quat::from_rotation_z(0.4);
        let t = track(
            5,
            0,
            100,
            q(base),
            vec![(0, q(Quat::IDENTITY)), (100, q(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)))],
        );
        let at = |ms| {
            let v = sample(&t, ms);
            Quat::from_xyzw(f32::from_bits(v[0]), f32::from_bits(v[1]), f32::from_bits(v[2]), f32::from_bits(v[3]))
        };
        assert!(at(0).angle_between(base) < 1e-5);
        assert!(at(100).angle_between(Quat::from_rotation_z(0.4 + std::f32::consts::FRAC_PI_2)) < 1e-5);
        assert!(at(50).angle_between(Quat::from_rotation_z(0.4 + std::f32::consts::FRAC_PI_4)) < 1e-4);
    }

    /// 0x5051A0: colours step by ftol((b − a)·f + 0.5), which truncates
    /// towards zero (−2.5 + 0.5 → −2, not round's −3).
    #[test]
    fn colour_rounding() {
        let t =
            track(6, 0, 100, vec![100, 0, 0, 0], vec![(0, vec![0, 0, 0, 0]), (100, vec![(-10i32) as u32, 7, 0, 0])]);
        let v = sample(&t, 25);
        assert_eq!(v[0] as i32, 100 - 2);
        assert_eq!(v[1] as i32, 2);
    }

    /// 0x52BDD0 loop mode 1: past the last key the value runs back
    /// towards the first key over the rest of the track's length.
    #[test]
    fn looping_track_wraps() {
        let f = |x: f32| x.to_bits();
        let t = track(4, 1, 100, vec![f(0.0)], vec![(0, vec![f(0.0)]), (50, vec![f(10.0)])]);
        assert_eq!(f32::from_bits(sample(&t, 50)[0]), 10.0);
        assert_eq!(f32::from_bits(sample(&t, 75)[0]), 5.0);
        let clamp = track(4, 0, 100, vec![f(0.0)], vec![(0, vec![f(0.0)]), (50, vec![f(10.0)])]);
        assert_eq!(f32::from_bits(sample(&clamp, 75)[0]), 10.0);
    }
}
