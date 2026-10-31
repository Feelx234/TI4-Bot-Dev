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
    let s = eff.state();
    let roller = s.transient().combat.last_reroll_player.expect("the roller is named while the event goes out");
    let staging = s.transient_mut().combat.reroll_staging.edit();
    let (_, set) = staging.iter_mut().find(|(seat, _)| *seat == roller).expect("the dice of the roller are staged while the event goes out");
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

/// Only a seat other than the roller may play it (the printed "another player").
fn another_seat(c: &ti4_engine_core::kernel::ability::CondCx<'_>) -> bool {
    c.event.payload.seat(Key::Player) != Some(c.def.owner)
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
    let arena = || Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()]).unit(sys(), None, Unit::DestroyerI, 0, 2).unit(sys(), None, Unit::FighterI, 1, 6);
    let (mut changed, mut unchanged) = (false, false);
    for seed in 0..40u64 {
        let mut plain = Rig::new(arena(), BarrageStep::new(barrage_facts())).seeded(seed).with_abilities(table());
        let mut bare = Rig::new(arena(), BarrageStep::new(BarrageFacts { announce: false, ..barrage_facts() })).seeded(seed);
        plain.run();
        bare.run();
        assert!(plain.is_done() && bare.is_done());
        // Two destroyers roll two dice each: the stub makes all four dice hit, whatever they showed; the other seat has no ship that rolls.
        assert_eq!(fighters_of(&plain, 1), 6 - 4, "seed {seed}: the hits are read from the faces that remain");
        assert!(plain.state.transient().combat.reroll_staging.is_empty(), "the staging is spent");
        assert_eq!(plain.state.transient().combat.last_reroll_player, None, "the roller is no longer named after the window");
        // The window's record of its rolls holds the redraws beside the rolls, and leaves them out of the rolls of its own reason.
        let redraws = plain.driver.window.drawn.len() - plain.driver.window.first_draws().len();
        assert_eq!(redraws, 4, "four dice drawn again are four records");
        assert_eq!(plain.driver.window.first_draws(), bare.driver.window.drawn, "the rolls of the barrage itself are the same with or without the card");
        let ledger_rerolls = plain.state.ledger().dice.iter().filter(|r| r.purpose == RollPurpose::Reroll).count();
        assert_eq!(ledger_rerolls, 4);
        changed |= fighters_of(&bare, 1) != 2;
        unchanged |= fighters_of(&bare, 1) == 2;
    }
    assert!(changed, "some seed shows the card changing the outcome");
    let _ = unchanged;
}

#[test]
fn a_card_that_draws_the_space_cannon_dice_again_decides_the_hits_of_each_gunner() {
    covers!("rules::combat::space_cannon_offense::SpaceCannonOffense");
    let p1 = sys().data().planets[0];
    let arena = || {
        Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
            .map_from(&[sys()])
            .unit(sys(), Some(p1), Unit::PDSI, 1, 3)
            .control(p1, 1)
            .unit(sys(), None, Unit::CruiserI, 0, 5)
    };
    let facts = CannonFacts { active: Seat(0), system: sys(), announce: true, staging: false, reaching: vec![], mapped: false, kind: Code(1), cause: Code(2), casualty: Code(3), commander: Code(4) };
    let mut played = 0;
    for seed in 0..40u64 {
        let mut bare = Rig::new(arena(), SpaceCannonOffense::new(CannonFacts { announce: false, ..facts.clone() })).seeded(seed);
        bare.run();
        bare.auto(Policy::First);
        let before: usize = bare.driver.window.drawn.iter().map(|(on, f)| f.iter().filter(|x| **x >= *on).count()).sum();
        let mut rig = Rig::new(arena(), SpaceCannonOffense::new(facts.clone())).seeded(seed).with_abilities(table());
        rig.run();
        rig.auto(Policy::First);
        assert!(rig.is_done());
        let rolled: usize = rig.driver.window.first_draws().iter().map(|(_, f)| f.len()).sum();
        let first: Vec<_> = rig.driver.window.first_draws();
        assert_eq!(first, bare.driver.window.drawn, "seed {seed}: the guns' own rolls are the same with or without the card");
        if before == 0 {
            // A gunner with no hit gets no window: nothing can be drawn again, nothing is played.
            assert_eq!(rig.driver.window.drawn.len(), first.len(), "seed {seed}: no event without a hit");
            continue;
        }
        played += 1;
        // Every die of the gunner now hits; each hit costs the active seat a ship.
        let cruisers = rig.state.board().sys(sys()).units.iter().filter(|p| p.owner() == Seat(0) && p.kind() == Unit::CruiserI).count();
        assert_eq!(cruisers, 5usize.saturating_sub(rolled), "seed {seed}: {rolled} dice, all hitting after the card");
        assert_eq!(rig.driver.window.drawn.len() - first.len(), rolled, "one record per die drawn again");
        assert!(rig.state.transient().combat.reroll_staging.is_empty() && rig.state.transient().combat.last_reroll_player.is_none());
    }
    assert!(played > 5, "the gunner scored in {played} of the runs");
}

#[test]
fn the_reroll_window_of_a_bombardment_emits_the_event_with_the_staged_hits_and_reads_the_faces_back() {
    covers!("rules::invasion::bombard_roll::BombardRolled");
    let stage = |s: &mut CoreState| {
        let entry = |faces: &[u8]| RerollEntry {
            unit: Code(1),
            planet: None,
            hits_on: Some(5),
            faces: faces.iter().copied().collect(),
            rerolled: 0,
            deltas: ti4_engine_core::containers::SortedMap::default(),
            unit_types: ti4_engine_core::containers::SortedMap::default(),
        };
        let set = RerollSet { kind: Code(7), system: sys(), rolls: vec![entry(&[2, 6]), entry(&[5])] };
        s.transient_mut().combat.reroll_staging.edit().push((Seat(0), set));
        s.transient_mut().combat.last_reroll_player = Some(Seat(0));
    };
    let facts = RollFacts { invader: Seat(0), system: sys(), announce: true, kind: Code(7) };
    let (mut state, kernel) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()]).build();
    stage(&mut state);
    let mut rig = Rig::from_parts(state, kernel, BombardRolled::new(facts)).with_abilities(table());
    rig.run();
    assert!(rig.is_done());
    let event = rig.driver.emitted.iter().find(|e| e.kind == EventKind::UnitAbilityRolled).expect("the event goes out");
    assert_eq!(event.payload.seat(Key::Player), Some(Seat(0)));
    assert_eq!(event.payload.int(Key::Hits), Some(2), "a 6 and a 5 hit on 5: the hits are those of the staged faces when the event goes out");
    assert_eq!(rig.driver.window.faces, vec![vec![10, 10], vec![10]], "the faces the card left are read back");
    assert!(rig.state.transient().combat.reroll_staging.is_empty(), "the staging is spent");
    assert_eq!(rig.state.transient().combat.last_reroll_player, None);
    // Without staged dice there is no window, and without the resolver's announcements the dice are only read.
    let (state, kernel) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()]).build();
    let mut none = Rig::from_parts(state, kernel, BombardRolled::new(facts)).with_abilities(table());
    none.run();
    assert!(none.is_done() && none.driver.emitted.is_empty() && none.driver.window.faces.is_empty());
    let (mut state, kernel) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()]).build();
    stage(&mut state);
    let mut quiet = Rig::from_parts(state, kernel, BombardRolled::new(RollFacts { announce: false, ..facts })).with_abilities(table());
    quiet.run();
    assert!(quiet.driver.emitted.is_empty());
    assert_eq!(quiet.driver.window.faces, vec![vec![2, 6], vec![5]]);
    let _ = EmitEvent::new;
}
