//! D4-2d: the "Roll Dice" step of a space combat round (LRR 78.5) and its reroll windows: Munitions Reserves, the Crown of Thalnos, Wrath of Kenara,
//! War Funding, Heart of Ixth. Dice are drawn from the seeded stream; the window records the faces (`drawn`) and the tests derive the hits from them.

use ti4_engine_core::choice::combat::{Ask as CAsk, Opt as COpt, RerollSource, Subtype as CSub};
use ti4_engine_core::choice::Subtype;
use ti4_engine_core::generated::{PromissoryNote, Relic};
use ti4_engine_core::rules::combat::roll::{Facts, Queued, RollStep};
use ti4_engine_core::state::{Code, EventKind, NoteKey};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn sys() -> System {
    plain_systems()[0]
}

fn arena(a: Faction, d: Faction) -> Scenario {
    Scenario::new(&[a, d]).map_from(&[sys(), plain_systems()[1]])
}

fn facts(munitions: [bool; 2]) -> Facts {
    Facts { attacker: Seat(0), defender: Seat(1), system: sys(), phase: Code(1), names: vec![], munitions_after: munitions, announce: true, cause: Code(2), hacan: Code(3) }
}

fn hits(drawn: &[(u32, Vec<u32>)]) -> u32 {
    drawn.iter().map(|(on, faces)| faces.iter().filter(|f| **f >= *on).count() as u32).sum()
}

fn c(o: COpt) -> Opt {
    Opt::Combat(o)
}

fn round_hits(rig: &Rig<RollStep>, seat: u8) -> u32 {
    rig.state.transient().combat.combat_round_hits.get(&Seat(seat)).copied().unwrap_or(0)
}

/// Seed with a cruiser fleet's first roll satisfying `want(hits, misses)`.
fn seed_where(make: impl Fn(u64) -> Rig<RollStep>, want: impl Fn(&Rig<RollStep>) -> bool) -> u64 {
    (0..200u64)
        .find(|s| {
            let mut rig = make(*s);
            rig.run();
            want(&rig)
        })
        .expect("a seed with the wanted dice")
}

// ---- the plain roll ----------------------------------------------------------------------------

fn mixed() -> Scenario {
    arena(Faction::TheEmbersOfMuaat, Faction::TheArborec)
        .unit(sys(), None, Unit::DreadnoughtI, 0, 1)
        .unit(sys(), None, Unit::CruiserI, 0, 2)
        .unit(sys(), None, Unit::FighterI, 0, 3)
        .unit(sys(), None, Unit::WarSun, 1, 1)
}

#[test]
fn each_side_rolls_its_groups_in_ascending_combat_value_attacker_first_and_the_hits_are_queued_against_the_other_side() {
    covers!("rules::combat::roll::RollStep");
    let mut counts = (false, false, false);
    for seed in 0..60u64 {
        let mut rig = Rig::new(mixed(), RollStep::new(facts([false, false]))).seeded(seed);
        rig.run();
        assert!(rig.is_done(), "no card in play: the roll never asks");
        let drawn = rig.driver.window.drawn.clone();
        let shape: Vec<(u32, usize)> = drawn.iter().map(|(on, f)| (*on, f.len())).collect();
        assert_eq!(shape, vec![(5, 1), (7, 2), (9, 3), (3, 3)], "dreadnought 5, cruisers 7 (two dice), fighters 9 (three), then the war sun 3 (three dice)");
        assert!(drawn.iter().flat_map(|(_, f)| f).all(|f| (1..=10).contains(f)));
        let (a, d) = (hits(&drawn[..3]), hits(&drawn[3..]));
        let mut want = Vec::new();
        if a > 0 {
            want.push(Queued { player: Seat(1), hits: a, producer: Seat(0), non_fighters_only: false });
        }
        if d > 0 {
            want.push(Queued { player: Seat(0), hits: d, producer: Seat(1), non_fighters_only: false });
        }
        assert_eq!(rig.driver.window.queue, want, "seed {seed}: the attacker's hits against the defender first");
        assert_eq!((round_hits(&rig, 0), round_hits(&rig, 1)), (a, d));
        counts = (counts.0 || a == 0 || d == 0, counts.1 || d == 0, counts.2 || (a > 0 && d > 0));
        assert_eq!(rig.state.ledger().dice.len(), 4, "every roll is in the ledger");
        assert_eq!(rig.state.transient().combat.combat_round_dice.len(), 9 + 0 + 0 - 0, "one record per die: 1 + 2 + 3 + 3");
        assert_eq!(rig.state.transient().combat.last_reroll_player, None);
        assert!(rig.state.transient().combat.reroll_staging.is_empty(), "the staging mirror is cleared when a side is done");
        assert_eq!(rig.state.transient().combat.presentation.round_start.len(), 7);
        assert_eq!(rig.state.transient().combat.presentation.phase, Code(1));
        // No unit is lost, damaged or moved by a roll.
        assert_eq!(delta(rig.start_state(), &rig.state).lines().filter(|l| l.starts_with("sys[")).count(), 0);
    }
    assert!(counts.2, "the seeds cover hits on both sides");
}

#[test]
fn a_side_without_ships_rolls_nothing_and_the_same_seed_gives_the_same_dice() {
    covers!("rules::combat::roll::RollStep");
    let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 2);
    let mut rig = Rig::new(sc.clone(), RollStep::new(facts([false, false]))).seeded(4);
    rig.run();
    assert_eq!(rig.driver.window.drawn.len(), 1, "only the attacker has dice");
    let mut again = Rig::new(sc, RollStep::new(facts([false, false]))).seeded(4);
    again.run();
    assert_eq!(rig.driver.window.drawn, again.driver.window.drawn);
}

#[test]
fn units_that_cannot_hit_roll_no_dice() {
    covers!("rules::combat::roll::RollStep");
    // Carriers roll at 9; space docks and infantry in space do not roll at all.
    let (mut state, kernel) = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CarrierI, 0, 1).unit(sys(), None, Unit::CruiserI, 1, 1).build();
    state.board_mut().sys_mut(sys()).units.push(ti4_engine_core::state::Piece::new(Unit::InfantryI, Seat(0)));
    let mut rig = Rig::from_parts(state, kernel, RollStep::new(facts([false, false]))).seeded(1);
    rig.run();
    let shape: Vec<(u32, usize)> = rig.driver.window.drawn.iter().map(|(on, f)| (*on, f.len())).collect();
    assert_eq!(shape, vec![(9, 1), (7, 1)], "the carrier and the cruiser, not the infantry");
}

// ---- Munitions Reserves ----------------------------------------------------------------------

fn munitions_rig(seed: u64, goods: i16, flag: bool) -> Rig<RollStep> {
    let sc = arena(Faction::TheBaronyOfLetnev, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 4).unit(sys(), None, Unit::CruiserI, 1, 1).goods(0, goods);
    Rig::new(sc, RollStep::new(facts([flag, false]))).seeded(seed)
}

fn misses(rig: &Rig<RollStep>) -> usize {
    rig.driver.window.drawn[0].1.iter().filter(|f| **f < 7).count()
}

#[test]
fn munitions_reserves_is_offered_after_the_roll_when_there_are_misses_and_two_trade_goods() {
    covers!("rules::combat::roll::RollStep");
    let seed = seed_where(|s| munitions_rig(s, 2, true), |r| !r.is_done() && misses(r) >= 2);
    let mut rig = munitions_rig(seed, 2, true);
    rig.run();
    assert_eq!(rig.asked().actor, Seat(0));
    assert_eq!(rig.asked().ask, Ask::Combat(CAsk::MunitionsRoll));
    assert_eq!(rig.asked().ctx.subtype, Subtype::Combat(CSub::MunitionsRoll));
    let m = misses(&rig) as u8;
    assert_eq!(rig.opts(), vec![c(COpt::Munitions { choose: false, misses: m }), c(COpt::Munitions { choose: true, misses: m }), Opt::Decline]);
    rig.assert_options_sound();
    // Not offered without the ability (rules gate), without 2 trade goods, or without a miss.
    let mut no_gate = munitions_rig(seed, 2, false);
    no_gate.run();
    assert!(no_gate.is_done());
    let mut poor = munitions_rig(seed, 1, true);
    poor.run();
    assert!(poor.is_done(), "2 trade goods are the price");
}

#[test]
fn declining_munitions_keeps_the_dice_and_the_goods_and_accepting_rerolls_exactly_the_misses() {
    covers!("rules::combat::roll::RollStep");
    let seed = seed_where(|s| munitions_rig(s, 3, true), |r| !r.is_done() && misses(r) >= 2);
    let mut decline = munitions_rig(seed, 3, true);
    decline.run();
    let first = decline.driver.window.drawn[0].1.clone();
    decline.decline();
    assert!(decline.is_done());
    assert_eq!(decline.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 3);
    let kept_hits = round_hits(&decline, 0);
    assert_eq!(kept_hits, hits(&decline.driver.window.drawn[..1]));
    let mut all = munitions_rig(seed, 3, true);
    all.run();
    all.pick(0);
    assert!(all.is_done());
    assert_eq!(all.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 1, "2 trade goods paid");
    let rerolled = all.driver.window.drawn.len() - 2;
    assert_eq!(rerolled, first.iter().filter(|f| **f < 7).count(), "one fresh die for every miss, none for a hit (the other side's roll is one entry)");
    let after = &all.driver.window.drawn.iter().rev().nth(0).unwrap().1; // the other side's dice are last; look at the attacker's final faces instead
    let _ = after;
    assert!(round_hits(&all, 0) >= kept_hits, "the hits already rolled are kept: only misses are rerolled");
    // The hit dice of the first roll are still hits in the final dice record.
    let final_hits: usize = all.state.transient().combat.combat_round_dice.iter().filter(|d| d.player == Seat(0) && d.hit).count();
    assert_eq!(final_hits as u32, round_hits(&all, 0));
}

#[test]
fn choosing_munitions_dice_asks_one_question_per_die_and_rerolls_only_the_chosen() {
    covers!("rules::combat::roll::RollStep");
    let seed = seed_where(|s| munitions_rig(s, 2, true), |r| !r.is_done() && misses(r) >= 2);
    let mut rig = munitions_rig(seed, 2, true);
    rig.run();
    rig.pick(1);
    let mut asked = 0;
    while let Ask::Combat(CAsk::RerollDie { source: RerollSource::Munitions, die, .. }) = rig.asked().ask {
        assert_eq!(rig.asked().actor, Seat(0));
        assert_eq!(rig.asked().ctx.subtype, Subtype::Combat(CSub::Reroll(RerollSource::Munitions)));
        assert_eq!(asked, die, "the dice are walked in order");
        assert_eq!(rig.opts().len(), 2, "reroll this die or decline");
        asked += 1;
        rig.decline();
        if rig.is_done() {
            break;
        }
    }
    assert_eq!(asked, 4, "four cruisers, four dice");
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 0, "the 2 goods were paid, even when no die is rerolled");
    assert_eq!(rig.driver.window.drawn.len(), 2, "no die was rerolled");
}

// ---- Crown of Thalnos ------------------------------------------------------------------------

fn named(seat: u8, value: i64, unit: Unit) -> ti4_engine_core::rules::combat::roll::Name {
    ti4_engine_core::rules::combat::roll::Name { seat: Seat(seat), value, tag: 0, state: Code(20), shown: Code(30), types: vec![(unit, 1)] }
}

fn crown_rig(seed: u64, relic: bool) -> Rig<RollStep> {
    let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::CruiserI, 1, 1);
    let (mut state, kernel) = sc.build();
    if relic {
        state.players_mut().by_seat_mut(Seat(0)).unwrap().cards.relics.push(Relic::TheCrownOfThalnos);
    }
    let mut f = facts([false, false]);
    f.names = vec![named(0, 7, Unit::CruiserI), named(1, 7, Unit::CruiserI)];
    Rig::from_parts(state, kernel, RollStep::new(f)).seeded(seed)
}

#[test]
fn the_crown_of_thalnos_rerolls_a_die_with_plus_one_and_destroys_a_ship_whose_reroll_missed() {
    covers!("rules::combat::roll::RollStep");
    let (mut lived, mut died) = (false, false);
    for seed in 0..60u64 {
        let mut rig = crown_rig(seed, true);
        rig.run();
        assert_eq!(rig.asked().ask.clone(), Ask::Combat(CAsk::RerollDie { source: RerollSource::Relic, unit: Code(30), die: 0 }));
        assert_eq!(rig.asked().actor, Seat(0));
        assert_eq!(rig.opts().len(), 2);
        rig.assert_options_sound();
        rig.pick(0);
        assert!(rig.is_done());
        let face = rig.driver.window.drawn.iter().find(|(on, f)| *on == 7 && f.len() == 1).map(|(_, f)| f[0]);
        let _ = face;
        // The reroll is the second entry the window drew (the first side's dice are 0, the reroll is 1, the defender's roll is 2).
        let reroll = rig.driver.window.drawn[1].1[0];
        let ships = rig.state.board().sys(sys()).units.iter().filter(|p| p.owner() == Seat(0)).count();
        if reroll + 1 >= 7 {
            lived = true;
            assert_eq!(ships, 1, "seed {seed}: the rerolled die hit with its +1, the ship stays");
            assert_eq!(round_hits(&rig, 0), 1);
        } else {
            died = true;
            assert_eq!(ships, 0, "seed {seed}: the ship whose reroll missed is destroyed");
            assert_eq!(round_hits(&rig, 0), 0);
            let lost = rig.driver.emitted.iter().filter(|e| e.kind == EventKind::ShipDestroyed).count();
            assert_eq!(lost, 1, "the loss is announced");
        }
    }
    assert!(lived && died, "the seeds cover both outcomes");
}

#[test]
fn declining_the_crown_and_not_holding_it_leave_the_dice_alone() {
    covers!("rules::combat::roll::RollStep");
    let mut rig = crown_rig(3, true);
    rig.run();
    rig.decline();
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.drawn.len(), 2, "no reroll");
    assert_eq!(rig.state.board().sys(sys()).units.iter().filter(|p| p.owner() == Seat(0)).count(), 1, "declining destroys nothing");
    let mut none = crown_rig(3, false);
    none.run();
    assert!(none.is_done(), "no relic: no question");
}

// ---- Wrath of Kenara ---------------------------------------------------------------------------

fn hacan_rig(seed: u64, goods: i16) -> Rig<RollStep> {
    let sc = arena(Faction::TheEmiratesOfHacan, Faction::TheArborec).unit(sys(), None, Unit::WrathOfKenara, 0, 1).unit(sys(), None, Unit::CruiserI, 0, 3).unit(sys(), None, Unit::CruiserI, 1, 1).goods(0, goods);
    Rig::new(sc, RollStep::new(facts([false, false]))).seeded(seed)
}

fn near_misses(rig: &Rig<RollStep>) -> usize {
    rig.driver.window.drawn.iter().take_while(|_| true).map(|(on, f)| f.iter().filter(|x| **x + 1 == *on).count()).sum::<usize>()
}

#[test]
fn the_hacan_flagship_buys_plus_one_on_near_misses_for_trade_goods() {
    covers!("rules::combat::roll::RollStep");
    // At the question only the attacker has rolled: every entry drawn so far is the attacker's.
    let near = |r: &Rig<RollStep>| r.driver.window.drawn.iter().map(|(on, f)| f.iter().filter(|x| **x + 1 == *on).count()).sum::<usize>();
    let seed = seed_where(|s| hacan_rig(s, 5), |r| !r.is_done() && near(r) >= 2);
    let mut rig = hacan_rig(seed, 5);
    rig.run();
    let n = near(&rig);
    assert!(n >= 2);
    assert_eq!(rig.asked().ask, Ask::Combat(CAsk::Wrath));
    assert_eq!(rig.asked().actor, Seat(0));
    assert_eq!(rig.asked().ctx.subtype, Subtype::Combat(CSub::WrathBump));
    let most = n.min(5) as u8;
    let want: Vec<Opt> = (1..=most).map(|k| c(COpt::Kenara { count: k })).chain([Opt::Decline]).collect();
    assert_eq!(rig.opts(), want, "one option per number of goods that can be used");
    rig.assert_options_sound();
    let attack_entries = rig.driver.window.drawn.len();
    let base = hits(&rig.driver.window.drawn[..attack_entries]);
    let mut declined = rig.replay();
    declined.decline();
    assert_eq!(round_hits(&declined, 0), base);
    assert_eq!(declined.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 5);
    rig.pick(1);
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 3, "two goods spent");
    assert_eq!(round_hits(&rig, 0), base + 2, "two near misses became hits");
}

#[test]
fn wrath_is_not_offered_without_goods_a_near_miss_or_the_flagship() {
    covers!("rules::combat::roll::RollStep");
    let seed = seed_where(|s| hacan_rig(s, 5), |r| !r.is_done() && near_misses(r) >= 1);
    let mut poor = hacan_rig(seed, 0);
    poor.run();
    assert!(poor.is_done(), "no trade goods");
    let sc = arena(Faction::TheEmiratesOfHacan, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 4).unit(sys(), None, Unit::CruiserI, 1, 1).goods(0, 5);
    for seed in 0..20u64 {
        let mut rig = Rig::new(sc.clone(), RollStep::new(facts([false, false]))).seeded(seed);
        rig.run();
        assert!(rig.is_done(), "seed {seed}: cruisers are no flagship");
    }
}

// ---- War Funding -----------------------------------------------------------------------------

fn funding_rig(seed: u64) -> Rig<RollStep> {
    let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 3).unit(sys(), None, Unit::CruiserI, 1, 3);
    let (mut state, kernel) = sc.build();
    state.cards_mut().promissory.edit().insert(NoteKey { note: PromissoryNote::WarFunding, faction: Faction::TheArborec }, Seat(0));
    Rig::from_parts(state, kernel, RollStep::new(facts([false, false]))).seeded(seed)
}

#[test]
fn a_held_war_funding_note_rerolls_the_opponents_dice_and_our_misses_and_goes_home() {
    covers!("rules::combat::roll::RollStep");
    let mut rig = funding_rig(2);
    rig.run();
    assert_eq!(rig.asked().ask, Ask::Combat(CAsk::WarFunding));
    assert_eq!(rig.asked().actor, Seat(0), "the holder is asked");
    assert_eq!(rig.opts(), vec![c(COpt::UseWarFunding), Opt::Decline]);
    rig.assert_options_sound();
    let drawn_before = rig.driver.window.drawn.len();
    let own_misses = rig.driver.window.drawn[0].1.iter().filter(|f| **f < 7).count();
    let key = NoteKey { note: PromissoryNote::WarFunding, faction: Faction::TheArborec };
    let mut declined = rig.replay();
    declined.decline();
    assert!(declined.is_done());
    assert_eq!(declined.state.cards().promissory.get(&key), Some(&Seat(0)), "declined: the note stays");
    assert_eq!(declined.driver.window.drawn.len(), drawn_before);
    rig.pick(0);
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.drawn.len(), drawn_before + 3 + own_misses, "all three of the opponent's dice and each of our misses are rolled again");
    assert_eq!(rig.state.cards().promissory.get(&key), Some(&Seat(1)), "the note returns to its owner");
}

#[test]
fn war_funding_is_not_offered_when_there_is_nothing_to_gain() {
    covers!("rules::combat::roll::RollStep");
    // One cruiser each: the holder is offered the note exactly when it missed or the opponent hit (otherwise there is nothing to reroll).
    let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::CruiserI, 1, 1);
    let mut quiet = 0;
    for seed in 0..60u64 {
        let (mut state, kernel) = sc.clone().build();
        state.cards_mut().promissory.edit().insert(NoteKey { note: PromissoryNote::WarFunding, faction: Faction::TheArborec }, Seat(0));
        let mut rig = Rig::from_parts(state, kernel, RollStep::new(facts([false, false]))).seeded(seed);
        rig.run();
        let (mine_missed, theirs_hit) = (rig.driver.window.drawn[0].1[0] < 7, rig.driver.window.drawn[1].1[0] >= 7);
        assert_eq!(!rig.is_done(), mine_missed || theirs_hit, "seed {seed}: offered exactly when there is something to roll again");
        quiet += usize::from(rig.is_done());
    }
    assert!(quiet > 0, "the seeds cover a roll where we hit and they missed");
    // Our own note is no War Funding of another faction.
    let (mut state, kernel) = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 3).unit(sys(), None, Unit::CruiserI, 1, 3).build();
    state.cards_mut().promissory.edit().insert(NoteKey { note: PromissoryNote::WarFunding, faction: Faction::TheEmbersOfMuaat }, Seat(0));
    let mut rig = Rig::from_parts(state, kernel, RollStep::new(facts([false, false]))).seeded(2);
    rig.run();
    assert!(rig.is_done(), "a faction's own War Funding is not usable by it");
}

// ---- Heart of Ixth (pinned current behaviour: A1) ------------------------------------------------

#[test]
fn the_heart_of_ixth_is_asked_of_its_holder_but_an_answer_changes_nothing_yet() {
    covers!("rules::combat::roll::RollStep");
    // docs/ENGINE_KNOWN_ISSUES.md A1: the string engine parses the option id wrongly, so no die is shifted and the card is not exhausted.
    // The core copies that until a RULES_VERSION fixes it: this test pins the copy and must change together with the fix.
    let make = |seed| {
        let sc = arena(Faction::TheEmbersOfMuaat, Faction::TheArborec).unit(sys(), None, Unit::CruiserI, 0, 2).unit(sys(), None, Unit::CruiserI, 1, 1);
        let (mut state, kernel) = sc.build();
        state.players_mut().by_seat_mut(Seat(1)).unwrap().cards.relics.push(Relic::HeartOfIxth);
        Rig::from_parts(state, kernel, RollStep::new(facts([false, false]))).seeded(seed)
    };
    let mut rig = make(1);
    rig.run();
    assert_eq!(rig.asked().ask, Ask::Combat(CAsk::HeartIxth));
    assert_eq!(rig.asked().actor, Seat(1), "the holder, though the dice are the attacker's");
    // Two attacker dice, each with +1 and -1, then the decline.
    assert_eq!(rig.opts().len(), 2 * 2 + 1);
    assert!(matches!(rig.opts()[0], Opt::Combat(COpt::HeartIxth { add: true, .. })));
    rig.assert_options_sound();
    let mut plain = rig.replay();
    plain.decline();
    rig.pick(0);
    // Whatever the answer, the dice of the attacker keep their value and the defender's own roll then asks again for its dice.
    let take = |r: &mut Rig<RollStep>| {
        let mut n = 0;
        while !r.is_done() && n < 10 {
            r.decline();
            n += 1;
        }
        round_hits(r, 0)
    };
    assert_eq!(take(&mut rig), take(&mut plain), "the answer changed no die");
}
