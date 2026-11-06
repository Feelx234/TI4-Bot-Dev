//! Statistics of the evaluation harness (docs/MCTS_BOT_PLAN.md 4.3, lane L7): Wilson intervals, paired differences, the
//! sequential probability ratio test.

/// The 95 % two-sided normal quantile.
pub const Z95: f64 = 1.959_963_985;

/// The Wilson score interval of `wins` successes in `n` trials (fractional wins allowed: ties split), at normal quantile `z`.
#[must_use]
pub fn wilson(wins: f64, n: f64, z: f64) -> (f64, f64) {
    if n <= 0.0 {
        return (0.0, 1.0);
    }
    let p = wins / n;
    let z2 = z * z;
    let den = 1.0 + z2 / n;
    let centre = (p + z2 / (2.0 * n)) / den;
    let half = z * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt() / den;
    ((centre - half).max(0.0), (centre + half).min(1.0))
}

/// Mean and the 95 % normal half-width of a sample (paired differences).
#[must_use]
pub fn mean_ci(xs: &[f64]) -> (f64, f64) {
    let n = xs.len() as f64;
    if n < 1.0 {
        return (0.0, 0.0);
    }
    let m = xs.iter().sum::<f64>() / n;
    if n < 2.0 {
        return (m, f64::INFINITY);
    }
    let var = xs.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0);
    (m, Z95 * (var / n).sqrt())
}

/// The verdict of an SPRT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The log-likelihood ratio crossed the upper bound: H1 (the better share) holds.
    H1,
    /// It crossed the lower bound: H0.
    H0,
    /// Neither bound yet.
    Continue,
}

impl Verdict {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::H1 => "H1",
            Self::H0 => "H0",
            Self::Continue => "continue",
        }
    }
}

/// Wald's sequential probability ratio test of a Bernoulli share: H0 `p = p0` against H1 `p = p1`, error rates `alpha`, `beta`.
/// An observation is a share in `[0, 1]` (a tie splits the win); its log-likelihood ratio is `x ln(p1/p0) + (1-x) ln((1-p1)/(1-p0))`.
#[derive(Debug, Clone, Copy)]
pub struct Sprt {
    pub p0: f64,
    pub p1: f64,
    pub alpha: f64,
    pub beta: f64,
    pub llr: f64,
    pub n: usize,
    /// The observation count at which a bound was first crossed, and the verdict then.
    pub decided: Option<(usize, Verdict)>,
}

impl Sprt {
    #[must_use]
    pub const fn new(p0: f64, p1: f64, alpha: f64, beta: f64) -> Self {
        Self { p0, p1, alpha, beta, llr: 0.0, n: 0, decided: None }
    }

    /// The bounds `(lower, upper)` of the log-likelihood ratio.
    #[must_use]
    pub fn bounds(&self) -> (f64, f64) {
        ((self.beta / (1.0 - self.alpha)).ln(), ((1.0 - self.beta) / self.alpha).ln())
    }

    /// Add one observation; returns the verdict so far (the first crossing is kept).
    pub fn push(&mut self, x: f64) -> Verdict {
        let x = x.clamp(0.0, 1.0);
        self.llr += x * (self.p1 / self.p0).ln() + (1.0 - x) * ((1.0 - self.p1) / (1.0 - self.p0)).ln();
        self.n += 1;
        if self.decided.is_none() {
            let (lo, hi) = self.bounds();
            if self.llr >= hi {
                self.decided = Some((self.n, Verdict::H1));
            } else if self.llr <= lo {
                self.decided = Some((self.n, Verdict::H0));
            }
        }
        self.verdict()
    }

    #[must_use]
    pub fn verdict(&self) -> Verdict {
        self.decided.map_or(Verdict::Continue, |(_, v)| v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wilson_matches_known_values() {
        // 25 of 100 at 95 %: (0.1755, 0.3430) (standard tables).
        let (lo, hi) = wilson(25.0, 100.0, Z95);
        assert!((lo - 0.1755).abs() < 1e-3 && (hi - 0.3430).abs() < 1e-3, "{lo} {hi}");
        let (lo, hi) = wilson(0.0, 10.0, Z95);
        assert!(lo == 0.0 && (hi - 0.2775).abs() < 1e-3, "{hi}");
        assert_eq!(wilson(1.0, 0.0, Z95), (0.0, 1.0));
    }

    #[test]
    fn mean_ci_of_a_constant_is_tight() {
        let (m, h) = mean_ci(&[0.5, 0.5, 0.5, 0.5]);
        assert!((m - 0.5).abs() < 1e-12 && h.abs() < 1e-12);
        let (m, h) = mean_ci(&[0.0, 1.0]);
        assert!((m - 0.5).abs() < 1e-12 && h > 0.5);
    }

    #[test]
    fn the_sprt_accepts_the_true_hypothesis() {
        // A deterministic stream with share 0.6: accepts H1 of 0.5 vs 0.55; with share 0.45 it accepts H0.
        let mut s = Sprt::new(0.5, 0.55, 0.05, 0.05);
        for i in 0..5000 {
            s.push(if i % 5 < 3 { 1.0 } else { 0.0 });
        }
        assert_eq!(s.verdict(), Verdict::H1);
        let mut s = Sprt::new(0.5, 0.55, 0.05, 0.05);
        for i in 0..5000 {
            s.push(if i % 20 < 9 { 1.0 } else { 0.0 });
        }
        assert_eq!(s.verdict(), Verdict::H0);
        // Bounds: ln(0.05/0.95) and ln(0.95/0.05).
        let (lo, hi) = s.bounds();
        assert!((lo + 2.944).abs() < 1e-3 && (hi - 2.944).abs() < 1e-3);
        // A tie (0.5) at p0 = 0.5, p1 = 0.55 moves the ratio a little down.
        let mut t = Sprt::new(0.5, 0.55, 0.05, 0.05);
        t.push(0.5);
        assert!(t.llr < 0.0 && t.llr > -0.01);
    }
}
