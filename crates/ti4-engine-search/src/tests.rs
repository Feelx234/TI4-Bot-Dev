//! The search API against the product (lane C6, docs/C6_RESUME.md).
//!
//! * The quick game played through [`Position`] with the rule bot's random pick at every seat is the product's quick game: the same
//!   fingerprint as `native_play --quick` (`fd54dc6c956758bc`, 1,110 steps), which the bridge's `wrapper_native` proves equal to the thin
//!   `Game` played with the blocking step.
//! * A fork at every question (C4 style): the copy equals the original; answered alike, they stay equal; a copy answering another option
//!   leaves the original untouched; the forked game ends as the game never forked.
//! * Search mode: the chance requests answered from the test's own copy of the streams give the played game, and the position's streams
//!   never move (also in copies answered with other faces).
//! * The C7 ledger trim at every question plays the same game.
//! * A recorded game replays to the same position.

use sha2::{Digest, Sha256};
use ti4_engine_core::state::Ledger;

use crate::agent::{Answer, RuleBotAgent, SeatAgent};
use crate::chance::SearchRng;
use crate::driver::{self, Limits, Record};
use crate::{Mode, Node, Position, quick_spec, start};

/// The quick game's pins (int-29: `crates/ti4-engine-core-bridge/src/native_play.rs`).
const QUICK_FINGERPRINT: &str = "fd54dc6c956758bc";
const QUICK_STEPS: usize = 1_110;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `NativeGame`'s decision chain and fingerprint, kept alongside a game.
#[derive(Default)]
struct Chain(String);

impl Chain {
    fn push(&mut self, player: &str, prompt: &str, journal: &str) {
        let mut h = Sha256::new();
        h.update(self.0.as_bytes());
        h.update(format!("{player}\u{1}{prompt}\u{1}{journal}").as_bytes());
        self.0 = hex(&h.finalize());
    }

    fn fingerprint(&self, pos: &Position) -> String {
        let mut h = Sha256::new();
        h.update(pos.materialized_json().expect("materialises").as_bytes());
        h.update(self.0.as_bytes());
        hex(&h.finalize())[..16].to_owned()
    }
}

/// The blind rule bot's answer at `pos`, with the chain entry it makes.
fn bot(pos: &Position, chain: &mut Chain) -> Answer {
    let rendered = pos.rendered().expect("a question");
    let (_, id, _, _) = RuleBotAgent::blind().pick(pos).expect("an answer");
    chain.push(rendered.player.as_str(), &rendered.prompt, &id);
    Answer::Id(id)
}

fn apply(pos: &mut Position, a: &Answer) {
    let r = match a {
        Answer::Id(id) => pos.answer_id(id),
        Answer::Index(i) => pos.answer_index(*i),
        Answer::Pick(p) => pos.answer(*p),
    };
    r.unwrap_or_else(|e| panic!("step {}: {e}", pos.steps()));
}

/// The quick game played to the end through the API: its chain, the records, the final position.
fn quick_game() -> (Chain, Vec<Record>, Position) {
    let mut pos = start(&quick_spec()).expect("the quick game starts");
    let mut chain = Chain::default();
    let mut records = Vec::new();
    while let Node::Decide(choice) = pos.node() {
        let seat = choice.actor.0;
        let a = bot(&pos, &mut chain);
        apply(&mut pos, &a);
        records.push(Record { seat, answer: a });
        assert!(pos.steps() < 20_000);
    }
    assert!(matches!(pos.node(), Node::End), "the quick game ends: {:?}", pos.node());
    (chain, records, pos)
}

#[test]
fn the_quick_game_through_the_api_is_the_product_quick_game() {
    let (chain, records, pos) = quick_game();
    assert_eq!(pos.steps(), QUICK_STEPS);
    assert_eq!(chain.fingerprint(&pos), QUICK_FINGERPRINT);
    let o = pos.outcome();
    assert!(o.finished && o.winner.is_some(), "{o:?}");
    // The driver with the agent at every seat plays the same game.
    let mut again = start(&quick_spec()).unwrap();
    let mut agents: Vec<Box<dyn SeatAgent>> = (0..4).map(|_| Box::new(RuleBotAgent::blind()) as Box<dyn SeatAgent>).collect();
    let mut rec2 = Vec::new();
    let ended = driver::play(&mut again, &mut agents, Limits::default(), true, |_, r| rec2.push(r.clone()));
    assert_eq!(ended.error, None);
    assert_eq!(rec2, records);
    assert!(again.same_as(&pos), "{:?}", again.differences(&pos));
    // A replay of the records is the same game (deterministic replay).
    let mut replayed = start(&quick_spec()).unwrap();
    driver::replay(&mut replayed, &records).unwrap();
    assert!(replayed.same_as(&pos), "{:?}", replayed.differences(&pos));
}

/// Every this many questions the copy with another answer also plays on `HORIZON` stops, against an independent replay of the game from
/// its start with the same answers (O(game) each).
const REPLAY_EVERY: usize = 97;
const HORIZON: usize = 4;

/// Play `pos` on `stops` stops with the blind bot (or to the end).
fn play_on(pos: &mut Position, stops: usize) {
    let mut sink = Chain::default();
    for _ in 0..stops {
        let Node::Decide(_) = pos.node() else { return };
        let a = bot(pos, &mut sink);
        apply(pos, &a);
    }
}

/// At every question: copies equal the original, a copy answered alike stays equal, a copy answering another option leaves the original
/// alone (and, every `REPLAY_EVERY` questions, plays on as an independent replay of the same answers does); the forked game ends as the
/// game never forked.
fn fork_everywhere(mut pos: Position, chain: &mut Chain) -> (Position, usize) {
    let spec_start = pos.fork();
    let mut answers: Vec<Answer> = Vec::new();
    let mut forks = 0;
    while let Node::Decide(choice) = pos.node() {
        let n = choice.opts.len();
        // (The bot renders the question first, as the product does: a render may mint texts into the code table.)
        let a = bot(&pos, chain);
        let same = pos.fork();
        assert!(same.same_as(&pos), "step {}: a fork differs: {:?}", pos.steps(), same.differences(&pos));
        // Another legal answer than the bot's (the bot answers by id: its option index is the one whose id it is).
        let rendered = pos.rendered().unwrap();
        let taken = rendered.options.iter().position(|o| matches!(&a, Answer::Id(id) if *id == o.id)).unwrap_or(0);
        let mut other = pos.fork();
        let before = pos.fork();
        if n > 1 {
            let k = (taken + 1) % n;
            let took = other.answer_index(k);
            match &took {
                Ok(()) | Err(crate::SearchError::Illegal | crate::SearchError::Failed(_)) => {}
                Err(e) => panic!("{e}"),
            }
            if took.is_ok() && answers.len() % REPLAY_EVERY == 0 {
                play_on(&mut other, HORIZON);
                let mut replay = spec_start.fork();
                for a in &answers {
                    apply(&mut replay, a);
                }
                replay.answer_index(k).unwrap();
                play_on(&mut replay, HORIZON);
                assert!(other.same_as(&replay), "question {}: the copy with another answer is not its replay: {:?}", answers.len(), other.differences(&replay));
                forks += 1;
            }
        }
        assert!(pos.same_as(&before), "step {}: a copy's answer changed the original: {:?}", pos.steps(), pos.differences(&before));
        let mut twin = same;
        apply(&mut pos, &a);
        apply(&mut twin, &a);
        assert!(twin.same_as(&pos), "step {}: a copy answered alike differs: {:?}", pos.steps(), twin.differences(&pos));
        answers.push(a);
        forks += 3;
    }
    (pos, forks)
}

#[test]
fn a_copy_at_every_question_of_the_quick_game_resumes_identically() {
    let mut chain = Chain::default();
    let (pos, forks) = fork_everywhere(start(&quick_spec()).unwrap(), &mut chain);
    assert!(forks > 3_000, "{forks}");
    assert_eq!(pos.steps(), QUICK_STEPS);
    assert_eq!(chain.fingerprint(&pos), QUICK_FINGERPRINT, "the forked game is the game never forked");
}

/// The 14 games of `perf_kernel suite` (players, seed, seat-a faction).
const SUITE: &[(usize, u64, &str)] = &[
    (3, 1, "sol"),
    (3, 2, "hacan"),
    (3, 3, "yssaril"),
    (4, 1, "sol"),
    (4, 2, "letnev"),
    (4, 3, "jolnar"),
    (4, 4, "mahact"),
    (4, 5, "nekro"),
    (5, 1, "xxcha"),
    (5, 2, "l1z1x"),
    (5, 3, "ghost"),
    (6, 1, "arborec"),
    (6, 2, "muaat"),
    (6, 3, "naalu"),
];

#[test]
#[ignore = "the 14 games of the perf suite, a fork at every question (minutes)"]
fn a_copy_at_every_question_of_the_suite_resumes_identically() {
    for &(players, seed, faction) in SUITE {
        let spec = crate::lobby_spec(faction, players, seed);
        let mut plain_chain = Chain::default();
        let mut plain = start(&spec).unwrap();
        while let Node::Decide(_) = plain.node() {
            let a = bot(&plain, &mut plain_chain);
            apply(&mut plain, &a);
        }
        let mut chain = Chain::default();
        let (forked, forks) = fork_everywhere(start(&spec).unwrap(), &mut chain);
        assert!(forked.same_as(&plain), "{players}p {seed} {faction}: {:?}", forked.differences(&plain));
        assert_eq!(chain.0, plain_chain.0);
        eprintln!("{players}p {seed} {faction}: {} steps, {forks} forks, {:?}", plain.steps(), plain.node());
    }
}

/// The quick game in Search mode, every chance request answered from the test's own copy of the starting streams; at every chance
/// request a copy is answered with other faces. The position's streams never move; the game is the played one.
#[test]
fn a_search_answers_every_chance_request_itself_and_never_moves_the_streams() {
    let requests = searched_is_played(&quick_spec());
    assert!(requests > 20, "{requests} chance requests");
}

#[test]
#[ignore = "the 14 games of the perf suite in Search mode (seconds)"]
fn the_suite_searched_is_the_suite_played() {
    let mut total = 0;
    for &(players, seed, faction) in SUITE {
        total += searched_is_played(&crate::lobby_spec(faction, players, seed));
    }
    eprintln!("suite: {total} chance requests answered by the search");
}

/// The game of `spec` played (Play mode) and searched (Search mode, the chance requests answered from a copy of the starting streams):
/// the same game, the searched position's streams never moved. Returns the chance requests met.
fn searched_is_played(spec: &ti4_selfplay::spec::GameSpec) -> usize {
    let mut play_chain = Chain::default();
    let mut played = start(spec).unwrap();
    while let Node::Decide(_) = played.node() {
        let a = bot(&played, &mut play_chain);
        apply(&mut played, &a);
    }
    let open = start(spec).unwrap();
    let mut own = open.core_for_tests().rng.clone();
    let positions = open.stream_positions();
    let mut pos = open.fork_search();
    assert_eq!(pos.mode(), Mode::Search);
    let mut chain = Chain::default();
    let mut other = SearchRng::new(99);
    let (mut requests, mut faces) = (0usize, 0usize);
    loop {
        assert_eq!(pos.stream_positions(), positions, "step {}: a search moved the game's streams", pos.steps());
        match pos.node() {
            Node::Decide(_) => {
                let a = bot(&pos, &mut chain);
                apply(&mut pos, &a);
            }
            Node::Chance(req) => {
                // A copy answered with other faces: its streams stay too.
                let mut copy = pos.fork();
                copy.answer_chance(other.sample(&req)).unwrap();
                assert_eq!(copy.stream_positions(), positions);
                let f = ti4_chance::answer_mut(&req, &mut own);
                requests += 1;
                faces += f.len();
                pos.answer_chance(f).unwrap();
            }
            Node::End => break,
            Node::Failed(why) => panic!("{why}"),
        }
    }
    assert_eq!(chain.0, play_chain.0, "the same questions and answers");
    assert_eq!(pos.steps(), played.steps());
    // The played game's streams end exactly where the search's own copy ends; with those, the positions are one.
    let mut core = pos.core_for_tests().clone();
    core.rng = own;
    assert!(core == *played.core_for_tests(), "the searched game is the played game");
    assert!(faces >= requests);
    requests
}

/// Lane C7: the ledger trimmed at every question plays the same game (the same answers; the positions equal but for the ledgers).
#[test]
fn the_trimmed_ledger_plays_the_same_game() {
    let (_, records, played) = quick_game();
    let mut pos = start(&quick_spec()).unwrap();
    let mut chain = Chain::default();
    let mut i = 0;
    while let Node::Decide(_) = pos.node() {
        pos.trim_ledger();
        assert!(pos.ledger_lens()[0] <= 1);
        let a = bot(&pos, &mut chain);
        assert_eq!(a, records[i].answer, "answer {i}");
        apply(&mut pos, &a);
        i += 1;
    }
    assert_eq!(i, records.len());
    let strip = |p: &Position| {
        let mut c = p.core_for_tests().clone();
        c.ledger = Ledger::default();
        c
    };
    assert!(strip(&pos) == strip(&played), "the trimmed game is the played game but for its ledgers");
    assert_eq!(pos.core_for_tests().ledger.dice, played.core_for_tests().ledger.dice, "the dice rows are kept");
}

/// A search at every tree decision of a game (the rule bot with its view at every seat, the MCTS setup) leaves the game as it is: the
/// simulations fork in Search mode, answer the chance requests from the search's own stream, run the rule bot (its movement memory is
/// saved and restored per position) and play on to the next tree decision; the real game, played on with the same seats, is the game
/// played without any search: the same answers, the same position, the same streams. The simulations are deterministic (the same halts).
#[test]
fn a_search_at_every_tree_decision_leaves_the_game_alone() {
    use crate::advance::{Halt, Rules, advance_to_tree_decision, default_tree};
    let spec = crate::mcts_spec(3);
    let rules = Rules::default();
    let play = |search: bool| {
        let mut pos = start(&spec).unwrap();
        let mut seat = RuleBotAgent::seeing();
        let mut halts = Vec::new();
        let (mut answers, mut nodes) = (Vec::new(), 0usize);
        while let Node::Decide(choice) = pos.node() {
            let choice = choice.clone();
            if search && default_tree(&pos, &choice) && nodes % 3 == 0 {
                let positions = pos.stream_positions();
                for i in 0..2u64 {
                    let mut f = pos.fork_search();
                    let mut rng = SearchRng::at(11, &[pos.decisions() as u64, i]);
                    let pick = rng.below(f.legal().len());
                    f.answer_index(pick).unwrap();
                    let mut delegate = RuleBotAgent { view: true, seed: Some(77), index: None };
                    let (halt, _) = advance_to_tree_decision(&mut f, &mut delegate, &mut rng, &rules).unwrap();
                    assert_eq!(f.stream_positions(), positions, "a simulation moved the streams");
                    match halt {
                        Halt::Tree => assert!(default_tree(&f, f.choice().unwrap())),
                        Halt::Chance(req) => assert!(crate::chance::is_hit_node(&req)),
                        Halt::End => assert!(f.is_terminal()),
                    }
                    halts.push((halt, f.state_hash()));
                }
                assert_eq!(pos.stream_positions(), positions);
            }
            nodes += usize::from(default_tree(&pos, &choice));
            let d = seat.decide(&pos, &choice).unwrap();
            answers.push(d.answer.clone());
            pos.apply(d).unwrap();
        }
        assert!(matches!(pos.node(), Node::End), "{:?}", pos.node());
        (pos, answers, halts)
    };
    let (plain, plain_answers, _) = play(false);
    let (searched, answers, halts) = play(true);
    assert_eq!(answers, plain_answers);
    assert!(searched.same_as(&plain), "{:?}", searched.differences(&plain));
    assert!(halts.len() > 100, "{} simulations", halts.len());
    let (_, _, again) = play(true);
    assert_eq!(again, halts, "the simulations are deterministic");
}

/// The combat rolls ask with their thresholds (the core's `Sx::dice_on`, lane C6): a searched game of the MCTS setup (the search's own
/// faces) meets chance nodes whose hit classes are all legal answers, each class a different continuation; the streams stay put; a fork
/// shares the ledgers with its original (copy-on-write, nothing copied).
#[test]
fn the_combat_rolls_are_chance_nodes_with_hit_classes() {
    let mut pos = start(&crate::mcts_spec(3)).unwrap().fork_search();
    let positions = pos.stream_positions();
    let mut rng = SearchRng::new(1);
    let mut seat = RuleBotAgent::seeing();
    let (mut with, mut without, mut classes) = (0usize, 0usize, 0usize);
    loop {
        match pos.node() {
            Node::Decide(c) => {
                let c = c.clone();
                let d = seat.decide(&pos, &c).unwrap();
                pos.apply(d).unwrap();
            }
            Node::Chance(req) => {
                if crate::chance::is_hit_node(&req) {
                    with += 1;
                    let outcomes = crate::chance::hit_outcomes(&req).unwrap();
                    let mut seen = Vec::new();
                    for (faces, p) in &outcomes {
                        assert!(*p > 0.0);
                        let mut f = pos.fork();
                        assert!(f.core_for_tests().ledger.events.shares_prefix_with(&pos.core_for_tests().ledger.events), "a fork shares the ledger");
                        f.answer_chance(*faces).unwrap();
                        assert_eq!(f.stream_positions(), positions);
                        seen.push(f.state_hash());
                        classes += 1;
                    }
                    if outcomes.len() > 1 {
                        seen.sort_unstable();
                        seen.dedup();
                        assert_eq!(seen.len(), outcomes.len(), "each hit class is another continuation");
                    }
                } else {
                    without += 1;
                }
                let f = rng.sample(&req);
                pos.answer_chance(f).unwrap();
            }
            Node::End => break,
            Node::Failed(why) => panic!("{why}"),
        }
    }
    assert_eq!(pos.stream_positions(), positions);
    assert!(with > 50 && with >= 10 * without, "{with} requests with a threshold, {without} without");
    eprintln!("{with} chance nodes ({classes} hit classes), {without} sampled requests");
}

/// The policy hook (greedy on the tree asks, the rule bot on the rest) and a random seat play through the driver, deterministically.
#[test]
fn a_policy_seat_and_a_random_seat_play_deterministic_games() {
    use crate::agent::{PolicyAgent, RandomAgent};
    let run = || {
        let mut pos = start(&crate::mcts_spec(5)).unwrap();
        let mut agents: Vec<Box<dyn SeatAgent>> = vec![
            Box::new(PolicyAgent::new(Box::new(|_, _, legal| (0..legal.len()).map(|i| -(i as f32)).collect()))),
            Box::new(RandomAgent(SearchRng::new(3))),
            Box::new(RuleBotAgent::seeing()),
            Box::new(RuleBotAgent::blind()),
        ];
        let mut answers = Vec::new();
        let ended = driver::play(&mut pos, &mut agents, Limits { max_steps: 20_000, max_rounds: 3 }, true, |_, r| answers.push(r.clone()));
        assert_eq!(ended.error, None);
        (answers, pos.state_hash(), ended)
    };
    let (a, h, ended) = run();
    assert!(ended.steps > 100, "{ended:?}");
    let (b, h2, _) = run();
    assert_eq!(a, b);
    assert_eq!(h, h2);
}
