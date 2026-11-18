//! Fix Original Model Errors (opt-in, Enhancements; docs/model-fixes.md).
//!
//! Small corrections of errors in the shipped car models, applied when a
//! car is loaded, to the player's own files' data: nothing from any mod is
//! used or copied. Each fix names the model it touches and first checks
//! that the data still has the error (the exact values the original
//! files hold), so other data is left alone. Off (the default) = the files
//! as shipped, untouched: every entry point returns at once.
//!
//! The errors were found by the private developer probe `model_scan` and confirmed in
//! close-ups of the clone and of SPEED2.EXE under Wine (main-menu
//! Customize); see docs/model-fixes.md for the evidence.
use crate::formats::hash::bin_hash;
use crate::formats::solid::Solid;

/// Whether the fixes are in force (the Enhancements row).
pub fn enabled() -> bool {
    crate::qol::fix_models()
}

/// IS300 FRENZY rear bumper (`IS300_KIT11_REAR_BUMPER_A`): its marker list
/// holds LEFT_EXHAUST twice, 2.7 cm apart, (−2.220, −0.504, 0.035) and
/// (−2.231, −0.503, 0.010), and REAR_LICENSE_PLATE twice at the same
/// spot. The exhaust walk (0x639843) files every EXHAUST-type marker, so
/// two tips are drawn into each other; the plate is drawn twice in place.
/// The fix keeps each marker's first record.
const DUPLICATE_MARKERS: &[(&str, &[&str])] = &[("IS300_KIT11_REAR_BUMPER_A", &["LEFT_EXHAUST", "REAR_LICENSE_PLATE"])];

/// GOLF SNYPER front bumper (`GOLF_KIT01_FRONT_BUMPER_*`): the lower
/// grille's slats (MOLDINGS on the skin texture `GOLF_SKIN1`) sample the
/// skin at (0.043, 0.854), a texel the paint covers, so they show in the
/// body colour (seen in the original's Customize). The fix moves those
/// corners to (0.795, 0.719), the black trim box of the Golf skin that the
/// car's other skin-mapped MOLDINGS (the KIT03 front bumper) sample; the
/// bumper's own (0, 1) sits on the texture's edge, where wrapped filtering
/// blends in the painted first row.
struct UvFix {
    solid: &'static str,
    light: &'static str,
    from: [f32; 2],
    to: [f32; 2],
}

const UV_FIXES: &[UvFix] = &[
    UvFix { solid: "GOLF_KIT01_FRONT_BUMPER_A", light: "MOLDINGS", from: [0.043, 0.854], to: [0.795, 0.719] },
    UvFix { solid: "GOLF_KIT01_FRONT_BUMPER_B", light: "MOLDINGS", from: [0.043, 0.854], to: [0.795, 0.719] },
    UvFix { solid: "GOLF_KIT01_FRONT_BUMPER_C", light: "MOLDINGS", from: [0.043, 0.854], to: [0.795, 0.719] },
];

/// Front wheels drawn ahead of their arches (car type table, GLOBALB
/// 0x34600 +0x120): the arch's circle (fitted to the stock body, front
/// bumper and skirt around the wheel, `model_scan --check wheel`) has its
/// centre this far behind the table's wheel centre; the front tyre runs
/// into the fender's front edge. Only the drawn wheels, brakes and
/// spinners move (x, metres); the physics keeps the table.
/// The visual review of every model added the four more cars whose front tyre the
/// original's Body Shop (Rims) shows touching the arch's front edge with a
/// gap behind: IMPREZAWRX (dx −0.044), PEUGOT106 (−0.038), PEUGOT
/// (−0.036) and NAVIGATOR (−0.056). The 3000GT's fit (−0.046) comes from a
/// poor circle (dz −0.165); its tyre sits centred in the original, so it
/// is left alone.
const WHEEL_SHIFTS: &[(&str, [f32; 4])] = &[
    ("MUSTANGGT", [-0.069, -0.069, 0.0, 0.0]),
    ("A3", [-0.042, -0.042, 0.0, 0.0]),
    ("IMPREZAWRX", [-0.044, -0.044, 0.0, 0.0]),
    ("PEUGOT106", [-0.038, -0.038, 0.0, 0.0]),
    ("PEUGOT", [-0.036, -0.036, 0.0, 0.0]),
    ("NAVIGATOR", [-0.056, -0.056, 0.0, 0.0]),
];

/// PEUGOT106 stock roof antenna: three crossed DECAL quads in
/// `PEUGOT106_BASE_A` / `_B` / `_C` (the `PEUGOT106_BADGING` strip at UV
/// u 0.719…0.773), standing at x −0.02…0.29, y ±0.01, z 1.16…1.33 on the
/// roof's centre line. Every roof scoop sits at the ROOF_SCOOP marker
/// (−0.130, 0, 1.175) on that line, so with a scoop fitted the antenna
/// runs through it (seen in the original's Body Shop). The fix
/// leaves the antenna out while a roof scoop (slot 7) above level 0 is
/// fitted. The box below picks exactly those six triangles.
const ANTENNA_UNDER_SCOOP: &[(&str, [f32; 3], [f32; 3])] =
    &[("PEUGOT106_BASE_", [-0.03, -0.02, 1.15], [0.30, 0.02, 1.34])];

/// The triangles (numbers, index / 3) of `solid` to leave out of the draw
/// with the fixes in force: the PEUGOT106 antenna while a roof scoop is
/// fitted ([`ANTENNA_UNDER_SCOOP`]). Empty when the fixes are off.
pub fn hidden_triangles(solid: &Solid, scoop_fitted: bool) -> Vec<u32> {
    if !enabled() {
        return Vec::new();
    }
    hidden_triangles_always(solid, scoop_fitted)
}

/// [`hidden_triangles`] regardless of the setting.
pub fn hidden_triangles_always(solid: &Solid, scoop_fitted: bool) -> Vec<u32> {
    let Some((_, lo, hi)) = ANTENNA_UNDER_SCOOP.iter().find(|(p, _, _)| solid.name.starts_with(p)) else {
        return Vec::new();
    };
    if !scoop_fitted {
        return Vec::new();
    }
    let inside = |i: u16| {
        let p = solid.vertices[i as usize].position;
        (0..3).all(|k| p[k] >= lo[k] && p[k] <= hi[k])
    };
    solid
        .indices
        .chunks(3)
        .enumerate()
        .filter(|(_, t)| t.len() == 3 && t.iter().all(|&i| inside(i)))
        .map(|(n, _)| n as u32)
        .collect()
}

/// The x shift of each drawn wheel (table order) for `car`; zeros when the
/// fixes are off.
pub fn wheel_shift(car: &str) -> [f32; 4] {
    if !enabled() {
        return [0.0; 4];
    }
    wheel_shift_always(car)
}

/// [`wheel_shift`] regardless of the setting.
pub fn wheel_shift_always(car: &str) -> [f32; 4] {
    WHEEL_SHIFTS.iter().find(|(c, _)| *c == car).map_or([0.0; 4], |(_, s)| *s)
}

/// Apply the fixes to a car's loaded solids (own and shared) when they are
/// in force. Returns how many solids changed.
pub fn apply(solids: &mut [Solid], light_names: &dyn Fn(u32) -> Option<String>) -> usize {
    if !enabled() {
        return 0;
    }
    apply_always(solids, light_names)
}

/// [`apply`] regardless of the setting (tests, the scanner).
pub fn apply_always(solids: &mut [Solid], light_names: &dyn Fn(u32) -> Option<String>) -> usize {
    let mut changed = 0;
    for s in solids.iter_mut() {
        let mut hit = false;
        if let Some((_, names)) = DUPLICATE_MARKERS.iter().find(|(n, _)| *n == s.name) {
            let before = s.markers.len();
            s.markers = dedup_markers(&s.markers, names);
            hit |= s.markers.len() != before;
        }
        let name = s.name.clone();
        for f in UV_FIXES.iter().filter(|f| f.solid == name) {
            hit |= fix_uvs(s, f, light_names) > 0;
        }
        changed += usize::from(hit);
    }
    changed
}

/// A solid's markers as the exhaust / plate walk reads them (the raw
/// list from the file): with the fixes in force, a duplicated marker of
/// the solid named in [`DUPLICATE_MARKERS`] keeps only its first record.
pub fn markers(solid_name: Option<&str>, raw: Vec<(u32, [f32; 16])>) -> Vec<(u32, [f32; 16])> {
    if !enabled() {
        return raw;
    }
    match solid_name.and_then(|n| DUPLICATE_MARKERS.iter().find(|(s, _)| *s == n)) {
        Some((_, names)) => dedup_markers(&raw, names),
        None => raw,
    }
}

fn dedup_markers(markers: &[(u32, [f32; 16])], names: &[&str]) -> Vec<(u32, [f32; 16])> {
    let hashes: Vec<u32> = names.iter().map(|n| bin_hash(n)).collect();
    let mut seen = Vec::new();
    markers
        .iter()
        .filter(|(h, _)| {
            if !hashes.contains(h) {
                return true;
            }
            if seen.contains(h) {
                return false;
            }
            seen.push(*h);
            true
        })
        .copied()
        .collect()
}

/// Move the corners of `f.light` draws at `f.from` (to 1/1000) to `f.to`.
/// Vertices shared with other materials are left alone (none are, in the
/// fixed models). Returns the corners moved.
fn fix_uvs(s: &mut Solid, f: &UvFix, light_names: &dyn Fn(u32) -> Option<String>) -> usize {
    let near = |uv: [f32; 2]| (uv[0] - f.from[0]).abs() < 1e-3 && (uv[1] - f.from[1]).abs() < 1e-3;
    let mut own = vec![false; s.vertices.len()];
    let mut other = vec![false; s.vertices.len()];
    for m in &s.materials {
        let light = s.light_materials.get(m.light_material as usize).and_then(|&h| light_names(h));
        let mine = light.as_deref() == Some(f.light);
        for &i in &s.indices[m.first_index as usize..(m.first_index + m.index_count) as usize] {
            if mine { own[i as usize] = true } else { other[i as usize] = true }
        }
    }
    let mut moved = 0;
    for (i, v) in s.vertices.iter_mut().enumerate() {
        if own[i] && !other[i] && near(v.uv) {
            v.uv = f.to;
            moved += 1;
        }
    }
    moved
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::bundle::{self, id};

    fn solids(car: &str) -> Option<Vec<Solid>> {
        let root = std::env::var_os("NFSU2_ROOT")?;
        let data = std::fs::read(std::path::Path::new(&root).join("CARS").join(car).join("GEOMETRY.BIN")).ok()?;
        let mut out = Vec::new();
        for l in bundle::chunks(&data, 0).filter(|c| c.id == id::SOLID_LIST) {
            out.extend(crate::formats::solid::parse_solid_list(l).ok()?);
        }
        Some(out)
    }

    fn light_names() -> Option<std::collections::HashMap<u32, String>> {
        let root = std::env::var_os("NFSU2_ROOT")?;
        let g = std::fs::read(std::path::Path::new(&root).join("GLOBAL/GLOBALB.BUN")).ok()?;
        Some(crate::car::parse_light_materials(&g).into_iter().map(|(h, l)| (h, l.name)).collect())
    }

    /// Off: nothing changes (the native path).
    #[test]
    fn off_changes_nothing() {
        assert!(!enabled());
        assert_eq!(wheel_shift("MUSTANGGT"), [0.0; 4]);
        let raw = vec![(bin_hash("LEFT_EXHAUST"), [0.0; 16]); 2];
        assert_eq!(markers(Some("IS300_KIT11_REAR_BUMPER_A"), raw.clone()).len(), 2);
        assert_eq!(apply(&mut [], &|_| None), 0);
    }

    /// IS300 FRENZY: the original holds LEFT_EXHAUST and REAR_LICENSE_PLATE
    /// twice; fixed, once each, the first records kept.
    #[test]
    fn is300_frenzy_exhaust_marker_once() {
        let Some(mut all) = solids("IS300") else { return };
        let count = |s: &Solid, n: &str| s.markers.iter().filter(|m| m.0 == bin_hash(n)).count();
        let s = all.iter().find(|s| s.name == "IS300_KIT11_REAR_BUMPER_A").expect("Frenzy rear bumper");
        assert_eq!((count(s, "LEFT_EXHAUST"), count(s, "REAR_LICENSE_PLATE")), (2, 2), "the original's duplicates");
        let first = s.markers.iter().find(|m| m.0 == bin_hash("LEFT_EXHAUST")).unwrap().1;
        let others =
            all.iter().filter(|s| s.name != "IS300_KIT11_REAR_BUMPER_A").map(|s| s.markers.clone()).collect::<Vec<_>>();
        assert_eq!(apply_always(&mut all, &|_| None), 1);
        let s = all.iter().find(|s| s.name == "IS300_KIT11_REAR_BUMPER_A").unwrap();
        assert_eq!((count(s, "LEFT_EXHAUST"), count(s, "REAR_LICENSE_PLATE")), (1, 1));
        assert_eq!(s.markers.iter().find(|m| m.0 == bin_hash("LEFT_EXHAUST")).unwrap().1, first);
        assert_eq!(
            all.iter().filter(|s| s.name != "IS300_KIT11_REAR_BUMPER_A").map(|s| s.markers.clone()).collect::<Vec<_>>(),
            others
        );
    }

    /// GOLF SNYPER: the lower-grille MOLDINGS sample (0.043, 0.854); fixed,
    /// (0, 1) like the bumper's other trim, and no other UV moves.
    #[test]
    fn golf_snyper_grille_unpainted() {
        let (Some(mut all), Some(names)) = (solids("GOLF"), light_names()) else { return };
        let ln = |h: u32| names.get(&h).cloned();
        let before: Vec<Vec<[f32; 2]>> = all.iter().map(|s| s.vertices.iter().map(|v| v.uv).collect()).collect();
        let painted = |s: &Solid| {
            s.vertices.iter().filter(|v| (v.uv[0] - 0.043).abs() < 1e-3 && (v.uv[1] - 0.854).abs() < 1e-3).count()
        };
        let a = all.iter().find(|s| s.name == "GOLF_KIT01_FRONT_BUMPER_A").expect("Snyper front bumper");
        assert!(painted(a) >= 40, "the original's slats sample the painted texel ({})", painted(a));
        assert!(apply_always(&mut all, &ln) >= 1);
        let a = all.iter().find(|s| s.name == "GOLF_KIT01_FRONT_BUMPER_A").unwrap();
        assert_eq!(painted(a), 0);
        for (s, b) in all.iter().zip(&before) {
            if !s.name.starts_with("GOLF_KIT01_FRONT_BUMPER") {
                assert!(s.vertices.iter().map(|v| v.uv).eq(b.iter().copied()), "{} untouched", s.name);
            }
        }
    }

    /// The wheel shifts are only the front pair of the cars confirmed in the
    /// original (MUSTANGGT, A3; IMPREZAWRX, PEUGOT106, PEUGOT,
    /// NAVIGATOR).
    /// PEUGOT106: with a roof scoop the six antenna triangles of each
    /// BASE LOD are left out; without one, or on other solids, nothing.
    #[test]
    fn peugot106_antenna_hidden_under_scoop() {
        let Some(all) = solids("PEUGOT106") else { return };
        let a = all.iter().find(|s| s.name == "PEUGOT106_BASE_A").expect("106 base");
        let hidden = hidden_triangles_always(a, true);
        assert_eq!(hidden.len(), 6, "{hidden:?}");
        assert!(hidden_triangles_always(a, false).is_empty());
        for s in all.iter().filter(|s| !s.name.starts_with("PEUGOT106_BASE_")) {
            assert!(hidden_triangles_always(s, true).is_empty(), "{}", s.name);
        }
    }

    #[test]
    fn wheel_shifts_front_only() {
        assert_eq!(wheel_shift_always("MUSTANGGT"), [-0.069, -0.069, 0.0, 0.0]);
        assert_eq!(wheel_shift_always("A3"), [-0.042, -0.042, 0.0, 0.0]);
        assert_eq!(wheel_shift_always("IMPREZAWRX"), [-0.044, -0.044, 0.0, 0.0]);
        assert_eq!(wheel_shift_always("PEUGOT106"), [-0.038, -0.038, 0.0, 0.0]);
        assert_eq!(wheel_shift_always("PEUGOT"), [-0.036, -0.036, 0.0, 0.0]);
        assert_eq!(wheel_shift_always("NAVIGATOR"), [-0.056, -0.056, 0.0, 0.0]);
        assert_eq!(wheel_shift_always("3000GT"), [0.0; 4]);
        assert_eq!(wheel_shift_always("350Z"), [0.0; 4]);
    }
}
