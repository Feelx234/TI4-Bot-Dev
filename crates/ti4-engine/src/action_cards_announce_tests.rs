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
