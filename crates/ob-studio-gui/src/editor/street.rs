//! Edit ▸ Select street: the props along the street the first selected prop stands on, up to the junctions at
//! both ends, as one selection (then Duplicate, Move or Delete work on the whole street at once). A street is the
//! road segment nearest the prop plus the segments that follow it while the road does not branch: at a junction
//! (an end with more than one way on) the street stops, and so does a copy made from it.

use glam::Vec2;
use std::collections::HashSet;

use super::state::EditorState;
use crate::formats::roads::RoadNet;

/// How far from the road's centre line a prop still belongs to the street (metres).
pub const REACH: f32 = 14.0;
/// The most segments one street holds (a loop road ends here, not never).
const MOST_SEGMENTS: usize = 64;

/// The segments of the street through segment `start`, in no particular order: the segment itself and those the
/// road leads on to at each end while that end has exactly one way on (a junction has more).
pub fn chain(net: &RoadNet, start: usize) -> Vec<usize> {
    let mut seen: HashSet<usize> = HashSet::from([start]);
    let mut out = vec![start];
    for first_end in [false, true] {
        // Leave `at` by its end `end` (0: the first node's, 1: the last's).
        let (mut at, mut end) = (start, usize::from(first_end));
        while out.len() < MOST_SEGMENTS {
            let Some(exits) = net.segments.get(at).map(|s| &s.exits[end]) else { break };
            let [(next, dir)] = exits.as_slice() else { break };
            // The next segment's own end must be a plain join too (more ways in there is a junction).
            let joined = net.segments.get(*next).is_some_and(|n| n.exits[usize::from(*dir != 0)].len() == 1);
            if !joined || !seen.insert(*next) {
                break;
            }
            out.push(*next);
            // Entered at the start (direction 0): the road goes on from the last node.
            (at, end) = (*next, usize::from(*dir == 0));
        }
    }
    out
}

/// The squared distance from `p` to the polyline of segment `k`.
fn near(net: &RoadNet, k: usize, p: Vec2) -> f32 {
    let Some(s) = net.segments.get(k) else { return f32::INFINITY };
    let end = s.end.min(net.nodes.len());
    let mut best = f32::INFINITY;
    if end - s.first.min(end) == 1 {
        best = net.nodes[s.first].0.distance_squared(p);
    }
    for n in s.first..end.saturating_sub(1) {
        let (a, b) = (net.nodes[n].0, net.nodes[n + 1].0);
        let ab = b - a;
        let t = if ab.length_squared() > 1e-9 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
        best = best.min((a + ab * t).distance_squared(p));
    }
    best
}

/// The placements along the street of `net` that passes nearest `from`.
pub fn along(s: &EditorState, net: &RoadNet, from: Vec2) -> Vec<usize> {
    let Some(rp) = crate::tools::map::race::snap(net, from) else { return Vec::new() };
    let segments = chain(net, rp.segment);
    let hidden = crate::world::HIDDEN_FLAG;
    (0..s.session.world.placements.len())
        .filter(|&i| s.session.world.placements[i].flags & hidden == 0)
        .filter(|&i| {
            let Some((p, _)) = crate::tools::map::edit::placement_parts(&s.session.world, i) else { return false };
            segments.iter().any(|&k| near(net, k, p.truncate()) <= REACH * REACH)
        })
        .collect()
}

/// Edit ▸ Select street: the selection becomes the props along the street under the first selected prop.
pub fn select_street(s: &mut EditorState) {
    use crate::studio::words;
    let Some(&first) = s.selection.first() else {
        s.status = words::NO_STREET_HERE.into();
        return;
    };
    let Some((p, _)) = crate::tools::map::edit::placement_parts(&s.session.world, first) else { return };
    let from = p.truncate();
    if let Some(t) = s.race.and_then(|k| s.session.map.races.get(k)).map(|r| r.track)
        && !s.nets.contains_key(&t)
    {
        let net = s.assets.road_net(t);
        s.nets.insert(t, net);
    }
    let found = match super::roads::race_network(s) {
        Some((_, net)) => along(s, net, from),
        None => match super::roads::overview_net(s.assets.as_ref(), &s.session.region) {
            Some((_, net)) => along(s, &net, from),
            None => Vec::new(),
        },
    };
    if found.is_empty() {
        s.status = words::NO_STREET_HERE.into();
    } else {
        s.selection = found;
        s.status = words::STREET_SELECTED.into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::roads::Segment;

    fn seg(first: usize, end: usize, exits: [Vec<(usize, u8)>; 2]) -> Segment {
        Segment {
            first,
            end,
            length: 0.0,
            spline_length: 0.0,
            exits,
            barriers: Vec::new(),
            tracks: Default::default(),
        }
    }

    /// Four segments in a row along x (0..100, 100..200, 200..300 and 300..400) where the end of the second
    /// branches into two ways on.
    pub fn street_with_a_junction() -> RoadNet {
        let mut net = RoadNet::default();
        for x in [0.0, 100.0, 100.0, 200.0, 200.0, 300.0, 300.0, 400.0] {
            net.nodes.push((Vec2::new(x, 0.0), 0));
        }
        net.segments = vec![
            seg(0, 2, [vec![], vec![(1, 0)]]),
            seg(2, 4, [vec![(0, 1)], vec![(2, 0), (3, 0)]]),
            seg(4, 6, [vec![(1, 1)], vec![]]),
            seg(6, 8, [vec![(1, 1)], vec![]]),
        ];
        net
    }

    #[test]
    fn a_street_stops_at_the_junction() {
        let net = street_with_a_junction();
        let mut c = chain(&net, 0);
        c.sort();
        assert_eq!(c, vec![0, 1], "the street ends where the road branches");
        let mut c = chain(&net, 1);
        c.sort();
        assert_eq!(c, vec![0, 1]);
        assert_eq!(chain(&net, 2), vec![2], "past the junction the branch is its own street");
    }

    #[test]
    fn select_street_stops_at_the_junction_and_a_copy_with_it() {
        use crate::editor::props::tests::state;
        use crate::tools::map::edit;
        use glam::{Mat4, Vec3};
        let mut s = state();
        let put = |s: &mut EditorState, x: f32, y: f32| {
            s.session.world.add_placement("XC_CONE_01", Mat4::from_translation(Vec3::new(x, y, 0.0))).unwrap()
        };
        let past_junction = put(&mut s, 250.0, 0.0);
        let off_road = put(&mut s, 150.0, 60.0);
        s.nets.insert(1, Some(street_with_a_junction()));
        s.selection = vec![0];
        select_street(&mut s);
        let mut got = s.selection.clone();
        got.sort();
        assert!(got.contains(&0) && got.contains(&1) && got.contains(&2), "{got:?}");
        assert!(!got.contains(&past_junction) && !got.contains(&off_road), "{got:?}");
        assert_eq!(s.status, crate::studio::words::STREET_SELECTED);
        // Copying it copies exactly those.
        crate::editor::props::duplicate_selection(&mut s, None);
        assert_eq!(s.selection.len(), got.len(), "the copies are the street's props");
        assert!(s.selection.iter().all(|&i| edit::placement_parts(&s.session.world, i).unwrap().0.x < 200.0));
        // Nothing selected: a plain word, nothing changes.
        s.selection.clear();
        select_street(&mut s);
        assert!(s.selection.is_empty());
        assert_eq!(s.status, crate::studio::words::NO_STREET_HERE);
    }
}
