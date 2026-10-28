//! Lockstep tests of the announce/resolve flow of a played card (package B6): the component action's path
//! (`reactions::announce` as one core window), a card played from the reaction slot, and a card declared in a window
//! and announced when the window closes. Each compares the string run with the core window through
//! `Area::ActionCards` on `Dual`; a bail is a failure unless the test is about the bail.
#![cfg(all(test, feature = "typed-core"))]

use ti4_content::ContentStore;
use ti4_model::id::{ActionCardId, PlayerId};
use ti4_model::state::GameState;

use crate::choice::{Choice, ChoiceOption, Decider, IllegalChoice, Table};
use crate::dual::{AreaMask, Backend, with_mask};
use crate::action_cards::tests::question_position;

/// Takes a reaction slot whenever one is offered and the last option of every other question (a first-option
/// default would hide an order difference).
pub(crate) struct PlaySlots;

impl Decider for PlaySlots {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        if let Some(slot) = choice.options.iter().find(|o| o.id.starts_with("reaction:")) {
            return Ok(slot.clone());
        }
        choice.options.last().cloned().ok_or_else(|| IllegalChoice::NoOptions { player: choice.player.clone(), prompt: choice.prompt.clone() })
    }
}

/// Declines every slot and takes the last option of every other question.
pub(crate) struct DeclineSlots;

impl Decider for DeclineSlots {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        if let Some(decline) = choice.options.iter().find(|o| o.is_decline()) {
            return Ok(decline.clone());
        }
        PlaySlots.choose(choice)
    }
}

/// Announce `alias` as played by `player` with the resolver armed for the table; whether the play stood.
pub(crate) fn announce(state: &mut GameState, alias: &str, player: &PlayerId, decider: Box<dyn Decider>) -> bool {
    let mut table = Table::with_default(decider);
    let mut dice = crate::dice::Dice::new();
    let mut rng = crate::rng::GameRng::new(0);
    let mut sequence = crate::event::EventSequence::new();
    let mut resolver = crate::fixtures::armed_resolver(state);
    let mut context = crate::timing::TimingContext {
        state,
        content: ContentStore::embedded(),
        sources: ti4_model::content_types::POK,
        table: &mut table,
        dice: &mut dice,
        rng: &mut rng,
        event_sequence: &mut sequence,
        galaxy: None,
    };
    crate::reactions::announce(&mut context, &mut resolver, player, &ActionCardId::new(alias)).expect("announced")
}

const MASK: AreaMask = AreaMask::STRING;

fn dual() -> AreaMask {
    MASK.with(crate::dual::Area::ActionCards, Backend::Dual)
}

fn windows(label: &str) -> (u64, u64) {
    let r = crate::dual::report();
    let w = r.windows.get(label).cloned().unwrap_or_default();
    (w.compared, w.bailed)
}

/// The component action's path: the card is announced, resolves in the core window and is discarded; the windows
/// around the two events have nothing to offer.
#[test]
fn a_played_card_is_announced_resolved_and_discarded_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let a = PlayerId::new("a");
    let cases: &[&str] = &[
        "mb1", "coup", "rally", "insub", "assassin", "imp_rider", "tech_rider", "sanction", "unexpected", "mining_initiative",
        "uprising", "cripple", "meltdown", "veto", "industrial_initiative", "silence_space", "dp1", "tactical", "messiah",
        "f_deployment", "war_effort", "insider", "sabo1",
    ];
    let _ = crate::dual::take_report();
    for alias in cases {
        let mut state = question_position();
        state.player_mut(&a).unwrap().action_cards = vec![ActionCardId::new(*alias)];
        let before = state.clone();
        // `perform` spends the card before it announces.
        let index = 0;
        crate::action_cards::discard(&mut state, &a, index);
        with_mask(dual(), || announce(&mut state, alias, &a, Box::new(DeclineSlots)));
        assert_eq!(state.discarded_action_cards.last(), Some(&ActionCardId::new(*alias)), "{alias} is on the pile");
        assert!(serde_json::to_string(&state).unwrap() != serde_json::to_string(&before).unwrap(), "{alias} changed nothing");
    }
    let report = crate::dual::report();
    let shown: String = format!("{:?}", report.differences).chars().take(1500).collect();
    assert!(report.differences.is_empty(), "{} differences: {shown}", report.differences.len());
    assert_eq!(windows("action_cards::announce"), (cases.len() as u64, 0), "{:?}", report.windows);
    let _ = crate::dual::take_report();
}

/// A Sabotage played from the reaction slot of `ACTION_CARD_PLAYED`: it is spent, announced (its own windows open),
/// and cancels the card being played, whose effect never runs while its spend stands.
#[test]
fn a_sabotage_played_from_the_slot_cancels_the_play_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
    let _ = crate::dual::take_report();
    let mut state = question_position();
    state.player_mut(&b).unwrap().action_cards = vec![ActionCardId::new("sabo1")];
    state.player_mut(&a).unwrap().action_cards = vec![ActionCardId::new("insub")];
    crate::action_cards::discard(&mut state, &a, 0);
    let tokens = state.player(&b).unwrap().tactic_tokens;
    let stood = with_mask(dual(), || announce(&mut state, "insub", &a, Box::new(PlaySlots)));
    assert!(!stood, "the play was cancelled");
    assert_eq!(state.player(&b).unwrap().tactic_tokens, tokens, "Insubordination never ran");
    assert!(state.player(&b).unwrap().action_cards.is_empty(), "Sabotage was spent");
    assert_eq!(state.discarded_action_cards.len(), 2, "both cards are on the pile");
    let report = crate::dual::report();
    let shown: String = format!("{:?}", report.differences).chars().take(1500).collect();
    assert!(report.differences.is_empty(), "{} differences: {shown}", report.differences.len());
    assert_eq!(windows("action_cards::announce"), (1, 0), "{:?}", report.windows);
    let _ = crate::dual::take_report();
}

/// Reverse Engineer is played into the window after a component-action card was discarded: from rules version 3 it
/// is only declared there (spent), and announced when the window closes, taking the discarded card.
#[test]
fn a_card_declared_in_a_window_is_announced_at_its_close_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
    let _ = crate::dual::take_report();
    let mut state = question_position();
    assert!(state.rules_version >= ti4_model::state::RULES_SABOTAGE_TIMING);
    state.player_mut(&b).unwrap().action_cards = vec![ActionCardId::new("reverse_engineer")];
    state.player_mut(&a).unwrap().action_cards = vec![ActionCardId::new("industrial_initiative")];
    crate::action_cards::discard(&mut state, &a, 0);
    let stood = with_mask(dual(), || announce(&mut state, "industrial_initiative", &a, Box::new(PlaySlots)));
    assert!(stood);
    assert_eq!(state.player(&b).unwrap().action_cards, vec![ActionCardId::new("industrial_initiative")], "Reverse Engineer took the card");
    assert_eq!(state.discarded_action_cards.last(), Some(&ActionCardId::new("reverse_engineer")));
    let report = crate::dual::report();
    let shown: String = format!("{:?}", report.differences).chars().take(1500).collect();
    assert!(report.differences.is_empty(), "{} differences: {shown}", report.differences.len());
    assert_eq!(windows("action_cards::announce"), (1, 0), "{:?}", report.windows);
    let _ = crate::dual::take_report();
}

/// Takes the first card of the "play an action card" question (the slot's second question) and a slot whenever
/// one is offered, the last option of everything else.
pub(crate) struct FirstCard;

impl Decider for FirstCard {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        if choice.prompt.starts_with("play an action card") {
            return Ok(choice.options[0].clone());
        }
        PlaySlots.choose(choice)
    }
}

/// Fights in system 18 (seeds 0..6) with the given hands, the slots played by `decider`; the differences, the cards
/// that ended on the pile and the report's window counts.
fn fights(a_hand: &[&str], b_hand: &[&str], rules_version: Option<u32>, decider: &dyn Fn() -> Box<dyn Decider>) -> (u64, usize, std::collections::BTreeMap<String, crate::dual::WindowCounts>) {
    use crate::dual::{report, take_report, Area};
    use crate::choice::Resolving;
    let _ = take_report();
    let mut played = 0;
    for seed in 0..6_u64 {
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan"), ("c", "xxcha")], ti4_model::content_types::DEFAULT);
        state.promissory_notes.clear();
        state.promissory_faceup.clear();
        state.rng_seed = seed;
        if let Some(v) = rules_version {
            state.rules_version = v;
        }
        let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
        let system = ti4_model::id::SystemId::new("18");
        crate::fixtures::put(&mut state, &system, "cruiser", &a, 3);
        crate::fixtures::put(&mut state, &system, "destroyer", &b, 2);
        crate::fixtures::put(&mut state, &system, "fighter", &b, 3);
        state.player_mut(&a).unwrap().action_cards = a_hand.iter().map(|x| ActionCardId::new(*x)).collect();
        state.player_mut(&b).unwrap().action_cards = b_hand.iter().map(|x| ActionCardId::new(*x)).collect();
        with_mask(MASK.with(Area::Driver, Backend::Dual).with(Area::ActionCards, Backend::Dual).with(Area::Combat, Backend::Dual), || {
            let mut table = Table::with_default(decider());
            let (mut dice, mut rng) = (crate::dice::Dice::new(), crate::rng::GameRng::new(seed));
            let mut resolver = crate::fixtures::armed_resolver(&state);
            let mut sequence = crate::event::EventSequence::new();
            let mut ctx = Resolving {
                content: ContentStore::embedded(),
                sources: ti4_model::content_types::DEFAULT,
                dice: &mut dice,
                rng: &mut rng,
                table: &mut table,
                timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
            };
            crate::combat::resolve_resolving(&mut state, &mut ctx, &system, None).unwrap();
        });
        played += state.discarded_action_cards.len();
    }
    let r = report();
    let shown: String = format!("{:?}", r.differences).chars().take(1500).collect();
    assert_eq!(r.total_differences(), 0, "{shown}");
    let slot_bails: Vec<&String> = r.windows.keys().filter(|k| k.contains("unported ability reaction:")).collect();
    assert!(slot_bails.is_empty(), "{slot_bails:?}");
    let out = (r.total_differences(), played, r.windows.clone());
    let _ = take_report();
    out
}

fn compared(w: &std::collections::BTreeMap<String, crate::dual::WindowCounts>, label: &str) -> u64 {
    w.get(label).map_or(0, |c| c.compared)
}

/// A fight whose seats hold reaction cards the core plays (Morale Boost into the start of a combat round): the
/// combat windows follow the plays through the ability table and compare them, instead of bailing on the slot.
#[test]
fn a_reaction_card_played_in_a_fight_is_followed_by_the_combat_windows() {
    let _serial = crate::dual::tests::serial();
    let (_, played, windows) = fights(&["mb1"], &["mb2", "rally"], None, &|| Box::new(PlaySlots));
    assert!(played > 0, "cards were played");
    assert!(compared(&windows, "combat::begin_round") > 0, "{windows:?}");
}

/// The window is offered again after a card was declared in it: the second offer names the cards held at that moment
/// (a regression of the first port, which rendered the offer from the state the driver had rolled back to).
#[test]
fn a_second_offer_in_the_same_window_names_the_cards_held_at_that_moment() {
    let _serial = crate::dual::tests::serial();
    let (_, played, windows) = fights(&[], &["mb2", "f_prototype", "mb3"], None, &|| Box::new(FirstCard));
    assert!(played > 0, "cards were played");
    assert!(compared(&windows, "combat::begin_round") > 0, "{windows:?}");
}

/// Before rules version 3 a card played into an ordinary window is announced at once, not declared.
#[test]
fn before_rules_version_three_a_card_played_into_a_window_is_announced_at_once() {
    let _serial = crate::dual::tests::serial();
    // A second card of the same seat is offered after the first: with an immediate play the Sabotage window of the first
    // (b holds one) is asked before that second offer, with a declared play after the window closes.
    let (_, played, windows) = fights(&["mb1", "mb2"], &["sabo1"], Some(2), &|| Box::new(PlaySlots));
    assert!(played > 0, "cards were played");
    assert!(compared(&windows, "combat::begin_round") > 0, "{windows:?}");
}

/// Bail: a Data Skimmer in play could take the discard (a breakthrough's pile, not the core's): the string
/// announcement stands, counted under its reason.
#[test]
fn bail_a_discard_the_data_skimmer_could_take_keeps_the_string_announcement() {
    let _serial = crate::dual::tests::serial();
    let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
    let _ = crate::dual::take_report();
    let mut state = question_position();
    state.player_mut(&b).unwrap().breakthrough = Some(ti4_model::id::BreakthroughId::new(crate::factions::ralnel_cards::BREAKTHROUGH));
    state.player_mut(&a).unwrap().action_cards = vec![ActionCardId::new("coup")];
    crate::action_cards::discard(&mut state, &a, 0);
    with_mask(dual(), || announce(&mut state, "coup", &a, Box::new(DeclineSlots)));
    let report = crate::dual::report();
    assert_eq!(report.total_differences(), 0, "{:?}", report.differences);
    assert!(
        report.windows.keys().any(|k| k.contains("action_cards::announce [bail: a discard the Data Skimmer could take]")),
        "{:?}",
        report.windows
    );
    assert_eq!(windows("action_cards::announce"), (0, 1));
    let _ = crate::dual::take_report();
}

/// Bail: a card whose effect the core does not play keeps the string announcement (counted as a bailed window).
#[test]
fn bail_a_card_with_an_unported_effect_keeps_the_string_announcement() {
    let _serial = crate::dual::tests::serial();
    let a = PlayerId::new("a");
    let _ = crate::dual::take_report();
    let mut state = question_position();
    state.player_mut(&a).unwrap().action_cards = vec![ActionCardId::new("dh1")];
    crate::action_cards::discard(&mut state, &a, 0);
    with_mask(dual(), || announce(&mut state, "dh1", &a, Box::new(DeclineSlots)));
    let report = crate::dual::report();
    assert_eq!(report.total_differences(), 0, "{:?}", report.differences);
    assert_eq!(windows("action_cards::announce"), (0, 1));
    let _ = crate::dual::take_report();
}

/// Bail: a card the core does not play (Skilled Retreat) played from the slot of a fight: the combat window that
/// followed the offer bails with the reason, instead of comparing a play the core cannot make.
#[test]
fn bail_an_unported_card_played_in_a_fight_bails_the_window_with_its_reason() {
    let _serial = crate::dual::tests::serial();
    let (_, played, windows) = fights(&["s_retreat1"], &[], None, &|| Box::new(PlaySlots));
    assert!(played > 0, "the card was played");
    assert!(
        windows.keys().any(|k| k.starts_with("combat::begin_round [bail: nested (an action card the core does not play)]")),
        "{windows:?}"
    );
}

fn unit(kind: &str, owner: &PlayerId) -> ti4_model::units::Unit {
    ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new(kind), owner.clone())
}

/// Play `alias` as the component action of `a` in `state` under the dual, the last option answering every question.
fn played(mut state: GameState, alias: &str) -> GameState {
    let a = PlayerId::new("a");
    state.player_mut(&a).unwrap().action_cards = vec![ActionCardId::new(alias)];
    crate::action_cards::discard(&mut state, &a, 0);
    let stood = with_mask(dual(), || announce(&mut state, alias, &a, Box::new(DeclineSlots)));
    assert!(stood, "{alias} stood");
    state
}

fn assert_compared_clean(label_count: u64) {
    let report = crate::dual::report();
    let shown: String = format!("{:?}", report.differences).chars().take(1500).collect();
    assert!(report.differences.is_empty(), "{} differences: {shown}", report.differences.len());
    assert_eq!(windows("action_cards::announce"), (label_count, 0), "{:?}", report.windows);
}

/// Intercept: the other combatants of the active system may not retreat this combat round.
#[test]
fn intercept_bars_the_retreat_of_the_other_combatants_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let (a, b, c) = (PlayerId::new("a"), PlayerId::new("b"), PlayerId::new("c"));
    let _ = crate::dual::take_report();
    let mut state = question_position();
    let sys = ti4_model::id::SystemId::new("33");
    state.combat_round_seq = 3;
    state.active_system = Some(sys.clone());
    state.system_mut(&sys).units.push(unit("cruiser", &b));
    let after = played(state.clone(), "intercept");
    assert_eq!(after.player(&b).unwrap().retreat_barred_round, Some(3));
    assert_eq!(after.player(&c).unwrap().retreat_barred_round, None, "c has no ship there");
    assert_eq!(after.player(&a).unwrap().retreat_barred_round, None, "the holder is not barred");
    // Not a combatant: the holder has no ship in the active system.
    state.system_mut(&sys).units.retain(|u| u.owner != a);
    let after = played(state, "intercept");
    assert_eq!(after.player(&b).unwrap().retreat_barred_round, None);
    assert_compared_clean(2);
    let _ = crate::dual::take_report();
}

/// Bail: Intercept at a table with a Naaz or Nekro seat (units that fight from a planet are the combat rules').
#[test]
fn bail_intercept_at_a_table_with_a_nekro_seat_keeps_the_string_announcement() {
    let _serial = crate::dual::tests::serial();
    let _ = crate::dual::take_report();
    let mut state = question_position();
    state.players[2].faction = ti4_model::id::FactionId::new("nekro");
    state.active_system = Some(ti4_model::id::SystemId::new("33"));
    let _ = played(state, "intercept");
    assert_eq!(windows("action_cards::announce"), (0, 1));
    let _ = crate::dual::take_report();
}

/// Rescue: bring a ship from a system without the holder's token into the system another player activated.
#[test]
fn rescue_moves_a_ship_into_the_active_system_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
    let _ = crate::dual::take_report();
    let mut state = question_position();
    let (dest, from_a, from_b) = (ti4_model::id::SystemId::new("33"), ti4_model::id::SystemId::new("34"), ti4_model::id::SystemId::new("35"));
    state.active = Some(b.clone());
    state.active_system = Some(dest.clone());
    for sys in [&dest, &from_a, &from_b] {
        state.system_mut(sys).command_tokens.remove(&a);
    }
    let after = played(state.clone(), "rescue");
    // Two origins: the question is asked and the last option (35) answers.
    assert_eq!(after.system_state(&from_b).units.iter().filter(|u| u.owner == a).count(), 0);
    assert_eq!(after.system_state(&dest).units.iter().filter(|u| u.owner == a).count(), 2);
    assert_eq!(after.system_state(&from_a).units.iter().filter(|u| u.owner == a).count(), 1);
    // One origin only (34 holds the holder's token): nothing is asked, the ship comes.
    let mut one = state.clone();
    one.system_mut(&from_a).command_tokens.insert(a.clone());
    let after = played(one, "rescue");
    assert_eq!(after.system_state(&dest).units.iter().filter(|u| u.owner == a).count(), 2);
    // The holder has no ship in the active system: nothing to rescue into.
    let mut empty = state.clone();
    empty.active_system = Some(ti4_model::id::SystemId::new("36"));
    let after = played(empty.clone(), "rescue");
    assert_eq!(after.system_state(&from_b).units.iter().filter(|u| u.owner == a).count(), 1);
    assert_eq!(after.system_state(&ti4_model::id::SystemId::new("36")).units.len(), empty.system_state(&ti4_model::id::SystemId::new("36")).units.len());
    // The holder activated: the window is not theirs.
    let mut own = state;
    own.active = Some(a.clone());
    let after = played(own.clone(), "rescue");
    assert_eq!(serde_json::to_string(&after.system_state(&dest)).unwrap(), serde_json::to_string(&own.system_state(&dest)).unwrap());
    assert_compared_clean(4);
    let _ = crate::dual::take_report();
}

/// Fighter Conscription: a fighter in each system with a space dock or a ship with capacity and no foreign ship.
#[test]
fn fighter_conscription_places_fighters_where_a_dock_or_a_carrier_stands_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
    let _ = crate::dual::take_report();
    let mut state = question_position();
    // Carriers of the holder in 33 and 35, a foreign ship in 35 bars it.
    state.system_mut(&ti4_model::id::SystemId::new("33")).units.push(unit("carrier", &a));
    state.system_mut(&ti4_model::id::SystemId::new("35")).units.push(unit("carrier", &a));
    state.system_mut(&ti4_model::id::SystemId::new("35")).units.push(unit("cruiser", &b));
    let fighters = |s: &GameState, sys: &str| s.system_state(&ti4_model::id::SystemId::new(sys)).units.iter().filter(|u| u.owner == a && u.type_id.as_str() == "fighter").count();
    let after = played(state.clone(), "f_conscription");
    assert_eq!(fighters(&after, "33"), 1);
    assert_eq!(fighters(&after, "35"), 0, "a foreign ship is there");
    // The supply holds two fighters only (the cap is 10 of the plastic): the first system takes the last one.
    let mut tight = state;
    tight.system_mut(&ti4_model::id::SystemId::new("34")).units.extend((0..9).map(|_| unit("fighter", &a)));
    tight.system_mut(&ti4_model::id::SystemId::new("34")).units.push(unit("carrier", &a));
    let after = played(tight, "f_conscription");
    assert_eq!(fighters(&after, "33"), 1, "the tenth fighter");
    assert_eq!(fighters(&after, "34"), 9, "the supply is empty: no eleventh");
    // A space dock alone is enough: a placed planet with the holder's dock, no ship in orbit.
    let (system, planet) = crate::fixtures::a_placed_planet();
    let mut docked = question_position();
    crate::fixtures::put_on_planet(&mut docked, &system, &planet, "spacedock", &a, 1);
    docked.system_mut(&system).units.retain(|u| u.owner != a);
    let after = played(docked, "f_conscription");
    assert_eq!(after.system_state(&system).units.iter().filter(|u| u.owner == a && u.type_id.as_str() == "fighter").count(), 1);
    assert_compared_clean(3);
    let _ = crate::dual::take_report();
}
