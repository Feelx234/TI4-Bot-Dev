//! D4 lane A3: Fire Team and Scramble Frequency against their printed text: "Fire Team: after your ground forces roll, reroll any number of those
//! dice" and "Scramble Frequency: after a player rolls dice for BOMBARDMENT, SPACE CANNON or ANTI-FIGHTER BARRAGE, that player rerolls all of them".

use ti4_engine_core::choice::combat::{Opt as COpt, RerollSource};
use ti4_engine_core::generated::ActionCard;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::rules::action_cards::effects::Play;
use ti4_engine_core::rules::action_cards::{Env, Facts, Placed, RerollFacts};
use ti4_engine_core::state::{Code, RerollEntry, RerollSet, RngDomain};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::rig::KitSeeds;

const GROUND: Code = Code(1);
const BOMBARD: Code = Code(2);
const CANNON: Code = Code(3);
const BARRAGE: Code = Code(4);
const FLEET: Code = Code(5);

fn env() -> Env {
    let mut shown = [Code(0); RerollFacts::GROUPS];
    shown[0] = Code(40);
    shown[1] = Code(41);
    Env {
        mahact_held: 0,
        scope: ALL_SOURCES,
        blanked: false,
        action_card: Code(0),
        supply_base: Code(0),
        note_order: |a, b| a.cmp(&b),
        infantry: Placed::NONE,
        cruiser: Placed::NONE,
        fighter: Placed::NONE,
        facts: Facts { reroll: Some(RerollFacts { ground: GROUND, forced: [BOMBARD, CANNON, BARRAGE], shown, entries: 2 }), rules_version: 35, ..Facts::default() },
    }
}

fn entry(unit: u16, hits_on: u8, faces: &[u8]) -> RerollEntry {
    RerollEntry { unit: Code(unit), planet: None, hits_on: Some(hits_on), faces: faces.to_vec(), rerolled: 0, deltas: Default::default(), unit_types: Default::default() }
}

fn base() -> CoreState {
    Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]).state()
}

fn stage(s: &mut CoreState, seat: u8, kind: Code, rolls: Vec<RerollEntry>) {
    let list = s.transient_mut().combat.reroll_staging.edit();
    list.push((Seat(seat), RerollSet { kind, system: System::ALL[0], rolls }));
    list.sort_by_key(|(s, _)| *s);
}

fn set_of(s: &CoreState, seat: u8) -> RerollSet {
    s.transient().combat.reroll_staging.iter().find(|(x, _)| *x == Seat(seat)).map(|(_, set)| set.clone()).expect("a staged set")
}

fn draws(s: &CoreState, seed: u64, n: usize) -> Vec<u8> {
    let mut probe = s.clone();
    (0..n).map(|_| u8::try_from(probe.rng_mut().die(RngDomain::Dice, &KitSeeds(seed), 10)).unwrap()).collect()
}

fn linked() -> AbilityTable {
    let mut t = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut t);
    t
}

fn play(card: ActionCard, s: CoreState, seed: u64) -> Rig<Play> {
    let p = Play::of(&s, card, Seat(0), env()).expect("a play");
    Rig::from_parts(s, ti4_engine_core_tests::kernel_all(), p).with_table(linked()).seeded(seed)
}

#[test]
fn fire_team_asks_about_each_die_of_the_holders_ground_roll_and_rolls_the_chosen_ones_again() {
    covers_card!(ActionCard::FireTeam);
    let mut s = base();
    let mut first = entry(7, 8, &[2, 9, 5]);
    first.deltas.insert(2, 1);
    first.rerolled = 0b010;
    stage(&mut s, 0, GROUND, vec![first, entry(9, 7, &[4])]);
    stage(&mut s, 1, GROUND, vec![entry(7, 8, &[3, 3])]);
    let want = draws(&s, 5, 3);
    let mut r = play(ActionCard::FireTeam, s, 5);
    r.run();
    // One optional question per die: dice 0 and 2 of the first group and the die of the second are rolled again, die 1 is declined.
    for (i, (e, d)) in [(0u8, 0u8), (0, 1), (0, 2), (1, 0)].into_iter().enumerate() {
        let shown = Code(40 + u16::from(e));
        assert_eq!(r.asked().actor, Seat(0), "{i}");
        assert_eq!(r.asked().ask, Ask::Combat(ti4_engine_core::choice::combat::Ask::RerollDie { source: RerollSource::FireTeam, unit: shown, die: d }), "{i}");
        assert_eq!(r.opts().len(), 2, "{i}: roll it again, or not");
        assert!(matches!(r.opts()[0], Opt::Combat(COpt::RerollDie { entry, die, .. }) if entry == e && die == d));
        assert_eq!(r.opts()[1], Opt::Decline);
        if i == 0 {
            r.assert_options_sound();
        }
        if i == 1 {
            r.decline();
        } else {
            r.pick(0);
        }
    }
    assert!(r.is_done());
    let mine = set_of(&r.state, 0);
    assert_eq!(mine.rolls[0].faces, [want[0], 9, want[1]], "dice 0 and 2 are the first two draws, die 1 stays");
    assert_eq!(mine.rolls[1].faces, [want[2]]);
    assert_eq!(mine.rolls[0].rerolled, 0b111);
    assert_eq!(mine.rolls[1].rerolled, 0b1);
    assert!(mine.rolls[0].deltas.is_empty(), "the +1 on a die dies with the die");
    assert_eq!(set_of(&r.state, 1).rolls[0].faces, [3, 3], "the other player's roll is not touched");
    // Only the dice stream moved, by three draws.
    let mut moved = r.start_state().clone();
    for _ in 0..3 {
        moved.rng_mut().die(RngDomain::Dice, &KitSeeds(5), 10);
    }
    assert_eq!(r.state.rng().positions().collect::<Vec<_>>(), moved.rng().positions().collect::<Vec<_>>());
    assert_eq!(r.driver.window.faces().len(), 7, "the whole group after each draw: 3 after the first, 3 after the second, 1 for the other group");
}

#[test]
fn fire_team_declined_everywhere_leaves_the_roll_and_the_dice_alone() {
    covers_card!(ActionCard::FireTeam);
    let mut s = base();
    stage(&mut s, 0, GROUND, vec![entry(7, 8, &[2, 9])]);
    let mut r = play(ActionCard::FireTeam, s, 3);
    r.run();
    r.decline();
    r.decline();
    assert!(r.is_done());
    assert_eq!(set_of(&r.state, 0).rolls[0].faces, [2, 9]);
    assert_eq!(set_of(&r.state, 0).rolls[0].rerolled, 0);
    assert!(r.state.rng().positions().collect::<Vec<_>>() == r.start_state().rng().positions().collect::<Vec<_>>());
    // A roll that is not a ground roll, or none at all: nothing to ask.
    let mut fleet = base();
    stage(&mut fleet, 0, FLEET, vec![entry(7, 8, &[2, 9])]);
    let mut r2 = play(ActionCard::FireTeam, fleet.clone(), 3);
    r2.run();
    assert!(r2.is_done() && r2.state == fleet);
    let none = base();
    let mut r3 = play(ActionCard::FireTeam, none.clone(), 3);
    r3.run();
    assert!(r3.is_done() && r3.state == none);
    // The other player's roll is not the holder's.
    let mut theirs = base();
    stage(&mut theirs, 1, GROUND, vec![entry(7, 8, &[2, 9])]);
    let mut r4 = play(ActionCard::FireTeam, theirs.clone(), 3);
    r4.run();
    assert!(r4.is_done() && r4.state == theirs);
}

#[test]
fn scramble_frequency_rolls_every_die_of_the_roller_again_without_asking() {
    covers_card!(ActionCard::ScrambleFrequency);
    for kind in [BOMBARD, CANNON, BARRAGE] {
        let mut s = base();
        stage(&mut s, 0, kind, vec![entry(7, 8, &[1, 1])]);
        let mut first = entry(7, 5, &[2, 9]);
        first.deltas.insert(0, 1);
        stage(&mut s, 1, kind, vec![first, entry(9, 7, &[4])]);
        s.transient_mut().combat.last_reroll_player = Some(Seat(1));
        let want = draws(&s, 9, 3);
        let mut r = play(ActionCard::ScrambleFrequency, s, 9);
        r.run();
        assert!(r.is_done(), "forced: no question");
        let theirs = set_of(&r.state, 1);
        assert_eq!(theirs.rolls[0].faces, [want[0], want[1]]);
        assert_eq!(theirs.rolls[1].faces, [want[2]]);
        assert_eq!((theirs.rolls[0].rerolled, theirs.rolls[1].rerolled), (0b11, 0b1));
        assert!(theirs.rolls[0].deltas.is_empty());
        assert_eq!(set_of(&r.state, 0).rolls[0].faces, [1, 1], "the holder's own roll is not the roller's");
    }
}

#[test]
fn scramble_frequency_leaves_other_rolls_alone() {
    covers_card!(ActionCard::ScrambleFrequency);
    let mut ground = base();
    stage(&mut ground, 1, GROUND, vec![entry(7, 8, &[2, 9])]);
    ground.transient_mut().combat.last_reroll_player = Some(Seat(1));
    let mut fleet = base();
    stage(&mut fleet, 1, FLEET, vec![entry(7, 8, &[2, 9])]);
    fleet.transient_mut().combat.last_reroll_player = Some(Seat(1));
    let mut nobody = base();
    stage(&mut nobody, 1, CANNON, vec![entry(7, 8, &[2, 9])]);
    let mut nothing = base();
    nothing.transient_mut().combat.last_reroll_player = Some(Seat(1));
    let mut hollow = base();
    stage(&mut hollow, 1, CANNON, vec![entry(7, 8, &[])]);
    hollow.transient_mut().combat.last_reroll_player = Some(Seat(1));
    for (name, s) in [("a ground roll", ground), ("a fleet roll", fleet), ("no roller named", nobody), ("nothing staged", nothing), ("a roll of no dice", hollow)] {
        let mut r = play(ActionCard::ScrambleFrequency, s.clone(), 2);
        r.run();
        assert!(r.is_done() && r.state == s, "{name}");
    }
    // Without the adapter's texts of the staged dice there is no play.
    let mut e = env();
    e.facts.reroll = None;
    assert!(Play::of(&base(), ActionCard::ScrambleFrequency, Seat(0), e).is_none());
    assert!(Play::of(&base(), ActionCard::FireTeam, Seat(0), e).is_none());
}
