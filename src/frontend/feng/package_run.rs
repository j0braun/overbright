//! The FEng runtime over one package: [`run_package`] (scripts, timed events, the message pump).

use crate::formats::hash::bin_hash;
use crate::frontend::feng;
use crate::frontend::feng::Package;
use crate::frontend::feng::dim::DIM;
use crate::frontend::feng::layout::tree::tree_index;
use crate::frontend::feng::script::FE_RATE;
use crate::frontend::feng::state::ScreenState;

/// What a package's own scripts and message responses did by `now`
/// (package ms, see `FE_RATE`).
#[derive(Clone, Debug, Default)]
pub struct PackageRun {
    /// Per object (package order): the scripts it was given, (time ms,
    /// script name), in order; see `replay`.
    pub history: Vec<History>,
    /// Messages for the screen's code (an event or response with target
    /// −1 or −5, the host callbacks +0x18 / +0x1C of 0x541FD0 and
    /// 0x534C80), with their time (ms).
    pub host: Vec<(f32, u32)>,
}

/// An object's script history in [`PackageRun`]: (time ms, script name)
/// entries in order, the first few held inline (most objects only ever
/// have their first: a run of a package made one allocation per object
/// each frame). Reads as a slice.
#[derive(Clone, Debug)]
pub enum History {
    Inline(u8, [(f32, u32); 3]),
    Heap(Vec<(f32, u32)>),
}

impl History {
    pub(crate) fn one(first: (f32, u32)) -> History {
        History::Inline(1, [first, (0.0, 0), (0.0, 0)])
    }

    pub fn push(&mut self, entry: (f32, u32)) {
        match self {
            History::Inline(n, items) if (*n as usize) < items.len() => {
                items[*n as usize] = entry;
                *n += 1;
            }
            History::Inline(n, items) => {
                let mut v = items[..*n as usize].to_vec();
                v.push(entry);
                *self = History::Heap(v);
            }
            History::Heap(v) => v.push(entry),
        }
    }
}

impl std::ops::Deref for History {
    type Target = [(f32, u32)];
    fn deref(&self) -> &[(f32, u32)] {
        match self {
            History::Inline(n, items) => &items[..*n as usize],
            History::Heap(v) => v,
        }
    }
}

/// Where a queued message goes (0x541FD0, by the node's value).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Route {
    /// 0: every object in every package with a response for it (and the
    /// packages' own lists); −2 and −4 (this package) reach the same
    /// objects here, the clone running one package at a time.
    Broadcast,
    /// An object (0x517FE0 resolves a target id, `OP` +0, to it).
    Object(usize),
    /// −1 / −5: the screen's code.
    Host,
}

/// The FEng runtime over one package from its opening (0 ms) to `now`:
/// every object starts INIT (the reader sets +0x54, 0x5515F0) unless the
/// screen's code set its script (`ScreenState::scripts`); scripts run as
/// `run_chain` does and their timed messages (`EV`, 0x534C80) fire as the
/// script's time passes them (each step fires the events in [old time, new
/// time), the end included when the new time is the length: a zero-length
/// INIT's events fire on the first step, a loop's every cycle); the
/// queued messages are pumped after the step (0x558D89 → 0x541FD0) to the
/// objects' `MsgR` nodes (0x5367F0 → 0x52C7D0). So the packages' "Event
/// Handler" (0x47FF4E7C, in most menus) INIT → CALL broadcasts 0xBDFD2DF9
/// at 0 ms, which the menu objects answer with their FADEIN script, and
/// sends 0x35F8620B to the screen at 180 ms (IconScrollerMenu 0x543D50 sets
/// its ready byte +0x130 then). The game steps by at most 20 ms (0x558D89);
/// here a message lands at its exact time.
pub fn run_package(package: &Package, state: &ScreenState, now: f32) -> PackageRun {
    use std::cmp::Reverse;
    // Real ms in, package ms (`FE_RATE`) within and in the result.
    let now = now * FE_RATE;
    use std::collections::BinaryHeap;
    let objects = &package.objects;
    let init = bin_hash("INIT");
    // By guid, from the package's cached index (`tree_index`).
    let index = tree_index(package);
    let by_guid = &index.by_guid;
    let mut run = PackageRun { history: Vec::with_capacity(objects.len()), host: Vec::new() };
    // A screen's script for an object holds from its start; package
    // messages replace it only after that.
    let mut fixed = vec![f32::NEG_INFINITY; objects.len()];
    for (i, o) in objects.iter().enumerate() {
        run.history.push(History::one(match state.scripts.get(&o.hash) {
            Some(&(name, start)) => {
                let start = start * FE_RATE;
                fixed[i] = start;
                (start, name)
            }
            None => (0.0, init),
        }));
    }
    if now < 0.0 {
        return run;
    }
    // Timed messages still to fire: (time, object, event), ordered by time
    // then object then event; `generation` drops those of a replaced script.
    let mut generation = vec![0u32; objects.len()];
    let mut heap: BinaryHeap<Reverse<(u32, usize, usize, u32, u32, u32)>> = BinaryHeap::new();
    let key = |t: f32| (t.max(0.0) * 16.0) as u32;
    const CAP: usize = 4096;
    let schedule = |heap: &mut BinaryHeap<Reverse<(u32, usize, usize, u32, u32, u32)>>,
                    i: usize,
                    gen_: u32,
                    from: f32,
                    start: f32,
                    name: u32| {
        // Walk the chain from `start`, collecting events at or after `from`.
        let o = &objects[i];
        let (mut name, mut at) = (name, start);
        let mut n = 0;
        for _ in 0..8 {
            let Some(s) = o.scripts.iter().find(|s| s.name == name) else { return };
            let len = s.length as f32;
            let cycles = match s.chain {
                Some(next) if next == name => s.length > 0,
                None => s.flags & 3 == 1 && s.length > 0,
                _ => false,
            };
            let mut base = at;
            // An event-less script pushes nothing: skip its cycle walk (which
            // grew with session time for looping scripts). Scripts with
            // events keep the repeated f32 adds; a jump-ahead would round
            // differently.
            while !s.events.is_empty() {
                for (k, e) in s.events.iter().enumerate() {
                    let t = base + e.time as f32;
                    if e.time <= s.length && t >= from && t <= now {
                        heap.push(Reverse((key(t), i, k, e.message, e.target, gen_)));
                        n += 1;
                    }
                }
                if !cycles || base + len > now || n >= CAP {
                    break;
                }
                base += len;
            }
            match s.chain {
                Some(next) if next != name && at + len <= now => {
                    at += len;
                    name = next;
                }
                _ => return,
            }
        }
    };
    for i in 0..objects.len() {
        let (start, name) = run.history[i][0];
        schedule(&mut heap, i, 0, start.max(0.0), start, name);
    }
    // Scripts the screen's code played on objects after their INIT.
    for &(object, name, start) in &state.played {
        let t = (start * FE_RATE).max(0.0);
        if t > now {
            continue;
        }
        for (i, o) in objects.iter().enumerate() {
            if o.hash == object && o.scripts.iter().any(|s| s.name == name) {
                generation[i] = generation[i].wrapping_add(1);
                fixed[i] = fixed[i].max(t);
                run.history[i].push((t, name));
                schedule(&mut heap, i, generation[i], t, t, name);
            }
        }
    }
    // A dialog over this screen started its DIM (see [`with_dim`]): the
    // named objects run their DIM script (0x9E99) from then on, after
    // what the screen had given them.
    if let Some(dim) = DIM.with(|d| d.borrow().clone())
        && package.name.trim_end_matches(".fng").eq_ignore_ascii_case(dim.package)
    {
        let t = (now - dim.age_ms * FE_RATE).max(0.0);
        for (i, o) in objects.iter().enumerate() {
            if dim.objects.contains(&o.hash) && o.scripts.iter().any(|s| s.name == dim.script) {
                generation[i] = generation[i].wrapping_add(1);
                fixed[i] = fixed[i].max(t);
                run.history[i].push((t, dim.script));
                schedule(&mut heap, i, generation[i], t, t, dim.script);
            }
        }
        // The dialog closed: the same objects run UNDIM from then on.
        if let Some(undim) = dim.undim_ms {
            let t2 = (now - undim * FE_RATE).max(t);
            for (i, o) in objects.iter().enumerate() {
                if dim.undim_objects.contains(&o.hash) && o.scripts.iter().any(|s| s.name == dim.unscript) {
                    generation[i] = generation[i].wrapping_add(1);
                    fixed[i] = fixed[i].max(t2);
                    run.history[i].push((t2, dim.unscript));
                    schedule(&mut heap, i, generation[i], t2, t2, dim.unscript);
                }
            }
        }
    }
    // The screen's focus changes run after the objects' step at their
    // time (they come from its handlers, called from the pump).
    for (k, &(t, _)) in state.focus.iter().enumerate() {
        let t = t * FE_RATE;
        if t <= now {
            heap.push(Reverse((key(t), usize::MAX, k, 0, 0, 0)));
        }
    }
    let mut focused: Option<u32> = None;
    // The script active on object `i` at `t` (0x300 / 0x301 test +0x54's
    // name, +0x30).
    let active = |history: &[(f32, u32)], i: usize, t: f32| -> u32 {
        let &(start, mut name) = history.iter().rev().find(|h| h.0 <= t).unwrap_or(&history[0]);
        let o = &objects[i];
        let mut at = start;
        for _ in 0..8 {
            let Some(s) = o.scripts.iter().find(|s| s.name == name) else { break };
            match s.chain {
                Some(next) if next != name && (at + s.length as f32 <= t) => {
                    at += s.length as f32;
                    name = next;
                }
                _ => break,
            }
        }
        name
    };
    let mut fired = 0;
    while let Some(Reverse((tk, i, k, message, target, tag))) = heap.pop() {
        // Stale: the object's script changed since this was scheduled.
        if i != usize::MAX && tag != generation[i] {
            continue;
        }
        fired += 1;
        if fired > CAP {
            break;
        }
        let t = tk as f32 / 16.0;
        // 0x534C80: the event's target picks the route.
        let mut queue: std::collections::VecDeque<(u32, Route, usize)> = std::collections::VecDeque::new();
        if i == usize::MAX {
            let to = state.focus[k].1;
            let each = |h: u32| (0..objects.len()).filter(move |&j| objects[j].hash == h);
            if let Some(old) = focused.filter(|&o| o != to) {
                queue.extend(each(old).map(|j| (0x55D1_E635, Route::Object(j), j)));
            }
            queue.extend(each(to).map(|j| (0xABC0_8912, Route::Object(j), j)));
            focused = Some(to);
        }
        match if i == usize::MAX { 0xFFFF_FFFA } else { target } {
            0xFFFF_FFFF | 0xFFFF_FFFB => run.host.push((t, message)),
            // −6 sets the manager's input flag; focus (0x1B3909AA, 0x52B3B0)
            // is not modelled.
            0xFFFF_FFFA => {}
            _ if message == 0x1B39_09AA => {}
            0 | 0xFFFF_FFFC | 0xFFFF_FFFE => queue.push_back((message, Route::Broadcast, i)),
            id => {
                if let Some(&j) = by_guid.get(&id) {
                    queue.push_back((message, Route::Object(j), i));
                }
            }
        }
        // The pump (0x541FD0), first in first out; responses queue more.
        let mut pumped = 0;
        while let Some((message, route, _from)) = queue.pop_front() {
            pumped += 1;
            if pumped > 256 {
                break;
            }
            let targets: Vec<usize> = match route {
                Route::Host => {
                    run.host.push((t, message));
                    continue;
                }
                Route::Object(j) => vec![j],
                Route::Broadcast => {
                    (0..objects.len()).filter(|&j| objects[j].messages.iter().any(|n| n.message == message)).collect()
                }
            };
            for j in targets {
                // 0x5367F0: the first node for the message.
                let Some(node) = objects[j].messages.iter().find(|n| n.message == message) else { continue };
                let mut e = 0;
                while e < node.entries.len() {
                    let [kind, a1, a2] = node.entries[e];
                    match kind {
                        // Set the script named by arg 1, time 0 (0x5046B0 →
                        // 0x5178B0).
                        0 if objects[j].scripts.iter().any(|s| s.name == a1) && t > fixed[j] => {
                            generation[j] = generation[j].wrapping_add(1);
                            run.history[j].push((t, a1));
                            schedule(&mut heap, j, generation[j], t, t, a1);
                        }
                        1 => {
                            let route = match a2 {
                                0xFFFF_FFFF => Route::Host,
                                0 | 0xFFFF_FFFC => Route::Broadcast,
                                id => match by_guid.get(&id) {
                                    Some(&k) => Route::Object(k),
                                    None => Route::Broadcast,
                                },
                            };
                            queue.push_back((a1, route, j));
                        }
                        2 | 3 => queue.push_back((a1, Route::Host, j)),
                        // 0x300 runs the block when the object's script is
                        // arg 1, 0x301 when it is not; 0x500 jumps past the
                        // else block (0x504490).
                        0x300 if active(&run.history[j], j, t) != a1 => e = skip_block(&node.entries, e),
                        0x301 if active(&run.history[j], j, t) == a1 => e = skip_block(&node.entries, e),
                        0x500 => e = skip_block(&node.entries, e),
                        _ => {}
                    }
                    e += 1;
                }
            }
        }
    }
    run
}

/// 0x504490: from the entry after `at`, count nested 0x300 / 0x301 (+1)
/// and 0x501 (−1); stop at the first 0x500 or 0x501 at depth 1. The
/// dispatcher resumes after the entry returned.
fn skip_block(entries: &[[u32; 3]], at: usize) -> usize {
    let mut depth = 1;
    let mut i = at + 1;
    while i < entries.len() {
        match entries[i][0] {
            0x300 | 0x301 => depth += 1,
            0x500 | 0x501 if depth == 1 => return i,
            0x501 => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    i
}

pub(crate) fn apply(a: &mut feng::Attributes, s: &feng::Script, t: u32) {
    for track in &s.tracks {
        // The value on the stack (tracks hold at most a few words).
        let mut buf = [0u32; 8];
        let heap;
        let v: &[u32] = if track.base.len() <= buf.len() {
            let v = &mut buf[..track.base.len()];
            v.copy_from_slice(&track.base);
            feng::sample_into(track, t, v);
            v
        } else {
            heap = feng::sample(track, t);
            &heap
        };
        for (i, w) in v.iter().enumerate() {
            if track.offset + i < a.words.len() {
                a.words[track.offset + i] = *w;
                a.len = a.len.max(track.offset + i + 1);
            }
        }
    }
}
