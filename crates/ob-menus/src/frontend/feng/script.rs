//! An object's scripts on the FEng clock: [`FE_RATE`], [`animate`], [`attributes`], [`host_time`].

use glam::Quat;

use crate::formats::hash::bin_hash;
use crate::frontend::feng;
use crate::frontend::feng::Package;
use crate::frontend::feng::package_run::{PackageRun, apply, run_package};
use crate::frontend::feng::state::ScreenState;

/// The FEng clock's rate against real time (**verified**, 2026-11-01):
/// the front end's per-frame update 0x5662D0 (from 0x5663C0, the main
/// loop's 0x581499 / 0x581675) gives the package manager 0x558C10 the step
/// trunc(0x865194 × 1200) ms — the frame's seconds × 1200 (0x78AAA0), not
/// × 1000 — and 0x558DAB caps a step at 20 ms unless its second argument
/// is set (game state 0x8654A4 = 6 with a real dt 0x865150 > 0). So every
/// package script runs 1.2× faster than real time at 60 fps or more (the
/// cap is exactly 1/60 s × 1200); below 60 fps the cap slows it. **Observed**
/// under Wine (fastgpu, 120 fps): the credits' lines scroll 156 px/s
/// against the clone's 130 px/s at 1.0 with the same 39 px line spacing,
/// a line every 250 ms (the script's 300 ms). Correction (measured):
/// the step is truncated per frame — 20, 19, 20 ms at 60 fps
/// (1180 a second), 40 per 1/30 s scene frame — and 20 ms is not a cap but
/// the piece size out of a running world. The game runs that package clock
/// on its ticks (`nfsu2::game_clock`), the same at every display
/// rate, and hands the screens `Game::now()` = package ms / 1.2: every time
/// a screen gives `layout`, `animate` or `run_package` (now and its
/// scripts' starts) is in those front-end ms, so × 1.2 here is the package
/// clock exactly; `host_time` answers front-end ms. Defined with the clock
/// (`game_clock::FE_RATE`).
pub use crate::game_clock::FE_RATE;

/// An object's attributes at `now` (ms) from its own script alone (no
/// package messages): the screen's script for it or INIT from 0.
pub fn animate(o: &feng::Object, state: &ScreenState, now: f32) -> feng::Attributes {
    let head = match state.scripts.get(&o.hash) {
        Some(&(name, start)) => (start * FE_RATE, name),
        None => (0.0, bin_hash("INIT")),
    };
    replay(o, &[head], now * FE_RATE)
}

/// An object's attributes at `now` (ms) from its script history: each
/// (time, script) entry starts that script then (0x5178B0 / 0x51CF80 set
/// +0x54 and its time +0x10 to 0), and the one before it runs up to that
/// moment, so the fields the new script's tracks do not drive keep what
/// the old one left (only tracks write the object's state, 0x541DC0).
fn replay(o: &feng::Object, history: &[(f32, u32)], now: f32) -> feng::Attributes {
    let mut a = o.attributes;
    for (k, &(start, name)) in history.iter().enumerate() {
        // The first entry may start after `now` (a screen's script timed
        // later): it is then evaluated at its time 0.
        if k > 0 && start > now {
            break;
        }
        let until = history.get(k + 1).filter(|n| n.0 <= now).map_or(now, |n| n.0);
        run_chain(o, &mut a, name, (until - start).max(0.0));
    }
    a
}

/// Run script `name` on `a` for `t` ms (0x54CE70 per step): at or past
/// the length a chained script applies its end (0x541DC0) and goes on with
/// the chain (+0x18), the time carried over (t − length), re-basing its
/// tracks (0x517720); a zero-length script ends on its first step, so
/// INIT → HIDE hides at once (the packages' FADEIN responses bring such
/// objects back, see `run_package`). Without a chain the flags (+0x14 & 3)
/// pick: 0 holds (time = length + 1, not evaluated again), 1 loops (t mod
/// length, re-based), 2 runs t mod twice the length.
fn run_chain(o: &feng::Object, a: &mut feng::Attributes, mut name: u32, mut t: f32) {
    for _ in 0..8 {
        let Some(s) = o.scripts.iter().find(|s| s.name == name) else { return };
        let s = rebased(s, a);
        let ends = (s.length > 0 && t >= s.length as f32) || (s.length == 0 && s.chain.is_some());
        let time = if ends {
            match s.chain {
                // A script chained to itself loops.
                Some(next) if next == name && s.length > 0 => t % s.length as f32,
                Some(next) if next == name => 0.0,
                Some(next) => {
                    t -= s.length as f32;
                    apply(a, &s, s.length);
                    name = next;
                    continue;
                }
                None if s.flags & 3 == 1 && s.length > 0 => t % s.length as f32,
                None if s.flags & 3 == 2 && s.length > 0 => t % (2.0 * s.length as f32),
                None => s.length as f32,
            }
        } else {
            t
        };
        apply(a, &s, time as u32);
        return;
    }
}

/// 0x517720, run when a script starts on an object (0x5178B0, 0x51CF80, a
/// chain step or a loop in 0x54CE70): every track's "done" bit (+3 bit 7)
/// is cleared, and a track with interpolation 3 or 4 has its first key's
/// delta (the key list at +0x30) set to the object's current value minus
/// the track's base, so it starts where the object is (param 1 integer,
/// 2–4 float vectors, 5 a quaternion, 6 colour integers). The other list
// prov: open frontend.rebased
/// (when the byte at +0x2C is set, 0x4FE890) is **open**. The only shipped
/// interpolation-3 track is EA_TRAX_Jukebox F3EBDC50's colour.
fn rebased<'a>(s: &'a feng::Script, a: &feng::Attributes) -> std::borrow::Cow<'a, feng::Script> {
    let moves = |t: &feng::Track| matches!(t.interpolation, 3 | 4) && !t.keys.is_empty();
    if !s.tracks.iter().any(moves) {
        return std::borrow::Cow::Borrowed(s);
    }
    let mut s = s.clone();
    for t in s.tracks.iter_mut().filter(|t| moves(t)) {
        let cur: Vec<u32> = (0..t.base.len()).map(|i| a.words.get(t.offset + i).copied().unwrap_or(0)).collect();
        let f = f32::from_bits;
        let delta: Vec<u32> = match t.param {
            1 | 6 => cur.iter().zip(&t.base).map(|(&c, &b)| (c as i32).wrapping_sub(b as i32) as u32).collect(),
            5 if cur.len() >= 4 => {
                // delta · base = current, as `sample` composes (0x4FE6B0;
                // prov: inferred frontend.rebased-2
                // the operand order is **inferred** from that).
                let q = |w: &[u32]| Quat::from_xyzw(f(w[0]), f(w[1]), f(w[2]), f(w[3]));
                let d = q(&cur) * q(&t.base).conjugate();
                [d.x, d.y, d.z, d.w].iter().map(|v| v.to_bits()).collect()
            }
            _ => cur.iter().zip(&t.base).map(|(&c, &b)| (f(c) - f(b)).to_bits()).collect(),
        };
        t.keys[0].1 = delta;
    }
    std::borrow::Cow::Owned(s)
}

/// Every object's attributes at `now` (ms), package order: its script
/// history from `run_package` replayed.
pub fn attributes(package: &Package, state: &ScreenState, now: f32) -> Vec<feng::Attributes> {
    let run = run_package(package, state, now);
    // The history is in package time (`FE_RATE`).
    let now = now * FE_RATE;
    (0..package.objects.len()).map(|i| object_attributes(package, state, &run, i, now)).collect()
}

/// Object `i`'s attributes at package time `now` from `run` (one entry
/// of [`attributes`]; `layout` asks only for those it places).
pub(crate) fn object_attributes(
    package: &Package,
    state: &ScreenState,
    run: &PackageRun,
    i: usize,
    now: f32,
) -> feng::Attributes {
    let init = bin_hash("INIT");
    let (o, h) = (&package.objects[i], &run.history[i]);
    let loops = |c: u32| o.scripts.iter().any(|s| s.name == c && s.chain == Some(c));
    let waits = !state.chain_init
        && !state.chained.contains(&o.hash)
        && h.len() == 1
        && h[0].1 == init
        && o.scripts.iter().any(|s| s.name == init && s.length == 0 && s.chain.is_some_and(|c| !loops(c)));
    if waits {
        let mut a = o.attributes;
        if let Some(s) = o.scripts.iter().find(|s| s.name == init) {
            apply(&mut a, s, 0);
        }
        a
    } else {
        let mut a = replay(o, h, now);
        let current = h.iter().rev().find(|e| e.0 <= now).or(h.first()).map(|e| e.1);
        if let Some(&c) = state.base_color.get(&o.hash) {
            rebase_color(o, current, &mut a, c);
        }
        if let Some(&(name, c)) = state.script_color.get(&o.hash)
            && current == Some(name)
            && let Some(sc) =
                o.scripts.iter().find(|sc| sc.name == name && sc.tracks.iter().any(|t| t.param == 6 && t.offset == 0))
        {
            let mut zero = o.attributes;
            apply(&mut zero, sc, 0);
            for i in 0..4 {
                a.words[i] = (a.words[i] as i32).wrapping_add(c[i] as i32 - zero.words[i] as i32) as u32;
            }
        }
        a
    }
}

/// 0x51D800 → 0x517AD0: the colour (ARGB) becomes the base of the
/// object's scripts' colour tracks (param 6, stored B, G, R, A); the
/// running script's value moves by the change of its base. A script with
/// no colour track leaves the object's colour as it is.
fn rebase_color(o: &feng::Object, script: Option<u32>, a: &mut feng::Attributes, argb: u32) {
    let Some(s) = script.and_then(|n| o.scripts.iter().find(|s| s.name == n)) else { return };
    let Some(t) = s.tracks.iter().find(|t| t.param == 6 && t.offset == 0 && t.base.len() >= 4) else { return };
    let new = [argb & 0xFF, (argb >> 8) & 0xFF, (argb >> 16) & 0xFF, argb >> 24];
    for i in 0..4 {
        a.words[i] = (a.words[i] as i32).wrapping_add(new[i] as i32 - t.base[i] as i32) as u32;
    }
}

/// When the package's own scripts first send `message` to the screen's
/// code (a host event, see `PackageRun::host`), from its opening, ms.
pub fn host_time(package: &Package, message: u32) -> Option<f32> {
    run_package(package, &ScreenState::default(), 10_000.0).host.iter().find(|h| h.1 == message).map(|h| h.0 / FE_RATE)
}
