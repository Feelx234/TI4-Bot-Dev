//! P2, the predicate: a yes/no answer composed of the rules of every provider.

use super::{Permute, Q, Source};

/// What one rule says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Not this rule's business.
    Abstain,
    Allow,
    Forbid,
}

/// How the verdicts combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combine {
    /// The string `all` kind (`may_sustain`): any `Forbid` makes it false; otherwise the default.
    ForbidWins,
    /// The string `any` kind (`hit_immune`): any `Allow` makes it true; otherwise the default.
    AllowWins,
}

pub struct Rule<S> {
    pub source: Source,
    pub f: fn(Q<'_>, &S) -> Verdict,
}

impl<S> Clone for Rule<S> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S> Copy for Rule<S> {}

pub struct Pred<S> {
    rules: Vec<Rule<S>>,
    combine: Combine,
}

impl<S> Clone for Pred<S> {
    fn clone(&self) -> Self {
        Self { rules: self.rules.clone(), combine: self.combine }
    }
}

impl<S> Pred<S> {
    #[must_use]
    pub const fn new(combine: Combine) -> Self {
        Self { rules: Vec::new(), combine }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn push(&mut self, source: Source, f: fn(Q<'_>, &S) -> Verdict) {
        self.rules.push(Rule { source, f });
    }

    /// The answer. `default` is returned when every rule abstains (and, for `ForbidWins`, when none forbids).
    #[must_use]
    pub fn decide(&self, q: Q<'_>, site: &S, default: bool) -> bool {
        match self.combine {
            Combine::ForbidWins => {
                if self.rules.iter().any(|r| (r.f)(q, site) == Verdict::Forbid) {
                    false
                } else {
                    default
                }
            }
            Combine::AllowWins => {
                if self.rules.iter().any(|r| (r.f)(q, site) == Verdict::Allow) {
                    true
                } else {
                    default
                }
            }
        }
    }

    /// Who decided it (`Forbid` for `ForbidWins`, `Allow` for `AllowWins`), for the "details" of a window.
    #[must_use]
    pub fn why(&self, q: Q<'_>, site: &S) -> crate::containers::SmallList<Source, 4> {
        let wanted = match self.combine {
            Combine::ForbidWins => Verdict::Forbid,
            Combine::AllowWins => Verdict::Allow,
        };
        self.rules.iter().filter(|r| (r.f)(q, site) == wanted).map(|r| r.source).collect()
    }

    /// Rules are pure and the combination is an `any`: every order gives the same answer.
    pub fn permute(&mut self, p: Permute) {
        p.apply(&mut self.rules);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Kernel;
    use crate::state::CoreState;

    #[test]
    fn forbid_wins_and_allow_wins_match_all_and_any() {
        let (k, s) = (Kernel::plain(), CoreState::all_loaded());
        let q = Q::new(&s, &k);
        let mut all = Pred::<u8>::new(Combine::ForbidWins);
        assert!(all.decide(q, &0, true));
        all.push(Source::Printed, |_, _| Verdict::Abstain);
        assert!(all.decide(q, &0, true));
        all.push(Source::Printed, |_, _| Verdict::Forbid);
        assert!(!all.decide(q, &0, true));
        let mut any = Pred::<u8>::new(Combine::AllowWins);
        any.push(Source::Printed, |_, _| Verdict::Forbid);
        assert!(!any.decide(q, &0, false));
        any.push(Source::Printed, |_, _| Verdict::Allow);
        assert!(any.decide(q, &0, false));
        assert_eq!(any.why(q, &0).len(), 1);
    }
}
