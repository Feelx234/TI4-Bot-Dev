//! D4-2d: SPACE CANNON (LRR 77): who fires (`cannon_guns`), how they roll (`cannon_plan`: Plasma Scoring, Antimass Deflectors, Disable), the
//! offense window (`SpaceCannonOffense`: dice, hits, Graviton Laser System, SUSTAIN DAMAGE, the feat) and `Absorb` (cancellations, sustain, casualties).
//! Dice come from the seeded stream: the faces the window drew (`drawn`) say what the hits must be.

use ti4_engine_core::choice::combat::{Ask as CAsk, Opt as COpt};
use ti4_engine_core::kernel::children::HitTexts;
use ti4_engine_core::kernel::hits;
use ti4_engine_core::query::board as qboard;
use ti4_engine_core::rules::combat::cannon_guns::{local_guns, opponent_with_ships, unit_guns};
use ti4_engine_core::rules::combat::effect_hits::Absorb;
use ti4_engine_core::rules::combat::space_cannon_offense::{Facts, SpaceCannonOffense};
use ti4_engine_core::rules::combat::{cannon_plan, CannonRoll, Rd};
use ti4_engine_core::state::{Code, EventKind, Feat, Key, RoundKind};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::rig::Policy;
use ti4_engine_core_tests::scenario::plain_systems;

/// A plain tile with two planets (the centre of the test map).
fn sys() -> System {
    plain_systems().into_iter().find(|s| s.data().planets.len() >= 2).expect("a tile with two planets")
}

/// A plain tile next to it.
fn dest() -> System {
    plain_systems().into_iter().find(|s| *s != sys()).expect("another tile")
}

fn planets() -> (Planet, Planet) {
    let p = sys().data().planets;
    (p[0], p[1])
}

fn arena() -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys(), dest()])
}

fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
    Rd::new(s, k)
}

// ---- the plan ----------------------------------------------------------------------------------

#[test]
fn a_pds_rolls_its_printed_number_and_units_without_space_cannon_do_not_roll() {
    covers!("rules::combat::cannon_plan");
    let (state, kernel) = arena().build();
    let guns = [(Seat(1), Unit::PDSI), (Seat(1), Unit::PDSII), (Seat(1), Unit::DreadnoughtI), (Seat(1), Unit::InfantryI)];
    let plan = cannon_plan(rd(&state, &kernel), sys(), Seat(0), Some(Seat(1)), &guns);
    assert_eq!(
        plan,
        vec![CannonRoll { owner: Seat(1), unit: Unit::PDSI, hits_on: 6, dice: 1 }, CannonRoll { owner: Seat(1), unit: Unit::PDSII, hits_on: 5, dice: 1 }],
        "PDS: SPACE CANNON 6; PDS II: 5; dreadnoughts and infantry have none"
    );
}

#[test]
fn plasma_scoring_gives_one_extra_die_to_the_unit_that_hits_most_easily() {
    covers!("rules::combat::cannon_plan");
    let (state, kernel) = arena().tech(1, Technology::PlasmaScoring).build();
    let guns = [(Seat(1), Unit::PDSI), (Seat(1), Unit::PDSII), (Seat(1), Unit::PDSI)];
    let plan = cannon_plan(rd(&state, &kernel), sys(), Seat(0), Some(Seat(1)), &guns);
    let dice: Vec<usize> = plan.iter().map(|r| r.dice).collect();
    assert_eq!(dice, vec![1, 2, 1], "one unit of the owner rolls one more die: the PDS II (5 beats 6)");
    // Only the owner of the technology profits.
    let (other, kernel) = arena().tech(0, Technology::PlasmaScoring).build();
    let plan = cannon_plan(rd(&other, &kernel), sys(), Seat(0), Some(Seat(1)), &guns);
    assert_eq!(plan.iter().map(|r| r.dice).collect::<Vec<_>>(), vec![1, 1, 1]);
}

#[test]
fn antimass_deflectors_make_the_cannon_miss_more_but_only_against_their_owner_and_from_version_15() {
    covers!("rules::combat::cannon_plan");
    let guns = [(Seat(1), Unit::PDSI)];
    let (state, kernel) = arena().tech(0, Technology::AntimassDeflectors).build();
    let plan = cannon_plan(rd(&state, &kernel), sys(), Seat(0), Some(Seat(1)), &guns);
    assert_eq!(plan[0].hits_on, 7, "-1 to the results: a 6 needs one more");
    // The deflectors of the gun's own owner do nothing for its own shots.
    let (own, kernel) = arena().tech(1, Technology::AntimassDeflectors).build();
    assert_eq!(cannon_plan(rd(&own, &kernel), sys(), Seat(0), Some(Seat(1)), &guns)[0].hits_on, 6);
    // Older rules versions did not apply the effect.
    let (old, kernel) = arena().rules(14).tech(0, Technology::AntimassDeflectors).build();
    assert_eq!(cannon_plan(rd(&old, &kernel), sys(), Seat(0), Some(Seat(1)), &guns)[0].hits_on, 6);
}

#[test]
fn a_played_disable_silences_the_pds_of_the_others() {
    covers!("rules::combat::cannon_plan");
    let guns = [(Seat(1), Unit::PDSI), (Seat(0), Unit::PDSI)];
    let (mut state, kernel) = arena().build();
    state.meta_mut().activation_seq = 4;
    state.players_mut().by_seat_mut(Seat(0)).unwrap().round_marks.push(RoundKind::DisableInvasion, 4);
    let plan = cannon_plan(rd(&state, &kernel), sys(), Seat(0), Some(Seat(1)), &guns);
    assert_eq!(plan.iter().map(|r| r.owner).collect::<Vec<_>>(), vec![Seat(0)], "the player of Disable keeps their own PDS; the others' are silenced");
    // A mark of another activation is not this one.
    state.meta_mut().activation_seq = 5;
    let plan = cannon_plan(rd(&state, &kernel), sys(), Seat(0), Some(Seat(1)), &guns);
    assert_eq!(plan.len(), 2);
}

// ---- who fires ---------------------------------------------------------------------------------

#[test]
fn the_defenders_guns_fire_at_the_active_player_and_the_active_players_only_at_a_ship() {
    covers!("rules::combat::cannon_guns");
    let (p1, _) = planets();
    let sc = arena().unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), Some(p1), Unit::PDSI, 1, 1).control(p1, 1);
    let (state, kernel) = sc.clone().build();
    let guns = local_guns(rd(&state, &kernel), sys(), Seat(0));
    assert_eq!(guns.iter().map(|p| (p.owner(), p.kind())).collect::<Vec<_>>(), vec![(Seat(1), Unit::PDSI)], "no ship of the opponent in space: the active player's units do not fire");
    assert_eq!(opponent_with_ships(rd(&state, &kernel), Seat(0), sys()), None);
    // An opposing ship in space lets the active player's units fire too, space units first, then the planets.
    let with_ship = sc.unit(sys(), None, Unit::CruiserI, 1, 1);
    let (state, kernel) = with_ship.build();
    assert_eq!(opponent_with_ships(rd(&state, &kernel), Seat(0), sys()), Some(Seat(1)));
    let guns = local_guns(rd(&state, &kernel), sys(), Seat(0));
    let order: Vec<(Seat, Unit)> = guns.iter().map(|p| (p.owner(), p.kind())).collect();
    assert_eq!(order, vec![(Seat(0), Unit::CruiserI), (Seat(1), Unit::CruiserI), (Seat(1), Unit::PDSI)]);
}

#[test]
fn a_solar_flare_silences_everyone_but_the_active_player() {
    covers!("rules::combat::cannon_guns");
    let (p1, _) = planets();
    let (mut state, kernel) = arena().unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::CruiserI, 1, 1).unit(sys(), Some(p1), Unit::PDSI, 1, 1).control(p1, 1).build();
    state.meta_mut().activation_seq = 2;
    state.players_mut().by_seat_mut(Seat(0)).unwrap().round_marks.push(RoundKind::SolarFlare, 2);
    let guns = local_guns(rd(&state, &kernel), sys(), Seat(0));
    assert_eq!(guns.iter().map(|p| p.owner()).collect::<Vec<_>>(), vec![Seat(0)]);
}

#[test]
fn pds_two_reaches_into_an_adjacent_system_when_the_card_says_so() {
    covers!("rules::combat::cannon_guns");
    let next = dest().data().planets[0];
    let sc = arena().unit(sys(), None, Unit::CruiserI, 0, 1).unit(dest(), Some(next), Unit::PDSII, 1, 1).control(next, 1);
    let (state, kernel) = sc.build();
    let adj = qboard::adjacency(&state).expect("a map");
    let far = unit_guns(rd(&state, &kernel), sys(), Seat(0), Some(&adj), &[]);
    assert!(far.is_empty(), "no unit reaches: only the local guns");
    let reach = unit_guns(rd(&state, &kernel), sys(), Seat(0), Some(&adj), &[Unit::PDSII]);
    assert_eq!(reach.iter().map(|p| p.kind()).collect::<Vec<_>>(), vec![Unit::PDSII]);
    let unmapped = unit_guns(rd(&state, &kernel), sys(), Seat(0), None, &[Unit::PDSII]);
    assert!(unmapped.is_empty(), "without a map nothing is looked up");
}

// ---- the offense -------------------------------------------------------------------------------

fn facts() -> Facts {
    Facts { active: Seat(0), system: sys(), announce: true, staging: false, reaching: vec![], mapped: false, kind: Code(1), cause: Code(2), casualty: Code(3), commander: Code(4) }
}

fn hits_of(rig: &Rig<SpaceCannonOffense>) -> usize {
    rig.driver.window.drawn.iter().map(|(on, faces)| faces.iter().filter(|f| **f >= *on).count()).sum()
}

fn guns_and_fleet() -> Scenario {
    let (p1, _) = planets();
    arena().unit(sys(), Some(p1), Unit::PDSI, 1, 3).control(p1, 1).unit(sys(), None, Unit::CruiserI, 0, 2).unit(sys(), None, Unit::DestroyerI, 0, 1)
}

#[test]
fn each_hit_of_the_guns_costs_the_active_player_a_ship_and_taking_the_last_non_fighters_scores() {
    covers!("rules::combat::space_cannon_offense::SpaceCannonOffense");
    let mut counts = [0usize; 4];
    for seed in 0..40u64 {
        let mut rig = Rig::new(guns_and_fleet(), SpaceCannonOffense::new(facts())).seeded(seed);
        rig.run();
        rig.auto(Policy::First);
        assert!(rig.is_done());
        let drawn = rig.driver.window.drawn.clone();
        assert_eq!(drawn.len(), 3, "three PDS, one entry each");
        assert!(drawn.iter().all(|(on, faces)| *on == 6 && faces.len() == 1));
        let hits = hits_of(&rig);
        counts[hits] += 1;
        let left = rig.state.board().sys(sys()).units.iter().filter(|p| p.owner() == Seat(0)).count();
        assert_eq!(left, 3usize.saturating_sub(hits), "seed {seed}: one ship per hit");
        assert_eq!(rig.state.board().sys(sys()).units.iter().filter(|p| p.owner() == Seat(1)).count(), 0);
        let kinds: Vec<EventKind> = rig.driver.emitted.iter().map(|e| e.kind).collect();
        let rolled = rig.driver.emitted.iter().filter(|e| e.kind == EventKind::UnitAbilityRolled).count();
        if hits > 0 {
            assert_eq!(rolled, 1, "the gunner that scored reports its roll");
            let hit_ev = rig.driver.emitted.iter().find(|e| e.kind == EventKind::SpaceCannonHits).expect("the hits window");
            assert_eq!((hit_ev.payload.seat(Key::Gunner), hit_ev.payload.seat(Key::Player), hit_ev.payload.int(Key::Hits)), (Some(Seat(1)), Some(Seat(0)), Some(hits as i64)));
        } else {
            assert_eq!(rolled, 0);
            assert!(!kinds.contains(&EventKind::SpaceCannonHits), "no hit, no window");
        }
        assert_eq!(rig.state.ledger().dice.len(), 3, "every roll is recorded in the ledger");
        let feats = rig.state.players().by_seat(Seat(1)).unwrap().misc.event_feats.as_slice().to_vec();
        if hits >= 3 {
            assert_eq!(rig.driver.window.scoring, Some(1), "the occurrence is handed to the caller");
            assert_eq!(feats, vec![(Feat::SpaceCannonTookTheLastNonFighters, 1)]);
            assert_eq!(rig.state.meta().feat_occurrence_seq, 1);
        } else {
            assert_eq!(rig.driver.window.scoring, None);
            assert!(feats.is_empty());
        }
    }
    assert!(counts.iter().all(|c| *c > 0), "the seeds cover 0, 1, 2 and 3 hits: {counts:?}");
}

#[test]
fn the_first_question_after_a_hit_is_the_victims_choice_of_casualty() {
    covers!("rules::combat::space_cannon_offense::SpaceCannonOffense");
    let seed = (0..60u64)
        .find(|s| {
            let mut rig = Rig::new(guns_and_fleet(), SpaceCannonOffense::new(facts())).seeded(*s);
            rig.run();
            hits_of(&rig) == 1
        })
        .expect("a seed with exactly one hit");
    let mut rig = Rig::new(guns_and_fleet(), SpaceCannonOffense::new(facts())).seeded(seed);
    rig.run();
    assert_eq!(rig.asked().actor, Seat(0), "the hit player assigns");
    assert_eq!(rig.asked().ask, Ask::Combat(CAsk::AssignHit));
    assert_eq!(rig.driver.window.absorbing(), Some((Seat(1), false)), "the gunner and whether the loss is bound to non-fighters");
    rig.assert_options_sound();
    rig.pick(1);
    assert!(rig.is_done());
    let left: Vec<Unit> = rig.state.board().sys(sys()).units.iter().map(|p| p.kind()).collect();
    assert_eq!(left, vec![Unit::CruiserI, Unit::CruiserI], "the destroyer (option 1) was chosen");
    assert_eq!(rig.driver.window.scoring, None, "two ships remain");
}

#[test]
fn a_sustain_is_offered_before_the_casualty_is_assigned() {
    covers!("rules::combat::space_cannon_offense::SpaceCannonOffense");
    let (p1, _) = planets();
    let make = |seed| {
        let sc = arena().unit(sys(), Some(p1), Unit::PDSI, 1, 3).control(p1, 1).unit(sys(), None, Unit::DreadnoughtI, 0, 1).unit(sys(), None, Unit::CruiserI, 0, 1);
        Rig::new(sc, SpaceCannonOffense::new(facts())).seeded(seed)
    };
    let seed = (0..60u64).find(|s| {
        let mut rig = make(*s);
        rig.run();
        hits_of(&rig) == 1
    });
    let mut rig = make(seed.expect("a seed with exactly one hit"));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Combat(CAsk::CancelHit { in_combat: false, ability: true, .. })), "{:?}", rig.asked().ask);
    assert!(matches!(rig.opts()[0], Opt::Combat(COpt::Sustain { unit: Unit::DreadnoughtI, .. })));
    rig.pick(0);
    assert!(rig.is_done(), "the one hit is cancelled");
    assert_eq!(rig.state.board().sys(sys()).units.iter().filter(|p| p.sustained()).count(), 1);
    assert_eq!(rig.state.board().sys(sys()).units.len(), 2);
}

#[test]
fn graviton_laser_system_binds_the_loss_to_non_fighters_and_is_exhausted() {
    covers!("rules::combat::space_cannon_offense::SpaceCannonOffense");
    let (p1, _) = planets();
    let make = |seed| {
        let sc = arena()
            .tech(1, Technology::GravitonLaserSystem)
            .unit(sys(), Some(p1), Unit::PDSI, 1, 3)
            .control(p1, 1)
            .unit(sys(), None, Unit::FighterI, 0, 3)
            .unit(sys(), None, Unit::CruiserI, 0, 1)
            .unit(sys(), None, Unit::DestroyerI, 0, 1);
        Rig::new(sc, SpaceCannonOffense::new(facts())).seeded(seed)
    };
    let seed = (0..60u64).find(|s| {
        let mut rig = make(*s);
        rig.run();
        hits_of(&rig) == 1
    });
    let mut rig = make(seed.expect("a seed with exactly one hit"));
    rig.run();
    assert_eq!(rig.driver.window.absorbing(), Some((Seat(1), true)), "bound to non-fighters");
    let kinds: Vec<Unit> = rig.opts().iter().filter_map(|o| if let Opt::Combat(COpt::Destroy { unit, .. }) = o { Some(*unit) } else { None }).collect();
    assert_eq!(kinds, vec![Unit::CruiserI, Unit::DestroyerI], "no fighter is offered");
    assert!(rig.state.players().by_seat(Seat(1)).unwrap().tech.exhausted.contains(Technology::GravitonLaserSystem), "the card is exhausted when it changes something");
    // Without the technology the fighters are on offer.
    let sc = arena().unit(sys(), Some(p1), Unit::PDSI, 1, 3).control(p1, 1).unit(sys(), None, Unit::FighterI, 0, 3).unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::DestroyerI, 0, 1);
    let seed = (0..60u64).find(|s| {
        let mut rig = Rig::new(sc.clone(), SpaceCannonOffense::new(facts())).seeded(*s);
        rig.run();
        hits_of(&rig) == 1
    });
    let mut plain = Rig::new(sc, SpaceCannonOffense::new(facts())).seeded(seed.unwrap());
    plain.run();
    assert!(plain.opts().len() == 3, "fighter, cruiser, destroyer: {:?}", plain.opts());
}

#[test]
fn nobody_fires_without_guns_or_under_a_solar_flare() {
    covers!("rules::combat::space_cannon_offense::SpaceCannonOffense");
    let mut quiet = Rig::new(arena().unit(sys(), None, Unit::CruiserI, 0, 2), SpaceCannonOffense::new(facts())).seeded(1);
    quiet.run();
    assert!(quiet.is_done() && quiet.driver.window.drawn.is_empty() && quiet.driver.emitted.is_empty());
    assert_eq!(delta(quiet.start_state(), &quiet.state), "");
    let (mut state, kernel) = guns_and_fleet().build();
    state.meta_mut().activation_seq = 3;
    state.players_mut().by_seat_mut(Seat(0)).unwrap().round_marks.push(RoundKind::SolarFlare, 3);
    let mut flare = Rig::from_parts(state, kernel, SpaceCannonOffense::new(facts())).seeded(1);
    flare.run();
    assert!(flare.is_done() && flare.driver.window.drawn.is_empty(), "Solar Flare: the PDS cannot fire");
}

#[test]
fn the_offense_is_deterministic_for_a_seed() {
    covers!("rules::combat::space_cannon_offense::SpaceCannonOffense");
    let run = |seed| {
        let mut rig = Rig::new(guns_and_fleet(), SpaceCannonOffense::new(facts())).seeded(seed);
        rig.run();
        rig.driver.window.drawn.clone()
    };
    assert_eq!(run(5), run(5));
    assert!((0..10).any(|s| run(s) != run(5)));
}

// ---- Absorb ------------------------------------------------------------------------------------

fn texts() -> HitTexts {
    HitTexts { staging: false, commander: Code(1), casualty: Code(2), effect_hit: Code(3) }
}

#[test]
fn cancellations_a_card_granted_are_spent_before_anything_is_assigned() {
    covers!("rules::combat::effect_hits::Absorb");
    let (mut state, kernel) = arena().unit(sys(), None, Unit::CruiserI, 1, 2).unit(sys(), None, Unit::DestroyerI, 1, 1).build();
    hits::grant(&mut state, Seat(1), 2);
    let mut rig = Rig::from_parts(state, kernel, Absorb::new(Seat(1), Seat(0), sys(), 3, texts()));
    rig.run();
    assert_eq!(rig.asked().actor, Seat(1));
    assert_eq!(rig.asked().ctx.outstanding.as_slice()[0].amount, 1, "three hits, two cancelled: one is assigned");
    rig.pick(0);
    assert!(rig.is_done());
    assert_eq!(rig.state.board().sys(sys()).units.len(), 2);
    assert_eq!(hits::cancellable(&rig.state, Seat(1)), 0, "the cancellations are used up");
}

#[test]
fn a_sustain_comes_first_then_the_remaining_hits_are_assigned() {
    covers!("rules::combat::effect_hits::Absorb");
    let sc = arena().unit(sys(), None, Unit::DreadnoughtI, 1, 1).unit(sys(), None, Unit::CruiserI, 1, 1).unit(sys(), None, Unit::DestroyerI, 1, 1);
    let mut rig = Rig::new(sc, Absorb::new(Seat(1), Seat(0), sys(), 2, texts()));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Combat(CAsk::CancelHit { hits: 2, in_combat: false, .. })), "an effect's hits are not combat hits");
    rig.pick(0);
    assert_eq!(rig.asked().ask, Ask::Combat(CAsk::AssignHit), "one hit is left after the sustain");
    assert_eq!(rig.asked().ctx.outstanding.as_slice()[0].amount, 1);
    rig.assert_options_sound();
    // The damaged dreadnought can now be lost too: three kinds are offered.
    assert_eq!(rig.opts().len(), 3);
    rig.pick(1);
    assert!(rig.is_done());
    assert_eq!(rig.state.board().sys(sys()).units.len(), 2);
    assert!(rig.driver.emitted.is_empty(), "an effect announces nothing");
}

#[test]
fn declining_the_sustain_sends_every_hit_to_the_assignment() {
    covers!("rules::combat::effect_hits::Absorb");
    let sc = arena().unit(sys(), None, Unit::DreadnoughtI, 1, 1).unit(sys(), None, Unit::CruiserI, 1, 1);
    let mut rig = Rig::new(sc, Absorb::new(Seat(1), Seat(0), sys(), 2, texts()));
    rig.run();
    rig.decline();
    assert_eq!(rig.asked().ask, Ask::Combat(CAsk::AssignHit));
    assert_eq!(rig.asked().ctx.outstanding.as_slice()[0].amount, 2);
    rig.pick(0);
    assert!(rig.is_done(), "the second hit takes the last ship unasked");
    assert_eq!(rig.state.board().sys(sys()).units.len(), 0);
}

#[test]
fn hits_on_a_seat_without_ships_do_nothing() {
    covers!("rules::combat::effect_hits::Absorb");
    let mut rig = Rig::new(arena().unit(sys(), None, Unit::CruiserI, 0, 1), Absorb::new(Seat(1), Seat(0), sys(), 4, texts()));
    rig.run();
    assert!(rig.is_done());
    assert_eq!(delta(rig.start_state(), &rig.state), "");
    let mut zero = Rig::new(arena().unit(sys(), None, Unit::CruiserI, 1, 1), Absorb::new(Seat(1), Seat(0), sys(), 0, texts()));
    zero.run();
    assert!(zero.is_done() && delta(zero.start_state(), &zero.state) == "", "no hits");
}
