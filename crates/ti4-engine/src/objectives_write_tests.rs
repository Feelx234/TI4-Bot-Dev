//! Package A4, the mutation half: the objective writes and questions run on the core under
//! `objectives=dual` and agree with the string side (`objectives_write`, `rules::objectives`,
//! `rules::objective_windows`). A bail is silent, so each test probes the core function itself and requires
//! it to answer.

use ti4_content::ContentStore;
use ti4_model::content_types::POK;
use ti4_model::id::{ObjectiveId, PlanetId, PlayerId, SecretObjectiveId, SystemId};
use ti4_model::state::GameState;

use crate::choice::{Capturing, Scripted, Table};
use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};
use crate::game::Game;

/// The scoring question as a test builds it (the delivery inventory scans separate files for `Choice::new`).
type Question = crate::choice::Choice;

fn a() -> PlayerId {
    PlayerId::new("a")
}

fn clean(seats: &[(&str, &str)]) -> GameState {
    let mut state = crate::fixtures::seated_game(seats, POK);
    state.promissory_notes.clear();
    state.promissory_faceup.clear();
    state
}

fn mask() -> AreaMask {
    AreaMask::STRING.with(Area::Objectives, Backend::Dual)
}

fn table(answers: &[&str]) -> Table {
    let (decider, _seen) = Capturing::new(Box::new(Scripted::new(answers.iter().map(|s| (*s).to_owned()))));
    Table::with_default(Box::new(decider))
}

/// Ids of public objectives worth `points`, in the order of the generated table, that are not bought.
fn publics(points: u8, count: usize) -> Vec<String> {
    use ti4_engine_core::generated::PublicObjective;
    use ti4_engine_core::query::objectives as q;
    PublicObjective::ALL
        .iter()
        .filter(|o| q::points_of(**o) == points && q::cost_of(**o).is_none())
        .take(count)
        .map(|o| crate::generated::IdStr::as_str(*o).to_owned())
        .collect()
}

fn give(state: &mut GameState, planet: &str, player: &str) {
    let catalogue = ti4_content::galaxy::all_planets(ContentStore::embedded(), POK);
    let system = catalogue.get(planet).and_then(ti4_content::galaxy::Planet::system_id).unwrap_or("18");
    state.system_mut(&SystemId::new(system)).set_control(PlanetId::new(planet), PlayerId::new(player));
}

fn non_home_planets(count: usize) -> Vec<String> {
    ti4_content::galaxy::all_planets(ContentStore::embedded(), POK)
        .iter()
        .filter(|(_, planet)| planet.homeworld_of().is_none() && !planet.is_placed_during_play())
        .map(|(id, _)| (*id).to_owned())
        .take(count)
        .collect()
}

/// Four pds on a planet: the secret `eap` is satisfied.
fn four_pds(state: &mut GameState) {
    let (system, planet) = crate::fixtures::a_placed_planet();
    for _ in 0..4 {
        state
            .system_mut(&system)
            .planet_units
            .entry(planet.clone())
            .or_default()
            .push(ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new("pds"), a()));
    }
}

#[test]
fn victory_points_move_in_lockstep_with_the_core() {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    with_mask(mask(), || {
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        assert_eq!(crate::objectives::adjust_victory_points(&mut state, &a(), 3, "objective"), 3);
        // A gain stops at the target, the row carries the change that happened.
        assert_eq!(crate::objectives::adjust_victory_points(&mut state, &a(), 20, "custodians"), 7);
        assert_eq!(crate::objectives::adjust_victory_points(&mut state, &a(), 1, "imperial_primary"), 0);
        // A loss stops at zero.
        assert_eq!(crate::objectives::adjust_victory_points(&mut state, &a(), -30, "styx"), -10);
        assert_eq!(crate::objectives::adjust_victory_points(&mut state, &a(), 2, "agenda:minister_of_policy"), 2);
        assert_eq!(crate::objectives::adjust_victory_points(&mut state, &PlayerId::new("nobody"), 1, "objective"), 0);
        assert_eq!(state.vp_ledger.len(), 5);
        // A reason the ledger does not type stays on the string write (bail), with the same result.
        assert_eq!(crate::objectives::adjust_victory_points(&mut state, &a(), 1, "a made up reason"), 1);
        // The core half answers for a typed reason.
        let mut probe = clean(&[("a", "letnev"), ("b", "sol")]);
        assert_eq!(crate::objectives_write::adjust(&mut probe, &a(), 1, "objective"), Some(1));
        assert_eq!(crate::objectives_write::adjust(&mut probe, &a(), 1, "a made up reason"), None);
    });
    let r = report();
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    let _ = take_report();
}

#[test]
fn objectives_are_revealed_in_lockstep_with_the_core() {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    with_mask(mask(), || {
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        let one = publics(1, 3);
        let two = publics(2, 2);
        // Stage I on top: the second reveal of a stage II skips them.
        state.objective_deck = vec![&one[0], &one[1], &two[0], &one[2], &two[1]].into_iter().map(|s| ObjectiveId::new(s.as_str())).collect();
        state.revealed_objectives.clear();
        assert_eq!(crate::objectives::reveal_top(&mut state), Some(ObjectiveId::new(one[0].as_str())));
        let content = ContentStore::embedded();
        assert_eq!(crate::objectives::reveal_stage(&mut state, content, 2), Some(ObjectiveId::new(two[0].as_str())));
        assert_eq!(crate::objectives::reveal_stage(&mut state, content, 2), Some(ObjectiveId::new(two[1].as_str())));
        assert_eq!(crate::objectives::reveal_stage(&mut state, content, 2), None);
        assert_eq!(crate::objectives::reveal_top(&mut state), Some(ObjectiveId::new(one[1].as_str())));
        assert_eq!(state.objective_deck, vec![ObjectiveId::new(one[2].as_str())]);
        assert_eq!(state.revealed_objectives.len(), 4);
        // An empty deck reveals nothing.
        state.objective_deck.clear();
        assert_eq!(crate::objectives::reveal_top(&mut state), None);
        // The core half answers on a plain position, and bails on a deck reserve (a turn redo).
        let mut probe = clean(&[("a", "letnev"), ("b", "sol")]);
        probe.objective_deck = vec![ObjectiveId::new(one[0].as_str())];
        assert_eq!(crate::objectives_write::reveal_top(&mut probe), Some(Some(ObjectiveId::new(one[0].as_str()))));
        assert_eq!(crate::objectives_write::reveal_stage(&mut probe, 1), Some(None));
    });
    let r = report();
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    let _ = take_report();
}

#[test]
fn a_secret_drawn_over_the_hand_limit_is_returned_in_lockstep_with_the_core() {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    with_mask(mask(), || {
        let content = ContentStore::embedded();
        let secret = |s: &str| SecretObjectiveId::new(s);
        // Three in hand and one drawn: the fourth is asked about.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.player_mut(&a()).unwrap().secret_objectives = vec![secret("eap"), secret("fwm"), secret("gamf")];
        state.secret_deck = vec![secret("ctr"), secret("faa")];
        let mut asked = table(&["fwm"]);
        assert_eq!(crate::secrets::draw(&mut state, content, &mut asked, &a()).unwrap(), Some(secret("ctr")));
        assert_eq!(state.player(&a()).unwrap().secret_objectives, vec![secret("eap"), secret("gamf"), secret("ctr")]);
        assert_eq!(state.secret_deck, vec![secret("faa"), secret("fwm")], "the returned secret goes to the bottom");

        // Scored secrets count towards the limit: with two scored, one in hand and one drawn, one is asked.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.player_mut(&a()).unwrap().secret_objectives = vec![secret("eap")];
        state.record_score(&a(), ObjectiveId::new("fwm"));
        state.record_score(&a(), ObjectiveId::new("gamf"));
        state.secret_deck = vec![secret("ctr")];
        let mut asked = table(&["eap"]);
        crate::secrets::draw(&mut state, content, &mut asked, &a()).unwrap();
        assert_eq!(state.player(&a()).unwrap().secret_objectives, vec![secret("ctr")]);

        // A lone unscored secret over the limit goes back without a question.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.player_mut(&a()).unwrap().secret_objectives.clear();
        for scored in ["fwm", "gamf", "faa"] {
            state.record_score(&a(), ObjectiveId::new(scored));
        }
        state.secret_deck = vec![secret("ctr")];
        let mut asked = table(&[]);
        crate::secrets::draw(&mut state, content, &mut asked, &a()).unwrap();
        assert!(state.player(&a()).unwrap().secret_objectives.is_empty());
        assert_eq!(state.secret_deck, vec![secret("ctr")]);

        // Every secret scored: a further draw is simply stuck over the limit? (nothing to return, no question)
        // An empty deck draws nothing.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.secret_deck.clear();
        let mut asked = table(&[]);
        assert_eq!(crate::secrets::draw(&mut state, content, &mut asked, &a()).unwrap(), None);

        // The Obsidian lifts the limit by one.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.player_mut(&a()).unwrap().secret_objectives = vec![secret("eap"), secret("fwm"), secret("gamf")];
        state.player_mut(&a()).unwrap().relics = vec![ti4_model::id::RelicId::new("obsidian")];
        state.secret_deck = vec![secret("ctr")];
        let mut asked = table(&[]);
        crate::secrets::draw(&mut state, content, &mut asked, &a()).unwrap();
        assert_eq!(state.player(&a()).unwrap().secret_objectives.len(), 4);

        // An answer that is no option leaves the question unanswered: both sides stop at the same place.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.player_mut(&a()).unwrap().secret_objectives = vec![secret("eap"), secret("fwm"), secret("gamf")];
        state.secret_deck = vec![secret("ctr")];
        let mut asked = table(&["not an option"]);
        assert!(crate::secrets::draw(&mut state, content, &mut asked, &a()).is_err());
    });
    let r = report();
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    // The report is shared with parallel tests, so the core halves are probed themselves.
    let mut probe = clean(&[("a", "letnev"), ("b", "sol")]);
    probe.player_mut(&a()).unwrap().secret_objectives = ["eap", "fwm", "gamf", "ctr"].map(SecretObjectiveId::new).to_vec();
    let window = crate::objectives_write::hand_limit(&mut probe, &a(), &crate::dual_window::Recording::default()).expect("the core window answers");
    assert_eq!(window.questions.len(), 1, "the question is put");
    assert!(crate::objectives_write::draw_secret(&mut probe, &a()).is_some());
    let _ = take_report();
}

/// The core half of the status window's next question for `state`, answered with `answer` as the string side
/// did, and the position it leaves. The question, the arguments and the book are the string window's own.
/// `None` when the string window asks nothing, or the core bailed (silent in the lockstep).
fn status_probe(state: &GameState, answer: &str) -> Option<(GameState, crate::dual::CoreWindow<crate::objectives_write::ScoreValue>)> {
    status_probe_with(state, answer, &crate::dual_window::Recording::default())
}

fn status_probe_with(
    state: &GameState,
    answer: &str,
    base: &crate::dual_window::Recording,
) -> Option<(GameState, crate::dual::CoreWindow<crate::objectives_write::ScoreValue>)> {
    let content = ContentStore::embedded();
    let window = crate::objectives::ScoringWindow::new(&state.initiative_order());
    let ask = window.pending_choice(state, content, POK)?;
    let mut script = base.clone();
    script.questions.push(crate::dual_window::Asked { choice: ask.clone(), answer: Some(answer.to_owned()), nested: false });
    let mut after = state.clone();
    let core = crate::objectives_write::score_one(&mut after, content, POK, None, &ask, &window.core_args(), &script)?;
    Some((after, core))
}

/// What the core window scored (`finished`, the objective), for the paths a test follows to their answer.
fn probe_score(state: &GameState, answer: &str) -> Option<(bool, Option<ObjectiveId>)> {
    status_probe(state, answer).map(|(_, w)| (w.run.value.0, w.run.value.1))
}

/// A game in the status phase, for `player`'s score.
fn status_game(state: GameState, answers: &[&str]) -> Game<'static> {
    let mut state = state;
    state.phase = ti4_model::state::Phase::Status;
    Game::with_table(state, ContentStore::embedded(), table(answers))
}

fn run_status(game: &mut Game<'_>) {
    let mut guard = 0;
    while game.state.phase == ti4_model::state::Phase::Status && guard < 30 {
        assert_eq!(game.step().error, None);
        guard += 1;
    }
}

#[test]
fn the_status_scoring_window_runs_in_lockstep_with_the_core() {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    with_mask(mask(), || {
        // A public (six non-home planets) and a secret (four pds): both asked, both scored, in one window.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.revealed_objectives = vec![ObjectiveId::new("expand_borders")];
        for planet in non_home_planets(6) {
            give(&mut state, &planet, "a");
        }
        state.player_mut(&a()).unwrap().secret_objectives = vec![SecretObjectiveId::new("eap")];
        four_pds(&mut state);
        let mut game = status_game(state, &["expand_borders", "eap"]);
        run_status(&mut game);
        let seat = game.state.player(&a()).unwrap();
        assert_eq!(seat.victory_points, 2);
        assert!(seat.secret_objectives.is_empty());
        assert_eq!(game.state.scored_by(&a()).len(), 2);
        assert_eq!(game.state.vp_ledger.iter().filter(|(p, _, _)| *p == a()).count(), 2);

        // Declining scores nothing.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.revealed_objectives = vec![ObjectiveId::new("expand_borders")];
        for planet in non_home_planets(6) {
            give(&mut state, &planet, "a");
        }
        let mut game = status_game(state, &["decline"]);
        run_status(&mut game);
        assert_eq!(game.state.player(&a()).unwrap().victory_points, 0);

        // A secret made public (Classified Document Leaks) is scored as a public objective: it stays where it is.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.revealed_objectives = vec![ObjectiveId::new("eap")];
        four_pds(&mut state);
        let mut game = status_game(state, &["eap"]);
        run_status(&mut game);
        assert_eq!(game.state.player(&a()).unwrap().victory_points, 1);
        assert!(game.state.revealed_objectives.contains(&ObjectiveId::new("eap")));

        // The score that reaches the target ends the game.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.revealed_objectives = vec![ObjectiveId::new("expand_borders")];
        for planet in non_home_planets(6) {
            give(&mut state, &planet, "a");
        }
        state.player_mut(&a()).unwrap().victory_points = 9;
        let mut game = status_game(state, &["expand_borders"]);
        let mut guard = 0;
        while !game.state.finished && guard < 30 {
            assert_eq!(game.step().error, None);
            guard += 1;
        }
        assert!(game.state.finished);
        assert_eq!(game.state.player(&a()).unwrap().victory_points, 10);
    });
    let r = report();
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    // Probed (a bail is silent in the lockstep): the core window follows each path to its answer.
    let mut public = clean(&[("a", "letnev"), ("b", "sol")]);
    public.revealed_objectives = vec![ObjectiveId::new("expand_borders")];
    for planet in non_home_planets(6) {
        give(&mut public, &planet, "a");
    }
    assert_eq!(probe_score(&public, "expand_borders"), Some((true, Some(ObjectiveId::new("expand_borders")))));
    assert_eq!(probe_score(&public, "decline"), Some((true, None)));
    let mut held = clean(&[("a", "letnev"), ("b", "sol")]);
    held.player_mut(&a()).unwrap().secret_objectives = vec![SecretObjectiveId::new("eap")];
    four_pds(&mut held);
    assert_eq!(probe_score(&held, "eap"), Some((true, Some(ObjectiveId::new("eap")))));
    let mut leaked = clean(&[("a", "letnev"), ("b", "sol")]);
    leaked.revealed_objectives = vec![ObjectiveId::new("eap")];
    four_pds(&mut leaked);
    assert_eq!(probe_score(&leaked, "eap"), Some((true, Some(ObjectiveId::new("eap")))));
    // The core works out who is asked and what they may score itself (A4b-2): a position where the string
    // window asks nothing has no question to follow.
    let empty = clean(&[("a", "letnev"), ("b", "sol")]);
    assert_eq!(probe_score(&empty, "eap"), None);
    // The core window puts the question the string put.
    let (_, window) = status_probe(&held, "decline").expect("the core window answers");
    assert_eq!(window.questions.len(), 1);
    assert_eq!(window.questions[0].prompt, "score an objective");
    let _ = take_report();
}

/// The documented bails of `objectives_write` (docs/ENGINE_KNOWN_ISSUES.md, A4): each is a position the core
/// declines, so the string write stands. Bails are silent in the lockstep, so each is probed here.
#[test]
fn the_documented_bails_stay_with_the_string_side() {
    use crate::objectives_write::{adjust, draw_secret, reveal_stage, reveal_top, score_public, score_secret};
    let content = ContentStore::embedded();

    // A turn redo draws through a deck reserve: the draws are the string's.
    let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
    state.objective_deck = vec![ObjectiveId::new(publics(1, 1)[0].as_str())];
    state.deck_reserve = Some(ti4_model::deck_reserve::DeckReserve::new(std::sync::Arc::default(), Vec::new()));
    assert!(reveal_top(&mut state).is_none());
    assert!(reveal_stage(&mut state, 1).is_none());
    assert!(draw_secret(&mut state, &a()).is_none());
    state.deck_reserve = None;
    assert!(reveal_top(&mut state).is_some());

    // A player who has no seat, and a position the Players codec refuses (the fixture faction "generic").
    assert!(adjust(&mut state, &PlayerId::new("nobody"), 1, "objective").is_none());
    assert!(draw_secret(&mut state, &PlayerId::new("nobody")).is_none());
    let mut generic = crate::fixtures::game(&["a", "b"]);
    assert!(adjust(&mut generic, &a(), 1, "objective").is_none());
    assert!(score_public(&mut generic, &a(), &ObjectiveId::new("expand_borders"), 1).is_none());

    // An id the tables do not know, and points the content and the tables disagree on.
    assert!(score_public(&mut state, &a(), &ObjectiveId::new(["not", "an", "objective"].join("_")), 1).is_none());
    assert!(score_public(&mut state, &a(), &ObjectiveId::new("expand_borders"), 5).is_none());
    assert!(score_secret(&mut state, &a(), &SecretObjectiveId::new(["not", "a", "secret"].join("_")), 1).is_none());
    assert!(score_secret(&mut state, &a(), &SecretObjectiveId::new("eap"), 5).is_none());
    // The core's own refusal: a secret that is not in the hand is not scored (the inner `None`).
    state.player_mut(&a()).unwrap().secret_objectives.clear();
    assert_eq!(score_secret(&mut state, &a(), &SecretObjectiveId::new("eap"), 1), Some(None));

}

// -- the price of a bought objective and of the priced secrets (package A4b-1) ------------------------------

mod price {
    use super::*;
    use crate::core_sync::Footprint;
    use crate::dual_window::lend;
    use crate::objectives::Cost;
    use crate::production::Spend;
    use ti4_engine_core::kernel::fx_pay::{self, PayEnv};
    use ti4_engine_core::query::economy::Spend as CoreSpend;
    use ti4_engine_core::query::objectives as q;
    use ti4_engine_core::state::Slice;

    /// What a price touches: the hand and the pools (Players), the planets (Board), the hooks' marks.
    const PRICE: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Marks, Slice::Agenda],
        &[Slice::Players, Slice::Board],
    );

    /// The scoring window's slices too.
    const PRICE_AND_SCORE: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Marks, Slice::Agenda, Slice::Objectives, Slice::Ledger, Slice::Pending],
        &[Slice::Players, Slice::Board, Slice::Objectives, Slice::Ledger],
    );

    fn core_cost(cost: Cost) -> q::Cost {
        match cost {
            Cost::Spend { amount, kind } => q::Cost::Spend {
                amount,
                kind: match kind {
                    Spend::Resources => CoreSpend::Resources,
                    Spend::Influence => CoreSpend::Influence,
                },
            },
            Cost::TradeGoods(n) => q::Cost::TradeGoods(n),
            Cost::Tokens(n) => q::Cost::Tokens(n),
            Cost::AllThree(n) => q::Cost::AllThree(i64::from(n)),
        }
    }

    /// Every bought objective's price, from the string table.
    fn costs() -> Vec<(String, Cost)> {
        use ti4_engine_core::generated::PublicObjective;
        PublicObjective::ALL
            .iter()
            .filter_map(|o| {
                let id = crate::generated::IdStr::as_str(*o);
                crate::objectives::cost_of(&ObjectiveId::new(id)).map(|c| (id.to_owned(), c))
            })
            .collect()
    }

    /// The core payment of `cost` on `gs` (the position written back), `None` where the core bailed.
    fn core_pay(gs: &mut GameState, cost: Cost) -> Option<bool> {
        let env = PayEnv { scope: crate::core_adapter::scope(POK), blanked: crate::laws::blanked(gs) };
        let cost = core_cost(cost);
        lend(gs, PRICE, Some(&a()), |core, who| {
            fx_pay::pay_cost(core, crate::production_view::kernel(), env, who.expect("seated"), cost)
        })
    }

    struct Scene {
        faction: &'static str,
        planets: usize,
        goods: i32,
        tokens: (i32, i32, i32),
    }

    fn scene(s: &Scene) -> GameState {
        let mut state = clean(&[("a", s.faction), ("b", "sol")]);
        for planet in non_home_planets(s.planets) {
            give(&mut state, &planet, "a");
        }
        let seat = state.player_mut(&a()).unwrap();
        seat.trade_goods = s.goods;
        (seat.strategic_tokens, seat.tactic_tokens, seat.fleet_tokens) = s.tokens;
        if s.faction == "mentak" {
            seat.technologies.insert(ti4_model::id::TechnologyId::new("mc"));
        }
        if s.faction.starts_with("keleres") {
            // The agent was used for this payment: the commodities pay as trade goods, first.
            seat.commodities = 3;
            state.faction_marks.insert("keleres|agent_goods|a".to_owned(), "b".to_owned());
        }
        state
    }

    fn json(state: &GameState) -> serde_json::Value {
        serde_json::to_value(state).expect("a state serialises")
    }

    /// `fx_pay` against `objectives::pay_for` (`payment::plans` + `payment::apply`) on every price, for plain,
    /// Jol-Nar, Mentak (a trade good pays two) and Keleres (commodities pay first) seats.
    #[test]
    fn the_core_payment_equals_the_string_pay_for_on_every_price() {
        let content = ContentStore::embedded();
        let mut compared = 0;
        let mut paid = 0;
        for faction in ["letnev", "jolnar", "mentak", "keleresm"] {
            for planets in [0, 2, 5, 8] {
                for goods in [0, 2, 5, 11] {
                    for tokens in [(0, 0, 0), (2, 1, 3), (5, 5, 0), (1, 0, 9)] {
                        let base = scene(&Scene { faction, planets, goods, tokens });
                        for (id, cost) in costs() {
                            let mut string = base.clone();
                            let mut typed = base.clone();
                            let want = crate::objectives::pay_for(&mut string, content, POK, &a(), cost);
                            let got = core_pay(&mut typed, cost).unwrap_or_else(|| panic!("the core bailed on {id}"));
                            let label = format!("{id} for {faction} with {planets} planets, {goods} goods, tokens {tokens:?}");
                            assert_eq!(got, want, "paid or not: {label}");
                            assert_eq!(json(&typed), json(&string), "the position after: {label}");
                            compared += 1;
                            paid += usize::from(want);
                        }
                    }
                }
            }
        }
        assert!(compared > 2000, "{compared} prices compared");
        assert!(paid > 200 && paid < compared - 200, "both outcomes are covered: {paid} of {compared} paid");
    }

    /// The core window of the status question answered with `answer`, and the position it leaves.
    fn probe_after(state: &GameState, _id: &str, answer: &str) -> Option<(GameState, (bool, Option<ObjectiveId>))> {
        let (after, w) = status_probe(state, answer)?;
        Some((after, (w.run.value.0, w.run.value.1)))
    }

    /// The first plan is the cheapest in goods and exhausts planets: a price that spends planets is covered.
    #[test]
    fn a_resource_price_exhausts_planets_and_spends_commodities_before_goods() {
        let mut state = scene(&Scene { faction: "keleresm", planets: 1, goods: 4, tokens: (0, 0, 0) });
        state.revealed_objectives = vec![ObjectiveId::new("monument")];
        assert_eq!(crate::objectives::cost_of(&ObjectiveId::new("monument")), Some(Cost::Spend { amount: 8, kind: Spend::Resources }));
        let (after, value) = probe_after(&state, "monument", "monument").expect("the core pays the price");
        assert_eq!(value, (true, Some(ObjectiveId::new("monument"))));
        let seat = after.player(&a()).unwrap();
        assert!(after.exhausted_planets.len() > state.exhausted_planets.len(), "a planet was exhausted");
        assert_eq!(seat.commodities, 0, "the commodities pay first");
        assert!(seat.trade_goods > 0 && seat.trade_goods <= 4, "the trade goods pay the rest: {} left", seat.trade_goods);
        assert_eq!(seat.victory_points, 1);
    }

    /// `secrets::pay_for` against `fx_pay::pay_secret`: fragments in map-key order, the five first cards.
    #[test]
    fn the_core_secret_payment_equals_the_string_pay_for() {
        use ti4_engine_core::generated::{ActionCard, SecretObjective};
        let hands = [0usize, 3, 4, 5, 6, 10];
        let fragments: [&[(&str, i32)]; 8] = [
            &[],
            &[("CULTURAL", 1)],
            &[("CULTURAL", 1), ("HAZARDOUS", 1)],
            &[("CULTURAL", 0), ("FRONTIER", 2), ("INDUSTRIAL", 0)],
            &[("CULTURAL", 3), ("FRONTIER", 1), ("HAZARDOUS", 1), ("INDUSTRIAL", 2)],
            &[("HAZARDOUS", 2)],
            &[("FRONTIER", 1), ("INDUSTRIAL", 1)],
            &[("CULTURAL", 0), ("HAZARDOUS", 0)],
        ];
        let cards: Vec<&str> = ActionCard::ALL.iter().take(10).map(|c| crate::generated::IdStr::as_str(*c)).collect();
        for (secret, alias) in [(SecretObjective::DestroyHereticalWorks, "dhw"), (SecretObjective::FormASpyNetwork, "fsn"), (SecretObjective::EstablishAPerimeter, "eap")] {
            for hand in hands {
                for frags in fragments {
                    let mut base = clean(&[("a", "letnev"), ("b", "sol")]);
                    let seat = base.player_mut(&a()).unwrap();
                    seat.action_cards = cards.iter().take(hand).map(|c| ti4_model::id::ActionCardId::new(*c)).collect();
                    seat.relic_fragments = frags.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect();
                    let mut string = base.clone();
                    let mut typed = base;
                    let want = crate::secrets::pay_for(&mut string, &a(), &SecretObjectiveId::new(alias));
                    let got = lend(&mut typed, PRICE, Some(&a()), |core, who| fx_pay::pay_secret(core, who.unwrap(), secret)).expect("the core answers");
                    let label = format!("{alias}: {hand} cards, fragments {frags:?}");
                    assert_eq!(got, want, "paid or not: {label}");
                    assert_eq!(json(&typed), json(&string), "the position after: {label}");
                }
            }
        }
    }

    /// A game in the status phase with `objective` revealed or held, `a` asked once, answering `answer`.
    fn scored(state: GameState, answer: &str) -> Game<'static> {
        let mut game = status_game(state, &[answer]);
        run_status(&mut game);
        game
    }

    fn rich(faction: &'static str) -> GameState {
        scene(&Scene { faction, planets: 6, goods: 12, tokens: (3, 3, 3) })
    }

    #[test]
    fn a_bought_objective_is_paid_on_the_answer_in_lockstep_with_the_core() {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let priced = costs();
        assert_eq!(priced.len(), 10);
        for faction in ["letnev", "jolnar", "mentak", "keleresm"] {
            for (id, cost) in &priced {
                let mut state = rich(faction);
                state.revealed_objectives = vec![ObjectiveId::new(id.as_str())];
                let mut game = None;
                with_mask(mask(), || game = Some(scored(state.clone(), id)));
                let game = game.unwrap();
                assert!(game.state.scored_by(&a()).contains(&ObjectiveId::new(id.as_str())), "{id} scored for {faction}");
                // The core half itself (a bail is silent in the lockstep): it follows the answer and pays.
                let (after, value) = probe_after(&state, id, id).unwrap_or_else(|| panic!("the core bailed on {id} for {faction}"));
                assert_eq!(value, (true, Some(ObjectiveId::new(id.as_str()))), "{id} for {faction}");
                let (was, now) = (state.player(&a()).unwrap(), after.player(&a()).unwrap());
                assert!(now.victory_points > was.victory_points, "{id} scored for {faction}");
                match cost {
                    Cost::TradeGoods(n) => assert_eq!(now.trade_goods, was.trade_goods - n, "{id}"),
                    Cost::Tokens(n) => assert_eq!(now.strategic_tokens + now.tactic_tokens, was.strategic_tokens + was.tactic_tokens - n, "{id}"),
                    Cost::Spend { .. } | Cost::AllThree(_) => assert!(!after.exhausted_planets.is_empty() || now.trade_goods < was.trade_goods, "{id} spent something"),
                }
                assert_eq!(now.fleet_tokens, was.fleet_tokens, "the fleet pool is never spent");
            }
        }
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    #[test]
    fn the_priced_secrets_are_paid_on_the_answer_in_lockstep_with_the_core() {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let cards: Vec<ti4_model::id::ActionCardId> =
            ["abs", "arch_expedition", "assassin", "blackmarketdealing", "blitz", "bribery", "brilliance"].map(ti4_model::id::ActionCardId::new).to_vec();
        // Destroy Heretical Works: two fragments leave, in key order, and the empty keys leave the map.
        let mut dhw = clean(&[("a", "letnev"), ("b", "sol")]);
        let seat = dhw.player_mut(&a()).unwrap();
        seat.secret_objectives = vec![SecretObjectiveId::new("dhw")];
        seat.relic_fragments = [("CULTURAL", 1), ("HAZARDOUS", 2), ("INDUSTRIAL", 0)].map(|(k, v)| (k.to_owned(), v)).into_iter().collect();
        let mut game = None;
        with_mask(mask(), || game = Some(scored(dhw.clone(), "dhw")));
        assert!(game.unwrap().state.scored_by(&a()).contains(&ObjectiveId::new("dhw")));
        let (after, value) = probe_after(&dhw, "dhw", "dhw").expect("the core pays Destroy Heretical Works");
        assert_eq!(value, (true, Some(ObjectiveId::new("dhw"))));
        let seat = after.player(&a()).unwrap();
        assert_eq!(seat.victory_points, 1);
        assert!(seat.secret_objectives.is_empty());
        assert_eq!(seat.relic_fragments, [("HAZARDOUS".to_owned(), 1)].into_iter().collect());
        // Form a Spy Network: the first five cards leave the hand (and no discard pile gets them).
        let mut fsn = clean(&[("a", "letnev"), ("b", "sol")]);
        let seat = fsn.player_mut(&a()).unwrap();
        seat.secret_objectives = vec![SecretObjectiveId::new("fsn")];
        seat.action_cards = cards.clone();
        let discard = fsn.discarded_action_cards.len();
        let mut game = None;
        with_mask(mask(), || game = Some(scored(fsn.clone(), "fsn")));
        assert!(game.unwrap().state.scored_by(&a()).contains(&ObjectiveId::new("fsn")));
        let (after, value) = probe_after(&fsn, "fsn", "fsn").expect("the core pays Form a Spy Network");
        assert_eq!(value, (true, Some(ObjectiveId::new("fsn"))));
        assert_eq!(after.player(&a()).unwrap().action_cards, cards[5..].to_vec());
        assert_eq!(after.discarded_action_cards.len(), discard, "the five cards are dropped, not discarded (copied)");
        assert_eq!(after.player(&a()).unwrap().victory_points, 1);
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// Paid on taking, never on offering (61.10): a declined offer costs nothing, and a bought objective is not
    /// free when the window has no price environment.
    #[test]
    fn a_declined_priced_offer_costs_nothing_and_an_unpriced_window_refuses_the_objective() {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mut state = rich("letnev");
        state.revealed_objectives = vec![ObjectiveId::new("amass_wealth")];
        let mut game = None;
        with_mask(mask(), || game = Some(scored(state.clone(), "decline")));
        assert!(game.unwrap().state.scored_by(&a()).is_empty());
        let (after, value) = probe_after(&state, "amass_wealth", "decline").expect("the core follows the decline");
        assert_eq!(value, (true, None));
        assert_eq!(json(&after), json(&state), "nothing was paid for a decline");
        // The window itself: no price environment, no bought objective.
        use ti4_engine_core::generated::PublicObjective;
        use ti4_engine_core::kernel::stage::AbilityTable;
        use ti4_engine_core::kernel::Resolver;
        use ti4_engine_core::state::ObjectiveRef;
        let (mut cx, r) = crate::dual_window::tables(&state).unwrap();
        let seat = r.seat("a").unwrap();
        let obj = ObjectiveRef::Public(PublicObjective::AmassWealth);
        let mut driver = ti4_engine_core::driver::Driver::new(
            ti4_engine_core::rules::objective_windows::ScoreOne::new(seat, true, &[ti4_engine_core::choice::objectives::Offer::Objective(obj)], None),
            Resolver::new(&[seat], Some(seat)),
            AbilityTable::default(),
            1,
        );
        let mut probe = state.clone();
        let outcome = crate::core_sync::with_core(&mut probe, &mut cx, crate::core_sync::Sources::default(), PRICE_AND_SCORE, |core, _| {
            driver.run(core, crate::production_view::kernel()).expect("asks");
            driver.resume(core, crate::production_view::kernel(), ti4_engine_core::choice::Pick::Opt(ti4_engine_core::choice::Opt::Objectives(ti4_engine_core::choice::objectives::Opt::Score(obj))))
        })
        .expect("loads");
        assert!(outcome.is_err(), "refused: a bought objective is never free");
        // The same window with its price environment pays and scores.
        let env = PayEnv { scope: crate::core_adapter::scope(POK), blanked: false };
        let mut driver = ti4_engine_core::driver::Driver::new(
            ti4_engine_core::rules::objective_windows::ScoreOne::new(seat, true, &[ti4_engine_core::choice::objectives::Offer::Objective(obj)], None).with_price(env),
            Resolver::new(&[seat], Some(seat)),
            AbilityTable::default(),
            1,
        );
        let mut probe = state.clone();
        let outcome = crate::core_sync::with_core(&mut probe, &mut cx, crate::core_sync::Sources::default(), PRICE_AND_SCORE, |core, _| {
            driver.run(core, crate::production_view::kernel()).expect("asks");
            driver.resume(core, crate::production_view::kernel(), ti4_engine_core::choice::Pick::Opt(ti4_engine_core::choice::Opt::Objectives(ti4_engine_core::choice::objectives::Opt::Score(obj))))
        })
        .expect("loads");
        assert!(outcome.is_ok(), "paid and scored");
        // A priced secret without the environment is refused as well (and with it, taken).
        let mut held = state.clone();
        let seat_state = held.player_mut(&a()).unwrap();
        seat_state.secret_objectives = vec![SecretObjectiveId::new("dhw")];
        seat_state.relic_fragments = [("CULTURAL".to_owned(), 2)].into_iter().collect();
        let secret = ObjectiveRef::Secret(ti4_engine_core::generated::SecretObjective::DestroyHereticalWorks);
        for (env, paid) in [(None, false), (Some(env), true)] {
            let mut window = ti4_engine_core::rules::objective_windows::ScoreOne::new(seat, true, &[ti4_engine_core::choice::objectives::Offer::Objective(secret)], None);
            if let Some(env) = env {
                window = window.with_price(env);
            }
            let mut driver = ti4_engine_core::driver::Driver::new(window, Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
            let mut probe = held.clone();
            let outcome = crate::core_sync::with_core(&mut probe, &mut cx, crate::core_sync::Sources::default(), PRICE_AND_SCORE, |core, _| {
                driver.run(core, crate::production_view::kernel()).expect("asks");
                driver.resume(core, crate::production_view::kernel(), ti4_engine_core::choice::Pick::Opt(ti4_engine_core::choice::Opt::Objectives(ti4_engine_core::choice::objectives::Opt::Score(secret))))
            })
            .expect("loads");
            assert_eq!(outcome.is_ok(), paid, "a priced secret is taken only with its price environment");
        }
        let _ = take_report();
    }

    /// The string took a price the core refuses (the facts differ): the lockstep reports it instead of bailing.
    #[test]
    fn an_answer_the_core_cannot_pay_is_reported_not_bailed() {
        // Something else is on offer, so a question exists; the bought objective is revealed but not affordable.
        let mut poor = clean(&[("a", "letnev"), ("b", "sol")]);
        poor.player_mut(&a()).unwrap().secret_objectives = vec![SecretObjectiveId::new("eap")];
        four_pds(&mut poor);
        poor.revealed_objectives = vec![ObjectiveId::new("amass_wealth")];
        let value = probe_score(&poor, "amass_wealth");
        assert_eq!(value, Some((false, None)), "a window that never finished: a difference against the string's score");
    }
}

// -- the window's bookkeeping and Plots Within Plots (package A4b-2) ----------------------------------------

mod book {
    use super::*;
    use crate::dual_window::{Asked, Recording};
    use crate::objectives::{EventScoreLimit, ScoringWindow};
    use crate::secrets::Timing;
    use ti4_model::state::Feat;

    fn json(state: &GameState) -> serde_json::Value {
        serde_json::to_value(state).expect("a state serialises")
    }

    /// What a walk saw: the steps taken and who was asked at each.
    #[derive(Debug, Default)]
    pub(super) struct Trace {
        pub asked: Vec<(String, Vec<String>)>,
    }

    /// Play `window` on `state` with the string engine, and at each question also with the core on a copy: the
    /// core must follow (a bail fails the walk), put the same question, and leave the same position, value and
    /// bookkeeping. `answers` are the option ids, in order; the walk ends when the window asks nothing.
    pub(super) fn walk(mut state: GameState, mut window: ScoringWindow, answers: &[&str]) -> (GameState, ScoringWindow, Trace) {
        let content = ContentStore::embedded();
        let mut trace = Trace::default();
        for (step, answer) in answers.iter().enumerate() {
            let ask = window
                .pending_choice(&state, content, POK)
                .unwrap_or_else(|| panic!("step {step}: the window asks nothing, wanted {answer}"));
            trace.asked.push((ask.player.to_string(), ask.ids().iter().map(|s| (*s).to_owned()).collect()));
            let mut script = Recording::default();
            script.questions.push(Asked { choice: ask.clone(), answer: Some((*answer).to_owned()), nested: false });
            let mut core_state = state.clone();
            let core = crate::objectives_write::score_one(&mut core_state, content, POK, None, &ask, &window.core_args(), &script)
                .unwrap_or_else(|| panic!("step {step}: the core bailed on {answer}"));
            assert_eq!(core.questions.len(), 1, "step {step}: one question");
            let rendered = &core.questions[0];
            assert_eq!(rendered.player, ask.player, "step {step}: who is asked");
            assert_eq!(rendered.prompt, ask.prompt, "step {step}");
            assert_eq!(rendered.ids(), ask.ids(), "step {step}: what is on offer");
            assert_eq!(rendered.context, ask.context, "step {step}: the context");
            let option = ask.option(answer).unwrap_or_else(|| panic!("step {step}: {answer} in {:?}", ask.ids())).clone();
            let scored = window.resolve(&mut state, content, POK, option).expect("the string resolves");
            assert_eq!(core.run.value, (true, scored, window.book()), "step {step}: value and book after {answer}");
            assert_eq!(json(&core_state), json(&state), "step {step}: the position after {answer}");
        }
        (state, window, trace)
    }

    /// The window has nothing left to ask.
    fn finished(state: &GameState, window: &ScoringWindow) -> bool {
        window.pending_choice(state, ContentStore::embedded(), POK).is_none()
    }

    fn two_scoring_seats() -> GameState {
        // `a` meets a public (six planets) and a secret (four pds); `b` meets the same secret.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.revealed_objectives = vec![ObjectiveId::new("expand_borders")];
        for planet in non_home_planets(6) {
            give(&mut state, &planet, "a");
        }
        state.player_mut(&a()).unwrap().secret_objectives = vec![SecretObjectiveId::new("eap")];
        four_pds(&mut state);
        state
    }

    #[test]
    fn the_status_window_asks_one_public_and_one_secret_then_the_next_player() {
        let state = two_scoring_seats();
        let window = ScoringWindow::new(&state.initiative_order());
        let (end, window, trace) = walk(state, window, &["expand_borders", "eap"]);
        assert!(finished(&end, &window), "nobody else has anything to score");
        assert_eq!(trace.asked.len(), 2);
        assert_eq!(trace.asked[0].0, "a");
        assert_eq!(trace.asked[1].0, "a", "the player stays while the other category is on offer");
        assert!(trace.asked[1].1.contains(&"eap".to_owned()) && !trace.asked[1].1.contains(&"expand_borders".to_owned()), "the public cap is spent");
        assert_eq!(end.player(&a()).unwrap().victory_points, 2);
    }

    #[test]
    fn a_decline_ends_the_players_whole_turn_in_the_status_window() {
        let state = two_scoring_seats();
        let window = ScoringWindow::new(&state.initiative_order());
        let (end, window, trace) = walk(state, window, &["decline"]);
        assert_eq!(trace.asked.len(), 1);
        assert!(finished(&end, &window), "declining leaves nothing pending for the only player with something to score");
    }

    #[test]
    fn the_next_player_is_asked_after_the_first_has_used_both_caps() {
        let mut state = two_scoring_seats();
        // `b` holds the same secret and meets it too (four pds is a position of `a`; give `b` its own).
        crate::fixtures::put(&mut state, &SystemId::new("37"), "pds", &PlayerId::new("b"), 4);
        state.player_mut(&PlayerId::new("b")).unwrap().secret_objectives = vec![SecretObjectiveId::new("eap")];
        let window = ScoringWindow::new(&state.initiative_order());
        let first = window.pending_choice(&state, ContentStore::embedded(), POK).expect("someone is asked").player;
        let answers: Vec<&str> = if first == a() { vec!["expand_borders", "eap", "eap"] } else { vec!["eap", "expand_borders", "eap"] };
        let (end, window, trace) = walk(state, window, &answers);
        assert!(finished(&end, &window));
        assert_eq!(trace.asked.len(), 3);
        let seats: Vec<&str> = trace.asked.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(seats.iter().filter(|p| **p == "b").count() + seats.iter().filter(|p| **p == "a").count(), 3);
    }

    /// An action-timed window with the per-event cap: the first score uses the occurrence; a second secret is
    /// not offered (one per player and event), while the unlimited window offers it again.
    #[test]
    fn the_event_window_caps_a_player_by_occurrence_or_not() {
        for limit in [EventScoreLimit::OnePerPlayer, EventScoreLimit::AnyPerPlayer] {
            let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
            state.player_mut(&a()).unwrap().secret_objectives = ["btv", "dtgs"].map(SecretObjectiveId::new).to_vec();
            let occurrence = state.begin_feat_occurrence();
            state.record_event_feat(&a(), Feat::WonInAnAnomaly, occurrence);
            state.record_event_feat(&a(), Feat::DestroyedACapitalShip, occurrence);
            let window = ScoringWindow::for_occurrence(&state.initiative_order(), Timing::Action, occurrence, limit);
            let answers: &[&str] = if limit == EventScoreLimit::OnePerPlayer { &["btv"] } else { &["btv", "dtgs"] };
            let (end, window, trace) = walk(state, window, answers);
            assert!(finished(&end, &window), "{limit:?}");
            assert_eq!(trace.asked.len(), answers.len(), "{limit:?}");
            assert_eq!(end.player(&a()).unwrap().victory_points, i32::try_from(answers.len()).unwrap(), "{limit:?}");
        }
    }

    // -- Plots Within Plots ----------------------------------------------------------------------------

    /// `a` (Firmament) has three space docks; `b` has scored `fwm` ("Have 3 space docks").
    fn docks_game() -> GameState {
        let mut state = clean(&[("a", crate::factions::firmament::FACTION), ("b", "sol"), ("c", "letnev")]);
        crate::fixtures::put(&mut state, &SystemId::new("18"), "spacedock", &a(), 3);
        state.record_score(&PlayerId::new("b"), ObjectiveId::new("fwm"));
        state
    }

    #[test]
    fn a_plot_is_scored_for_no_point_and_the_card_is_asked_next() {
        let state = docks_game();
        let window = ScoringWindow::new(&state.initiative_order());
        let (end, window, trace) = walk(state, window, &["plot|fwm|b", "assail"]);
        assert!(finished(&end, &window));
        assert_eq!(trace.asked.len(), 2);
        assert_eq!(trace.asked[1].1, ["enervate", "siphon", "seethe", "assail", "extract"], "the card question lists the remaining cards");
        let seat = end.player(&a()).unwrap();
        assert_eq!(seat.victory_points, 0, "a plot gives no victory point");
        assert_eq!(seat.plots, ["d:assail:b"]);
        assert!(seat.plot_objectives.contains(ti4_model::id::text(&"fwm")));
        assert!(!end.scored_by(&a()).contains(ti4_model::id::text(&"fwm")), "a plot is no scored objective");
    }

    #[test]
    fn the_last_plot_card_is_placed_at_once() {
        let mut state = docks_game();
        for (card, token) in [("enervate", "b"), ("siphon", "c"), ("seethe", "b"), ("assail", "c")] {
            let cards = ti4_model::plots::Plot::facedown(card, &PlayerId::new(token)).encode();
            state.player_mut(&PlayerId::new("c")).unwrap().plots.push(cards);
        }
        let window = ScoringWindow::new(&state.initiative_order());
        let (end, window, trace) = walk(state, window, &["plot|fwm|b"]);
        assert!(finished(&end, &window), "one card remained: placed without a question");
        assert_eq!(trace.asked.len(), 1);
        assert_eq!(end.player(&a()).unwrap().plots, ["d:extract:b"]);
    }

    #[test]
    fn a_plot_leaves_the_ordinary_caps_open_in_the_status_window() {
        for plot_first in [true, false] {
            let mut state = docks_game();
            crate::fixtures::put(&mut state, &SystemId::new("18"), "pds", &a(), 4);
            state.player_mut(&a()).unwrap().secret_objectives = vec![SecretObjectiveId::new("eap")];
            let window = ScoringWindow::new(&state.initiative_order());
            let answers: &[&str] = if plot_first { &["plot|fwm|b", "seethe", "eap"] } else { &["eap", "plot|fwm|b", "seethe"] };
            let (end, window, _) = walk(state, window, answers);
            assert!(finished(&end, &window), "plot first: {plot_first}");
            assert_eq!(end.player(&a()).unwrap().victory_points, 1, "only the ordinary secret scores");
        }
    }

    #[test]
    fn a_plot_is_scored_in_an_event_window_beside_the_capped_secret() {
        let mut state = clean(&[("a", crate::factions::firmament::FACTION), ("b", "sol")]);
        state.record_score(&PlayerId::new("b"), ObjectiveId::new("dtgs"));
        state.player_mut(&a()).unwrap().secret_objectives = vec![SecretObjectiveId::new("btv")];
        let occurrence = state.begin_feat_occurrence();
        state.record_event_feat(&a(), Feat::DestroyedACapitalShip, occurrence);
        state.record_event_feat(&a(), Feat::WonInAnAnomaly, occurrence);
        let window = ScoringWindow::for_occurrence(&[a()], Timing::Action, occurrence, EventScoreLimit::OnePerPlayer);
        let (end, window, _) = walk(state, window, &["btv", "plot|dtgs|b", "extract"]);
        assert!(finished(&end, &window));
        assert_eq!(end.player(&a()).unwrap().plots, ["d:extract:b"]);
    }

    /// The Game stepper, with the objectives area on dual: the same window through `scoring::step`.
    #[test]
    fn the_plot_window_runs_in_lockstep_through_the_game() {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let state = docks_game();
        let mut game = None;
        with_mask(mask(), || {
            let mut g = status_game(state.clone(), &["plot|fwm|b", "assail"]);
            run_status(&mut g);
            game = Some(g);
        });
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
        let game = game.unwrap();
        assert_eq!(game.state.player(&a()).unwrap().plots, ["d:assail:b"]);
    }

    /// A plot whose secret has a price (Destroy Heretical Works needs two fragments) pays it, like the string
    /// `award_plot`; without the fragments the plot is not scored.
    #[test]
    fn a_priced_plot_is_paid_with_its_secret_price() {
        let mut state = clean(&[("a", crate::factions::firmament::FACTION), ("b", "sol")]);
        state.record_score(&PlayerId::new("b"), ObjectiveId::new("dhw"));
        let seat = state.player_mut(&a()).unwrap();
        seat.relic_fragments = [("CULTURAL", 1), ("HAZARDOUS", 2)].map(|(k, v)| (k.to_owned(), v)).into_iter().collect();
        let window = ScoringWindow::new(&state.initiative_order());
        assert!(window.pending_choice(&state, ContentStore::embedded(), POK).is_some(), "the plot is on offer");
        let (end, _, _) = walk(state, window, &["plot|dhw|b", "enervate"]);
        let held: i32 = end.player(&a()).unwrap().relic_fragments.values().sum();
        assert_eq!(held, 1, "two of the three fragments were paid");
    }
}
