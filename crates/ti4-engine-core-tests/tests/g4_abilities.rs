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
    ti4_engine_core_factions::silent::register_abilities(&mut t);
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
    fn a_replaced_research_asks_the_pool_three_times_and_settles_one_pending_research() {
        covers!("silent::g4::propagation::PROPAGATION");
        let mut r = rig(Scenario::new(&[Faction::TheNekroVirus, Faction::TheEmiratesOfHacan]), owes(0, 2), announced(0));
        let before: Vec<i16> = [Pool::Tactic, Pool::Fleet, Pool::Strategic].iter().map(|p| pool_of(&r, 0, *p)).collect();
        for left in [3_u8, 2, 1] {
            assert_eq!(r.asked().actor, Seat(0), "{}", r.transcript());
            assert_eq!(r.asked().ask, Ask::Strategy(SAsk::GainPool { left }), "{}", r.transcript());
            // Tactic, Fleet, Strategic in this order.
            let at = r.opts().iter().position(|o| *o == Opt::Strategy(SOpt::Pool(Pool::Fleet))).expect("the fleet pool");
            r.pick(at);
        }
        assert!(r.is_done(), "{}", r.transcript());
        assert_eq!(pool_of(&r, 0, Pool::Fleet) - before[1], 3);
        assert_eq!((pool_of(&r, 0, Pool::Tactic), pool_of(&r, 0, Pool::Strategic)), (before[0], before[2]));
        assert_eq!(pending(&r, 0), Some(MarkVal::Atom(Atom::Num(1))), "one research is settled, one is left for the next announcement");
        // The last pending research removes the mark.
        let mut last = rig(Scenario::new(&[Faction::TheNekroVirus, Faction::TheEmiratesOfHacan]), owes(0, 1), announced(0));
        for _ in 0..3 {
            last.pick(0);
        }
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
