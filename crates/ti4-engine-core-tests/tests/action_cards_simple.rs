//! D4 lane A3: `rules/action_cards/mod.rs`: the cards that only set a stamp or a flag (`Simple`, `play_simple`), the reads that decide when a card can
//! be played or reacted with (`has_legal_target`, `reaction_can_matter`, `cost_is_payable`, `is_playable`, `playable_hand`). Each expectation is the
//! printed text of the card (the cards' windows are `Timing`: the effect runs in its window; this is the effect).

use ti4_engine_core::generated::{ActionCard as C, Agenda, Planet, Technology as T, Unit};
use ti4_engine_core::rules::action_cards::{self as ac, Env, Facts, Gates, Placed, Simple, Window};
use ti4_engine_core::state::{CoreState, Outcome, PlanetFlags, RoundKind, TransientFlags};
use ti4_engine_core::tables::Scope;
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn four() -> Scenario {
    Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol, Faction::TheWinnu]).map_ring(7)
}

fn env() -> Env {
    Env {
        mahact_held: 0,
        scope: ALL_SOURCES,
        blanked: false,
        action_card: ti4_engine_core::state::Code(0),
        supply_base: ti4_engine_core::state::Code(0),
        note_order: |a, b| a.cmp(&b),
        infantry: Placed { unit: Some(Unit::InfantryI), on_card: 0 },
        cruiser: Placed { unit: Some(Unit::CruiserI), on_card: 0 },
        fighter: Placed { unit: Some(Unit::FighterI), on_card: 0 },
        facts: Facts { neutral: Some(Seat(1)), rules_version: 35, ..Facts::default() },
    }
}

fn play(s: &mut CoreState, who: u8, effect: Simple) {
    ac::play_simple(s, Seat(who), effect, env());
}

fn flag(s: &CoreState, f: u8) -> bool {
    s.transient().misc.flags.has(f)
}

fn marks(s: &CoreState, who: u8, kind: RoundKind) -> Vec<u32> {
    s.players().by_seat(Seat(who)).unwrap().round_marks.of(kind).collect()
}

fn timers(s: &CoreState, who: u8) -> ti4_engine_core::state::CardTimers {
    s.players().by_seat(Seat(who)).unwrap().card_timers.clone()
}

// ---- which card plays which effect -------------------------------------------------------------------

#[test]
fn every_copy_of_a_card_plays_the_same_effect_and_cards_with_a_window_of_their_own_play_none_here() {
    for (cards, want) in [
        (&[C::MoraleBoost1, C::MoraleBoost2, C::MoraleBoost3, C::MoraleBoost4][..], Simple::MoraleBoost),
        (&[C::FlankSpeed1, C::FlankSpeed2, C::FlankSpeed3, C::FlankSpeed4][..], Simple::FlankSpeed),
        (&[C::WarMachine1, C::WarMachine2, C::WarMachine3, C::WarMachine4][..], Simple::WarMachine),
        (&[C::ManeuveringJets1, C::ManeuveringJets2, C::ManeuveringJets3, C::ManeuveringJets4][..], Simple::ManeuveringJets),
        (&[C::ShieldsHolding1, C::ShieldsHolding2, C::ShieldsHolding3, C::ShieldsHolding4][..], Simple::ShieldsHolding),
    ] {
        for c in cards {
            assert_eq!(Simple::of(*c), Some(want), "{c:?}");
        }
    }
    let singles = [
        (C::SolarFlare, Simple::SolarFlare),
        (C::CoupDEtat, Simple::Coup),
        (C::Crisis, Simple::Crisis),
        (C::MasterPlan, Simple::MasterPlan),
        (C::LostStarChart, Simple::LostStar),
        (C::HackElection, Simple::HackElection),
        (C::PoliticalStability, Simple::PoliticalStability),
        (C::Rout, Simple::Rout),
        (C::Waylay, Simple::Waylay),
        (C::FighterPrototype, Simple::FighterPrototype),
        (C::Bunker, Simple::Bunker),
        (C::Blitz, Simple::Blitz),
        (C::NavSuite, Simple::NavSuite),
        (C::BlackMarketDealings, Simple::BlackMarket),
        (C::Rally, Simple::Rally),
        (C::ReflectiveShielding, Simple::Reflective),
        (C::ConfoundingLegalText, Simple::Confounding),
        (C::DeadlyPlot, Simple::DeadlyPlot),
        (C::Salvage, Simple::Salvage),
        (C::ReverseEngineer, Simple::ReverseEngineer),
        (C::EconomicInitiative, Simple::EconomicInitiative),
        (C::DistinguishedCouncilor, Simple::Councilor),
        (C::EmergencyRepairs, Simple::EmergencyRepairs),
        (C::Disable, Simple::Disable),
    ];
    for (c, want) in singles {
        assert_eq!(Simple::of(c), Some(want), "{c:?}");
    }
    for c in [C::Spy, C::Plague, C::Sabotage1, C::Veto, C::Upgrade, C::Uprising] {
        assert_eq!(Simple::of(c), None, "{c:?}");
    }
    assert_eq!(ActionCard::ALL.into_iter().filter(|c| Simple::of(*c).is_some()).count(), 4 * 5 + singles.len());
}

// ---- the stamps and flags ----------------------------------------------------------------------------

#[test]
fn the_stamps_hold_the_round_or_the_activation_the_card_was_played_in_for_its_player_only() {
    let mut s = four().state();
    s.meta_mut().combat_round_seq = 7;
    s.meta_mut().activation_seq = 11;
    s.meta_mut().agenda_seq = 5;
    play(&mut s, 0, Simple::MoraleBoost);
    assert_eq!(timers(&s, 0).combat_bonus_round.get(), Some(7), "+1 to combat rolls during this combat round");
    assert_eq!(timers(&s, 1).combat_bonus_round.get(), None);
    play(&mut s, 0, Simple::FlankSpeed);
    assert_eq!(timers(&s, 0).move_bonus_activation.get(), Some(11), "+1 move during this tactical action");
    play(&mut s, 1, Simple::NavSuite);
    assert_eq!(timers(&s, 1).anomalies_ignored_activation.get(), Some(11));
    assert_eq!(timers(&s, 0).anomalies_ignored_activation.get(), None);
    play(&mut s, 2, Simple::HackElection);
    assert_eq!(timers(&s, 2).hack_votes_last_agenda.get(), Some(5), "votes last on this agenda");
    play(&mut s, 3, Simple::Rout);
    play(&mut s, 3, Simple::Waylay);
    assert_eq!((timers(&s, 3).rout_round.get(), timers(&s, 3).waylay_barrage_round.get()), (Some(7), Some(7)));
    assert_eq!((timers(&s, 0).rout_round.get(), timers(&s, 0).waylay_barrage_round.get()), (None, None));
}

#[test]
fn the_marks_of_a_round_or_an_activation_are_kept_per_kind_and_pile_up() {
    let mut s = four().state();
    s.meta_mut().combat_round_seq = 3;
    s.meta_mut().activation_seq = 8;
    for (effect, kind, value) in [
        (Simple::SolarFlare, RoundKind::SolarFlare, 8),
        (Simple::LostStar, RoundKind::LostStar, 8),
        (Simple::FighterPrototype, RoundKind::FighterBonus, 3),
        (Simple::Bunker, RoundKind::BunkerInvasion, 8),
        (Simple::WarMachine, RoundKind::WarMachine, 8),
        (Simple::Blitz, RoundKind::BlitzInvasion, 8),
    ] {
        play(&mut s, 0, effect);
        assert_eq!(marks(&s, 0, kind), vec![value], "{effect:?}");
        assert!(marks(&s, 1, kind).is_empty());
    }
    // A second copy in the same activation adds a second mark (two War Machines are +10).
    play(&mut s, 0, Simple::WarMachine);
    assert_eq!(marks(&s, 0, RoundKind::WarMachine), vec![8, 8]);
}

#[test]
fn the_table_flags_are_raised_by_the_cards_that_name_them() {
    for (effect, f) in [
        (Simple::Coup, TransientFlags::STRATEGIC_CANCELLED),
        (Simple::Crisis, TransientFlags::SKIP_NEXT_TURN),
        (Simple::MasterPlan, TransientFlags::ADDITIONAL_ACTION),
        (Simple::BlackMarket, TransientFlags::BLACK_MARKET),
    ] {
        let mut s = four().state();
        assert!(!flag(&s, f));
        play(&mut s, 0, effect);
        assert!(flag(&s, f), "{effect:?}");
        for other in [TransientFlags::STRATEGIC_CANCELLED, TransientFlags::SKIP_NEXT_TURN, TransientFlags::ADDITIONAL_ACTION, TransientFlags::BLACK_MARKET] {
            assert_eq!(flag(&s, other), other == f, "{effect:?} raises only its own flag");
        }
    }
    let mut s = four().state();
    assert!(!s.players().by_seat(Seat(2)).unwrap().econ.stability);
    play(&mut s, 2, Simple::PoliticalStability);
    assert!(s.players().by_seat(Seat(2)).unwrap().econ.stability, "keep the strategy cards");
    assert!(!s.players().by_seat(Seat(0)).unwrap().econ.stability);
}

#[test]
fn rally_gives_two_fleet_tokens_as_far_as_the_reinforcements_reach() {
    let fleet = |s: &CoreState| s.players().by_seat(Seat(0)).unwrap().econ.fleet_tokens;
    // 16 tokens in all: 3 + 3 + 2 on the sheet leave 8 in the reinforcements.
    let mut s = four().tokens(0, 3, 3, 2).state();
    play(&mut s, 0, Simple::Rally);
    assert_eq!(fleet(&s), 5);
    // One token left: one is given; none left: none.
    let mut one = four().tokens(0, 6, 6, 3).state();
    play(&mut one, 0, Simple::Rally);
    assert_eq!(fleet(&one), 7);
    let mut none = four().tokens(0, 6, 6, 4).state();
    play(&mut none, 0, Simple::Rally);
    assert_eq!(fleet(&none), 6);
    // Tokens on the board and tokens held by Mahact are out of the reinforcements too.
    let t = plain_systems();
    let mut board = four().tokens(0, 5, 5, 4).command_token(t[1], 0).state();
    play(&mut board, 0, Simple::Rally);
    assert_eq!(fleet(&board), 6, "15 out of 16: one is left");
    let mut held = four().tokens(0, 5, 5, 4).state();
    ac::play_simple(&mut held, Seat(0), Simple::Rally, Env { mahact_held: 2, ..env() });
    assert_eq!(fleet(&held), 5, "two held by Mahact: none left");
}

#[test]
fn cancelled_hits_pile_up_within_a_round_and_start_over_in_the_next() {
    let mut s = four().state();
    s.meta_mut().combat_round_seq = 4;
    play(&mut s, 0, Simple::ManeuveringJets);
    assert_eq!(timers(&s, 0).cancel_hits_round, Some((4, 1)));
    play(&mut s, 0, Simple::ShieldsHolding);
    assert_eq!(timers(&s, 0).cancel_hits_round, Some((4, 3)), "1 + 2 in the same round");
    s.meta_mut().combat_round_seq = 5;
    play(&mut s, 0, Simple::ShieldsHolding);
    assert_eq!(timers(&s, 0).cancel_hits_round, Some((5, 2)), "a new round starts over");
    assert_eq!(timers(&s, 1).cancel_hits_round, None);
}

#[test]
fn reflective_shielding_hits_the_player_whose_ship_produced_the_hit_that_was_sustained() {
    let t = plain_systems();
    let mut s = four().state();
    s.transient_mut().combat.last_sustain = Some((t[1], Seat(0), Unit::DreadnoughtI, Seat(2), true));
    play(&mut s, 0, Simple::Reflective);
    assert_eq!(s.transient().combat.pending_reflective_hits, Some((t[1], Seat(2), 2)));
    // Not our unit that sustained: nothing.
    let mut other = four().state();
    other.transient_mut().combat.last_sustain = Some((t[1], Seat(1), Unit::DreadnoughtI, Seat(2), true));
    play(&mut other, 0, Simple::Reflective);
    assert_eq!(other.transient().combat.pending_reflective_hits, None);
    // The producer is ourselves (a hit of our own): nothing.
    let mut own = four().state();
    own.transient_mut().combat.last_sustain = Some((t[1], Seat(0), Unit::DreadnoughtI, Seat(0), true));
    play(&mut own, 0, Simple::Reflective);
    assert_eq!(own.transient().combat.pending_reflective_hits, None);
    // No sustain at all.
    let mut none = four().state();
    play(&mut none, 0, Simple::Reflective);
    assert_eq!(none.transient().combat.pending_reflective_hits, None);
}

#[test]
fn confounding_legal_text_and_deadly_plot_change_the_agenda() {
    let mut s = four().control(Planet::Quann, 2).control(Planet::Lodor, 2).control(Planet::Wellon, 3).state();
    assert_eq!(s.agenda().elected_override, None);
    play(&mut s, 2, Simple::Confounding);
    assert_eq!(s.agenda().elected_override, Some(Seat(2)), "the holder is the elected player");
    play(&mut s, 2, Simple::DeadlyPlot);
    assert!(flag(&s, TransientFlags::AGENDA_DISCARDED), "the outcome is discarded");
    for p in [Planet::Quann, Planet::Lodor] {
        assert!(s.board().planet(p).flags.has(PlanetFlags::EXHAUSTED), "{p:?}: all the holder's planets are exhausted");
    }
    assert!(!s.board().planet(Planet::Wellon).flags.has(PlanetFlags::EXHAUSTED), "another player's planet is not");
}

#[test]
fn salvage_takes_the_commodities_of_the_other_sides_of_the_combat() {
    let t = plain_systems();
    let mut s = four().commodities(0, 1).commodities(1, 3).commodities(2, 2).commodities(3, 5).state();
    s.transient_mut().combat.last_combat_sides = Some((t[1], [Seat(0), Seat(1), Seat(2)].into_iter().collect()));
    play(&mut s, 0, Simple::Salvage);
    let c = |n: u8| s.players().by_seat(Seat(n)).unwrap().econ.commodities;
    assert_eq!((c(0), c(1), c(2), c(3)), (1 + 3 + 2, 0, 0, 5), "only the other sides of this combat lose theirs");
    // No combat remembered: nothing.
    let mut none = four().commodities(1, 3).state();
    play(&mut none, 0, Simple::Salvage);
    assert_eq!(none.players().by_seat(Seat(1)).unwrap().econ.commodities, 3);
    // No commodities to take: the holder is not changed.
    let mut dry = four().commodities(0, 4).state();
    dry.transient_mut().combat.last_combat_sides = Some((t[1], [Seat(0), Seat(1)].into_iter().collect()));
    play(&mut dry, 0, Simple::Salvage);
    assert_eq!(dry.players().by_seat(Seat(0)).unwrap().econ.commodities, 4);
}

#[test]
fn reverse_engineer_takes_the_component_action_card_another_player_just_discarded() {
    let mut s = four().state();
    let component = ActionCard::ALL.into_iter().find(|c| ac::is_component_action(*c)).unwrap();
    let reaction = ActionCard::ALL.into_iter().find(|c| !ac::is_component_action(*c)).unwrap();
    s.cards_mut().discarded_action.edit().push(component);
    s.cards_mut().discarded_action.edit().push(reaction);
    s.transient_mut().combat.last_action_discarded = Some((Seat(1), component));
    play(&mut s, 0, Simple::ReverseEngineer);
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice(), &[component]);
    assert!(!s.cards().discarded_action.contains(&component), "it left the discard pile");
    assert!(s.cards().discarded_action.contains(&reaction));
    // Our own discard, or a card with a different window, or a card no longer in the pile: nothing.
    for (by, card) in [(Seat(0), component), (Seat(1), reaction)] {
        let mut t = four().state();
        t.cards_mut().discarded_action.edit().push(card);
        t.transient_mut().combat.last_action_discarded = Some((by, card));
        play(&mut t, 0, Simple::ReverseEngineer);
        assert!(t.players().by_seat(Seat(0)).unwrap().cards.hand.is_empty(), "{by:?} {card:?}");
    }
    let mut gone = four().state();
    gone.transient_mut().combat.last_action_discarded = Some((Seat(1), component));
    play(&mut gone, 0, Simple::ReverseEngineer);
    assert!(gone.players().by_seat(Seat(0)).unwrap().cards.hand.is_empty());
    let mut idle = four().state();
    play(&mut idle, 0, Simple::ReverseEngineer);
    assert!(idle.players().by_seat(Seat(0)).unwrap().cards.hand.is_empty());
}

#[test]
fn economic_initiative_readies_the_cultural_planets_we_control_in_the_sources_in_play() {
    // Quann is cultural, Wellon industrial, Vefut II hazardous, Olergodt hazardous and cultural (Thunder's Edge).
    let sc = four().control(Planet::Quann, 0).control(Planet::Wellon, 0).control(Planet::VefutII, 0).control(Planet::Olergodt, 0).control(Planet::Lodor, 1);
    let mut s = sc.clone().exhaust(Planet::Quann).exhaust(Planet::Wellon).exhaust(Planet::VefutII).exhaust(Planet::Olergodt).exhaust(Planet::Lodor).state();
    play(&mut s, 0, Simple::EconomicInitiative);
    let ex = |p: Planet| s.board().planet(p).flags.has(PlanetFlags::EXHAUSTED);
    assert!(!ex(Planet::Quann) && !ex(Planet::Olergodt), "cultural planets are readied (a dual one too)");
    assert!(ex(Planet::Wellon) && ex(Planet::VefutII), "other traits stay exhausted");
    assert!(ex(Planet::Lodor), "another player's cultural planet");
    // A source out of play: its planet is not readied.
    let mut base_only = sc.exhaust(Planet::Olergodt).exhaust(Planet::Quann).state();
    let base = ti4_engine_core::tables::source_bit(ti4_engine_core::generated::Source::Base);
    ac::play_simple(&mut base_only, Seat(0), Simple::EconomicInitiative, Env { scope: Scope(base), ..env() });
    assert!(!base_only.board().planet(Planet::Quann).flags.has(PlanetFlags::EXHAUSTED));
    assert!(base_only.board().planet(Planet::Olergodt).flags.has(PlanetFlags::EXHAUSTED));
}

#[test]
fn the_distinguished_councilor_adds_five_votes_to_this_agenda_and_again_to_the_same_one() {
    let mut s = four().state();
    s.meta_mut().agenda_seq = 3;
    play(&mut s, 1, Simple::Councilor);
    assert_eq!(timers(&s, 1).extra_votes_agenda, Some(ti4_engine_core::state::ExtraVotes { agenda: 3, votes: 5 }));
    play(&mut s, 1, Simple::Councilor);
    assert_eq!(timers(&s, 1).extra_votes_agenda.unwrap().votes, 10);
    s.meta_mut().agenda_seq = 4;
    play(&mut s, 1, Simple::Councilor);
    assert_eq!(timers(&s, 1).extra_votes_agenda, Some(ti4_engine_core::state::ExtraVotes { agenda: 4, votes: 5 }), "the votes of the earlier agenda are gone");
    assert_eq!(timers(&s, 0).extra_votes_agenda, None);
}

#[test]
fn emergency_repairs_repairs_every_unit_of_the_holder_in_the_active_system_only() {
    let t = plain_systems();
    let planet = t[1].data().planets[0];
    let hurt = |s: &mut CoreState, system: ti4_engine_core::generated::System, planet: Option<Planet>, seat: u8, kind: Unit| {
        let list = match planet {
            Some(p) => &mut s.board_mut().planet_mut(p).units,
            None => &mut s.board_mut().sys_mut(system).units,
        };
        for u in list.as_mut_slice() {
            if u.owner() == Seat(seat) && u.kind() == kind {
                *u = u.with_sustained(true);
            }
        }
    };
    let mut s = four()
        .active_system(t[1])
        .unit(t[1], None, Unit::DreadnoughtI, 0, 2)
        .unit(t[1], None, Unit::DreadnoughtI, 1, 1)
        .unit(t[1], Some(planet), Unit::FactionlessMech, 0, 1)
        .unit(t[2], None, Unit::DreadnoughtI, 0, 1)
        .state();
    hurt(&mut s, t[1], None, 0, Unit::DreadnoughtI);
    hurt(&mut s, t[1], None, 1, Unit::DreadnoughtI);
    hurt(&mut s, t[1], Some(planet), 0, Unit::FactionlessMech);
    hurt(&mut s, t[2], None, 0, Unit::DreadnoughtI);
    play(&mut s, 0, Simple::EmergencyRepairs);
    let sustained = |list: &[ti4_engine_core::state::Piece], seat: u8| list.iter().filter(|u| u.owner() == Seat(seat) && u.sustained()).count();
    assert_eq!(sustained(s.board().sys(t[1]).units.as_slice(), 0), 0, "ours in space");
    assert_eq!(sustained(s.board().planet(planet).units.as_slice(), 0), 0, "ours on a planet");
    assert_eq!(sustained(s.board().sys(t[1]).units.as_slice(), 1), 1, "a rival's stays damaged");
    assert_eq!(sustained(s.board().sys(t[2]).units.as_slice(), 0), 1, "another system");
    // No active system: nothing happens.
    let mut idle = four().unit(t[1], None, Unit::DreadnoughtI, 0, 1).state();
    hurt(&mut idle, t[1], None, 0, Unit::DreadnoughtI);
    let before = idle.clone();
    play(&mut idle, 0, Simple::EmergencyRepairs);
    assert!(idle == before);
}

#[test]
fn disable_marks_the_activation_only_when_an_opponent_has_a_pds_in_the_active_system() {
    let t = plain_systems();
    let planet = t[1].data().planets[0];
    let mut s = four().active_system(t[1]).unit(t[1], Some(planet), Unit::PDSI, 1, 1).state();
    s.meta_mut().activation_seq = 6;
    play(&mut s, 0, Simple::Disable);
    assert_eq!(marks(&s, 0, RoundKind::DisableInvasion), vec![6]);
    // Our own PDS, a rival's other structure, or a PDS in another system: nothing.
    for sc in [
        four().active_system(t[1]).unit(t[1], Some(planet), Unit::PDSI, 0, 1),
        four().active_system(t[1]).unit(t[1], Some(planet), Unit::SpaceDockI, 1, 1),
        four().active_system(t[1]).unit(t[2], Some(t[2].data().planets[0]), Unit::PDSI, 1, 1),
        four().unit(t[1], Some(planet), Unit::PDSI, 1, 1),
    ] {
        let mut s = sc.state();
        play(&mut s, 0, Simple::Disable);
        assert!(marks(&s, 0, RoundKind::DisableInvasion).is_empty());
    }
}

// ---- when a card can be reacted with ----------------------------------------------------------------------

fn can(s: &CoreState, who: u8, card: ActionCard, system: ti4_engine_core::generated::System, actor: Option<u8>) -> Option<bool> {
    ac::reaction_can_matter(s, ALL_SOURCES, Seat(who), card, Window { system, actor: actor.map(Seat), planet: None })
}

#[test]
fn bunker_matters_when_another_player_acts_in_a_system_where_we_control_a_planet() {
    let t = plain_systems();
    let planet = t[1].data().planets[0];
    let s = four().control(planet, 0).state();
    assert_eq!(can(&s, 0, C::Bunker, t[1], Some(1)), Some(true));
    assert_eq!(can(&s, 0, C::Bunker, t[1], Some(0)), Some(false), "our own action");
    assert_eq!(can(&s, 0, C::Bunker, t[1], None), Some(false), "nobody acts");
    assert_eq!(can(&s, 0, C::Bunker, t[2], Some(1)), Some(false), "another system");
    assert_eq!(can(&s, 1, C::Bunker, t[1], Some(0)), Some(false), "not our planet");
}

#[test]
fn blitz_and_disable_matter_to_the_attacker_with_the_right_pieces() {
    let t = plain_systems();
    let planet = t[1].data().planets[0];
    // Blitz: our own non-fighter ship without BOMBARDMENT in the system, and we act.
    let s = four().unit(t[1], None, Unit::CruiserI, 0, 1).state();
    assert_eq!(can(&s, 0, C::Blitz, t[1], Some(0)), Some(true));
    assert_eq!(can(&s, 0, C::Blitz, t[1], Some(1)), Some(false), "only the active player");
    let fighters = four().unit(t[1], None, Unit::FighterI, 0, 2).state();
    assert_eq!(can(&fighters, 0, C::Blitz, t[1], Some(0)), Some(false), "a fighter does not count");
    let bombarders = four().unit(t[1], None, Unit::DreadnoughtI, 0, 1).state();
    assert_eq!(can(&bombarders, 0, C::Blitz, t[1], Some(0)), Some(false), "a dreadnought already has BOMBARDMENT");
    let mixed = four().unit(t[1], None, Unit::DreadnoughtI, 0, 1).unit(t[1], None, Unit::CruiserI, 0, 1).state();
    assert_eq!(can(&mixed, 0, C::Blitz, t[1], Some(0)), Some(true));
    let other = four().unit(t[1], None, Unit::CruiserI, 1, 1).state();
    assert_eq!(can(&other, 0, C::Blitz, t[1], Some(0)), Some(false), "their ship");
    // Disable: another player's PDS, on a planet, while we act.
    let pds = four().unit(t[1], Some(planet), Unit::PDSI, 1, 1).state();
    assert_eq!(can(&pds, 0, C::Disable, t[1], Some(0)), Some(true));
    assert_eq!(can(&pds, 0, C::Disable, t[1], Some(1)), Some(false));
    assert_eq!(can(&pds, 1, C::Disable, t[1], Some(1)), Some(false), "their own PDS");
    assert_eq!(can(&four().unit(t[1], Some(planet), Unit::SpaceDockI, 1, 1).state(), 0, C::Disable, t[1], Some(0)), Some(false));
}

#[test]
fn the_combat_cards_matter_where_their_pieces_are() {
    let t = plain_systems();
    let planet = t[1].data().planets[0];
    // Fighter Prototype and Upgrade need our own fighter / cruiser in the system, in space or on a planet.
    let s = four().unit(t[1], None, Unit::FighterI, 0, 1).unit(t[1], None, Unit::CruiserI, 0, 1).state();
    assert_eq!(can(&s, 0, C::FighterPrototype, t[1], None), Some(true));
    assert_eq!(can(&s, 0, C::Upgrade, t[1], None), Some(true));
    assert_eq!(can(&s, 1, C::FighterPrototype, t[1], None), Some(false));
    assert_eq!(can(&s, 0, C::FighterPrototype, t[2], None), Some(false));
    let none = four().unit(t[1], None, Unit::DreadnoughtI, 0, 1).state();
    assert_eq!(can(&none, 0, C::Upgrade, t[1], None), Some(false), "a dreadnought is no cruiser");
    assert_eq!(can(&none, 0, C::FighterPrototype, t[1], None), Some(false));
    // Rally: any ship of another player in space (a fighter too).
    let rally = four().unit(t[1], None, Unit::FighterI, 1, 1).state();
    assert_eq!(can(&rally, 0, C::Rally, t[1], None), Some(true));
    assert_eq!(can(&rally, 1, C::Rally, t[1], None), Some(false), "their own ship");
    assert_eq!(can(&four().unit(t[1], Some(planet), Unit::InfantryI, 1, 2).state(), 0, C::Rally, t[1], None), Some(false), "ground forces are no ship");
    // Rescue and Intercept: we have a ship in the system and we are not the one acting.
    let ours = four().unit(t[1], None, Unit::CruiserI, 0, 1).state();
    for card in [C::Rescue, C::Intercept] {
        assert_eq!(can(&ours, 0, card, t[1], Some(1)), Some(true), "{card:?}");
        assert_eq!(can(&ours, 0, card, t[1], Some(0)), Some(false), "{card:?}: our own action");
        assert_eq!(can(&ours, 0, card, t[2], Some(1)), Some(false), "{card:?}: another system");
        assert_eq!(can(&four().unit(t[1], Some(planet), Unit::InfantryI, 0, 1).state(), 0, card, t[1], Some(1)), Some(false), "{card:?}: ground forces only");
    }
    // Emergency Repairs: one of our pieces, in space or on a planet, is damaged.
    let mut hurt = four().unit(t[1], None, Unit::DreadnoughtI, 0, 1).unit(t[1], Some(planet), Unit::FactionlessMech, 0, 1).state();
    assert_eq!(can(&hurt, 0, C::EmergencyRepairs, t[1], None), Some(false));
    let u = hurt.board().planet(planet).units.as_slice()[0].with_sustained(true);
    hurt.board_mut().planet_mut(planet).units.as_mut_slice()[0] = u;
    assert_eq!(can(&hurt, 0, C::EmergencyRepairs, t[1], None), Some(true));
    assert_eq!(can(&hurt, 1, C::EmergencyRepairs, t[1], None), Some(false));
    // A card with no extra requirement always matters; two need the string's answer.
    assert_eq!(can(&ours, 0, C::Counterstroke, t[1], None), Some(true));
    assert_eq!(can(&ours, 0, C::HarnessEnergy, t[1], None), None);
    assert_eq!(can(&ours, 0, C::Infiltrate, t[1], None), None);
}

// ---- when a card can be played -------------------------------------------------------------------------------

#[test]
fn the_cards_with_a_condition_in_the_action_window_need_their_target() {
    let t = plain_systems();
    let a = t[1].data().planets[0];
    let b = t[2].data().planets[0];
    let has = |s: &CoreState, c: ActionCard| ac::has_legal_target(s, false, ALL_SOURCES, Seat(0), c);
    let empty = four().state();
    for c in [C::LuckyShot, C::TacticalBombardment, C::Uprising, C::FrontlineDeployment, C::ReactorMeltdown, C::UnstablePlanet] {
        assert_eq!(has(&empty, c), Some(false), "{c:?}");
    }
    // Lucky Shot: a dreadnought, cruiser or destroyer of another player in a system where we control a planet.
    let lucky = four().control(a, 0).unit(t[1], None, Unit::DestroyerI, 1, 1).state();
    assert_eq!(has(&lucky, C::LuckyShot), Some(true));
    assert_eq!(has(&four().control(a, 0).unit(t[1], None, Unit::CarrierI, 1, 1).state(), C::LuckyShot), Some(false), "a carrier is not a target");
    assert_eq!(has(&four().control(a, 0).unit(t[1], None, Unit::DestroyerI, 0, 1).state(), C::LuckyShot), Some(false), "our own ship");
    assert_eq!(has(&four().control(a, 1).unit(t[1], None, Unit::DestroyerI, 1, 1).state(), C::LuckyShot), Some(false), "not our planet");
    // Tactical Bombardment: our ship and a planet of another player in one system.
    let bomb = four().control(a, 1).unit(t[1], None, Unit::CruiserI, 0, 1).state();
    assert_eq!(has(&bomb, C::TacticalBombardment), Some(true));
    assert_eq!(has(&four().control(a, 0).unit(t[1], None, Unit::CruiserI, 0, 1).state(), C::TacticalBombardment), Some(false), "our own planet");
    assert_eq!(has(&four().control(a, 1).unit(t[2], None, Unit::CruiserI, 0, 1).state(), C::TacticalBombardment), Some(false), "ship elsewhere");
    // Uprising: a planet of another player outside the home systems.
    assert_eq!(has(&four().control(a, 1).state(), C::Uprising), Some(true));
    let home = Planet::Muaat;
    assert_eq!(has(&four().control(home, 1).state(), C::Uprising), Some(false), "a home system");
    assert_eq!(has(&four().control(a, 0).state(), C::Uprising), Some(false), "ours");
    // Rise of a Messiah and Frontline Deployment: any planet of ours.
    for c in [C::RiseOfAMessiah, C::FrontlineDeployment] {
        assert_eq!(has(&four().control(a, 0).state(), c), Some(true));
        assert_eq!(has(&four().control(a, 1).state(), c), Some(false));
    }
    // Reactor Meltdown: a space dock outside the home systems.
    let dock = |planet: Planet, system: ti4_engine_core::generated::System| four().control(planet, 1).unit(system, Some(planet), Unit::SpaceDockI, 1, 1).state();
    assert_eq!(has(&dock(a, t[1]), C::ReactorMeltdown), Some(true));
    assert_eq!(has(&dock(Planet::Muaat, System::MuaatMuaat), C::ReactorMeltdown), Some(false), "a home system");
    assert_eq!(has(&four().control(a, 1).unit(t[1], Some(a), Unit::PDSI, 1, 1).state(), C::ReactorMeltdown), Some(false), "a PDS is no dock");
    // Unstable Planet: a hazardous planet with units on it (any owner).
    let hazard = Planet::VefutII;
    assert_eq!(has(&four().unit(System::VefutII, Some(hazard), Unit::InfantryI, 1, 1).state(), C::UnstablePlanet), Some(true));
    assert_eq!(has(&four().control(hazard, 1).state(), C::UnstablePlanet), Some(false), "no units entry");
    assert_eq!(has(&four().unit(System::Wellon, Some(Planet::Wellon), Unit::InfantryI, 1, 1).state(), C::UnstablePlanet), Some(false), "an industrial planet");
    let _ = b;
}

#[test]
fn the_price_of_a_card_decides_whether_it_can_be_played() {
    let s = four().tech(0, T::DuraniumArmor).tech(1, T::CruiserII).tech(2, T::Bioplasmosis).state();
    // Divert Funding: a technology that is not a unit upgrade and not a faction technology.
    assert_eq!(ac::cost_is_payable(&s, Seat(0), C::DivertFunding, 0), Some(true));
    assert_eq!(ac::cost_is_payable(&s, Seat(1), C::DivertFunding, 0), Some(false), "a unit upgrade");
    assert_eq!(ac::cost_is_payable(&s, Seat(2), C::DivertFunding, 0), Some(false), "a faction technology");
    assert_eq!(ac::cost_is_payable(&s, Seat(3), C::DivertFunding, 0), Some(false), "none");
    assert_eq!(ac::cost_is_payable(&s, Seat(0), C::Bribery, 0), Some(false));
    assert_eq!(ac::cost_is_payable(&s, Seat(0), C::Bribery, 1), Some(true));
    assert_eq!(ac::cost_is_payable(&s, Seat(0), C::Spy, 0), Some(true));
}

#[test]
fn a_card_is_playable_by_the_gates_then_the_cost_then_the_target_by_rules_version() {
    let t = plain_systems();
    let goods = || 0;
    let gates = Gates { faction_forbids: false, political_secret_bars: false, goods: &goods };
    let at = |s: &CoreState, c: ActionCard, g: &Gates<'_>| ac::is_playable(s, false, ALL_SOURCES, Seat(0), c, g);
    let s = four().state();
    // Bribery costs a trade good (from version 23); Spy needs a target (from version 10); Signal Jamming is the string's.
    assert_eq!(at(&s, C::Bribery, &gates), Some(false));
    assert_eq!(at(&s, C::Spy, &gates), Some(false));
    assert_eq!(at(&s, C::Summit, &gates), Some(true));
    assert_eq!(at(&s, C::SignalJamming, &gates), None);
    let rich = |n: i64| move || n;
    let two = rich(2);
    let g2 = Gates { faction_forbids: false, political_secret_bars: false, goods: &two };
    assert_eq!(at(&s, C::Bribery, &g2), Some(true), "two trade goods pay");
    let mut old = four().state();
    old.meta_mut().rules_version = 22;
    assert_eq!(at(&old, C::Bribery, &gates), Some(true), "no cost check before version 23");
    assert_eq!(at(&old, C::Spy, &gates), Some(false), "targets are checked from version 10");
    old.meta_mut().rules_version = 9;
    assert_eq!(at(&old, C::Spy, &gates), Some(true), "no target check before version 10");
    // The gates come first, whatever the card.
    let barred = Gates { faction_forbids: true, political_secret_bars: false, goods: &two };
    assert_eq!(at(&s, C::Summit, &barred), Some(false));
    assert_eq!(at(&s, C::SignalJamming, &barred), Some(false), "a barred player has no Signal Jamming question either");
    let secret = Gates { faction_forbids: false, political_secret_bars: true, goods: &two };
    assert_eq!(at(&s, C::Summit, &secret), Some(false));
    let mut censure = four().state();
    censure.agenda_mut().laws.insert(Agenda::PoliticalCensure, Outcome::Seat(Seat(0)));
    assert_eq!(at(&censure, C::Summit, &gates), Some(false));
    let _ = t;
}

#[test]
fn the_playable_hand_lists_the_component_actions_that_can_be_played_in_hand_order() {
    let component: Vec<ActionCard> = ActionCard::ALL.into_iter().filter(|c| ac::is_component_action(*c) && ac::has_legal_target(&four().state(), false, ALL_SOURCES, Seat(0), *c) == Some(true) && !matches!(*c, C::Bribery | C::SignalJamming | C::DivertFunding)).take(2).collect();
    let reaction = ActionCard::ALL.into_iter().find(|c| !ac::is_component_action(*c)).unwrap();
    let hand = [component[0], reaction, C::Spy, component[1]];
    let s = four().hand(0, &hand).state();
    let goods = || 0;
    let gates = Gates { faction_forbids: false, political_secret_bars: false, goods: &goods };
    // Spy has no target (nobody else holds a card): left out; the reaction card is no component action.
    let got = ac::playable_hand(&s, false, ALL_SOURCES, Seat(0), &gates).unwrap();
    assert_eq!(got.as_slice(), &[0, 3]);
    let with_target = four().hand(0, &hand).hand(1, &[C::Summit]).state();
    assert_eq!(ac::playable_hand(&with_target, false, ALL_SOURCES, Seat(0), &gates).unwrap().as_slice(), &[0, 2, 3]);
    // A card the core cannot judge makes the whole hand the string's.
    let jam = four().hand(0, &[C::SignalJamming]).state();
    assert!(ac::playable_hand(&jam, false, ALL_SOURCES, Seat(0), &gates).is_none());
    // A seat that is not there.
    assert!(ac::playable_hand(&s, false, ALL_SOURCES, Seat(9), &gates).is_none());
}

#[test]
fn discarding_takes_the_card_at_the_position_out_of_the_hand() {
    let mut s = four().hand(0, &[C::Spy, C::Summit, C::Rally]).state();
    assert_eq!(ac::discard_from_hand(&mut s, Seat(0), 1), Some(C::Summit));
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice(), &[C::Spy, C::Rally]);
    assert_eq!(ac::discard_from_hand(&mut s, Seat(0), 2), None, "past the end");
    assert_eq!(ac::discard_from_hand(&mut s, Seat(9), 0), None, "no such seat");
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 2);
    assert_eq!(ac::discard_from_hand(&mut s, Seat(0), 0), Some(C::Spy));
}

#[test]
fn the_component_action_gate_names_the_home_systems_of_the_sources_in_play() {
    // Uprising skips home systems of sources in play only.
    let s = four().control(Planet::Muaat, 1).state();
    let base_only = Scope(ti4_engine_core::tables::source_bit(ti4_engine_core::generated::Source::Base));
    assert_eq!(ac::has_legal_target(&s, false, base_only, Seat(0), C::Uprising), Some(false));
    assert_eq!(ac::has_legal_target(&s, false, Scope(0), Seat(0), C::Uprising), Some(true), "no source in play: no home system is known");
}

// ---- survivor-driven (mutation round 1) ------------------------------------------------------------------------------

#[test]
fn rout_and_waylay_each_set_their_own_stamp() {
    let mut s = four().state();
    s.meta_mut().combat_round_seq = 6;
    play(&mut s, 2, Simple::Rout);
    play(&mut s, 3, Simple::Waylay);
    assert_eq!((timers(&s, 2).rout_round.get(), timers(&s, 2).waylay_barrage_round.get()), (Some(6), None));
    assert_eq!((timers(&s, 3).rout_round.get(), timers(&s, 3).waylay_barrage_round.get()), (None, Some(6)));
}

#[test]
fn scuttle_and_war_effort_need_a_ship_of_ours_and_unstable_planet_one_of_a_source_in_play() {
    let t = plain_systems();
    let has = |s: &CoreState, c: ActionCard, scope: Scope| ac::has_legal_target(s, false, scope, Seat(0), c);
    // Only fighters of ours, or only ships of a rival: no Scuttle; War Effort needs a ship of ours (a fighter is one).
    let fighters = four().unit(t[1], None, Unit::FighterI, 0, 2).state();
    assert_eq!(has(&fighters, C::Scuttle, ALL_SOURCES), Some(false));
    assert_eq!(has(&fighters, C::WarEffort, ALL_SOURCES), Some(true));
    let rival = four().unit(t[1], None, Unit::CruiserI, 1, 1).state();
    assert_eq!(has(&rival, C::Scuttle, ALL_SOURCES), Some(false));
    assert_eq!(has(&rival, C::WarEffort, ALL_SOURCES), Some(false));
    let ours = four().unit(t[1], None, Unit::DestroyerI, 0, 1).state();
    assert_eq!(has(&ours, C::Scuttle, ALL_SOURCES), Some(true));
    // A unit of a source that is not in play is not looked up: Scuttle may still return it (the unit table does not know it).
    let base = Scope(ti4_engine_core::tables::source_bit(ti4_engine_core::generated::Source::Base));
    let te_ship = four().unit(t[1], None, Unit::NeutralDestroyer, 0, 1).state();
    let _ = (has(&te_ship, C::Scuttle, base), base);
    // Unstable Planet: a hazardous planet with units on it, in a source that is in play.
    let hazard = four().unit(System::Tempesta, Some(Planet::Tempesta), Unit::InfantryI, 1, 1).state();
    assert_eq!(has(&hazard, C::UnstablePlanet, ALL_SOURCES), Some(true));
    assert_eq!(has(&hazard, C::UnstablePlanet, base), Some(false), "Tempesta is a Thunder's Edge planet");
}

#[test]
fn the_default_facts_of_a_play_are_empty_and_the_latest_rules() {
    let f = Facts::default();
    assert_eq!(f.neutral, None);
    assert!(!f.agent_offered && !f.holder_deepwrought && !f.replenish_events && !f.fracture_in_play && !f.map);
    assert_eq!(f.rules_version, u32::MAX);
    assert_eq!((f.destroyer, f.dreadnought, f.mech, f.pds, f.spacedock), (Placed::NONE, Placed::NONE, Placed::NONE, Placed::NONE, Placed::NONE));
    assert!(f.roster.is_none() && f.reroll.is_none() && f.reveal.iter().next().is_none());
    assert_eq!(f.traders, [None, None]);
    assert!(f.own_breakthrough.iter().all(Option::is_none));
    assert!(!f.carried.plain_draw && !f.carried.research && !f.carried.payments && f.carried.structures.is_none());
    assert!(!f.hits.staging);
    assert_eq!(f.courageous_subtype, ti4_engine_core::state::Code(0));
    assert_eq!((f.tech_order)(T::AntimassDeflectors, T::GravityDrive), core::cmp::Ordering::Less);
    assert_eq!((f.unit_order)(Unit::InfantryI, Unit::CruiserI), Unit::InfantryI.cmp(&Unit::CruiserI));
}
