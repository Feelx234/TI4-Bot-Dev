//! D4-2d: the anti-fighter barrage (LRR 8, 78.3), Raid Formation (Argent), Assault Cannon (technology) and the barrage / space cannon plans.
//! Dice are never hard-coded: the window records the faces it drew from the seeded stream (`drawn`), and the tests derive the hits from them.

use ti4_engine_core::choice::combat::{Ask as CAsk, Opt as COpt, Subtype as CSub};
use ti4_engine_core::choice::Subtype;
use ti4_engine_core::rules::combat::assault_cannon::{AssaultCannon, Facts as AcFacts};
use ti4_engine_core::rules::combat::barrage::{BarrageStep, Facts as BarrageFacts};
use ti4_engine_core::rules::combat::destroy_fighters::{DestroyFighters, Fighters};
use ti4_engine_core::rules::combat::raid_formation::{Facts as RaidFacts, RaidFormation};
use ti4_engine_core::rules::combat::{barrage_plan, BarrageRoll, Rd};
use ti4_engine_core::state::{Code, EventKind, Feat, Key};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn sys() -> System {
    plain_systems()[0]
}

fn dest() -> System {
    plain_systems()[1]
}

fn arena(a: Faction, d: Faction) -> Scenario {
    Scenario::new(&[a, d]).map_from(&[sys(), dest()])
}

fn count(rig: &Rig<impl Window>, seat: u8, f: impl Fn(Unit) -> bool) -> usize {
    rig.state.board().sys(sys()).units.iter().filter(|p| p.owner() == Seat(seat) && f(p.kind())).count()
}

fn fighters(unit: Unit) -> bool {
    matches!(unit, Unit::FighterI | Unit::FighterII)
}

// ---- the plan ----------------------------------------------------------------------------------

#[test]
fn a_destroyer_barrages_with_the_printed_number_and_dice_and_other_ships_do_not() {
    covers!("rules::combat::barrage_plan");
    let (state, kernel) = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec)
        .unit(sys(), None, Unit::DestroyerI, 0, 1)
        .unit(sys(), None, Unit::DestroyerII, 0, 1)
        .unit(sys(), None, Unit::CruiserI, 0, 1)
        .unit(sys(), None, Unit::FighterI, 0, 2)
        .build();
    let plan = barrage_plan(Rd::new(&state, &kernel), Seat(0), sys());
    assert_eq!(plan, vec![BarrageRoll { unit: Some(Unit::DestroyerI), hits_on: 9, dice: 2 }, BarrageRoll { unit: Some(Unit::DestroyerII), hits_on: 6, dice: 3 }], "destroyer: ANTI-FIGHTER BARRAGE 9 (x2); destroyer II: 6 (x3)");
    assert_eq!(barrage_plan(Rd::new(&state, &kernel), Seat(1), sys()), vec![], "nobody else has a ship here");
}

#[test]
fn iconoclast_omega_forbids_the_barrage_against_its_owner_from_rules_version_17() {
    covers!("rules::combat::barrage_plan");
    let planet = sys().data().planets[0];
    let mk = |rules: u32| {
        arena(Faction::TheEmbersOfMuaat, Faction::TheYinBrotherhood).rules(rules).unit(sys(), None, Unit::DestroyerII, 0, 1).unit(sys(), Some(planet), Unit::IconoclastOmega, 1, 1).control(planet, 1).build()
    };
    let (state, kernel) = mk(35);
    assert_eq!(barrage_plan(Rd::new(&state, &kernel), Seat(0), sys()), vec![], "other players cannot use ANTI-FIGHTER BARRAGE against the Omega's owner here");
    let (older, kernel) = mk(16);
    assert_eq!(barrage_plan(Rd::new(&older, &kernel), Seat(0), sys()).len(), 1, "before rules version 17 the mech did not have the clause");
    // The owner of the mech itself still barrages.
    let (own, kernel) = arena(Faction::TheYinBrotherhood, Faction::TheEmbersOfMuaat).unit(sys(), None, Unit::DestroyerII, 0, 1).unit(sys(), Some(planet), Unit::IconoclastOmega, 0, 1).control(planet, 0).build();
    assert_eq!(barrage_plan(Rd::new(&own, &kernel), Seat(0), sys()).len(), 1);
}

// ---- the step ----------------------------------------------------------------------------------

fn facts(staging: bool, announce: bool) -> BarrageFacts {
    BarrageFacts {
        attacker: Seat(0),
        defender: Seat(1),
        system: sys(),
        round: 1,
        occurrence: 7,
        staging,
        announce,
        phase: Code(1),
        cause: Code(2),
        kind: Code(3),
        extra: Code(4),
        unit_codes: vec![(Unit::DestroyerI, Code(10)), (Unit::DestroyerII, Code(11))],
        raid: None,
    }
}

fn hits_of(drawn: &[(u32, Vec<u32>)]) -> usize {
    drawn.iter().map(|(on, faces)| faces.iter().filter(|f| **f >= *on).count()).sum()
}

#[test]
fn the_barrage_destroys_as_many_fighters_as_it_hit_in_unit_order_and_never_other_ships() {
    covers!("rules::combat::barrage::BarrageStep");
    let mut seen_hit = false;
    let mut seen_miss = false;
    for seed in 0..40u64 {
        let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec)
            .unit(sys(), None, Unit::DestroyerII, 0, 1)
            .unit(sys(), None, Unit::FighterI, 1, 2)
            .unit(sys(), None, Unit::CruiserI, 1, 1)
            .unit(sys(), None, Unit::FighterII, 1, 1);
        let mut rig = Rig::new(sc, BarrageStep::new(facts(false, false))).seeded(seed);
        rig.run();
        assert!(rig.is_done(), "the barrage never asks");
        let drawn = &rig.driver.window.drawn;
        assert_eq!(drawn.len(), 1, "one destroyer II: one entry");
        assert_eq!((drawn[0].0, drawn[0].1.len()), (6, 3), "3 dice, hitting on 6 or more");
        assert!(drawn[0].1.iter().all(|f| (1..=10).contains(f)), "d10 faces: {:?}", drawn[0].1);
        let hits = hits_of(drawn);
        seen_hit |= hits > 0;
        seen_miss |= hits == 0;
        let left = count(&rig, 1, fighters);
        assert_eq!(left, 3usize.saturating_sub(hits), "seed {seed}: every hit takes a fighter, no more than there are");
        assert_eq!(count(&rig, 1, |u| u == Unit::CruiserI), 1, "the cruiser cannot be hit by the barrage");
        // The first fighters in unit order go: the two fighter I before the fighter II.
        if hits == 1 || hits == 2 {
            assert_eq!(count(&rig, 1, |u| u == Unit::FighterII), 1, "seed {seed}: unit order, the fighter II is last");
        }
        assert_eq!(rig.state.transient().combat.presentation.barrage_hits.get(&Seat(0)).copied(), Some(hits as u32));
        assert_eq!(rig.state.transient().combat.presentation.barrage_dice.len(), 3);
        assert_eq!(rig.state.ledger().dice.len(), 1, "the dice are recorded");
        assert_eq!(count(&rig, 0, |u| u == Unit::DestroyerII), 1, "the destroyer itself is untouched");
    }
    assert!(seen_hit && seen_miss, "the seeds cover both a hit and a miss");
}

#[test]
fn both_sides_barrage_at_once_the_attacker_rolls_first_and_neither_loses_its_dice_to_the_other() {
    covers!("rules::combat::barrage::BarrageStep");
    for seed in 0..25u64 {
        let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec)
            .unit(sys(), None, Unit::DestroyerII, 0, 1)
            .unit(sys(), None, Unit::FighterI, 0, 3)
            .unit(sys(), None, Unit::DestroyerII, 1, 1)
            .unit(sys(), None, Unit::FighterI, 1, 3);
        let mut rig = Rig::new(sc, BarrageStep::new(facts(false, false))).seeded(seed);
        rig.run();
        let drawn = rig.driver.window.drawn.clone();
        assert_eq!(drawn.len(), 2, "one entry per side, the attacker's first");
        let (a_hits, d_hits) = (hits_of(&drawn[..1]), hits_of(&drawn[1..]));
        assert_eq!(count(&rig, 1, fighters), 3usize.saturating_sub(a_hits), "seed {seed}: the defender's fighters fall to the attacker's dice");
        assert_eq!(count(&rig, 0, fighters), 3usize.saturating_sub(d_hits), "seed {seed}: simultaneously the attacker's fall to the defender's dice");
    }
}

#[test]
fn the_same_seed_rolls_the_same_dice_and_another_seed_eventually_differs() {
    covers!("rules::combat::barrage::BarrageStep");
    let make = |seed| {
        let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::DestroyerII, 0, 2).unit(sys(), None, Unit::FighterI, 1, 5);
        let mut rig = Rig::new(sc, BarrageStep::new(facts(false, false))).seeded(seed);
        rig.run();
        rig.driver.window.drawn.clone()
    };
    assert_eq!(make(3), make(3));
    assert!((0..10).any(|s| make(s) != make(3)), "the stream depends on the seed");
}

#[test]
fn taking_the_last_fighters_scores_the_barrage_feat_once_and_only_when_there_were_fighters() {
    covers!("rules::combat::barrage::BarrageStep");
    let mut scored = 0;
    for seed in 0..30u64 {
        let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::DestroyerII, 0, 1).unit(sys(), None, Unit::FighterI, 1, 1).unit(sys(), None, Unit::CruiserI, 1, 1);
        let mut rig = Rig::new(sc, BarrageStep::new(facts(false, false))).seeded(seed);
        rig.run();
        let hit = hits_of(&rig.driver.window.drawn) > 0;
        let feats = rig.state.players().by_seat(Seat(0)).unwrap().misc.event_feats.as_slice().to_vec();
        if hit {
            scored += 1;
            assert_eq!(feats, vec![(Feat::BarrageTookTheLastFighters, 7)], "seed {seed}: the feat against the combat's occurrence");
            assert_eq!(rig.driver.window.feat, vec![Seat(0)]);
        } else {
            assert!(feats.is_empty() && rig.driver.window.feat.is_empty(), "seed {seed}: no hit, no feat");
        }
        assert!(rig.state.players().by_seat(Seat(1)).unwrap().misc.event_feats.is_empty());
    }
    assert!(scored > 0);
    // No fighters to begin with: no feat however the dice fall.
    for seed in 0..10u64 {
        let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::DestroyerII, 0, 2).unit(sys(), None, Unit::CruiserI, 1, 1);
        let mut rig = Rig::new(sc, BarrageStep::new(facts(false, false))).seeded(seed);
        rig.run();
        assert!(rig.state.players().by_seat(Seat(0)).unwrap().misc.event_feats.is_empty(), "nothing to take");
    }
}

#[test]
fn the_barrage_is_announced_side_by_side_and_each_loss_follows_with_staging() {
    covers!("rules::combat::barrage::BarrageStep");
    let mut checked = false;
    for seed in 0..40u64 {
        let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::DestroyerII, 0, 1).unit(sys(), None, Unit::FighterI, 1, 2).unit(sys(), None, Unit::CruiserI, 1, 1);
        let mut rig = Rig::new(sc, BarrageStep::new(facts(true, true))).seeded(seed);
        rig.run();
        let hits = hits_of(&rig.driver.window.drawn);
        let kinds: Vec<EventKind> = rig.driver.emitted.iter().map(|e| e.kind).collect();
        let started = kinds.iter().filter(|k| **k == EventKind::AntiFighterBarrageStarted).count();
        assert_eq!(started, 2, "both sides are announced, whether or not they have a barrage");
        assert_eq!(rig.driver.emitted[0].payload.seat(Key::Player), Some(Seat(0)), "the attacker first");
        let rolled: Vec<_> = rig.driver.emitted.iter().filter(|e| e.kind == EventKind::UnitAbilityRolled).collect();
        assert_eq!(rolled.len(), 1, "only the side that rolled dice reports a roll");
        assert_eq!(rolled[0].payload.int(Key::Hits), Some(hits as i64));
        assert_eq!(rolled[0].payload.code(Key::Kind), Some(Code(3)));
        let lost = kinds.iter().filter(|k| **k == EventKind::ShipDestroyed).count();
        assert_eq!(lost, hits.min(2), "seed {seed}: one SHIP_DESTROYED per fighter lost");
        if hits > 0 {
            assert!(kinds.contains(&EventKind::AntiFighterBarrageHits), "the hits window opens before the fighters are taken");
            checked = true;
        } else {
            assert!(!kinds.contains(&EventKind::AntiFighterBarrageHits));
        }
    }
    assert!(checked);
}

// ---- DestroyFighters -------------------------------------------------------------------------

fn kill(hits: u16, staging: bool) -> Fighters {
    Fighters { target: Seat(1), system: sys(), hits, cause: Code(5), staging, announce: staging }
}

#[test]
fn destroy_fighters_takes_the_first_fighters_in_unit_order_and_stops_when_none_are_left() {
    covers!("rules::combat::destroy_fighters::DestroyFighters");
    let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::FighterI, 1, 1).unit(sys(), None, Unit::CruiserI, 1, 1).unit(sys(), None, Unit::FighterII, 1, 2).unit(sys(), None, Unit::FighterI, 0, 2);
    let mut rig = Rig::new(sc.clone(), DestroyFighters::new(kill(2, false)));
    rig.run();
    assert!(rig.is_done());
    let left: Vec<Unit> = rig.state.board().sys(sys()).units.iter().filter(|p| p.owner() == Seat(1)).map(|p| p.kind()).collect();
    assert_eq!(left, vec![Unit::CruiserI, Unit::FighterII], "the fighter I and then the first fighter II go");
    assert_eq!(count(&rig, 0, fighters), 2, "the other side's fighters are untouched");
    let mut many = Rig::new(sc.clone(), DestroyFighters::new(kill(9, false)));
    many.run();
    assert_eq!(count(&many, 1, fighters), 0);
    assert_eq!(count(&many, 1, |u| u == Unit::CruiserI), 1, "only fighters");
    let mut none = Rig::new(sc, DestroyFighters::new(kill(0, false)));
    none.run();
    assert_eq!(delta(none.start_state(), &none.state), "");
}

#[test]
fn destroy_fighters_announces_each_loss_after_all_were_removed_and_only_with_staging() {
    covers!("rules::combat::destroy_fighters::DestroyFighters");
    let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::FighterI, 1, 3).unit(sys(), None, Unit::CruiserI, 1, 1);
    let mut rig = Rig::new(sc.clone(), DestroyFighters::new(kill(2, true)));
    rig.run();
    let lost: Vec<_> = rig.driver.emitted.iter().filter(|e| e.kind == EventKind::ShipDestroyed).collect();
    assert_eq!(lost.len(), 2);
    assert!(lost.iter().all(|e| e.payload.unit(Key::Unit) == Some(Unit::FighterI) && e.payload.flag(Key::Last) == Some(false) && e.payload.code(Key::Cause) == Some(Code(5))));
    let mut quiet = Rig::new(sc, DestroyFighters::new(kill(2, false)));
    quiet.run();
    assert!(quiet.driver.emitted.is_empty());
    assert_eq!(count(&quiet, 1, fighters), 1, "the same losses without the announcements");
}

#[test]
fn destroy_fighters_never_asks_and_refuses_every_pick() {
    covers!("rules::combat::destroy_fighters::DestroyFighters");
    let mut rig = Rig::new(arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::FighterI, 1, 1), DestroyFighters::new(kill(1, false)));
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.ask_of(&rig.state, &rig.kernel), None);
}

// ---- Raid Formation --------------------------------------------------------------------------

fn raid(excess: u16) -> RaidFacts {
    RaidFacts { producer: Seat(0), target: Seat(1), system: sys(), excess, subtype: Code(8) }
}

#[test]
fn raid_formation_lets_the_producer_pick_the_type_to_damage_and_takes_the_last_type_unasked() {
    covers!("rules::combat::raid_formation::RaidFormation");
    let sc = arena(Faction::TheArgentFlight, Faction::TheEmbersOfMuaat).unit(sys(), None, Unit::DreadnoughtI, 1, 2).unit(sys(), None, Unit::WarSun, 1, 1).unit(sys(), None, Unit::CruiserI, 1, 1);
    let mut rig = Rig::new(sc, RaidFormation::new(raid(2)));
    rig.run();
    assert_eq!(rig.asked().actor, Seat(0), "the producer of the barrage chooses, not the owner of the ships");
    assert_eq!(rig.asked().ask, Ask::Combat(CAsk::RaidDamage { target: Seat(1) }));
    assert_eq!(rig.asked().ctx.subtype, Subtype::Combat(CSub::Casualty(Code(8))));
    assert_eq!(rig.opts(), vec![Opt::Combat(COpt::RaidDamage { unit: Unit::DreadnoughtI }), Opt::Combat(COpt::RaidDamage { unit: Unit::WarSun })], "the cruiser has no SUSTAIN DAMAGE; one option per type");
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(Opt::Combat(COpt::RaidDamage { unit: Unit::CruiserI })));
    rig.expect_illegal(Pick::Opt(Opt::Decline));
    rig.pick(0);
    assert!(!rig.is_done(), "the second excess hit is still owed");
    // Two excess hits: after the dreadnought, both types remain (a second dreadnought is undamaged).
    assert_eq!(rig.opts().len(), 2);
    rig.pick(1);
    assert!(rig.is_done());
    let damaged: Vec<Unit> = rig.state.board().sys(sys()).units.iter().filter(|p| p.sustained()).map(|p| p.kind()).collect();
    assert_eq!(damaged, vec![Unit::DreadnoughtI, Unit::WarSun]);
}

#[test]
fn raid_formation_with_one_candidate_type_needs_no_question_and_runs_out_of_candidates() {
    covers!("rules::combat::raid_formation::RaidFormation");
    let sc = arena(Faction::TheArgentFlight, Faction::TheEmbersOfMuaat).unit(sys(), None, Unit::DreadnoughtI, 1, 2).unit(sys(), None, Unit::CruiserI, 1, 1);
    let mut rig = Rig::new(sc.clone(), RaidFormation::new(raid(5)));
    rig.run();
    assert!(rig.is_done(), "one type: both dreadnoughts are damaged unasked, the other hits find nothing");
    assert_eq!(rig.state.board().sys(sys()).units.iter().filter(|p| p.sustained()).count(), 2);
    let mut one = Rig::new(sc, RaidFormation::new(raid(1)));
    one.run();
    assert_eq!(one.state.board().sys(sys()).units.iter().filter(|p| p.sustained()).count(), 1, "one excess hit damages one ship");
    let mut zero = Rig::new(arena(Faction::TheArgentFlight, Faction::TheEmbersOfMuaat).unit(sys(), None, Unit::DreadnoughtI, 1, 1), RaidFormation::new(raid(0)));
    zero.run();
    assert!(zero.is_done() && delta(zero.start_state(), &zero.state).is_empty());
    let mut none = Rig::new(arena(Faction::TheArgentFlight, Faction::TheEmbersOfMuaat).unit(sys(), None, Unit::CruiserI, 1, 2), RaidFormation::new(raid(3)));
    none.run();
    assert!(none.is_done() && delta(none.start_state(), &none.state).is_empty(), "no ship with SUSTAIN DAMAGE: the ability ends");
}

#[test]
fn raid_formation_skips_ships_that_are_already_damaged() {
    covers!("rules::combat::raid_formation::RaidFormation");
    let (mut state, kernel) = arena(Faction::TheArgentFlight, Faction::TheEmbersOfMuaat).unit(sys(), None, Unit::DreadnoughtI, 1, 1).unit(sys(), None, Unit::WarSun, 1, 1).build();
    let dread = state.board_mut().sys_mut(sys()).units.as_mut_slice().iter_mut().find(|p| p.kind() == Unit::DreadnoughtI).unwrap();
    *dread = dread.with_sustained(true);
    let mut rig = Rig::from_parts(state, kernel, RaidFormation::new(raid(1)));
    rig.run();
    assert!(rig.is_done(), "only the war sun is undamaged: it takes the hit without a question");
    assert_eq!(rig.state.board().sys(sys()).units.iter().filter(|p| p.sustained()).count(), 2);
}

// ---- Assault Cannon --------------------------------------------------------------------------

fn ac() -> AcFacts {
    AcFacts { attacker: Seat(0), defender: Seat(1), system: sys(), subtype: Code(20), cause: Code(21), announce: true }
}

#[test]
fn assault_cannon_makes_the_opponent_choose_a_non_fighter_to_destroy() {
    covers!("rules::combat::assault_cannon::AssaultCannon");
    let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec)
        .tech(0, Technology::AssaultCannon)
        .unit(sys(), None, Unit::CruiserI, 0, 3)
        .unit(sys(), None, Unit::DestroyerI, 1, 1)
        .unit(sys(), None, Unit::CarrierI, 1, 1)
        .unit(sys(), None, Unit::FighterI, 1, 4);
    let mut rig = Rig::new(sc, AssaultCannon::new(ac()));
    rig.run();
    assert_eq!(rig.asked().actor, Seat(1), "the victim chooses");
    assert_eq!(rig.opts().len(), 2, "the destroyer and the carrier, never the fighters: {:?}", rig.opts());
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(Opt::Combat(COpt::Destroy { index: 5, unit: Unit::FighterI, damaged: false })));
    rig.pick(1);
    assert!(rig.is_done());
    assert_eq!(count(&rig, 1, |u| u == Unit::CarrierI), 0);
    assert_eq!(count(&rig, 1, |u| u == Unit::DestroyerI), 1);
    assert_eq!(count(&rig, 1, fighters), 4);
    assert_eq!(count(&rig, 0, |_| true), 3, "the holder loses nothing");
    let lost: Vec<_> = rig.driver.emitted.iter().filter(|e| e.kind == EventKind::ShipDestroyed).collect();
    assert_eq!(lost.len(), 1);
    assert_eq!(lost[0].payload.code(Key::Cause), Some(Code(21)));
    ti4_engine_core_tests::assert_snapshot("combat_barrage/assault_cannon_one", &rig.snapshot());
}

#[test]
fn assault_cannon_needs_the_technology_and_three_non_fighter_ships() {
    covers!("rules::combat::assault_cannon::AssaultCannon");
    let enemy = |sc: Scenario| sc.unit(sys(), None, Unit::DestroyerI, 1, 2);
    // Without the technology.
    let mut rig = Rig::new(enemy(arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 3)), AssaultCannon::new(ac()));
    rig.run();
    assert!(rig.is_done() && delta(rig.start_state(), &rig.state).is_empty());
    // Two non-fighters and many fighters do not count.
    let two = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).tech(0, Technology::AssaultCannon).unit(sys(), None, Unit::CruiserI, 0, 2).unit(sys(), None, Unit::FighterI, 0, 5);
    let mut rig = Rig::new(enemy(two), AssaultCannon::new(ac()));
    rig.run();
    assert!(rig.is_done() && delta(rig.start_state(), &rig.state).is_empty(), "fighters do not count towards the three");
}

#[test]
fn assault_cannon_a_single_candidate_is_lost_unasked_and_fighters_alone_are_safe() {
    covers!("rules::combat::assault_cannon::AssaultCannon");
    let one = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).tech(0, Technology::AssaultCannon).unit(sys(), None, Unit::CruiserI, 0, 3).unit(sys(), None, Unit::DestroyerI, 1, 1).unit(sys(), None, Unit::FighterI, 1, 2);
    let mut rig = Rig::new(one, AssaultCannon::new(ac()));
    rig.run();
    assert!(rig.is_done());
    assert_eq!(count(&rig, 1, |u| u == Unit::DestroyerI), 0);
    assert_eq!(count(&rig, 1, fighters), 2);
    let safe = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).tech(0, Technology::AssaultCannon).unit(sys(), None, Unit::CruiserI, 0, 3).unit(sys(), None, Unit::FighterI, 1, 2);
    let mut rig = Rig::new(safe, AssaultCannon::new(ac()));
    rig.run();
    assert!(rig.is_done() && count(&rig, 1, fighters) == 2, "the victim has no non-fighter ship: nothing to destroy");
}

#[test]
fn assault_cannon_of_both_sides_fires_in_attacker_defender_order_with_eligibility_read_first() {
    covers!("rules::combat::assault_cannon::AssaultCannon");
    // Each side has exactly three non-fighters. The attacker fires first; the defender has two left when its own cannon fires,
    // yet it still fires because eligibility was read before either loss.
    let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec)
        .tech(0, Technology::AssaultCannon)
        .tech(1, Technology::AssaultCannon)
        .unit(sys(), None, Unit::CruiserI, 0, 2)
        .unit(sys(), None, Unit::DestroyerI, 0, 1)
        .unit(sys(), None, Unit::CruiserI, 1, 2)
        .unit(sys(), None, Unit::DestroyerI, 1, 1);
    let mut rig = Rig::new(sc, AssaultCannon::new(ac()));
    rig.run();
    assert_eq!(rig.asked().actor, Seat(1), "the attacker's cannon: the defender chooses first");
    rig.pick(0);
    assert_eq!(rig.asked().actor, Seat(0), "then the defender's cannon: the attacker chooses");
    rig.pick(0);
    assert!(rig.is_done());
    assert_eq!(count(&rig, 0, |_| true), 2);
    assert_eq!(count(&rig, 1, |_| true), 2);
}

// ---- Raid Formation inside the step (D4A4) ----------------------------------------------------------------------------------------------

#[test]
fn hits_beyond_the_fighters_are_asked_about_as_raid_formation_damage_inside_the_barrage_step() {
    covers!("rules::combat::barrage::BarrageStep");
    let sc = || {
        arena(Faction::TheArgentFlight, Faction::TheEmbersOfMuaat)
            .unit(sys(), None, Unit::DestroyerII, 0, 6)
            .unit(sys(), None, Unit::DreadnoughtI, 1, 1)
            .unit(sys(), None, Unit::WarSun, 1, 1)
            .unit(sys(), None, Unit::FighterI, 1, 1)
    };
    let mut f = facts(false, false);
    f.raid = Some(Code(8));
    let mut asked = 0;
    for seed in 0..60u64 {
        let mut rig = Rig::new(sc(), BarrageStep::new(f.clone())).seeded(seed);
        rig.run();
        if matches!(rig.status(), Some(ti4_engine_core::driver::Status::Asking(_))) {
            asked += 1;
            assert!(matches!(rig.asked().ask, Ask::Combat(CAsk::RaidDamage { target: Seat(1) })));
            assert_eq!(rig.asked().actor, Seat(0));
            assert_eq!(rig.stages.last().map(String::as_str), Some("Applying"));
            if asked == 1 {
                rig.assert_options_sound();
            }
            rig.pick(0);
            assert_eq!(count(&rig, 1, fighters), 0, "the fighter is destroyed first");
        }
    }
    assert!(asked > 0, "some seed rolls more hits than the one fighter");
}
