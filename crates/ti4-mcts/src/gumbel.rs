//! The arithmetic of Gumbel AlphaZero (Danihelka et al., "Policy improvement by planning with Gumbel", ICLR 2022; the `mctx` reference):
//! Gumbel noise, the completed Q-values, `sigma`, the improved policy and the sequential-halving plan. Pure functions, tested alone.

use ti4_engine_search::chance::SearchRng;

/// A uniform float in `(0, 1)` from the search's own stream (53 bits).
pub fn uniform(rng: &mut SearchRng) -> f64 {
    const N: usize = 1 << 53;
    (rng.below(N) as f64 + 0.5) / N as f64
}

/// A standard Gumbel sample from the search's own stream.
pub fn gumbel(rng: &mut SearchRng) -> f32 {
    let u = uniform(rng);
    (-(-u.ln()).ln()) as f32
}

/// `softmax(x)` (entries of `-inf` get 0).
#[must_use]
pub fn softmax(x: &[f32]) -> Vec<f32> {
    let m = x.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    if !m.is_finite() {
        return vec![1.0 / x.len().max(1) as f32; x.len()];
    }
    let e: Vec<f32> = x.iter().map(|v| (v - m).exp()).collect();
    let s: f32 = e.iter().sum();
    e.into_iter().map(|v| v / s).collect()
}

/// The monotone transform of a (completed, `[0, 1]`) Q-value: `(c_visit + max_n) * c_scale * q`.
#[must_use]
pub fn sigma(q: f32, max_n: u32, c_visit: f32, c_scale: f32) -> f32 {
    (c_visit + max_n as f32) * c_scale * q
}

/// The completed Q-values of a node's actions: the visited ones their Q, the unvisited ones `v_mix` (the node's value estimate `v_hat`
/// mixed with the prior-weighted Q of the visited actions, weighted by the visit count). With `rescale` the result is mapped to `[0, 1]`
/// by its min and max (as `mctx` does), so `sigma` sees the spread of the values, not their level.
#[must_use]
pub fn completed_q(prior: &[f32], n: &[u32], q: &[f32], v_hat: f32, rescale: bool) -> Vec<f32> {
    let sum_n: u32 = n.iter().sum();
    let (mut pq, mut pv) = (0.0f64, 0.0f64);
    for i in 0..n.len() {
        if n[i] > 0 {
            pq += f64::from(prior[i]) * f64::from(q[i]);
            pv += f64::from(prior[i]);
        }
    }
    let weighted = if pv > 0.0 { pq / pv } else { f64::from(v_hat) };
    let v_mix = ((f64::from(v_hat) + f64::from(sum_n) * weighted) / (1.0 + f64::from(sum_n))) as f32;
    let mut c: Vec<f32> = (0..n.len()).map(|i| if n[i] > 0 { q[i] } else { v_mix }).collect();
    if rescale && !c.is_empty() {
        let lo = c.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = c.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let span = (hi - lo).max(1e-6);
        for v in &mut c {
            *v = (*v - lo) / span;
        }
    }
    c
}

/// The improved policy `softmax(logits + sigma(completed_q))` (the training target of a searched root, and the selection target below
/// the root).
#[must_use]
pub fn improved_policy(logits: &[f32], completed: &[f32], max_n: u32, c_visit: f32, c_scale: f32) -> Vec<f32> {
    let x: Vec<f32> = logits.iter().zip(completed).map(|(l, q)| l + sigma(*q, max_n, c_visit, c_scale)).collect();
    softmax(&x)
}

/// The sequential-halving plan for `n` simulations over `m` considered actions: per phase, how many actions are left and how many
/// simulations each gets (`max(1, n / (phases * left))`, `phases = ceil(log2 m)`, at least 1). The simulations the plan does not use are
/// given round-robin to the last phase's actions; the search stops at `n` in any case, so a plan that asks for more (`n < m`) is cut.
#[must_use]
pub fn halving_plan(n: usize, m: usize) -> Vec<(usize, usize)> {
    if m == 0 {
        return Vec::new();
    }
    let phases = (usize::BITS - (m - 1).leading_zeros()).max(1) as usize;
    let mut out = Vec::with_capacity(phases);
    let mut left = m;
    for _ in 0..phases {
        out.push((left, (n / (phases * left)).max(1)));
        left = left.div_ceil(2);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_halving_plan_halves_and_fits_the_budget() {
        for (n, m) in [(16, 8), (32, 8), (8, 8), (8, 3), (4, 2), (100, 5), (2, 1), (5, 7)] {
            let plan = halving_plan(n, m);
            assert_eq!(plan[0].0, m);
            for w in plan.windows(2) {
                assert_eq!(w[1].0, w[0].0.div_ceil(2), "{n} {m}: {plan:?}");
            }
            let phases = plan.len();
            assert_eq!(phases, if m <= 1 { 1 } else { (m as f64).log2().ceil() as usize });
            let used: usize = plan.iter().map(|(l, p)| l * p).sum();
            if n >= m * phases {
                assert!(used <= n, "{n} {m}: {plan:?} uses {used}");
            }
        }
        assert_eq!(halving_plan(16, 8), vec![(8, 1), (4, 1), (2, 2)]);
        assert_eq!(halving_plan(32, 4), vec![(4, 4), (2, 8)]);
    }

    #[test]
    fn completed_q_mixes_the_unvisited_and_rescales() {
        let prior = [0.5, 0.25, 0.25];
        let c = completed_q(&prior, &[2, 0, 0], &[0.8, 0.0, 0.0], 0.2, false);
        // v_mix = (0.2 + 2 * 0.8) / 3 = 0.6
        assert!((c[0] - 0.8).abs() < 1e-6 && (c[1] - 0.6).abs() < 1e-6 && (c[2] - 0.6).abs() < 1e-6);
        let r = completed_q(&prior, &[2, 1, 0], &[0.8, 0.4, 0.0], 0.2, true);
        assert!((r[0] - 1.0).abs() < 1e-6 && r[1].abs() < 1e-6, "{r:?}");
        let p = improved_policy(&[0.0, 0.0, 0.0], &[1.0, 0.0, 0.5], 4, 50.0, 1.0);
        assert!(p[0] > p[2] && p[2] > p[1]);
        assert!((p.iter().sum::<f32>() - 1.0).abs() < 1e-5);
    }
}
