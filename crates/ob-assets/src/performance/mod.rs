//! Performance upgrades: the packages (career chunk 0x34A15, registered by
//! 0x500000) and what fitted ones do to a car-type record's tuning block
//! before the physics reads it (0x5AC500 with the per-category 0x5A6B60 …
//! 0x5A74B0).
//!
//! A package (0x17C bytes) is a category, a level (1–3) and up to four
//! parts. A part (0x5C bytes): +0x00 its id (0–0x43; the car's fitted ones
//! are bytes at the upgrade state +0x50 + id), +0x04 its name's language
//! hash, +0x08 its price, +0x0C its brand count and +0x10 eight brands,
//! +0x30 four gains (−1: none), +0x40 five ids it stands in for and +0x54
//! two it needs fitted first (0x5A6330; −1: none).
//!
//! A category's level (0x5A6590) is, per gain, the sum over its packages'
//! fitted parts, averaged over the category's gains and clamped to 0–1.
//! The upgrades move tuning-block fields (record +0x110 on) towards, or add,
//! the record's upgrade data further on; offsets below are from +0x110.

mod apply;
mod fit;
mod packages;
mod ratings;

pub use self::apply::*;
pub use self::fit::*;
pub use self::packages::*;
pub use self::ratings::*;

#[cfg(test)]
mod unique_tests {
    use super::*;
    use crate::formats::bundle::read_f32;

    fn put(r: &mut [u8], o: usize, v: f32) {
        r[0x110 + o..0x114 + o].copy_from_slice(&v.to_le_bytes());
    }

    fn at(r: &[u8], o: usize) -> f32 {
        read_f32(r, 0x110 + o)
    }

    /// 0x5997F0: each unique adds the car type's unique data.
    #[test]
    fn uniques_add_the_unique_data() {
        let mut r = vec![0u8; 0x890];
        put(&mut r, 0x110, 1000.0);
        put(&mut r, 0x2B0, 0.015);
        put(&mut r, 0x2B4, 0.2);
        put(&mut r, 0x2B8, 0.125);
        put(&mut r, 0x2D0, 0.01);
        put(&mut r, 0x2F4, 0.02);
        put(&mut r, 0x318, 0.04);
        put(&mut r, 0x200, 0.1);
        let stock = r.clone();
        apply_uniques(&mut r, &|_| false);
        assert_eq!(r, stock);
        apply_uniques(&mut r, &|i| matches!(i, 0 | 1 | 2 | 3 | 4 | 7));
        assert!((at(&r, 0x110) - 985.0).abs() < 1e-3);
        assert_eq!(at(&r, 0x1C0), 0.2);
        assert_eq!(at(&r, 0x2AC), 0.125);
        assert!((at(&r, 0x200) - 0.17).abs() < 1e-6);
        assert_eq!(at(&r, 0x224), 0.04);
        // Through `upgrade`: UNIQUE_BASE + i.
        let mut u = stock.clone();
        upgrade(&mut u, &[], &|id| id == UNIQUE_BASE + 2);
        assert_eq!(at(&u, 0x2AC), 0.125);
    }
}

#[cfg(test)]
mod aero_tests {
    use super::*;

    /// Category 9 as the game's data has it: per level, front bumper,
    /// skirt, rear bumper and spoiler parts, each level replacing the one
    /// below.
    fn aero() -> Vec<Package> {
        (1..=3u8)
            .map(|level| Package {
                category: 9,
                level,
                parts: [0x38u8, 0x3B, 0x3E, 0x41]
                    .iter()
                    .map(|&base| PerfPart {
                        id: base + level - 1,
                        name: 0,
                        price: 0,
                        brands: Vec::new(),
                        gains: [0.0; 4],
                        replaces: if level > 1 { vec![base + level - 2] } else { Vec::new() },
                        requires: Vec::new(),
                    })
                    .collect(),
            })
            .collect()
    }

    /// 0x639AA2: a wide body puts all of category 9 on at level 3.
    #[test]
    fn wide_body_fits_level_three() {
        let packages = aero();
        let mut fitted: std::collections::BTreeSet<u8> = [0x39, 0x3C, 0x3F, 0x42, 5].into();
        fit_aero(&mut fitted, &packages, &|slot| (slot == 6).then_some(1));
        assert_eq!(fitted, [5, 0x3A, 0x3D, 0x40, 0x43].into());
    }

    /// Without a wide body each of bumpers, skirt and spoiler gives its
    /// own aero part by its level; stock (level 0) and empty slots none.
    #[test]
    fn body_parts_fit_their_own_level() {
        let packages = aero();
        let mut fitted: std::collections::BTreeSet<u8> = [0x3A, 0x3D].into();
        fit_aero(&mut fitted, &packages, &|slot| match slot {
            1 => Some(2),
            2 => Some(0),
            0xC => Some(5),
            6 => Some(0),
            _ => None,
        });
        assert_eq!(fitted, [0x39, 0x43].into());
    }
}

#[cfg(test)]
mod refit_tests {
    use super::*;

    fn part(id: u8, g: f32) -> PerfPart {
        PerfPart {
            id,
            name: 0,
            price: 0,
            brands: Vec::new(),
            gains: [g, g, -1.0, -1.0],
            replaces: Vec::new(),
            requires: Vec::new(),
        }
    }

    /// 0x5A6890 on one level-1 category-9 package: pass 0 tries the
    /// largest gain first (0.6, no closer than 0 to 0.3), pass 1 the
    /// smallest (0.2, closer) — that one is kept.
    #[test]
    fn refit_keeps_the_closest_level() {
        let packages = vec![Package { category: 9, level: 1, parts: vec![part(0x38, 0.6), part(0x39, 0.2)] }];
        let mut fitted = std::collections::BTreeSet::new();
        let mean = refit(&mut fitted, &packages, 0.3, &[9]);
        assert_eq!(fitted.into_iter().collect::<Vec<_>>(), vec![0x39]);
        assert!((mean - 0.2).abs() < 1e-6);
        // Within 0.05 of the target at once: nothing goes on.
        let mut fitted = [0x38u8].into_iter().collect();
        refit(&mut fitted, &packages, 0.02, &[9]);
        assert!(fitted.is_empty());
    }

    #[test]
    fn fit_category_clears_then_fits_up_to_the_level() {
        let packages = vec![
            Package { category: 2, level: 1, parts: vec![part(0x10, 0.3)] },
            Package { category: 2, level: 2, parts: vec![part(0x11, 0.3)] },
        ];
        let mut fitted = [0x11u8].into_iter().collect();
        fit_category(&mut fitted, &packages, 2, 1);
        assert_eq!(fitted.iter().copied().collect::<Vec<_>>(), vec![0x10]);
        fit_category(&mut fitted, &packages, 2, 0);
        assert!(fitted.is_empty());
    }
}
