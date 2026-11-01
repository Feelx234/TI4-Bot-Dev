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

/// Found by the whole-game shadow (muaat 3p, ghost 4p): the Letnev agent's extra die (a Ssruu round-agent mark) is a COMBAT die. The string reads it at the space
/// combat roll and the ground combat roll only; the core's dice seam added it to every kind of roll, so a destroyer the agent named barraged with a third die.
#[test]
fn the_extra_die_of_a_round_agent_is_not_a_barrage_die() {
    covers!("rules::combat::barrage_plan");
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::keys::{Atom, DecimalTextOrd, MarkKey, MarkVal};
    let (mut state, mut kernel) = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::DestroyerI, 0, 1).build();
    // The live kernel's hook for the round agents' marks (`ti4_engine::live::kernel`); the test kernel has none of the faction hooks.
    kernel.hooks.direct.borrowed_round_agents_extra_die_for.push(ti4_engine_core_factions::cc_ground::borrowed::query::extra_die_for);
    let plain = barrage_plan(Rd::new(&state, &kernel), Seat(0), sys());
    assert_eq!(plain, vec![BarrageRoll { unit: Some(Unit::DestroyerI), hits_on: 9, dice: 2 }]);
    // The Letnev agent was used by seat 1 on seat 0's destroyer (ordinal 0 of its ships here) in combat round 3.
    let mark = MarkVal::Rec(vec![
        MarkVal::Atom(Atom::Seat(Seat(1))),
        MarkVal::Atom(Atom::Seat(Seat(1))),
        MarkVal::Atom(Atom::Leader(Leader::ViscountUnlenn)),
        MarkVal::Atom(Atom::System(sys())),
        MarkVal::Atom(Atom::Absent),
        MarkVal::Atom(Atom::Absent),
        MarkVal::Atom(Atom::Seat(Seat(0))),
        MarkVal::Atom(Atom::Unit(Unit::DestroyerI)),
        MarkVal::Atom(Atom::Num(0)),
        MarkVal::Atom(Atom::Num(0)),
        MarkVal::Atom(Atom::Num(0)),
        MarkVal::Atom(Atom::Num(3)),
    ]);
    state.meta_mut().combat_round_seq = 3;
    state.marks_mut().tail.edit().insert(MarkKey::YssarilSsruuRoundAgent(Seat(1), Leader::ViscountUnlenn, DecimalTextOrd(3)), mark);
    assert_eq!(barrage_plan(Rd::new(&state, &kernel), Seat(0), sys()), plain, "the extra die is rolled with the combat dice, not with the barrage");
    // The dice seam: the combat roll of that destroyer has the extra die, no other kind of roll has it.
    use ti4_engine_core::kernel::seam::{Clamp, RollKind};
    use ti4_engine_core::rules::combat::roll_site;
    let rd = Rd::new(&state, &kernel);
    let mut site = roll_site(rd, Seat(0), Unit::DestroyerI, Some(sys()), 0);
    let dice = |site: &ti4_engine_core::kernel::seam::RollSite| rd.k.hooks.seam.dice.eval(rd.q(), site, 1, Clamp::None).value;
    assert_eq!(dice(&site), 2, "a space combat roll of the unit the agent named has the extra die");
    for kind in [RollKind::Barrage, RollKind::SpaceCannon, RollKind::Bombardment] {
        site.kind = kind;
        assert_eq!(dice(&site), 1, "{kind:?}");
    }
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
        commander: [false, false],
        shown: vec![(Unit::DestroyerI, Code(10)), (Unit::DestroyerII, Code(11))],
        extra_shown: Code(12),
        hit_texts: ti4_engine_core::kernel::children::HitTexts { staging: false, commander: Code(13), casualty: Code(14), effect_hit: Code(15) },
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

// ---- the Jol-Nar commander and Waylay (package W10) ----------------------------------------------------

fn jolnar_facts(commander: [bool; 2]) -> BarrageFacts {
    BarrageFacts { commander, ..facts(false, true) }
}

fn commander_arena(seed: u64, commander: [bool; 2]) -> Rig<BarrageStep> {
    let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::DestroyerII, 0, 1).unit(sys(), None, Unit::FighterI, 1, 3);
    Rig::new(sc, BarrageStep::new(jolnar_facts(commander))).seeded(seed)
}

#[test]
fn the_jolnar_commander_asks_about_each_die_and_rerolls_only_the_picked_ones() {
    covers!("rules::combat::barrage::BarrageStep");
    use ti4_engine_core::choice::combat::RerollSource;
    use ti4_engine_core::choice::generic::Content;
    use ti4_engine_core::choice::Origin;
    let (mut rerolled_hit, mut rerolled_miss) = (false, false);
    for seed in 0..60u64 {
        let mut rig = commander_arena(seed, [true, false]);
        rig.run();
        let mut asked = 0;
        let mut picked = Vec::new();
        while !rig.is_done() {
            assert_eq!(rig.asked().actor, Seat(0), "the holder is asked");
            assert_eq!(rig.asked().ask, Ask::Combat(CAsk::RerollDie { source: RerollSource::Commander, unit: Code(11), die: asked }), "one question per die, in order");
            assert_eq!(rig.asked().ctx.origin, Origin::Content(Content::Leader(ti4_engine_core::generated::Leader::AgnlanOln)));
            assert_eq!(rig.asked().ctx.subtype, Subtype::Combat(CSub::Reroll(RerollSource::Commander)));
            assert_eq!(rig.opts().len(), 2, "reroll the die or decline");
            // The staged dice are mirrored for the question's display facts while it is open.
            let staged = rig.state.transient().combat.reroll_staging.iter().find(|(s, _)| *s == Seat(0)).map(|(_, set)| set.clone()).expect("the dice are staged");
            assert_eq!(staged.rolls.len(), 1);
            assert_eq!(staged.rolls[0].faces.len(), 3);
            // Reroll the first die when it missed; keep the others.
            let COpt::RerollDie { face, .. } = (match rig.opts()[0] { Opt::Combat(o) => o, _ => panic!() }) else { panic!() };
            if asked == 0 && face < 6 {
                picked.push(0u8);
                rig.pick(0);
            } else {
                rig.decline();
            }
            asked += 1;
        }
        assert_eq!(asked, 3, "three dice, three questions");
        assert!(rig.state.transient().combat.reroll_staging.is_empty(), "the staging is cleared");
        let w = &rig.driver.window;
        assert_eq!(w.first_draws().len(), 1, "the roll itself");
        assert_eq!(w.drawn.len(), 1 + picked.len(), "one redraw per picked die");
        assert_eq!(rig.state.ledger().dice.len(), 1 + picked.len());
        let final_faces = &w.drawn.last().unwrap().1;
        let hits = final_faces.iter().filter(|f| **f >= 6).count();
        assert_eq!(rig.state.transient().combat.presentation.barrage_hits.get(&Seat(0)).copied(), Some(hits as u32), "the hits read the final faces");
        assert_eq!(count(&rig, 1, fighters), 3usize.saturating_sub(hits), "seed {seed}");
        let announced = rig.driver.emitted.iter().filter(|e| e.kind == EventKind::UnitAbilityRolled).count();
        assert_eq!(announced, 1, "the roll is announced once, after the rerolls");
        if !picked.is_empty() {
            let (before, after) = (w.drawn[0].1[0], w.drawn[1].1[0]);
            assert_ne!(rig.state.ledger().dice.last().unwrap().rerolled, 0, "the redraw is marked in the ledger");
            rerolled_hit |= after >= 6 && before < 6;
            rerolled_miss |= after < 6;
        }
    }
    assert!(rerolled_hit && rerolled_miss, "the seeds cover a reroll that hit and one that missed");
}

#[test]
fn a_barrage_whose_side_does_not_hold_the_commander_asks_nothing() {
    covers!("rules::combat::barrage::BarrageStep");
    for seed in 0..5u64 {
        // The commander with the defender, who rolls no dice: the attacker is not asked.
        let mut rig = commander_arena(seed, [false, true]);
        rig.run();
        assert!(rig.is_done(), "seed {seed}: neither the attacker's roll nor the empty roll of the defender is asked");
        assert_eq!(rig.driver.window.drawn.len(), 1);
    }
}

#[test]
fn only_the_side_that_holds_the_commander_is_asked_when_both_sides_barrage() {
    covers!("rules::combat::barrage::BarrageStep");
    for commander in [[true, false], [false, true], [true, true], [false, false]] {
        for seed in 0..6u64 {
            let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::DestroyerII, 0, 1).unit(sys(), None, Unit::FighterI, 0, 2).unit(sys(), None, Unit::DestroyerII, 1, 1).unit(sys(), None, Unit::FighterI, 1, 2);
            let mut rig = Rig::new(sc, BarrageStep::new(jolnar_facts(commander))).seeded(seed);
            rig.run();
            let mut askers = Vec::new();
            while !rig.is_done() {
                askers.push(rig.asked().actor.0);
                rig.decline();
            }
            let want: Vec<u8> = (0..2u8).filter(|s| commander[usize::from(*s)]).flat_map(|s| [s, s, s]).collect();
            assert_eq!(askers, want, "seed {seed} {commander:?}: three dice per holder, attacker's side first");
        }
    }
}

fn waylay_arena(seed: u64, waylay: bool) -> Rig<BarrageStep> {
    let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::DestroyerII, 0, 1).unit(sys(), None, Unit::FighterI, 1, 1).unit(sys(), None, Unit::DreadnoughtI, 1, 1).unit(sys(), None, Unit::CruiserI, 1, 1);
    let (mut state, kernel) = sc.build();
    state.meta_mut().combat_round_seq = 4;
    if waylay {
        state.players_mut().by_seat_mut(Seat(0)).unwrap().card_timers.waylay_barrage_round.set(Some(4));
    }
    Rig::from_parts(state, kernel, BarrageStep::new(BarrageFacts { hit_texts: ti4_engine_core::kernel::children::HitTexts { staging: true, commander: Code(13), casualty: Code(14), effect_hit: Code(15) }, ..facts(true, true) })).seeded(seed)
}

#[test]
fn a_waylay_sends_the_hits_to_every_ship_with_sustain_damage_first_and_the_owner_choosing_the_losses() {
    covers!("rules::combat::barrage::BarrageStep");
    let (mut asked_sustain, mut asked_casualty) = (false, false);
    for seed in 0..60u64 {
        let mut plain = waylay_arena(seed, false);
        plain.run();
        assert!(plain.is_done(), "without the card the barrage takes the fighters and asks nothing");
        let hits = hits_of(&plain.driver.window.drawn);
        let mut rig = waylay_arena(seed, true);
        rig.run();
        if hits == 0 {
            assert!(rig.is_done(), "seed {seed}: no hit, nothing to assign");
            continue;
        }
        let mut guard = 0;
        while !rig.is_done() {
            let q = rig.asked().clone();
            assert_eq!(q.actor, Seat(1), "the victim's owner answers");
            match q.ask {
                Ask::Combat(CAsk::CancelHit { .. }) => asked_sustain = true,
                Ask::Combat(CAsk::AssignHit) => asked_casualty = true,
                ref other => panic!("{other:?}"),
            }
            rig.pick(0);
            guard += 1;
            assert!(guard < 10);
        }
        // The hits were absorbed by the dreadnought's sustain, the cruiser or the fighter, not only the fighter.
        let lost = 3 - count(&rig, 1, |_| true);
        let sustained = rig.state.board().sys(sys()).units.iter().any(|p| p.owner() == Seat(1) && p.sustained());
        assert!(lost >= 1 || sustained, "seed {seed}: {hits} hits had an effect");
        let destroyed = rig.driver.emitted.iter().filter(|e| e.kind == EventKind::ShipDestroyed).count();
        assert!(destroyed >= lost, "each loss is announced");
        assert!(rig.driver.emitted.iter().filter(|e| e.kind == EventKind::ShipDestroyed).all(|e| e.payload.flag(Key::DuringSpaceCombat) == Some(true)), "a Waylay's hits are part of the space combat");
    }
    assert!(asked_sustain && asked_casualty, "the seeds cover a sustain offer and a casualty choice");
}

#[test]
fn the_hits_window_of_a_waylay_barrage_opens_even_when_the_target_has_no_fighter() {
    covers!("rules::combat::barrage::BarrageStep");
    let make = |seed: u64, waylay: bool| {
        let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::DestroyerII, 0, 1).unit(sys(), None, Unit::CruiserI, 1, 1);
        let (mut state, kernel) = sc.build();
        state.meta_mut().combat_round_seq = 4;
        if waylay {
            state.players_mut().by_seat_mut(Seat(0)).unwrap().card_timers.waylay_barrage_round.set(Some(4));
        }
        Rig::from_parts(state, kernel, BarrageStep::new(BarrageFacts { hit_texts: ti4_engine_core::kernel::children::HitTexts { staging: true, commander: Code(13), casualty: Code(14), effect_hit: Code(15) }, ..facts(true, true) })).seeded(seed)
    };
    let (mut opened, mut plain_opened) = (0, 0);
    for seed in 0..30u64 {
        let mut plain = make(seed, false);
        plain.run();
        assert!(plain.is_done());
        plain_opened += plain.driver.emitted.iter().filter(|e| e.kind == EventKind::AntiFighterBarrageHits).count();
        let mut rig = make(seed, true);
        rig.run();
        if hits_of(&rig.driver.window.drawn) > 0 {
            opened += rig.driver.emitted.iter().filter(|e| e.kind == EventKind::AntiFighterBarrageHits).count();
            assert_eq!(rig.state.board().sys(sys()).units.iter().filter(|p| p.owner() == Seat(1)).count(), 0, "seed {seed}: a lone cruiser takes the hit (a single casualty needs no question)");
        }
    }
    assert_eq!(plain_opened, 0, "no fighter, no card: nothing to hit, no window");
    assert!(opened > 0, "the card widens the hits to the cruiser, so the window opens");
}
