//! D4 lane A2: tests written from the first mutation run over `rules/combat` (survivors that were real gaps): the barrage's dice records and window
//! conditions, the cannon gates (the Argent flagship, attachments with SPACE CANNON), Graviton Laser System's conditions, the feat of the
//! space cannon offense, the Eidolon Maximum fighting from a planet, and the fields every question of these windows carries.

use ti4_engine_core::choice::combat::{Ask as CAsk, Opt as COpt};
use ti4_engine_core::generated::Attachment;
use ti4_engine_core::rules::combat::assault_cannon::{AssaultCannon, Facts as AcFacts};
use ti4_engine_core::rules::combat::assign::{Assign, AssignHits};
use ti4_engine_core::rules::combat::barrage::{BarrageStep, Facts as BarrageFacts};
use ti4_engine_core::rules::combat::cannon_guns::{attachment_guns, local_guns};
use ti4_engine_core::rules::combat::destroy_fighters::{DestroyFighters, Fighters};
use ti4_engine_core::rules::combat::flow::{Facts as FlowFacts, Flow, Phase, Step};
use ti4_engine_core::rules::combat::raid_formation::{Facts as RaidFacts, RaidFormation};
use ti4_engine_core::rules::combat::retreat::{Facts as RetreatFacts, Retreat};
use ti4_engine_core::rules::combat::space_cannon_offense::{Facts as CannonFacts, SpaceCannonOffense};
use ti4_engine_core::rules::combat::Rd;
use ti4_engine_core::state::{Code, EventKind, Feat, Key, SysFlags};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::rig::Policy;
use ti4_engine_core_tests::scenario::plain_systems;

fn sys() -> System {
    plain_systems().into_iter().find(|s| s.data().planets.len() >= 2).expect("a two-planet tile")
}

fn dest() -> System {
    plain_systems().into_iter().find(|s| *s != sys()).unwrap()
}

fn arena(a: Faction, d: Faction) -> Scenario {
    Scenario::new(&[a, d]).map_from(&[sys(), dest()])
}

fn count(rig: &Rig<impl Window>, seat: u8, f: impl Fn(Unit) -> bool) -> usize {
    rig.state.board().sys(sys()).units.iter().filter(|p| p.owner() == Seat(seat) && f(p.kind())).count()
}

// ---- the barrage ----------------------------------------------------------------------------------

fn bfacts(announce: bool) -> BarrageFacts {
    BarrageFacts {
        attacker: Seat(0),
        defender: Seat(1),
        system: sys(),
        round: 1,
        occurrence: 3,
        staging: announce,
        announce,
        phase: Code(1),
        cause: Code(2),
        kind: Code(3),
        extra: Code(4),
        unit_codes: vec![(Unit::DestroyerI, Code(10)), (Unit::DestroyerII, Code(11))],
        raid: None,
    }
}

#[test]
fn the_barrage_records_its_dice_for_the_presentation_with_the_unit_codes_the_target_and_whether_each_hit() {
    covers!("rules::combat::barrage::BarrageStep");
    for seed in 0..10u64 {
        let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::DestroyerI, 0, 1).unit(sys(), None, Unit::DestroyerII, 0, 1).unit(sys(), None, Unit::FighterI, 1, 6);
        let mut rig = Rig::new(sc, BarrageStep::new(bfacts(false))).seeded(seed);
        rig.run();
        let dice = rig.state.transient().combat.presentation.barrage_dice.clone();
        assert_eq!(dice.len(), 5, "two dice of the destroyer I and three of the destroyer II");
        assert_eq!(dice.iter().map(|d| d.unit).collect::<Vec<_>>(), vec![Code(10), Code(10), Code(11), Code(11), Code(11)], "each die carries the text of its unit type");
        assert!(dice.iter().all(|d| d.player == Seat(0)));
        assert_eq!(dice.iter().map(|d| d.target).collect::<Vec<_>>(), vec![9, 9, 6, 6, 6]);
        assert!(dice.iter().all(|d| d.hit == (d.roll >= d.target)), "a die hits at its number or above");
        assert_eq!(rig.state.transient().combat.presentation.phase, Code(1));
        let total = dice.iter().filter(|d| d.hit).count();
        assert_eq!(rig.state.transient().combat.presentation.barrage_hits.get(&Seat(0)).copied(), Some(total as u32));
        assert_eq!(count(&rig, 1, |_| true), 6usize.saturating_sub(total));
    }
}

#[test]
fn the_barrage_hits_window_opens_only_for_hits_on_a_target_with_fighters_and_with_the_timing_machinery() {
    covers!("rules::combat::barrage::BarrageStep");
    let with_hits = |units: &dyn Fn(Scenario) -> Scenario, announce: bool| -> Vec<Vec<EventKind>> {
        (0..30u64)
            .map(|seed| {
                let sc = units(arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::DestroyerI, 0, 1));
                let mut rig = Rig::new(sc, BarrageStep::new(bfacts(announce))).seeded(seed);
                rig.run();
                rig.driver.emitted.iter().map(|e| e.kind).collect::<Vec<_>>().into_iter().filter(|k| *k == EventKind::AntiFighterBarrageHits).collect()
            })
            .collect()
    };
    let fighters = with_hits(&|sc| sc.unit(sys(), None, Unit::FighterI, 1, 8), true);
    assert!(fighters.iter().any(|e| !e.is_empty()) && fighters.iter().any(|e| e.is_empty()), "with fighters: the window opens exactly when something hit");
    let none = with_hits(&|sc| sc.unit(sys(), None, Unit::CruiserI, 1, 2), true);
    assert!(none.iter().all(|e| e.is_empty()), "no fighters to hit: no window, however the dice fall");
    let quiet = with_hits(&|sc| sc.unit(sys(), None, Unit::FighterI, 1, 8), false);
    assert!(quiet.iter().all(|e| e.is_empty()), "no timing machinery: no window");
    // The fighters still fall without it.
    let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::DestroyerII, 0, 3).unit(sys(), None, Unit::FighterI, 1, 8);
    let mut rig = Rig::new(sc, BarrageStep::new(bfacts(false))).seeded(1);
    rig.run();
    assert!(count(&rig, 1, |u| u == Unit::FighterI) < 8);
}

// ---- who may fire ---------------------------------------------------------------------------------

#[test]
fn the_argent_flagship_bars_the_space_cannon_against_its_ships_whoever_is_active() {
    covers!("rules::combat::cannon_guns");
    let (p1, _) = (sys().data().planets[0], 0);
    // The Argent seat is the active one: the PDS of the other player cannot fire at it.
    let (s, k) = arena(Faction::TheArgentFlight, Faction::TheEmbersOfMuaat).unit(sys(), None, Unit::Quetzecoatl, 0, 1).unit(sys(), Some(p1), Unit::PDSI, 1, 1).control(p1, 1).build();
    assert!(local_guns(Rd::new(&s, &k), sys(), Seat(0)).iter().all(|p| p.owner() == Seat(0)), "the others' guns are barred");
    // Without the flagship they fire.
    let (s, k) = arena(Faction::TheArgentFlight, Faction::TheEmbersOfMuaat).unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), Some(p1), Unit::PDSI, 1, 1).control(p1, 1).build();
    assert!(local_guns(Rd::new(&s, &k), sys(), Seat(0)).iter().any(|p| p.owner() == Seat(1)));
    // The Argent seat defends: the active player's own guns are barred against its ship.
    let (s, k) = arena(Faction::TheEmbersOfMuaat, Faction::TheArgentFlight).unit(sys(), None, Unit::DreadnoughtI, 0, 1).unit(sys(), None, Unit::Quetzecoatl, 1, 1).build();
    assert!(local_guns(Rd::new(&s, &k), sys(), Seat(0)).iter().all(|p| p.owner() != Seat(0)), "the active player cannot fire at the flagship's fleet");
    // The bar is for the other players only: the Argent seat's own guns fire.
    let (s, k) = arena(Faction::TheEmbersOfMuaat, Faction::TheArgentFlight).unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::Quetzecoatl, 1, 1).build();
    assert!(local_guns(Rd::new(&s, &k), sys(), Seat(0)).iter().any(|p| p.owner() == Seat(1)));
}

#[test]
fn an_attachment_with_space_cannon_fires_for_the_planets_controller_as_if_it_were_a_unit() {
    covers!("rules::combat::cannon_guns");
    let (p1, p2) = (sys().data().planets[0], sys().data().planets[1]);
    let (mut s, k) = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 1).control(p1, 1).control(p2, 1).build();
    s.board_mut().planet_mut(p1).attach.push(Attachment::Geoform);
    s.board_mut().planet_mut(p2).attach.push(Attachment::Terraform);
    assert_eq!(attachment_guns(Rd::new(&s, &k), sys(), Seat(0)), vec![(Seat(1), p1, 5, 3)], "Geoform: SPACE CANNON 5 (x3); Terraform has none");
    // A planet nobody controls fires nothing.
    let (mut s, k) = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 1).build();
    s.board_mut().planet_mut(p1).attach.push(Attachment::Geoform);
    assert!(attachment_guns(Rd::new(&s, &k), sys(), Seat(0)).is_empty());
    // The offense rolls the attachment's three dice.
    let (mut s, k) = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 3).control(p1, 1).build();
    s.board_mut().planet_mut(p1).attach.push(Attachment::Geoform);
    let f = CannonFacts { active: Seat(0), system: sys(), announce: true, staging: false, reaching: vec![], mapped: false, kind: Code(1), cause: Code(2), casualty: Code(3), commander: Code(4) };
    let mut rig = Rig::from_parts(s, k, SpaceCannonOffense::new(f)).seeded(2);
    rig.run();
    assert_eq!(rig.driver.window.drawn.iter().map(|(on, d)| (*on, d.len())).collect::<Vec<_>>(), vec![(5, 3)]);
}

// ---- the space cannon offense -------------------------------------------------------------------------

fn cfacts() -> CannonFacts {
    CannonFacts { active: Seat(0), system: sys(), announce: true, staging: false, reaching: vec![], mapped: false, kind: Code(1), cause: Code(2), casualty: Code(3), commander: Code(4) }
}

fn graviton_rig(seed: u64, victims: &dyn Fn(Scenario) -> Scenario) -> Rig<SpaceCannonOffense> {
    let p1 = sys().data().planets[0];
    let sc = victims(arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).tech(1, Technology::GravitonLaserSystem).unit(sys(), Some(p1), Unit::PDSI, 1, 3).control(p1, 1));
    Rig::new(sc, SpaceCannonOffense::new(cfacts())).seeded(seed)
}

fn first_seed(make: impl Fn(u64) -> Rig<SpaceCannonOffense>) -> u64 {
    (0..80u64)
        .find(|s| {
            let mut r = make(*s);
            r.run();
            r.driver.window.drawn.iter().map(|(on, f)| f.iter().filter(|x| **x >= *on).count()).sum::<usize>() == 1
        })
        .expect("a seed with exactly one hit")
}

#[test]
fn graviton_laser_system_does_nothing_when_the_victim_has_only_fighters_or_only_non_fighters() {
    covers!("rules::combat::space_cannon_offense::SpaceCannonOffense");
    let only_cruisers = |sc: Scenario| sc.unit(sys(), None, Unit::CruiserI, 0, 2).unit(sys(), None, Unit::DestroyerI, 0, 1);
    let only_fighters = |sc: Scenario| sc.unit(sys(), None, Unit::FighterI, 0, 3);
    for victims in [&only_cruisers as &dyn Fn(Scenario) -> Scenario, &only_fighters] {
        let seed = first_seed(|s| graviton_rig(s, victims));
        let mut rig = graviton_rig(seed, victims);
        rig.run();
        assert_eq!(rig.driver.window.absorbing().map(|(_, bound)| bound), Some(false), "nothing to bind the loss to");
        assert!(!rig.state.players().by_seat(Seat(1)).unwrap().tech.exhausted.contains(Technology::GravitonLaserSystem), "the card is not exhausted when it changes nothing");
    }
}

#[test]
fn losing_the_last_non_fighters_scores_only_when_the_active_player_had_some_and_the_gunners_scored_hits() {
    covers!("rules::combat::space_cannon_offense::SpaceCannonOffense");
    let p1 = sys().data().planets[0];
    // Only fighters before: losing them is no feat.
    for seed in 0..30u64 {
        let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), Some(p1), Unit::PDSI, 1, 3).control(p1, 1).unit(sys(), None, Unit::FighterI, 0, 1);
        let mut rig = Rig::new(sc, SpaceCannonOffense::new(cfacts())).seeded(seed);
        rig.run();
        rig.auto(Policy::First);
        assert!(rig.driver.window.scoring.is_none(), "no non-fighter ship was lost");
        assert!(rig.state.players().by_seat(Seat(1)).unwrap().misc.event_feats.is_empty());
        assert_eq!(rig.state.meta().feat_occurrence_seq, 0);
    }
    // One cruiser before, three guns: the feat is scored exactly when it fell.
    let mut scored = false;
    for seed in 0..30u64 {
        let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), Some(p1), Unit::PDSI, 1, 3).control(p1, 1).unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::FighterI, 0, 1);
        let mut rig = Rig::new(sc, SpaceCannonOffense::new(cfacts())).seeded(seed);
        rig.run();
        rig.auto(Policy::First);
        let gone = count(&rig, 0, |u| u == Unit::CruiserI) == 0;
        assert_eq!(rig.driver.window.scoring.is_some(), gone, "seed {seed}");
        if gone {
            scored = true;
            assert_eq!(rig.state.players().by_seat(Seat(1)).unwrap().misc.event_feats.as_slice(), &[(Feat::SpaceCannonTookTheLastNonFighters, 1)]);
        }
    }
    assert!(scored);
}

// ---- the details of the small windows ---------------------------------------------------------------

#[test]
fn a_question_of_these_windows_is_not_optional_and_not_part_of_a_space_battle_and_names_its_system_or_not() {
    covers!("rules::combat::assault_cannon::AssaultCannon");
    covers!("rules::combat::raid_formation::RaidFormation");
    covers!("rules::combat::assign::AssignHits");
    let ac = AcFacts { attacker: Seat(0), defender: Seat(1), system: sys(), subtype: Code(20), cause: Code(21), announce: true };
    let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).tech(0, Technology::AssaultCannon).unit(sys(), None, Unit::CruiserI, 0, 3).unit(sys(), None, Unit::DestroyerI, 1, 1).unit(sys(), None, Unit::CarrierI, 1, 1);
    let mut cannon = Rig::new(sc, AssaultCannon::new(ac));
    cannon.run();
    let c = cannon.asked().ctx.clone();
    assert!(!c.optional && !c.space_battle && c.invasion_seq.is_none());
    assert!(c.outstanding.is_empty());
    assert_eq!(c.target, Some(ti4_engine_core::choice::Target::System(sys())));
    let raid = RaidFacts { producer: Seat(0), target: Seat(1), system: sys(), excess: 2, subtype: Code(8) };
    let sc = arena(Faction::TheArgentFlight, Faction::TheEmbersOfMuaat).unit(sys(), None, Unit::DreadnoughtI, 1, 1).unit(sys(), None, Unit::WarSun, 1, 1);
    let mut raid_rig = Rig::new(sc, RaidFormation::new(raid));
    raid_rig.run();
    let c = raid_rig.asked().ctx.clone();
    assert!(!c.optional && !c.space_battle && c.invasion_seq.is_none() && c.target.is_none() && c.outstanding.is_empty());
    let a = Assign { player: Seat(0), system: sys(), ability: false, non_fighters_first: false, subtype: Code(5), cause: Code(6), during_space_combat: true, staging: false, announce: false };
    let mut assign = Rig::new(arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::DestroyerI, 0, 1), AssignHits::new(a, 1));
    assign.run();
    let c = assign.asked().ctx.clone();
    assert!(!c.optional && !c.space_battle && c.invasion_seq.is_none());
    assert_eq!(c.outstanding.as_slice()[0].paid, 0);
    assert_eq!(c.target, Some(ti4_engine_core::choice::Target::System(sys())));
    assert_eq!(assign.asked().ask, Ask::Combat(CAsk::AssignHit));
}

#[test]
fn losses_announced_by_these_windows_are_marked_as_during_a_space_combat() {
    covers!("rules::combat::destroy_fighters::DestroyFighters");
    covers!("rules::combat::assault_cannon::AssaultCannon");
    let f = Fighters { target: Seat(1), system: sys(), hits: 1, cause: Code(5), staging: true, announce: true };
    let mut rig = Rig::new(arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::FighterI, 1, 2), DestroyFighters::new(f));
    rig.run();
    let ev = rig.driver.emitted.iter().find(|e| e.kind == EventKind::ShipDestroyed).unwrap();
    assert_eq!(ev.payload.flag(Key::DuringSpaceCombat), Some(true));
    let ac = AcFacts { attacker: Seat(0), defender: Seat(1), system: sys(), subtype: Code(20), cause: Code(21), announce: true };
    let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).tech(0, Technology::AssaultCannon).unit(sys(), None, Unit::CruiserI, 0, 3).unit(sys(), None, Unit::DestroyerI, 1, 1);
    let mut cannon = Rig::new(sc, AssaultCannon::new(ac));
    cannon.run();
    let ev = cannon.driver.emitted.iter().find(|e| e.kind == EventKind::ShipDestroyed).unwrap();
    assert_eq!(ev.payload.flag(Key::DuringSpaceCombat), Some(true));
}

#[test]
fn a_retreat_touches_the_destination_tile() {
    covers!("rules::combat::retreat::Retreat");
    let f = RetreatFacts { player: Seat(0), system: sys(), destination: dest(), staging: false, cause: Code(7), pool: Code(8) };
    let mut rig = Rig::new(arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 1), Retreat::new(f));
    rig.run();
    assert!(rig.state.board().sys(dest()).flags.has(SysFlags::TOUCHED), "the destination now has an entry in the board");
}

#[test]
fn the_retreat_step_is_announced_only_at_the_defenders_first_turn() {
    covers!("rules::combat::flow::Flow");
    let facts = FlowFacts {
        attacker: Seat(0),
        defender: Seat(1),
        system: sys(),
        announce: true,
        over: false,
        asking_can_retreat: false,
        leaver_destinations: 1,
        front_alive: 1,
        offered_before: false,
        sustainers: false,
        pending_retreats: vec![],
        phase: Phase::Round,
        resuming: false,
    };
    let run = |asking: u8, announced: Vec<Seat>| {
        let mut rig = Rig::new(arena(Faction::TheEmbersOfMuaat, Faction::TheArborec), Flow::new(facts.clone(), Step::Announcing { round: 1, asking: Seat(asking), announced }));
        rig.run();
        rig.driver.emitted.iter().map(|e| e.kind).collect::<Vec<_>>()
    };
    assert_eq!(run(1, vec![]), vec![EventKind::RetreatStepStarted]);
    assert!(run(0, vec![]).is_empty(), "the attacker's turn does not open the step again");
    assert!(run(1, vec![Seat(1)]).is_empty());
}

// ---- the Eidolon Maximum fights from its planet ------------------------------------------------------------

#[test]
fn an_eidolon_maximum_on_a_planet_takes_hits_with_the_fleet_and_leaves_the_planet_when_lost() {
    covers!("rules::combat::assign::AssignHits");
    let p1 = sys().data().planets[0];
    let a = Assign { player: Seat(1), system: sys(), ability: false, non_fighters_first: false, subtype: Code(5), cause: Code(6), during_space_combat: true, staging: false, announce: false };
    let sc = || arena(Faction::TheEmbersOfMuaat, Faction::TheNaazRokhaAlliance).unit(sys(), None, Unit::CruiserI, 1, 1).unit(sys(), Some(p1), Unit::EidolonMaximum, 1, 1).control(p1, 1);
    let mut rig = Rig::new(sc(), AssignHits::new(Assign { player: Seat(1), ..a }, 1));
    rig.run();
    let kinds: Vec<Unit> = rig.opts().iter().filter_map(|o| if let Opt::Combat(COpt::Destroy { unit, .. }) = o { Some(*unit) } else { None }).collect();
    assert_eq!(kinds, vec![Unit::CruiserI, Unit::EidolonMaximum], "the Maximum fights with the ships");
    rig.pick(1);
    assert!(rig.state.board().planet(p1).units.is_empty(), "it leaves its planet when it falls");
    assert_eq!(count(&rig, 1, |u| u == Unit::CruiserI), 1);
    // A hit of a unit ability cannot be assigned to it (Naaz).
    let mut ability = Rig::new(sc(), AssignHits::new(Assign { player: Seat(1), ability: true, ..a }, 1));
    ability.run();
    assert!(ability.is_done(), "the only candidate left is the cruiser: lost unasked");
    assert_eq!(count(&ability, 1, |u| u == Unit::CruiserI), 0);
    assert_eq!(ability.state.board().planet(p1).units.len(), 1, "the Maximum is immune to unit abilities");
}
