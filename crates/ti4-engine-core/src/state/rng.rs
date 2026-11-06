//! `Rng`: the seed and the ChaCha8 streams by typed domain, as data of the position. The types are the bottom crate's
//! (`ti4_chance::Streams`, `ti4_chance::Domain`; docs/FORK_RNG_THEORY.md 4.1, lane C2): a stream is keyed
//! `SHA-256(seed_le || domain text)` by that crate, so the core needs no seed source and holds no text. Old saves with
//! dynamically named streams are not supported (decision D3).
//!
//! The core never draws from them (lane C3, decision D1): every random event is a chance question (`Tape::chance`), and the
//! resolver (`ti4-engine-chance`) answers it from these streams in play. `tools/engine_core/check_layers.py core` forbids a draw.
//!
//! The names `Rng` / `RngDomain` stay as aliases so that the rest of the core reads as before.

pub use ti4_chance::{Domain as RngDomain, Streams as Rng};

/// The chance census (lane C0's draw census, `docs/C0_RESUME.md`, redone for lane C3): thread-local counters of the chance requests the
/// runs of the core meet, and of those met in runs that were rolled back. Feature `probe-counters` only (a dev-dependency feature of the
/// bridge's tests): no product build has it, nothing the rules read, and it changes no answer.
///
/// Since lane C3 the core draws nothing: a request is answered from the tape (an answer the resolver or a search gave) or, when the tape is
/// dry, it is the question the run stops at or a placeholder in the speculative rest of that run. What a game COMMITS is what the resolver
/// answered (`ti4_engine_chance::advance_observed`).
#[cfg(feature = "probe-counters")]
pub mod census {
    use std::cell::RefCell;

    /// What the census saw on this thread since the last [`reset`].
    #[derive(Debug, Clone, Default)]
    pub struct Counts {
        /// Chance requests met by a run (`Tape::chance` calls), answered or not.
        pub requests: u64,
        /// Their faces.
        pub faces: u64,
        /// Requests the tape held an answer for (a resumed run reading what was answered before).
        pub from_tape: u64,
        /// Requests that became the question of a dry run (one per surfaced chance question).
        pub asked: u64,
        /// Requests answered with the fixed placeholder in the speculative rest of a dry run (after an earlier question).
        pub placeholders: u64,
        /// Runs of `Driver::settle` rolled back because a nested or chance question found the tape dry.
        pub dry_rollbacks: u64,
        /// Requests met inside those runs.
        pub dry_requests: u64,
        /// Runs of `Driver::settle` rolled back because the run failed, and the requests inside them.
        pub err_rollbacks: u64,
        pub err_requests: u64,
    }

    thread_local! {
        static COUNTS: RefCell<Counts> = RefCell::new(Counts::default());
    }

    /// Start counting afresh.
    pub fn reset() {
        COUNTS.with(|c| *c.borrow_mut() = Counts::default());
    }

    /// What was counted so far.
    #[must_use]
    pub fn counts() -> Counts {
        COUNTS.with(|c| c.borrow().clone())
    }

    /// The requests met so far (the mark a run takes at its snapshot).
    #[must_use]
    pub fn mark() -> u64 {
        COUNTS.with(|c| c.borrow().requests)
    }

    /// A run met a request of `faces` faces: answered from the tape, asked (the run's question), or a placeholder.
    pub(crate) fn met(faces: usize, from_tape: bool, asked: bool) {
        COUNTS.with(|c| {
            let mut c = c.borrow_mut();
            c.requests += 1;
            c.faces += u64::try_from(faces).unwrap_or(u64::MAX);
            if from_tape {
                c.from_tape += 1;
            } else if asked {
                c.asked += 1;
            } else {
                c.placeholders += 1;
            }
        });
    }

    /// A run that took its snapshot at `mark` was rolled back: `dry` for a question that found the tape dry, else a failure.
    pub fn rolled_back(mark: u64, dry: bool) {
        COUNTS.with(|c| {
            let mut c = c.borrow_mut();
            let n = c.requests - mark;
            if dry {
                c.dry_rollbacks += 1;
                c.dry_requests += n;
            } else {
                c.err_rollbacks += 1;
                c.err_requests += n;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_round_trip_and_clone_shares() {
        let mut a = Rng::new(3);
        a.die(RngDomain::Dice, 10);
        a.die(RngDomain::Agendas, 10);
        let b = a.clone();
        assert!(a.shares_streams_with(&b));
        let pos: Vec<_> = a.positions().collect();
        a.die(RngDomain::Dice, 10);
        assert!(!a.shares_streams_with(&b));
        let mut c = Rng::new(3);
        c.set_positions(&pos);
        assert_eq!(c.positions().collect::<Vec<_>>(), pos);
        assert_eq!(c.die(RngDomain::Dice, 10), {
            let mut d = b.clone();
            d.die(RngDomain::Dice, 10)
        });
    }
}
