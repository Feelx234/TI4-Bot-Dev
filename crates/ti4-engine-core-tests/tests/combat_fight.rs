//! Lent-bodies lane 9: `rules::combat::fight::FightToEnd` (a whole space combat as one window) and `rules::combat::mahact_hero::MahactHero`
//! (Airo Shir Aur: the pair, the move, the enforcement and the fight as children).

use ti4_engine_core::kernel::children::SeeingLimits;
use ti4_engine_core::keys::MarkKey;
use ti4_engine_core::rules::combat::end_announce::Codes as EndCodes;
use ti4_engine_core::rules::combat::fight::FightToEnd;
use ti4_engine_core::rules::combat::flow::{Phase, Step};
use ti4_engine_core::rules::combat::machine::{Bail, Env, Fields};
use ti4_engine_core::rules::combat::mahact_hero::{MahactHero, Prepared};
use ti4_engine_core::state::{Before, Code, EventKind};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn sys() -> System {
    plain_systems()[0]
}

fn there() -> System {
    plain_systems()[1]
}

const KINDS: [Unit; 7] = [Unit::CarrierI, Unit::CruiserI, Unit::DestroyerI, Unit::DestroyerII, Unit::DreadnoughtI, Unit::FighterI, Unit::WarSun];

/// The text tables the string side would hand over: every non-empty set of the unit types used, in id order.
fn groups() -> Vec<(Vec<Unit>, Code, Code)> {
    let mut out = Vec::new();
    for mask in 1u16..(1 << KINDS.len()) {
        let mut kinds: Vec<Unit> = KINDS.iter().enumerate().filter(|(i, _)| mask & (1 << i) != 0).map(|(_, u)| *u).collect();
        kinds.sort_unstable();
        out.push((kinds, Code(1000 + mask), Code(2000 + mask)));
    }
    out
}

fn env() -> Env {
    let mut alpha = KINDS.to_vec();
    alpha.sort_unstable();
    Env {
        attacker: Seat(0),
        defender: Seat(1),
        system: sys(),
        announce: true,
        staging: false,
        occurrence: 0,
        begin_occurrence: true,
        phases: [Code(1), Code(2), Code(3), Code(4), Code(5)],
        opening_unported: false,
        round_seq_listens: false,
        barrage_unported: false,
        jolnar: [false, false],
        shown_codes: KINDS.iter().enumerate().map(|(i, u)| (*u, Code(100 + i as u16))).collect(),
        barrage_extra_shown: Code(99),
        barrage_cause: Code(6),
        barrage_kind: Code(7),
        barrage_extra: Code(8),
        unit_codes: KINDS.iter().enumerate().map(|(i, u)| (*u, Code(100 + i as u16))).collect(),
        roll_unported: false,
        munitions_after: [false, false],
        munitions_start: [false, false],
        roll_cause: Code(9),
        hacan: Code(10),
        groups: groups(),
        alpha,
        retreat_cause: Code(11),
        retreat_event: Code(11),
        retreat_pool: Code(12),
        retreat_unported: false,
        neighbors: [vec![], vec![]],
        may_retreat: [false, false],
        duranium: [false, false],
        devotion: false,
        end_codes: EndCodes { losers: Code(40), retreated: Code(41), reasons: [Code(42), Code(43), Code(44), Code(45)] },
        casualty: Code(13),
        commander: Code(14),
        show_neutral: false,
        det: [false, false],
        wormhole: false,
        ambush: Code(15),
        cause_core: Code(16),
        cause_devotion: Code(17),
        raid: Code(18),
        ac: false,
        ac_subtype: Code(19),
        ac_cause: Code(20),
    }
}

fn fields(s: &CoreState) -> Fields {
    let fleet = |seat: Seat| s.board().sys(sys()).units.iter().filter(|p| p.owner() == seat).map(|p| p.kind()).collect::<Vec<_>>();
    Fields {
        step: Step::Opening { round: 1 },
        phase: Phase::Round,
        resuming: false,
        pending_retreats: vec![],
        offered: vec![],
        damaged: vec![],
        fled: vec![],
        at_start: vec![(Seat(0), fleet(Seat(0))), (Seat(1), fleet(Seat(1)))],
        scoring: false,
        outcome: None,
        end_announced: false,
    }
}


fn arena() -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys(), there()]).tokens(0, 3, 3, 2).tokens(1, 3, 3, 2)
}

fn two_fleets(attackers: usize, defenders: usize) -> Scenario {
    arena().unit(sys(), None, Unit::CruiserI, 0, attackers).unit(sys(), None, Unit::CruiserI, 1, defenders)
}

fn fight_rig(sc: Scenario, env: Env, seed: u64) -> Rig<FightToEnd> {
    let (state, kernel) = sc.build();
    let f = fields(&state);
    Rig::from_parts(state, kernel, FightToEnd::new(env, f)).seeded(seed)
}

fn answer_first(rig: &mut Rig<impl Window>) {
    let mut guard = 0;
    while !rig.is_done() && guard < 300 {
        guard += 1;
        rig.pick(0);
    }
}

#[test]
fn a_whole_combat_is_fought_by_one_window_to_the_announcement_of_its_end() {
    covers!("rules::combat::fight::FightToEnd");
    for seed in 0..6 {
        let mut rig = fight_rig(two_fleets(3, 2), env(), seed);
        rig.run();
        answer_first(&mut rig);
        assert!(rig.is_done(), "seed {seed}: {}", rig.transcript());
        let w = &rig.driver.window;
        assert!(!w.unsupported, "seed {seed}");
        let (winner, rounds) = w.outcome().expect("the fight is over");
        assert!(rounds >= 1, "seed {seed}");
        let left = |seat: u8| rig.state.board().sys(sys()).units.as_slice().iter().filter(|p| p.owner() == Seat(seat)).count();
        assert!(left(0) == 0 || left(1) == 0, "seed {seed}: one side is gone ({}, {})", left(0), left(1));
        assert_eq!(winner.is_some(), left(0) != left(1), "seed {seed}");
        assert!(w.occurrence() > 0, "the first round begins the occurrence");
        assert!(rig.driver.emitted.iter().any(|e| e.kind == EventKind::SpaceCombatEnded), "the end is announced");
    }
}

#[test]
fn a_position_the_machine_does_not_carry_stops_the_run_with_its_reason() {
    covers!("rules::combat::fight::FightToEnd");
    let mut ungated = env();
    ungated.opening_unported = true;
    let mut rig = fight_rig(two_fleets(2, 2), ungated, 1);
    rig.run();
    assert!(rig.is_done());
    assert!(rig.driver.window.unsupported && rig.driver.window.outcome().is_none());
    assert_eq!(rig.driver.window.bailed, Some(Bail::Unported));
}

#[test]
fn the_decisions_inside_a_combat_are_asked_by_the_window() {
    covers!("rules::combat::fight::FightToEnd");
    // Different ships to hit and a sustain: the owner is asked, and the window goes on after each answer.
    let sc = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 2).unit(sys(), None, Unit::CruiserI, 0, 2).unit(sys(), None, Unit::DestroyerI, 1, 4).unit(sys(), None, Unit::CarrierI, 1, 2);
    let mut asked = 0;
    for seed in 0..6 {
        let mut rig = fight_rig(sc.clone(), env(), seed);
        rig.run();
        while !rig.is_done() {
            asked += 1;
            rig.assert_options_sound();
            rig.pick(0);
        }
        assert!(!rig.driver.window.unsupported, "{}", rig.transcript());
    }
    assert!(asked > 0, "some seed puts a question");
}

// -- Airo Shir Aur ---------------------------------------------------------------------------------------------------

fn limits() -> SeeingLimits {
    SeeingLimits { enforce: 8, shown: 8, halves: [None, None] }
}

fn prepared(from: System, to: System) -> Prepared {
    let mut e = env();
    e.system = to;
    Prepared { from, to, limits: limits(), env: e, fields: fields(&arena().build().0), before: Before::default() }
}

fn hero_game(extra: impl Fn(Scenario) -> Scenario) -> (Rig<MahactHero>, System, System) {
    let (origin, destination) = (there(), sys());
    let sc = extra(arena().unit(origin, None, Unit::CruiserI, 0, 2).unit(destination, None, Unit::CruiserI, 1, 1).active_system(plain_systems()[2]));
    let (state, kernel) = sc.build();
    let window = MahactHero::new(Seat(0), vec![(origin, destination)], vec![prepared(origin, destination)]);
    (Rig::from_parts(state, kernel, window).seeded(3), origin, destination)
}

#[test]
fn the_hero_moves_the_fleet_fights_to_the_end_and_puts_the_bar_and_the_active_system_back() {
    covers!("rules::combat::mahact_hero::MahactHero");
    let (mut rig, origin, destination) = hero_game(|sc| sc);
    let previous = rig.state.meta().active_system;
    rig.run();
    let mut barred_while_fighting = false;
    while !rig.is_done() {
        barred_while_fighting |= rig.state.marks().tail.iter().any(|(k, _)| matches!(k, MarkKey::MahactBenediction()))
            && rig.state.meta().active_system == Some(destination);
        rig.pick(0);
    }
    assert!(rig.driver.window.used && !rig.driver.window.unsupported, "{}", rig.transcript());
    let ships = |system: System, seat: u8| rig.state.board().sys(system).units.as_slice().iter().filter(|p| p.owner() == Seat(seat)).count();
    assert_eq!(ships(origin, 0), 0, "the fleet left the origin");
    assert!(ships(destination, 0) == 0 || ships(destination, 1) == 0, "one side is gone");
    assert!(!rig.state.marks().tail.iter().any(|(k, _)| matches!(k, MarkKey::MahactBenediction())), "the bar ends with the combat");
    assert_eq!(rig.state.meta().active_system, previous, "the active system is restored");
    assert!(barred_while_fighting || rig.picks().is_empty(), "the bar and the destination stand while the fight is fought");
}

#[test]
fn two_pairs_are_asked_and_only_the_chosen_fleet_moves() {
    covers!("rules::combat::mahact_hero::MahactHero");
    let (a, b, destination) = (there(), plain_systems()[2], sys());
    let sc = arena_with(b).unit(a, None, Unit::CruiserI, 0, 2).unit(b, None, Unit::CruiserI, 0, 1).unit(destination, None, Unit::CruiserI, 1, 1);
    let (state, kernel) = sc.build();
    let window = MahactHero::new(Seat(0), vec![(a, destination), (b, destination)], vec![prepared(b, destination)]);
    let mut rig = Rig::from_parts(state, kernel, window).seeded(2);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Leaders(ti4_engine_core::choice::leaders::Ask::MahactMove)), "{}", rig.transcript());
    assert_eq!(rig.opts().len(), 2);
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(Opt::Decline));
    rig.pick_opt(Opt::Leaders(ti4_engine_core::choice::leaders::Opt::MahactMove(b, destination)));
    answer_first(&mut rig);
    assert!(rig.driver.window.used, "{}", rig.transcript());
    let at = |system: System| rig.state.board().sys(system).units.as_slice().iter().filter(|p| p.owner() == Seat(0)).count();
    assert_eq!(at(b), 0, "the chosen fleet moved");
    assert_eq!(at(a), 2, "the other stayed");
}

fn arena_with(extra: System) -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys(), there(), extra]).tokens(0, 3, 3, 2).tokens(1, 3, 3, 2)
}

#[test]
fn a_pair_the_adapter_did_not_prepare_declines_the_run_before_anything_moves() {
    covers!("rules::combat::mahact_hero::MahactHero");
    let (origin, destination) = (there(), sys());
    let sc = arena().unit(origin, None, Unit::CruiserI, 0, 2).unit(destination, None, Unit::CruiserI, 1, 1);
    let (state, kernel) = sc.build();
    let window = MahactHero::new(Seat(0), vec![(origin, destination)], Vec::new());
    let mut rig = Rig::from_parts(state, kernel, window);
    rig.run();
    assert!(rig.is_done() && rig.driver.window.unsupported && !rig.driver.window.used);
    assert_eq!(delta(rig.start_state(), &rig.state), "");
}

#[test]
fn no_pair_means_the_card_is_not_used() {
    covers!("rules::combat::mahact_hero::MahactHero");
    let (state, kernel) = arena().build();
    let mut rig = Rig::from_parts(state, kernel, MahactHero::new(Seat(0), Vec::new(), Vec::new()));
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.used && !rig.driver.window.unsupported);
}

#[test]
fn a_fleet_that_does_not_fit_its_destination_is_enforced_before_the_fight() {
    covers!("rules::combat::mahact_hero::MahactHero");
    // Cruisers carry nothing: the fighters over the capacity of the destination are removed by their owner, a question each.
    let (mut rig, _, destination) = hero_game(|sc| sc.unit(there(), None, Unit::FighterI, 0, 2));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Tactical(_)), "{}", rig.transcript());
    rig.pick(0);
    answer_first(&mut rig);
    assert!(rig.driver.window.used, "{}", rig.transcript());
    let fighters = rig.state.board().sys(destination).units.as_slice().iter().filter(|p| p.kind() == Unit::FighterI).count();
    assert!(fighters < 2, "some fighter was removed ({fighters} left)");
}

#[test]
fn a_fleet_that_stands_after_the_fight_earns_the_feat_against_the_occurrence_of_the_combat() {
    covers!("rules::combat::mahact_hero::MahactHero");
    use ti4_engine_core::state::Feat;
    let mut earned = 0;
    for seed in 0..8 {
        let (origin, destination) = (there(), sys());
        let sc = arena().unit(origin, None, Unit::DreadnoughtI, 0, 3).unit(destination, None, Unit::FighterI, 1, 1);
        let (state, kernel) = sc.build();
        let window = MahactHero::new(Seat(0), vec![(origin, destination)], vec![prepared(origin, destination)]);
        let mut rig = Rig::from_parts(state, kernel, window).seeded(seed);
        rig.run();
        answer_first(&mut rig);
        assert!(rig.driver.window.used);
        let feats = rig.state.players().by_seat(Seat(0)).unwrap().misc.event_feats.as_slice().to_vec();
        if feats.iter().any(|(f, o)| *f == Feat::HeldThreeShipsAfterASpaceCombat && *o == rig.state.meta().feat_occurrence_seq) {
            earned += 1;
        }
    }
    assert!(earned > 0, "three dreadnoughts that stand earn the feat");
}

#[test]
fn only_the_seats_units_move_and_the_preparation_of_the_chosen_pair_is_used() {
    covers!("rules::combat::mahact_hero::MahactHero");
    let (a, b, destination) = (there(), plain_systems()[2], sys());
    let sc = arena_with(b).unit(a, None, Unit::CruiserI, 0, 2).unit(a, None, Unit::CruiserI, 1, 1).unit(destination, None, Unit::CruiserI, 1, 1);
    let (state, kernel) = sc.build();
    let window = MahactHero::new(Seat(0), vec![(a, destination)], vec![prepared(b, destination), prepared(a, destination)]);
    let mut rig = Rig::from_parts(state, kernel, window).seeded(2);
    rig.run();
    answer_first(&mut rig);
    assert!(rig.driver.window.used && !rig.driver.window.unsupported);
    let other = rig.state.board().sys(a).units.as_slice().iter().filter(|p| p.owner() == Seat(1)).count();
    assert_eq!(other, 1, "another player's ship in the origin stays");
}
