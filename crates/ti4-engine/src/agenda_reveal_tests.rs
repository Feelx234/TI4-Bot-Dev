//! Lockstep tests of the agenda reveal window (package B9c-1): `Game::reveal_agenda` run under `agenda=dual`, the
//! string chain and `rules::agenda::reveal::Reveal` side by side. Every test asserts that the core half ran
//! (`compared`), not only that nothing differed: a window that bails compares nothing.

use ti4_content::ContentStore;
use ti4_model::content_types::DEFAULT;
use ti4_model::id::PlayerId;
use ti4_model::state::{GameState, TokenPool};

use crate::choice::{Scripted, Table};
use crate::dual::{AreaMask, Backend, local_windows, take_report, with_mask};
use crate::game::Game;

const LABEL: &str = "game::reveal_agenda";

/// Three real factions with their promissory notes dealt (the generic fixture factions would bail the codecs).
fn position(tokens: i32, deck: &[&str]) -> GameState {
    let content = ContentStore::embedded();
    let mut state = crate::fixtures::seated_game(&[("a", "hacan"), ("b", "xxcha"), ("c", "arborec")], DEFAULT);
    crate::promissory::deal(&mut state, content, DEFAULT);
    let b = PlayerId::new("b");
    let seat = state.player_mut(&b).unwrap();
    let held = seat.tokens(TokenPool::Strategic);
    seat.gain_token_uncapped(TokenPool::Strategic, tokens - held);
    state.agenda_deck = deck.iter().map(|s| (*s).to_owned()).collect();
    state
}

fn game(state: GameState, script: &[&str]) -> Game<'static> {
    let table = Table::with_default(Box::new(Scripted::new(script.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>())));
    Game::with_table(state, ContentStore::embedded(), table).with_sources(DEFAULT)
}

/// Run `body` under `agenda=dual`; returns what it returned, the windows compared and bailed under the label, and
/// the differences found (empty when the two halves agree).
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

fn reveal(game: &mut Game<'static>, alias: &str) -> Option<(String, Vec<String>)> {
    game.reveal_agenda(alias).expect("the reveal runs")
}

fn tokens(state: &GameState, id: &str) -> i32 {
    state.player(&PlayerId::new(id)).unwrap().tokens(TokenPool::Strategic)
}

#[test]
fn a_plain_reveal_declined_quash_runs_in_lockstep() {
    let ((found, state), compared, bailed, diff) = lockstep(|| {
        let mut g = game(position(2, &["committee", "conventions"]), &["decline"]);
        let found = reveal(&mut g, "secret");
        (found, g.state)
    });
    assert_eq!((compared, bailed), (1, 0), "{diff}");
    assert!(diff.is_empty(), "{diff}");
    assert_eq!(found.map(|f| f.0), Some("secret".to_owned()));
    assert_eq!(tokens(&state, "b"), 2);
    assert_eq!(state.agenda_seq, 1);
}

fn log(state: &GameState) -> Vec<(String, Option<String>, bool)> {
    state.agenda_log.iter().map(|r| (r.agenda.clone(), r.discard_reason.clone(), r.discarded)).collect()
}

fn staged_spends(state: &GameState) -> usize {
    state.faction_marks.keys().filter(|k| k.contains("STRATEGY_TOKEN_SPENT")).count().max(
        state.faction_marks.values().filter(|v| v.contains("STRATEGY_TOKEN_SPENT")).count(),
    )
}

fn ok(compared: u64, bailed: u64, diff: &str) {
    assert_eq!((compared, bailed), (1, 0), "{diff}");
    assert!(diff.is_empty(), "{diff}");
}

#[test]
fn quash_used_replaces_the_agenda_and_logs_it_as_replaced() {
    let ((found, state), compared, bailed, diff) = lockstep(|| {
        let mut g = game(position(2, &["committee", "conventions"]), &["use", "decline"]);
        let found = reveal(&mut g, "secret");
        (found, g.state)
    });
    ok(compared, bailed, &diff);
    assert_eq!(found.map(|f| f.0), Some("committee".to_owned()));
    assert_eq!(tokens(&state, "b"), 1);
    assert_eq!(state.agenda_deck, ["conventions"]);
    assert_eq!(log(&state), [("secret".to_owned(), Some("replaced".to_owned()), true)]);
    assert_eq!(state.agenda_seq, 2);
    assert!(staged_spends(&state) > 0, "the spent token is announced");
}

#[test]
fn quash_without_a_token_or_with_an_empty_deck_is_not_offered() {
    let (_, compared, bailed, diff) = lockstep(|| {
        let mut g = game(position(0, &["committee"]), &[]);
        assert_eq!(reveal(&mut g, "secret").map(|f| f.0), Some("secret".to_owned()));
    });
    ok(compared, bailed, &diff);
    let (_, compared, bailed, diff) = lockstep(|| {
        let mut g = game(position(2, &[]), &[]);
        assert_eq!(reveal(&mut g, "secret").map(|f| f.0), Some("secret".to_owned()));
    });
    ok(compared, bailed, &diff);
}

fn lend(state: &mut GameState, alias: &str, faction: &str, holder: &str) {
    state.promissory_notes.insert(crate::promissory::note_id(alias, faction), PlayerId::new(holder));
}

#[test]
fn political_favor_replaces_the_agenda_and_goes_home() {
    let ((found, state), compared, bailed, diff) = lockstep(|| {
        let mut s = position(2, &["committee", "conventions"]);
        lend(&mut s, "favor", "xxcha", "a");
        // b (Xxcha) declines Quash, a uses Favor, b declines Quash on the replacement.
        let mut g = game(s, &["decline", "use", "decline"]);
        let found = reveal(&mut g, "secret");
        (found, g.state)
    });
    ok(compared, bailed, &diff);
    assert_eq!(found.map(|f| f.0), Some("committee".to_owned()));
    assert_eq!(tokens(&state, "b"), 1);
    assert_eq!(state.promissory_notes.get("favor:xxcha"), Some(&PlayerId::new("b")), "returned to its owner");
    assert_eq!(log(&state), [("secret".to_owned(), Some("replaced".to_owned()), true)]);
}

#[test]
fn political_favor_is_not_offered_when_its_owner_cannot_pay() {
    let (_, compared, bailed, diff) = lockstep(|| {
        let mut s = position(0, &["committee"]);
        lend(&mut s, "favor", "xxcha", "a");
        let mut g = game(s, &[]);
        assert_eq!(reveal(&mut g, "secret").map(|f| f.0), Some("secret".to_owned()));
        assert_eq!(g.state.promissory_notes.get("favor:xxcha"), Some(&PlayerId::new("a")));
    });
    ok(compared, bailed, &diff);
}

#[test]
fn political_secret_bars_its_owner_and_goes_home() {
    let ((found, state), compared, bailed, diff) = lockstep(|| {
        let mut s = position(0, &["committee"]);
        lend(&mut s, "ps", "hacan", "c");
        let mut g = game(s, &["use"]);
        let found = reveal(&mut g, "secret");
        (found, g.state)
    });
    ok(compared, bailed, &diff);
    assert_eq!(found.map(|f| f.0), Some("secret".to_owned()));
    assert_eq!(state.agenda_predictions.get(&PlayerId::new("a")).map(String::as_str), Some("none|political_secret"));
    assert_eq!(state.promissory_notes.get("ps:hacan"), Some(&PlayerId::new("a")));
}

#[test]
fn political_secret_declined_and_already_barred_owners() {
    let (state, compared, bailed, diff) = lockstep(|| {
        let mut s = position(0, &["committee"]);
        lend(&mut s, "ps", "hacan", "c");
        lend(&mut s, "ps", "arborec", "a");
        s.agenda_predictions.insert(PlayerId::new("b"), "none|assassin".to_owned());
        lend(&mut s, "ps", "xxcha", "a");
        // Asked in the text order of the note ids: a's note on c (arborec) is declined, then c's note on a
        // (hacan) is used; b is already barred: not offered.
        let mut g = game(s, &["decline", "use"]);
        reveal(&mut g, "secret").expect("an agenda");
        g.state
    });
    ok(compared, bailed, &diff);
    assert_eq!(state.promissory_notes.get("ps:arborec"), Some(&PlayerId::new("a")), "declined: stays lent");
    assert_eq!(state.promissory_notes.get("ps:hacan"), Some(&PlayerId::new("a")), "used: goes home");
    assert!(state.agenda_predictions.contains_key(&PlayerId::new("a")));
    assert!(!state.agenda_predictions.contains_key(&PlayerId::new("c")));
    assert_eq!(state.promissory_notes.get("ps:xxcha"), Some(&PlayerId::new("a")), "the barred owner is not offered");
}

#[test]
fn an_agenda_with_nothing_to_elect_is_logged_and_the_next_one_revealed() {
    let ((found, state), compared, bailed, diff) = lockstep(|| {
        let mut g = game(position(0, &["committee", "conventions"]), &[]);
        let found = reveal(&mut g, "classified");
        (found, g.state)
    });
    ok(compared, bailed, &diff);
    assert_eq!(found.map(|f| f.0), Some("committee".to_owned()));
    assert_eq!(log(&state), [("classified".to_owned(), Some("no_choice".to_owned()), true)]);
    assert_eq!(state.agenda_seq, 2);
    // Below rules version 14 the first agenda with nothing to elect ends the reveal, unlogged.
    let ((found, state), compared, bailed, diff) = lockstep(|| {
        let mut s = position(0, &["committee"]);
        s.rules_version = ti4_model::state::RULES_AGENDA_GRANT_REVEAL - 1;
        let mut g = game(s, &[]);
        let found = reveal(&mut g, "classified");
        (found, g.state)
    });
    ok(compared, bailed, &diff);
    assert!(found.is_none());
    assert!(state.agenda_log.is_empty());
    assert_eq!(state.agenda_deck, ["committee"]);
}

#[test]
fn an_empty_deck_ends_the_reveal_after_the_discard() {
    let ((found, state), compared, bailed, diff) = lockstep(|| {
        let mut g = game(position(0, &[]), &[]);
        let found = reveal(&mut g, "classified");
        (found, g.state)
    });
    ok(compared, bailed, &diff);
    assert!(found.is_none());
    assert_eq!(log(&state), [("classified".to_owned(), Some("no_choice".to_owned()), true)]);
}

#[test]
fn covert_legislation_draws_its_face_down_agenda_and_loses_it_when_replaced() {
    let ((found, state), compared, bailed, diff) = lockstep(|| {
        let mut g = game(position(0, &["classified", "committee", "prophecy"]), &[]);
        let found = reveal(&mut g, "covert");
        (found, g.state)
    });
    ok(compared, bailed, &diff);
    let (alias, choices) = found.expect("a vote");
    assert_eq!(alias, "covert");
    assert_eq!(choices, ["a", "b", "c"]);
    assert_eq!(state.covert_agenda.as_deref(), Some("committee"));
    assert_eq!(state.agenda_deck, ["prophecy"]);
    // Quash on Covert Legislation: the face-down agenda goes with it.
    let ((found, state), compared, bailed, diff) = lockstep(|| {
        let mut g = game(position(2, &["committee", "conventions", "prophecy"]), &["use", "decline"]);
        let found = reveal(&mut g, "covert");
        (found, g.state)
    });
    ok(compared, bailed, &diff);
    assert_eq!(found.map(|f| f.0), Some("conventions".to_owned()));
    assert!(state.covert_agenda.is_none());
    assert_eq!(log(&state), [("covert".to_owned(), Some("replaced".to_owned()), true)]);
    // An empty deck leaves Covert Legislation nothing to elect.
    let ((found, state), compared, bailed, diff) = lockstep(|| {
        let mut s = position(0, &[]);
        s.covert_agenda = Some("committee".to_owned());
        let mut g = game(s, &[]);
        let found = reveal(&mut g, "covert");
        (found, g.state)
    });
    ok(compared, bailed, &diff);
    assert!(found.is_none());
    assert!(state.covert_agenda.is_none());
}

#[test]
fn a_deck_reserve_has_no_reveal_window() {
    let (_, compared, bailed, _) = lockstep(|| {
        let mut s = position(0, &["committee"]);
        s.deck_reserve = Some(ti4_model::deck_reserve::DeckReserve::new(
            std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            ti4_model::deck_reserve::DeckPlan::default(),
        ));
        let mut g = game(s, &[]);
        let _ = g.reveal_agenda("secret");
    });
    assert_eq!((compared, bailed), (0, 1));
}

fn with_cards(mut state: GameState, who: &str, cards: &[&str]) -> GameState {
    state.phase = ti4_model::state::Phase::Agenda;
    state.player_mut(&PlayerId::new(who)).unwrap().action_cards =
        cards.iter().map(|c| ti4_model::id::ActionCardId::new(*c)).collect();
    state
}

#[test]
fn veto_played_into_the_window_replaces_the_agenda() {
    let ((found, state), compared, bailed, diff) = lockstep(|| {
        let s = with_cards(position(0, &["prophecy", "committee"]), "a", &["veto"]);
        let mut g = Game::new(s, ContentStore::embedded()).with_sources(DEFAULT);
        let found = reveal(&mut g, "secret");
        (found, g.state)
    });
    ok(compared, bailed, &diff);
    assert_eq!(found.map(|f| f.0), Some("prophecy".to_owned()));
    assert_eq!(log(&state), [("secret".to_owned(), Some("replaced".to_owned()), true)]);
    assert!(state.player(&PlayerId::new("a")).unwrap().action_cards.is_empty(), "the Veto is spent");
    assert_eq!(state.agenda_seq, 2);
}

#[test]
fn two_vetos_played_into_one_window_take_the_last_replacement() {
    let ((found, state), compared, bailed, diff) = lockstep(|| {
        let s = with_cards(position(0, &["prophecy", "committee", "conventions"]), "a", &["veto", "veto3"]);
        let mut g = Game::new(s, ContentStore::embedded()).with_sources(DEFAULT);
        let found = reveal(&mut g, "secret");
        (found, g.state)
    });
    ok(compared, bailed, &diff);
    assert_eq!(found.map(|f| f.0), Some("committee".to_owned()));
    // Both cards are played into the first window; the last replacement stands and the discarded agenda is logged once.
    assert_eq!(log(&state), [("secret".to_owned(), Some("replaced".to_owned()), true)]);
    assert_eq!(state.agenda_seq, 2);
    assert!(state.player(&PlayerId::new("a")).unwrap().action_cards.is_empty());
}

#[test]
fn cards_played_into_the_window_are_followed() {
    // Cards "when" or "after an agenda is revealed" that the reaction table plays: the card is spent in both halves.
    for card in ["hack", "assassin", "imp_rider", "lead_rider", "dp1", "insider", "sanction"] {
        let ((found, left), compared, bailed, diff) = lockstep(|| {
            let s = with_cards(position(0, &["prophecy", "committee"]), "a", &[card]);
            let mut g = Game::new(s, ContentStore::embedded()).with_sources(DEFAULT);
            let found = reveal(&mut g, "secret").map(|f| f.0);
            (found, g.state.player(&PlayerId::new("a")).unwrap().action_cards.len())
        });
        ok(compared, bailed, &diff);
        assert_eq!((found.as_deref(), left), (Some("secret"), 0), "{card} was played and the agenda stands");
    }
}

#[test]
fn generic_fixture_seats_and_unknown_agendas_have_no_reveal_window() {
    // The fixture faction `generic` (and notes keyed by it) are refused by the codecs: the core half bails and
    // compares nothing (B6-design-02), it does not fail the string.
    let (found, compared, bailed, _) = lockstep(|| {
        let mut s = crate::fixtures::game(&["a", "b"]);
        s.agenda_deck = vec!["committee".to_owned()];
        let mut g = game(s, &[]);
        reveal(&mut g, "secret").map(|f| f.0)
    });
    assert_eq!((found.as_deref(), compared, bailed), (Some("secret"), 0, 1));
    // An agenda the core has no id for.
    let (_, compared, bailed, _) = lockstep(|| {
        let mut g = game(position(0, &["committee"]), &[]);
        let _ = g.reveal_agenda("not_an_agenda");
    });
    assert_eq!((compared, bailed), (0, 1));
}

#[test]
fn covert_legislation_reads_its_own_outcomes_below_rules_version_33() {
    let ((found, state), compared, bailed, diff) = lockstep(|| {
        let mut s = position(0, &["committee", "prophecy"]);
        s.rules_version = ti4_model::state::RULES_COVERT_LEGISLATION - 1;
        let mut g = game(s, &[]);
        let found = reveal(&mut g, "covert");
        (found, g.state)
    });
    ok(compared, bailed, &diff);
    let (alias, choices) = found.expect("a vote");
    assert_eq!((alias.as_str(), choices.as_slice()), ("covert", ["for".to_owned(), "against".to_owned()].as_slice()));
    assert!(state.covert_agenda.is_none());
    assert_eq!(state.agenda_deck, ["committee", "prophecy"], "nothing drawn face down");
}

#[test]
fn a_used_quash_leaves_political_favor_for_the_replacement() {
    let ((found, state), compared, bailed, diff) = lockstep(|| {
        let mut s = position(2, &["committee", "conventions"]);
        lend(&mut s, "favor", "xxcha", "a");
        // Quash is used on the first agenda (Favor is not offered on it); on the replacement b declines Quash and a
        // declines Favor.
        let mut g = game(s, &["use", "decline", "decline"]);
        let found = reveal(&mut g, "secret");
        (found, g.state)
    });
    ok(compared, bailed, &diff);
    assert_eq!(found.map(|f| f.0), Some("committee".to_owned()));
    assert_eq!(tokens(&state, "b"), 1);
    assert_eq!(state.promissory_notes.get("favor:xxcha"), Some(&PlayerId::new("a")), "still lent");
}

#[test]
fn political_favor_is_not_offered_with_an_empty_deck() {
    let (_, compared, bailed, diff) = lockstep(|| {
        let mut s = position(2, &[]);
        lend(&mut s, "favor", "xxcha", "a");
        let mut g = game(s, &[]);
        assert_eq!(reveal(&mut g, "secret").map(|f| f.0), Some("secret".to_owned()));
    });
    ok(compared, bailed, &diff);
}

// -- the plain events of the reveal (package P1.1) --------------------------------------------------------------

/// The report of one reveal run under `agenda=dual` (the lock held, the windows counted).
fn dual_report<T>(body: impl FnOnce() -> T) -> (T, crate::dual::DualReport, (u64, u64)) {
    let _guard = crate::dual::tests::serial();
    let mask = AreaMask::parse("agenda=dual", AreaMask::all(Backend::String)).unwrap();
    let _ = take_report();
    let before = local_windows(LABEL);
    let out = with_mask(mask, body);
    let report = take_report();
    let (c, b) = local_windows(LABEL);
    (out, report, (c - before.0, b - before.1))
}

#[test]
fn the_plain_events_of_the_reveal_are_counted_and_compared_with_the_core_windows() {
    let (found, report, windows) = dual_report(|| {
        let mut g = game(position(2, &["committee", "conventions"]), &["use", "decline"]);
        (reveal(&mut g, "secret").map(|f| f.0), g.events.clone())
    });
    assert_eq!(windows, (1, 0), "{report}");
    assert_eq!(report.total_differences(), 0, "{report}");
    let (agenda, events) = found;
    assert_eq!(agenda.as_deref(), Some("committee"));
    // `Game::events` holds the plain texts and the mirrored typed names, in the order they were pushed.
    let plain: Vec<&str> = events.iter().map(String::as_str).filter(|e| e.contains(':') || e.ends_with("_USED")).collect();
    assert_eq!(plain, ["AGENDA_REVEALED:secret", "QUASH_USED", "AGENDA_DISCARDED:secret", "AGENDA_REVEALED:committee"]);
    let census = |name: &str| report.plain.get(&format!("{LABEL} | {name}")).copied().unwrap_or(0);
    assert_eq!((census("AGENDA_REVEALED"), census("QUASH_USED"), census("AGENDA_DISCARDED")), (2, 1, 1), "{:?}", report.plain);
}

#[test]
fn the_core_reveal_that_forgets_a_plain_event_is_a_difference() {
    // The harness half of the mutation check: the two sides give different plain lists for one window.
    use crate::dual::{Area, Run, compare_runs};
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    let state = position(2, &["committee"]);
    let string = Run::new(1).with_plain(vec!["AGENDA_REVEALED:secret".to_owned(), "QUASH_USED".to_owned()]);
    let core = Run::new(1).with_plain(vec!["AGENDA_REVEALED:secret".to_owned()]);
    assert!(!compare_runs(Area::AgendaLaws, LABEL, &state, &string, &state, &core));
    let report = take_report();
    assert_eq!(report.total_differences(), 1, "{report}");
    assert!(report.differences[0].label.ends_with(".plain"), "{}", report.differences[0]);
    // A side that gives no list compares nothing.
    let silent = Run::new(1);
    assert!(compare_runs(Area::AgendaLaws, LABEL, &state, &string, &state, &silent));
    assert_eq!(take_report().total_differences(), 0);
}

#[test]
fn plain_events_pushed_outside_any_window_are_counted_under_the_game() {
    let _guard = crate::dual::tests::serial();
    let mask = AreaMask::parse("agenda=dual", AreaMask::all(Backend::String)).unwrap();
    let _ = take_report();
    with_mask(mask, || {
        crate::dual_window::note_plain_event("TURN_PASSED");
        crate::dual_window::note_plain_event("LEADER_UNLOCKED:sol_hero");
        crate::dual_window::note_plain_event("LEADER_UNLOCKED:l1z1x_hero");
    });
    let report = take_report();
    assert_eq!(report.plain.get("(game) | TURN_PASSED"), Some(&1), "{:?}", report.plain);
    assert_eq!(report.plain.get("(game) | LEADER_UNLOCKED"), Some(&2), "the argument is not part of the name");
    // Nothing is counted when no area is on Dual (the production path).
    let _ = take_report();
    with_mask(AreaMask::all(Backend::String), || crate::dual_window::note_plain_event("TURN_PASSED"));
    assert!(take_report().plain.is_empty());
}

// -- the interleaved order of the pushes (package P1.1b) ------------------------------------------------------------

#[test]
fn the_recorder_keeps_plain_events_and_mirrored_names_in_one_sequence_in_the_order_of_the_pushes() {
    use crate::dual_window::{Pushed, finish, note_mirrored_events, note_plain_event, start};
    let _guard = crate::dual::tests::serial();
    let mask = AreaMask::parse("agenda=dual", AreaMask::all(Backend::String)).unwrap();
    let rec = with_mask(mask, || {
        assert!(start());
        // One timing call: the plain event pushed after the emission is stored BEFORE the names the emission mirrors, because
        // `mirror_timing_log` copies them only when the surrounding call returns.
        note_plain_event("AGENDA_REVEALED:secret");
        note_plain_event("QUASH_USED");
        note_mirrored_events(&["AGENDA_REVEALED".to_owned(), "STRATEGY_TOKEN_SPENT".to_owned()]);
        note_plain_event("AGENDA_DISCARDED:secret");
        note_mirrored_events(&["AGENDA_REVEALED".to_owned()]);
        finish()
    });
    let want = [
        Pushed::Plain("AGENDA_REVEALED:secret".to_owned()),
        Pushed::Plain("QUASH_USED".to_owned()),
        Pushed::Mirrored("AGENDA_REVEALED".to_owned()),
        Pushed::Mirrored("STRATEGY_TOKEN_SPENT".to_owned()),
        Pushed::Plain("AGENDA_DISCARDED:secret".to_owned()),
        Pushed::Mirrored("AGENDA_REVEALED".to_owned()),
    ];
    assert_eq!(rec.order, want);
    assert_eq!(rec.plain, ["AGENDA_REVEALED:secret", "QUASH_USED", "AGENDA_DISCARDED:secret"], "the plain view");
    assert_eq!(rec.mirrored, ["AGENDA_REVEALED", "STRATEGY_TOKEN_SPENT", "AGENDA_REVEALED"], "the mirrored view");
    assert_eq!(want.iter().map(Pushed::text).collect::<Vec<_>>(), ["AGENDA_REVEALED:secret", "QUASH_USED", "AGENDA_REVEALED", "STRATEGY_TOKEN_SPENT", "AGENDA_DISCARDED:secret", "AGENDA_REVEALED"]);
}

#[test]
fn two_sides_that_push_the_same_events_in_another_order_are_a_difference_of_the_order() {
    use crate::dual::{Area, Run, compare_runs};
    use crate::dual_window::Pushed::{Mirrored, Plain};
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    let state = position(2, &["committee"]);
    let string = Run::new(1).with_order(vec![Plain("QUASH_USED".to_owned()), Mirrored("AGENDA_REVEALED".to_owned())]);
    let same = Run::new(1).with_order(vec![Plain("QUASH_USED".to_owned()), Mirrored("AGENDA_REVEALED".to_owned())]);
    assert!(compare_runs(Area::AgendaLaws, LABEL, &state, &string, &state, &same));
    assert_eq!(take_report().total_differences(), 0);
    // The same two events, the typed name first: the plain lists are equal, only the interleaving differs.
    let swapped = Run::new(1).with_order(vec![Mirrored("AGENDA_REVEALED".to_owned()), Plain("QUASH_USED".to_owned())]);
    assert!(!compare_runs(Area::AgendaLaws, LABEL, &state, &string, &state, &swapped));
    let report = take_report();
    assert_eq!(report.total_differences(), 1, "{report}");
    assert!(report.differences[0].label.ends_with(".order"), "{}", report.differences[0]);
    // A side that gives no order (or only a plain list) compares no order.
    let silent = Run::new(1).with_plain(vec!["QUASH_USED".to_owned()]);
    assert!(compare_runs(Area::AgendaLaws, LABEL, &state, &string, &state, &silent));
    assert_eq!(take_report().total_differences(), 0);
}

// -- Galactic Threat (package W9): the Nekro predicts after a reveal ------------------------------------------------------------

/// A Nekro (a), Xxcha (b) and Arborec (c), no strategic token to quash with.
fn nekro_position() -> GameState {
    let content = ContentStore::embedded();
    let mut state = crate::fixtures::seated_game(&[("a", "nekro"), ("b", "xxcha"), ("c", "arborec")], DEFAULT);
    crate::promissory::deal(&mut state, content, DEFAULT);
    state.agenda_deck = vec!["committee".to_owned()];
    for seat in &mut state.players {
        let held = seat.tokens(TokenPool::Strategic);
        seat.gain_token_uncapped(TokenPool::Strategic, -held);
    }
    state
}

#[test]
fn the_nekro_may_predict_the_outcome_once_per_round_in_lockstep() {
    // The first option of every question: the ability is used, the first outcome predicted.
    let ((found, state), compared, bailed, diff) = lockstep(|| {
        let mut g = game(nekro_position(), &[]);
        let found = reveal(&mut g, "checks");
        (found, g.state)
    });
    ok(compared, bailed, &diff);
    assert_eq!(found.map(|f| f.0), Some("checks".to_owned()));
    let mark = state.faction_marks.get("nekro:threat:a").cloned();
    assert!(mark.as_deref().is_some_and(|m| m.starts_with("1|")), "a prediction of agenda 1 is held: {mark:?}");
    assert_eq!(state.faction_marks.get("nekro:threat_phase:a"), Some(&state.round.to_string()));
    // Declined: nothing is held.
    let ((_, state), compared, bailed, diff) = lockstep(|| {
        let mut g = game(nekro_position(), &["decline"]);
        let found = reveal(&mut g, "checks");
        (found, g.state)
    });
    ok(compared, bailed, &diff);
    assert!(!state.faction_marks.contains_key("nekro:threat:a"));
    // Already predicted this round: the ability is not offered again (no question, nothing held but the old mark).
    let ((_, state), compared, bailed, diff) = lockstep(|| {
        let mut s = nekro_position();
        s.faction_marks.insert("nekro:threat_phase:a".to_owned(), s.round.to_string());
        let mut g = game(s, &[]);
        let found = reveal(&mut g, "checks");
        (found, g.state)
    });
    ok(compared, bailed, &diff);
    assert!(!state.faction_marks.contains_key("nekro:threat:a"));
}
