//! D4A4: two second steps of action cards that run as child windows: Impersonation's secret objective over the hand limit and the Exploration
//! Probe finding a Merchant Station. (The first steps are in `action_card_effects3.rs`.)
//!
//! Printed text: Impersonation: "ACTION: Spend 3 influence to draw 1 secret objective." (Secret objective hand limit: 3, a player over it returns
//! 1 unscored secret to the deck.) Exploration Probe: "ACTION: Explore a frontier token adjacent to 1 of your ships." Merchant Station (frontier):
//! "Replenish your commodities or convert all of your commodities to trade goods."

use ti4_engine_core::choice::explore::Opt as EOpt;
use ti4_engine_core::containers::Deck;
use ti4_engine_core::generated::{ActionCard, Explore, ExploreType, Planet, SecretObjective};
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::query::tech::UnitRoster;
use ti4_engine_core::rules::action_cards::effects::Play;
use ti4_engine_core::rules::action_cards::{Env, Facts, Placed};
use ti4_engine_core::state::Code;
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn four() -> Scenario {
    Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol, Faction::TheWinnu]).map_ring(7)
}

fn linked() -> AbilityTable {
    let mut t = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut t);
    t
}

fn env() -> Env {
    let placed = |u: Unit| Placed { unit: Some(u), on_card: 0 };
    Env {
        mahact_held: 0,
        scope: ALL_SOURCES,
        blanked: false,
        action_card: Code(0),
        supply_base: Code(0),
        note_order: |a, b| a.cmp(&b),
        infantry: placed(Unit::InfantryI),
        cruiser: placed(Unit::CruiserI),
        fighter: placed(Unit::FighterI),
        facts: Facts { neutral: Some(Seat(8)), action_draw_plain: true, rules_version: 35, roster: Some(Box::leak(Box::new(UnitRoster::default()))), ..Facts::default() },
    }
}

fn rig(card: ActionCard, st: CoreState) -> Rig<Play> {
    let play = Play::of(&st, card, Seat(0), env()).unwrap_or_else(|| panic!("{card:?} has a play"));
    Rig::from_parts(st, kernel_all(), play).with_table(linked()).seeded(1)
}

#[test]
fn impersonation_over_the_secret_hand_limit_asks_which_unscored_secret_goes_back_to_the_deck() {
    covers_card!(ActionCard::Impersonation);
    let st = || {
        let mut s = four().control(Planet::Torkan, 0).state();
        s.decks_mut().secret = Deck::new(vec![SecretObjective::ALL[9], SecretObjective::ALL[10]]);
        for i in 0..3 {
            s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.secrets.push(SecretObjective::ALL[i]);
        }
        s
    };
    let mut r = rig(ActionCard::Impersonation, st());
    r.run();
    assert!(!r.is_done(), "four secrets: over the limit of three");
    assert_eq!(r.asked().actor, Seat(0));
    assert_eq!(r.opts().len(), 4, "every unscored secret in hand");
    r.assert_options_sound();
    assert!(r.stages.iter().any(|s| s == "Limiting"), "{:?}", r.stages);
    r.pick_where(|o| matches!(o, Opt::Objectives(_)));
    assert!(r.is_done());
    let held = r.state.players().by_seat(Seat(0)).unwrap().cards.secrets.as_slice().to_vec();
    assert_eq!(held.len(), 3);
    assert_eq!(held, vec![SecretObjective::ALL[1], SecretObjective::ALL[2], SecretObjective::ALL[9]], "the first option (the first secret) went back, the drawn one stays");
    assert_eq!(r.state.decks().secret.cards().len(), 2, "one drawn, one returned to the bottom: the deck is back to two");
    smoke("action_card_probe_limit/impersonation", || rig(ActionCard::Impersonation, st()));
}

#[test]
fn the_probe_finding_a_merchant_station_asks_replenish_or_convert() {
    covers_card!(ActionCard::ExplorationProbe);
    let sys1 = plain_systems()[1];
    let st = |commodities: i16| {
        let mut s = four().state();
        s.board_mut().sys_mut(sys1).units.push(ti4_engine_core::state::Piece::new(Unit::CruiserI, Seat(0)));
        s.explore_mut().frontier.insert(sys1);
        s.decks_mut().explore[ExploreType::Frontier as usize] = Deck::new(vec![Explore::MerchantStation1]);
        s.decks_mut().explore_present |= 1 << ExploreType::Frontier as u8;
        s.meta_mut().active_system = Some(sys1);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.commodities = commodities;
        s
    };
    let mut r = rig(ActionCard::ExplorationProbe, st(1));
    r.run();
    assert!(!r.is_done(), "a Merchant Station asks which of its two rewards");
    assert_eq!(r.asked().actor, Seat(0));
    assert_eq!(r.opts().len(), 2, "replenish or convert: {:?}", r.opts());
    r.assert_options_sound();
    assert!(r.stages.iter().any(|s| s == "Rewarding"), "{:?}", r.stages);
    r.pick_where(|o| matches!(o, Opt::Explore(EOpt::Convert)));
    assert!(r.is_done());
    assert_eq!(r.state.players().by_seat(Seat(0)).unwrap().econ.commodities, 0, "all commodities converted");
    assert_eq!(r.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 1, "into trade goods");
    smoke("action_card_probe_limit/probe_merchant", || rig(ActionCard::ExplorationProbe, st(1)));
    let mut rep = rig(ActionCard::ExplorationProbe, st(0));
    rep.run();
    if !rep.is_done() {
        rep.pick_where(|o| matches!(o, Opt::Explore(EOpt::Replenish)));
    }
    assert!(rep.is_done());
    assert_eq!(rep.state.players().by_seat(Seat(0)).unwrap().econ.commodities, i16::from(Faction::TheArborec.data().commodities), "replenished to the faction's value");
}
