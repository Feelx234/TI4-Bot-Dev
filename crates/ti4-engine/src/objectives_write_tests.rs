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

/// The core window of one scoring question, answered with `answer` as the string side did: `None` when the
/// core bailed (silent in the lockstep), else what the window scored.
fn probe_score(state: &GameState, offers: &[&str], answer: &str) -> Option<(bool, Option<ObjectiveId>)> {
    let mut options: Vec<crate::choice::ChoiceOption> =
        offers.iter().map(|id| crate::choice::ChoiceOption::labelled(*id, "score", *id)).collect();
    options.push(crate::choice::ChoiceOption::decline());
    let ask = Question::new(a(), "score an objective", options);
    let mut script = crate::dual_window::Recording::default();
    script.questions.push(crate::dual_window::Asked { choice: ask.clone(), answer: Some(answer.to_owned()), nested: false });
    let args = crate::objectives_write::ScoreArgs { status: true, occurrence: None };
    let window = crate::objectives_write::score_one(&mut state.clone(), ContentStore::embedded(), POK, &ask, args, &script)?;
    Some(window.run.value)
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
    assert_eq!(probe_score(&public, &["expand_borders"], "expand_borders"), Some((true, Some(ObjectiveId::new("expand_borders")))));
    assert_eq!(probe_score(&public, &["expand_borders"], "decline"), Some((true, None)));
    let mut held = clean(&[("a", "letnev"), ("b", "sol")]);
    held.player_mut(&a()).unwrap().secret_objectives = vec![SecretObjectiveId::new("eap")];
    assert_eq!(probe_score(&held, &["eap"], "eap"), Some((true, Some(ObjectiveId::new("eap")))));
    let mut leaked = clean(&[("a", "letnev"), ("b", "sol")]);
    leaked.revealed_objectives = vec![ObjectiveId::new("eap")];
    assert_eq!(probe_score(&leaked, &["eap"], "eap"), Some((true, Some(ObjectiveId::new("eap")))));
    // A secret that is neither in the hand nor public is not scored by the core (stale window): the step bails.
    let empty = clean(&[("a", "letnev"), ("b", "sol")]);
    assert_eq!(probe_score(&empty, &["eap"], "eap"), None);
    // Probed: the core window of a scoring question answers (a question put, nothing answered yet).
    let mut probe = clean(&[("a", "letnev"), ("b", "sol")]);
    let ask = Question::new(
        a(),
        "score an objective",
        vec![crate::choice::ChoiceOption::labelled("eap", "score", "eap"), crate::choice::ChoiceOption::decline()],
    );
    let args = crate::objectives_write::ScoreArgs { status: true, occurrence: None };
    let window = crate::objectives_write::score_one(&mut probe, ContentStore::embedded(), POK, &ask, args, &crate::dual_window::Recording::default())
        .expect("the core window answers");
    assert_eq!(window.questions.len(), 1);
    assert_eq!(window.questions[0].prompt, "score an objective");
    let _ = take_report();
}

/// The documented bails of `objectives_write` (docs/ENGINE_KNOWN_ISSUES.md, A4): each is a position the core
/// declines, so the string write stands. Bails are silent in the lockstep, so each is probed here.
#[test]
fn the_documented_bails_stay_with_the_string_side() {
    use crate::choice::ChoiceOption;
    use crate::dual_window::Recording;
    use crate::objectives_write::{ScoreArgs, adjust, draw_secret, reveal_stage, reveal_top, score_one, score_public, score_secret};
    let content = ContentStore::embedded();
    let args = ScoreArgs { status: true, occurrence: None };

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

    // The scoring question: an option that is no objective (a Firmament plot), a bought objective (the price
    // is the string's), a question the string put inside a nested emission.
    let ask = |ids: &[&str]| {
        let mut options: Vec<ChoiceOption> = ids.iter().map(|id| ChoiceOption::labelled(*id, "score", *id)).collect();
        options.push(ChoiceOption::decline());
        Question::new(a(), "score an objective", options)
    };
    let script = Recording::default();
    let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
    assert!(score_one(&mut state, content, POK, &ask(&["eap"]), args, &script).is_some(), "the plain question is followed");
    assert!(score_one(&mut state, content, POK, &ask(&["eap", "eap|plot|b"]), args, &script).is_none());
    // A bought objective and the two secrets with a price (two relic fragments, five action cards) are followed
    // (the core window pays them, `the_price_*` tests below); the price stays with the string at a table with a
    // Last Bastion or a Crimson seat, and for a payer holding ocean cards.
    assert!(score_one(&mut state, content, POK, &ask(&["amass_wealth"]), args, &script).is_some());
    assert!(score_one(&mut state, content, POK, &ask(&["dhw"]), args, &script).is_some());
    assert!(score_one(&mut state, content, POK, &ask(&["fsn"]), args, &script).is_some());
    for faction in [crate::factions::bastion::FACTION, crate::factions::crimson::FACTION] {
        let mut table = clean(&[("a", "letnev"), ("b", faction)]);
        assert!(score_one(&mut table, content, POK, &ask(&["amass_wealth"]), args, &script).is_none(), "{faction} at the table");
        assert!(score_one(&mut table, content, POK, &ask(&["dhw"]), args, &script).is_none(), "{faction} at the table");
        assert!(score_one(&mut table, content, POK, &ask(&["eap"]), args, &script).is_some(), "an unpriced offer is not affected");
    }
    let mut oceans = clean(&[("a", crate::factions::deepwrought::FACTION), ("b", "sol")]);
    let (system, planet) = crate::fixtures::a_placed_planet();
    oceans.system_mut(&system).set_control(planet.clone(), PlayerId::new("b"));
    crate::fixtures::put_on_planet(&mut oceans, &system, &planet, "infantry", &PlayerId::new("b"), 1);
    crate::fixtures::put_on_planet(&mut oceans, &system, &planet, "infantry", &a(), 1);
    assert!(crate::factions::deepwrought::begin_coexisting(&mut oceans, &a(), &system, &planet, None).expect("begins"));
    assert!(!crate::factions::deepwrought::spendable_oceans(&oceans, &a()).is_empty(), "the fixture holds an ocean card");
    assert!(score_one(&mut oceans, content, POK, &ask(&["amass_wealth"]), args, &script).is_none());
    assert!(score_one(&mut oceans, content, POK, &ask(&["eap"]), args, &script).is_some());
    let mut nested = Recording::default();
    nested.nested = true;
    assert!(score_one(&mut state, content, POK, &ask(&["eap"]), args, &nested).is_none());
    state.diplomacy.enabled = true;
    assert!(score_one(&mut state, content, POK, &ask(&["eap"]), args, &script).is_none());
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

    /// The core window of one scoring question answered with `answer`, and the position it leaves.
    fn probe_after(state: &GameState, id: &str, answer: &str) -> Option<(GameState, (bool, Option<ObjectiveId>))> {
        let mut options = vec![crate::choice::ChoiceOption::labelled(id, "score", id)];
        options.push(crate::choice::ChoiceOption::decline());
        let ask = Question::new(a(), "score an objective", options);
        let mut script = crate::dual_window::Recording::default();
        script.questions.push(crate::dual_window::Asked { choice: ask.clone(), answer: Some(answer.to_owned()) });
        let args = crate::objectives_write::ScoreArgs { status: true, occurrence: None };
        let mut after = state.clone();
        let window = crate::objectives_write::score_one(&mut after, ContentStore::embedded(), POK, &ask, args, &script)?;
        Some((after, window.run.value))
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
            ti4_engine_core::rules::objective_windows::ScoreOne::new(seat, true, &[obj], None),
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
            ti4_engine_core::rules::objective_windows::ScoreOne::new(seat, true, &[obj], None).with_price(env),
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
            let mut window = ti4_engine_core::rules::objective_windows::ScoreOne::new(seat, true, &[secret], None);
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
        let poor = clean(&[("a", "letnev"), ("b", "sol")]);
        let value = probe_score(&poor, &["amass_wealth"], "amass_wealth");
        assert_eq!(value, Some((false, None)), "a window that never finished: a difference against the string's score");
    }
}
