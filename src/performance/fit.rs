use super::*;

/// Fit a part (0x5A6330 without forcing): not when it is already fitted or
/// a part it needs is not; the parts it stands in for come off. Returns
/// whether it went on.
pub fn install(fitted: &mut std::collections::BTreeSet<u8>, packages: &[Package], id: u8) -> bool {
    let Some((_, part)) = part(packages, id) else { return false };
    if fitted.contains(&id) || part.requires.iter().any(|r| !fitted.contains(r)) {
        return false;
    }
    for r in &part.replaces {
        fitted.remove(r);
    }
    fitted.insert(id);
    true
}

/// The aerodynamics (category 9, the parts no shop sells) a ride's visual
/// parts give it (0x6399E0 without its refit, run by 0x639CF0 whenever a
/// ride's parts or performance change; the branch at 0x639AA2 under the
/// static flag 0x7FC290 = 1): category 9 comes off (0x5A6460(9)); a wide
/// body (slot 6) of level 1 or more (part byte +5 >> 5) puts on all of
/// category 9 at level 3 (0x5A64B0(3, 9)); else the front bumper (slot 1),
/// rear bumper (2), skirt (0xB) and spoiler (0xC) each put on the aero
/// part of their own level (at most 3): ids 0x38–0x3A, 0x3E–0x40,
/// 0x3B–0x3D and 0x41–0x43 by level (0x599690). `level(slot)` is the
/// installed part's level, `None` for an empty slot.
pub fn fit_aero(fitted: &mut std::collections::BTreeSet<u8>, packages: &[Package], level: &dyn Fn(u8) -> Option<u8>) {
    for p in packages.iter().filter(|p| p.category == 9) {
        for x in &p.parts {
            fitted.remove(&x.id);
        }
    }
    if level(6).is_some_and(|l| l > 0) {
        for l in 1..=3 {
            for p in packages.iter().filter(|p| p.category == 9 && p.level == l) {
                for x in &p.parts {
                    install(fitted, packages, x.id);
                }
            }
        }
        return;
    }
    for (slot, ids) in
        [(1u8, [0x38u8, 0x39, 0x3A]), (2, [0x3E, 0x3F, 0x40]), (0xB, [0x3B, 0x3C, 0x3D]), (0xC, [0x41, 0x42, 0x43])]
    {
        if let Some(l) = level(slot).filter(|&l| l > 0) {
            install(fitted, packages, ids[l.min(3) as usize - 1]);
        }
    }
}

/// A category's level (0x5A6590): the per-gain sums over its packages'
/// fitted parts (0x5A61B0; −1 gains skipped), and their mean over `n`
/// gains, each clamped to 0–1 when the mean falls outside it.
pub fn level(packages: &[Package], fitted: &dyn Fn(u8) -> bool, category: u8, n: usize) -> (f32, [f32; 4]) {
    let mut gains = [0.0f32; 4];
    for p in packages.iter().filter(|p| p.category == category && (1..4).contains(&p.level)) {
        for part in p.parts.iter().filter(|part| fitted(part.id)) {
            for k in 0..n {
                if part.gains[k] != -1.0 {
                    gains[k] += part.gains[k];
                }
            }
        }
    }
    let mut mean = gains[..n].iter().sum::<f32>() / n as f32;
    if !(0.0..=1.0).contains(&mean) {
        for g in &mut gains[..n] {
            *g = g.clamp(0.0, 1.0);
        }
        mean = gains[..n].iter().sum::<f32>() / n as f32;
    }
    (mean.clamp(0.0, 1.0), gains)
}

/// A category's gain count, the `n` of [`level`]: 0x5A6590's default (3
/// for 1 and 5, 2 for 9, else 1), which 0x5AC500's per-category calls
/// (0x5A6B60 … 0x5A74B0) also pass.
pub fn gain_count(category: u8) -> usize {
    match category {
        1 | 5 => 3,
        9 => 2,
        _ => 1,
    }
}

/// The upgrade state's float for a category (+0x00 + 4·c): 3 × its level
/// (0x5A6590 stores it; 0x5AC500 zeroes all ten, then its per-category
/// calls write them; the AI refit 0x5A6890 writes its category's).
pub fn state_float(packages: &[Package], fitted: &dyn Fn(u8) -> bool, category: u8) -> f32 {
    level(packages, fitted, category, gain_count(category)).0 * 3.0
}

/// The rounded level 0x5995F0 reads (float + 0.5, truncated), per category.
pub fn state_levels(packages: &[Package], fitted: &dyn Fn(u8) -> bool) -> [u8; 10] {
    std::array::from_fn(|c| (state_float(packages, fitted, c as u8) + 0.5) as u8)
}

/// Take a category off (0x5A6460: every part of its packages, any level,
/// unfitted through 0x5A6330(part, 0, 0)).
pub fn clear_category(fitted: &mut std::collections::BTreeSet<u8>, packages: &[Package], category: u8) {
    for p in packages.iter().filter(|p| p.category == category) {
        for x in &p.parts {
            fitted.remove(&x.id);
        }
    }
}

/// Fit a category up to a level (0x5A64B0(level, category)): the category
/// comes off first (0x5A6460, also at level 0), then for levels 1 to
/// `level` each package's parts go on in order, unforced (0x5A6330(part,
/// 1, 0)).
pub fn fit_category(fitted: &mut std::collections::BTreeSet<u8>, packages: &[Package], category: u8, to: u8) {
    clear_category(fitted, packages, category);
    for l in 1..=to {
        for p in packages.iter().filter(|p| p.category == category && p.level == l) {
            for x in &p.parts {
                install(fitted, packages, x.id);
            }
        }
    }
}

/// A part's mean gain (0x591140): the count of its gains that are not −1
/// (by their bits, 0xBF800000), and the mean of that many gains from the
/// first.
pub(super) fn mean_gain(part: &PerfPart) -> f32 {
    let n = part.gains.iter().filter(|g| g.to_bits() != 0xBF80_0000).count();
    part.gains[..n].iter().sum::<f32>() / n as f32
}

/// 0x5A6220(state, level, [category], 1, smallest): of the category's
/// packages at `level` (0x500100), the unfitted part with the largest mean
/// gain, or with `smallest` the smallest; ties and NaN keep the earlier.
pub(super) fn refit_pick(
    fitted: &std::collections::BTreeSet<u8>,
    packages: &[Package],
    category: u8,
    level: u8,
    smallest: bool,
) -> Option<u8> {
    let mut best: Option<&PerfPart> = None;
    for p in packages.iter().filter(|p| p.category == category && p.level == level) {
        for x in p.parts.iter().filter(|x| !fitted.contains(&x.id)) {
            let take = match best {
                None => true,
                Some(b) if smallest => mean_gain(x) < mean_gain(b),
                Some(b) => mean_gain(b) < mean_gain(x),
            };
            if take {
                best = Some(x);
            }
        }
    }
    best.map(|x| x.id)
}

/// The performance refit (0x5A6890(target, categories, n), on the upgrade
/// state): per category, its parts come off (0x5A6460), then parts are
/// tried until its level (0x5A6590) lies within 0.05 of `target`, keeping
/// the closest found. Levels 1 to min(3, round(3·target) + 1) (ftol of
/// 3·target + 0.5, plus 1), each in three passes from the state with the
/// lower levels fitted: the unfitted part of that level with the largest
/// mean gain fitted (0x5A6220 / 0x5A6330(part, 1, 0)), again and again;
/// the second pass the smallest first; the third alternating, smallest
/// first. Between levels, while not within 0.05, the level is fitted whole
/// (0x5A64B0). Returns the mean of the categories' levels. No random
// prov: inferred performance.refit
/// draws. A part that will not go on ends its pass (**inferred** guard:
/// the game would ask for the same part again; no category-9 part needs
/// another of its own level).
pub fn refit(fitted: &mut std::collections::BTreeSet<u8>, packages: &[Package], target: f32, categories: &[u8]) -> f32 {
    let top = (((f64::from(target * 3.0)) + 0.5) as i32 + 1).min(3);
    let mut sum = 0.0f32;
    for &c in categories {
        clear_category(fitted, packages, c);
        let mut base = fitted.clone();
        let mut kept = base.clone();
        let lv = |s: &std::collections::BTreeSet<u8>| level(packages, &|id| s.contains(&id), c, gain_count(c)).0;
        let mut best = lv(&base);
        let mut done = (best - target).abs() < 0.05;
        let mut l = 1;
        while l <= top && !done {
            for pass in 0..3u8 {
                if done {
                    break;
                }
                let mut a = base.clone();
                let mut ebx = pass;
                let mut next = refit_pick(&a, packages, c, l as u8, ebx != 0);
                while let Some(id) = next {
                    if done || !install(&mut a, packages, id) {
                        break;
                    }
                    let v = lv(&a);
                    if (v - target).abs() < (best - target).abs() {
                        best = v;
                        kept = a.clone();
                        done = (v - target).abs() < 0.05;
                    }
                    if pass == 2 {
                        ebx = u8::from(ebx == 0);
                    }
                    next = refit_pick(&a, packages, c, l as u8, ebx != 0);
                }
            }
            if !done && l < top {
                fit_category(&mut base, packages, c, l as u8);
            }
            l += 1;
        }
        *fitted = kept;
        sum += lv(fitted);
    }
    sum / categories.len().max(1) as f32
}

/// 0x6399E0's refit half (refit = 1, as 0x532020, 0x5260C0 and an AI
/// racer's creation 0x639CF0(owner kind == 2) run it): the target is the
/// mean of the tyres', suspension's and weight's levels (0x599610: float ×
/// 0.33333334, categories 8, 5, 0), clamped to 0–1, and category 9 (the
/// aerodynamics) is refitted to it (0x5A6890(target, {9}, 1)). The
/// human's racers take theirs from their body parts instead
/// ([`fit_aero`]).
pub fn ai_refit(fitted: &mut std::collections::BTreeSet<u8>, packages: &[Package]) {
    let m = |c: u8| state_float(packages, &|id| fitted.contains(&id), c) * 0.333_333_34;
    let t = ((m(5) + m(8)) + m(0)) * 0.333_333_34;
    refit(fitted, packages, t.clamp(0.0, 1.0), &[9]);
}
