//! Lane G4: the last loud gap rows turned into rules of the core table (`ti4_engine_core_factions::silent::g4`). The lockstep against the string is in `ti4-engine`
//! (`silent4_live_tests` and the lockstep files of the harnesses); these tests pin the rules.

use ti4_engine_core::generated::Faction;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::state::{EventKind, Key, Payload, Val};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::windows::EmitEvent;

fn table() -> AbilityTable {
    let mut t = AbilityTable::default();
    ti4_engine_core_factions::silent::register_before_movement(&mut t);
    ti4_engine_core_factions::silent::g3::register_after_ca_combat(&mut t);
    ti4_engine_core_factions::silent::register_before_muaat(&mut t);
    ti4_engine_core_factions::silent::register_abilities(&mut t);
    ti4_engine_core_factions::silent::g4::magen_hit::register_abilities(&mut t);
    t
}

fn rig(scenario: Scenario, state_edit: impl FnOnce(&mut CoreState), event: ti4_engine_core::state::EventRec) -> Rig<EmitEvent> {
    let (mut state, kernel) = scenario.build();
    state_edit(&mut state);
    let mut r = Rig::from_parts(state, kernel, EmitEvent::new(event)).with_abilities(table());
    r.run();
    r
}

// -- Propagation --------------------------------------------------------------------------------------------------------------

mod propagation {
    use super::*;
    use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt, Pool};

    fn announced(seat: u8) -> ti4_engine_core::state::EventRec {
        fx::event(EventKind::PropagationResearch, Payload::new().with(Key::Player, Val::Seat(Seat(seat))))
    }

    fn owes(seat: u8, count: u32) -> impl FnOnce(&mut CoreState) {
        move |s| {
            s.marks_mut().tail.edit().insert(MarkKey::NekroPropagation(Seat(seat)), MarkVal::Atom(Atom::Num(count)));
        }
    }

    fn pending(r: &Rig<EmitEvent>, seat: u8) -> Option<MarkVal> {
        r.state.marks().tail.get(&MarkKey::NekroPropagation(Seat(seat))).cloned()
    }

    fn pool_of(r: &Rig<EmitEvent>, seat: u8, pool: Pool) -> i16 {
        let p = r.state.players().by_seat(Seat(seat)).unwrap();
        match pool {
            Pool::Tactic => i16::from(p.econ.tactic_tokens),
            Pool::Fleet => i16::from(p.econ.fleet_tokens),
            Pool::Strategic => i16::from(p.econ.strategic_tokens),
        }
    }

    #[test]
    fn a_replaced_research_asks_the_pools_of_three_tokens_once_and_settles_one_pending_research() {
        covers!("silent::g4::propagation::PROPAGATION");
        let mut r = rig(Scenario::new(&[Faction::TheNekroVirus, Faction::TheEmiratesOfHacan]), owes(0, 2), announced(0));
        let before: Vec<i16> = [Pool::Tactic, Pool::Fleet, Pool::Strategic].iter().map(|p| pool_of(&r, 0, *p)).collect();
        // One question for the three tokens.
        assert_eq!(r.asked().actor, Seat(0), "{}", r.transcript());
        assert_eq!(r.asked().ask, Ask::Strategy(SAsk::GainPool { left: 3 }), "{}", r.transcript());
        let at = r.opts().iter().position(|o| *o == Opt::Strategy(SOpt::gain_of(&[Pool::Fleet; 3]))).expect("all three in the fleet pool");
        r.pick(at);
        assert!(r.is_done(), "{}", r.transcript());
        assert_eq!(pool_of(&r, 0, Pool::Fleet) - before[1], 3);
        assert_eq!((pool_of(&r, 0, Pool::Tactic), pool_of(&r, 0, Pool::Strategic)), (before[0], before[2]));
        assert_eq!(pending(&r, 0), Some(MarkVal::Atom(Atom::Num(1))), "one research is settled, one is left for the next announcement");
        // The last pending research removes the mark.
        let mut last = rig(Scenario::new(&[Faction::TheNekroVirus, Faction::TheEmiratesOfHacan]), owes(0, 1), announced(0));
        last.pick(0);
        assert!(last.is_done(), "{}", last.transcript());
        assert_eq!(pending(&last, 0), None);
    }

    #[test]
    fn only_the_nekro_with_a_pending_research_is_asked() {
        covers!("silent::g4::propagation::PROPAGATION");
        // The announcement names another seat: nothing happens to the Nekro's count.
        let other = rig(Scenario::new(&[Faction::TheNekroVirus, Faction::TheEmiratesOfHacan]), owes(0, 1), announced(1));
        assert!(other.is_done(), "{}", other.transcript());
        assert_eq!(pending(&other, 0), Some(MarkVal::Atom(Atom::Num(1))));
        // Nothing owed: no question.
        let none = rig(Scenario::new(&[Faction::TheNekroVirus, Faction::TheEmiratesOfHacan]), |_| {}, announced(0));
        assert!(none.is_done(), "{}", none.transcript());
        // A seat of another faction with the mark (it cannot happen in a game) gets nothing.
        let hacan = rig(Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheNekroVirus]), owes(0, 1), announced(0));
        assert!(hacan.is_done(), "{}", hacan.transcript());
        assert_eq!(pending(&hacan, 0), Some(MarkVal::Atom(Atom::Num(1))));
    }
}

// -- Tungstantus (the Titans commander) ---------------------------------------------------------------------------------------

fn set_leader(s: &mut CoreState, seat: u8, leader: ti4_engine_core::generated::Leader, status: ti4_engine_core::state::LeaderStatus) {
    s.players_mut().by_seat_mut(Seat(seat)).unwrap().leaders.status.insert(leader, status);
}

fn goods(r: &Rig<EmitEvent>, seat: u8) -> i16 {
    r.state.players().by_seat(Seat(seat)).unwrap().econ.trade_goods
}

mod titans_commander {
    use super::*;
    use ti4_engine_core::generated::{Leader, System};
    use ti4_engine_core::state::LeaderStatus;

    fn produced(seat: u8) -> ti4_engine_core::state::EventRec {
        fx::event(EventKind::ProductionUsed, Payload::new().with(Key::Player, Val::Seat(Seat(seat))).with(Key::System, Val::System(System::ALL[0])))
    }

    #[test]
    fn the_holder_of_the_commander_may_gain_a_trade_good_when_it_uses_production() {
        covers!("silent::g4::titans_commander::TUNGSTANTUS");
        let scenario = || Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol]).goods(0, 2);
        let mut r = rig(scenario(), |s| set_leader(s, 0, Leader::Tungstantus, LeaderStatus::Unlocked), produced(0));
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the commander");
        r.pick(use_it);
        assert!(r.is_done(), "{}", r.transcript());
        assert_eq!(goods(&r, 0), 3);
        let mut declined = rig(scenario(), |s| set_leader(s, 0, Leader::Tungstantus, LeaderStatus::Unlocked), produced(0));
        declined.decline();
        assert_eq!(goods(&declined, 0), 2);
    }

    #[test]
    fn only_an_unlocked_commander_of_the_producing_seat_is_offered() {
        covers!("silent::g4::titans_commander::TUNGSTANTUS");
        let scenario = || Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol]).goods(0, 2).goods(1, 2);
        let locked = rig(scenario(), |s| set_leader(s, 0, Leader::Tungstantus, LeaderStatus::Locked), produced(0));
        assert!(locked.is_done(), "{}", locked.transcript());
        // Seat 1 uses production while seat 0 holds the commander: the card is about its holder's own production.
        let other = rig(scenario(), |s| set_leader(s, 0, Leader::Tungstantus, LeaderStatus::Unlocked), produced(1));
        assert!(other.is_done(), "{}", other.transcript());
        assert_eq!((goods(&other, 0), goods(&other, 1)), (2, 2));
    }
}

// -- Spy Net -------------------------------------------------------------------------------------------------------------------

mod spy_net {
    use super::*;
    use ti4_engine_core::choice::cards::{Ask as CAsk, Opt as COpt};
    use ti4_engine_core::generated::{ActionCard, PromissoryNote};
    use ti4_engine_core::state::NoteKey;

    const NOTE: NoteKey = NoteKey { note: PromissoryNote::SpyNet, faction: Faction::TheYssarilTribes };

    fn turn(seat: u8) -> ti4_engine_core::state::EventRec {
        fx::event(EventKind::TurnBegan, Payload::new().with(Key::Player, Val::Seat(Seat(seat))))
    }

    /// Seat 0 (Hacan) holds the note, seat 1 is the Yssaril with `hand`; the printed names of the cards are `groups` (the same number: the same card).
    fn rig_with(hand: &[ActionCard], groups: &[(ActionCard, u8)], mine: &[ActionCard]) -> Rig<EmitEvent> {
        let (mut state, kernel) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheYssarilTribes]).hand(1, hand).hand(0, mine).build();
        state.cards_mut().promissory.edit().insert(NOTE, Seat(0));
        let mut t = table();
        t.facts.card_groups = (0..ActionCard::COUNT).map(|i| u8::try_from(i).unwrap_or(u8::MAX)).collect();
        for (card, group) in groups {
            t.facts.card_groups[*card as usize] = *group;
        }
        let mut r = Rig::from_parts(state, kernel, EmitEvent::new(turn(0))).with_abilities(t);
        r.run();
        r
    }

    fn hand(r: &Rig<EmitEvent>, seat: u8) -> Vec<ActionCard> {
        r.state.players().by_seat(Seat(seat)).unwrap().cards.hand.as_slice().to_vec()
    }

    #[test]
    fn the_holder_chooses_one_card_of_the_yssaril_hand_and_the_note_goes_home() {
        covers!("silent::g4::spynet::SPY_NET");
        let mut r = rig_with(&[ActionCard::Bribery, ActionCard::Blitz, ActionCard::Bunker], &[], &[]);
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the note");
        r.pick(use_it);
        assert_eq!((r.asked().actor, r.asked().ask), (Seat(0), Ask::Cards(CAsk::TakeRevealed { owner: Seat(1) })), "{}", r.transcript());
        let ctx = r.asked().ctx.clone();
        assert!(!ctx.optional && !ctx.space_battle && ctx.target.is_none() && ctx.invasion_seq.is_none(), "a plain question of the note");
        // One option per printed card, in the order of the names.
        assert_eq!(r.opts(), vec![Opt::Cards(COpt::Card(ActionCard::Blitz)), Opt::Cards(COpt::Card(ActionCard::Bribery)), Opt::Cards(COpt::Card(ActionCard::Bunker))]);
        r.pick(1);
        assert!(r.is_done(), "{}", r.transcript());
        assert_eq!((hand(&r, 0), hand(&r, 1)), (vec![ActionCard::Bribery], vec![ActionCard::Blitz, ActionCard::Bunker]));
        assert_eq!(r.state.cards().promissory.get(&NOTE), Some(&Seat(1)), "back with the Yssaril");
        assert!(r.state.marks().tail.iter().any(|(k, _)| matches!(k, ti4_engine_core::keys::MarkKey::CardsStaged(_))), "the take waits for the game step");
    }

    #[test]
    fn two_copies_are_one_option_and_one_card_is_taken_unasked() {
        covers!("silent::g4::spynet::SPY_NET");
        // Two copies of one printed card: a single option, so no question.
        let mut twins = rig_with(&[ActionCard::Bribery, ActionCard::Blitz], &[(ActionCard::Blitz, ActionCard::Bribery as u8)], &[]);
        let use_it = twins.opts().iter().position(|o| *o != Opt::Decline).expect("the note");
        twins.pick(use_it);
        assert!(twins.is_done(), "{}", twins.transcript());
        assert_eq!((hand(&twins, 0).len(), hand(&twins, 1).len()), (1, 1));
        // Declining leaves the cards where they are and the note with the holder.
        let mut declined = rig_with(&[ActionCard::Bribery, ActionCard::Blitz], &[], &[]);
        declined.decline();
        assert!(declined.is_done());
        assert_eq!((hand(&declined, 0).len(), hand(&declined, 1).len()), (0, 2));
        assert_eq!(declined.state.cards().promissory.get(&NOTE), Some(&Seat(0)));
    }

    #[test]
    fn the_note_needs_its_holder_on_his_turn_and_a_hand_to_look_at() {
        covers!("silent::g4::spynet::SPY_NET");
        // The Yssaril's hand is empty: nothing is offered.
        let empty = rig_with(&[], &[], &[]);
        assert!(empty.is_done(), "{}", empty.transcript());
        // Another seat's turn begins: the holder is not asked.
        let (mut state, kernel) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheYssarilTribes]).hand(1, &[ActionCard::Bribery]).build();
        state.cards_mut().promissory.edit().insert(NOTE, Seat(0));
        let mut other = Rig::from_parts(state, kernel, EmitEvent::new(turn(1))).with_abilities(table());
        other.run();
        assert!(other.is_done(), "{}", other.transcript());
    }
}

// -- Stymie --------------------------------------------------------------------------------------------------------------------

mod stymie {
    use super::*;
    use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt};
    use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt};
    use ti4_engine_core::choice::silent4::{Ask as UAsk, Opt as UOpt};
    use ti4_engine_core::generated::{PromissoryNote, System, Unit};
    use ti4_engine_core::state::NoteKey;

    const NOTE: NoteKey = NoteKey { note: PromissoryNote::Stymie, faction: Faction::TheArborec };

    fn moved(mover: u8, system: System) -> ti4_engine_core::state::EventRec {
        fx::event(EventKind::ShipMoved, Payload::new().with(Key::Player, Val::Seat(Seat(mover))).with(Key::System, Val::System(system)))
    }

    /// Seat 1 (Sol) holds Stymie and a cruiser in `systems[0]` of a ring of `n` plain systems; seat 0 (Hacan) is the mover, seat 2 the Arborec.
    fn rig_ring(n: usize, mover: u8, holds: bool) -> (Rig<EmitEvent>, Vec<System>) {
        let systems = ti4_engine_core_tests::scenario::plain_systems();
        let ring: Vec<System> = systems[..n].to_vec();
        let (mut state, kernel) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol, Faction::TheArborec]).map_from(&ring).unit(ring[0], None, Unit::CruiserI, 1, 1).build();
        if holds {
            state.cards_mut().promissory.edit().insert(NOTE, Seat(1));
        }
        let mut r = Rig::from_parts(state, kernel, EmitEvent::new(moved(mover, ring[0]))).with_abilities(table());
        r.run();
        (r, ring)
    }

    #[test]
    fn the_holder_picks_a_system_for_the_movers_token_and_the_note_goes_home() {
        covers!("silent::g4::stymie::STYMIE");
        let (mut r, ring) = rig_ring(3, 0, true);
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the note");
        r.pick(use_it);
        assert_eq!(r.asked().actor, Seat(1), "{}", r.transcript());
        assert_eq!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::U(UAsk::Stymie { mover: Seat(0) }))));
        // Every non-home system of the map takes the token: the three plain systems, in the order of the ids.
        let mut want = ring.clone();
        want.sort();
        assert_eq!(r.opts(), want.iter().map(|s| Opt::Reaction(ROpt::G(GOpt::U(UOpt::System(*s))))).collect::<Vec<_>>());
        r.pick(1);
        assert!(r.is_done(), "{}", r.transcript());
        assert!(ti4_engine_core::query::board::has_token(&r.state, want[1], Seat(0)), "the mover's token is in the system picked");
        assert_eq!(ring.iter().filter(|s| ti4_engine_core::query::board::has_token(&r.state, **s, Seat(0))).count(), 1);
        assert_eq!(r.state.cards().promissory.get(&NOTE), Some(&Seat(2)), "back with the Arborec");
        assert!(!ti4_engine_core::kernel::fx::has_staged(&r.state), "no faction module is seated (the adapter says so): the placement is announced to nobody");
    }

    #[test]
    fn with_a_module_seated_the_placement_of_the_token_is_staged_as_an_event() {
        covers!("silent::g4::stymie::STYMIE");
        let systems = ti4_engine_core_tests::scenario::plain_systems();
        let ring: Vec<System> = systems[..2].to_vec();
        let (mut state, kernel) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol, Faction::TheArborec]).map_from(&ring).unit(ring[0], None, Unit::CruiserI, 1, 1).build();
        state.cards_mut().promissory.edit().insert(NOTE, Seat(1));
        fx::place_command_token(&mut state, Seat(0), ring[0]);
        let mut t = table();
        t.facts.staging = true;
        let mut r = Rig::from_parts(state, kernel, EmitEvent::new(moved(0, ring[0]))).with_abilities(t);
        r.run();
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the note");
        r.pick(use_it);
        assert!(r.is_done(), "{}", r.transcript());
        assert!(r.state.pending().staged.iter().any(|e| e.kind == EventKind::CommandTokenPlaced && e.payload.seat(Key::Player) == Some(Seat(0)) && e.payload.system(Key::System) == Some(ring[1])), "the token placed is announced with the next emission");
    }

    #[test]
    fn a_system_where_the_mover_has_a_token_is_not_offered_and_one_system_needs_no_question() {
        covers!("silent::g4::stymie::STYMIE");
        let systems = ti4_engine_core_tests::scenario::plain_systems();
        let ring: Vec<System> = systems[..2].to_vec();
        let (mut state, kernel) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol, Faction::TheArborec]).map_from(&ring).unit(ring[0], None, Unit::CruiserI, 1, 1).build();
        state.cards_mut().promissory.edit().insert(NOTE, Seat(1));
        fx::place_command_token(&mut state, Seat(0), ring[0]);
        let mut r = Rig::from_parts(state, kernel, EmitEvent::new(moved(0, ring[0]))).with_abilities(table());
        r.run();
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the note");
        r.pick(use_it);
        assert!(r.is_done(), "the only system left takes the token unasked: {}", r.transcript());
        assert!(ti4_engine_core::query::board::has_token(&r.state, ring[1], Seat(0)));
    }

    #[test]
    fn the_note_needs_its_holder_another_players_move_and_a_unit_of_the_holder_there() {
        covers!("silent::g4::stymie::STYMIE");
        // The holder moves itself: not offered.
        let (own, _) = rig_ring(3, 1, true);
        assert!(own.is_done(), "{}", own.transcript());
        // Nobody holds the note.
        let (none, _) = rig_ring(3, 0, false);
        assert!(none.is_done(), "{}", none.transcript());
        // The holder has no unit in the system the ships moved into.
        let systems = ti4_engine_core_tests::scenario::plain_systems();
        let (mut state, kernel) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol, Faction::TheArborec]).map_from(&systems[..3]).unit(systems[1], None, Unit::CruiserI, 1, 1).build();
        state.cards_mut().promissory.edit().insert(NOTE, Seat(1));
        let mut elsewhere = Rig::from_parts(state, kernel, EmitEvent::new(moved(0, systems[0]))).with_abilities(table());
        elsewhere.run();
        assert!(elsewhere.is_done(), "{}", elsewhere.transcript());
        // The Arborec holding its own note has nobody to hold it against.
        let (mut state, kernel) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheArborec]).map_from(&systems[..3]).unit(systems[0], None, Unit::CruiserI, 1, 1).build();
        state.cards_mut().promissory.edit().insert(NOTE, Seat(1));
        let mut arborec = Rig::from_parts(state, kernel, EmitEvent::new(moved(0, systems[0]))).with_abilities(table());
        arborec.run();
        assert!(arborec.is_done(), "{}", arborec.transcript());
    }
}

// -- Magen Defense Grid's hit -------------------------------------------------------------------------------------------------

mod magen_hit {
    use super::*;
    use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt};
    use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt};
    use ti4_engine_core::choice::silent4::{Ask as UAsk, Opt as UOpt};
    use ti4_engine_core::generated::{Planet, System, Technology, Unit};
    use ti4_engine_core::query::board;

    fn started(system: System, planet: Planet, attacker: u8, defender: u8) -> ti4_engine_core::state::EventRec {
        fx::event(
            EventKind::GroundCombatStarted,
            Payload::new().with(Key::System, Val::System(system)).with(Key::Planet, Val::Planet(planet)).with(Key::Attacker, Val::Seat(Seat(attacker))).with(Key::Defender, Val::Seat(Seat(defender))),
        )
    }

    fn two_planet_system() -> (System, Vec<Planet>) {
        let system = ti4_engine_core_tests::scenario::plain_systems().into_iter().find(|s| !s.data().planets.is_empty()).expect("a plain system with a planet");
        (system, system.data().planets.to_vec())
    }

    /// Seat 0 (Hacan) holds the Magen Defense Grid and a PDS on the planet; seat 1 (Saar) has `forces` there. Seat 0 defends (`defends`) or invades.
    fn rig(forces: &[(Unit, usize)], structure: bool, holds: bool, event_seats: (u8, u8)) -> (Rig<EmitEvent>, Planet, System) {
        let (system, planets) = two_planet_system();
        let mut scenario = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheClanOfSaar]).control(planets[0], 0);
        if structure {
            scenario = scenario.unit(system, Some(planets[0]), Unit::PDSI, 0, 1);
        }
        for (unit, n) in forces {
            scenario = scenario.unit(system, Some(planets[0]), *unit, 1, *n);
        }
        let (mut state, kernel) = scenario.build();
        if holds {
            state.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::MagenDefenseGridOmegaOmega);
        }
        let mut r = Rig::from_parts(state, kernel, EmitEvent::new(started(system, planets[0], event_seats.0, event_seats.1))).with_abilities(table());
        r.run();
        (r, planets[0], system)
    }

    fn units(r: &Rig<EmitEvent>, planet: Planet, seat: u8) -> Vec<(Unit, bool)> {
        board::planet_units(&r.state, planet).iter().filter(|p| p.owner() == Seat(seat)).map(|p| (p.kind(), p.sustained())).collect()
    }

    #[test]
    fn the_owner_assigns_the_hit_when_several_kinds_stand_there_and_the_unit_without_sustain_dies() {
        covers!("silent::g4::magen_hit::MD_HIT");
        let (mut r, planet, _) = rig(&[(Unit::InfantryI, 2), (Unit::ScavengerZeta, 1)], true, true, (1, 0));
        assert_eq!((r.asked().actor, r.asked().ask), (Seat(0), Ask::Reaction(RAsk::G(GAsk::U(UAsk::MagenHit)))), "{}", r.transcript());
        assert_eq!(r.opts().len(), 2, "identical units are one option: {}", r.transcript());
        let infantry = r.opts().iter().position(|o| matches!(o, Opt::Reaction(ROpt::G(GOpt::U(UOpt::Target { unit: Unit::InfantryI, sturdy: false, damaged: false, .. }))))).expect("the infantry option");
        r.pick(infantry);
        assert!(r.is_done(), "an infantry has no SUSTAIN DAMAGE, so its owner is not asked: {}", r.transcript());
        assert_eq!(units(&r, planet, 1), vec![(Unit::InfantryI, false), (Unit::ScavengerZeta, false)]);
        assert_eq!(units(&r, planet, 0), vec![(Unit::PDSI, false)], "the holder's structure stands");
    }

    #[test]
    fn the_victim_decides_on_sustain_damage_for_a_mech_and_a_damaged_mech_is_destroyed_by_the_hit() {
        covers!("silent::g4::magen_hit::MD_HIT");
        // A lone mech: no question for the owner; its owner may sustain.
        let (mut r, planet, system) = rig(&[(Unit::ScavengerZeta, 1)], true, true, (1, 0));
        assert_eq!(r.asked().actor, Seat(1), "the victim answers: {}", r.transcript());
        assert!(matches!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::U(UAsk::GroundSustain { unit: Unit::ScavengerZeta, system: s, planet: p, .. }))) if s == system && p == planet), "{}", r.transcript());
        assert_eq!(r.opts(), vec![Opt::Reaction(ROpt::G(GOpt::U(UOpt::Sustain))), Opt::Decline]);
        r.pick(0);
        assert!(r.is_done());
        assert_eq!(units(&r, planet, 1), vec![(Unit::ScavengerZeta, true)], "damaged, not destroyed");
        // Declined: destroyed.
        let (mut declined, planet, _) = rig(&[(Unit::ScavengerZeta, 1)], true, true, (1, 0));
        declined.pick(1);
        assert_eq!(units(&declined, planet, 1), vec![]);
        // A mech that is damaged already cannot sustain: no question, destroyed.
        let (system, planets) = two_planet_system();
        let (mut state, kernel) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheClanOfSaar]).control(planets[0], 0).unit(system, Some(planets[0]), Unit::PDSI, 0, 1).unit(system, Some(planets[0]), Unit::ScavengerZeta, 1, 1).build();
        state.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::MagenDefenseGridOmegaOmega);
        let at = state.board().planet(planets[0]).units.iter().position(|p| p.owner() == Seat(1)).unwrap();
        let hurt = state.board().planet(planets[0]).units.as_slice()[at].with_sustained(true);
        state.board_mut().planet_mut(planets[0]).units.as_mut_slice()[at] = hurt;
        let mut worn = Rig::from_parts(state, kernel, EmitEvent::new(started(system, planets[0], 1, 0))).with_abilities(table());
        worn.run();
        assert!(worn.is_done(), "{}", worn.transcript());
        assert_eq!(units(&worn, planets[0], 1), vec![]);
    }

    #[test]
    fn the_hit_needs_the_technology_a_structure_on_the_planet_and_a_side_of_the_combat() {
        covers!("silent::g4::magen_hit::MD_HIT");
        for (label, structure, holds, seats) in [("no technology", true, false, (1, 0)), ("no structure", false, true, (1, 0)), ("not a side", true, true, (1, 1))] {
            let (r, planet, _) = rig(&[(Unit::InfantryI, 2)], structure, holds, seats);
            if label == "not a side" {
                // The holder is neither the attacker nor the defender of the combat.
                assert_eq!(units(&r, planet, 1).len(), 2, "{label}");
            }
            assert!(r.is_done(), "{label}: {}", r.transcript());
            assert_eq!(units(&r, planet, 1).len(), 2, "{label}: nothing is hit");
        }
        // The holder fights too: a lone infantry is hit with no question at all.
        let (r, planet, _) = rig(&[(Unit::InfantryI, 1)], true, true, (0, 1));
        assert!(r.is_done(), "{}", r.transcript());
        assert_eq!(units(&r, planet, 1), vec![]);
    }
}

// -- the placement from the reinforcements (Yin Spinner Omega, Iconoclast) ------------------------------------------------------

mod placement {
    use super::*;
    use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt};
    use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt};
    use ti4_engine_core::choice::silent4::{Ask as UAsk, Opt as UOpt};
    use ti4_engine_core::generated::{Planet, System, Technology, Unit, UnitBaseType};
    use ti4_engine_core::query::board;

    fn event(kind: EventKind, seat: u8) -> ti4_engine_core::state::EventRec {
        fx::event(kind, Payload::new().with(Key::Player, Val::Seat(Seat(seat))))
    }

    /// Two systems with planets (`a` first), and their planets.
    fn systems_with_planets() -> Vec<(System, Planet)> {
        ti4_engine_core_tests::scenario::plain_systems().into_iter().filter_map(|s| s.data().planets.first().map(|p| (s, *p))).take(3).collect()
    }

    fn infantry_on(r: &Rig<EmitEvent>, planet: Planet, seat: u8) -> usize {
        board::planet_units(&r.state, planet).iter().filter(|p| p.owner() == Seat(seat) && p.kind().data().base_type == UnitBaseType::Infantry).count()
    }

    fn spinner_rig(carrier: Option<UnitCarrier>, controls: usize, producer: u8, tech: bool) -> (Rig<EmitEvent>, Vec<(System, Planet)>) {
        let spots = systems_with_planets();
        let mut scenario = Scenario::new(&[Faction::TheYinBrotherhood, Faction::TheEmiratesOfHacan]);
        for (_, planet) in spots.iter().take(controls) {
            scenario = scenario.control(*planet, 0);
        }
        if let Some((unit, system_at)) = carrier {
            scenario = scenario.unit(spots[system_at].0, None, unit, 0, 1);
        }
        let (mut state, kernel) = scenario.build();
        if tech {
            state.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::YinSpinnerOmega);
        }
        let mut r = Rig::from_parts(state, kernel, EmitEvent::new(event(EventKind::UnitsProduced, producer))).with_abilities(table());
        r.run();
        (r, spots)
    }

    type UnitCarrier = (Unit, usize);

    #[test]
    fn yin_spinner_asks_where_two_infantry_go_among_the_planets_and_the_space_areas_that_take_them() {
        covers!("silent::g4::yin_spinner::YIN_SPINNER");
        // Two controlled planets and a carrier in the space of the third system.
        let (mut r, spots) = spinner_rig(Some((Unit::CarrierI, 2)), 2, 0, true);
        // The window itself (an optional ability), then the spot.
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the technology");
        r.pick(use_it);
        assert_eq!((r.asked().actor, r.asked().ask), (Seat(0), Ask::Reaction(RAsk::G(GAsk::U(UAsk::Place { base: UnitBaseType::Infantry, fit: 2 })))), "{}", r.transcript());
        let mut want: Vec<(System, Option<Planet>)> = vec![(spots[0].0, Some(spots[0].1)), (spots[1].0, Some(spots[1].1)), (spots[2].0, None)];
        want.sort_by_key(|(s, _)| *s);
        assert_eq!(r.opts(), want.iter().map(|(system, planet)| Opt::Reaction(ROpt::G(GOpt::U(UOpt::Spot { system: *system, planet: *planet, fit: 2 })))).collect::<Vec<_>>());
        let at = want.iter().position(|(_, planet)| planet.is_some()).unwrap();
        r.pick(at);
        assert!(r.is_done(), "{}", r.transcript());
        let planet = want[at].1.unwrap();
        assert_eq!(infantry_on(&r, planet, 0), 2, "both infantry go to the spot chosen");
    }

    #[test]
    fn a_space_area_takes_only_as_many_as_its_ships_carry_and_one_spot_is_no_decision() {
        covers!("silent::g4::yin_spinner::YIN_SPINNER");
        // A destroyer carries nothing: the space area is no spot, the only planet takes both infantry unasked.
        let (mut r, spots) = spinner_rig(Some((Unit::DestroyerI, 2)), 1, 0, true);
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the technology");
        r.pick(use_it);
        assert!(r.is_done(), "{}", r.transcript());
        assert_eq!(infantry_on(&r, spots[0].1, 0), 2);
        // A dreadnought carries one: the space area takes one infantry (fit 1), the planet two.
        let (mut cruiser, spots) = spinner_rig(Some((Unit::DreadnoughtI, 1)), 1, 0, true);
        let use_it = cruiser.opts().iter().position(|o| *o != Opt::Decline).expect("the technology");
        cruiser.pick(use_it);
        let fits: Vec<u8> = cruiser.opts().iter().filter_map(|o| if let Opt::Reaction(ROpt::G(GOpt::U(UOpt::Spot { fit, .. }))) = o { Some(*fit) } else { None }).collect();
        let mut sorted = fits.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, vec![1, 2], "the dreadnought's space takes one, the planet two: {}", cruiser.transcript());
        let space = cruiser.opts().iter().position(|o| matches!(o, Opt::Reaction(ROpt::G(GOpt::U(UOpt::Spot { planet: None, .. }))))).unwrap();
        cruiser.pick(space);
        let in_space = board::space_units(&cruiser.state, spots[1].0).iter().filter(|p| p.owner() == Seat(0) && p.kind().data().base_type == UnitBaseType::Infantry).count();
        assert_eq!(in_space, 1);
    }

    #[test]
    fn yin_spinner_needs_the_technology_the_producer_and_a_spot() {
        covers!("silent::g4::yin_spinner::YIN_SPINNER");
        let (no_tech, _) = spinner_rig(None, 1, 0, false);
        assert!(no_tech.is_done(), "{}", no_tech.transcript());
        let (other, _) = spinner_rig(None, 1, 1, true);
        assert!(other.is_done(), "another seat produced: {}", other.transcript());
        let (nowhere, _) = spinner_rig(None, 0, 0, true);
        assert!(nowhere.is_done(), "no planet and no ship: {}", nowhere.transcript());
    }

    fn mechs_on(r: &Rig<EmitEvent>, planet: Planet, seat: u8) -> usize {
        board::planet_units(&r.state, planet).iter().filter(|p| p.owner() == Seat(seat) && p.kind().data().base_type == UnitBaseType::Mech).count()
    }

    #[test]
    fn iconoclast_deploys_a_mech_after_another_players_relic_on_a_planet_the_naalu_pick() {
        covers!("silent::g4::iconoclast::ICONOCLAST");
        let spots = systems_with_planets();
        let scenario = || Scenario::new(&[Faction::TheNaaluCollective, Faction::TheEmiratesOfHacan]).control(spots[0].1, 0).control(spots[1].1, 0);
        let (state, kernel) = scenario().build();
        let mut r = Rig::from_parts(state, kernel, EmitEvent::new(event(EventKind::RelicGained, 1))).with_abilities(table());
        r.run();
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the mech");
        r.pick(use_it);
        assert_eq!((r.asked().actor, r.asked().ask), (Seat(0), Ask::Reaction(RAsk::G(GAsk::U(UAsk::Place { base: UnitBaseType::Mech, fit: 1 })))), "{}", r.transcript());
        assert_eq!(r.opts().len(), 2, "not optional: no decline beside the two planets");
        let order: Vec<Option<Planet>> = r.opts().iter().filter_map(|o| if let Opt::Reaction(ROpt::G(GOpt::U(UOpt::Spot { planet, .. }))) = o { Some(*planet) } else { None }).collect();
        r.pick(1);
        assert!(r.is_done(), "{}", r.transcript());
        let chosen = order[1].unwrap();
        assert_eq!(mechs_on(&r, chosen, 0), 1);
        // The Naalu's own relic does not trigger it.
        let (state, kernel) = scenario().build();
        let mut own = Rig::from_parts(state, kernel, EmitEvent::new(event(EventKind::RelicGained, 0))).with_abilities(table());
        own.run();
        assert!(own.is_done(), "{}", own.transcript());
        // One planet is no decision.
        let (state, kernel) = Scenario::new(&[Faction::TheNaaluCollective, Faction::TheEmiratesOfHacan]).control(spots[0].1, 0).build();
        let mut one = Rig::from_parts(state, kernel, EmitEvent::new(event(EventKind::RelicGained, 1))).with_abilities(table());
        one.run();
        let use_it = one.opts().iter().position(|o| *o != Opt::Decline).expect("the mech");
        one.pick(use_it);
        assert!(one.is_done(), "{}", one.transcript());
        assert_eq!(mechs_on(&one, spots[0].1, 0), 1);
        // Another faction holds no Iconoclast.
        let (state, kernel) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheNaaluCollective]).control(spots[0].1, 0).build();
        let mut hacan = Rig::from_parts(state, kernel, EmitEvent::new(event(EventKind::RelicGained, 1))).with_abilities(table());
        hacan.run();
        assert!(hacan.is_done(), "{}", hacan.transcript());
    }
}

// -- Foresight -----------------------------------------------------------------------------------------------------------------

mod foresight {
    use super::*;
    use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt};
    use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt};
    use ti4_engine_core::choice::silent4::{Ask as UAsk, Opt as UOpt};
    use ti4_engine_core::generated::{System, Unit};
    use ti4_engine_core::query::board;

    fn finished(mover: u8, system: System, ships: i64) -> ti4_engine_core::state::EventRec {
        fx::event(EventKind::MovementFinished, Payload::new().with(Key::Player, Val::Seat(Seat(mover))).with(Key::System, Val::System(system)).with(Key::ShipsMoved, Val::Int(ships)))
    }

    /// Seat 0 (Hacan) moved into `ring[0]`, the centre of a ring of four plain systems, where seat 1 (the Naalu) has a cruiser and two fighters.
    fn rig_ring(mover: u8, ships: i64, tokens: i16, edit: impl FnOnce(&mut CoreState, &[System])) -> (Rig<EmitEvent>, Vec<System>) {
        let ring: Vec<System> = ti4_engine_core_tests::scenario::plain_systems()[..4].to_vec();
        let (mut state, kernel) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheNaaluCollective]).map_from(&ring).unit(ring[0], None, Unit::CruiserI, 1, 1).unit(ring[0], None, Unit::FighterI, 1, 2).build();
        state.players_mut().by_seat_mut(Seat(1)).unwrap().econ.strategic_tokens = tokens;
        edit(&mut state, &ring);
        let mut r = Rig::from_parts(state, kernel, EmitEvent::new(finished(mover, ring[0], ships))).with_abilities(table());
        r.run();
        (r, ring)
    }

    fn destinations(r: &Rig<EmitEvent>) -> Vec<System> {
        r.opts().iter().filter_map(|o| if let Opt::Reaction(ROpt::G(GOpt::U(UOpt::Destination(s)))) = o { Some(*s) } else { None }).collect()
    }

    #[test]
    fn the_naalu_moves_its_ships_and_what_is_aboard_into_an_adjacent_system_and_places_the_token_there() {
        covers!("silent::g4::foresight::FORESIGHT");
        let (mut r, ring) = rig_ring(0, 1, 2, |_, _| {});
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the ability");
        r.pick(use_it);
        assert_eq!((r.asked().actor, r.asked().ask), (Seat(1), Ask::Reaction(RAsk::G(GAsk::U(UAsk::Foresight { from: ring[0] })))), "{}", r.transcript());
        let mut want = ring[1..].to_vec();
        want.sort();
        assert_eq!(destinations(&r), want, "the three neighbours");
        r.pick(1);
        assert!(r.is_done(), "{}", r.transcript());
        let to = want[1];
        let kinds = |system: System| board::space_units(&r.state, system).iter().filter(|p| p.owner() == Seat(1)).map(|p| p.kind()).collect::<Vec<_>>();
        assert_eq!(kinds(to), vec![Unit::CruiserI, Unit::FighterI, Unit::FighterI], "the ship, then what was aboard");
        assert_eq!(kinds(ring[0]), vec![]);
        assert!(board::has_token(&r.state, to, Seat(1)), "the strategy token is placed in the new system");
        assert_eq!(r.state.players().by_seat(Seat(1)).unwrap().econ.strategic_tokens, 1, "and spent from the pool");
        assert!(ti4_engine_core::kernel::fx::has_staged(&r.state), "the spend waits for the game step");
    }

    #[test]
    fn a_destination_with_the_naalu_token_or_another_players_ships_is_not_offered() {
        covers!("silent::g4::foresight::FORESIGHT");
        let (mut r, ring) = rig_ring(0, 1, 2, |s, ring| {
            ti4_engine_core::kernel::fx::place_command_token(s, Seat(1), ring[1]);
            ti4_engine_core::kernel::fx::place_unit(s, Seat(0), ring[2], None, Unit::CruiserI);
        });
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the ability");
        r.pick(use_it);
        assert_eq!(destinations(&r), vec![ring[3]], "only the neighbour without a token and without foreign ships: {}", r.transcript());
    }

    #[test]
    fn foresight_needs_another_players_move_a_strategy_token_and_a_destination() {
        covers!("silent::g4::foresight::FORESIGHT");
        let (own, _) = rig_ring(1, 1, 2, |_, _| {});
        assert!(own.is_done(), "the Naalu's own move: {}", own.transcript());
        let (nothing_came, _) = rig_ring(0, 0, 2, |_, _| {});
        assert!(nothing_came.is_done(), "no ship moved in: {}", nothing_came.transcript());
        let (broke, _) = rig_ring(0, 1, 0, |_, _| {});
        assert!(broke.is_done(), "no strategy token: {}", broke.transcript());
        let (boxed_in, _) = rig_ring(0, 1, 2, |s, ring| {
            for neighbour in &ring[1..] {
                ti4_engine_core::kernel::fx::place_command_token(s, Seat(1), *neighbour);
            }
        });
        assert!(boxed_in.is_done(), "every neighbour holds the token already: {}", boxed_in.transcript());
    }
}

// -- what the mutation check of the lane found -----------------------------------------------------------------------------------

mod regressions {
    use super::*;
    use ti4_engine_core::choice::reaction::Opt as ROpt;
    use ti4_engine_core::choice::silent::Opt as GOpt;
    use ti4_engine_core::choice::silent4::Opt as UOpt;
    use ti4_engine_core::generated::{Planet, System, Technology, Unit};
    use ti4_engine_core::hooks::movement::{ExtraWormholes, Pairs};
    use ti4_engine_core::ids::Seat;
    use ti4_engine_core::kernel::Kernel;
    use ti4_engine_core::state::Piece;

    // -- Foresight: the adjacency of the Naalu, the strategy token, the event ---------------------------------------------------

    fn ring7() -> Vec<System> {
        ti4_engine_core_tests::scenario::plain_systems()[..7].to_vec()
    }

    fn finished(mover: u8, system: System, ships: Option<i64>) -> ti4_engine_core::state::EventRec {
        let mut payload = Payload::new().with(Key::Player, Val::Seat(Seat(mover))).with(Key::System, Val::System(system));
        if let Some(n) = ships {
            payload = payload.with(Key::ShipsMoved, Val::Int(n));
        }
        fx::event(EventKind::MovementFinished, payload)
    }

    /// Seat 0 moved into `from`, a ring tile of a seven-system map where seat 1 (the Naalu) has a cruiser; `edit` changes the kernel.
    fn foresight_rig(tokens: i16, ships: Option<i64>, edit: impl FnOnce(&mut Kernel)) -> (Rig<EmitEvent>, Vec<System>, System) {
        let ring = ring7();
        let from = ring[1];
        let (mut state, mut kernel) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheNaaluCollective]).map_from(&ring).unit(from, None, Unit::CruiserI, 1, 1).build();
        state.players_mut().by_seat_mut(Seat(1)).unwrap().econ.strategic_tokens = tokens;
        edit(&mut kernel);
        let mut r = Rig::from_parts(state, kernel, EmitEvent::new(finished(0, from, ships))).with_abilities(table());
        r.run();
        (r, ring, from)
    }

    fn destinations(r: &Rig<EmitEvent>) -> Vec<System> {
        r.opts().iter().filter_map(|o| if let Opt::Reaction(ROpt::G(GOpt::U(UOpt::Destination(s)))) = o { Some(*s) } else { None }).collect()
    }

    fn open(r: &mut Rig<EmitEvent>) {
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the ability");
        r.pick(use_it);
    }

    #[test]
    fn one_strategy_token_is_enough_and_the_count_of_ships_must_be_in_the_event() {
        covers!("silent::g4::foresight::FORESIGHT");
        let (mut one, _, _) = foresight_rig(1, Some(1), |_| {});
        open(&mut one);
        assert!(!destinations(&one).is_empty(), "{}", one.transcript());
        let (silent, _, _) = foresight_rig(2, None, |_| {});
        assert!(silent.is_done(), "an event with no count of ships moved: {}", silent.transcript());
    }

    fn not_adjacent_to(r: &Rig<EmitEvent>, ring: &[System], from: System) -> System {
        let galaxy = r.start_state().galaxy().unwrap();
        *ring.iter().find(|s| **s != from && !galaxy.adjacent(from).contains(s.index())).expect("a ring tile that is not beside the first")
    }

    fn link_far(_: &CoreState, _: &Kernel, _: Seat) -> Pairs {
        let ring = ring7();
        // The tile not beside ring[1]: the first ring tile that is not adjacent to it (the map is the same every time).
        let (state, _) = Scenario::new(&[Faction::TheEmiratesOfHacan]).map_from(&ring).build();
        let galaxy = state.galaxy().unwrap();
        let far = *ring.iter().find(|s| **s != ring[1] && !galaxy.adjacent(ring[1]).contains(s.index())).unwrap();
        [(ring[1], far)].into_iter().collect()
    }

    fn extra_far(_: &CoreState) -> ExtraWormholes {
        let ring = ring7();
        let (state, _) = Scenario::new(&[Faction::TheEmiratesOfHacan]).map_from(&ring).build();
        let galaxy = state.galaxy().unwrap();
        let far = *ring.iter().find(|s| **s != ring[1] && !galaxy.adjacent(ring[1]).contains(s.index())).unwrap();
        [(ring[1], ti4_engine_core::generated::Wormhole::Beta), (far, ti4_engine_core::generated::Wormhole::Beta)].into_iter().collect()
    }

    fn block_first_border(_: &CoreState, _: Seat) -> Pairs {
        let ring = ring7();
        let (state, _) = Scenario::new(&[Faction::TheEmiratesOfHacan]).map_from(&ring).build();
        let galaxy = state.galaxy().unwrap();
        // The border of ring[1] with its first neighbour (both orders of the ids are used over the cases below).
        let next = galaxy.adjacent(ring[1]).iter().filter_map(System::from_index).next().unwrap();
        [(ring[1], next)].into_iter().collect()
    }

    #[test]
    fn the_links_the_wormholes_and_the_blocked_borders_of_the_faction_hooks_shape_the_neighbours() {
        covers!("silent::g4::foresight::FORESIGHT");
        let (mut plain, ring, from) = foresight_rig(2, Some(1), |_| {});
        open(&mut plain);
        let base = destinations(&plain);
        let far = not_adjacent_to(&plain, &ring, from);
        assert!(!base.contains(&far), "a tile across the ring is no neighbour");
        // A link of the faction hooks (the Creuss gate) adds it.
        let (mut linked, _, _) = foresight_rig(2, Some(1), |k| k.hooks.movement.linked_systems.push(link_far));
        open(&mut linked);
        assert!(destinations(&linked).contains(&far), "{}", linked.transcript());
        // So does a wormhole a module adds to both ends.
        let (mut worm, _, _) = foresight_rig(2, Some(1), |k| k.hooks.movement.extra_wormholes.push(extra_far));
        open(&mut worm);
        assert!(destinations(&worm).contains(&far), "{}", worm.transcript());
        // A blocked border takes its other end out of the list, whichever of the two ids is the smaller.
        let (mut blocked, _, _) = foresight_rig(2, Some(1), |k| k.hooks.movement.blocked_borders.push(block_first_border));
        open(&mut blocked);
        let galaxy = blocked.start_state().galaxy().unwrap();
        let next = galaxy.adjacent(from).iter().filter_map(System::from_index).next().unwrap();
        assert!(base.contains(&next) && !destinations(&blocked).contains(&next), "{}", blocked.transcript());
    }

    #[test]
    fn a_link_is_followed_from_either_end() {
        covers!("silent::g4::foresight::FORESIGHT");
        // The ships stand at the far end of the link: the near end is a destination (the pair is stored smaller first, so one of the two orders is this one).
        let ring = ring7();
        let (probe, _) = Scenario::new(&[Faction::TheEmiratesOfHacan]).map_from(&ring).build();
        let galaxy = probe.galaxy().unwrap();
        let far = *ring.iter().find(|s| **s != ring[1] && !galaxy.adjacent(ring[1]).contains(s.index())).unwrap();
        let (mut state, mut kernel) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheNaaluCollective]).map_from(&ring).unit(far, None, Unit::CruiserI, 1, 1).build();
        state.players_mut().by_seat_mut(Seat(1)).unwrap().econ.strategic_tokens = 2;
        kernel.hooks.movement.linked_systems.push(link_far);
        let mut r = Rig::from_parts(state, kernel, EmitEvent::new(finished(0, far, Some(1)))).with_abilities(table());
        r.run();
        open(&mut r);
        assert!(destinations(&r).contains(&ring[1]), "{}", r.transcript());
    }

    #[test]
    fn a_neighbour_off_the_map_is_not_a_destination_and_the_naalus_own_ship_there_does_not_bar_it() {
        covers!("silent::g4::foresight::FORESIGHT");
        // A system a link names that the map does not hold.
        fn link_off_map(_: &CoreState, _: &Kernel, _: Seat) -> Pairs {
            let ring = ring7();
            let off = ti4_engine_core_tests::scenario::plain_systems()[8];
            [(ring[1], off)].into_iter().collect()
        }
        let (mut off, _, _) = foresight_rig(2, Some(1), |k| k.hooks.movement.linked_systems.push(link_off_map));
        open(&mut off);
        assert!(!destinations(&off).contains(&ti4_engine_core_tests::scenario::plain_systems()[8]), "{}", off.transcript());
        // The Naalu's own ship in a neighbour is no other player's ship.
        let ring = ring7();
        let (mut own, _, from) = {
            let from = ring[1];
            let (mut state, kernel) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheNaaluCollective]).map_from(&ring).unit(from, None, Unit::CruiserI, 1, 1).unit(ring[0], None, Unit::CruiserI, 1, 1).build();
            state.players_mut().by_seat_mut(Seat(1)).unwrap().econ.strategic_tokens = 2;
            let mut r = Rig::from_parts(state, kernel, EmitEvent::new(finished(0, from, Some(1)))).with_abilities(table());
            r.run();
            (r, ring.clone(), from)
        };
        let _ = from;
        open(&mut own);
        assert!(destinations(&own).contains(&ring[0]), "the centre holds the Naalu's own cruiser: {}", own.transcript());
    }

    // -- Magen Defense Grid's hit: what it writes ----------------------------------------------------------------------------------

    fn started(system: System, planet: Planet) -> ti4_engine_core::state::EventRec {
        fx::event(EventKind::GroundCombatStarted, Payload::new().with(Key::System, Val::System(system)).with(Key::Planet, Val::Planet(planet)).with(Key::Attacker, Val::Seat(Seat(1))).with(Key::Defender, Val::Seat(Seat(0))))
    }

    /// A listener on the destroyed ground force that notes what the event says in the trade goods of seat 1: +1 for a galvanized unit, +10 for a damaged one, +100 for the event at all.
    fn note_destruction(eff: &mut ti4_engine_core::kernel::ability::Eff<'_, '_>) -> Result<(), ti4_engine_core::kernel::resolver::TimingError> {
        let p = eff.event.payload.clone();
        let gain = 100 + 10 * i32::from(p.flag(Key::Damaged) == Some(true)) + i32::from(p.flag(Key::Galvanized) == Some(true));
        fx::gain_trade_goods(eff.state(), Seat(1), gain);
        Ok(())
    }

    fn md_rig(unit: Unit, edit: impl FnOnce(&mut CoreState, Planet)) -> Rig<EmitEvent> {
        let system = ti4_engine_core_tests::scenario::plain_systems().into_iter().find(|s| !s.data().planets.is_empty()).unwrap();
        let planet = system.data().planets[0];
        let (mut state, kernel) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol]).control(planet, 0).unit(system, Some(planet), Unit::PDSI, 0, 1).unit(system, Some(planet), unit, 1, 1).build();
        state.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::MagenDefenseGridOmegaOmega);
        edit(&mut state, planet);
        let mut t = table();
        t.add(ti4_engine_core::kernel::ability::Spec::new(ti4_engine_core::kernel::ability::Slot(65_001), ti4_engine_core::choice::reaction::AbilityKind::Technology, EventKind::GroundForceDestroyed, ti4_engine_core::choice::reaction::Relation::After, note_destruction));
        let mut r = Rig::from_parts(state, kernel, EmitEvent::new(started(system, planet))).with_abilities(t);
        r.run();
        r
    }

    #[test]
    fn a_destroyed_spec_ops_is_counted_for_its_survival_roll_and_the_event_carries_its_state() {
        covers!("silent::g4::magen_hit::MD_HIT");
        let r = md_rig(Unit::SpecOpsII, |_, _| {});
        assert!(r.is_done(), "{}", r.transcript());
        assert_eq!(r.state.players().by_seat(Seat(1)).unwrap().misc.spec_ops_destroyed, 1);
        assert_eq!(r.state.players().by_seat(Seat(1)).unwrap().econ.trade_goods, 100 + r.start_state().players().by_seat(Seat(1)).unwrap().econ.trade_goods, "the event, undamaged and not galvanized");
        // A damaged and galvanized unit: the event says both.
        let r = md_rig(Unit::InfantryI, |s, planet| {
            let at = s.board().planet(planet).units.iter().position(|p| p.owner() == Seat(1)).unwrap();
            let piece: Piece = s.board().planet(planet).units.as_slice()[at].with_sustained(true).with_galvanized(true);
            s.board_mut().planet_mut(planet).units.as_mut_slice()[at] = piece;
        });
        assert!(r.is_done(), "{}", r.transcript());
        assert_eq!(r.state.players().by_seat(Seat(1)).unwrap().econ.trade_goods, 111 + r.start_state().players().by_seat(Seat(1)).unwrap().econ.trade_goods);
        assert_eq!(r.state.players().by_seat(Seat(1)).unwrap().misc.spec_ops_destroyed, 0);
    }

    // -- the placement ------------------------------------------------------------------------------------------------------------

    #[test]
    fn a_space_area_is_a_spot_only_for_the_seats_own_ships() {
        covers!("silent::g4::place::spots");
        use ti4_engine_core_factions::silent::g4::place::{spots, Target};
        let ring = ring7();
        let (state, _) = Scenario::new(&[Faction::TheYinBrotherhood, Faction::TheEmiratesOfHacan])
            .map_from(&ring)
            .unit(ring[1], None, Unit::CruiserI, 1, 1) // another player's ship
            .unit(ring[2], None, Unit::CruiserI, 0, 1) // the seat's own ship
            .build();
        let found = spots(&state, Seat(0), Target::ControlledPlanetOrShipSpace);
        assert_eq!(found, vec![(ring[2], None)]);
        assert!(spots(&state, Seat(0), Target::ControlledPlanet).is_empty());
    }

    #[test]
    fn nothing_is_asked_when_the_box_holds_none() {
        covers!("silent::g4::iconoclast::ICONOCLAST");
        use ti4_engine_core::tables::Scope;
        use ti4_engine_core::generated::UnitBaseType;
        use ti4_engine_core_factions::cd_registry::yin::timing::placed_form;
        let spots_systems: Vec<(System, Planet)> = ti4_engine_core_tests::scenario::plain_systems().into_iter().filter_map(|s| s.data().planets.first().map(|p| (s, *p))).take(2).collect();
        // Every Naalu mech stands on the board: Iconoclast asks nothing when another player gains a relic.
        let (probe, kernel) = Scenario::new(&[Faction::TheNaaluCollective, Faction::TheEmiratesOfHacan]).build();
        let mech = placed_form(&probe, Scope(u32::from(kernel.sources.0)), Seat(0), UnitBaseType::Mech).expect("the Naalu place a mech");
        let scenario = Scenario::new(&[Faction::TheNaaluCollective, Faction::TheEmiratesOfHacan]).control(spots_systems[0].1, 0).control(spots_systems[1].1, 0).unit(spots_systems[0].0, Some(spots_systems[0].1), mech, 0, 4);
        let (state, kernel) = scenario.build();
        let mut r = Rig::from_parts(state, kernel, EmitEvent::new(fx::event(EventKind::RelicGained, Payload::new().with(Key::Player, Val::Seat(Seat(1)))))).with_abilities(table());
        r.run();
        let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the mech");
        r.pick(use_it);
        assert!(r.is_done(), "no mech in the box, so no question and nothing placed: {}", r.transcript());
    }
}
