//! Lockstep tests of the close of a vote (package B9c-2): `Game::close_vote` run under `agenda=dual`, the string
//! close and `rules::agenda::close::Close` side by side. Every test asserts that the core half ran (`compared`), not only
//! that nothing differed: a window that bails compares nothing.

use ti4_content::ContentStore;
use ti4_model::content_types::DEFAULT;
use ti4_model::id::{ActionCardId, PlayerId};
use ti4_model::state::{GameState, Phase, TokenPool};

use crate::choice::{Scripted, Table};
use crate::dual::{AreaMask, Backend, local_windows, take_report, with_mask};
use crate::game::Game;
use crate::vote::VoteWindow;

const LABEL: &str = "game::close_vote";

fn p(id: &str) -> PlayerId {
    PlayerId::new(id)
}

/// Three real factions with their promissory notes dealt, in the agenda phase.
fn position() -> GameState {
    let content = ContentStore::embedded();
    let mut state = crate::fixtures::seated_game(&[("a", "hacan"), ("b", "xxcha"), ("c", "arborec")], DEFAULT);
    crate::promissory::deal(&mut state, content, DEFAULT);
    state.phase = Phase::Agenda;
    for seat in &mut state.players {
        // Room in every pool for the tokens a rider gains.
        seat.tactic_tokens = 0;
        seat.fleet_tokens = 0;
        seat.strategic_tokens = 0;
    }
    state
}

fn game(state: GameState, script: &[&str]) -> Game<'static> {
    let table = Table::with_default(Box::new(Scripted::new(script.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>())));
    Game::with_table(state, ContentStore::embedded(), table).with_sources(DEFAULT)
}

fn lockstep<T>(body: impl FnOnce() -> T) -> (T, u64, u64, String) {
    let _guard = crate::dual::tests::serial();
    let mask = AreaMask::parse("agenda=dual", AreaMask::all(Backend::String)).unwrap();
    let _ = take_report();
    let before = local_windows(LABEL);
    let out = with_mask(mask, body);
    let report = take_report();
    let (compared_now, bailed_now) = local_windows(LABEL);
    let (compared, bailed) = (compared_now - before.0, bailed_now - before.1);
    let differences = if report.total_differences() == 0 { String::new() } else { format!("{report:?}") };
    (out, compared, bailed, differences)
}

fn ok(compared: u64, bailed: u64, diff: &str) {
    assert_eq!((compared, bailed), (1, 0), "{diff}");
    assert!(diff.is_empty(), "{diff}");
}

/// A law vote of `alias` for/against: a backed "for", b backed "against".
fn law_vote(alias: &str, winner: Option<&str>) -> VoteWindow {
    VoteWindow::finished(alias, &["for", "against"], &[("a", "for"), ("b", "against")], &[("for", 3), ("against", 1)], winner)
}

fn close(mut g: Game<'static>, window: VoteWindow) -> Game<'static> {
    g.put_vote(window);
    assert_eq!(g.close_vote().error, None);
    g
}

fn points(state: &GameState, id: &str) -> i32 {
    state.player(&p(id)).unwrap().victory_points
}

fn tokens(state: &GameState, id: &str, pool: TokenPool) -> i32 {
    state.player(&p(id)).unwrap().tokens(pool)
}

fn predict(state: &mut GameState, id: &str, text: &str) {
    state.agenda_predictions.insert(p(id), text.to_owned());
}

fn hand(state: &mut GameState, id: &str, cards: &[&str]) {
    state.player_mut(&p(id)).unwrap().action_cards = cards.iter().map(|c| ActionCardId::new(*c)).collect();
}

#[test]
fn a_law_that_passes_is_enacted_logged_and_in_lockstep() {
    let (g, compared, bailed, diff) = lockstep(|| close(game(position(), &[]), law_vote("checks", Some("for"))));
    ok(compared, bailed, &diff);
    assert_eq!(g.state.laws.get("checks").map(String::as_str), Some("for"));
    let rec = g.state.agenda_log.last().unwrap();
    assert_eq!((rec.agenda.as_str(), rec.outcome.as_deref(), rec.law, rec.enacted, rec.discarded, rec.speaker_decided), ("checks", Some("for"), true, true, false, false));
    assert_eq!(rec.votes.get("for"), Some(&3));
    assert!(g.state.agenda_votes.is_empty());
    assert!(g.events.iter().any(|e| e == "AGENDA_RESOLVED:checks:for"));
}

#[test]
fn against_and_a_directive_enact_nothing() {
    let (g, compared, bailed, diff) = lockstep(|| close(game(position(), &[]), law_vote("checks", Some("against"))));
    ok(compared, bailed, &diff);
    assert!(g.state.laws.is_empty());
    assert!(!g.state.agenda_log.last().unwrap().enacted);
    let (g, compared, bailed, diff) = lockstep(|| close(game(position(), &[]), law_vote("seed_empire", Some("for"))));
    ok(compared, bailed, &diff);
    assert!(g.state.laws.is_empty());
    assert!(!g.state.agenda_log.last().unwrap().law);
}

#[test]
fn a_vote_without_a_winner_is_logged_and_forgets_the_predictions() {
    let (g, compared, bailed, diff) = lockstep(|| {
        let mut s = position();
        predict(&mut s, "a", "for");
        close(game(s, &[]), VoteWindow::finished("checks", &[], &[], &[], None))
    });
    ok(compared, bailed, &diff);
    assert!(g.state.agenda_predictions.is_empty());
    let rec = g.state.agenda_log.last().unwrap();
    assert_eq!((rec.outcome.as_deref(), rec.enacted, rec.speaker_decided), (None, false, false));
    assert!(g.state.laws.is_empty());
}

#[test]
fn a_tie_the_speaker_broke_is_recorded_as_such() {
    let (g, compared, bailed, diff) = lockstep(|| {
        let w = VoteWindow::finished("checks", &["for", "against"], &[("a", "for"), ("b", "against")], &[("for", 2), ("against", 2)], Some("against"));
        close(game(position(), &[]), w)
    });
    ok(compared, bailed, &diff);
    assert!(g.state.agenda_log.last().unwrap().speaker_decided);
}

fn election(winner: &str) -> VoteWindow {
    VoteWindow::finished("censure", &["a", "b", "c"], &[("a", winner), ("c", winner)], &[(winner, 2)], Some(winner))
}

#[test]
fn an_election_names_the_elected_seat_and_runs_the_effect_after_the_close() {
    let (g, compared, bailed, diff) = lockstep(|| close(game(position(), &[]), election("b")));
    ok(compared, bailed, &diff);
    assert_eq!(points(&g.state, "b"), 1, "Political Censure's point");
    assert_eq!(g.state.laws.get("censure").map(String::as_str), Some("b"));
    assert!(g.events.iter().any(|e| e == "AGENDA_EFFECT_RESOLVED:censure"));
}

#[test]
fn a_planet_election_elects_its_controller() {
    // Holy Planet of Ixth: the controller of the elected planet gains a point (the effect, after the close).
    let (g, compared, bailed, diff) = lockstep(|| {
        let mut s = position();
        let planet = ti4_content::galaxy::planet(ContentStore::embedded(), "arcprime", DEFAULT).expect("a printed planet");
        let system = ti4_model::id::SystemId::new(planet.system_id().expect("a printed system"));
        s.system_mut(&system).planet_control.insert(ti4_model::id::PlanetId::new("arcprime"), p("b"));
        let w = VoteWindow::finished("holy_planet_of_ixth", &["arcprime"], &[("a", "arcprime")], &[("arcprime", 1)], Some("arcprime"));
        close(game(s, &[]), w)
    });
    ok(compared, bailed, &diff);
    assert_eq!(points(&g.state, "b"), 1);
    assert_eq!(g.state.agenda_log.last().unwrap().outcome.as_deref(), Some("arcprime"));
}

#[test]
fn confounding_legal_text_redirects_the_election() {
    let (g, compared, bailed, diff) = lockstep(|| {
        let mut s = position();
        hand(&mut s, "c", &["confounding"]);
        let mut g = Game::new(s, ContentStore::embedded()).with_sources(DEFAULT);
        g.put_vote(election("b"));
        assert_eq!(g.close_vote().error, None);
        g
    });
    ok(compared, bailed, &diff);
    assert_eq!(points(&g.state, "c"), 1, "the redirected seat gets the point");
    assert_eq!(points(&g.state, "b"), 0);
    assert_eq!(g.state.laws.get("censure").map(String::as_str), Some("b"), "the law keeps the vote's result");
    assert_eq!(g.state.agenda_log.last().unwrap().redirected_to.as_deref(), Some("c"));
    assert!(g.state.agenda_elected_override.is_none());
    assert!(g.state.player(&p("c")).unwrap().action_cards.is_empty());
}

#[test]
fn deadly_plot_discards_the_agenda_with_no_effect() {
    let (g, compared, bailed, diff) = lockstep(|| {
        let mut s = position();
        hand(&mut s, "b", &["deadly_plot"]);
        let mut g = Game::new(s, ContentStore::embedded()).with_sources(DEFAULT);
        g.put_vote(election("a"));
        assert_eq!(g.close_vote().error, None);
        g
    });
    ok(compared, bailed, &diff);
    // b did not vote or predict another outcome: the card is not playable and the election stands.
    assert_eq!(points(&g.state, "a"), 1);
    let (g, compared, bailed, diff) = lockstep(|| {
        let mut s = position();
        hand(&mut s, "b", &["deadly_plot"]);
        let w = VoteWindow::finished("censure", &["a", "b", "c"], &[("a", "a"), ("b", "c")], &[("a", 2), ("c", 1)], Some("a"));
        let mut g = Game::new(s, ContentStore::embedded()).with_sources(DEFAULT);
        g.put_vote(w);
        assert_eq!(g.close_vote().error, None);
        g
    });
    ok(compared, bailed, &diff);
    assert_eq!(points(&g.state, "a"), 0, "no effect");
    assert!(g.state.laws.is_empty(), "no law");
    let rec = g.state.agenda_log.last().unwrap();
    assert!(rec.discarded && !rec.enacted);
    assert!(g.state.player(&p("b")).unwrap().action_cards.is_empty());
    assert!(g.events.iter().any(|e| e == "AGENDA_DISCARDED:censure"));
    assert!(!g.state.transient_flags.has(ti4_model::state::TransientFlags::AGENDA_DISCARDED));
}

#[test]
fn covert_legislation_resolves_the_face_down_agenda() {
    let (g, compared, bailed, diff) = lockstep(|| {
        let mut s = position();
        s.covert_agenda = Some("checks".to_owned());
        close(game(s, &[]), law_vote("covert", Some("for")))
    });
    ok(compared, bailed, &diff);
    assert_eq!(g.state.laws.get("checks").map(String::as_str), Some("for"));
    assert!(g.state.laws.get("covert").is_none());
    let log: Vec<&str> = g.state.agenda_log.iter().map(|r| r.agenda.as_str()).collect();
    assert_eq!(log, ["covert", "checks"]);
    assert!(g.state.covert_agenda.is_none());
    assert!(g.events.iter().any(|e| e == "COVERT_AGENDA_RESOLVED:checks:for"));
}

#[test]
fn classified_document_leaks_publishes_the_elected_secret() {
    let (g, compared, bailed, diff) = lockstep(|| {
        let mut s = position();
        s.scored_objectives.entry(p("a")).or_default().insert(ti4_model::id::ObjectiveId::new("ans"));
        let w = VoteWindow::finished("classified", &["ans"], &[("a", "ans")], &[("ans", 1)], Some("ans"));
        close(game(s, &[]), w)
    });
    ok(compared, bailed, &diff);
    assert!(g.state.revealed_objectives.iter().any(|o| o.as_str() == "ans"));
    assert_eq!(g.state.laws.get("classified").map(String::as_str), Some("ans"));
}

#[test]
fn imperial_rider_pays_a_point_and_a_miss_pays_nothing() {
    let (g, compared, bailed, diff) = lockstep(|| {
        let mut s = position();
        predict(&mut s, "a", "for");
        predict(&mut s, "b", "against");
        predict(&mut s, "c", "none|assassin");
        close(game(s, &[]), law_vote("checks", Some("for")))
    });
    ok(compared, bailed, &diff);
    assert_eq!((points(&g.state, "a"), points(&g.state, "b"), points(&g.state, "c")), (1, 0, 0));
    assert!(g.state.agenda_predictions.is_empty());
    assert!(g.events.iter().any(|e| e == "AGENDA_PREDICTION_CORRECT:a"));
}

#[test]
fn trade_rider_and_sanction_in_lockstep() {
    let (g, compared, bailed, diff) = lockstep(|| {
        let mut s = position();
        predict(&mut s, "a", "for|trade_rider");
        predict(&mut s, "b", "for|sanction");
        close(game(s, &[]), law_vote("checks", Some("for")))
    });
    ok(compared, bailed, &diff);
    let goods = |id: &str| g.state.player(&p(id)).unwrap().trade_goods;
    assert_eq!((goods("a"), goods("b")), (5, 0));
}

#[test]
fn leadership_rider_asks_three_pools() {
    let (g, compared, bailed, diff) = lockstep(|| {
        let mut s = position();
        predict(&mut s, "c", "for|lead_rider");
        predict(&mut s, "a", "for");
        close(game(s, &["fleet_tokens", "tactic_tokens", "fleet_tokens"]), law_vote("checks", Some("for")))
    });
    ok(compared, bailed, &diff);
    assert_eq!((tokens(&g.state, "c", TokenPool::Fleet), tokens(&g.state, "c", TokenPool::Tactic)), (2, 1));
    assert_eq!(points(&g.state, "a"), 1);
}

#[test]
fn politics_and_keleres_riders_draw_and_pay() {
    let (g, compared, bailed, diff) = lockstep(|| {
        let mut s = position();
        s.action_card_deck = ["sabo1", "sabo2", "sabo3", "veto", "hack"].iter().map(|c| (*c).to_owned()).map(ActionCardId::new).collect();
        predict(&mut s, "b", "for|politic_rider");
        close(game(s, &[]), law_vote("checks", Some("for")))
    });
    ok(compared, bailed, &diff);
    assert_eq!(g.state.player(&p("b")).unwrap().action_cards.len(), 3);
    assert_eq!(g.state.speaker, p("b"));
    let (g, compared, bailed, diff) = lockstep(|| {
        let mut s = position();
        s.action_card_deck = ["sabo1", "veto"].iter().map(|c| ActionCardId::new(*c)).collect();
        predict(&mut s, "a", "for|keleres_rider");
        close(game(s, &[]), law_vote("checks", Some("for")))
    });
    ok(compared, bailed, &diff);
    assert_eq!(g.state.player(&p("a")).unwrap().action_cards.len(), 1);
    assert_eq!(g.state.player(&p("a")).unwrap().trade_goods, 2);
}

#[test]
fn riders_the_core_does_not_pay_have_no_window() {
    for rider in ["tech_rider", "const_rider", "diplo_rider", "war_rider"] {
        let (_, compared, bailed, _) = lockstep(|| {
            let mut s = position();
            predict(&mut s, "a", &format!("for|{rider}"));
            // The string asks its research / placement questions with no answers scripted: the table's default.
            let mut g = Game::new(s, ContentStore::embedded()).with_sources(DEFAULT);
            g.put_vote(law_vote("checks", Some("for")));
            let _ = g.close_vote();
        });
        assert_eq!((compared, bailed), (0, 1), "{rider}");
        // The same rider on a missed prediction pays nothing: the window runs.
        let (_, compared, bailed, diff) = lockstep(|| {
            let mut s = position();
            predict(&mut s, "a", &format!("against|{rider}"));
            close(game(s, &[]), law_vote("checks", Some("for")))
        });
        ok(compared, bailed, &diff);
    }
}

#[test]
fn a_draw_a_faction_hook_changes_has_no_window() {
    // The drawer is the Yssaril: its Scheming changes the draw. A Yssaril beside the drawer does not (W6: every seat was hooked).
    for (drawer, bail) in [("b", true), ("a", false)] {
        let (_, compared, bailed, _) = lockstep(|| {
            let content = ContentStore::embedded();
            let mut s = crate::fixtures::seated_game(&[("a", "hacan"), ("b", "yssaril"), ("c", "arborec")], DEFAULT);
            crate::promissory::deal(&mut s, content, DEFAULT);
            s.phase = Phase::Agenda;
            predict(&mut s, drawer, "for|politic_rider");
            let mut g = Game::new(s, ContentStore::embedded()).with_sources(DEFAULT);
            g.put_vote(law_vote("checks", Some("for")));
            let _ = g.close_vote();
        });
        assert_eq!((compared, bailed), if bail { (0, 1) } else { (1, 0) }, "drawer {drawer}");
    }
}

#[test]
fn old_games_diplomacy_reserves_and_unknown_agendas_have_no_window() {
    let (_, compared, bailed, _) = lockstep(|| {
        let mut s = position();
        s.rules_version = 11;
        predict(&mut s, "a", "for");
        let mut g = game(s, &[]);
        g.put_vote(law_vote("checks", Some("for")));
        let _ = g.close_vote();
    });
    assert_eq!((compared, bailed), (0, 1), "below the pool question a correct rider is not paid");
    let (_, compared, bailed, _) = lockstep(|| {
        let mut s = position();
        s.deck_reserve = Some(ti4_model::deck_reserve::DeckReserve::new(
            std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            ti4_model::deck_reserve::DeckPlan::default(),
        ));
        let mut g = game(s, &[]);
        g.put_vote(law_vote("checks", Some("for")));
        let _ = g.close_vote();
    });
    assert_eq!((compared, bailed), (0, 1));
    let (_, compared, bailed, _) = lockstep(|| {
        let mut s = position();
        s.diplomacy.enabled = true;
        let mut g = game(s, &[]);
        g.put_vote(law_vote("checks", Some("for")));
        let _ = g.close_vote();
    });
    assert_eq!((compared, bailed), (0, 1));
    let (_, compared, bailed, _) = lockstep(|| {
        let mut g = game(position(), &[]);
        g.put_vote(law_vote("not_an_agenda", Some("for")));
        let _ = g.close_vote();
    });
    assert_eq!((compared, bailed), (0, 1));
    // The fixture faction `generic` is refused by the codecs (B6-design-02).
    let (_, compared, bailed, _) = lockstep(|| {
        let mut s = crate::fixtures::game(&["a", "b"]);
        s.phase = Phase::Agenda;
        let mut g = game(s, &[]);
        g.put_vote(law_vote("checks", Some("for")));
        let _ = g.close_vote();
    });
    assert_eq!((compared, bailed), (0, 1));
}

#[test]
fn the_unofficial_action_cards_leave_the_deck_before_a_rider_draws_from_rules_version_30() {
    for (version, kept) in [(35, ["sabo1", "sabo2", "sabo3"]), (29, ["veto3", "sabo1", "sabo2"])] {
        let (g, compared, bailed, diff) = lockstep(|| {
            let mut s = position();
            s.rules_version = version;
            s.action_card_deck = ["veto3", "sabo1", "sabo2", "sabo3"].iter().map(|c| ActionCardId::new(*c)).collect();
            predict(&mut s, "b", "for|politic_rider");
            close(game(s, &[]), law_vote("checks", Some("for")))
        });
        ok(compared, bailed, &diff);
        let drawn: Vec<String> = g.state.player(&p("b")).unwrap().action_cards.iter().map(|c| c.to_string()).collect();
        assert_eq!(drawn, kept, "rules version {version}");
    }
}

#[test]
fn a_hook_leaves_the_rider_to_the_string_and_a_full_hand_is_the_cores() {
    // Politics Rider with six cards in hand: the draw leaves the hand over its limit and asks a discard in the core window
    // (W6; it was a bail of the window).
    let (_, compared, bailed, _) = lockstep(|| {
        let mut s = position();
        s.action_card_deck = ["sabo1", "sabo2", "sabo3"].iter().map(|c| ActionCardId::new(*c)).collect();
        hand(&mut s, "b", &["hack", "hack", "hack", "hack", "hack", "hack"]);
        predict(&mut s, "b", "for|politic_rider");
        let mut g = Game::new(s, ContentStore::embedded()).with_sources(DEFAULT);
        g.put_vote(law_vote("checks", Some("for")));
        let _ = g.close_vote();
        assert_eq!(g.state.player(&p("b")).unwrap().action_cards.len(), 7);
    });
    assert_eq!((compared, bailed), (1, 0));
    // The drawer's own Scheming is the string's.
    let (_, compared, bailed, _) = lockstep(|| {
        let mut s = position();
        s.player_mut(&p("b")).unwrap().faction = ti4_model::id::FactionId::new("yssaril");
        s.action_card_deck = ["sabo1", "sabo2", "sabo3", "sabo1"].iter().map(|c| ActionCardId::new(*c)).collect();
        predict(&mut s, "b", "for|politic_rider");
        let mut g = Game::new(s, ContentStore::embedded()).with_sources(DEFAULT);
        g.put_vote(law_vote("checks", Some("for")));
        let _ = g.close_vote();
    });
    assert_eq!((compared, bailed), (0, 1));
}

#[test]
fn a_deck_reserve_is_the_named_reason_of_the_bail() {
    let (_, compared, bailed, _) = lockstep(|| {
        let mut s = position();
        s.deck_reserve = Some(ti4_model::deck_reserve::DeckReserve::new(
            std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            ti4_model::deck_reserve::DeckPlan::default(),
        ));
        let mut g = game(s, &[]);
        g.put_vote(law_vote("checks", Some("for")));
        let _ = g.close_vote();
        assert_eq!(crate::dual::report().windows.get("game::close_vote [bail: deck reserve]").map(|w| w.bailed), Some(1));
    });
    assert_eq!((compared, bailed), (0, 1));
}

// -- Committee Formation ------------------------------------------------------------------------------------------

const COMMITTEE: &str = "game::committee_formation";

fn committee_position() -> GameState {
    let mut s = position();
    s.laws.insert("committee".to_owned(), "b".to_owned());
    s.speaker = p("a");
    s
}

fn committee_lockstep<T>(body: impl FnOnce() -> T) -> (T, u64, u64, String) {
    let _guard = crate::dual::tests::serial();
    let mask = AreaMask::parse("agenda=dual", AreaMask::all(Backend::String)).unwrap();
    let _ = take_report();
    let before = local_windows(COMMITTEE);
    let out = with_mask(mask, body);
    let report = take_report();
    let now = local_windows(COMMITTEE);
    let differences = if report.total_differences() == 0 { String::new() } else { format!("{report:?}") };
    (out, now.0 - before.0, now.1 - before.1, differences)
}

#[test]
fn committee_formation_elects_the_player_its_owner_names_and_the_vote_is_skipped() {
    let (g, compared, bailed, diff) = committee_lockstep(|| {
        // b discards the card, then names c; no vote follows for the agenda.
        let mut g = game(committee_position(), &["discard", "c"]);
        let _ = g.open_next_vote(vec!["censure".to_owned()]);
        g
    });
    ok(compared, bailed, &diff);
    assert_eq!(g.state.laws.get("censure").map(String::as_str), Some("c"));
    assert!(!g.state.laws.contains_key("committee"), "the card is discarded");
    let rec = g.state.agenda_log.last().unwrap();
    assert_eq!((rec.agenda.as_str(), rec.outcome.as_deref(), rec.enacted, rec.speaker_decided, rec.votes.len()), ("censure", Some("c"), true, false, 0));
    assert!(g.events.iter().any(|e| e == "AGENDA_RESOLVED:censure:c") && g.events.iter().any(|e| e == "LAW_ENACTED:censure:c"));
}

#[test]
fn committee_formation_declined_or_not_applicable_asks_nothing() {
    // Declined: the discard window runs, the election window does not.
    let (g, compared, bailed, _) = committee_lockstep(|| {
        let mut g = game(committee_position(), &["decline", "decline"]);
        let _ = g.open_next_vote(vec!["censure".to_owned()]);
        g
    });
    assert_eq!((compared, bailed), (0, 0));
    assert!(g.state.laws.contains_key("committee") && !g.state.laws.contains_key("censure"));
    // An agenda that elects no player is not offered the card at all.
    let (g, compared, bailed, _) = committee_lockstep(|| {
        let mut g = game(committee_position(), &[]);
        let _ = g.open_next_vote(vec!["checks".to_owned()]);
        g
    });
    assert_eq!((compared, bailed), (0, 0));
    assert!(g.state.laws.contains_key("committee"));
}

#[test]
fn committee_formation_on_generic_fixture_seats_has_no_window() {
    let (_, compared, bailed, _) = committee_lockstep(|| {
        let mut s = crate::fixtures::game(&["a", "b"]);
        s.phase = Phase::Agenda;
        s.laws.insert("committee".to_owned(), "b".to_owned());
        let mut g = game(s, &["discard", "a"]);
        let _ = g.open_next_vote(vec!["censure".to_owned()]);
    });
    assert_eq!((compared, bailed), (0, 1));
}

#[test]
fn confusing_legal_text_asks_who_is_elected_instead() {
    let (g, compared, bailed, diff) = lockstep(|| {
        let mut s = position();
        hand(&mut s, "b", &["confusing"]);
        let mut g = Game::new(s, ContentStore::embedded()).with_sources(DEFAULT);
        g.put_vote(election("b"));
        assert_eq!(g.close_vote().error, None);
        g
    });
    ok(compared, bailed, &diff);
    assert!(g.state.player(&p("b")).unwrap().action_cards.is_empty(), "the card was played");
    assert_eq!(points(&g.state, "b"), 0);
    assert_eq!(points(&g.state, "a") + points(&g.state, "c"), 1, "the redirect names another seat");
}

#[test]
fn several_cards_played_into_one_resolution_are_followed() {
    // (holder, card) sets played into the `AGENDA_RESOLVED` window of an election of b by a and c.
    let sets: [&[(&str, &str)]; 4] = [
        &[("b", "confusing"), ("c", "confounding")],
        &[("c", "confounding"), ("a", "deadly_plot")],
        &[("b", "confusing"), ("a", "deadly_plot")],
        &[("a", "confounding"), ("b", "confusing"), ("c", "deadly_plot")],
    ];
    for set in sets {
        let (_, compared, bailed, diff) = lockstep(|| {
            let mut s = position();
            for (who, card) in set {
                let mut cards: Vec<String> = s.player(&p(who)).unwrap().action_cards.iter().map(|c| c.to_string()).collect();
                cards.push((*card).to_owned());
                s.player_mut(&p(who)).unwrap().action_cards = cards.iter().map(|c| ActionCardId::new(c.as_str())).collect();
            }
            let w = VoteWindow::finished("censure", &["a", "b", "c"], &[("a", "b"), ("c", "a")], &[("b", 2), ("a", 1)], Some("b"));
            let mut g = Game::new(s, ContentStore::embedded()).with_sources(DEFAULT);
            g.put_vote(w);
            let _ = g.close_vote();
        });
        assert!(diff.is_empty(), "{set:?}: {diff}");
        assert_eq!((compared, bailed), (1, 0), "{set:?}");
    }
}

// -- the plain events of the close (package P1.1) ---------------------------------------------------------------

#[test]
fn the_plain_events_of_a_covert_close_with_a_prediction_are_counted_and_compared() {
    let _guard = crate::dual::tests::serial();
    let mask = AreaMask::parse("agenda=dual", AreaMask::all(Backend::String)).unwrap();
    let _ = take_report();
    let before = local_windows(LABEL);
    let g = with_mask(mask, || {
        let mut s = position();
        s.covert_agenda = Some("checks".to_owned());
        predict(&mut s, "a", "for");
        close(game(s, &[]), law_vote("covert", Some("for")))
    });
    let report = take_report();
    let now = local_windows(LABEL);
    assert_eq!((now.0 - before.0, now.1 - before.1), (1, 0), "{report}");
    assert_eq!(report.total_differences(), 0, "{report}");
    // The effect of the agenda (`AGENDA_EFFECT_RESOLVED`) is a window of its own and runs after the close.
    let plain: Vec<&str> = g.events.iter().map(String::as_str).filter(|e| e.contains(':') && !e.starts_with("AGENDA_EFFECT")).collect();
    assert_eq!(
        plain,
        ["AGENDA_RESOLVED:covert:for", "COVERT_AGENDA_RESOLVED:checks:for", "AGENDA_PREDICTION_CORRECT:a", "LAW_ENACTED:checks:for"]
    );
    for name in ["AGENDA_RESOLVED", "COVERT_AGENDA_RESOLVED", "AGENDA_PREDICTION_CORRECT", "LAW_ENACTED"] {
        assert_eq!(report.plain.get(&format!("{LABEL} | {name}")), Some(&1), "{name}: {:?}", report.plain);
    }
}
