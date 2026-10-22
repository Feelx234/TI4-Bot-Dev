//! Hyperlane lanes: which systems the printed lanes of the hyperlane tiles join.
//!
//! The corpus only flags a tile `isHyperlane` (ids `83a`..`91b`, and `<face><degrees>` for the
//! five turned copies, e.g. `87a180`). The lane table below is the printed one of the nine
//! two-sided Prophecy of Kings tiles. It is the same table and the same rule as the web's
//! display-only network (`web/src/components/hyperlane/`, see `docs/HYPERLANE_NETWORK.md`).
//!
//! Rule: a ship that enters a hyperlane tile through an edge leaves it only through the other end
//! of a printed lane that starts on that edge (lanes that share an end are separate lanes), and
//! then enters the neighbour across that edge, which carries it on only if that tile has a lane
//! end on the shared edge. Two systems that such a chain joins are adjacent.
//!
//! Edges are numbered 0..5 in the order of `Hex::ring(1)` (edge `k` faces the hex at ring-1 slot
//! `k`), and a tile turned by `d` degrees has every edge shifted by `d / 60`.

use std::collections::{BTreeMap, BTreeSet};

use ti4_model::hex::Hex;

/// Ring-1 direction of each edge, as `(q, r)` offsets (`Hex::ring(1)` order).
const EDGE_DIRS: [(i32, i32); 6] = [(-1, 1), (0, 1), (1, 0), (1, -1), (0, -1), (-1, 0)];

/// The printed lanes of every face at rotation 0, as pairs of edges.
fn face_lanes(face: &str) -> Option<&'static [(u8, u8)]> {
    Some(match face {
        "83a" => &[(1, 4)],
        "83b" => &[(0, 2), (0, 3), (3, 5)],
        "84a" => &[(2, 5)],
        "84b" => &[(0, 3), (0, 4), (1, 3)],
        "85a" => &[(1, 5)],
        "85b" => &[(0, 2), (0, 3), (3, 5)],
        "86a" => &[(1, 5)],
        "86b" => &[(0, 3), (0, 4), (1, 3)],
        "87a" => &[(0, 2), (2, 4), (2, 5)],
        "87b" => &[(0, 2), (0, 3)],
        "88a" => &[(0, 4), (1, 4), (2, 4)],
        "88b" => &[(0, 2), (0, 3), (3, 5)],
        "89a" => &[(0, 2), (0, 4), (2, 4)],
        "89b" => &[(0, 3), (0, 4)],
        "90a" => &[(1, 5), (2, 4)],
        "90b" => &[(0, 3), (0, 4)],
        "91a" => &[(0, 3), (0, 4), (1, 3)],
        "91b" => &[(0, 2), (0, 3)],
        _ => return None,
    })
}

/// The lanes of a hyperlane tile id after its turn (`83a`, `83a120`, or the map-string spelling
/// `83a2`), as edge pairs. `None` for an id that is not one of the hyperlane tiles.
#[must_use]
pub fn lanes_of(id: &str) -> Option<Vec<(u8, u8)>> {
    let id = id.trim().to_ascii_lowercase();
    let digits_at = id.find(['a', 'b'])? + 1;
    let (face, tail) = id.split_at(digits_at);
    let turn: u8 = match tail.len() {
        0 => 0,
        1 => tail.parse().ok().filter(|t| *t <= 5)?,
        _ => {
            let degrees: u32 = tail.parse().ok()?;
            if degrees % 60 != 0 || degrees >= 360 {
                return None;
            }
            u8::try_from(degrees / 60).ok()?
        }
    };
    let base = face_lanes(face)?;
    Some(
        base.iter()
            .map(|&(a, b)| ((a + turn) % 6, (b + turn) % 6))
            .collect(),
    )
}

fn step(hex: Hex, edge: u8) -> Hex {
    let (dq, dr) = EDGE_DIRS[usize::from(edge)];
    Hex::new(hex.q + dq, hex.r + dr)
}

/// For every ordinary system that a chain of lanes joins to another, the systems it is joined to.
/// `placement` is the whole board (hex to tile id); `is_hyperlane` says which of them are lane
/// tiles. Both directions are listed. A system is never joined to itself.
#[must_use]
pub fn links<F: Fn(&str) -> bool>(
    placement: &BTreeMap<Hex, String>,
    is_hyperlane: F,
) -> BTreeMap<String, BTreeSet<String>> {
    let lanes: BTreeMap<Hex, Vec<(u8, u8)>> = placement
        .iter()
        .filter(|(_, id)| is_hyperlane(id))
        .filter_map(|(hex, id)| lanes_of(id).map(|l| (*hex, l)))
        .collect();
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    if lanes.is_empty() {
        return out;
    }
    for (&start, from) in placement {
        if lanes.contains_key(&start) {
            continue;
        }
        for edge in 0..6u8 {
            let first = step(start, edge);
            let entry = (edge + 3) % 6;
            if lanes
                .get(&first)
                .is_some_and(|l| l.iter().any(|&(a, b)| a == entry || b == entry))
            {
                let mut seen = BTreeSet::new();
                walk(&lanes, placement, first, entry, &mut seen, &mut |hex| {
                    if hex != start {
                        if let Some(to) = placement.get(&hex) {
                            out.entry(from.clone()).or_default().insert(to.clone());
                            out.entry(to.clone()).or_default().insert(from.clone());
                        }
                    }
                });
            }
        }
    }
    out
}

/// Arrived at the lane tile `at` through its port `entry`: follow every lane that starts there.
/// `found` is called with each ordinary system a lane leaves into.
fn walk(
    lanes: &BTreeMap<Hex, Vec<(u8, u8)>>,
    placement: &BTreeMap<Hex, String>,
    at: Hex,
    entry: u8,
    seen: &mut BTreeSet<(Hex, u8)>,
    found: &mut impl FnMut(Hex),
) {
    if !seen.insert((at, entry)) {
        return;
    }
    for &(a, b) in &lanes[&at] {
        if a != entry && b != entry {
            continue;
        }
        let out = if a == entry { b } else { a };
        if seen.contains(&(at, out)) {
            continue;
        }
        seen.insert((at, out));
        let next = step(at, out);
        let back = (out + 3) % 6;
        if let Some(neighbour) = lanes.get(&next) {
            if neighbour.iter().any(|&(x, y)| x == back || y == back) {
                walk(lanes, placement, next, back, seen, found);
            }
        } else if placement.contains_key(&next) {
            found(next);
        }
        seen.remove(&(at, out));
    }
    seen.remove(&(at, entry));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_shifts_every_edge() {
        assert_eq!(lanes_of("83a"), Some(vec![(1, 4)]));
        assert_eq!(lanes_of("83a120"), Some(vec![(3, 0)]));
        assert_eq!(lanes_of("83A2"), Some(vec![(3, 0)]), "map-string spelling");
        assert_eq!(lanes_of("18"), None);
        assert_eq!(lanes_of("83a90"), None);
    }

    #[test]
    fn a_straight_lane_joins_the_systems_across_it_and_chains_continue() {
        // 83a joins edges 1 and 4 (the hex at slot 1 and slot 4 of its ring): (0,1) and (0,-1).
        let mut placement = BTreeMap::new();
        placement.insert(Hex::new(0, 0), "83a".to_owned());
        placement.insert(Hex::new(0, 1), "s1".to_owned());
        placement.insert(Hex::new(0, -1), "s2".to_owned());
        placement.insert(Hex::new(1, 0), "s3".to_owned());
        let joined = links(&placement, |id| id == "83a");
        assert_eq!(
            joined.get("s1"),
            Some(&BTreeSet::from(["s2".to_owned()])),
            "s3 sits on an edge the lane does not touch"
        );
        assert_eq!(joined.get("s2"), Some(&BTreeSet::from(["s1".to_owned()])));
        assert!(!joined.contains_key("s3"));
    }
}
