//! Chance nodes (`docs/MCTS_BOT_PLAN.md` 2.3) and the search's own random stream.
//!
//! A combat roll group (`Kind::Dice` with a threshold) is a chance node whose outcomes are its hit counts: `n + 1` classes, weights
//! `ti4_chance::search::hit_weights`, one representative answer each ([`hit_outcomes`]). Every other request (an `Index` pick: Spy,
//! Sanctions, Neuraloop, the Yssaril hero; dice without a threshold: an ability that reads the faces) is not branched on: it is answered
//! once with a face drawn from a [`SearchRng`], never from the game's streams.
//!
//! Caveat (from M1, not re-checked here): a rule that reads a face beyond the threshold (a 9-10 of the Jol-Nar flagship) sees the
//! representative's faces (the threshold for a hit, 1 for a miss) in a hit-count class. Use [`exact_outcomes`] or open-loop sampling where
//! that matters.

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use ti4_chance::{Faces, Kind, Request};

/// The search's own random stream: the answers to the chance requests it does not branch on, and any other draw a search makes. Seeded
/// by the caller (a search seed, a node path), independent of every game stream.
#[derive(Clone)]
pub struct SearchRng(ChaCha8Rng);

impl SearchRng {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self(ChaCha8Rng::seed_from_u64(seed ^ 0x5ea2_c4ed_7a11_0c6e))
    }

    /// A stream for `seed` and a node path (any words that name the node): the same node gets the same stream.
    #[must_use]
    pub fn at(seed: u64, path: &[u64]) -> Self {
        let mut h = seed ^ 0xcbf2_9ce4_8422_2325;
        for p in path {
            h = (h ^ p).wrapping_mul(0x0100_0000_01b3).rotate_left(29);
        }
        Self::new(h)
    }

    /// Faces for `req` (each die uniform and independent, as the game's streams draw them).
    pub fn sample(&mut self, req: &Request) -> Faces {
        ti4_chance::search::sample(req, &mut self.0)
    }

    /// A uniform index below `n` (`n > 0`).
    pub fn below(&mut self, n: usize) -> usize {
        use rand::Rng;
        self.0.random_range(0..n.max(1))
    }

    /// The generator itself.
    pub fn rng(&mut self) -> &mut ChaCha8Rng {
        &mut self.0
    }
}

/// Whether `req` is a chance node of the tree by default: a roll group with a hit threshold (the rules read its hit count).
#[must_use]
pub const fn is_hit_node(req: &Request) -> bool {
    matches!(req.kind, Kind::Dice { hits_on: Some(_), .. })
}

/// The hit-count classes of a roll group with a threshold: `(representative faces, probability)` for 0..=n hits (classes of probability
/// 0 left out). `None` for a request without a threshold.
#[must_use]
pub fn hit_outcomes(req: &Request) -> Option<Vec<(Faces, f64)>> {
    let Kind::Dice { n, sides, hits_on: Some(on) } = req.kind else { return None };
    if on <= 1 || on > sides {
        // Every die hits (or none can): one class.
        let face = if on <= 1 { sides.max(1) } else { 1 };
        let faces = Faces::from_slice(&vec![face; usize::from(n)])?;
        return Some(vec![(faces, 1.0)]);
    }
    let weights = ti4_chance::search::hit_weights(req)?;
    Some(weights.into_iter().enumerate().filter(|(_, w)| *w > 0.0).filter_map(|(k, w)| ti4_chance::search::faces_for_hits(req, k).map(|f| (f, w))).collect())
}

/// Every ordered answer and its probability, when there are at most `limit` (small requests: an `Index`, one or two dice).
#[must_use]
pub fn exact_outcomes(req: &Request, limit: usize) -> Option<Vec<(Faces, f64)>> {
    ti4_chance::search::outcomes(req, limit)
}

/// The hit count of `faces` for `req` (`None` without a threshold).
#[must_use]
pub fn hits(req: &Request, faces: &Faces) -> Option<usize> {
    match req.kind {
        Kind::Dice { hits_on: Some(on), .. } => Some(faces.hits(on)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_chance::Domain;

    #[test]
    fn hit_classes_sum_to_one_and_have_their_hit_count() {
        for (n, on) in [(1u8, 6u8), (3, 8), (5, 2), (2, 1), (2, 11)] {
            let req = Request::new(Domain::Dice, Kind::Dice { n, sides: 10, hits_on: Some(on) });
            let classes = hit_outcomes(&req).unwrap();
            let total: f64 = classes.iter().map(|(_, w)| w).sum();
            assert!((total - 1.0).abs() < 1e-9, "{n} dice on {on}: {total}");
            for (faces, _) in &classes {
                assert!(req.accepts(faces));
            }
            if (2..=10).contains(&on) {
                assert_eq!(classes.len(), usize::from(n) + 1);
                for (k, (faces, _)) in classes.iter().enumerate() {
                    assert_eq!(hits(&req, faces), Some(k));
                }
            }
        }
        assert!(hit_outcomes(&Request::new(Domain::Dice, Kind::Index { n: 4 })).is_none());
    }

    #[test]
    fn the_search_stream_is_seeded_and_answers_legally() {
        let req = Request::new(Domain::Dice, Kind::Dice { n: 6, sides: 10, hits_on: Some(5) });
        let (mut a, mut b) = (SearchRng::at(7, &[1, 2]), SearchRng::at(7, &[1, 2]));
        for _ in 0..20 {
            let f = a.sample(&req);
            assert!(req.accepts(&f));
            assert_eq!(f, b.sample(&req));
        }
        let mut c = SearchRng::at(7, &[2, 1]);
        let mut d = SearchRng::at(7, &[1, 2]);
        assert!((0..20).any(|_| c.sample(&req) != d.sample(&req)), "another node path, another stream");
    }
}
