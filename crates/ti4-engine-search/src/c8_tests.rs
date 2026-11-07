//! Lane C8: the determinisation of the hidden cards (`crate::determinize`, docs/C8_RESUME.md).
//!
//! * The seat's view is the same in the copy (`seen_digest`, the MCTS features of the question, the rule bot's rendered question), in the
//!   position the question was asked in and in the live core.
//! * What it does not see is redrawn: two seeds give different arrangements, the true one is not recovered, the copies are permutations of
//!   the same cards (hand and deck sizes kept), the public objectives' stages kept.
//! * The real game is untouched (same answers, same end as a game never determinised), the game's streams never move, one seed gives one
//!   copy, and a determinised copy forks and resumes like any position (C4/C6 style).

use ti4_engine_core::Seat;
use ti4_engine_core::generated::{Agenda, PublicObjective, SecretObjective};
use ti4_engine_core::query::objectives::stage_of;
use ti4_engine_core::state::{CoreState, ObjectiveRef};

use crate::advance::{Rules, advance_to_tree_decision, default_tree, rollout};
use crate::agent::{RuleBotAgent, SeatAgent};
use crate::chance::SearchRng;
use crate::determinize::{determinize, determinize_report, seen_digest};
use crate::{Node, Position, mcts_spec, start};

/// The secret objectives `seat` does not hold or see: the deck, then the other seats' hands in seat order.
fn unseen_secrets(core: &CoreState, seat: Seat) -> Vec<SecretObjective> {
    let mut v = core.decks.secret.cards().to_vec();
    for p in core.players.iter().filter(|p| p.seat != seat) {
        v.extend_from_slice(p.cards.secrets.as_slice());
    }
    v
}

fn sorted<T: Ord + Clone>(v: &[T]) -> Vec<T> {
    let mut v = v.to_vec();
    v.sort();
    v
}

/// The facts a determinisation must keep: the public slices compared as they are, the hidden ones by size and (cards of a closed
/// pool) by content.
fn assert_consistent(real: &CoreState, det: &CoreState, seat: Seat, pools: &[Vec<PublicObjective>; 2]) {
    let (r, d) = (&real.decks, &det.decks);
    assert_eq!(sorted(&unseen_secrets(real, seat)), sorted(&unseen_secrets(det, seat)), "the secrets are redealt from one pool");
    for (pr, pd) in real.players.iter().zip(det.players.iter()) {
        assert_eq!(pr.cards.secrets.len(), pd.cards.secrets.len(), "hand sizes kept");
        if pr.seat == seat {
            assert_eq!(pr.cards.secrets, pd.cards.secrets, "the seat's own secrets stay");
        }
        assert_eq!(pr.cards.hand, pd.cards.hand, "action cards are not resampled in v1");
    }
    assert_eq!(r.secret.len(), d.secret.len());
    assert_eq!(sorted(r.agenda.cards()), sorted(d.agenda.cards()), "the agenda deck holds the same cards");
    assert_eq!(sorted(r.relic.cards()), sorted(d.relic.cards()));
    for (a, b) in r.explore.iter().zip(&d.explore) {
        assert_eq!(sorted(a.cards()), sorted(b.cards()));
    }
    assert_eq!(r.action, d.action, "the action deck is not resampled in v1");
    assert_eq!(r.objective.len(), d.objective.len());
    let stages = |deck: &[PublicObjective]| deck.iter().map(|o| stage_of(*o)).collect::<Vec<_>>();
    assert_eq!(stages(r.objective.cards()), stages(d.objective.cards()), "every slot keeps its stage");
    let mut seen = std::collections::BTreeSet::new();
    for o in d.objective.cards() {
        assert!(seen.insert(*o), "an objective twice in the deck");
        assert!(!det.objectives.revealed.as_slice().contains(&ObjectiveRef::Public(*o)), "a revealed objective back in the deck");
        let stage = usize::from(stage_of(*o).unwrap() - 1);
        assert!(pools[stage].contains(o) || r.objective.cards().contains(o), "an objective out of the game's pool");
    }
    assert_eq!(real.objectives, det.objectives);
    assert_eq!(real.agenda, det.agenda);
    assert_eq!(real.cards, det.cards);
    assert_eq!(real.board, det.board);
    assert_eq!(real.marks, det.marks);
    assert_eq!(real.rng.positions().collect::<Vec<_>>(), det.rng.positions().collect::<Vec<_>>());
}

/// What a seat may see at `pos`: the view of the question's position (the actor's), and the live core's view of `seat`.
fn view(pos: &Position, seat: Seat) -> (Option<String>, String) {
    (pos.seen().map(|s| seen_digest(&s)), seen_digest(&pos.seen_for(seat)))
}

/// The tallies of [`checked_game`].
#[derive(Default, Debug)]
struct Tally {
    checks: usize,
    /// The two seeds gave different unseen secret arrangements / agenda orders / objective decks.
    secrets_differ: usize,
    agendas_differ: usize,
    objectives_differ: usize,
    /// The copy's secret-deck top is the real one.
    top_is_real: usize,
    /// The real stage II objectives all still in the copy's deck.
    stage2_kept: usize,
    pinned: usize,
    pool: usize,
    /// Questions asked in a position of their own (`Asking::at` / nested): those states were relabelled too.
    asked_in: usize,
}

/// The MCTS game of `seed`, the rule bot with its view at every seat; at every `every`-th question its actor's two determinisations are
/// checked. Returns the tally, the answers and the final position.
fn checked_game(seed: u64, every: usize) -> (Tally, Vec<String>, Position) {
    let mut pos = start(&mcts_spec(seed)).unwrap();
    let pools = pos.table().objective_pools.clone();
    let mut bot = RuleBotAgent::seeing();
    let (mut t, mut answers, mut n) = (Tally::default(), Vec::new(), 0usize);
    while let Node::Decide(choice) = pos.node() {
        let choice = choice.clone();
        if n % every == 0 {
            let seat = choice.actor;
            let (before, positions) = (pos.state_hash(), pos.stream_positions());
            let mut r1 = SearchRng::at(1, &[n as u64]);
            let (d1, report) = determinize_report(&pos, seat, &mut r1);
            let d1b = determinize(&pos, seat, &mut SearchRng::at(1, &[n as u64]));
            let d2 = determinize(&pos, seat, &mut SearchRng::at(2, &[n as u64]));
            assert_eq!(pos.state_hash(), before, "the real position is untouched");
            assert_eq!(pos.stream_positions(), positions);
            assert!(d1.same_as(&d1b) && d1.state_hash() == d1b.state_hash(), "one seed, one copy");
            for d in [&d1, &d2] {
                assert_eq!(view(d, seat), view(&pos, seat), "question {n}: the seat sees the same");
                assert_eq!(d.stream_positions(), positions, "the streams are not drawn");
                assert_eq!(format!("{:?}", d.rendered()), format!("{:?}", pos.rendered()), "the rule bot's question");
                let (a, b) = (d.seen().unwrap(), pos.seen().unwrap());
                assert!(ti4_mcts_features::encode::encode(&a, &choice) == ti4_mcts_features::encode::encode(&b, &choice), "the net's features");
                assert_consistent(pos.core_for_tests(), d.core_for_tests(), seat, &pools);
                // The positions the question was asked in are relabelled alike: what equals the live core in the real position equals
                // it in the copy.
                let (rs, ds) = (pos.card_states(), d.card_states());
                assert_eq!(rs.len(), ds.len());
                for (r, x) in rs.iter().zip(&ds).skip(1) {
                    assert_consistent(r, x, seat, &pools);
                    let core = (pos.core_for_tests(), d.core_for_tests());
                    if unseen_secrets(r, seat) == unseen_secrets(core.0, seat) {
                        assert_eq!(unseen_secrets(x, seat), unseen_secrets(core.1, seat), "an asked-in position not relabelled");
                    }
                    if r.decks.agenda == core.0.decks.agenda {
                        assert_eq!(x.decks.agenda, core.1.decks.agenda, "an asked-in position not relabelled");
                    }
                }
                assert_eq!(d.mode(), crate::Mode::Search);
                assert_eq!(d.differences(&pos).iter().filter(|p| !["core", "stop", "mode"].contains(*p)).count(), 0, "{:?}", d.differences(&pos));
            }
            let (c1, c2, real) = (d1.core_for_tests(), d2.core_for_tests(), pos.core_for_tests());
            t.checks += 1;
            t.secrets_differ += usize::from(unseen_secrets(c1, seat) != unseen_secrets(c2, seat));
            t.agendas_differ += usize::from(c1.decks.agenda.cards() != c2.decks.agenda.cards());
            t.objectives_differ += usize::from(c1.decks.objective.cards() != c2.decks.objective.cards());
            t.top_is_real += usize::from(c1.decks.secret.top() == real.decks.secret.top());
            let stage2 = |c: &CoreState| c.decks.objective.cards().iter().copied().filter(|o| stage_of(*o) == Some(2)).collect::<Vec<_>>();
            t.stage2_kept += usize::from(sorted(&stage2(c1)) == sorted(&stage2(real)));
            t.pinned += report.pinned;
            t.asked_in += pos.card_states().len() - 1;
            t.pool += report.secrets + report.objectives[0] + report.objectives[1] + report.agendas;
        }
        n += 1;
        let d = bot.decide(&pos, &choice).unwrap();
        answers.push(format!("{:?}", d.answer));
        pos.apply(d).unwrap();
    }
    assert!(matches!(pos.node(), Node::End), "{:?}", pos.node());
    (t, answers, pos)
}

/// The seat's view, the features and the rule bot's question are the same in a determinised copy; what it does not see is redrawn (two
/// seeds differ, the real arrangement is not recovered, which five stage II objectives were dealt is redrawn too); the real game is the
/// game never determinised.
#[test]
fn a_determinisation_keeps_what_the_seat_sees_and_redraws_the_rest() {
    let (t, answers, end) = checked_game(3, 4);
    eprintln!("{t:?}");
    assert!(t.checks > 200 && t.asked_in > 20, "{t:?}");
    // Two seeds: the secret pool is ~30 cards, the agenda deck ~45 (until the game ends): practically always different.
    assert!(t.secrets_differ * 100 >= t.checks * 98, "{t:?}");
    assert!(t.agendas_differ * 100 >= t.checks * 98, "{t:?}");
    assert!(t.objectives_differ * 100 >= t.checks * 90, "{t:?}");
    // The real top is found about once per pool size (~1/30), and the real stage II set (5 of ~18 unseen) hardly ever.
    assert!(t.top_is_real * 10 < t.checks, "{t:?}");
    assert!(t.stage2_kept * 10 < t.checks, "{t:?}");
    // The game itself is the game played without any determinisation.
    let mut plain = start(&mcts_spec(3)).unwrap();
    let mut bot = RuleBotAgent::seeing();
    let mut plain_answers = Vec::new();
    while let Node::Decide(choice) = plain.node() {
        let choice = choice.clone();
        let d = bot.decide(&plain, &choice).unwrap();
        plain_answers.push(format!("{:?}", d.answer));
        plain.apply(d).unwrap();
    }
    assert_eq!(answers, plain_answers);
    assert!(end.same_as(&plain), "{:?}", end.differences(&plain));
}

/// A determinised search copy forks and resumes like any position: a fork equals it, two forks played alike (the rule bot delegate, the
/// same search seed) stay equal, a fork answering another option leaves it alone; nothing moves the game's streams; determinised copies
/// play on to the end without a failure.
#[test]
fn determinised_copies_fork_and_resume_identically() {
    let rules = Rules::default();
    let mut pos = start(&mcts_spec(5)).unwrap();
    let mut bot = RuleBotAgent::seeing();
    let (mut nodes, mut checked, mut rolled) = (0usize, 0usize, 0usize);
    while let Node::Decide(choice) = pos.node() {
        let choice = choice.clone();
        if default_tree(&pos, &choice) {
            if nodes % 5 == 0 {
                let positions = pos.stream_positions();
                let d = determinize(&pos, choice.actor, &mut SearchRng::at(9, &[nodes as u64]));
                let before = d.state_hash();
                let f = d.fork();
                assert!(f.same_as(&d));
                let play = |mut f: Position, pick: usize| {
                    f.answer_index(pick).unwrap();
                    let mut delegate = RuleBotAgent { view: true, seed: Some(5), index: None };
                    let mut rng = SearchRng::at(3, &[nodes as u64]);
                    let halt = advance_to_tree_decision(&mut f, &mut delegate, &mut rng, &rules).unwrap();
                    (f, halt.0)
                };
                let (a, ha) = play(d.fork(), 0);
                let (b, hb) = play(d.fork(), 0);
                assert!(a.same_as(&b) && a.state_hash() == b.state_hash() && ha == hb, "{:?}", a.differences(&b));
                let (c, _) = play(d.fork(), d.legal().len() - 1);
                assert_eq!(d.state_hash(), before, "the copies leave the determinised position alone");
                for p in [&a, &b, &c, &d] {
                    assert_eq!(p.stream_positions(), positions, "a search copy moved the streams");
                }
                checked += 1;
                if nodes % 40 == 0 {
                    let mut r = d.fork();
                    let mut delegate = RuleBotAgent { view: true, seed: Some(7), index: None };
                    rollout(&mut r, &mut delegate, &mut SearchRng::at(4, &[nodes as u64]), 50_000).unwrap_or_else(|e| panic!("node {nodes}: {e}"));
                    assert!(r.outcome().finished);
                    rolled += 1;
                }
            }
            nodes += 1;
        }
        let d = bot.decide(&pos, &choice).unwrap();
        pos.apply(d).unwrap();
    }
    assert!(checked > 50 && rolled >= 5, "{checked} checked, {rolled} rolled out");
}

/// Politics: the agendas a seat put back on the deck stay where it put them in its own determinisations (it knows them), and move in
/// the other seats' ones.
#[test]
fn politics_placements_stay_where_their_placer_put_them() {
    let (mut placer_checks, mut other_moved) = (0usize, 0usize);
    for seed in 1..=4u64 {
        let mut pos = start(&mcts_spec(seed)).unwrap();
        let mut bot = RuleBotAgent::seeing();
        let mut n = 0u64;
        while let Node::Decide(choice) = pos.node() {
            let choice = choice.clone();
            for s in 0..4u8 {
                let seat = Seat(s);
                let placed: Vec<Agenda> = pos.placed_by(seat).collect();
                let real = pos.core_for_tests().decks.agenda.cards().to_vec();
                let known: Vec<(usize, Agenda)> = real.iter().enumerate().filter(|(_, a)| placed.contains(a)).map(|(i, a)| (i, *a)).collect();
                if known.is_empty() {
                    continue;
                }
                let d = determinize(&pos, seat, &mut SearchRng::at(seed, &[n, u64::from(s)]));
                let deck = d.core_for_tests().decks.agenda.cards().to_vec();
                for (i, a) in &known {
                    assert_eq!(deck[*i], *a, "seed {seed}: the placer's agenda moved");
                }
                placer_checks += 1;
                let other = Seat((s + 1) % 4);
                if pos.placed_by(other).all(|a| !placed.contains(&a)) {
                    let d = determinize(&pos, other, &mut SearchRng::at(seed, &[n, 99]));
                    let deck = d.core_for_tests().decks.agenda.cards().to_vec();
                    other_moved += usize::from(known.iter().any(|(i, a)| deck[*i] != *a));
                }
            }
            n += 1;
            let d = bot.decide(&pos, &choice).unwrap();
            pos.apply(d).unwrap();
        }
    }
    eprintln!("placer checks {placer_checks}, moved for another seat {other_moved}");
    assert!(placer_checks > 100, "no Politics placement met: {placer_checks}");
    assert!(other_moved * 10 > placer_checks * 8, "{other_moved} of {placer_checks}");
}

/// The game's objective pools hold every objective the setup dealt, the stages apart; over seeds, a determinisation of the opening
/// position deals stage I and II objectives that are not among the real ones.
#[test]
fn the_objective_pools_hold_the_dealt_objectives_and_the_draw_leaves_them() {
    let pos = start(&mcts_spec(1)).unwrap();
    let pools = &pos.table().objective_pools;
    let core = pos.core_for_tests();
    assert!(pools[0].len() > 5 && pools[1].len() > 5, "{pools:?}");
    let dealt: Vec<PublicObjective> =
        core.decks.objective.cards().iter().copied().chain(core.objectives.revealed.as_slice().iter().filter_map(|o| if let ObjectiveRef::Public(p) = o { Some(*p) } else { None })).collect();
    for o in &dealt {
        let stage = usize::from(stage_of(*o).unwrap() - 1);
        assert!(pools[stage].contains(o), "{o:?} dealt but not in the pool");
    }
    let seat = pos.choice().map_or(Seat(0), |c| c.actor);
    let mut drawn = std::collections::BTreeSet::new();
    for i in 0..64u64 {
        let d = determinize(&pos, seat, &mut SearchRng::new(i));
        drawn.extend(d.core_for_tests().decks.objective.cards().iter().copied());
    }
    let not_dealt = drawn.iter().filter(|o| !dealt.contains(o)).count();
    assert!(not_dealt > 10, "only {not_dealt} objectives outside the real deal were drawn");
    for o in &drawn {
        assert!(!core.objectives.revealed.as_slice().contains(&ObjectiveRef::Public(*o)));
    }
}

