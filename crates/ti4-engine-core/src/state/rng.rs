//! `Rng`: the seed and the ChaCha8 streams by typed domain, as data of the position. The types are the bottom crate's
//! (`ti4_chance::Streams`, `ti4_chance::Domain`; docs/FORK_RNG_THEORY.md 4.1, lane C2): a stream is keyed
//! `SHA-256(seed_le || domain text)` by that crate, so the core needs no seed source and holds no text. Old saves with
//! dynamically named streams are not supported (decision D3).
//!
//! The names `Rng` / `RngDomain` stay as aliases so that the rest of the core reads as before.

pub use ti4_chance::{Domain as RngDomain, Streams as Rng};

/// Draw one die with `sides` sides from the stream of `domain` (`1..=sides`, today's `GameRng::die`). The streams are a slice of
/// the state, so a rolled-back run restores them with the rest and a re-run draws the same.
pub fn die(rng: &mut Rng, domain: RngDomain, sides: u32) -> u32 {
    #[cfg(feature = "probe-counters")]
    let at = rng.position(domain);
    let face = rng.die(domain, sides);
    #[cfg(feature = "probe-counters")]
    census::drew(domain, at, rng.position(domain));
    face
}

/// The draw census of lane C0 (`docs/FORK_RNG_THEORY.md` 3.2, `docs/C0_RESUME.md`): thread-local counters of every die drawn and of
/// the draws a rolled-back run threw away. Feature `probe-counters` only (a dev-dependency feature of the bridge's tests): no product
/// build has it, nothing the rules read, and it never changes a draw.
///
/// A draw is counted when it is made, whether or not its run is later rolled back; what a game COMMITS is its final stream positions
/// (the streams are a field of `CoreState`, so every rollback restores them). [`Counts::starts`] keeps the start position of every
/// draw: a reverted draw is re-drawn later from the same position, so the distinct starts below the final position are the committed draws.
#[cfg(feature = "probe-counters")]
pub mod census {
    use std::cell::RefCell;
    use std::collections::BTreeSet; // lint-allow(core): the probe-counters census (bridge test builds only), never in a product build

    use super::RngDomain;

    /// What the census saw on this thread since the last [`reset`].
    #[derive(Debug, Clone, Default)]
    pub struct Counts {
        /// Draws (`state::rng::die` calls).
        pub draws: u64,
        /// Stream words those calls consumed (a die is one word, a rejected sample two or more).
        pub words: u64,
        /// Shuffles: always 0 since lane C2 (no in-game shuffle exists; the setup shuffles on its own `Streams`, which the census does
        /// not see).
        pub shuffles: u64,
        /// Runs of `Driver::settle` rolled back because a nested question found the tape dry (R1 phantom rest, R2 re-ask).
        pub dry_rollbacks: u64,
        /// Draws inside those runs (everything drawn between the run's snapshot and its rollback).
        pub dry_draws: u64,
        /// Runs of `Driver::settle` rolled back because the run failed, and the draws inside them.
        pub err_rollbacks: u64,
        pub err_draws: u64,
        /// `(domain, word position before the draw)` of every draw.
        pub starts: BTreeSet<(RngDomain, u128)>, // lint-allow(core): test-build census, see the `use` above
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

    /// The draws made so far (the mark a run takes at its snapshot).
    #[must_use]
    pub fn mark() -> u64 {
        COUNTS.with(|c| c.borrow().draws)
    }

    pub(crate) fn drew(domain: RngDomain, from: u128, to: u128) {
        COUNTS.with(|c| {
            let mut c = c.borrow_mut();
            c.draws += 1;
            c.words += u64::try_from(to.saturating_sub(from)).unwrap_or(u64::MAX);
            c.starts.insert((domain, from));
        });
    }

    /// A run that took its snapshot at `mark` was rolled back: `dry` for a nested question, else a failure.
    pub fn rolled_back(mark: u64, dry: bool) {
        COUNTS.with(|c| {
            let mut c = c.borrow_mut();
            let n = c.draws - mark;
            if dry {
                c.dry_rollbacks += 1;
                c.dry_draws += n;
            } else {
                c.err_rollbacks += 1;
                c.err_draws += n;
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
        die(&mut a, RngDomain::Dice, 10);
        die(&mut a, RngDomain::Agendas, 10);
        let b = a.clone();
        assert!(a.shares_streams_with(&b));
        let pos: Vec<_> = a.positions().collect();
        die(&mut a, RngDomain::Dice, 10);
        assert!(!a.shares_streams_with(&b));
        let mut c = Rng::new(3);
        c.set_positions(&pos);
        assert_eq!(c.positions().collect::<Vec<_>>(), pos);
        assert_eq!(die(&mut c, RngDomain::Dice, 10), {
            let mut d = b.clone();
            die(&mut d, RngDomain::Dice, 10)
        });
    }
}
