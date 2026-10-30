//! D4-2b: `rules::relic_windows` (Neuraloop, the Codex, the Titan Prototype, the Stellar Converter, the technology of the Enigmatic Device and the Maw).

use ti4_engine_core::choice::relic::Opt as ROpt;
use ti4_engine_core::generated::{ActionCard, PublicObjective, Relic, Technology as T};
use ti4_engine_core::rules::relic_windows::*;
use ti4_engine_core::state::PlanetFlags;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn table() -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec, Faction::TheYinBrotherhood]).map_ring(7)
}

fn r(o: ROpt) -> Opt {
    Opt::Relic(o)
}

// ---- Neuraloop --------------------------------------------------------------------------------

fn neuraloop_state(relics: &[Relic]) -> CoreState {
    let mut s = table().state();
    for x in relics {
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.relics.push(*x);
    }
    s
}

#[test]
fn the_neuraloop_offers_each_relic_in_hand_to_purge_and_a_decline() {
    covers!("rules::relic_windows::Neuraloop");
    let w = || Neuraloop::new(Seat(0), PublicObjective::ALL[0]);
    let mut rig = Rig::from_parts(neuraloop_state(&[Relic::TheCodex, Relic::TheTriad]), ti4_engine_core_tests::kernel_all(), w());
    rig.run();
    assert_eq!(rig.opts(), vec![r(ROpt::PurgeRelic(Relic::TheCodex)), r(ROpt::PurgeRelic(Relic::TheTriad)), Opt::Decline]);
    rig.expect_illegal(Pick::Opt(r(ROpt::PurgeRelic(Relic::MawOfWorlds))));
    rig.pick(1);
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.chosen, Some(Relic::TheTriad));
    let mut decl = Rig::from_parts(neuraloop_state(&[Relic::TheCodex]), ti4_engine_core_tests::kernel_all(), w());
    decl.run().decline();
    assert!(decl.is_done() && decl.driver.window.chosen.is_none());
    let mut none = Rig::from_parts(neuraloop_state(&[]), ti4_engine_core_tests::kernel_all(), w());
    none.run();
    assert!(none.is_done(), "no relic, no question");
    smoke_quiet(|| Rig::from_parts(neuraloop_state(&[Relic::TheCodex, Relic::TheTriad]), ti4_engine_core_tests::kernel_all(), w()));
}

// ---- Codex ------------------------------------------------------------------------------------

fn codex_state(discards: &[ActionCard]) -> CoreState {
    let mut s = table().state();
    s.cards_mut().discarded_action.edit().extend(discards.iter().copied());
    s
}

#[test]
fn the_codex_takes_up_to_three_cards_from_the_discard_pile_one_at_a_time() {
    covers!("rules::relic_windows::Codex");
    let pile = [ActionCard::ALL[0], ActionCard::ALL[1], ActionCard::ALL[2], ActionCard::ALL[3], ActionCard::ALL[4]];
    let mut rig = Rig::from_parts(codex_state(&pile), ti4_engine_core_tests::kernel_all(), Codex::new(Seat(0)));
    rig.run();
    assert_eq!(rig.asked().opts.len(), 6, "five cards and a decline");
    rig.assert_options_sound();
    rig.pick_where(|o| matches!(o, Opt::Relic(ROpt::Take(c, _)) if *c == pile[2]));
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice(), &[pile[2]]);
    assert_eq!(rig.asked().opts.len(), 5, "the card taken is gone from the pile");
    rig.pick(0);
    rig.pick(0);
    assert!(rig.is_done(), "three cards and it ends by itself");
    assert_eq!(rig.driver.window.taken, 3);
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 3);
    assert_eq!(rig.state.cards().discarded_action.len(), 2);
}

#[test]
fn the_codex_may_stop_early_and_an_empty_pile_asks_nothing() {
    covers!("rules::relic_windows::Codex");
    let mut rig = Rig::from_parts(codex_state(&[ActionCard::ALL[0]]), ti4_engine_core_tests::kernel_all(), Codex::new(Seat(0)));
    rig.run().decline();
    assert!(rig.is_done() && rig.driver.window.taken == 0);
    let mut one = Rig::from_parts(codex_state(&[ActionCard::ALL[0]]), ti4_engine_core_tests::kernel_all(), Codex::new(Seat(0)));
    one.run().pick(0);
    assert!(one.is_done(), "the pile ran out");
    let mut empty = Rig::from_parts(codex_state(&[]), ti4_engine_core_tests::kernel_all(), Codex::new(Seat(0)));
    empty.run();
    assert!(empty.is_done());
    let mut c = Rig::from_parts(codex_state(&[ActionCard::ALL[0]]), ti4_engine_core_tests::kernel_all(), Codex::new(Seat(0)));
    c.run();
    c.expect_illegal(Pick::Opt(r(ROpt::Take(ActionCard::ALL[9], Default::default()))));
    smoke("relic_windows/codex", || Rig::from_parts(codex_state(&[ActionCard::ALL[0], ActionCard::ALL[1]]), ti4_engine_core_tests::kernel_all(), Codex::new(Seat(0))));
}

// ---- Titan Prototype --------------------------------------------------------------------------

#[test]
fn the_titan_prototype_names_a_builder_among_all_players() {
    covers!("rules::relic_windows::TitanPrototype");
    let mut rig = Rig::new(table(), TitanPrototype::new(Seat(0)));
    rig.run();
    assert_eq!(rig.opts(), vec![r(ROpt::Builder(Seat(0))), r(ROpt::Builder(Seat(1))), r(ROpt::Builder(Seat(2)))]);
    rig.expect_illegal(Pick::Opt(r(ROpt::Builder(Seat(5)))));
    rig.pick(2);
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.chosen, Some(Seat(2)));
    assert_eq!(delta(rig.start_state(), &rig.state), "");
    smoke_quiet(|| Rig::new(table(), TitanPrototype::new(Seat(0))));
}

// ---- Stellar Converter ------------------------------------------------------------------------

#[test]
fn the_stellar_converter_purges_the_planet_chosen_with_everything_on_it() {
    covers!("rules::relic_windows::StellarConverter");
    let s = plain_systems();
    let (p0, p1) = (s[1].data().planets[0], s[2].data().planets[0]);
    let sc = || table().control(p0, 1).unit(s[1], Some(p0), Unit::InfantryI, 1, 2).control(p1, 2);
    let w = || StellarConverter::new(Seat(0), &[(s[1], p0), (s[2], p1)]);
    let mut rig = Rig::new(sc(), w());
    rig.run();
    assert_eq!(rig.asked().opts.len(), 2, "no decline: the action is committed");
    rig.pick(0);
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.destroyed, Some(p0));
    let cell = rig.state.board().planet(p0);
    assert!(cell.flags.has(PlanetFlags::PURGED) && cell.control.is_none() && cell.units.is_empty(), "purged: no control, no units");
    assert_eq!(rig.state.board().planet(p1).control, Some(Seat(2)), "the other planet is untouched");
    let mut bad = Rig::new(sc(), w());
    bad.run();
    bad.expect_illegal(Pick::Opt(r(ROpt::Destroy(s[3].data().planets[0], Default::default()))));
    let mut none = Rig::new(sc(), StellarConverter::new(Seat(0), &[]));
    none.run();
    assert!(none.is_done());
    smoke("relic_windows/stellar_converter", || Rig::new(sc(), w()));
}

// ---- GainTechnology ---------------------------------------------------------------------------

#[test]
fn the_enigmatic_device_and_the_maw_offer_their_technologies_and_the_choice_is_recorded() {
    covers!("rules::relic_windows::GainTechnology");
    let opts = [T::GravityDrive, T::AntimassDeflectors];
    for relic in [Relic::EnigmaticDevice, Relic::MawOfWorlds] {
        let mut rig = Rig::new(table(), GainTechnology::new(Seat(0), relic, &opts));
        rig.run();
        assert_eq!(rig.opts(), vec![r(ROpt::Tech(T::GravityDrive)), r(ROpt::Tech(T::AntimassDeflectors))]);
        rig.expect_illegal(Pick::Opt(r(ROpt::Tech(T::AssaultCannon))));
        rig.pick(1);
        assert!(rig.is_done());
        assert_eq!(rig.driver.window.chosen, Some(T::AntimassDeflectors));
        assert_eq!(delta(rig.start_state(), &rig.state), "", "the grant itself is the caller's");
    }
    let mut none = Rig::new(table(), GainTechnology::new(Seat(0), Relic::MawOfWorlds, &[]));
    none.run();
    assert!(none.is_done() && none.driver.window.chosen.is_none());
    smoke_quiet(|| Rig::new(table(), GainTechnology::new(Seat(0), Relic::EnigmaticDevice, &opts)));
}
