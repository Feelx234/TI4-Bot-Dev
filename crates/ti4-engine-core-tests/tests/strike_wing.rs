//! Lane H2: Strike Wing (Argent) as rules of the core table (`ti4_engine_core_factions::silent::strike_wing`): Strike Wing Alpha II, Strike Wing Ambuscade and
//! the Argent commander, all reading the staged roll of a unit ability (`transient.combat.reroll_staging`). The lockstep against the string is in `ti4-engine`
//! (`combat_live`: the barrage and the space cannon offense); these tests pin the rules and the conditions the lockstep does not reach.

use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt};
use ti4_engine_core::choice::silent::{Ask as GAsk, DieSource, Opt as GOpt};
use ti4_engine_core::containers::SortedMap;
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::state::{Code, EventKind, Key, LeaderStatus, NoteKey, Payload, RerollEntry, RerollSet, RollPurpose, Val};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::windows::EmitEvent;

fn table() -> AbilityTable {
    let mut t = AbilityTable::default();
    ti4_engine_core_factions::silent::register_abilities(&mut t);
    t
}

const SYSTEM: System = System::MecatolRex18;
const BARRAGE: Code = Code(1);

/// A staged entry of `unit` (typed), its text `Code(10 + unit)` and the faces.
fn entry(unit: Option<Unit>, on: u8, faces: &[u8]) -> RerollEntry {
    let code = Code(10 + unit.map_or(0, |u| u as u16));
    let mut unit_types = SortedMap::default();
    if unit.is_some() {
        unit_types.insert(code, 1);
    }
    RerollEntry { unit: code, planet: None, hits_on: Some(on), faces: faces.to_vec(), rerolled: 0, deltas: SortedMap::default(), unit_types, kind: unit }
}

fn stage(s: &mut CoreState, seat: u8, entries: Vec<RerollEntry>) {
    s.transient_mut().combat.reroll_staging.edit().push((Seat(seat), RerollSet { kind: BARRAGE, system: SYSTEM, rolls: entries }));
}

fn rolled(seat: u8) -> ti4_engine_core::state::EventRec {
    fx::event(EventKind::UnitAbilityRolled, Payload::new().with(Key::Player, Val::Seat(Seat(seat))).with(Key::System, Val::System(SYSTEM)))
}

fn rig(scenario: Scenario, edit: impl FnOnce(&mut CoreState), event: ti4_engine_core::state::EventRec) -> Rig<EmitEvent> {
    let (mut state, kernel) = scenario.build();
    edit(&mut state);
    let mut r = Rig::from_parts(state, kernel, EmitEvent::new(event)).with_abilities(table());
    r.run();
    r
}

const ARGENT: [Faction; 2] = [Faction::TheArgentFlight, Faction::TheEmiratesOfHacan];

/// The Argent (seat 0) with the technology and a fleet in the system, the opponent's ships.
fn swa2_scenario() -> Scenario {
    Scenario::new(&ARGENT).tech(0, Technology::StrikeWingAlphaII).unit(SYSTEM, None, Unit::StrikeWingAlphaII, 0, 3).unit(SYSTEM, None, Unit::FighterI, 1, 3)
}

/// The opponent's infantry riding in the space area (the scenario places ground forces on planets only).
fn with_infantry(infantry: usize, infantry2: usize) -> impl FnOnce(&mut CoreState) {
    move |s| {
        for (unit, n) in [(Unit::InfantryI, infantry), (Unit::InfantryII, infantry2)] {
            for _ in 0..n {
                s.board_mut().sys_mut(SYSTEM).units.push(ti4_engine_core::state::Piece::new(unit, Seat(1)));
            }
        }
    }
}

/// `rig` with the infantry of `with_infantry` and the staged dice `alpha_dice`.
fn swa2_rig(infantry: usize, infantry2: usize, faces: &[u8], event_seat: u8) -> Rig<EmitEvent> {
    let infantry = with_infantry(infantry, infantry2);
    rig(swa2_scenario(), |s| {
        infantry(s);
        stage(s, 0, vec![entry(Some(Unit::StrikeWingAlphaII), 6, faces)]);
    }, rolled(event_seat))
}

fn infantry_left(r: &Rig<EmitEvent>, unit: Unit) -> usize {
    r.state.board().sys(SYSTEM).units.iter().filter(|p| p.owner() == Seat(1) && p.kind() == unit).count()
}

// -- Strike Wing Alpha II ---------------------------------------------------------------------------------------------------------

#[test]
fn each_nine_or_ten_of_the_alpha_ii_dice_destroys_one_infantry_of_the_space_area() {
    covers!("silent::strike_wing::SWA2");
    // Three results of 9 or 10 (an 8 and a 1 do nothing) against five infantry of one type: three are gone, no question.
    let r = swa2_rig(5, 0, &[9, 10, 8, 1, 10], 0);
    assert!(r.is_done(), "mandatory, nothing is asked: {}", r.transcript());
    assert_eq!(infantry_left(&r, Unit::InfantryI), 2);
    // One result of 9 is one hit.
    assert_eq!(infantry_left(&swa2_rig(4, 0, &[9], 0), Unit::InfantryI), 3);
    // More hits than infantry: they all go and the rest is lost.
    let all = swa2_rig(2, 0, &[9, 9, 10, 10], 0);
    assert_eq!(infantry_left(&all, Unit::InfantryI), 0);
}

#[test]
fn the_alpha_ii_asks_which_type_when_the_opponent_has_two_and_asks_again_for_each_hit() {
    covers!("silent::strike_wing::SWA2");
    let mut r = swa2_rig(1, 2, &[9, 10, 4], 0);
    let Ask::Reaction(RAsk::G(GAsk::StrikeWing)) = r.asked().ask else { panic!("{}", r.transcript()) };
    assert_eq!(r.asked().actor, Seat(0));
    let ctx = &r.asked().ctx;
    assert!(!ctx.optional && !ctx.space_battle && ctx.target.is_none() && ctx.invasion_seq.is_none() && ctx.outstanding.is_empty(), "the string's context is bare");
    assert_eq!(r.opts(), vec![Opt::Reaction(ROpt::G(GOpt::Infantry { unit: Unit::InfantryI, victim: Seat(1) })), Opt::Reaction(ROpt::G(GOpt::Infantry { unit: Unit::InfantryII, victim: Seat(1) }))]);
    r.pick_opt(Opt::Reaction(ROpt::G(GOpt::Infantry { unit: Unit::InfantryII, victim: Seat(1) })));
    // The second hit asks again (both types still stand).
    let Ask::Reaction(RAsk::G(GAsk::StrikeWing)) = r.asked().ask else { panic!("{}", r.transcript()) };
    r.pick_opt(Opt::Reaction(ROpt::G(GOpt::Infantry { unit: Unit::InfantryII, victim: Seat(1) })));
    assert!(r.is_done());
    assert_eq!((infantry_left(&r, Unit::InfantryI), infantry_left(&r, Unit::InfantryII)), (1, 0));
    // The last infantry of a type gone, the other type is the only choice: no question for the next hit.
    let mut last = swa2_rig(1, 1, &[9, 10], 0);
    last.pick_opt(Opt::Reaction(ROpt::G(GOpt::Infantry { unit: Unit::InfantryI, victim: Seat(1) })));
    assert!(last.is_done(), "{}", last.transcript());
    assert_eq!((infantry_left(&last, Unit::InfantryI), infantry_left(&last, Unit::InfantryII)), (0, 0));
}

#[test]
fn the_alpha_ii_needs_the_technology_a_nine_on_its_own_dice_its_own_roll_and_infantry_to_destroy() {
    covers!("silent::strike_wing::SWA2");
    let untouched = |r: &Rig<EmitEvent>| infantry_left(r, Unit::InfantryI) == 3;
    assert!(untouched(&swa2_rig(3, 0, &[8, 3, 1], 0)), "no 9 or 10");
    assert!(untouched(&swa2_rig(3, 0, &[9, 10], 1)), "another seat's roll");
    let (mut s, k) = swa2_scenario().build();
    with_infantry(3, 0)(&mut s);
    s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned = Default::default();
    stage(&mut s, 0, vec![entry(Some(Unit::StrikeWingAlphaII), 6, &[9, 10])]);
    let mut without = Rig::from_parts(s, k, EmitEvent::new(rolled(0))).with_abilities(table());
    without.run();
    assert!(untouched(&without), "the technology is not owned");
    // The nines of another unit's dice (a Destroyer II also barrages) do nothing.
    let other = rig(swa2_scenario(), |s| {
        with_infantry(3, 0)(s);
        stage(s, 0, vec![entry(Some(Unit::DestroyerII), 6, &[9, 10])]);
    }, rolled(0));
    assert!(untouched(&other), "only the Alpha II's dice count");
    // Nothing staged at all.
    assert!(untouched(&rig(swa2_scenario(), with_infantry(3, 0), rolled(0))), "no staged roll");
    // No infantry to destroy: nothing is offered and nothing breaks.
    let none = swa2_rig(0, 0, &[9, 10], 0);
    assert!(none.is_done());
    // No opposing ship: the opponent is nobody's.
    let (mut s, k) = Scenario::new(&ARGENT).tech(0, Technology::StrikeWingAlphaII).build();
    with_infantry(3, 0)(&mut s);
    stage(&mut s, 0, vec![entry(Some(Unit::StrikeWingAlphaII), 6, &[9])]);
    let mut shipless = Rig::from_parts(s, k, EmitEvent::new(rolled(0))).with_abilities(table());
    shipless.run();
    assert_eq!(infantry_left(&shipless, Unit::InfantryI), 3, "the string takes the opponent from the first of their ships");
    // Another entry that also rolled for the Alpha II adds its nines to the count.
    let two = rig(swa2_scenario(), |s| {
        with_infantry(5, 0)(s);
        stage(s, 0, vec![entry(Some(Unit::StrikeWingAlphaII), 6, &[9]), entry(Some(Unit::DestroyerII), 6, &[10]), entry(Some(Unit::StrikeWingAlphaII), 6, &[10, 9])]);
    }, rolled(0));
    assert_eq!(infantry_left(&two, Unit::InfantryI), 2, "three nines or tens on Alpha II entries");
}

// -- the additional die ----------------------------------------------------------------------------------------------------------

fn commander_scenario() -> Scenario {
    Scenario::new(&ARGENT).unit(SYSTEM, None, Unit::FighterI, 1, 3)
}

fn unlock_commander(s: &mut CoreState) {
    s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::TrrakanAunZulok, LeaderStatus::Unlocked);
}

fn faces_of(r: &Rig<EmitEvent>, entry: usize) -> Vec<u8> {
    r.state.transient().combat.reroll_staging.iter().find(|(s, _)| *s == Seat(0)).map(|(_, set)| set.rolls[entry].faces.clone()).unwrap()
}

fn use_it(r: &mut Rig<EmitEvent>) {
    let at = r.opts().iter().position(|o| *o != Opt::Decline).expect("the ability is offered");
    r.pick(at);
}

#[test]
fn the_commander_gives_the_only_rolling_unit_one_more_die_and_declining_changes_nothing() {
    covers!("silent::strike_wing::COMMANDER");
    let stage_one = |s: &mut CoreState| {
        unlock_commander(s);
        stage(s, 0, vec![entry(Some(Unit::DestroyerI), 9, &[2, 3])]);
    };
    let mut r = rig(commander_scenario(), stage_one, rolled(0));
    assert!(matches!(r.asked().ask, Ask::Reaction(RAsk::Window { .. })), "optional: {}", r.transcript());
    use_it(&mut r);
    assert!(r.is_done(), "one unit: no question about which: {}", r.transcript());
    let faces = faces_of(&r, 0);
    assert_eq!((faces.len(), &faces[..2]), (3, &[2, 3][..]), "the new die is appended after the rolled ones");
    assert!(r.state.ledger().dice.iter().any(|row| row.purpose == RollPurpose::Ability(ti4_engine_core_factions::silent::strike_wing::COMMANDER.0) && row.owner == Some(Seat(0)) && row.hits_on == Some(9) && row.faces.len() == 1), "the draw is in the ledger as the ability's");
    let mut declined = rig(commander_scenario(), stage_one, rolled(0));
    declined.decline();
    assert!(declined.is_done());
    assert_eq!(faces_of(&declined, 0), vec![2, 3]);
}

#[test]
fn the_commander_asks_which_of_two_rolling_unit_types_takes_the_die_and_skips_the_relics_dice() {
    covers!("silent::strike_wing::COMMANDER");
    let edit = |s: &mut CoreState| {
        unlock_commander(s);
        // The relic's dice name no unit and are never a choice; the second entry of a type already listed is not a second choice.
        stage(s, 0, vec![entry(None, 7, &[1]), entry(Some(Unit::DestroyerI), 9, &[2]), entry(Some(Unit::DestroyerII), 6, &[3, 4]), entry(Some(Unit::DestroyerI), 9, &[5])]);
    };
    let mut r = rig(commander_scenario(), edit, rolled(0));
    use_it(&mut r);
    let Ask::Reaction(RAsk::G(GAsk::ExtraDie(DieSource::Commander))) = r.asked().ask else { panic!("{}", r.transcript()) };
    let ctx = &r.asked().ctx;
    assert!(!ctx.optional && !ctx.space_battle && ctx.target.is_none() && ctx.invasion_seq.is_none() && ctx.outstanding.is_empty(), "the string's context is bare");
    assert_eq!(r.opts(), vec![Opt::Reaction(ROpt::G(GOpt::ExtraDie { entry: 1, unit: Unit::DestroyerI })), Opt::Reaction(ROpt::G(GOpt::ExtraDie { entry: 2, unit: Unit::DestroyerII }))]);
    r.pick_opt(Opt::Reaction(ROpt::G(GOpt::ExtraDie { entry: 2, unit: Unit::DestroyerII })));
    assert!(r.is_done());
    assert_eq!((faces_of(&r, 1), faces_of(&r, 2).len(), faces_of(&r, 3), faces_of(&r, 0)), (vec![2], 3, vec![5], vec![1]), "only the chosen entry took the die");
}

#[test]
fn the_commander_is_offered_to_its_holder_with_a_unit_that_rolled_only() {
    covers!("silent::strike_wing::COMMANDER");
    let rolled_destroyer = |s: &mut CoreState| stage(s, 0, vec![entry(Some(Unit::DestroyerI), 9, &[2])]);
    assert!(rig(commander_scenario(), rolled_destroyer, rolled(0)).is_done(), "the commander is still locked");
    let relic_only = |s: &mut CoreState| {
        unlock_commander(s);
        stage(s, 0, vec![entry(None, 7, &[1])]);
    };
    assert!(rig(commander_scenario(), relic_only, rolled(0)).is_done(), "only the relic's dice rolled");
    let other_seat = |s: &mut CoreState| {
        unlock_commander(s);
        rolled_destroyer(s);
    };
    assert!(rig(commander_scenario(), other_seat, rolled(1)).is_done(), "another seat's roll");
    assert!(rig(commander_scenario(), |s| unlock_commander(s), rolled(0)).is_done(), "nothing staged");
}

const NOTE: NoteKey = NoteKey { note: PromissoryNote::StrikeWingAmbuscade, faction: Faction::TheArgentFlight };

#[test]
fn ambuscade_gives_one_unit_a_die_and_the_note_goes_home_to_the_argent() {
    covers!("silent::strike_wing::AMBUSCADE");
    let factions = [Faction::TheEmiratesOfHacan, Faction::TheArgentFlight];
    let scenario = || Scenario::new(&factions).unit(SYSTEM, None, Unit::FighterI, 1, 3);
    let hold = |s: &mut CoreState| {
        s.cards_mut().promissory.edit().insert(NOTE, Seat(0));
        stage(s, 0, vec![entry(Some(Unit::DestroyerI), 9, &[2, 3])]);
    };
    let mut r = rig(scenario(), hold, rolled(0));
    use_it(&mut r);
    assert!(r.is_done(), "{}", r.transcript());
    assert_eq!(faces_of(&r, 0).len(), 3);
    assert_eq!(r.state.cards().promissory.get(&NOTE), Some(&Seat(1)), "the note is back with the Argent");
    // Declined: the note stays with its holder and the dice are as they were.
    let mut declined = rig(scenario(), hold, rolled(0));
    declined.decline();
    assert_eq!((faces_of(&declined, 0).len(), declined.state.cards().promissory.get(&NOTE)), (2, Some(&Seat(0))));
    // The card with its owner, not played; held by somebody who did not roll: nothing is offered.
    assert!(rig(scenario(), |s| {
        s.cards_mut().promissory.edit().insert(NOTE, Seat(1));
        stage(s, 1, vec![entry(Some(Unit::DestroyerI), 9, &[2])]);
    }, rolled(1)).is_done(), "the Argent never plays its own note");
    assert!(rig(scenario(), |s| {
        s.cards_mut().promissory.edit().insert(NOTE, Seat(0));
        stage(s, 1, vec![entry(Some(Unit::DestroyerI), 9, &[2])]);
    }, rolled(1)).is_done(), "the holder's units did not roll");
}

// -- the hosts stage the unit of each entry ----------------------------------------------------------------------------------------

mod hosts {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
    use ti4_engine_core::kernel::ability::{Eff, Slot, Spec};
    use ti4_engine_core::kernel::resolver::TimingError;
    use ti4_engine_core::rules::combat::barrage::{BarrageStep, Facts as BarrageFacts};
    use ti4_engine_core::rules::combat::space_cannon_offense::{Facts as CannonFacts, SpaceCannonOffense};
    use ti4_engine_core_tests::rig::Policy;
    use ti4_engine_core_tests::scenario::plain_systems;

    static SEEN: AtomicUsize = AtomicUsize::new(0);

    fn sys() -> System {
        plain_systems().into_iter().find(|s| s.data().planets.len() >= 2).expect("a tile with two planets")
    }

    /// Looks at the staged set of the roller while the event goes out and checks it names the unit behind every entry the way the string does.
    fn inspect(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
        let roller = eff.state().transient().combat.last_reroll_player.expect("the roller is named");
        let set = eff.state().transient().combat.reroll_staging.iter().find(|(s, _)| *s == roller).map(|(_, set)| set.clone()).expect("staged");
        for e in &set.rolls {
            match e.kind {
                // A unit's entry: one type, its text once (a live position has no unit texts: the entry's own code stands for the type).
                Some(_) => assert_eq!(e.unit_types.iter().map(|(code, n)| (*code, *n)).collect::<Vec<_>>(), vec![(e.unit, 1)], "the type behind the entry is its own code"),
                None => assert!(e.unit_types.is_empty(), "an attachment's cannon names no unit"),
            }
            SEEN.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }

    fn table() -> AbilityTable {
        let mut t = AbilityTable::default();
        t.add(Spec::new(Slot(9_002), AbilityKind::Other, EventKind::UnitAbilityRolled, Relation::After, inspect));
        t
    }

    #[test]
    fn the_barrage_stages_the_typed_unit_of_each_entry() {
        covers!("rules::combat::barrage::BarrageStep");
        let facts = BarrageFacts {
            attacker: Seat(0),
            defender: Seat(1),
            system: sys(),
            round: 1,
            occurrence: 7,
            staging: false,
            announce: true,
            phase: Code(1),
            cause: Code(2),
            kind: Code(3),
            extra: Code(4),
            unit_codes: vec![(Unit::DestroyerI, Code(10)), (Unit::DestroyerII, Code(11))],
            raid: None,
            commander: [false, false],
            shown: vec![(Unit::DestroyerI, Code(10)), (Unit::DestroyerII, Code(11))],
            extra_shown: Code(12),
            hit_texts: ti4_engine_core::kernel::children::HitTexts { staging: false, commander: Code(13), casualty: Code(14), effect_hit: Code(15) },
        };
        let arena = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
            .map_from(&[sys()])
            .unit(sys(), None, Unit::DestroyerI, 0, 2)
            .unit(sys(), None, Unit::DestroyerII, 0, 1)
            .unit(sys(), None, Unit::FighterI, 1, 4);
        let before = SEEN.load(Ordering::SeqCst);
        let mut rig = Rig::new(arena, BarrageStep::new(facts)).seeded(3).with_abilities(table());
        rig.run();
        assert!(rig.is_done());
        // (Both types of the attacker are staged: two entries. The defender has no destroyer, hence no dice.)
        assert!(SEEN.load(Ordering::SeqCst) >= before + 2, "the inspector saw the two entries of the attacker");
        let typed: Vec<Option<Unit>> = rig.state.ledger().dice.iter().filter(|r| r.purpose == RollPurpose::AntiFighterBarrage).map(|r| r.unit).collect();
        assert!(!typed.is_empty());
    }

    #[test]
    fn the_space_cannon_offense_stages_the_typed_unit_of_each_entry() {
        covers!("rules::combat::space_cannon_offense::SpaceCannonOffense");
        let p1 = sys().data().planets[0];
        let arena = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
            .map_from(&[sys()])
            .unit(sys(), Some(p1), Unit::PDSI, 1, 2)
            .unit(sys(), Some(p1), Unit::PDSII, 1, 1)
            .control(p1, 1)
            .unit(sys(), None, Unit::CruiserI, 0, 6);
        let facts = CannonFacts { active: Seat(0), system: sys(), announce: true, staging: false, reaching: vec![], mapped: false, kind: Code(1), cause: Code(2), casualty: Code(3), commander: Code(4), jolnar: vec![], unit_texts: vec![], planet_texts: vec![] };
        let before = SEEN.load(Ordering::SeqCst);
        let mut rig = Rig::new(arena, SpaceCannonOffense::new(facts)).seeded(5).with_abilities(table());
        rig.run();
        rig.auto(Policy::First);
        assert!(rig.is_done());
        assert!(SEEN.load(Ordering::SeqCst) >= before + 2, "the inspector saw an entry per PDS type");
    }
}
