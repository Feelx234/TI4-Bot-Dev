//! W15: the hosts of Scramble Frequency stage their dice before the event `UNIT_ABILITY_ROLLED` and take the hits from the faces that remain
//! after it. A stub ability stands for the card: it draws every die of the roller's staged set as a 10 (so the expected hits are known
//! without a seed) and writes the ledger rows of the redraws as the registered card does. The registered card itself is tested in
//! `ti4-engine-core-factions::cards`, the whole play in the lockstep tests of `ti4-engine`.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::kernel::ability::{AbilityTable, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::rules::combat::barrage::{BarrageStep, Facts as BarrageFacts};
use ti4_engine_core::rules::combat::space_cannon_offense::{Facts as CannonFacts, SpaceCannonOffense};
use ti4_engine_core::rules::invasion::bombard_roll::{BombardRolled, Facts as RollFacts};
use ti4_engine_core::state::{Code, EventKind, Key, RerollEntry, RerollSet, RollPurpose};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::rig::Policy;
use ti4_engine_core_tests::scenario::plain_systems;
use ti4_engine_core_tests::windows::EmitEvent;

const STUB: Slot = Slot(9_001);

fn sys() -> System {
    plain_systems().into_iter().find(|s| s.data().planets.len() >= 2).expect("a tile with two planets")
}

/// The card: every die of the staged set of the seat that rolled becomes a 10 (a hit on any number); the redraws are in the ledger.
fn scramble_stub(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    // The event shows the hits of the faces the roll made (before any card).
    let shown = eff.event.payload.int(Key::Hits).expect("the event states the hits");
    let s = eff.state();
    let roller = s.transient().combat.last_reroll_player.expect("the roller is named while the event goes out");
    let staging = s.transient_mut().combat.reroll_staging.edit();
    let (_, set) = staging.iter_mut().find(|(seat, _)| *seat == roller).expect("the dice of the roller are staged while the event goes out");
    // (The hits count the ADJUSTED face: a die with a +3 hits on 5 as a 5 does. The stub counted the printed faces, which contradicted the
    // assertion of the bombardment test below that the staged 2 with a +3 is a hit: failing on `core-w-l15` itself.)
    let staged = set
        .rolls
        .iter()
        .map(|e| e.faces.iter().enumerate().filter(|(i, f)| e.hits_on.is_some_and(|on| i64::from(**f) + e.deltas.get(&u8::try_from(*i).unwrap_or(u8::MAX)).map_or(0, |d| i64::from(*d)) >= i64::from(on))).count())
        .sum::<usize>();
    assert_eq!(shown, i64::try_from(staged).unwrap(), "the payload of the event states the hits of the staged faces");
    let mut redrawn = Vec::new();
    for entry in &mut set.rolls {
        for d in 0..entry.faces.len() {
            entry.faces[d] = 10;
            entry.rerolled |= 1 << d;
            redrawn.push((entry.hits_on, entry.faces.to_vec(), d));
        }
    }
    let system = set.system;
    for (hits_on, faces, d) in redrawn {
        s.ledger_mut().dice.push(ti4_engine_core::state::RollRec { purpose: RollPurpose::Reroll, owner: Some(roller), unit: None, planet: None, scene: Some(system), hits_on, faces: faces.into_iter().collect(), rerolled: 1 << d });
    }
    Ok(())
}

/// Only one seat other than the roller plays it (the printed "another player"): the lowest seat that is not the roller.
fn another_seat(c: &ti4_engine_core::kernel::ability::CondCx<'_>) -> bool {
    let roller = c.event.payload.seat(Key::Player);
    roller != Some(c.def.owner) && c.def.owner == if roller == Some(Seat(0)) { Seat(1) } else { Seat(0) }
}

fn table() -> AbilityTable {
    let mut t = AbilityTable::default();
    t.add(Spec::new(STUB, AbilityKind::Other, EventKind::UnitAbilityRolled, Relation::After, scramble_stub).when(another_seat));
    t
}

fn barrage_facts() -> BarrageFacts {
    BarrageFacts {
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
    }
}

fn fighters_of(rig: &Rig<impl Window>, seat: u8) -> usize {
    rig.state.board().sys(sys()).units.iter().filter(|p| p.owner() == Seat(seat) && p.kind() == Unit::FighterI).count()
}

#[test]
fn a_card_that_draws_the_barrage_dice_again_decides_the_hits_the_window_takes() {
    covers!("rules::combat::barrage::BarrageStep");
    // Both sides barrage: two destroyers roll two dice each; the stub makes the four dice of each side hit, whatever they showed.
    let arena = || {
        Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
            .map_from(&[sys()])
            .unit(sys(), None, Unit::DestroyerI, 0, 2)
            .unit(sys(), None, Unit::FighterI, 0, 6)
            .unit(sys(), None, Unit::DestroyerI, 1, 2)
            .unit(sys(), None, Unit::FighterI, 1, 6)
    };
    let mut changed = false;
    for seed in 0..40u64 {
        let mut plain = Rig::new(arena(), BarrageStep::new(barrage_facts())).seeded(seed).with_abilities(table());
        let mut bare = Rig::new(arena(), BarrageStep::new(BarrageFacts { announce: false, ..barrage_facts() })).seeded(seed);
        plain.run();
        bare.run();
        assert!(plain.is_done() && bare.is_done());
        assert_eq!((fighters_of(&plain, 0), fighters_of(&plain, 1)), (6 - 4, 6 - 4), "seed {seed}: the hits of each side are read from the faces that remain");
        assert!(plain.state.transient().combat.reroll_staging.is_empty(), "the staging is spent");
        assert_eq!(plain.state.transient().combat.last_reroll_player, None, "the roller is no longer named after the window");
        // The window's record of its rolls holds the redraws beside the rolls (four per side) and leaves them out of the rolls of its own reason.
        assert_eq!(plain.driver.window.drawn.len() - plain.driver.window.first_draws().len(), 8, "eight dice drawn again are eight records");
        assert_eq!(plain.driver.window.first_draws(), bare.driver.window.drawn, "the rolls of the barrage itself are the same with or without the card");
        let ledger_rerolls: Vec<_> = plain.state.ledger().dice.iter().filter(|r| r.purpose == RollPurpose::Reroll).map(|r| r.owner).collect();
        assert_eq!(ledger_rerolls, [vec![Some(Seat(0)); 4], vec![Some(Seat(1)); 4]].concat(), "the rows are the roller's, side by side");
        changed |= (fighters_of(&bare, 0), fighters_of(&bare, 1)) != (2, 2);
    }
    assert!(changed, "some seed shows the card changing the outcome");
}

#[test]
fn a_card_that_draws_the_space_cannon_dice_again_decides_the_hits_of_each_gunner() {
    covers!("rules::combat::space_cannon_offense::SpaceCannonOffense");
    let p1 = sys().data().planets[0];
    let p2 = sys().data().planets[1];
    // Two gunners (seats 1 and 2) fire three PDS each at the active seat's five cruisers.
    let arena = || {
        Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec, Faction::TheVuilraithCabal])
            .map_from(&[sys()])
            .unit(sys(), Some(p1), Unit::PDSI, 1, 3)
            .control(p1, 1)
            .unit(sys(), Some(p2), Unit::PDSI, 2, 3)
            .control(p2, 2)
            .unit(sys(), None, Unit::CruiserI, 0, 12)
    };
    let facts = CannonFacts { active: Seat(0), system: sys(), announce: true, staging: false, reaching: vec![], mapped: false, kind: Code(1), cause: Code(2), casualty: Code(3), commander: Code(4), jolnar: vec![], unit_texts: vec![], planet_texts: vec![] };
    let (mut one, mut both) = (0, 0);
    for seed in 0..60u64 {
        let mut bare = Rig::new(arena(), SpaceCannonOffense::new(CannonFacts { announce: false, ..facts.clone() })).seeded(seed);
        bare.run();
        bare.auto(Policy::First);
        let rows: Vec<_> = bare.state.ledger().dice.iter().filter(|r| r.purpose == RollPurpose::SpaceCannon).cloned().collect();
        let scored = |seat: u8| rows.iter().filter(|r| r.owner == Some(Seat(seat))).map(|r| r.faces.iter().filter(|f| Some(**f) >= r.hits_on).count()).sum::<usize>();
        let (b_hits, c_hits) = (scored(1), scored(2));
        let mut rig = Rig::new(arena(), SpaceCannonOffense::new(facts.clone())).seeded(seed).with_abilities(table());
        rig.run();
        rig.auto(Policy::First);
        assert!(rig.is_done());
        let first = rig.driver.window.first_draws();
        assert_eq!(first, bare.driver.window.drawn, "seed {seed}: the guns' own rolls are the same with or without the card");
        // A gunner with no hit gets no window; a gunner with one has all its three dice hit after the card.
        let expected_dice = usize::from(b_hits > 0) * 3 + usize::from(c_hits > 0) * 3;
        assert_eq!(rig.driver.window.drawn.len() - first.len(), expected_dice, "seed {seed}: one record per die of each gunner that scored");
        let cruisers = rig.state.board().sys(sys()).units.iter().filter(|p| p.owner() == Seat(0) && p.kind() == Unit::CruiserI).count();
        let hits_after = (if b_hits > 0 { 3 } else { 0 }) + (if c_hits > 0 { 3 } else { 0 });
        assert_eq!(cruisers, 12 - hits_after, "seed {seed}: the hits are read from the faces that remain");
        assert!(rig.state.transient().combat.reroll_staging.is_empty() && rig.state.transient().combat.last_reroll_player.is_none());
        one += usize::from((b_hits > 0) != (c_hits > 0));
        both += usize::from(b_hits > 0 && c_hits > 0);
    }
    assert!(one > 0 && both > 0, "the seeds cover one gunner scoring ({one}) and both ({both})");
}

#[test]
fn the_reroll_window_of_a_bombardment_emits_the_event_with_the_staged_hits_and_reads_the_faces_back() {
    covers!("rules::invasion::bombard_roll::BombardRolled");
    let stage = |s: &mut CoreState| {
        let entry = |faces: &[u8], bonus: Option<(u8, i8)>| {
            let mut deltas = ti4_engine_core::containers::SortedMap::default();
            if let Some((die, by)) = bonus {
                deltas.insert(die, by);
            }
            RerollEntry { unit: Code(1), planet: None, hits_on: Some(5), faces: faces.iter().copied().collect(), rerolled: 0, deltas, unit_types: ti4_engine_core::containers::SortedMap::default() }
        };
        // A 2 with a +3 adjustment hits on 5 as a 5 does: the hits count the adjusted face.
        let set = RerollSet { kind: Code(7), system: sys(), rolls: vec![entry(&[2, 6], Some((0, 3))), entry(&[5], None), entry(&[1], Some((0, 1)))] };
        s.transient_mut().combat.reroll_staging.edit().push((Seat(0), set));
        // (The string names the roller when it stages the roll.)
        s.transient_mut().combat.last_reroll_player = Some(Seat(0));
    };
    let facts = RollFacts { invader: Seat(0), system: sys(), announce: true, kind: Code(7), commander: false, shown: vec![] };
    let (mut state, kernel) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()]).build();
    stage(&mut state);
    let mut rig = Rig::from_parts(state, kernel, BombardRolled::new(facts.clone())).with_abilities(table());
    rig.run();
    assert!(rig.is_done());
    let event = rig.driver.emitted.iter().find(|e| e.kind == EventKind::UnitAbilityRolled).expect("the event goes out");
    assert_eq!(event.payload.seat(Key::Player), Some(Seat(0)));
    assert_eq!(event.payload.int(Key::Hits), Some(3), "a 2 adjusted by +3, a 6 and a 5 hit on 5, a 1 adjusted by +1 does not: the hits are those of the staged faces when the event goes out");
    assert_eq!(rig.driver.window.faces, vec![vec![10, 10], vec![10], vec![10]], "the faces the card left are read back");
    assert!(rig.state.transient().combat.reroll_staging.is_empty(), "the staging is spent");
    assert_eq!(rig.state.transient().combat.last_reroll_player, None);
    // Without staged dice there is no window, and without the resolver's announcements the dice are only read.
    let (state, kernel) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()]).build();
    let mut none = Rig::from_parts(state, kernel, BombardRolled::new(facts.clone())).with_abilities(table());
    none.run();
    assert!(none.is_done() && none.driver.emitted.is_empty() && none.driver.window.faces.is_empty());
    let (mut state, kernel) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()]).build();
    stage(&mut state);
    let mut quiet = Rig::from_parts(state, kernel, BombardRolled::new(RollFacts { announce: false, ..facts.clone() })).with_abilities(table());
    quiet.run();
    assert!(quiet.driver.emitted.is_empty());
    assert_eq!(quiet.driver.window.faces, vec![vec![2, 6], vec![5], vec![1]]);
    let _ = EmitEvent::new;
}
