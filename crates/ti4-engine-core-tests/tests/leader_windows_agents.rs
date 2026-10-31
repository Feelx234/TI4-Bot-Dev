//! D4A4: the leader windows `rules::leader_windows` that had no rule test: Gurno Aggero (Saar hero), Riftwalker Meian (Ghost hero), Nekro Malleon
//! (Nekro agent) and Letani Ospha (Arborec agent), against their printed text.

use ti4_engine_core::bits::SysSet;
use ti4_engine_core::choice::cards::{Ask as CAsk, Opt as CO, Reason};
use ti4_engine_core::choice::leaders::{Ask as LAsk, Opt as LOpt};
use ti4_engine_core::choice::strategy::Pool;
use ti4_engine_core::generated::{ActionCard, UnitBaseType};
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::rules::leader_windows::{ArmageddonRelay, LetaniOspha, NekroMalleon, RiftwalkerMeian, SwapFacts};
use ti4_engine_core::state::Code;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn l(o: LOpt) -> Opt {
    Opt::Leaders(o)
}

fn on_board(s: &CoreState, system: System, planet: Option<Planet>, seat: u8, unit: Unit) -> usize {
    let units = match planet {
        Some(p) => s.board().planet(p).units.as_slice(),
        None => s.board().sys(system).units.as_slice(),
    };
    units.iter().filter(|u| u.owner() == Seat(seat) && u.kind() == unit).count()
}

use ti4_engine_core::generated::Planet;

// ---- Gurno Aggero -----------------------------------------------------------------------------

fn armageddon_scene() -> Scenario {
    let s = plain_systems();
    let (home, near, far) = (s[0], s[1], s[10]);
    let dock = home.data().planets[0];
    let on_planet = near.data().planets[0];
    Scenario::new(&[Faction::TheClanOfSaar, Faction::TheArborec, Faction::TheYinBrotherhood])
        .map_ring(19)
        .control(dock, 0)
        .unit(home, Some(dock), Unit::SpaceDockI, 0, 1)
        .unit(near, None, Unit::FighterI, 1, 2)
        .unit(near, None, Unit::FighterI, 0, 1)
        .unit(near, None, Unit::CruiserI, 1, 1)
        .control(on_planet, 1)
        .unit(near, Some(on_planet), Unit::InfantryI, 1, 3)
        .unit(near, Some(on_planet), Unit::InfantryI, 0, 1)
        .unit(far, None, Unit::FighterI, 2, 1)
}

#[test]
fn the_saar_hero_destroys_all_other_players_infantry_and_fighters_in_a_system_next_to_one_of_its_docks() {
    covers!("rules::leader_windows::ArmageddonRelay");
    let s = plain_systems();
    let (home, near, far) = (s[0], s[1], s[10]);
    let planet = near.data().planets[0];
    let mut rig = Rig::new(armageddon_scene(), ArmageddonRelay::new(Seat(0), true));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Leaders(LAsk::SaarHero)));
    let offered = rig.opts();
    assert!(offered.contains(&l(LOpt::System(near))), "a neighbour of the dock: {offered:?}");
    assert!(!offered.contains(&l(LOpt::System(home))) || on_board(&rig.state, home, None, 1, Unit::FighterI) == 0, "it is the neighbours that are named");
    assert!(!offered.contains(&l(LOpt::System(far))), "a system not next to a dock is never offered");
    assert!(!offered.contains(&Opt::Decline), "the ACTION is committed: no decline");
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(l(LOpt::System(far))));
    rig.pick_opt(l(LOpt::System(near)));
    assert!(rig.is_done() && rig.driver.window.used);
    assert_eq!(on_board(&rig.state, near, None, 1, Unit::FighterI), 0, "other players' fighters die");
    assert_eq!(on_board(&rig.state, near, Some(planet), 1, Unit::InfantryI), 0, "other players' infantry die");
    assert_eq!(on_board(&rig.state, near, None, 1, Unit::CruiserI), 1, "but not their other ships");
    assert_eq!(on_board(&rig.state, near, None, 0, Unit::FighterI), 1, "nor the hero player's own");
    assert_eq!(on_board(&rig.state, near, Some(planet), 0, Unit::InfantryI), 1);
    assert_eq!(on_board(&rig.state, far, None, 2, Unit::FighterI), 1, "nor units in another system");
    smoke("leader_windows_agents/saar", || Rig::new(armageddon_scene(), ArmageddonRelay::new(Seat(0), true)));
}

#[test]
fn the_saar_hero_needs_a_map_and_a_dock_with_a_neighbour() {
    covers!("rules::leader_windows::ArmageddonRelay");
    let mut nomap = Rig::new(armageddon_scene(), ArmageddonRelay::new(Seat(0), false));
    nomap.run();
    assert!(nomap.is_done() && !nomap.driver.window.used);
    assert_eq!(delta(nomap.start_state(), &nomap.state), "");
    let nodock = Scenario::new(&[Faction::TheClanOfSaar, Faction::TheArborec]).map_ring(7).unit(plain_systems()[1], None, Unit::FighterI, 1, 1);
    let mut none = Rig::new(nodock, ArmageddonRelay::new(Seat(0), true));
    none.run();
    assert!(none.is_done() && !none.driver.window.used);
}

// ---- Riftwalker Meian -------------------------------------------------------------------------

fn everywhere() -> SwapFacts {
    let mut allowed = SysSet::EMPTY;
    for s in System::ALL {
        allowed.insert(s.index());
    }
    SwapFacts { allowed, wormholes: SysSet::EMPTY }
}

fn ghost_scene() -> Scenario {
    let s = plain_systems();
    Scenario::new(&[Faction::TheGhostsOfCreuss, Faction::TheArborec]).map_ring(7).unit(s[0], None, Unit::CruiserI, 0, 1).unit(s[2], None, Unit::DestroyerI, 0, 1).unit(s[3], None, Unit::DestroyerI, 0, 1).unit(s[4], None, Unit::DestroyerI, 1, 1)
}

#[test]
fn the_ghost_hero_names_two_systems_with_its_units_or_wormholes_and_records_the_swap() {
    covers!("rules::leader_windows::RiftwalkerMeian");
    let s = plain_systems();
    let mut rig = Rig::new(ghost_scene(), RiftwalkerMeian::new(Seat(0), true, everywhere()));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Leaders(LAsk::GhostSwapFirst)));
    let galaxy = rig.state.galaxy().expect("a map");
    let mut want = vec![s[0], s[2], s[3]];
    want.sort_by_key(|x| galaxy.coord_of(*x).expect("on the map"));
    let mut expected: Vec<Opt> = want.into_iter().map(|x| l(LOpt::System(x))).collect();
    expected.push(Opt::Decline);
    assert_eq!(rig.opts(), expected, "systems with the hero player's units in map (hex) order; another player's units do not count");
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(l(LOpt::System(s[4]))));
    rig.pick_opt(l(LOpt::System(s[2])));
    assert!(matches!(rig.asked().ask, Ask::Leaders(LAsk::GhostSwapSecond)));
    assert!(!rig.opts().contains(&l(LOpt::System(s[2]))), "the second system is another one");
    rig.expect_illegal(Pick::Opt(l(LOpt::System(s[2]))));
    rig.assert_options_sound();
    rig.pick_opt(l(LOpt::System(s[3])));
    assert!(rig.is_done() && rig.driver.window.used);
    assert_eq!(rig.state.marks().tail.iter().filter(|(k, _)| matches!(k, ti4_engine_core::keys::MarkKey::MapEdit(_))).count(), 1, "the swap waits for the game step as a map-edit mark");
    smoke("leader_windows_agents/ghost", || Rig::new(ghost_scene(), RiftwalkerMeian::new(Seat(0), true, everywhere())));
}

#[test]
fn the_ghost_hero_may_decline_either_question_and_needs_a_map_and_two_candidates() {
    covers!("rules::leader_windows::RiftwalkerMeian");
    let s = plain_systems();
    let mut first = Rig::new(ghost_scene(), RiftwalkerMeian::new(Seat(0), true, everywhere()));
    first.run().decline();
    assert!(first.is_done() && !first.driver.window.used);
    let mut second = Rig::new(ghost_scene(), RiftwalkerMeian::new(Seat(0), true, everywhere()));
    second.run().pick(0);
    second.decline();
    assert!(second.is_done() && !second.driver.window.used);
    assert_eq!(delta(second.start_state(), &second.state), "", "no swap is recorded");
    let mut nomap = Rig::new(ghost_scene(), RiftwalkerMeian::new(Seat(0), false, everywhere()));
    nomap.run();
    assert!(nomap.is_done());
    let one = Scenario::new(&[Faction::TheGhostsOfCreuss, Faction::TheArborec]).map_ring(7).unit(s[0], None, Unit::CruiserI, 0, 1);
    let mut single = Rig::new(one, RiftwalkerMeian::new(Seat(0), true, everywhere()));
    single.run();
    assert!(single.is_done(), "one candidate is not a swap");
    let mut barred = everywhere();
    barred.allowed.remove(s[0].index());
    barred.allowed.remove(s[2].index());
    let mut forbidden = Rig::new(ghost_scene(), RiftwalkerMeian::new(Seat(0), true, barred));
    forbidden.run();
    assert!(forbidden.is_done(), "the Creuss system and the Nexus cannot be named: with them out only one candidate is left");
    let mut wormholes = everywhere();
    wormholes.wormholes.insert(s[5].index());
    let mut wh = Rig::new(ghost_scene(), RiftwalkerMeian::new(Seat(0), true, wormholes));
    wh.run();
    assert!(wh.opts().contains(&l(LOpt::System(s[5]))), "a system with a wormhole qualifies without units");
}

// ---- Nekro Malleon ----------------------------------------------------------------------------

fn groups_table() -> AbilityTable {
    let mut t = AbilityTable::default();
    t.facts.card_groups = (0..ActionCard::COUNT).map(|i| u8::try_from(i).unwrap()).collect();
    t
}

fn malleon(sc: Scenario) -> Rig<NekroMalleon> {
    Rig::new(sc, NekroMalleon::new(Seat(0), Code(3))).with_table(groups_table())
}

fn three() -> Scenario {
    Scenario::new(&[Faction::TheNekroVirus, Faction::TheArborec, Faction::TheYinBrotherhood])
}

fn goods(s: &CoreState, seat: u8) -> i16 {
    s.players().by_seat(Seat(seat)).unwrap().econ.trade_goods
}

#[test]
fn nekro_malleon_asks_the_payer_for_a_card_a_token_or_nothing() {
    covers!("rules::leader_windows::NekroMalleon");
    let sc = || three().tokens(0, 0, 0, 0).tokens(1, 2, 0, 1).tokens(2, 0, 0, 0).hand(1, &[ActionCard::Rally, ActionCard::Spy]).goods(1, 1);
    let mut rig = malleon(sc());
    rig.run();
    // One payer (seat 1): the owner is not asked who.
    assert_eq!(rig.asked().actor, Seat(1), "the payer answers");
    assert!(matches!(rig.asked().ask, Ask::Leaders(LAsk::NekroPayment)));
    assert_eq!(rig.opts(), vec![l(LOpt::Discard), l(LOpt::Token(Pool::Tactic)), l(LOpt::Token(Pool::Strategic)), Opt::Decline], "a card, a token of a pool that holds one, or neither");
    rig.assert_options_sound();
    rig.pick_opt(l(LOpt::Discard));
    assert!(matches!(rig.asked().ask, Ask::Cards(CAsk::OwnHand { reason: Reason::MalleonDiscard })));
    assert_eq!(rig.opts().len(), 2);
    rig.assert_options_sound();
    rig.pick_opt(Opt::Cards(CO::Card(ActionCard::Spy)));
    assert!(rig.is_done() && rig.driver.window.used);
    assert_eq!(goods(&rig.state, 1), 3, "gain 2 trade goods");
    assert_eq!(rig.state.players().by_seat(Seat(1)).unwrap().cards.hand.as_slice(), &[ActionCard::Rally]);
    smoke("leader_windows_agents/malleon", || malleon(sc()));
    let mut tok = malleon(sc());
    tok.run().pick_opt(l(LOpt::Token(Pool::Tactic)));
    assert!(tok.is_done() && tok.driver.window.used);
    assert_eq!(tok.state.players().by_seat(Seat(1)).unwrap().econ.tactic_tokens, 1, "1 command token from the command sheet");
    assert_eq!(goods(&tok.state, 1), 3);
    let mut no = malleon(sc());
    no.run().decline();
    assert!(no.is_done() && no.driver.window.used, "the agent was used whatever the answer");
    assert_eq!(goods(&no.state, 1), 1);
}

#[test]
fn nekro_malleon_asks_the_owner_which_player_when_several_could_pay_and_takes_a_lone_card_unasked() {
    covers!("rules::leader_windows::NekroMalleon");
    let sc = || three().tokens(0, 0, 0, 0).tokens(1, 0, 0, 0).tokens(2, 0, 0, 0).hand(1, &[ActionCard::Rally]).hand(2, &[ActionCard::Spy]);
    let mut rig = malleon(sc());
    rig.run();
    assert_eq!(rig.asked().actor, Seat(0), "the owner of the agent chooses");
    assert_eq!(rig.opts(), vec![l(LOpt::Player(Seat(1))), l(LOpt::Player(Seat(2)))]);
    rig.expect_illegal(Pick::Opt(l(LOpt::Player(Seat(0)))));
    rig.pick(1);
    assert_eq!(rig.asked().actor, Seat(2));
    assert_eq!(rig.opts(), vec![l(LOpt::Discard), Opt::Decline], "no tokens, so a card or nothing");
    rig.pick_opt(l(LOpt::Discard));
    assert!(rig.is_done() && rig.driver.window.used, "a single card is discarded without a question");
    assert_eq!(goods(&rig.state, 2), 2);
    assert!(rig.state.players().by_seat(Seat(2)).unwrap().cards.hand.is_empty());
    let mut nobody = malleon(three().tokens(0, 0, 0, 0).tokens(1, 0, 0, 0).tokens(2, 0, 0, 0));
    nobody.run();
    assert!(nobody.is_done() && !nobody.driver.window.used, "nobody could pay: not used");
}

// ---- Letani Ospha -----------------------------------------------------------------------------

fn forms() -> Vec<(Seat, Unit)> {
    vec![(Seat(1), Unit::CarrierI), (Seat(1), Unit::CruiserI), (Seat(1), Unit::DestroyerI), (Seat(1), Unit::DreadnoughtI)]
}

fn ospha(sc: Scenario) -> Rig<LetaniOspha> {
    Rig::new(sc, LetaniOspha::new(Seat(0), forms()))
}

fn arborec() -> Scenario {
    Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_ring(7)
}

#[test]
fn letani_ospha_offers_the_ships_owner_a_replacement_costing_up_to_two_more() {
    covers!("rules::leader_windows::LetaniOspha");
    let s = plain_systems();
    let sc = || arborec().unit(s[1], None, Unit::DestroyerI, 1, 1);
    let mut rig = ospha(sc());
    rig.run();
    assert_eq!(rig.asked().actor, Seat(1), "the owner of the ship chooses; one ship qualifies, so the holder is not asked");
    assert!(matches!(rig.asked().ask, Ask::Leaders(LAsk::OsphaReplace { .. })));
    assert_eq!(rig.opts(), vec![l(LOpt::OsphaReplace(UnitBaseType::Carrier)), l(LOpt::OsphaReplace(UnitBaseType::Cruiser)), Opt::Decline], "a destroyer (1) -> up to 3: carrier (3), cruiser (2); not the dreadnought (4)");
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(l(LOpt::OsphaReplace(UnitBaseType::Dreadnought))));
    rig.pick_opt(l(LOpt::OsphaReplace(UnitBaseType::Carrier)));
    assert!(rig.is_done() && rig.driver.window.used);
    assert_eq!(on_board(&rig.state, s[1], None, 1, Unit::CarrierI), 1);
    assert_eq!(on_board(&rig.state, s[1], None, 1, Unit::DestroyerI), 0);
    smoke("leader_windows_agents/ospha", || ospha(sc()));
    let mut dec = ospha(sc());
    dec.run().decline();
    assert!(dec.is_done() && dec.driver.window.used, "used whenever a ship qualified");
    assert_eq!(on_board(&dec.state, s[1], None, 1, Unit::DestroyerI), 1);
}

#[test]
fn letani_ospha_asks_the_holder_which_ship_when_several_qualify_and_skips_fighters_and_empty_boxes() {
    covers!("rules::leader_windows::LetaniOspha");
    let s = plain_systems();
    let sc = || arborec().unit(s[1], None, Unit::DestroyerI, 1, 1).unit(s[2], None, Unit::CruiserI, 1, 1).unit(s[2], None, Unit::FighterI, 1, 3);
    let mut rig = ospha(sc());
    rig.run();
    assert_eq!(rig.asked().actor, Seat(0), "the holder of the agent picks the ship");
    assert!(matches!(rig.asked().ask, Ask::Leaders(LAsk::OsphaShip)));
    assert_eq!(rig.opts().len(), 2, "the destroyer and the cruiser; fighters are never offered");
    rig.assert_options_sound();
    rig.pick(1);
    assert_eq!(rig.asked().actor, Seat(1));
    assert!(rig.opts().contains(&l(LOpt::OsphaReplace(UnitBaseType::Dreadnought))), "a cruiser (2) -> up to 4: the dreadnought is in range");
    rig.pick_opt(l(LOpt::OsphaReplace(UnitBaseType::Dreadnought)));
    assert!(rig.is_done());
    assert_eq!(on_board(&rig.state, s[2], None, 1, Unit::DreadnoughtI), 1);
    assert_eq!(on_board(&rig.state, s[2], None, 1, Unit::FighterI), 3);
    let mut empty = ospha(arborec());
    empty.run();
    assert!(empty.is_done() && !empty.driver.window.used, "no ship on the board");
    let mut nobox = Rig::new(sc(), LetaniOspha::new(Seat(0), vec![]));
    nobox.run();
    assert!(nobox.is_done() && !nobox.driver.window.used, "the owner has no form to replace with");
}
