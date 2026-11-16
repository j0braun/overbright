//! Goal 5 target search (0x41B820 → 0x41B230, mode 1).
//!
//! This component reproduces the bounded native search; it is deliberately not
//! wired into the controller until its think cadence and cache lifecycle are
//! established. See the maintainer's private notes for capture limits.
use crate::formats::ai_routes::{ControlPoint, RouteFile};
use glam::{Vec2, Vec3};
use std::collections::VecDeque;

type Node = (usize, usize);

#[derive(Debug)]
pub struct SearchResult {
    pub record: usize,
    pub point: usize,
    pub forward: bool,
    pub spent: f32,
    pub decisions: Vec<(usize, bool)>,
}

#[derive(Clone)]
struct State {
    node: Node,
    forward: bool,
    remaining: f32,
    decisions: Vec<(usize, bool)>,
}

fn next(f: &RouteFile, (r, c): Node, forward: bool) -> Option<Node> {
    let c = if forward {
        c.checked_add(1)?
    } else {
        c.checked_sub(1)?
    };
    (c < f.records[r].points.len()).then_some((r, c))
}

fn along(p: &ControlPoint, q: Vec2) -> f64 {
    (q.x as f64 - p.position.x as f64) * p.tangent.x as f64
        + (q.y as f64 - p.position.y as f64) * p.tangent.y as f64
}

fn inside(f: &RouteFile, (r, c): Node, q: Vec3, inclusive: bool) -> bool {
    let p = &f.records[r].points[c];
    let side = (q.x as f64 - p.position.x as f64) * p.tangent.y as f64
        - (q.y as f64 - p.position.y as f64) * p.tangent.x as f64;
    let a = along(p, q.truncate());
    let back = f.records[r].points[c.saturating_sub(1)].length as f64;
    let lateral = if side > 0.0 {
        side - p.half_width[1] as f64
    } else {
        -side - p.half_width[0] as f64
    };
    let longitudinal = if a > 0.0 {
        a - p.length as f64
    } else {
        -a - back
    };
    (q.z - p.position.z).abs() < 4.0
        && if inclusive {
            lateral <= 0.0 && longitudinal <= 0.0
        } else {
            lateral < 0.0 && longitudinal < 0.0
        }
}

fn push(queue: &mut VecDeque<State>, state: State, front: bool) {
    // Native full queues discard the new entry, not an older candidate.
    if queue.len() < 100 {
        if front {
            queue.push_front(state)
        } else {
            queue.push_back(state)
        }
    }
}

fn pop(queue: &mut VecDeque<State>, pops: &mut usize, limit: usize) -> Option<State> {
    *pops += 1;
    if *pops > limit {
        None
    } else {
        queue.pop_front()
    }
}

fn walk(
    f: &RouteFile,
    origin: Node,
    forward: bool,
    target: Vec3,
    budget: f32,
    race_built: bool,
    candidates: usize,
) -> Option<State> {
    let mut queue = VecDeque::with_capacity(100);
    let mut pops = 0;
    let mut state = State {
        node: origin,
        forward,
        remaining: budget
            + if forward {
                0.0
            } else {
                f.records[origin.0].points[origin.1].length
            },
        decisions: Vec::new(),
    };
    for _ in 0..candidates.saturating_mul(1024) {
        let p = &f.records[state.node.0].points[state.node.1];
        state.remaining -= p.length;
        // Native checks containment before rejecting exhausted distance.
        if inside(f, state.node, target, false) {
            return Some(state);
        }
        if state.remaining < 0.0 {
            state = pop(&mut queue, &mut pops, f.junctions.len())?;
            continue;
        }
        let mut advance = true;
        let mut branched = false;
        let junctions: Vec<_> = f.junctions_at(p).collect();
        for (slot, &ji) in junctions.iter().enumerate() {
            let j = &f.junctions[ji];
            if j.direction() == state.forward {
                continue;
            }
            advance = false;
            if j.kind() == 3 {
                state.remaining = -1.0;
                break;
            }
            if p.flags & (1 << 11) != 0 && matches!(j.kind(), 5 | 7 | 10) {
                advance = true;
                continue;
            }
            let (rid, cp) = j.target();
            let r = f.record(rid)?;
            let c = usize::try_from(cp).ok()?;
            let dest = f.records[r].points.get(c)?;
            let remaining = (p.length as f64 + state.remaining as f64
                - along(p, dest.position.truncate())) as f32;
            if p.flags & (1 << 11) != 0 && j.kind() == 2 {
                state.node = (r, c);
                state.forward ^= j.flips();
                state.remaining = remaining;
                break;
            }
            let mut branch = state.clone();
            branch.node = (r, c);
            branch.forward ^= j.flips();
            branch.remaining = remaining;
            if branch.decisions.len() < 15 {
                branch.decisions.push((ji, true));
            }
            if state.decisions.len() < 15 {
                state.decisions.push((ji, false));
            }
            push(&mut queue, branch, !race_built || j.kind() == 0);
            if slot + 1 == junctions.len() {
                if let Some(node) = next(f, state.node, state.forward) {
                    let mut straight = state.clone();
                    straight.node = node;
                    push(&mut queue, straight, true);
                }
            }
            branched = true;
        }
        let mut node = (!branched).then_some(state.node);
        if advance {
            node = node.and_then(|n| next(f, n, state.forward));
        }
        // The mask uses the original direction, even after a flipping link.
        let mask = if forward {
            (1 << 11) | (1 << 25) | (1 << 27)
        } else {
            (1 << 18) | (1 << 26) | (1 << 28)
        };
        if let Some(node) = node.filter(|&(r, c)| f.records[r].points[c].flags & mask != 0) {
            state.node = node;
        } else {
            state = pop(&mut queue, &mut pops, f.junctions.len())?;
        }
    }
    None
}

/// Native goal-5 mode: at most six successful origins, minimum spent distance.
/// `routes` must already contain the native post-load flags and junction order.
/// `race_built` is the traversal global, not the current race's type number.
pub fn search(
    routes: &RouteFile,
    position: Vec3,
    direction: Vec2,
    target: Vec3,
    budget: f32,
    race_built: bool,
) -> Option<SearchResult> {
    let candidates = routes.records.iter().filter(|r| r.is_race()).count();
    let mut best: Option<SearchResult> = None;
    let mut successes = 0;
    for (r, record) in routes
        .records
        .iter()
        .enumerate()
        .filter(|(_, r)| r.is_race())
    {
        let [x0, y0, x1, y1] = record.bounds;
        if position.x < x0 || position.x > x1 || position.y < y0 || position.y > y1 {
            continue;
        }
        let mut c = 0;
        while c < record.points.len() {
            let p = &record.points[c];
            if inside(routes, (r, c), position, true) {
                let forward = p.tangent.dot(direction) > 0.0;
                if let Some(found) = walk(
                    routes,
                    (r, c),
                    forward,
                    target,
                    budget,
                    race_built,
                    candidates,
                ) {
                    let result = SearchResult {
                        record: r,
                        point: c,
                        forward,
                        spent: budget - found.remaining,
                        decisions: found.decisions,
                    };
                    successes += 1;
                    if best.as_ref().is_none_or(|old| result.spent < old.spent) {
                        best = Some(result);
                    }
                }
                // Stop sampling this record even when its traversal failed.
                break;
            }
            let dx = p.position.x as f64 - position.x as f64;
            let dy = p.position.y as f64 - position.y as f64;
            c += ((dx.hypot(dy) * 0.02f32 as f64) as usize).max(1);
        }
        if successes == 6 {
            break;
        }
    }
    best
}
