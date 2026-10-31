//! Lockstep tests of the follower window's step (package B8c-1): a real strategic action of a real game position is
//! played under the `Dual` strategy backend and the core `SecondaryStep` is compared with the string
//! `next_choice` / table / `take_choice` of every step. Real factions, real notes (`promissory::deal`).

use ti4_content::ContentStore;
use ti4_model::content_types::POK;
use ti4_model::id::{PlayerId, StrategyCardId};
use ti4_model::state::GameState;

use crate::choice::{Choice, ChoiceOption, Decider, IllegalChoice, Table};
use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
use crate::game::Game;

pub(crate) const LABEL: &str = "strategy::secondary";

pub(crate) fn pid(id: &str) -> PlayerId {
    PlayerId::new(id)
}

/// Takes the strategic action when offered, answers a follower's question with `follow` (or the first option when
/// that is not offered) and declines everything else.
pub(crate) struct Followers {
    pub follow: &'static str,
}

impl Decider for Followers {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        if choice.details.get("kind").and_then(|k| k.as_str()) == Some("strategy_secondary") {
            let wanted = |o: &&ChoiceOption| o.id == self.follow || (self.follow.ends_with('|') && o.id.starts_with(self.follow));
            let pick = choice.options.iter().find(wanted).or_else(|| choice.options.first());
            return Ok(pick.cloned().expect("an option"));
        }
        if let Some(strategic) = choice.options.iter().find(|o| o.id == crate::strategy::STRATEGIC_ACTION_ID) {
            return Ok(strategic.clone());
        }
        Ok(choice.options.iter().find(|o| o.is_decline()).or_else(|| choice.options.first()).cloned().expect("an option"))
    }
}

/// Three seats; `a` holds the card named `name` and is the primary; every seat has tokens and four trade goods.
pub(crate) fn dealt(factions: &[(&str, &str)], name: &str) -> GameState {
    dealt_to(crate::fixtures::seated_game(factions, POK), name, "a")
}

/// [`dealt`] on a given position, the card going to `holder`, who becomes the active player.
pub(crate) fn dealt_to(mut state: GameState, name: &str, holder: &str) -> GameState {
    let content = ContentStore::embedded();
    crate::promissory::deal(&mut state, content, POK);
    let card: StrategyCardId = state
        .unclaimed_strategy_cards
        .iter()
        .find(|card| crate::strategy_cards::card_name(content, card.as_str()).as_deref() == Some(name))
        .cloned()
        .expect("the deck carries this card");
    state.unclaimed_strategy_cards.retain(|c| c != &card);
    assert!(state.deal_strategy_card(&pid(holder), card));
    state.phase = ti4_model::state::Phase::Action;
    state.active = Some(pid(holder));
    for seat in &mut state.players {
        seat.strategic_tokens = 3;
        seat.trade_goods = 4;
    }
    state
}

/// Play the strategic action of `a` to its end under the Dual backend; `(compared, bailed)` of the step's window, the
/// bails with their reasons, and the game (for the position after).
pub(crate) fn play(state: GameState, follow: &'static str) -> ((u64, u64), Vec<String>, Game<'static>) {
    let _serial = crate::dual::tests::serial();
    let _ = take_report();
    crate::dual::reset_thread_windows();
    let mut game = None;
    with_mask(AreaMask::STRING.with(Area::Strategy, Backend::Dual), || {
        let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow })));
        for _ in 0..40 {
            let step = g.step();
            assert!(step.error.is_none(), "{:?}", step.error);
            if g.events.iter().any(|e| e == "STRATEGIC_ACTION_COMPLETE") {
                break;
            }
        }
        game = Some(g);
    });
    let r = report();
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    let bails: Vec<String> = r.windows.keys().filter(|k| k.starts_with(LABEL) && k.contains("[bail")).cloned().collect();
    let counts = crate::dual::thread_window_counts(LABEL);
    let _ = take_report();
    (counts, bails, game.expect("played"))
}

const KINDS: [&str; 8] = ["Leadership", "Diplomacy", "Politics", "Construction", "Trade", "Warfare", "Technology", "Imperial"];
const F3: [(&str, &str); 3] = [("a", "sol"), ("b", "hacan"), ("c", "arborec")];

/// Make the followers able to follow whatever the card: a ready planet is exhausted for Diplomacy, commodities are gone
/// for Trade.
fn prepare(state: &mut GameState) {
    for who in ["b", "c"] {
        state.player_mut(&pid(who)).unwrap().commodities = 0;
    }
    // Two placed planets, one controlled and exhausted by each follower (Diplomacy's secondary readies one).
    let content = ContentStore::embedded();
    let candidates: Vec<_> = ti4_content::galaxy::all_planets(content, POK)
        .iter()
        .filter(|(_, planet)| planet.system_id().is_some() && !planet.is_placed_during_play())
        .map(|(id, planet)| (ti4_model::id::SystemId::new(planet.system_id().unwrap_or("18")), ti4_model::id::PlanetId::new(*id)))
        .take(2)
        .collect();
    for (who, (system, planet)) in ["b", "c"].into_iter().zip(candidates) {
        state.system_mut(&system).set_control(planet.clone(), pid(who));
        state.exhausted_planets.insert(planet);
    }
}

#[test]
fn every_card_kind_is_followed_and_declined_in_lockstep() {
    for name in KINDS {
        for answer in ["yes", "no"] {
            let mut state = dealt(&F3, name);
            prepare(&mut state);
            let (counts, bails, game) = play(state, answer);
            assert!(counts.0 >= 2, "{name} {answer}: every follower step was compared: {counts:?} {bails:?}");
            assert!(game.events.iter().any(|e| e == "STRATEGIC_ACTION_COMPLETE"), "{name} {answer}");
        }
    }
}

#[test]
fn followers_without_a_strategy_token_are_skipped_and_the_card_is_exhausted() {
    let mut state = dealt(&F3, "Warfare");
    for who in ["b", "c"] {
        state.player_mut(&pid(who)).unwrap().strategic_tokens = 0;
    }
    let (counts, _, game) = play(state, "yes");
    assert_eq!(counts, (1, 0), "one step: nobody is asked");
    let a = game.state.player(&pid("a")).unwrap();
    assert!(!a.exhausted_strategy_cards.is_empty(), "the card is spent");
    assert_eq!(game.events.iter().filter(|e| e.starts_with("STRATEGY_SECONDARY_")).count(), 0);
}

#[test]
fn a_follow_spends_the_token_and_announces_it() {
    let mut state = dealt(&F3, "Politics");
    state.player_mut(&pid("c")).unwrap().strategic_tokens = 0;
    let (counts, _, game) = play(state, "yes");
    assert!(counts.0 >= 2);
    assert_eq!(game.state.player(&pid("b")).unwrap().strategic_tokens, 2);
    assert!(game.events.iter().any(|e| e == "STRATEGY_SECONDARY_FOLLOWED"));
}

/// Acquiescence: Winnu's promissory note lets its holder follow without a token. The question carries the waiver
/// option (compared), a plain answer is compared, and taking the waiver is paid by the core step (the note goes home).
#[test]
fn a_waiver_is_offered_and_compared_and_taking_it_is_paid_in_lockstep() {
    let winnu = [("a", "winnu"), ("b", "sol"), ("c", "arborec")];
    for answer in ["yes", "no"] {
        let mut state = dealt(&winnu, "Trade");
        state.promissory_notes.insert(crate::factions::winnu::ACQUIESCENCE.to_owned(), pid("b"));
        prepare(&mut state);
        let (counts, bails, game) = play(state, answer);
        assert!(counts.0 >= 2, "{answer}: {counts:?}");
        assert!(bails.is_empty(), "{answer}: {bails:?}");
        assert_eq!(game.state.promissory_notes.get(crate::factions::winnu::ACQUIESCENCE), Some(&pid("b")), "{answer}: kept");
    }
    let mut state = dealt(&winnu, "Trade");
    state.promissory_notes.insert(crate::factions::winnu::ACQUIESCENCE.to_owned(), pid("b"));
    prepare(&mut state);
    let (counts, bails, game) = play(state, "follow|waived|");
    assert!(bails.is_empty(), "the waiver is paid by the core step: {bails:?}");
    assert!(counts.0 >= 2, "{counts:?}");
    assert_eq!(game.state.promissory_notes.get(crate::factions::winnu::ACQUIESCENCE), Some(&pid("a")), "the note went home");
    assert_eq!(game.state.player(&pid("b")).unwrap().strategic_tokens, 3, "no token was spent");
}

/// Jae Mir Kan: the Mahact follower exhausts the agent instead of a token; the primary player's token leaves the board.
#[test]
fn jae_mir_kan_is_paid_by_the_core_step_in_lockstep() {
    let mahact = [("a", "sol"), ("b", "mahact"), ("c", "arborec")];
    let system = ti4_model::id::SystemId::new("18");
    let setup = || {
        let mut state = dealt(&mahact, "Politics");
        prepare(&mut state);
        let agent = ti4_model::id::LeaderId::new(crate::factions::mahact::AGENT);
        state.player_mut(&pid("b")).unwrap().leaders.insert(agent, ti4_model::state::LeaderStatus::Readied);
        state.system_mut(&system).command_tokens.insert(pid("a"));
        state
    };
    let (counts, bails, game) = play(setup(), "follow|waived|");
    assert!(bails.is_empty(), "{bails:?}");
    assert!(counts.0 >= 2, "{counts:?}");
    let agent = ti4_model::id::LeaderId::new(crate::factions::mahact::AGENT);
    assert_eq!(game.state.player(&pid("b")).unwrap().leaders.get(&agent), Some(&ti4_model::state::LeaderStatus::Exhausted));
    assert!(!game.state.board.get(&system).is_some_and(|b| b.command_tokens.contains(&pid("a"))), "the token left the board");
    assert_eq!(game.state.player(&pid("b")).unwrap().strategic_tokens, 3, "no token was spent");
    // Declined or followed with a token: the agent stays ready (compared too).
    for answer in ["yes", "no"] {
        let (counts, bails, game) = play(setup(), answer);
        assert!(counts.0 >= 2 && bails.is_empty(), "{answer}: {counts:?} {bails:?}");
        assert_eq!(game.state.player(&pid("b")).unwrap().leaders.get(&agent), Some(&ti4_model::state::LeaderStatus::Readied));
    }
}

/// While the Nomad's suite watches agents exhaust, Jae Mir Kan's waiver is paid by the core step as well, which stages the `AGENT_EXHAUSTED` the string
/// stages (it was the counted bail "a waiver taken whose payment the core tables do not carry"): compared, no bail, the event written to the position.
#[test]
fn jae_mir_kan_while_the_nomad_watches_is_paid_by_the_core_step_and_the_nomad_is_told() {
    let table = [("a", "sol"), ("b", "mahact"), ("c", "nomad")];
    let system = ti4_model::id::SystemId::new("18");
    let mut state = dealt(&table, "Politics");
    prepare(&mut state);
    let agent = ti4_model::id::LeaderId::new(crate::factions::mahact::AGENT);
    state.player_mut(&pid("b")).unwrap().leaders.insert(agent.clone(), ti4_model::state::LeaderStatus::Readied);
    state.system_mut(&system).command_tokens.insert(pid("a"));
    state.player_mut(&pid("c")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("tcs"));
    assert!(crate::factions::nomad::watches_agent_exhaustion(&state), "the Temporal Command Suite is watching");
    let (counts, bails, game) = play(state, "follow|waived|");
    assert!(bails.is_empty(), "no bail: {bails:?}");
    assert!(counts.0 >= 2, "{counts:?}");
    assert_eq!(game.state.player(&pid("b")).unwrap().leaders.get(&agent), Some(&ti4_model::state::LeaderStatus::Exhausted), "the core paid");
}

/// Mindsieve: the Naalu follower hands the primary player a promissory note instead of a token.
#[test]
fn mindsieve_is_paid_by_the_core_step_in_lockstep() {
    let naalu = [("a", "sol"), ("b", "naalu"), ("c", "arborec")];
    let mut state = dealt(&naalu, "Politics");
    prepare(&mut state);
    state.player_mut(&pid("b")).unwrap().breakthrough = Some(ti4_model::id::BreakthroughId::new("naalubt"));
    let held = crate::promissory::held_by(&state, &pid("b"));
    assert!(!held.is_empty(), "the Naalu seat holds its own notes");
    let (counts, bails, game) = play(state, "follow|waived|");
    assert!(bails.is_empty(), "{bails:?}");
    assert!(counts.0 >= 2, "{counts:?}");
    let handed: Vec<_> = held.iter().filter(|n| game.state.promissory_notes.get(n.as_str()) == Some(&pid("a"))).collect();
    assert_eq!(handed.len(), 1, "exactly one note went to the primary player: {handed:?}");
}

/// A preview (the planning fork asks one follower in isolation) is not the step the core covers: it bails, and the
/// string answers.
#[test]
fn bail_a_preview_window_keeps_the_string_step() {
    let _serial = crate::dual::tests::serial();
    let _ = take_report();
    crate::dual::reset_thread_windows();
    let state = dealt(&F3, "Leadership");
    let card = state.player(&pid("a")).unwrap().strategy_cards[0].clone();
    with_mask(AreaMask::STRING.with(Area::Strategy, Backend::Dual), || {
        let mut game = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: "no" })));
        let mut preview = game.fork();
        preview.prepare_hypothetical_secondary(&pid("b"), &pid("a"), &card).unwrap();
        let step = preview.step();
        assert!(step.error.is_none() && step.resolved_choice);
    });
    let r = report();
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    assert!(r.windows.keys().any(|k| k.contains("strategy::secondary [bail: a preview window or a card nobody holds]")), "{:?}", r.windows);
    assert_eq!(crate::dual::thread_window_counts(LABEL).0, 0, "nothing was compared");
    let _ = take_report();
}
