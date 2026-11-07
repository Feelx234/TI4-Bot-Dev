//! Whole games with MCTS seats (lane L6): they end without engine errors, the search is deterministic, the real game is the game its
//! answers make (no search leaks into it), tree reuse happens, and the A/B plumbing against the rule bot runs. No strength claim.

use ti4_engine_search::driver::{Limits, replay};
use ti4_engine_search::{mcts_spec, start};
use ti4_mcts::Config;
use ti4_mcts::play::{EvalSpec, play_game};

fn small() -> Config {
    Config { sims_full: 8, sims_fast: 2, ..Config::default() }
}

/// The rollout evaluator (rule-bot rollouts of 40 answers, flat priors) at one seat: full 4-player games to the end on three seeds, no
/// engine error; searches, chance nodes and reused trees happen; the game replays from its answers alone to the same position.
#[test]
fn an_mcts_seat_with_rollouts_plays_whole_games() {
    for (seed, seat) in [(1u64, 0u8), (2, 1), (3, 2)] {
        let r = play_game(seed, &[seat], &small(), EvalSpec::Rollout { horizon: 40, rule_prior: false }, Limits::default()).unwrap();
        assert_eq!(r.error, None, "seed {seed}");
        assert!(r.outcome.finished, "seed {seed}: {:?}", r.outcome);
        assert!(r.counts.searched > 30, "seed {seed}: {:?}", r.counts);
        assert!(r.stats.sims >= r.counts.searched, "seed {seed}: {:?}", r.stats);
        assert!(r.stats.reused > 0, "seed {seed}: no tree was reused: {:?}", r.stats);
        assert_eq!(r.stats.errors, 0, "seed {seed}: failed expansions: {:?}", r.stats);
        eprintln!(
            "seed {seed} seat {seat}: {:?} in {:.1} s; searches {} (full {}), forced {}, delegated {}; sims {}, {:.0} sims/s; expanded {}, chance {}, reused {} ({} nodes, re-derived {}, mismatched {})",
            r.outcome,
            r.secs,
            r.counts.searched,
            r.counts.full,
            r.counts.forced,
            r.counts.delegated,
            r.stats.sims,
            r.stats.sims_per_s(),
            r.stats.expanded,
            r.stats.chance_nodes,
            r.stats.reused,
            r.stats.reused_nodes,
            r.stats.rederived,
            r.stats.mismatched
        );
        // The real game is the game of its answers: the search never changed it (nor its streams).
        let mut again = start(&mcts_spec(seed)).unwrap();
        replay(&mut again, &r.records).unwrap();
        assert_eq!(again.outcome(), r.outcome, "seed {seed}");
        assert_eq!(again.steps(), r.steps);
    }
}

/// The same seed and search seed give the same game, answer for answer; another search seed is another search.
#[test]
fn the_search_is_deterministic() {
    let limits = Limits { max_steps: 400, max_rounds: 30 };
    let a = play_game(4, &[0, 2], &small(), EvalSpec::RulePrior, limits).unwrap();
    let b = play_game(4, &[0, 2], &small(), EvalSpec::RulePrior, limits).unwrap();
    assert_eq!(a.error, None);
    assert!(a.counts.searched > 20, "{:?}", a.counts);
    assert_eq!(a.records, b.records, "the same seeds, another game");
    assert_eq!(a.stats.sims, b.stats.sims);
    assert_eq!(a.stats.expanded, b.stats.expanded);
    let c = play_game(4, &[0, 2], &Config { seed: 99, ..small() }, EvalSpec::RulePrior, limits).unwrap();
    assert_eq!(c.error, None);
    assert!(c.records != a.records || c.stats.expanded != a.stats.expanded, "another search seed searched alike");
}

/// Four MCTS seats (the self-play table) with the uniform evaluator and a tiny budget: a whole game, no error.
#[test]
fn four_mcts_seats_play_a_whole_game() {
    let cfg = Config { sims_full: 4, sims_fast: 2, ..Config::default() };
    let r = play_game(5, &[0, 1, 2, 3], &cfg, EvalSpec::RulePrior, Limits::default()).unwrap();
    assert_eq!(r.error, None);
    assert!(r.outcome.finished, "{:?}", r.outcome);
    assert!(r.counts.searched > 100, "{:?}", r.counts);
    eprintln!("4 MCTS seats: {:?} in {:.1} s, {:?}", r.outcome, r.secs, r.counts);
}

/// The A/B plumbing against the rule bot (a very small budget, two seeds, paired with the all-rule-bot game of the same seed): both
/// games end; the report is printed. Not a strength claim.
#[test]
fn the_ab_smoke_against_the_rule_bot_runs() {
    let cfg = Config { sims_full: 4, sims_fast: 2, ..Config::default() };
    let (mut mcts_wins, mut base_wins, mut dvp) = (0, 0, 0i32);
    for seed in [11u64, 12] {
        let seat = (seed % 4) as u8;
        let base = play_game(seed, &[], &cfg, EvalSpec::RulePrior, Limits::default()).unwrap();
        let ab = play_game(seed, &[seat], &cfg, EvalSpec::RulePrior, Limits::default()).unwrap();
        assert_eq!(base.error, None);
        assert_eq!(ab.error, None);
        assert!(base.outcome.finished && ab.outcome.finished);
        mcts_wins += i32::from(ab.outcome.winner == Some(usize::from(seat)));
        base_wins += i32::from(base.outcome.winner == Some(usize::from(seat)));
        dvp += i32::from(ab.outcome.vp[usize::from(seat)]) - i32::from(base.outcome.vp[usize::from(seat)]);
    }
    eprintln!("A/B smoke (2 seeds): MCTS seat wins {mcts_wins}, the rule bot in that seat {base_wins}, delta VP {dvp}");
}
