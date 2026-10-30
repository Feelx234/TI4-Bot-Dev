//! D4 lane A3: `query/reach.rs`, the rules one mover carries (`SeatReach`) and the ships that get there (`SeatFleet`), from the printed movement
//! rules (anomalies, enemy ships, command tokens), the laws, the technologies and the faction abilities that change them.

use ti4_engine_core::containers::Opt32;
use ti4_engine_core::generated::{Agenda, Breakthrough, Planet, Relic};
use ti4_engine_core::hex::Hex;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::query::reach::{self, Movable, SeatFleet, SeatReach};
use ti4_engine_core::state::{MetaFlags, Outcome, Phase, RoundKind};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

/// Four tiles in a row, 0 .. 3, the factions seated in the given order.
fn row(tiles: [System; 4], factions: &[Faction]) -> Scenario {
    Scenario::new(factions)
        .map_hexes(&[(tiles[0], Hex::new(0, 0)), (tiles[1], Hex::new(1, 0)), (tiles[2], Hex::new(2, 0)), (tiles[3], Hex::new(3, 0))])
        .tokens(0, 3, 3, 2)
        .tokens(1, 3, 3, 2)
}

fn plain4() -> [System; 4] {
    let p = plain_systems();
    [p[0], p[1], p[2], p[3]]
}

const DUEL: [Faction; 2] = [Faction::TheArborec, Faction::TheEmiratesOfHacan];

/// The origins of the ships of `seat` that reach `dest`.
fn reaching(s: &CoreState, k: &Kernel, seat: u8, dest: System, effects: bool) -> Vec<Movable> {
    let reach = SeatReach::new(s, k, Seat(seat), effects).expect("a map");
    SeatFleet::new(s, k, Seat(seat)).movable(&reach.rules_for(s, k, dest.index_ix()))
}

trait Ix {
    fn index_ix(self) -> ti4_engine_core::ids::SystemIx;
}
impl Ix for System {
    fn index_ix(self) -> ti4_engine_core::ids::SystemIx {
        ti4_engine_core::ids::system_at(self.index())
    }
}

fn origins(found: &[Movable]) -> Vec<System> {
    found.iter().map(|m| m.origin).collect()
}

fn anomaly(f: impl Fn(&ti4_engine_core::generated::Anomalies) -> bool) -> System {
    System::ALL.iter().copied().find(|s| s.data().planets.is_empty() && !s.data().hyperlane && f(&s.data().anomalies)).expect("an anomaly tile")
}

// ---- the map and the board -------------------------------------------------------------------

#[test]
fn a_position_without_a_map_has_no_reach() {
    let s = Scenario::new(&DUEL).state();
    assert!(s.galaxy().is_none() || SeatReach::new(&s, &kernel_all(), Seat(0), true).is_some());
    let bare = CoreState::all_loaded();
    assert!(bare.galaxy().is_none());
    assert!(SeatReach::new(&bare, &kernel_all(), Seat(0), true).is_none());
    assert!(SeatReach::new(&bare, &kernel_all(), Seat(0), false).is_none());
}

#[test]
fn the_rules_carry_the_other_players_ships_and_the_movers_own_tokens() {
    let t = plain4();
    let (s, k) = row(t, &DUEL)
        .unit(t[1], None, Unit::CruiserI, 1, 1) // an enemy ship
        .unit(t[2], Some(Planet::ALL.iter().copied().find(|p| p.data().system == Some(t[2])).unwrap()), Unit::InfantryI, 1, 2) // ground forces are no ships
        .unit(t[0], None, Unit::CruiserI, 0, 1) // our own ship
        .command_token(t[0], 0)
        .command_token(t[3], 1)
        .build();
    let r = SeatReach::new(&s, &k, Seat(0), true).unwrap();
    assert_eq!(r.seat(), Seat(0));
    let tpl = r.template();
    assert_eq!(tpl.enemy_ships.iter().collect::<Vec<_>>(), vec![t[1].index()]);
    assert_eq!(tpl.own_command_tokens.iter().collect::<Vec<_>>(), vec![t[0].index()]);
    let theirs = SeatReach::new(&s, &k, Seat(1), true).unwrap();
    assert_eq!(theirs.template().enemy_ships.iter().collect::<Vec<_>>(), vec![t[0].index()]);
    assert_eq!(theirs.template().own_command_tokens.iter().collect::<Vec<_>>(), vec![t[3].index()]);
    // The rules for a destination are the template with that active system.
    let rules = r.rules_for(&s, &k, t[2].index_ix());
    assert_eq!(rules.active, t[2].index_ix());
    assert_eq!(rules.enemy_ships, tpl.enemy_ships);
}

#[test]
fn own_unit_systems_are_the_systems_with_a_unit_of_the_mover_in_space_or_on_a_planet() {
    let t = plain4();
    let planet = t[2].data().planets[0];
    let (s, k) = row(t, &DUEL)
        .unit(t[0], None, Unit::CruiserI, 0, 1)
        .unit(t[2], Some(planet), Unit::InfantryI, 0, 1)
        .unit(t[1], None, Unit::CruiserI, 1, 1)
        .build();
    let r = SeatReach::new(&s, &k, Seat(0), true).unwrap();
    assert_eq!(r.own_unit_systems().iter().collect::<Vec<_>>(), vec![t[0].index(), t[2].index()]);
}

// ---- the path --------------------------------------------------------------------------------

#[test]
fn an_enemy_fleet_blocks_the_way_through_unless_the_light_wave_deflector_is_in_effect() {
    let t = plain4();
    let base = row(t, &DUEL).unit(t[1], None, Unit::CruiserI, 0, 1).unit(t[2], None, Unit::CruiserI, 1, 1);
    let (s, k) = base.clone().build();
    assert!(reaching(&s, &k, 0, t[3], true).is_empty(), "the cruiser needs two steps and an enemy fleet sits between");
    let (s, k) = base.tech(0, Technology::LightWaveDeflector).build();
    // The effect of the technology is part of a real move (`effects`), not of what a player can observe in general.
    assert_eq!(origins(&reaching(&s, &k, 0, t[3], true)), vec![t[1]]);
    assert!(reaching(&s, &k, 0, t[3], false).is_empty());
    assert!(SeatReach::new(&s, &k, Seat(0), true).unwrap().template().ignore_enemy_ships);
    assert!(!SeatReach::new(&s, &k, Seat(0), false).unwrap().template().ignore_enemy_ships);
}

#[test]
fn a_ship_cannot_leave_a_system_with_its_own_command_token_unless_the_dominus_orb_is_in_play() {
    let t = plain4();
    let (mut s, k) = row(t, &DUEL).unit(t[2], None, Unit::CruiserI, 0, 1).command_token(t[2], 0).build();
    assert!(reaching(&s, &k, 0, t[3], true).is_empty(), "58.4: not from a system holding our token");
    s.meta_mut().activation_seq = 3;
    s.players_mut().by_seat_mut(Seat(0)).unwrap().round_marks.push(RoundKind::DominusOrb, 3);
    assert_eq!(origins(&reaching(&s, &k, 0, t[3], true)), vec![t[2]]);
    assert!(SeatReach::new(&s, &k, Seat(0), true).unwrap().template().command_tokens_ignored);
    assert!(!SeatReach::new(&s, &k, Seat(0), false).unwrap().template().command_tokens_ignored, "the card is a movement effect");
    // Another activation: the orb is spent.
    s.meta_mut().activation_seq = 4;
    assert!(reaching(&s, &k, 0, t[3], true).is_empty());
}

#[test]
fn the_cards_of_the_activation_open_the_anomalies_and_silence_the_fleets() {
    let nebula = anomaly(|a| a.nebula);
    let t = [plain_systems()[0], nebula, plain_systems()[1], plain_systems()[2]];
    let (mut s, k) = row(t, &DUEL).unit(t[0], None, Unit::CruiserI, 0, 1).build();
    s.meta_mut().activation_seq = 5;
    assert!(reaching(&s, &k, 0, t[2], true).is_empty(), "a nebula is entered only as the active system");
    s.players_mut().by_seat_mut(Seat(0)).unwrap().card_timers.anomalies_ignored_activation = Opt32::some(5);
    assert_eq!(origins(&reaching(&s, &k, 0, t[2], true)), vec![t[0]]);
    assert!(SeatReach::new(&s, &k, Seat(0), true).unwrap().template().anomalies_ignored);
    assert!(!SeatReach::new(&s, &k, Seat(0), false).unwrap().template().anomalies_ignored);
    s.meta_mut().activation_seq = 6;
    assert!(reaching(&s, &k, 0, t[2], true).is_empty());
    // Silence: the system whose enemy ships are ignored is set for the activation only.
    let sys = t[3];
    let p = s.players_mut().by_seat_mut(Seat(0)).unwrap();
    p.card_timers.silence_activation = Opt32::some(6);
    p.card_timers.silence_system = Some(sys);
    assert_eq!(SeatReach::new(&s, &k, Seat(0), true).unwrap().template().ignore_enemy_ships_from, Some(sys.index_ix()));
    assert_eq!(SeatReach::new(&s, &k, Seat(0), false).unwrap().template().ignore_enemy_ships_from, None);
    s.meta_mut().activation_seq = 7;
    assert_eq!(SeatReach::new(&s, &k, Seat(0), true).unwrap().template().ignore_enemy_ships_from, None);
}

#[test]
fn circlet_of_the_void_lets_the_ships_ignore_the_rifts() {
    let t = plain4();
    let (s, k) = row(t, &DUEL).state_with_relic(Relic::CircletOfTheVoid);
    assert!(SeatReach::new(&s, &k, Seat(0), true).unwrap().template().rifts_ignored);
    assert!(!SeatReach::new(&s, &k, Seat(0), false).unwrap().template().rifts_ignored);
    assert!(!SeatReach::new(&s, &k, Seat(1), true).unwrap().template().rifts_ignored);
}

trait WithRelic {
    fn state_with_relic(self, relic: Relic) -> (CoreState, Kernel);
}
impl WithRelic for Scenario {
    fn state_with_relic(self, relic: Relic) -> (CoreState, Kernel) {
        let (mut s, k) = self.build();
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.relics.push(relic);
        (s, k)
    }
}

// ---- the technologies and laws --------------------------------------------------------------

#[test]
fn antimass_deflectors_open_the_asteroid_fields_and_a_valefar_assimilator_carries_them() {
    let field = anomaly(|a| a.asteroid_field);
    let p = plain_systems();
    let t = [p[0], field, p[1], p[2]];
    let base = row(t, &DUEL).unit(t[0], None, Unit::CruiserI, 0, 1);
    let (s, k) = base.clone().build();
    assert!(reaching(&s, &k, 0, t[2], true).is_empty(), "58.4c: no passage through an asteroid field");
    let (s, k) = base.clone().tech(0, Technology::AntimassDeflectors).build();
    assert_eq!(origins(&reaching(&s, &k, 0, t[2], true)), vec![t[0]]);
    assert!(reach::technology_text(&s, Seat(0), Technology::AntimassDeflectors));
    assert!(!reach::technology_text(&s, Seat(1), Technology::AntimassDeflectors));
    // The Nekro's card stands for the technology while another player still owns it.
    let nekro = Scenario::new(&[Faction::TheNekroVirus, Faction::TheArborec]).tech(0, Technology::ValefarAssimilatorX).tech(1, Technology::AntimassDeflectors);
    let mut s = nekro.clone().state();
    assert!(!reach::technology_text(&s, Seat(0), Technology::AntimassDeflectors), "the card names no technology yet");
    s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.assimilated.insert(Technology::ValefarAssimilatorX, Technology::AntimassDeflectors);
    assert!(reach::technology_text(&s, Seat(0), Technology::AntimassDeflectors));
    assert!(!reach::technology_text(&s, Seat(0), Technology::GravityDrive));
    // The owner lost it: the card stands for nothing.
    s.players_mut().by_seat_mut(Seat(1)).unwrap().tech.owned.remove(Technology::AntimassDeflectors);
    assert!(!reach::technology_text(&s, Seat(0), Technology::AntimassDeflectors));
    // Only the Nekro has Valefar Assimilators.
    let mut other = nekro.state();
    other.players_mut().by_seat_mut(Seat(1)).unwrap().tech.owned.insert(Technology::ValefarAssimilatorX);
    other.players_mut().by_seat_mut(Seat(1)).unwrap().tech.assimilated.insert(Technology::ValefarAssimilatorX, Technology::AntimassDeflectors);
    assert!(!reach::technology_text(&other, Seat(1), Technology::GravityDrive));
}

#[test]
fn shared_research_opens_the_nebulae_until_the_laws_are_blanked() {
    let nebula = anomaly(|a| a.nebula);
    let p = plain_systems();
    let t = [p[0], nebula, p[1], p[2]];
    let base = row(t, &DUEL).unit(t[0], None, Unit::CruiserI, 0, 1);
    let (mut s, k) = base.build();
    assert!(reaching(&s, &k, 0, t[2], true).is_empty());
    assert!(!reaching(&s, &k, 0, t[1], true).is_empty(), "the nebula itself can be the destination");
    s.agenda_mut().laws.insert(Agenda::SharedResearch, Outcome::Empty);
    assert_eq!(origins(&reaching(&s, &k, 0, t[2], true)), vec![t[0]]);
    // Law's Order: the Keleres mark for this very turn blanks every law in the action phase.
    s.meta_mut().turn_seq = 9;
    s.marks_mut().tail.edit().insert(MarkKey::KeleresLawsBlank(), MarkVal::Atom(Atom::Num(9)));
    assert!(reach::blanked(&s));
    assert!(reaching(&s, &k, 0, t[2], true).is_empty());
    assert!(!SeatReach::new(&s, &k, Seat(0), true).unwrap().template().nebulae_open);
}

#[test]
fn law_blanking_needs_the_mark_of_this_turn_in_the_action_phase() {
    let mut s = Scenario::new(&DUEL).state();
    s.meta_mut().turn_seq = 4;
    assert!(!reach::blanked(&s));
    s.marks_mut().tail.edit().insert(MarkKey::KeleresLawsBlank(), MarkVal::Atom(Atom::Num(4)));
    assert!(reach::blanked(&s));
    s.meta_mut().turn_seq = 5;
    assert!(!reach::blanked(&s), "a mark of an earlier turn");
    s.meta_mut().turn_seq = 4;
    s.meta_mut().phase = Phase::Status;
    assert!(!reach::blanked(&s), "only the action phase");
    s.meta_mut().phase = Phase::Action;
    s.marks_mut().tail.edit().insert(MarkKey::KeleresLawsBlank(), MarkVal::Atom(Atom::Num(3)));
    assert!(!reach::blanked(&s));
}

// ---- the faction abilities ------------------------------------------------------------------------

#[test]
fn the_muaat_pass_supernovae_and_enter_them_with_the_magmus_reactor() {
    let nova = anomaly(|a| a.supernova);
    let p = plain_systems();
    let t = [p[0], nova, p[1], p[2]];
    let duel = [Faction::TheEmbersOfMuaat, Faction::TheArborec];
    let base = row(t, &duel).unit(t[0], None, Unit::CruiserI, 0, 1).unit(t[0], None, Unit::CruiserI, 1, 1);
    let (s, k) = base.clone().build();
    assert_eq!(origins(&reaching(&s, &k, 0, t[2], true)), vec![t[0]], "Gashlai Physiology: through, not in");
    assert!(reaching(&s, &k, 1, t[2], true).is_empty(), "nobody else passes a supernova");
    assert!(reaching(&s, &k, 0, t[1], true).is_empty(), "entering needs the reactor");
    let (s, k) = base.tech(0, Technology::MagmusReactor).build();
    assert_eq!(origins(&reaching(&s, &k, 0, t[1], true)), vec![t[0]]);
    assert!(reaching(&s, &k, 1, t[1], true).is_empty());
}

#[test]
fn the_empyrean_ignore_the_nebulae() {
    let nebula = anomaly(|a| a.nebula);
    let p = plain_systems();
    let t = [p[0], nebula, p[1], p[2]];
    let (s, k) = row(t, &[Faction::TheEmpyrean, Faction::TheArborec]).unit(t[0], None, Unit::CruiserI, 0, 1).unit(t[0], None, Unit::CruiserI, 1, 1).build();
    assert_eq!(origins(&reaching(&s, &k, 0, t[2], true)), vec![t[0]]);
    assert!(reaching(&s, &k, 1, t[2], true).is_empty());
    assert!(SeatReach::new(&s, &k, Seat(0), true).unwrap().template().nebulae_ignored);
    assert!(!SeatReach::new(&s, &k, Seat(1), true).unwrap().template().nebulae_ignored);
}

#[test]
fn the_yssaril_flagship_moves_through_other_players_ships() {
    let t = plain4();
    let duel = [Faction::TheYssarilTribes, Faction::TheArborec];
    let blocker = |sc: Scenario| sc.unit(t[1], None, Unit::CruiserI, 1, 1);
    let (s, k) = blocker(row(t, &duel).unit(t[0], None, Unit::YsiaYssrila, 0, 1).unit(t[0], None, Unit::CruiserI, 0, 1)).build();
    let found = reaching(&s, &k, 0, t[2], true);
    assert_eq!(found.iter().map(|m| m.kind).collect::<Vec<_>>(), vec![Unit::YsiaYssrila], "only the flagship passes; the cruiser is stopped");
    // Without the flagship on the board nothing passes.
    let (s, k) = blocker(row(t, &duel).unit(t[0], None, Unit::CruiserI, 0, 1)).build();
    assert!(reaching(&s, &k, 0, t[2], true).is_empty());
}

#[test]
fn the_mentak_corsair_passes_only_when_the_active_system_holds_a_non_fighter_ship_of_another_player() {
    let t = plain4();
    let duel = [Faction::TheMentakCoalition, Faction::TheArborec];
    let base = row(t, &duel).unit(t[0], None, Unit::Corsair, 0, 1).unit(t[1], None, Unit::FighterI, 1, 1);
    // A fighter is in the way and the active system holds a fighter only: the Corsair cannot pass.
    let (s, k) = base.clone().unit(t[2], None, Unit::FighterI, 1, 1).build();
    assert!(reaching(&s, &k, 0, t[2], true).is_empty());
    // The destination holds an enemy cruiser: the Corsair moves through the fighter.
    let (s, k) = base.clone().unit(t[2], None, Unit::CruiserI, 1, 1).build();
    assert_eq!(origins(&reaching(&s, &k, 0, t[2], true)), vec![t[0]]);
    // The rules are per destination: for another active system the permission is gone.
    let r = SeatReach::new(&s, &k, Seat(0), true).unwrap();
    assert!(!r.rules_for(&s, &k, t[2].index_ix()).passing_ship_types.is_empty());
    assert!(r.rules_for(&s, &k, t[3].index_ix()).passing_ship_types.is_empty());
}

#[test]
fn the_aerie_hololattice_bars_the_passage_through_its_structures_but_not_the_arrival() {
    let t = plain4();
    let planet = t[2].data().planets[0];
    let duel = [Faction::TheArborec, Faction::TheArgentFlight];
    let base = row(t, &duel).unit(t[1], None, Unit::CruiserI, 0, 1).unit(t[2], Some(planet), Unit::PDSI, 1, 1).tech(1, Technology::AerieHololattice);
    let (mut s, k) = base.clone().build();
    // The string board has an entry for every system that holds a planet's units: the flag the kit does not set for a unit on a planet.
    s.board_mut().sys_mut(t[2]).flags.set(ti4_engine_core::state::SysFlags::TOUCHED, true);
    assert!(reaching(&s, &k, 0, t[3], true).is_empty(), "no passage through the structure's system");
    assert_eq!(origins(&reaching(&s, &k, 0, t[2], true)), vec![t[1]], "but the ship may stop there");
    assert!(SeatReach::new(&s, &k, Seat(0), true).unwrap().template().barred_transit.contains(t[2].index()));
    assert!(SeatReach::new(&s, &k, Seat(1), true).unwrap().template().barred_transit.is_empty(), "the owner is not barred");
    // Without the technology the structure is only a structure.
    s.players_mut().by_seat_mut(Seat(1)).unwrap().tech.owned.remove(Technology::AerieHololattice);
    assert!(SeatReach::new(&s, &k, Seat(0), true).unwrap().template().barred_transit.is_empty());
}

#[test]
fn the_ghosts_link_every_wormhole_system_to_each_other() {
    let alpha = System::AlphaWormhole;
    let beta = System::BetaWormhole;
    let p = plain_systems();
    // A, alpha, then far away: beta; the cruiser at alpha reaches beta in one step for the Ghosts, not for the others.
    let build = |factions: &[Faction]| {
        Scenario::new(factions)
            .map_hexes(&[(alpha, Hex::new(0, 0)), (p[0], Hex::new(1, 0)), (p[1], Hex::new(2, 0)), (p[2], Hex::new(3, 0)), (beta, Hex::new(4, 0))])
            .unit(alpha, None, Unit::CruiserI, 0, 1)
            .tokens(0, 3, 3, 2)
            .build()
    };
    let (s, k) = build(&[Faction::TheGhostsOfCreuss, Faction::TheArborec]);
    assert_eq!(origins(&reaching(&s, &k, 0, beta, true)), vec![alpha], "Quantum Entanglement");
    let (s, k) = build(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]);
    assert_eq!(origins(&reaching(&s, &k, 0, beta, true)), vec![alpha], "alpha and beta are adjacent to each other for everyone");
    let (s, k) = build(&[Faction::TheGhostsOfCreuss, Faction::TheArborec]);
    assert_eq!(origins(&reaching(&s, &k, 1, beta, true)), Vec::<System>::new());
}

// ---- the Fracture ---------------------------------------------------------------------------------

#[test]
fn the_fracture_tiles_are_chained_and_every_ingress_is_adjacent_to_the_egress_tiles() {
    let t = plain4();
    let (mut s, k) = row(t, &DUEL).build();
    let none = SeatReach::new(&s, &k, Seat(0), true).unwrap();
    assert!(none.template().extra_adjacency.is_empty());
    s.meta_mut().flags.set(MetaFlags::FRACTURE_IN_PLAY, true);
    s.explore_mut().ingress.insert(t[0]);
    let r = SeatReach::new(&s, &k, Seat(0), true).unwrap();
    let extra = &r.template().extra_adjacency;
    let links = |a: System| extra.iter().find(|(from, _)| *from == a.index_ix()).map(|(_, set)| set.iter().collect::<Vec<_>>()).unwrap_or_default();
    // The five tiles of the Fracture in a row: egress, void, Styx, void, egress.
    let fracture = [System::FractureEgressLeft, System::FractureVoidLeft, System::Styx, System::FractureVoidRight, System::FractureEgressRight];
    for tile in fracture {
        assert!(!links(tile).is_empty(), "{tile:?} is part of the chain");
    }
    // The ingress tile is linked with both egress tiles and with nothing else of the Fracture.
    let from_ingress = links(t[0]);
    assert!(from_ingress.contains(&System::FractureEgressLeft.index()) && from_ingress.contains(&System::FractureEgressRight.index()));
    assert!(!from_ingress.contains(&System::FractureVoidLeft.index()) && !from_ingress.contains(&System::FractureVoidRight.index()));
    assert!(links(System::FractureEgressLeft).contains(&t[0].index()));
    // The void tiles are not linked to the ingress.
    assert!(!links(System::FractureVoidLeft).contains(&t[0].index()));
    // The chain itself: each tile is next to its neighbours in the row, and to no other tile of the row.
    for (i, tile) in fracture.iter().enumerate() {
        let mut want: Vec<usize> = Vec::new();
        if i > 0 {
            want.push(fracture[i - 1].index());
        }
        if i + 1 < fracture.len() {
            want.push(fracture[i + 1].index());
        }
        let mut got: Vec<usize> = links(*tile).into_iter().filter(|x| fracture.iter().any(|f| f.index() == *x)).collect();
        got.sort_unstable();
        want.sort_unstable();
        assert_eq!(got, want, "{tile:?}");
    }
}

// ---- move values and boosts ---------------------------------------------------------------------

#[test]
fn the_ionian_fuel_refinery_needs_its_planet_ready_and_adds_one() {
    let p: Vec<System> = plain_systems().into_iter().filter(|s| *s != System::Tempesta).collect();
    let tempesta_sys = System::Tempesta;
    let t = [p[0], p[1], p[2], p[3]];
    // A row where the destination is one step further than a cruiser (move 2) goes; Tempesta is elsewhere on the map.
    let base = Scenario::new(&DUEL)
        .map_hexes(&[(t[0], Hex::new(0, 0)), (t[1], Hex::new(1, 0)), (t[2], Hex::new(2, 0)), (t[3], Hex::new(3, 0)), (tempesta_sys, Hex::new(0, 1))])
        .unit(t[0], None, Unit::CruiserI, 0, 1)
        .tokens(0, 3, 3, 2);
    let (s, k) = base.clone().build();
    assert!(reaching(&s, &k, 0, t[3], true).is_empty());
    let (mut s, k) = base.clone().control(Planet::Tempesta, 0).build();
    let found = reaching(&s, &k, 0, t[3], true);
    assert_eq!(found.len(), 1);
    assert!((found[0].ionian, found[0].gravity_drive) == (true, false));
    // Exhausted ability: not available.
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.legendary_exhausted.insert(Planet::Tempesta, ());
    assert!(reaching(&s, &k, 0, t[3], true).is_empty());
    // Someone else holds the planet: not ours.
    let (s, k) = base.clone().control(Planet::Tempesta, 1).build();
    assert!(reaching(&s, &k, 0, t[3], true).is_empty());
    // Both boosts together bring one more step, and the single boosts are tried first.
    let far = Scenario::new(&DUEL)
        .map_hexes(&[(t[0], Hex::new(0, 0)), (t[1], Hex::new(1, 0)), (t[2], Hex::new(2, 0)), (t[3], Hex::new(3, 0)), (System::MecatolRex18, Hex::new(4, 0)), (tempesta_sys, Hex::new(0, 1))])
        .unit(t[0], None, Unit::CruiserI, 0, 1)
        .control(Planet::Tempesta, 0)
        .tech(0, Technology::GravityDrive)
        .tokens(0, 3, 3, 2);
    let (s, k) = far.clone().build();
    let both = reaching(&s, &k, 0, System::MecatolRex18, true);
    assert_eq!(both.len(), 1);
    assert!(both[0].ionian && both[0].gravity_drive);
    let (s, k) = far.build();
    let one = reaching(&s, &k, 0, t[3], true);
    assert_eq!((one[0].gravity_drive, one[0].ionian), (true, false), "the Gravity Drive first");
}

#[test]
fn gravleash_maneuvers_anchors_the_move_of_ships_that_are_not_fighters() {
    let t = plain4();
    let (mut s, k) = row(t, &DUEL).unit(t[1], None, Unit::CruiserI, 0, 1).unit(t[1], None, Unit::FighterI, 0, 1).build();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.breakthrough = Some(Breakthrough::GravleashManeuvers);
    s.transient_mut().movement.gravleash_move_values.insert(t[1], 5);
    let v = |s: &CoreState, kind, index| reach::effective_move_value_for_ship(s, &k, Seat(0), t[1], kind, index, (false, false));
    assert_eq!(v(&s, Unit::CruiserI, 0), Some(5), "the larger of the printed move and the anchor");
    assert_eq!(v(&s, Unit::FighterI, 1), Some(i32::from(Unit::FighterI.data().movement.unwrap_or(0))), "a fighter is not anchored");
    assert_eq!(reach::effective_move_value(&s, &k, Seat(0), t[1], Unit::CruiserI, (false, false)), Some(5));
    // The anchor is a larger number only: a smaller one does not lower the move.
    s.transient_mut().movement.gravleash_move_values.insert(t[1], 1);
    assert_eq!(v(&s, Unit::CruiserI, 0), Some(2));
    // Without the breakthrough nothing is anchored; the anchor value itself ignores it.
    s.transient_mut().movement.gravleash_move_values.insert(t[1], 5);
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.breakthrough = None;
    assert_eq!(v(&s, Unit::CruiserI, 0), Some(2));
    assert_eq!(reach::anchor_move_value(&s, &k, Seat(0), t[1], Unit::CruiserI, 0), 2);
    let fleet = SeatFleet::new(&s, &k, Seat(0));
    assert_eq!(fleet.move_value(t[1], 0, (false, false)), Some(2));
}

#[test]
fn a_unit_type_outside_the_sources_in_play_has_no_move_value() {
    let t = plain4();
    let (s, mut k) = row(t, &DUEL).build();
    k.sources = ti4_engine_core::kernel::Sources::NONE;
    assert_eq!(reach::effective_move_value(&s, &k, Seat(0), t[0], Unit::CruiserI, (false, false)), None);
    assert_eq!(reach::effective_move_value_for_ship(&s, &k, Seat(0), t[0], Unit::CruiserI, 0, (false, false)), None);
    assert_eq!(reach::anchor_move_value(&s, &k, Seat(0), t[0], Unit::CruiserI, 0), 0);
    assert_eq!(reach::scope_of(&k).0, 0);
}

// ---- survivor-driven (mutation round 1) ------------------------------------------------------------------------------

#[test]
fn hil_colish_gives_its_system_a_delta_wormhole_that_joins_every_other_delta_system() {
    let gate = System::ALL.iter().copied().find(|s| s.data().wormholes.contains(&ti4_engine_core::generated::Wormhole::Delta)).expect("a tile with a delta wormhole");
    let p = plain_systems();
    let tiles = [gate, p[0], p[1], p[2]];
    let duel = DUEL;
    let with = row(tiles, &duel).unit(tiles[3], None, Unit::HilColish, 0, 1).unit(tiles[3], None, Unit::CruiserI, 0, 1);
    let (s, k) = with.build();
    let found = reaching(&s, &k, 0, gate, true);
    let kinds: Vec<Unit> = found.iter().map(|m| m.kind).collect();
    assert!(kinds.contains(&Unit::CruiserI), "three tiles away, one step by the delta wormhole: {kinds:?}");
    // Without the flagship the cruiser is three tiles away and out of reach.
    let (s2, k2) = row(tiles, &duel).unit(tiles[3], None, Unit::CruiserI, 0, 1).build();
    assert!(reaching(&s2, &k2, 0, gate, true).is_empty());
}

#[test]
fn aetherpassage_lets_the_named_mover_pass_the_ships_of_one_player_but_not_of_two() {
    let t = plain4();
    let three = [Faction::TheArborec, Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol];
    let sc = || row(t, &three).unit(t[1], None, Unit::CruiserI, 0, 1).unit(t[2], None, Unit::CruiserI, 1, 1);
    let mark = |s: &mut CoreState, stamp: u32, who: u8, owner: u8| {
        s.meta_mut().activation_seq = 6;
        s.marks_mut().tail.edit().insert(MarkKey::EmpyreanPassage(), MarkVal::Rec(vec![MarkVal::Atom(Atom::Num(stamp)), MarkVal::Atom(Atom::Seat(Seat(who))), MarkVal::Atom(Atom::Seat(Seat(owner)))]));
    };
    let (mut s, k) = sc().build();
    assert!(reaching(&s, &k, 0, t[3], true).is_empty(), "blocked by the rival's ship");
    mark(&mut s, 6, 0, 1);
    assert_eq!(origins(&reaching(&s, &k, 0, t[3], true)), vec![t[1]], "this activation, this mover, this owner");
    for (stamp, who, owner) in [(5, 0, 1), (6, 2, 1), (6, 0, 2)] {
        let (mut other, k2) = sc().build();
        mark(&mut other, stamp, who, owner);
        assert!(reaching(&other, &k2, 0, t[3], true).is_empty(), "{stamp} {who} {owner}");
    }
    // A system with the ships of the named player and of a third one still blocks.
    let (mut two, k3) = sc().unit(t[2], None, Unit::CruiserI, 2, 1).build();
    mark(&mut two, 6, 0, 1);
    assert!(reaching(&two, &k3, 0, t[3], true).is_empty());
    // Not a ship: ground forces of the named player never blocked.
    let (mut grounds, k4) = row(t, &three).unit(t[1], None, Unit::CruiserI, 0, 1).unit(t[2], Some(t[2].data().planets[0]), Unit::InfantryI, 1, 2).build();
    mark(&mut grounds, 6, 0, 1);
    assert_eq!(origins(&reaching(&grounds, &k4, 0, t[3], true)), vec![t[1]]);
}

#[test]
fn an_aetherstream_mark_and_the_flank_speed_card_add_one_to_the_move_and_to_the_anchor() {
    let t = plain4();
    let (mut s, k) = row(t, &DUEL).unit(t[1], None, Unit::CruiserI, 0, 1).build();
    s.meta_mut().activation_seq = 4;
    let printed = i32::from(Unit::CruiserI.data().movement.unwrap());
    let value = |s: &CoreState, seat: u8| reach::effective_move_value_for_ship(s, &k, Seat(seat), t[1], Unit::CruiserI, 0, (false, false)).unwrap();
    let anchor = |s: &CoreState, seat: u8| reach::anchor_move_value(s, &k, Seat(seat), t[1], Unit::CruiserI, 0);
    s.marks_mut().tail.edit().insert(MarkKey::EmpyreanAetherstream(), MarkVal::Rec(vec![MarkVal::Atom(Atom::Num(4)), MarkVal::Atom(Atom::Seat(Seat(0)))]));
    assert_eq!(value(&s, 0), printed + 1, "Aetherstream: +1 for the named player");
    assert_eq!(anchor(&s, 0), printed + 1);
    assert_eq!(value(&s, 1), printed);
    s.players_mut().by_seat_mut(Seat(0)).unwrap().card_timers.move_bonus_activation = Opt32::some(4);
    assert_eq!(value(&s, 0), printed + 2, "and the Flank Speed card");
    assert_eq!(anchor(&s, 0), printed + 2);
    // With both boosts and the card: 2 + the hook's 1 + the card's 1 + 2 boosts.
    assert_eq!(reach::effective_move_value_for_ship(&s, &k, Seat(0), t[1], Unit::CruiserI, 0, (true, true)), Some(printed + 4));
}
