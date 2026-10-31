//! W16: the Jol-Nar commander ("after you roll dice for a unit ability: you may reroll any of those dice") asks its holder about each die of
//! a bombardment and of the space cannon offense before the event `UNIT_ABILITY_ROLLED` goes out (`kernel::commander_walk::Walk`).
//! One optional question per die in the order of the staged entries, the picked dice drawn again afterwards, the event and the hits read
//! the faces that remain. The whole play against the string engine is the lockstep tests of `ti4-engine`.

use ti4_engine_core::choice::combat::{Ask as CAsk, Opt as COpt, RerollSource, Subtype as CSub};
use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::{Origin, Subtype, Target};
use ti4_engine_core::rules::combat::space_cannon_offense::{Facts as CannonFacts, SpaceCannonOffense};
use ti4_engine_core::rules::invasion::bombard_roll::{BombardRolled, Facts as RollFacts};
use ti4_engine_core::state::{Code, EventKind, Key, RerollEntry, RerollSet, RollPurpose};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::rig::Policy;
use ti4_engine_core_tests::scenario::plain_systems;

fn sys() -> System {
    plain_systems().into_iter().find(|s| s.data().planets.len() >= 2).expect("a tile with two planets")
}

// ---- the bombardment -----------------------------------------------------------------------------

fn staged(faces: &[&[u8]], hits_on: u8) -> RerollSet {
    let rolls = faces
        .iter()
        .enumerate()
        .map(|(i, f)| RerollEntry {
            unit: Code(u16::try_from(i).unwrap() + 1),
            planet: None,
            hits_on: Some(hits_on),
            faces: f.iter().copied().collect(),
            rerolled: 0,
            deltas: ti4_engine_core::containers::SortedMap::default(),
            unit_types: ti4_engine_core::containers::SortedMap::default(),
            kind: None,
        })
        .collect();
    RerollSet { kind: Code(7), system: sys(), rolls }
}

fn bombard_rig(seed: u64, commander: bool, set: RerollSet) -> Rig<BombardRolled> {
    let (mut state, kernel) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()]).build();
    state.transient_mut().combat.reroll_staging.edit().push((Seat(0), set));
    state.transient_mut().combat.last_reroll_player = Some(Seat(0));
    let shown = if commander { vec![Code(21), Code(22), Code(23)] } else { vec![] };
    let facts = RollFacts { invader: Seat(0), system: sys(), announce: true, kind: Code(7), commander, shown };
    Rig::from_parts(state, kernel, BombardRolled::new(facts)).seeded(seed)
}

#[test]
fn the_invader_with_the_commander_is_asked_about_each_die_of_the_bombardment_and_the_event_shows_the_final_hits() {
    covers!("rules::invasion::bombard_roll::BombardRolled");
    covers!("kernel::commander_walk::Walk");
    let (mut redrawn_any, mut kept_any) = (false, false);
    for seed in 0..40u64 {
        let before: [&[u8]; 3] = [&[2, 6], &[1], &[4, 9, 3]];
        let mut rig = bomb_rig_run(seed, &before);
        // The questions: every die of every entry in order, labelled by the entry's shown text.
        let order = [(0u8, 0u8), (0, 1), (1, 0), (2, 0), (2, 1), (2, 2)];
        let mut picked = Vec::new();
        for (n, (e, d)) in order.into_iter().enumerate() {
            assert!(!rig.is_done(), "seed {seed}: question {n}");
            let asked = rig.asked();
            assert_eq!(asked.actor, Seat(0));
            assert_eq!(asked.ask, Ask::Combat(CAsk::RerollDie { source: RerollSource::Commander, unit: Code(21 + u16::from(e)), die: d }), "seed {seed}: question {n}");
            assert_eq!(asked.ctx.origin, Origin::Content(Content::Leader(ti4_engine_core::generated::Leader::AgnlanOln)));
            assert_eq!(asked.ctx.subtype, Subtype::Combat(CSub::Reroll(RerollSource::Commander)));
            assert_eq!(asked.ctx.target, Some(Target::System(sys())));
            assert!(!asked.ctx.optional && !asked.ctx.space_battle && asked.ctx.invasion_seq.is_none(), "an ordinary question about the system");
            assert_eq!(rig.opts().len(), 2, "reroll the die or decline");
            let Opt::Combat(COpt::RerollDie { unit, entry, die, face, hits_on }) = rig.opts()[0] else { panic!("the first option rerolls") };
            assert_eq!((unit, entry, die, hits_on), (Code(21 + u16::from(e)), e, d, Some(5)));
            assert_eq!(face, before[usize::from(e)][usize::from(d)], "the question shows the face the die has now");
            // Reroll the dice that miss (face below 5), keep the hits.
            if face < 5 {
                picked.push((e, d));
                rig.pick(0);
            } else {
                rig.decline();
            }
        }
        assert!(rig.is_done());
        let w = &rig.driver.window;
        // Dice that were not picked are as they were; the picked ones were drawn again (a die is 1..=10).
        for (e, faces) in before.iter().enumerate() {
            for (d, face) in faces.iter().enumerate() {
                let now = w.faces[e][d];
                if picked.contains(&(u8::try_from(e).unwrap(), u8::try_from(d).unwrap())) {
                    assert!((1..=10).contains(&now));
                    redrawn_any = true;
                } else {
                    assert_eq!(now, *face, "seed {seed}: a die that was kept stays");
                    kept_any = true;
                }
            }
        }
        let hits = w.faces.iter().flatten().filter(|f| **f >= 5).count();
        let event = rig.driver.emitted.iter().find(|e| e.kind == EventKind::UnitAbilityRolled).expect("the event goes out after the rerolls");
        assert_eq!(event.payload.int(Key::Hits), Some(i64::try_from(hits).unwrap()), "seed {seed}: the event shows the hits of the faces that remain");
        let rows: Vec<_> = rig.state.ledger().dice.iter().filter(|r| r.purpose == RollPurpose::Reroll).collect();
        assert_eq!(rows.len(), picked.len(), "one ledger row per die drawn again");
        for (row, (e, d)) in rows.iter().zip(&picked) {
            assert_eq!(row.rerolled, 1 << d, "the row marks the die");
            assert_eq!(row.owner, Some(Seat(0)));
            assert_eq!(row.hits_on, Some(5));
            assert_eq!(row.faces.len(), before[usize::from(*e)].len(), "the faces of the whole entry after the draw");
        }
        assert!(rig.state.transient().combat.reroll_staging.is_empty() && rig.state.transient().combat.last_reroll_player.is_none(), "the staging is spent");
    }
    assert!(redrawn_any && kept_any);
}

fn bomb_rig_run(seed: u64, before: &[&[u8]; 3]) -> Rig<BombardRolled> {
    let mut rig = bombard_rig(seed, true, staged(before, 5));
    rig.run();
    rig
}

#[test]
fn a_redraw_drops_the_adjustment_the_die_carried_and_a_declined_walk_changes_nothing() {
    covers!("kernel::commander_walk::Walk");
    // A +3 on the 2 makes it hit on 5; drawn again it is a plain 1..=10, so the hits follow the new face.
    let mut set = staged(&[&[2, 1]], 5);
    set.rolls[0].deltas.insert(0, 3);
    let mut rig = bombard_rig(3, true, set);
    rig.run();
    let Opt::Combat(COpt::RerollDie { face, .. }) = rig.opts()[0] else { panic!() };
    assert_eq!(face, 2, "the question shows the printed face, not the adjusted one");
    rig.pick(0);
    rig.decline();
    assert!(rig.is_done());
    let now = rig.driver.window.faces[0][0];
    let event = rig.driver.emitted.iter().find(|e| e.kind == EventKind::UnitAbilityRolled).unwrap();
    assert_eq!(event.payload.int(Key::Hits), Some(i64::from(now >= 5)), "the adjustment is gone with the old face");
    // Declining everything draws nothing: the faces, the ledger and the streams are as the staging had them.
    let mut keep = bombard_rig(3, true, staged(&[&[2, 6]], 5));
    keep.run();
    keep.decline();
    keep.decline();
    assert!(keep.is_done());
    assert_eq!(keep.driver.window.faces, vec![vec![2, 6]]);
    assert!(keep.state.ledger().dice.is_empty());
    let hits = keep.driver.emitted.iter().find(|e| e.kind == EventKind::UnitAbilityRolled).unwrap().payload.int(Key::Hits);
    assert_eq!(hits, Some(1));
}

#[test]
fn a_bombardment_without_the_commander_or_without_dice_asks_nothing() {
    covers!("rules::invasion::bombard_roll::BombardRolled");
    let mut plain = bombard_rig(1, false, staged(&[&[2, 6]], 5));
    plain.run();
    assert!(plain.is_done(), "no commander, no question");
    assert_eq!(plain.driver.window.faces, vec![vec![2, 6]]);
    // The commander with an empty staged set (no die was rolled): nothing to ask about, the window just reports.
    let mut empty = bombard_rig(1, true, staged(&[&[]], 5));
    empty.run();
    assert!(empty.is_done());
    // A commander and no staged set for the invader: no window at all.
    let (state, kernel) = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys()]).build();
    let facts = RollFacts { invader: Seat(0), system: sys(), announce: true, kind: Code(7), commander: true, shown: vec![] };
    let mut none = Rig::from_parts(state, kernel, BombardRolled::new(facts));
    none.run();
    assert!(none.is_done() && none.driver.emitted.is_empty());
}

// ---- the space cannon offense -----------------------------------------------------------------------

fn dest() -> System {
    plain_systems().into_iter().find(|s| *s != sys()).expect("another tile")
}

fn arena() -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec, Faction::TheVuilraithCabal]).map_from(&[sys(), dest()])
}

fn cannon_facts(jolnar: Vec<Seat>) -> CannonFacts {
    CannonFacts {
        active: Seat(0),
        system: sys(),
        announce: true,
        staging: false,
        reaching: vec![],
        mapped: false,
        kind: Code(1),
        cause: Code(2),
        casualty: Code(3),
        commander: Code(4),
        jolnar,
        unit_texts: vec![(Unit::PDSI, Code(31), Code(41))],
        planet_texts: vec![],
    }
}

fn guns(owners: &[(u8, usize)]) -> Scenario {
    let planets = sys().data().planets;
    let mut sc = arena().unit(sys(), None, Unit::CruiserI, 0, 12);
    for (i, (seat, count)) in owners.iter().enumerate() {
        sc = sc.unit(sys(), Some(planets[i]), Unit::PDSI, *seat, *count).control(planets[i], *seat);
    }
    sc
}

#[test]
fn the_gunner_with_the_commander_is_asked_about_each_die_before_the_event_and_the_hits_follow_the_faces_that_remain() {
    covers!("rules::combat::space_cannon_offense::SpaceCannonOffense");
    covers!("kernel::commander_walk::Walk");
    let (mut asked_seeds, mut changed) = (0, 0);
    for seed in 0..60u64 {
        let mut plain = Rig::new(guns(&[(1, 3)]), SpaceCannonOffense::new(cannon_facts(vec![]))).seeded(seed);
        plain.run();
        plain.auto(Policy::First);
        let first_hits = plain.driver.window.drawn.iter().map(|(on, f)| f.iter().filter(|x| **x >= *on).count()).sum::<usize>();
        let mut rig = Rig::new(guns(&[(1, 3)]), SpaceCannonOffense::new(cannon_facts(vec![Seat(1)]))).seeded(seed);
        rig.run();
        if first_hits == 0 {
            assert!(rig.is_done(), "seed {seed}: a gunner with no hit gets no reroll window, so no question");
            continue;
        }
        asked_seeds += 1;
        // Three PDS, one entry of one die each: the questions go entry by entry; reroll the misses.
        let mut picked: Vec<usize> = Vec::new();
        for e in 0..3u8 {
            assert!(!rig.is_done());
            let asked = rig.asked();
            assert_eq!(asked.actor, Seat(1), "the gunner is asked");
            assert_eq!(asked.ask, Ask::Combat(CAsk::RerollDie { source: RerollSource::Commander, unit: Code(41), die: 0 }), "the label is the unit's shown text");
            assert_eq!(asked.ctx.subtype, Subtype::Combat(CSub::Reroll(RerollSource::Commander)));
            assert!(!asked.ctx.optional && !asked.ctx.space_battle && asked.ctx.invasion_seq.is_none());
            let staged = rig.state.transient().combat.reroll_staging.iter().find(|(s, _)| *s == Seat(1)).map(|(_, set)| set.clone()).expect("the dice are staged while the question is open");
            assert_eq!(staged.rolls.len(), 3);
            assert!(staged.rolls.iter().all(|r| r.unit == Code(31)), "the staged entries carry the unit's text");
            let Opt::Combat(COpt::RerollDie { entry, face, hits_on, .. }) = rig.opts()[0] else { panic!() };
            assert_eq!((entry, hits_on), (e, Some(6)));
            if face < 6 {
                picked.push(usize::from(e));
                rig.pick(0);
            } else {
                rig.decline();
            }
        }
        rig.auto(Policy::First);
        assert!(rig.is_done());
        let w = &rig.driver.window;
        assert_eq!(w.first_draws(), plain.driver.window.drawn, "seed {seed}: the guns' own rolls are the same with or without the commander");
        let redraws = picked.len();
        assert_eq!(w.drawn.len(), 3 + redraws, "one record per die drawn again");
        let final_hits = rig.state.ledger().dice.iter().filter(|r| r.purpose == RollPurpose::SpaceCannon).count();
        assert_eq!(final_hits, 3);
        let reroll_rows = rig.state.ledger().dice.iter().filter(|r| r.purpose == RollPurpose::Reroll).count();
        assert_eq!(reroll_rows, redraws);
        // The event and the hits of the gunner read the faces that remain: each entry's last record is its final roll.
        // (Each redraw is recorded with its entry's faces after the draw, in the order the dice were picked.)
        let mut last: Vec<Option<(u32, Vec<u32>)>> = w.drawn[..3].iter().cloned().map(Some).collect();
        for (n, e) in picked.iter().enumerate() {
            last[*e] = Some(w.drawn[3 + n].clone());
        }
        let final_total: usize = last.iter().flatten().map(|(on, f)| f.iter().filter(|x| **x >= *on).count()).sum();
        let event = rig.driver.emitted.iter().find(|e| e.kind == EventKind::UnitAbilityRolled).expect("the gunner reports its roll");
        let shown = event.payload.int(Key::Hits).unwrap();
        assert_eq!(shown, i64::try_from(final_total).unwrap(), "seed {seed}: the event shows the hits of the faces that remain");
        if let Some(hit_ev) = rig.driver.emitted.iter().find(|e| e.kind == EventKind::SpaceCannonHits) {
            assert_eq!(hit_ev.payload.int(Key::Hits), Some(shown), "the hits that are absorbed are the final ones");
        } else {
            assert_eq!(shown, 0);
        }
        let cruisers = rig.state.board().sys(sys()).units.iter().filter(|p| p.owner() == Seat(0)).count();
        assert_eq!(cruisers, 12 - usize::try_from(shown).unwrap(), "seed {seed}: one ship per hit that remains");
        changed += usize::from(redraws > 0);
        assert!(rig.state.transient().combat.reroll_staging.is_empty() && rig.state.transient().combat.last_reroll_player.is_none());
    }
    assert!(asked_seeds > 10 && changed > 5, "the seeds cover the walk: {asked_seeds} asked, {changed} with a redraw");
}

#[test]
fn only_the_gunners_that_hold_the_commander_are_asked_and_each_walks_its_own_dice() {
    covers!("rules::combat::space_cannon_offense::SpaceCannonOffense");
    for jolnar in [vec![Seat(1)], vec![Seat(2)], vec![Seat(1), Seat(2)], vec![]] {
        let mut gunners_seen = 0;
        for seed in 0..30u64 {
            let mut rig = Rig::new(guns(&[(1, 2), (2, 2)]), SpaceCannonOffense::new(cannon_facts(jolnar.clone()))).seeded(seed);
            rig.run();
            let mut askers = Vec::new();
            while !rig.is_done() {
                let asked = rig.asked();
                if !matches!(asked.ask, Ask::Combat(CAsk::RerollDie { .. })) {
                    // The absorption of the hits: take whatever is first.
                    rig.pick(0);
                    continue;
                }
                askers.push(asked.actor);
                rig.decline();
            }
            assert!(askers.iter().all(|s| jolnar.contains(s)), "{jolnar:?} seed {seed}: {askers:?}");
            // Every holder whose roll scored a hit walks all its dice (two PDS, one die each): none is skipped.
            let rows: Vec<_> = rig.state.ledger().dice.iter().filter(|r| r.purpose == RollPurpose::SpaceCannon).cloned().collect();
            for holder in &jolnar {
                let scored = rows.iter().filter(|r| r.owner == Some(*holder)).any(|r| r.faces.iter().any(|f| Some(*f) >= r.hits_on));
                assert_eq!(askers.iter().filter(|s| *s == holder).count(), if scored { 2 } else { 0 }, "{jolnar:?} seed {seed}: {holder:?}");
            }
            // The questions of one gunner come together, the lower seat first (the window of each gunner opens in seat order).
            let mut sorted = askers.clone();
            sorted.sort();
            assert_eq!(askers, sorted, "seed {seed}");
            gunners_seen += askers.len();
        }
        assert_eq!(gunners_seen > 0, !jolnar.is_empty(), "{jolnar:?}: asked at all only with a holder");
    }
}

#[test]
fn a_walk_without_the_resolvers_announcements_leaves_no_staged_dice_behind() {
    covers!("rules::combat::space_cannon_offense::SpaceCannonOffense");
    let mut walked = 0;
    for seed in 0..40u64 {
        let facts = CannonFacts { announce: false, ..cannon_facts(vec![Seat(1)]) };
        let mut rig = Rig::new(guns(&[(1, 3)]), SpaceCannonOffense::new(facts)).seeded(seed);
        rig.run();
        while !rig.is_done() {
            if matches!(rig.asked().ask, Ask::Combat(CAsk::RerollDie { .. })) {
                walked += 1;
                rig.pick(0);
            } else {
                rig.pick(0);
            }
        }
        assert!(rig.state.transient().combat.reroll_staging.is_empty() && rig.state.transient().combat.last_reroll_player.is_none(), "seed {seed}");
        assert!(rig.driver.emitted.iter().all(|e| e.kind != EventKind::UnitAbilityRolled), "nothing is announced");
    }
    assert!(walked > 10);
}

#[test]
fn an_attachments_cannon_is_labelled_by_its_planet_in_the_commanders_question() {
    covers!("rules::combat::space_cannon_offense::SpaceCannonOffense");
    use ti4_engine_core::generated::Attachment;
    let planets = sys().data().planets;
    let mut asked_labels = 0;
    for seed in 0..40u64 {
        let sc = arena().unit(sys(), None, Unit::CruiserI, 0, 12).control(planets[0], 1);
        let (mut state, kernel) = sc.build();
        state.board_mut().planet_mut(planets[0]).attach.push(Attachment::Geoform);
        let mut facts = cannon_facts(vec![Seat(1)]);
        facts.planet_texts = vec![(planets[0], Code(51), Code(61))];
        let mut rig = Rig::from_parts(state, kernel, SpaceCannonOffense::new(facts)).seeded(seed);
        rig.run();
        while !rig.is_done() {
            if let Ask::Combat(CAsk::RerollDie { unit, .. }) = rig.asked().ask.clone() {
                assert_eq!(unit, Code(61), "the shown label of the attachment's cannon");
                let staged = rig.state.transient().combat.reroll_staging.iter().find(|(s, _)| *s == Seat(1)).map(|(_, set)| set.clone()).unwrap();
                assert!(staged.rolls.iter().all(|r| r.unit == Code(51)), "the staged entry carries the attachment's text");
                asked_labels += 1;
                rig.decline();
            } else {
                rig.pick(0);
            }
        }
    }
    assert!(asked_labels > 10);
}
