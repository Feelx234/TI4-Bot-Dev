//! D4-1 low layer: `query/{relic,promissory,leaders}`.

use ti4_engine_core::generated::{ExploreType, Faction, Leader, LeaderType, PromissoryNote, Relic};
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::query::promissory::{self, RULES_FACTION_EFFECTS};
use ti4_engine_core::query::relic::{self, UseArm};
use ti4_engine_core::query::leaders;
use ti4_engine_core::state::{FragmentCounts, LeaderStatus, NoteKey};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn table() -> Scenario {
    Scenario::new(&[Faction::TheMahactGeneSorcerers, Faction::TheArborec, Faction::TheBaronyOfLetnev])
}

fn give_relic(s: &mut CoreState, seat: u8, r: Relic) {
    s.players_mut().by_seat_mut(Seat(seat)).unwrap().cards.relics.push(r);
}

// ---- relics -----------------------------------------------------------------------------------

#[test]
fn holding_and_readiness_of_a_relic() {
    let mut s = table().state();
    assert!(!relic::holds(&s, Seat(0), Relic::TheCodex));
    give_relic(&mut s, 0, Relic::TheCodex);
    assert!(relic::holds(&s, Seat(0), Relic::TheCodex) && relic::ready(&s, Seat(0), Relic::TheCodex));
    assert!(!relic::holds(&s, Seat(1), Relic::TheCodex) && !relic::holds(&s, Seat(9), Relic::TheCodex));
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.relics_exhausted.insert(Relic::TheCodex);
    assert!(relic::holds(&s, Seat(0), Relic::TheCodex) && !relic::ready(&s, Seat(0), Relic::TheCodex));
    assert!(!relic::ready(&s, Seat(1), Relic::TheCodex));
}

#[test]
fn the_triad_is_worth_three_plus_the_kinds_of_fragment_held() {
    let mut s = table().state();
    assert_eq!(relic::triad_value(&s, Seat(0)), None, "not held");
    give_relic(&mut s, 0, Relic::TheTriad);
    assert_eq!(relic::triad_value(&s, Seat(0)), Some(3));
    let frags = &mut s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.fragments;
    frags.set(ExploreType::Cultural, 2);
    frags.set(ExploreType::Hazardous, 1);
    frags.set(ExploreType::Frontier, 0);
    assert_eq!(relic::fragment_kinds(&s.players().by_seat(Seat(0)).unwrap().cards.fragments), 2, "a kind with none counts nothing");
    assert_eq!(relic::triad_value(&s, Seat(0)), Some(5));
    assert_eq!(relic::fragment_kinds(&FragmentCounts::default()), 0);
}

#[test]
fn the_passive_relics_answer_for_their_holder_only() {
    let mut s = table().state();
    for r in [Relic::ScepterOfEmelpar, Relic::MetaliVoidShielding, Relic::MetaliVoidArmaments, Relic::LightrailOrdnance, Relic::TheProphetsTears, Relic::CircletOfTheVoid, Relic::TheObsidian] {
        give_relic(&mut s, 0, r);
    }
    assert!(relic::substitutes_strategy_token(&s, Seat(0)) && !relic::substitutes_strategy_token(&s, Seat(1)));
    assert!(relic::grants_sustain(&s, Seat(0)) && !relic::grants_sustain(&s, Seat(1)));
    assert_eq!(relic::extra_barrage(&s, Seat(0)), Some((6, 3)));
    assert_eq!(relic::extra_barrage(&s, Seat(1)), None);
    assert_eq!(relic::space_dock_cannon(&s, Seat(0)), Some((5, 2)));
    assert_eq!(relic::space_dock_cannon(&s, Seat(1)), None);
    assert_eq!(relic::prerequisite_waivers(&s, Seat(0)), 1);
    assert_eq!(relic::prerequisite_waivers(&s, Seat(1)), 0);
    assert!(relic::ignores_gravity_rifts(&s, Seat(0)) && !relic::ignores_gravity_rifts(&s, Seat(1)));
    assert_eq!(relic::secret_objective_bonus(&s, Seat(0)), 1);
    assert_eq!(relic::secret_objective_bonus(&s, Seat(1)), 0);
}

#[test]
fn only_the_action_relics_have_an_action_and_each_has_its_own_arm() {
    let actions = [Relic::BookOfLatvinia, Relic::TheCodex, Relic::DynamisCore, Relic::EnigmaticDevice, Relic::MawOfWorlds, Relic::StellarConverter, Relic::JRXS455O, Relic::TheSilverFlame, Relic::NanoForge];
    for r in Relic::ALL {
        assert_eq!(relic::has_action(r), actions.contains(&r), "{r:?}");
    }
    let mut s = table().state();
    assert_eq!(relic::use_arm(&s, Seat(0), Relic::TheCodex), UseArm::NotHeld);
    let arms = [
        (Relic::EnigmaticDevice, UseArm::EnigmaticDevice),
        (Relic::MawOfWorlds, UseArm::MawOfWorlds),
        (Relic::TheCodex, UseArm::Codex),
        (Relic::StellarConverter, UseArm::StellarConverter),
        (Relic::JRXS455O, UseArm::TitanPrototype),
        (Relic::DynamisCore, UseArm::DynamisCore),
        (Relic::BookOfLatvinia, UseArm::BookOfLatvinia),
        (Relic::TheSilverFlame, UseArm::SilverFlame),
        (Relic::TheTriad, UseArm::Unresolved),
    ];
    for (r, _) in arms {
        give_relic(&mut s, 0, r);
    }
    for (r, arm) in arms {
        assert_eq!(relic::use_arm(&s, Seat(0), r), arm, "{r:?}");
        assert_eq!(relic::use_arm(&s, Seat(1), r), UseArm::NotHeld);
    }
}

#[test]
fn action_relics_are_listed_in_hand_order_without_an_unresolvable_enigmatic_device_or_stellar_converter() {
    let mut s = table().state();
    for r in [Relic::TheTriad, Relic::EnigmaticDevice, Relic::TheCodex, Relic::StellarConverter, Relic::ScepterOfEmelpar] {
        give_relic(&mut s, 0, r);
    }
    let all = relic::action_relics(&s, Seat(0), true, true);
    assert_eq!(all.as_slice(), &[Relic::EnigmaticDevice, Relic::TheCodex, Relic::StellarConverter]);
    assert_eq!(relic::action_relics(&s, Seat(0), false, true).as_slice(), &[Relic::TheCodex, Relic::StellarConverter]);
    assert_eq!(relic::action_relics(&s, Seat(0), true, false).as_slice(), &[Relic::EnigmaticDevice, Relic::TheCodex]);
    assert!(relic::action_relics(&s, Seat(9), true, true).is_empty());
    assert!(relic::action_relics(&s, Seat(1), true, true).is_empty());
}

// ---- promissory notes -------------------------------------------------------------------------

fn note(s: &mut CoreState, note: PromissoryNote, owner: Faction, holder: u8) -> NoteKey {
    let key = NoteKey { note, faction: owner };
    s.cards_mut().promissory.edit().insert(key, Seat(holder));
    key
}

#[test]
fn the_notes_placed_by_their_own_action_are_a_fixed_list_plus_protection_from_its_version() {
    let s = table().rules(RULES_FACTION_EFFECTS - 1).state();
    for n in [PromissoryNote::TradeConvoys, PromissoryNote::BloodPact, PromissoryNote::DarkPact, PromissoryNote::ShareKnowledge, PromissoryNote::Sever] {
        assert!(promissory::is_action_placed(&s, n), "{n:?}");
    }
    assert!(!promissory::is_action_placed(&s, PromissoryNote::Ceasefire) && !promissory::is_action_placed(&s, PromissoryNote::Alliance));
    assert!(!promissory::is_action_placed(&s, PromissoryNote::PromiseOfProtection), "before its version");
    let s = table().rules(RULES_FACTION_EFFECTS).state();
    assert!(promissory::is_action_placed(&s, PromissoryNote::PromiseOfProtection));
}

#[test]
fn a_note_is_found_by_holder_and_owner_and_the_owners_own_hand_does_not_count_as_lent() {
    let mut s = table().state();
    let alliance = note(&mut s, PromissoryNote::Alliance, Faction::TheArborec, 2);
    let cease = note(&mut s, PromissoryNote::Ceasefire, Faction::TheArborec, 1);
    assert_eq!(promissory::faction_of(&s, Seat(1)), Some(Faction::TheArborec));
    assert_eq!(promissory::faction_of(&s, Seat(9)), None);
    assert_eq!(promissory::held_by(&s, Seat(2)), vec![alliance]);
    assert_eq!(promissory::holder_of(&s, PromissoryNote::Alliance, Seat(1)), Some(Seat(2)));
    assert_eq!(promissory::holder_of(&s, PromissoryNote::Ceasefire, Seat(1)), None, "the owner has it");
    assert_eq!(promissory::holder_of(&s, PromissoryNote::Sever, Seat(1)), None, "nobody has it");
    assert_eq!(promissory::holder_of(&s, PromissoryNote::Alliance, Seat(9)), None);
    assert_eq!(promissory::held_foreign(&s, Seat(2), PromissoryNote::Alliance), 1);
    assert_eq!(promissory::held_foreign(&s, Seat(1), PromissoryNote::Ceasefire), 0, "an own copy is not foreign");
    assert_eq!(promissory::held_by(&s, Seat(1)), vec![cease]);
}

#[test]
fn action_notes_in_hand_are_foreign_unplayed_and_of_the_action_kind() {
    let mut s = table().state();
    let convoys = note(&mut s, PromissoryNote::TradeConvoys, Faction::TheArborec, 2);
    let own = note(&mut s, PromissoryNote::Sever, Faction::TheBaronyOfLetnev, 2);
    let _alliance = note(&mut s, PromissoryNote::Alliance, Faction::TheArborec, 2);
    assert_eq!(promissory::action_notes_in_hand(&s, Seat(2)), vec![convoys], "own copy and non-action notes are left out");
    assert!(!promissory::reaches_anyone(&s, Seat(2)));
    s.cards_mut().promissory_faceup.edit().insert(convoys, ());
    assert!(promissory::reaches_anyone(&s, Seat(2)), "a faceup Trade Convoys");
    assert!(!promissory::reaches_anyone(&s, Seat(1)));
    assert!(promissory::action_notes_in_hand(&s, Seat(2)).is_empty(), "faceup: no longer in hand to place");
    let _ = own;
}

#[test]
fn mahact_may_not_receive_an_alliance_note_and_nobody_else_is_barred() {
    let s = table().state();
    assert!(!promissory::may_receive(&s, Seat(0), PromissoryNote::Alliance));
    assert!(promissory::may_receive(&s, Seat(0), PromissoryNote::Ceasefire));
    assert!(promissory::may_receive(&s, Seat(1), PromissoryNote::Alliance));
}

#[test]
fn a_lent_ceasefire_bars_the_owner_from_systems_where_the_holder_has_units() {
    let systems = plain_systems();
    let planet = systems[1].data().planets[0];
    let mut s = table().unit(systems[0], None, Unit::CruiserI, 2, 1).unit(systems[1], Some(planet), Unit::InfantryI, 2, 1).state();
    assert!(!promissory::denies_movement_into(&s, Seat(1), systems[0]), "no note lent");
    note(&mut s, PromissoryNote::Ceasefire, Faction::TheArborec, 2);
    assert!(promissory::denies_movement_into(&s, Seat(1), systems[0]), "a ship of the holder in space");
    assert!(promissory::denies_movement_into(&s, Seat(1), systems[1]), "a ground force of the holder on a planet");
    assert!(!promissory::denies_movement_into(&s, Seat(1), systems[2]), "no units there");
    assert!(!promissory::denies_movement_into(&s, Seat(0), systems[0]), "another seat's mover");
    let mut own = table().unit(systems[0], None, Unit::CruiserI, 1, 1).state();
    note(&mut own, PromissoryNote::Ceasefire, Faction::TheArborec, 1);
    assert!(!promissory::denies_movement_into(&own, Seat(1), systems[0]), "held by the owner");
}

// ---- leaders ----------------------------------------------------------------------------------

fn commander_of(faction: Faction) -> Leader {
    Leader::ALL.into_iter().find(|l| l.data().kind == LeaderType::Commander && l.data().faction.factions() == [faction]).expect("a commander")
}

fn set_status(s: &mut CoreState, seat: u8, l: Leader, st: LeaderStatus) {
    s.players_mut().by_seat_mut(Seat(seat)).unwrap().leaders.status.insert(l, st);
}

#[test]
fn a_leaders_status_is_the_seats_own_entry() {
    let cmd = commander_of(Faction::TheBaronyOfLetnev);
    let mut s = table().state();
    assert_eq!(leaders::status(&s, Seat(2), cmd), None);
    set_status(&mut s, 2, cmd, LeaderStatus::Locked);
    assert_eq!(leaders::status(&s, Seat(2), cmd), Some(LeaderStatus::Locked));
    assert_eq!(leaders::status(&s, Seat(1), cmd), None);
    assert_eq!(leaders::status(&s, Seat(9), cmd), None);
}

#[test]
fn a_commander_ability_comes_from_unlocking_a_mark_an_alliance_or_mahacts_imperia() {
    let cmd = commander_of(Faction::TheBaronyOfLetnev);
    let mut s = table().state();
    assert!(!leaders::has_commander_ability_in(&s, Seat(2), cmd));
    set_status(&mut s, 2, cmd, LeaderStatus::Readied);
    assert!(!leaders::has_commander_ability_in(&s, Seat(2), cmd), "only unlocked counts");
    set_status(&mut s, 2, cmd, LeaderStatus::Unlocked);
    assert!(leaders::has_commander_ability_in(&s, Seat(2), cmd));
    assert!(!leaders::has_commander_ability_in(&s, Seat(1), cmd));
    // A recorded mark gives the ability without the unlock.
    let mut m = table().state();
    m.marks_mut().tail.edit().insert(MarkKey::CommanderAbility(Seat(1), cmd), MarkVal::Atom(Atom::Word(ti4_engine_core::keys::Word::Yes)));
    assert!(leaders::has_commander_ability_in(&m, Seat(1), cmd));
    // The imperia closure.
    assert!(leaders::has_commander_ability_with(&table().state(), Seat(1), cmd, || true));
    assert!(!leaders::has_commander_ability_with(&table().state(), Seat(1), cmd, || false));
}

#[test]
fn a_faceup_alliance_note_lends_its_owners_unlocked_commander_to_the_holder() {
    let cmd = commander_of(Faction::TheBaronyOfLetnev);
    let mut s = table().state();
    set_status(&mut s, 2, cmd, LeaderStatus::Unlocked);
    let key = note(&mut s, PromissoryNote::Alliance, Faction::TheBaronyOfLetnev, 1);
    assert!(!leaders::has_commander_ability_in(&s, Seat(1), cmd), "the note is still in hand");
    s.cards_mut().promissory_faceup.edit().insert(key, ());
    assert!(leaders::has_commander_ability_in(&s, Seat(1), cmd), "faceup: the holder uses the commander");
    assert!(!leaders::has_commander_ability_in(&s, Seat(0), cmd));
    set_status(&mut s, 2, cmd, LeaderStatus::Readied);
    assert!(!leaders::has_commander_ability_in(&s, Seat(1), cmd), "only while the owner has it unlocked");
}

#[test]
fn mahacts_imperia_pools_the_unlocked_commanders_of_the_players_whose_tokens_it_holds() {
    let cmd = commander_of(Faction::TheArborec);
    let mut s = table().state();
    set_status(&mut s, 1, cmd, LeaderStatus::Unlocked);
    assert!(!leaders::imperia_from_pool(&s, Seat(0), cmd), "no tokens held");
    s.marks_mut().tail.edit().insert(MarkKey::MahactFleet(Seat(0)), MarkVal::List(vec![MarkVal::Atom(Atom::Seat(Seat(1)))]));
    assert!(leaders::imperia_from_pool(&s, Seat(0), cmd));
    assert!(leaders::has_commander_ability_in(&s, Seat(0), cmd));
    assert!(!leaders::imperia_from_pool(&s, Seat(1), cmd), "only Mahact has Imperia");
    set_status(&mut s, 1, cmd, LeaderStatus::Readied);
    assert!(!leaders::imperia_from_pool(&s, Seat(0), cmd), "the owner's commander must be unlocked");
    let mut own = table().state();
    set_status(&mut own, 0, cmd, LeaderStatus::Unlocked);
    own.marks_mut().tail.edit().insert(MarkKey::MahactFleet(Seat(0)), MarkVal::List(vec![MarkVal::Atom(Atom::Seat(Seat(0)))]));
    assert!(!leaders::imperia_from_pool(&own, Seat(0), cmd), "its own seat in the pool does not count");
}
