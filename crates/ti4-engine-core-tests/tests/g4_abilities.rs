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
