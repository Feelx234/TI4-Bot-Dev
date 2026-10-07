//! H7 turn redo: rewind a seat's last turn, play a new one, auto-play the others' recorded
//! decisions with forced dice.
//!
//! Histories come from real engine games driven by a reproducible pseudo-random decider, so the
//! tests exercise the actual question stream rather than hand-built records.

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine::choice::{Choice, ChoiceOption, Decider, DecisionRecord, IllegalChoice, Table};
use ti4_engine::dice::Roll;
use ti4_engine::game::Game;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;
use ti4_server::protocol::splice::ConflictKind;
use ti4_server::protocol::turn_redo::{DeckDelta, TurnRedoStop};
use ti4_server::session::turn_redo::{
    AutoplayResult, RedoWindow, TailBaseline, TailSource, TurnRedoError, autoplay, find_turns,
    redo_window,
};
use ti4_server::session::{
    GameSession, RngForce, RngMarks, SeatController, SessionConfig, replay_session_forced,
};

/// A tiny LCG picks options, so histories are varied but reproducible. The aggressive flavour
/// marches fleets at each other, which is what makes dice.
pub struct Lcg(pub u64, pub bool);

impl Decider for Lcg {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let roll = usize::try_from(self.0 >> 33).unwrap();
        if self.1 {
            let pushy: Vec<usize> = choice
                .options
                .iter()
                .enumerate()
                .filter(|(_, o)| {
                    o.id.starts_with("move|") || o.id == "tactical" || o.id.starts_with("build|")
                })
                .map(|(i, _)| i)
                .collect();
            if !pushy.is_empty() && roll % 10 < 8 {
                return Ok(choice.options[pushy[roll / 10 % pushy.len()]].clone());
            }
        }
        Ok(choice.options[roll % choice.options.len()].clone())
    }
}

/// Answers from a recorded list first, then hands over to another decider.
struct PrefixThen {
    prefix: Vec<String>,
    at: usize,
    then: Box<dyn Decider>,
}

impl Decider for PrefixThen {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        if let Some(wanted) = self.prefix.get(self.at) {
            self.at += 1;
            return choice
                .option(wanted)
                .cloned()
                .ok_or_else(|| IllegalChoice::ScriptDiverged {
                    player: choice.player.clone(),
                    wanted: wanted.clone(),
                    offered: choice.ids().into_iter().map(str::to_owned).collect(),
                });
        }
        self.then.choose(choice)
    }
}

pub fn setup(seed: u64) -> (GameState, Galaxy) {
    let players = vec![
        PlayerId::new("p1"),
        PlayerId::new("p2"),
        PlayerId::new("p3"),
    ];
    ti4_server::map::create_game_with_map(ContentStore::embedded(), &players, seed).unwrap()
}

pub fn play(
    state: &GameState,
    galaxy: &Galaxy,
    decider: Box<dyn Decider>,
    max: usize,
) -> Vec<DecisionRecord> {
    let table = Table::with_default(decider);
    let mut game = Game::with_table(state.clone(), ContentStore::embedded(), table)
        .with_galaxy(galaxy.clone());
    while game.table.log.records.len() < max {
        let result = game.step();
        if result.error.is_some() || result.finished {
            break;
        }
    }
    game.table.log.records
}

/// The history of a seeded game (`aggressive` marches fleets at each other).
pub fn history(
    seed: u64,
    aggressive: bool,
    max: usize,
) -> (GameState, Galaxy, Vec<DecisionRecord>) {
    let (state, galaxy) = setup(seed);
    let h = play(&state, &galaxy, Box::new(Lcg(1, aggressive)), max);
    (state, galaxy, h)
}

/// Replay `prefix`, then let `then` play live until `seat`'s turn starting at `window_start` is
/// complete. `None` when the game ends or stalls first.
pub fn play_new_turn(
    state: &GameState,
    galaxy: &Galaxy,
    prefix: &[DecisionRecord],
    seat: &PlayerId,
    then: Box<dyn Decider>,
) -> Option<Vec<DecisionRecord>> {
    let window_start = prefix.len();
    let ids = prefix.iter().map(|r| r.chosen.clone()).collect();
    let table = Table::with_default(Box::new(PrefixThen {
        prefix: ids,
        at: 0,
        then,
    }));
    let mut game = Game::with_table(state.clone(), ContentStore::embedded(), table)
        .with_galaxy(galaxy.clone());
    loop {
        let log = &game.table.log.records;
        if log.len() > window_start
            && find_turns(log)
                .iter()
                .any(|t| &t.seat == seat && t.start == window_start && t.complete)
        {
            return Some(game.table.log.records);
        }
        if log.len() > window_start + 150 {
            return None;
        }
        let result = game.step();
        if result.error.is_some() || result.finished {
            return None;
        }
    }
}

/// Dice rolled after the engine had recorded `from_len` decisions, replaying `decisions` with
/// `marks` forced.
pub fn replay_rolls(
    state: &GameState,
    galaxy: &Galaxy,
    decisions: &[DecisionRecord],
    marks: &RngMarks,
    from_len: usize,
) -> Vec<Roll> {
    struct Forced {
        script: Vec<String>,
        at: usize,
        force: Option<RngForce>,
    }
    impl Decider for Forced {
        fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
            let index = self.at;
            let Some(wanted) = self.script.get(index) else {
                return Err(IllegalChoice::DeciderFailed {
                    player: choice.player.clone(),
                    prompt: choice.prompt.clone(),
                    reason: "done".to_owned(),
                });
            };
            self.at += 1;
            if let Some(force) = &self.force {
                force.before_answer(index);
            }
            choice
                .option(wanted)
                .cloned()
                .ok_or_else(|| IllegalChoice::ScriptDiverged {
                    player: choice.player.clone(),
                    wanted: wanted.clone(),
                    offered: Vec::new(),
                })
        }
    }
    let force = RngForce::new(marks);
    let table = Table::with_default(Box::new(Forced {
        script: decisions.iter().map(|r| r.chosen.clone()).collect(),
        at: 0,
        force: force.clone(),
    }));
    let mut game = Game::with_table(state.clone(), ContentStore::embedded(), table)
        .with_galaxy(galaxy.clone());
    if let Some(force) = &force {
        force.attach(&mut game);
    }
    let mut at_from: Option<usize> = None;
    while game.table.log.records.len() < decisions.len() || at_from.is_none() {
        let result = game.step();
        if at_from.is_none() && game.table.log.records.len() >= from_len {
            at_from = Some(game.dice().count());
        }
        if result.error.is_some() || result.finished {
            break;
        }
    }
    let from = at_from.unwrap_or_else(|| game.dice().count());
    game.dice().history()[from..].to_vec()
}

/// One redo, computed without a server.
pub struct Redo {
    pub window: RedoWindow,
    pub source: TailSource,
    pub current: Vec<DecisionRecord>,
    pub result: AutoplayResult,
}

#[expect(clippy::too_many_arguments, reason = "a test fixture")]
pub fn redo(
    state: &GameState,
    galaxy: &Galaxy,
    base: &TailBaseline,
    orig: &[DecisionRecord],
    orig_marks: &RngMarks,
    seat: &PlayerId,
    turns: u8,
    then: Box<dyn Decider>,
) -> Option<Redo> {
    let window = redo_window(orig, seat, turns).ok()?;
    let source = base.tail(&window).ok()?;
    let current = play_new_turn(state, galaxy, &orig[..window.start], seat, then)?;
    let mut current_marks = orig_marks.clone();
    current_marks.retain(|i, _| *i < window.start);
    let result = autoplay(
        state,
        Some(galaxy),
        &current,
        &current_marks,
        window.start,
        seat,
        &source,
    )
    .ok()?;
    Some(Redo {
        window,
        source,
        current,
        result,
    })
}

pub fn baseline(state: &GameState, galaxy: &Galaxy, h: &[DecisionRecord]) -> TailBaseline {
    let base = TailBaseline::new(state, Some(galaxy), h, &RngMarks::new(), &[]);
    assert_eq!(base.replayed(), h.len(), "the fixture replays strictly");
    base
}

/// Answers the recorded ids in order, then the first option.
pub struct Ids(pub Vec<String>, pub usize);

impl Decider for Ids {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        let wanted = self.0.get(self.1).cloned();
        self.1 += 1;
        Ok(wanted
            .and_then(|w| choice.option(&w).cloned())
            .unwrap_or_else(|| choice.options[0].clone()))
    }
}

/// Who the engine asks right after `decisions` (and what), replaying with `marks` forced.
pub fn next_asker(
    state: &GameState,
    galaxy: &Galaxy,
    decisions: &[DecisionRecord],
    marks: &RngMarks,
) -> Option<(PlayerId, String)> {
    use std::sync::{Arc, Mutex};
    struct Asker {
        script: Vec<String>,
        at: usize,
        force: Option<RngForce>,
        asked: Arc<Mutex<Option<(PlayerId, String)>>>,
    }
    impl Decider for Asker {
        fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
            let index = self.at;
            let Some(wanted) = self.script.get(index) else {
                *self.asked.lock().unwrap() = Some((choice.player.clone(), choice.prompt.clone()));
                return Err(IllegalChoice::DeciderFailed {
                    player: choice.player.clone(),
                    prompt: choice.prompt.clone(),
                    reason: "asked".to_owned(),
                });
            };
            self.at += 1;
            if let Some(force) = &self.force {
                force.before_answer(index);
            }
            Ok(choice.option(wanted).cloned().expect("on offer"))
        }
    }
    let asked = Arc::new(Mutex::new(None));
    let force = RngForce::new(marks);
    let table = Table::with_default(Box::new(Asker {
        script: decisions.iter().map(|r| r.chosen.clone()).collect(),
        at: 0,
        force: force.clone(),
        asked: Arc::clone(&asked),
    }));
    let mut game = Game::with_table(state.clone(), ContentStore::embedded(), table)
        .with_galaxy(galaxy.clone());
    if let Some(force) = &force {
        force.attach(&mut game);
    }
    for _ in 0..decisions.len() * 8 + 64 {
        let result = game.step();
        if asked.lock().unwrap().is_some() || result.finished {
            break;
        }
    }
    let found = asked.lock().unwrap().clone();
    found
}

pub struct Scenario {
    pub seat: PlayerId,
    pub cut: usize,
    pub turns: u8,
    pub seed: u64,
    pub hh: Vec<DecisionRecord>,
    pub redo: Redo,
}

/// A grid of real redos over one seeded game, computed once for every test of this file.
pub fn scenarios() -> &'static (GameState, Galaxy, Vec<Scenario>) {
    use std::sync::OnceLock;
    static CELL: OnceLock<(GameState, Galaxy, Vec<Scenario>)> = OnceLock::new();
    CELL.get_or_init(|| {
        let (state, galaxy, h) = history(7, true, 300);
        let mut out = Vec::new();
        for cut in [120usize, 200, 300] {
            let hh = h[..cut].to_vec();
            let base = baseline(&state, &galaxy, &hh);
            for seat in ["p1", "p2", "p3"] {
                let seat = PlayerId::new(seat);
                for turns in [1u8, 2] {
                    for seed in [1u64, 2] {
                        if let Some(redo) = redo(
                            &state,
                            &galaxy,
                            &base,
                            &hh,
                            &RngMarks::new(),
                            &seat,
                            turns,
                            Box::new(Lcg(seed, true)),
                        ) {
                            out.push(Scenario {
                                seat: seat.clone(),
                                cut,
                                turns,
                                seed,
                                hh: hh.clone(),
                                redo,
                            });
                        }
                    }
                }
            }
        }
        (state, galaxy, out)
    })
}

pub fn kind(stop: &TurnRedoStop) -> String {
    match stop {
        TurnRedoStop::Handoff { .. } => "handoff".to_owned(),
        TurnRedoStop::TailExhausted => "exhausted".to_owned(),
        TurnRedoStop::Conflict { conflict } => format!("{:?}", conflict.kind),
    }
}

// ---- tests ---------------------------------------------------------------------------------

fn ids(records: &[DecisionRecord]) -> Vec<String> {
    records.iter().map(|r| r.chosen.clone()).collect()
}

fn final_state_json(
    state: &GameState,
    galaxy: &Galaxy,
    decisions: &[DecisionRecord],
    marks: &RngMarks,
) -> String {
    let report = replay_session_forced(state, Some(galaxy), decisions, marks).expect("replays");
    assert!(report.hashes_match, "the log replays to the same records");
    assert_eq!(report.decision_count, decisions.len());
    serde_json::to_string(&report.final_state).unwrap()
}

#[test]
fn turn_spans_open_with_the_menu_and_do_not_overlap() {
    let (_, _, all) = scenarios();
    let h = &all.iter().max_by_key(|s| s.cut).unwrap().hh;
    let spans = find_turns(h);
    assert!(
        spans.len() > 20,
        "a 300-decision game has many turns: {}",
        spans.len()
    );
    let mut at = 0;
    for span in &spans {
        assert!(span.start >= at, "turns do not overlap");
        assert_eq!(h[span.start].prompt, "action phase");
        assert_eq!(h[span.start].player, span.seat);
        assert!(span.end > span.start);
        at = span.end;
    }
    assert!(spans.iter().all(|t| t.complete || t.end == h.len()));
}

#[test]
fn every_redo_keeps_the_new_turn_then_others_recorded_decisions_unchanged() {
    let (state, galaxy, all) = scenarios();
    assert!(
        all.len() >= 30,
        "the grid should produce many redos, got {}",
        all.len()
    );
    let mut kept_any = 0;
    for s in all {
        let r = &s.redo;
        let result = &r.result;
        let tail_start = r.window.end;
        // The new turn is exactly what was played; the kept tail is exactly the recorded one.
        assert_eq!(
            result.decisions[..result.prefix_len],
            r.current[..result.prefix_len]
        );
        assert_eq!(
            result.decisions[result.prefix_len..],
            s.hh[tail_start..tail_start + result.kept],
            "{} cut {} turns {} seed {}",
            s.seat,
            s.cut,
            s.turns,
            s.seed
        );
        assert_eq!(result.decisions.len(), result.prefix_len + result.kept);
        assert!(result.kept <= result.tail_total);
        kept_any += usize::from(result.kept > 0);
        // The history replays by itself, with its stored marks, under the fixed seed.
        let report =
            replay_session_forced(state, Some(galaxy), &result.decisions, &result.marks).unwrap();
        assert!(report.hashes_match);
        assert_eq!(
            report.final_state.rng_seed, state.rng_seed,
            "the seed is never changed"
        );
        // Marks only exist for the join and the kept tail.
        assert!(
            result
                .marks
                .keys()
                .all(|i| *i + 1 >= result.prefix_len && *i < result.decisions.len())
        );
    }
    assert!(
        kept_any >= 15,
        "others' decisions survive often enough to be useful: {kept_any}"
    );
}

#[test]
fn an_unchanged_turn_reproduces_the_original_history_and_stops_at_the_seats_next_decision() {
    let (state, galaxy, h) = history(7, true, 200);
    let base = baseline(&state, &galaxy, &h);
    for seat in ["p1", "p2", "p3"] {
        let seat = PlayerId::new(seat);
        let window = redo_window(&h, &seat, 1).unwrap();
        let same = Box::new(Ids(ids(&h[window.start..window.end]), 0));
        let r = redo(&state, &galaxy, &base, &h, &RngMarks::new(), &seat, 1, same).expect("redo");
        // A turn still in progress at the end of the history is finished past it.
        let n = r.result.decisions.len().min(h.len());
        assert_eq!(
            r.result.decisions[..n],
            h[..n],
            "{seat}: replaying the same turn changes nothing"
        );
        assert!(
            !matches!(r.result.stop, TurnRedoStop::Conflict { .. }),
            "{seat}: {:?}",
            r.result.stop
        );
        assert!(r.result.deck_offsets.is_empty());
        // Same history, same dice: the state equals the original's at that length.
        if r.result.decisions.len() <= h.len() {
            let again = final_state_json(&state, &galaxy, &r.result.decisions, &r.result.marks);
            let orig = final_state_json(
                &state,
                &galaxy,
                &h[..r.result.decisions.len()],
                &RngMarks::new(),
            );
            assert_eq!(again, orig);
        }
    }
}

#[test]
fn a_conflict_hands_control_to_the_seat_the_engine_asks() {
    let (state, galaxy, all) = scenarios();
    let mut checked = [0usize; 3];
    for s in all {
        let r = &s.result_view();
        let asker = next_asker(state, galaxy, &r.decisions, &r.marks);
        match &r.stop {
            TurnRedoStop::Handoff { seat } => {
                assert_eq!(asker.map(|(p, _)| p.to_string()), Some(seat.clone()));
                checked[0] += 1;
            }
            TurnRedoStop::Conflict { conflict } => {
                let (asked, prompt) = asker.expect("the engine asks someone");
                assert_eq!(asked.to_string(), conflict.seat, "{:?}", conflict.kind);
                assert_eq!(prompt, conflict.prompt, "{:?}", conflict.kind);
                assert_eq!(r.asking_seat.as_deref(), Some(conflict.seat.as_str()));
                checked[1] += 1;
            }
            TurnRedoStop::TailExhausted => {
                assert_eq!(
                    asker.map(|(p, _)| p.to_string()),
                    r.asking_seat,
                    "the exhausted tail asks the next seat live"
                );
                checked[2] += 1;
            }
        }
    }
    assert!(
        checked.iter().all(|n| *n > 0),
        "all three stops occur in the grid: {checked:?}"
    );
}

impl Scenario {
    fn result_view(&self) -> &AutoplayResult {
        &self.redo.result
    }
}

#[test]
fn two_turns_back_rewinds_to_the_second_last_turn_and_hands_back_at_the_next_decision() {
    let (_, _, all) = scenarios();
    let mut seen = 0;
    for two in all.iter().filter(|s| s.turns == 2) {
        let one = all
            .iter()
            .find(|s| s.turns == 1 && s.seat == two.seat && s.cut == two.cut && s.seed == two.seed)
            .expect("the matching one-turn redo");
        assert_eq!(two.redo.window.turns, 2);
        assert!(
            two.redo.window.start < one.redo.window.start,
            "{} cut {}",
            two.seat,
            two.cut
        );
        assert!(two.redo.window.end <= one.redo.window.start);
        let spans = find_turns(&two.hh);
        let mine: Vec<_> = spans.iter().filter(|t| t.seat == two.seat).collect();
        assert_eq!(two.redo.window.start, mine[mine.len() - 2].start);
        if let TurnRedoStop::Handoff { seat } = &two.redo.result.stop {
            assert_eq!(seat, &two.seat.to_string());
            seen += 1;
        }
    }
    assert!(seen > 0);
    let (state, galaxy) = setup(7);
    let h = play(&state, &galaxy, Box::new(Lcg(1, true)), 60);
    assert_eq!(
        redo_window(&h, &PlayerId::new("p1"), 3),
        Err(TurnRedoError::TooManyTurns)
    );
    assert!(matches!(
        redo_window(&h[..3], &PlayerId::new("p1"), 1),
        Err(TurnRedoError::NoTurn(_))
    ));
}

#[test]
fn the_same_redo_twice_gives_identical_results() {
    let (state, galaxy, all) = scenarios();
    for s in all.iter().step_by(7) {
        let r = &s.redo;
        let again = autoplay(
            state,
            Some(galaxy),
            &r.current,
            &RngMarks::new(),
            r.window.start,
            &s.seat,
            &r.source,
        )
        .unwrap();
        assert_eq!(again.decisions, r.result.decisions);
        assert_eq!(again.marks, r.result.marks);
        assert_eq!(
            serde_json::to_string(&again.stop).unwrap(),
            serde_json::to_string(&r.result.stop).unwrap()
        );
    }
}

#[test]
fn a_new_turn_that_is_not_finished_is_refused() {
    let (state, galaxy, all) = scenarios();
    let s = &all[0];
    let r = &s.redo;
    let unfinished = &r.current[..s.redo.window.start + 1];
    let err = autoplay(
        state,
        Some(galaxy),
        unfinished,
        &RngMarks::new(),
        r.window.start,
        &s.seat,
        &r.source,
    )
    .unwrap_err();
    assert_eq!(err, TurnRedoError::NewTurnNotComplete);
    let err = autoplay(
        state,
        Some(galaxy),
        &r.current[..r.window.start],
        &RngMarks::new(),
        r.window.start,
        &s.seat,
        &r.source,
    )
    .unwrap_err();
    assert_eq!(err, TurnRedoError::NewTurnMissing);
}

// ---- decks ---------------------------------------------------------------------------------

/// A scenario whose auto-play ran to a hand-off with kept decisions and no deck offset.
fn clean_scenario() -> &'static Scenario {
    let (_, _, all) = scenarios();
    all.iter()
        .find(|s| {
            s.redo.result.kept >= 3
                && s.redo.result.deck_offsets.is_empty()
                && matches!(s.redo.result.stop, TurnRedoStop::Handoff { .. })
        })
        .expect("a clean redo with a hand-off exists")
}

#[test]
fn a_draw_count_that_differs_from_the_original_is_reported_as_a_deck_cursor_conflict() {
    let (state, galaxy, _) = scenarios();
    let s = clean_scenario();
    let r = &s.redo;
    // Pretend the original timeline drew one more action card by the time of its third kept
    // decision, as the redone turn did not: the deck is then drawn from at different positions.
    let mut source = r.source.clone();
    for (rel, decks) in &mut source.decks {
        if *rel >= 3 {
            *decks.entry("action_card".to_owned()).or_insert(0) += 1;
        }
    }
    let result = autoplay(
        state,
        Some(galaxy),
        &r.current,
        &RngMarks::new(),
        r.window.start,
        &s.seat,
        &source,
    )
    .unwrap();
    let TurnRedoStop::Conflict { conflict } = &result.stop else {
        panic!("expected a deck conflict, got {:?}", result.stop);
    };
    assert_eq!(conflict.kind, ConflictKind::DeckCursor);
    assert!(conflict.deck_deltas.iter().any(|d| d.deck == "action_card"));
    assert!(
        result.kept < r.result.kept,
        "the auto-play stops before the shifted draw"
    );
    assert_eq!(result.decisions.len(), result.prefix_len + result.kept);
    // The seat that is asked live is the seat of the decision at the cut.
    let asked = next_asker(state, galaxy, &result.decisions, &result.marks)
        .unwrap()
        .0;
    assert_eq!(asked.to_string(), conflict.seat);
}

#[test]
fn a_constant_deck_offset_is_reported_but_does_not_stop_the_auto_play() {
    let (state, galaxy, _) = scenarios();
    let s = clean_scenario();
    let r = &s.redo;
    let mut source = r.source.clone();
    for (_, decks) in &mut source.decks {
        *decks.entry("action_card".to_owned()).or_insert(0) += 1;
    }
    let result = autoplay(
        state,
        Some(galaxy),
        &r.current,
        &RngMarks::new(),
        r.window.start,
        &s.seat,
        &source,
    )
    .unwrap();
    assert_eq!(
        result.kept, r.result.kept,
        "nothing drew from the shifted deck"
    );
    assert_eq!(
        result.deck_offsets,
        vec![DeckDelta {
            deck: "action_card".to_owned(),
            delta: -1
        }]
    );
}

// ---- dice ----------------------------------------------------------------------------------

/// Map 7, pseudo-random player 4: p3's turn 840..850 rolled dice at 849, and a roll at 853 follows
/// before p3's next decision at 855.
fn dice_history() -> &'static (GameState, Galaxy, Vec<DecisionRecord>) {
    use std::sync::OnceLock;
    static CELL: OnceLock<(GameState, Galaxy, Vec<DecisionRecord>)> = OnceLock::new();
    CELL.get_or_init(|| {
        let (state, galaxy) = setup(7);
        let h = play(&state, &galaxy, Box::new(Lcg(4, true)), 855);
        (state, galaxy, h)
    })
}

#[test]
fn forced_dice_reproduce_the_others_rolls_when_the_new_turn_rolls_fewer_dice_and_chain_back() {
    let (state, galaxy, h) = dice_history();
    let seat = PlayerId::new("p3");
    let window = redo_window(h, &seat, 1).unwrap();
    assert_eq!((window.start, window.end), (840, 850), "the fixture turn");
    let base = TailBaseline::new(state, Some(galaxy), h, &RngMarks::new(), &[]);
    assert_eq!(base.replayed(), h.len());
    let orig_total = replay_rolls(state, galaxy, h, &RngMarks::new(), 0);
    let orig_tail = replay_rolls(state, galaxy, h, &RngMarks::new(), window.end);
    let old_turn_dice = orig_total.len() - orig_tail.len();
    assert!(
        old_turn_dice > 0 && !orig_tail.is_empty(),
        "the old turn and the tail both roll"
    );

    // A quiet new turn: no combat, so it draws none of the old turn's dice.
    let quiet = redo(
        state,
        galaxy,
        &base,
        h,
        &RngMarks::new(),
        &seat,
        1,
        Box::new(Lcg(9, false)),
    )
    .expect("redo");
    let result = &quiet.result;
    assert!(result.kept >= 1, "stop {:?}", result.stop);
    let new_prefix_dice = replay_rolls(
        state,
        galaxy,
        &result.decisions[..result.prefix_len],
        &RngMarks::new(),
        0,
    )
    .len();
    assert!(
        new_prefix_dice < old_turn_dice,
        "the new turn rolled fewer dice ({new_prefix_dice} < {old_turn_dice})"
    );

    let forced = replay_rolls(
        state,
        galaxy,
        &result.decisions,
        &result.marks,
        result.prefix_len,
    );
    assert!(!forced.is_empty(), "the kept tail rolls dice");
    assert!(orig_tail.len() >= forced.len());
    assert_eq!(
        forced,
        orig_tail[..forced.len()],
        "the others' dice are exactly the original ones"
    );
    // Without the forcing the same decisions meet different dice.
    let plain = replay_rolls(
        state,
        galaxy,
        &result.decisions,
        &RngMarks::new(),
        result.prefix_len,
    );
    assert_ne!(plain, forced, "a plain replay shifts the dice");
    // The marks make the history recoverable: it replays to the same state a second time.
    let a = final_state_json(state, galaxy, &result.decisions, &result.marks);
    let b = final_state_json(state, galaxy, &result.decisions, &result.marks);
    assert_eq!(a, b);
    // A live session (the recovery path) given the decisions and their marks reaches that state.
    let session_state = |marks: &RngMarks| {
        let players = vec![
            PlayerId::new("p1"),
            PlayerId::new("p2"),
            PlayerId::new("p3"),
        ];
        let tiles = ti4_server::map::build_board_tiles(ContentStore::embedded(), galaxy);
        let mut config = SessionConfig::new("redo_dice_worker", state.clone())
            .with_seed(7)
            .with_player_ids(players.clone())
            .with_galaxy(galaxy.clone(), tiles)
            .with_prior_history(result.decisions.clone(), Vec::new());
        for p in players {
            config = config.with_seat(p, SeatController::Human);
        }
        config.rng_marks = marks.clone();
        let session = GameSession::start(config);
        let replayed = session.wait_replayed();
        let state = serde_json::to_string(&session.current_state()).unwrap();
        session.stop();
        (replayed, state)
    };
    let (ok, with_marks) = session_state(&result.marks);
    ok.expect("the worker replays a redone history with its marks");
    assert_eq!(
        with_marks, a,
        "the worker reaches the same state as the direct replay"
    );

    // Chained: redo the quiet turn again with the original, dice-rolling turn (more dice than
    // the quiet turn consumed). The baseline now carries the first redo's marks.
    let history2 = result.decisions.clone();
    let marks2 = result.marks.clone();
    let tail2_orig = replay_rolls(state, galaxy, &history2, &marks2, result.prefix_len);
    let base2 = TailBaseline::new(state, Some(galaxy), &history2, &marks2, &[]);
    assert_eq!(
        base2.replayed(),
        history2.len(),
        "a history with marks replays strictly"
    );
    let window2 = redo_window(&history2, &seat, 1).unwrap();
    assert_eq!(window2.start, 840);
    let loud = redo(
        state,
        galaxy,
        &base2,
        &history2,
        &marks2,
        &seat,
        1,
        Box::new(Ids(ids(&h[840..850]), 0)),
    )
    .expect("second redo");
    let r2 = &loud.result;
    assert!(r2.kept >= 1, "stop {:?}", r2.stop);
    let forced2 = replay_rolls(state, galaxy, &r2.decisions, &r2.marks, r2.prefix_len);
    assert!(!forced2.is_empty());
    assert_eq!(
        forced2,
        tail2_orig[..forced2.len()],
        "still the same dice after a second redo"
    );
    assert_eq!(
        forced2,
        orig_tail[..forced2.len()],
        "and the very first ones"
    );
}
