//! The 4-seat value vector (`docs/MCTS_BOT_PLAN.md` 1.3): one win probability per seat.
//!
//! Two orders exist:
//! * **absolute** (seat order, `v[s]` is seat `s`): what the tree stores and backs up (max^n: a node's actor reads its own component);
//! * **actor-relative** (`v[0]` the actor of the position, then the others in seat order after it): what an [`crate::Evaluator`] returns,
//!   the order of the value net's head.

use ti4_engine_search::Outcome;

/// Seats of the MCTS setup (Sol, Letnev, Xxcha, Jol-Nar). The search refuses a table of another size.
pub const SEATS: usize = 4;

/// One probability per seat (they sum to 1).
pub type Value = [f32; SEATS];

/// Every seat equally likely.
pub const UNIFORM: Value = [0.25; SEATS];

/// Actor-relative to absolute: `abs[(actor + k) % 4] = rel[k]`.
#[must_use]
pub fn to_absolute(rel: &Value, actor: usize) -> Value {
    let mut abs = [0.0; SEATS];
    for (k, v) in rel.iter().enumerate() {
        abs[(actor + k) % SEATS] = *v;
    }
    abs
}

/// Absolute to actor-relative: `rel[k] = abs[(actor + k) % 4]`.
#[must_use]
pub fn to_relative(abs: &Value, actor: usize) -> Value {
    let mut rel = [0.0; SEATS];
    for (k, r) in rel.iter_mut().enumerate() {
        *r = abs[(actor + k) % SEATS];
    }
    rel
}

/// The exact value of an ended game (absolute): 1 for the winner; without a winner (a game stopped unfinished) the seats with the most
/// points share it.
#[must_use]
pub fn outcome_value(o: &Outcome) -> Value {
    let mut v = [0.0; SEATS];
    if let Some(w) = o.winner.filter(|w| *w < SEATS) {
        v[w] = 1.0;
        return v;
    }
    let best = o.vp.iter().take(SEATS).copied().max().unwrap_or(0);
    let top: Vec<usize> = (0..SEATS.min(o.vp.len())).filter(|&s| o.vp[s] == best).collect();
    if top.is_empty() {
        return UNIFORM;
    }
    for s in &top {
        v[*s] = 1.0 / top.len() as f32;
    }
    v
}

/// A heuristic value of an unfinished position (absolute): `softmax(vp / temp)`. The leaf value of the stubs, never a training target.
#[must_use]
pub fn vp_value(vp: &[i16], temp: f32) -> Value {
    let t = temp.max(1e-3);
    let mut v = [0.0f32; SEATS];
    for (s, x) in v.iter_mut().enumerate() {
        *x = f32::from(vp.get(s).copied().unwrap_or(0)) / t;
    }
    let m = v.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut sum = 0.0;
    for x in &mut v {
        *x = (*x - m).exp();
        sum += *x;
    }
    for x in &mut v {
        *x /= sum;
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_and_absolute_are_inverse_rotations() {
        let rel = [0.4, 0.3, 0.2, 0.1];
        for actor in 0..SEATS {
            let abs = to_absolute(&rel, actor);
            assert!((abs[actor] - 0.4).abs() < 1e-9, "the actor's own component is rel[0]");
            assert!((abs[(actor + 1) % SEATS] - 0.3).abs() < 1e-9);
            assert_eq!(to_relative(&abs, actor), rel);
        }
    }

    #[test]
    fn outcomes_and_vp_values_are_distributions() {
        let won = Outcome { finished: true, round: 5, vp: vec![10, 7, 3, 2], winner: Some(0) };
        assert_eq!(outcome_value(&won), [1.0, 0.0, 0.0, 0.0]);
        let tie = Outcome { finished: false, round: 30, vp: vec![6, 6, 3, 2], winner: None };
        assert_eq!(outcome_value(&tie), [0.5, 0.5, 0.0, 0.0]);
        let v = vp_value(&[4, 2, 2, 0], 2.0);
        assert!((v.iter().sum::<f32>() - 1.0).abs() < 1e-5);
        assert!(v[0] > v[1] && (v[1] - v[2]).abs() < 1e-6 && v[2] > v[3]);
    }
}
