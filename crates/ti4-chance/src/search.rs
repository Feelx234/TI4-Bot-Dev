//! Chance nodes for a tree search (feature `search`; not in the browser build).
//!
//! A search that reaches an `Ask::Chance(req)` either samples an answer or enumerates the outcomes with their
//! weights. Both work from the [`Request`] alone, with the same per-die model as [`crate::answer`]: each face is
//! uniform in `1..=sides` and independent. Sampling uses a caller's generator, never the game's streams, so a sampled
//! branch cannot read the real future dice.

use rand::Rng;

use crate::request::{Faces, Kind, Request};

/// Faces for `req` from `rng`, one die per face in order (the same draw as [`crate::answer_mut`] on a stream).
///
/// # Panics
/// On a request [`crate::answer_mut`] refuses for its size (more than 15 dice, no sides).
pub fn sample<R: Rng + ?Sized>(req: &Request, rng: &mut R) -> Faces {
    let sides = u32::from(req.sides());
    assert!(req.len() <= crate::MAX_FACES && (sides > 0 || req.is_empty()), "ti4-chance: unanswerable {req:?}");
    let mut faces = Faces::default();
    for _ in 0..req.len() {
        #[allow(clippy::cast_possible_truncation)] // at most `sides`, a u8
        let _ = faces.try_push(rng.random_range(1..=sides) as u8);
    }
    faces
}

/// The probability of each element a one-face request picks: `Index { n }` is uniform over `0..n`; `DieMod` carries
/// its modulo bias (element `k` has `#faces mapping to k / sides`); `Dice` is not an element pick (`None`).
#[must_use]
pub fn element_weights(req: &Request) -> Option<Vec<f64>> {
    match req.kind {
        Kind::Index { n } => Some(vec![1.0 / f64::from(n); usize::from(n)]),
        Kind::DieMod { sides, n, form } => {
            let mut count = vec![0u8; usize::from(n)];
            for face in 1..=sides {
                count[usize::from(form.index(face, n))] += 1;
            }
            Some(count.into_iter().map(|c| f64::from(c) / f64::from(sides)).collect())
        }
        Kind::Dice { .. } => None,
    }
}

/// `P(k hits)` for `k` in `0..=n` of a `Dice` request with a threshold (binomial, `p = (sides - hits_on + 1) / sides`);
/// `None` without a threshold or for another kind.
#[must_use]
pub fn hit_weights(req: &Request) -> Option<Vec<f64>> {
    let Kind::Dice { n, sides, hits_on: Some(on) } = req.kind else { return None };
    if sides == 0 {
        return None;
    }
    let hitting = if on > sides { 0 } else { sides - on.max(1) + 1 };
    let p = f64::from(hitting) / f64::from(sides);
    let n = usize::from(n);
    let mut out = vec![0.0; n + 1];
    let mut choose = 1.0;
    for (k, slot) in out.iter_mut().enumerate() {
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // k, n <= 15
        let (hits, misses) = (k as i32, (n - k) as i32);
        *slot = choose * p.powi(hits) * (1.0 - p).powi(misses);
        #[allow(clippy::cast_precision_loss)] // small integers
        {
            choose = choose * (n - k) as f64 / (k + 1) as f64;
        }
    }
    Some(out)
}

/// A representative answer with exactly `k` hits: the first `k` dice at the threshold, the rest at 1. For a search that
/// branches on the hit count only (`hit_weights`). `None` if `k > n`, without a threshold, or when 1 would hit.
#[must_use]
pub fn faces_for_hits(req: &Request, k: usize) -> Option<Faces> {
    let Kind::Dice { n, hits_on: Some(on), sides } = req.kind else { return None };
    if k > usize::from(n) || on <= 1 || on > sides {
        return None;
    }
    let mut faces = Faces::default();
    for i in 0..usize::from(n) {
        faces.try_push(if i < k { on } else { 1 })?;
    }
    Some(faces)
}

/// Every ordered answer and its weight (`sides^-n` each), if there are at most `limit` of them. For small requests
/// (an `Index`, one or two dice) a search can expand the node fully.
#[must_use]
pub fn outcomes(req: &Request, limit: usize) -> Option<Vec<(Faces, f64)>> {
    let sides = usize::from(req.sides());
    let n = req.len();
    let count = (0..n).try_fold(1usize, |acc, _| acc.checked_mul(sides))?;
    if count > limit || (sides == 0 && n > 0) {
        return None;
    }
    #[allow(clippy::cast_precision_loss)] // count <= limit
    let weight = 1.0 / count as f64;
    let mut out = Vec::with_capacity(count);
    for mut code in 0..count {
        let mut faces = [0u8; crate::MAX_FACES];
        for slot in faces[..n].iter_mut().rev() {
            #[allow(clippy::cast_possible_truncation)] // < sides <= 255
            let face = (code % sides) as u8 + 1;
            *slot = face;
            code /= sides;
        }
        out.push((Faces::from_slice(&faces[..n])?, weight));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Domain, ModForm, Streams, answer_mut, derive_seed};
    use rand_chacha::ChaCha8Rng;
    use rand_core::SeedableRng;

    #[test]
    fn sample_is_the_answer_on_the_same_stream() {
        let req = Request::new(Domain::Dice, Kind::Dice { n: 9, sides: 10, hits_on: Some(6) });
        let mut s = Streams::new(5);
        let mut rng = ChaCha8Rng::from_seed(derive_seed(5, "dice"));
        for _ in 0..50 {
            assert_eq!(sample(&req, &mut rng), answer_mut(&req, &mut s));
        }
    }

    #[test]
    fn weights_sum_to_one() {
        let d = Request::new(Domain::Dice, Kind::Dice { n: 7, sides: 10, hits_on: Some(8) });
        let w = hit_weights(&d).unwrap();
        assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!((w[0] - 0.7f64.powi(7)).abs() < 1e-12);
        let spy = Request::new(Domain::Dice, Kind::DieMod { sides: 10, n: 3, form: ModForm::FaceMinusOne });
        assert_eq!(element_weights(&spy).unwrap(), [0.4, 0.3, 0.3], "faces 1,4,7,10 pick the first card");
        let o = outcomes(&Request::new(Domain::Dice, Kind::Dice { n: 2, sides: 6, hits_on: None }), 100).unwrap();
        assert_eq!(o.len(), 36);
        assert_eq!(o[1].0.as_slice(), [1, 2]);
        assert!(outcomes(&d, 1000).is_none());
        let f = faces_for_hits(&d, 3).unwrap();
        assert_eq!(f.hits(8), 3);
        assert!(d.accepts(&f));
    }
}
