//! Lane C5 (docs/C5_RESUME.md): the thin `Game` copied at EVERY question, not only between two steps.
//!
//! A game is played with `Game::pull` / `Game::answer` (the step cut at its questions; feature `search` of the thin crate). At every question:
//!
//! * the game is copied (`fork_keeping_log`). One copy answers ANOTHER legal option (the first one the bot did not pick; the same one when there
//!   is only one), plays to the end of the step and `HORIZON` more steps with the bot. An independent replay does the same from the snapshot of
//!   the step boundary through the BLOCKING `Game::step` (a scripted decider with this step's answers so far and the other option, then the
//!   bot): after the step and after each further step the two must be equal in the fingerprint (the position as JSON and the chain of the
//!   journal), the events, the rolls, the step count, the observation latch (randomness, hidden information), the questions the deciders were
//!   shown, the result of each step (an error included) and the core position (`Game::position_differences`: the core, and at a question the
//!   step's own state);
//! * a second copy forgets what the core remembers of its question (the nestq memo and the parked step of a strategy card: they are derived,
//!   V5) and answers the same other option: at the end of the step it equals the first copy;
//! * a third copy answers what the original answers: at the end of the step it equals the original;
//! * the original, compared with a copy taken before the copies played, is unchanged.
//!
//! The original goes on with the bot's answer, and at the end its fingerprint is the one of the game played with the blocking `Game::step`
//! (the quick game: `43c985a35397f2c9` since chance-2 C3b, 1,165 steps). At every `LOAD_EVERY`-th question the game is also saved and loaded: the decision
//! journal replayed from the opening through `Game::replayed` stands at the same question in the same position, and goes on identically.
//!
//! `cargo test --profile lockstep -p ti4-engine-core-bridge --test fork_mid` runs the quick game; `-- --ignored` the 14 games of
//! `perf_kernel suite` (every question of each).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use sha2::{Digest, Sha256};
use ti4_content::ContentStore;
use ti4_engine::native::{Game, Pulled, StepResult};
use ti4_engine::wire::choice::{Choice, ChoiceOption, Decider, IllegalChoice, Scripted, Table};
use ti4_engine::wire::observation::ExecutionObservation;
use ti4_engine_core_bridge::native_play::lobby_spec;

/// Steps a copy plays after the step it was copied in.
const HORIZON: usize = 2;
/// Every this many questions the game is also saved and loaded (a replay from the opening: O(game) each).
const LOAD_EVERY: usize = 150;

/// The bot of the native playthrough (the rule bot's random pick, seeded by game seed, seat place and decision index).
#[derive(Clone)]
struct Bot {
    seed: u64,
    seats: Vec<String>,
}

impl Bot {
    fn pick(&self, choice: &Choice, index: usize) -> String {
        let place = self.seats.iter().position(|s| s == choice.player.as_str()).unwrap_or(0);
        let (picked, _, _) = ti4_rule_bot::choose_explained(choice, None, self.seed, place, index);
        choice.options[picked.min(choice.options.len() - 1)].id.clone()
    }

    /// Another legal answer than `taken` (the first option that is not it), or `taken` when it is the only one.
    fn other(choice: &Choice, taken: &str) -> String {
        choice.options.iter().map(|o| o.id.clone()).find(|id| id != taken).unwrap_or_else(|| taken.to_owned())
    }
}

/// The bot as a table decider; `index` counts its answers (it starts at the journal's length, so it gives the pull path's answers).
struct BotDecider {
    bot: Bot,
    index: Arc<AtomicUsize>,
}

impl Decider for BotDecider {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        let id = self.bot.pick(choice, self.index.fetch_add(1, Ordering::SeqCst));
        Ok(choice.option(&id).expect("an option").clone())
    }
}

/// Records every question a decider is asked, then lets `inner` answer.
struct Recording {
    inner: Box<dyn Decider>,
    seen: Rc<RefCell<Vec<Choice>>>,
}

impl Decider for Recording {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.seen.borrow_mut().push(choice.clone());
        self.inner.choose(choice)
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// `NativeGame::fingerprint`: the position (JSON) and the chain of the answers.
fn fingerprint(game: &Game<'_>) -> String {
    let mut chain = String::new();
    for r in &game.table.log.records {
        let mut h = Sha256::new();
        h.update(chain.as_bytes());
        h.update(format!("{}\u{1}{}\u{1}{}", r.player, r.prompt, r.chosen).as_bytes());
        chain = hex(&h.finalize());
    }
    let mut h = Sha256::new();
    h.update(serde_json::to_value(&game.state).unwrap().to_string().as_bytes());
    h.update(chain.as_bytes());
    hex(&h.finalize())[..16].to_owned()
}

fn new_game(players: usize, seed: u64, faction: &str) -> (Game<'static>, Bot) {
    let spec = lobby_spec(faction, players, seed);
    let legacy = spec.build().expect("a lobby game builds");
    let seats: Vec<String> = legacy.state.players.iter().map(|p| p.id.as_str().to_owned()).collect();
    let galaxy = legacy.galaxy().cloned().expect("a lobby game has a map");
    let game = Game::try_with_table(legacy.state.clone(), ContentStore::embedded(), Table::new()).expect("playable").with_galaxy(galaxy);
    (game, Bot { seed, seats })
}

/// The game played with the blocking step: (fingerprint, steps).
fn blocking_reference(players: usize, seed: u64, faction: &str) -> (String, usize) {
    let (mut game, bot) = new_game(players, seed, faction);
    game.install_table(Table::with_default(Box::new(BotDecider { bot, index: Arc::default() })));
    while !game.state.finished {
        let r = game.step();
        assert_eq!(r.error, None, "{players}p seed {seed} {faction}: blocking step {}", game.steps());
        assert!(game.steps() < 20_000);
    }
    (fingerprint(&game), game.steps())
}

/// What two games must agree in.
#[derive(Debug, PartialEq)]
struct Look {
    fingerprint: String,
    events: usize,
    events_tail: Vec<String>,
    rolls: Vec<ti4_engine::wire::dice::Roll>,
    steps: usize,
    pending: Option<Choice>,
    activity: Option<ti4_engine::wire::observation::ObservedActivity>,
    finished: bool,
}

fn look(game: &Game<'_>, observation: Option<&ExecutionObservation>) -> Look {
    Look {
        fingerprint: fingerprint(game),
        events: game.events.len(),
        events_tail: game.events.iter().rev().take(40).cloned().collect(),
        rolls: game.rolls().to_vec(),
        steps: game.steps(),
        pending: game.pending_question().cloned(),
        activity: observation.map(ExecutionObservation::activity),
        finished: game.state.finished,
    }
}

fn same(a: &Game<'_>, oa: Option<&ExecutionObservation>, b: &Game<'_>, ob: Option<&ExecutionObservation>, what: &dyn Fn() -> String) {
    let (la, lb) = (look(a, oa), look(b, ob));
    if la != lb {
        let mut diff = Vec::new();
        if la.fingerprint != lb.fingerprint {
            diff.push(format!("fingerprint {} vs {}", la.fingerprint, lb.fingerprint));
        }
        if la.events != lb.events || la.events_tail != lb.events_tail {
            diff.push(format!("events {} vs {} (last {:?} vs {:?})", la.events, lb.events, la.events_tail.first(), lb.events_tail.first()));
        }
        if la.rolls != lb.rolls {
            diff.push(format!("rolls {} vs {}", la.rolls.len(), lb.rolls.len()));
        }
        if la.steps != lb.steps {
            diff.push(format!("steps {} vs {}", la.steps, lb.steps));
        }
        if la.pending != lb.pending {
            diff.push(format!("pending {:?} vs {:?}", la.pending.as_ref().map(|c| &c.prompt), lb.pending.as_ref().map(|c| &c.prompt)));
        }
        if la.activity != lb.activity {
            diff.push(format!("observation {:?} vs {:?}", la.activity, lb.activity));
        }
        panic!("{}: {}", what(), diff.join("; "));
    }
    assert_eq!(full_events(a), full_events(b), "{}: the events", what());
    let inner = a.position_differences(b);
    assert!(inner.is_empty(), "{}: the positions differ in {inner:?}", what());
}

fn full_events<'g>(game: &'g Game<'_>) -> &'g [String] {
    &game.events
}

/// Answer the questions of the step the game stands in with the bot, up to its end. Returns the step's result and the questions asked.
fn finish_step(game: &mut Game<'_>, bot: &Bot, mut pulled: Pulled, seen: &mut Vec<Choice>) -> StepResult {
    loop {
        match pulled {
            Pulled::Stepped(result) => return result,
            Pulled::Asks(q) => {
                let id = bot.pick(&q, game.table.log.records.len());
                seen.push(*q);
                pulled = game.answer(&id).expect("the bot's answer is an option");
            }
        }
    }
}

/// One whole step through the pull path.
fn pull_step(game: &mut Game<'_>, bot: &Bot, seen: &mut Vec<Choice>) -> StepResult {
    let pulled = game.pull();
    finish_step(game, bot, pulled, seen)
}

#[derive(Default, Debug)]
struct Counts {
    asks: usize,
    divergent: usize,
    nested: usize,
    loads: usize,
    copy_errors: usize,
    steps_compared: usize,
}

/// The test of the module text over one game.
fn fork_at_every_question(players: usize, seed: u64, faction: &str) -> Counts {
    let label = format!("{players}p seed {seed} {faction}");
    let (reference, reference_steps) = blocking_reference(players, seed, faction);
    let (mut game, bot) = new_game(players, seed, faction);
    let observation = ExecutionObservation::default();
    game.bind_observation(observation.clone());
    let opening = game.snapshot();
    let mut counts = Counts::default();
    let mut answered_in_step: Vec<String> = Vec::new();
    while !game.state.finished {
        let boundary = game.snapshot();
        answered_in_step.clear();
        let mut pulled = game.pull();
        let mut same_answer_copies: Vec<Game<'_>> = Vec::new();
        while let Pulled::Asks(q) = pulled {
            counts.asks += 1;
            let taken = bot.pick(&q, game.table.log.records.len());
            let other = Bot::other(&q, &taken);
            if other != taken {
                counts.divergent += 1;
            }
            let at = || format!("{label}: step {} question {} ({:?}, answer {other:?} instead of {taken:?})", game.steps(), answered_in_step.len(), q.prompt);
            let saved = game.fork_keeping_log();
            // 1. The copy with the other answer, against the replay through the blocking step.
            {
                let mut copy = game.fork_keeping_log();
                let copy_obs = ExecutionObservation::default();
                copy.bind_observation(copy_obs.clone());
                let mut replay = boundary.clone().into_game();
                let replay_obs = ExecutionObservation::default();
                replay.bind_observation(replay_obs.clone());
                let script: Vec<String> = answered_in_step.iter().cloned().chain(std::iter::once(other.clone())).collect();
                let replay_seen = Rc::new(RefCell::new(Vec::new()));
                let index = Arc::new(AtomicUsize::new(boundary.decisions().len() + script.len()));
                let fallback = Box::new(BotDecider { bot: bot.clone(), index });
                replay.install_table(Table::with_default(Box::new(Recording { inner: Box::new(Scripted::with_fallback(script.clone(), fallback)), seen: replay_seen.clone() })));
                let mut copy_seen = vec![(*q).clone()];
                let first = copy.answer(&other).expect("another option is legal");
                let copy_result = finish_step(&mut copy, &bot, first, &mut copy_seen);
                let replay_result = replay.step();
                assert_eq!(copy_result, replay_result, "{}: the step's result", at());
                same(&copy, Some(&copy_obs), &replay, Some(&replay_obs), &|| format!("{} after its step", at()));
                // 2. The copy without the core's memos.
                {
                    let mut bare = game.fork_keeping_log();
                    bare.forget_memos();
                    let first = bare.answer(&other).expect("another option is legal");
                    let bare_result = finish_step(&mut bare, &bot, first, &mut Vec::new());
                    assert_eq!(bare_result, copy_result, "{}: the step's result without the memos", at());
                    same(&bare, None, &copy, None, &|| format!("{} without the memos", at()));
                }
                counts.steps_compared += 1;
                let mut stopped = copy_result.error.is_some();
                if stopped {
                    counts.copy_errors += 1;
                }
                for k in 0..HORIZON {
                    if stopped || copy.state.finished {
                        break;
                    }
                    let copy_result = pull_step(&mut copy, &bot, &mut copy_seen);
                    let replay_result = replay.step();
                    assert_eq!(copy_result, replay_result, "{}: the result of step +{}", at(), k + 1);
                    same(&copy, Some(&copy_obs), &replay, Some(&replay_obs), &|| format!("{} after step +{}", at(), k + 1));
                    counts.steps_compared += 1;
                    stopped = copy_result.error.is_some();
                }
                let replay_seen = replay_seen.borrow();
                assert!(replay_seen.len() >= answered_in_step.len(), "{}: the replay asked fewer questions than the step had", at());
                assert_eq!(&replay_seen[answered_in_step.len()..], copy_seen.as_slice(), "{}: the questions after the copy", at());
            }
            // 3. The copy with the original's answer: compared with the original at the end of the step.
            {
                let mut twin = game.fork_keeping_log();
                let first = twin.answer(&taken).expect("the bot's answer is an option");
                let _ = finish_step(&mut twin, &bot, first, &mut Vec::new());
                same_answer_copies.push(twin);
            }
            // 4. The original is what it was.
            same(&game, None, &saved, None, &|| format!("{}: the original after its copies played", at()));
            // 5. Saved and loaded at this question.
            if counts.asks % LOAD_EVERY == 0 {
                counts.loads += 1;
                let answers = game.table.log.as_script(None);
                let mut fresh = opening.clone().into_game();
                fresh.bind_observation(ExecutionObservation::default());
                let mut loaded = Game::replayed(fresh, &answers).unwrap_or_else(|e| panic!("{}: the replay of the journal fails: {e}", at()));
                same(&loaded, None, &game, None, &|| format!("{}: loaded from the journal", at()));
                let mut original = game.fork_keeping_log();
                for (copy, label) in [(&mut loaded, "loaded"), (&mut original, "original")] {
                    let first = copy.answer(&taken).expect("an option");
                    let r = finish_step(copy, &bot, first, &mut Vec::new());
                    assert_eq!(r.error, None, "{}: {label}", at());
                    let r = pull_step(copy, &bot, &mut Vec::new());
                    let _ = r;
                }
                same(&loaded, None, &original, None, &|| format!("{}: loaded, one step on", at()));
            }
            drop(saved);
            answered_in_step.push(taken.clone());
            pulled = game.answer(&taken).expect("the bot's answer is an option");
        }
        let Pulled::Stepped(result) = pulled else { unreachable!() };
        assert_eq!(result.error, None, "{label}: step {}", game.steps());
        for twin in &same_answer_copies {
            same(twin, None, &game, None, &|| format!("{label}: step {}: the copy with the same answer", game.steps()));
        }
        if answered_in_step.len() > 1 {
            counts.nested += answered_in_step.len() - 1;
        }
        assert!(game.steps() < 20_000);
    }
    assert_eq!(game.steps(), reference_steps, "{label}: the pulled game plays the steps of the blocking one");
    assert_eq!(fingerprint(&game), reference, "{label}: the pulled game, copied at every question, ends as the blocking one");
    eprintln!("C5 {label}: {counts:?}, fingerprint {reference}");
    counts
}

#[test]
fn the_quick_game_copied_at_every_question_continues_as_its_replay_and_the_original_is_untouched() {
    let counts = fork_at_every_question(4, 1, "sol");
    assert!(counts.asks > 500 && counts.divergent > 100 && counts.loads > 2, "{counts:?}");
}

#[test]
fn the_quick_game_played_by_pull_is_the_blocking_game() {
    let (reference, steps) = blocking_reference(4, 1, "sol");
    assert_eq!((reference.as_str(), steps), ("43c985a35397f2c9", 1165));
    let (mut game, bot) = new_game(4, 1, "sol");
    while !game.state.finished {
        let r = pull_step(&mut game, &bot, &mut Vec::new());
        assert_eq!(r.error, None);
    }
    assert_eq!((fingerprint(&game), game.steps()), (reference, steps));
}

/// The 14 games of `perf_kernel suite` (3p..6p, several faction mixes).
const SUITE: &[(usize, u64, &str)] = &[
    (3, 1, "sol"), (3, 2, "hacan"), (3, 3, "yssaril"),
    (4, 1, "sol"), (4, 2, "letnev"), (4, 3, "jolnar"), (4, 4, "mahact"), (4, 5, "nekro"),
    (5, 1, "xxcha"), (5, 2, "l1z1x"), (5, 3, "ghost"),
    (6, 1, "arborec"), (6, 2, "muaat"), (6, 3, "naalu"),
];

#[test]
#[ignore = "the 14-game suite, every question: minutes (run with --ignored in the lockstep profile)"]
fn the_suite_copied_at_every_question_continues_as_its_replay() {
    let only = std::env::var("C5_GAME").ok();
    let mut total = Counts::default();
    for &(players, seed, faction) in SUITE {
        if only.as_deref().is_some_and(|o| o != format!("{players}-{seed}-{faction}")) {
            continue;
        }
        let c = fork_at_every_question(players, seed, faction);
        total.asks += c.asks;
        total.divergent += c.divergent;
        total.nested += c.nested;
        total.loads += c.loads;
        total.copy_errors += c.copy_errors;
        total.steps_compared += c.steps_compared;
    }
    eprintln!("C5 suite total: {total:?}");
}
