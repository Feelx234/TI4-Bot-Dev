//! `Area::Technology` (package W12): the free technology windows of a turn beside their core windows
//! (`rules::technology_windows`): Psychoarchaeology at the start of a turn (`technology::start_turn`), Predictive Intelligence and
//! Bio-Stims at its end (`technology::end_turn`).
//!
//! The core half answers `None` (the string result stands, counted as a bail) where the position holds more than the core windows:
//! a ready Transit Diodes or a Chaos Mapping at the start of a turn (a redeployment over the map's planets, a production nested
//! in the card), a question inside a battle or an invasion, diplomacy on, a nested emission.

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

#[cfg(feature = "typed-core")]
mod real {
    use ti4_engine_core::choice::technology::Ask as TAsk;
    use ti4_engine_core::choice::Ask;
    use ti4_engine_core::driver::Driver;
    use ti4_engine_core::generated::Technology;
    use ti4_engine_core::kernel::stage::{AbilityTable, Window};
    use ti4_engine_core::kernel::{Kernel, Resolver};
    use ti4_engine_core::rules::technology_windows::{Psychoarchaeology, TurnEnd};
    use ti4_engine_core::state::Slice;
    use ti4_model::id::PlayerId;
    use ti4_model::state::GameState;

    use crate::core_sync::{with_core, Footprint, Sources};
    use crate::dual::{CoreWindow, Run, Why};
    use crate::dual_window::{apply_staged, drive_with, Recording};

    /// What the windows read and write: the tokens, the trade good and the exhausted cards (Players), the planets (Board).
    const WINDOW: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Pending], &[Slice::Players, Slice::Board]);

    fn owned_ready(gs: &GameState, player: &PlayerId, alias: &str, ready_only: bool) -> bool {
        gs.player(player).is_some_and(|p| {
            p.technologies.contains(ti4_model::id::text(&alias)) && (!ready_only || !p.exhausted_technologies.contains(ti4_model::id::text(&alias)))
        })
    }

    fn drive_one<W: Window + Clone>(gs: &mut GameState, player: &PlayerId, script: &Recording, make: impl FnOnce(ti4_engine_core::ids::Seat, &crate::render::Render) -> W) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let letnev = gs.player(player)?.faction.as_str() == "letnev";
        let before = gs.clone();
        let kernel = Kernel::plain();
        let mut driver = Driver::new(make(seat, &r), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let mut staged = Vec::new();
        let driven = with_core(gs, &mut cx, Sources::default(), WINDOW, |core, _| {
            let driven = drive_with(&mut driver, core, &kernel, &r, &before, script, &|c, core, rendered| {
                if matches!(c.ask, Ask::Technology(TAsk::Restack)) {
                    rendered.details.extend(crate::strategy_cards_core::pool_details(core, c.actor, letnev, "restack", None));
                }
            });
            staged = std::mem::take(&mut core.pending_mut().staged);
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        Some(CoreWindow { run: Run::new(driven.finished).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    /// The leaf writes (`grant`, `purge`, `exhaust_technology_text`): the seat's technologies, the units of a purged upgrade.
    const WRITES: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board], &[Slice::Players, Slice::Board]);

    fn write<T>(gs: &mut GameState, player: &PlayerId, f: impl FnOnce(&mut ti4_engine_core::state::CoreState, ti4_engine_core::ids::Seat) -> Option<T>) -> Option<T> {
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        with_core(gs, &mut cx, Sources::default(), WRITES, |core, _| f(core, seat)).ok().flatten()
    }

    /// `technology::grant`.
    pub fn grant(gs: &mut GameState, player: &PlayerId, alias: &str) -> Option<()> {
        let tech = crate::core_adapter::typed::<Technology>(alias)?;
        write(gs, player, |core, seat| {
            ti4_engine_core::kernel::fx::grant_technology(core, seat, tech);
            Some(())
        })
    }

    /// `technology::purge`.
    pub fn purge(gs: &mut GameState, sources: ti4_model::content_types::SourceSet, player: &PlayerId, alias: &str) -> Option<bool> {
        let tech = crate::core_adapter::typed::<Technology>(alias)?;
        let scope = crate::core_adapter::scope(sources);
        write(gs, player, |core, seat| Some(ti4_engine_core::kernel::fx_strategy::purge_technology(core, ti4_engine_core::tables::Scope(u32::try_from(scope.0).ok()?), seat, tech)))
    }

    /// `technology::exhaust_technology_text`; a Nekro seat that does not own the card may carry its text on a Valefar Assimilator,
    /// which the string decides.
    pub fn exhaust_text(gs: &mut GameState, player: &PlayerId, alias: &str) -> Option<bool> {
        let tech = crate::core_adapter::typed::<Technology>(alias)?;
        write(gs, player, |core, seat| {
            let p = core.players().by_seat(seat)?;
            if !p.tech.owned.contains(tech) {
                if p.faction == ti4_engine_core::generated::Faction::TheNekroVirus {
                    return None;
                }
                return Some(false);
            }
            ti4_engine_core::kernel::fx_strategy::exhaust_technology(core, seat, tech);
            Some(true)
        })
    }

    /// The core half of `technology::start_turn`.
    pub fn start_turn(gs: &mut GameState, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
        if owned_ready(gs, player, "td", true) || owned_ready(gs, player, "cm", false) {
            crate::dual::note_bail_here("Transit Diodes or Chaos Mapping at the start of a turn (a redeployment over the map, a production nested in the card)");
            return None;
        }
        drive_one(gs, player, script, |seat, r| Psychoarchaeology::new(seat, r.code("psychoarchaeology")))
    }

    /// The core half of `technology::end_turn`.
    pub fn end_turn(gs: &mut GameState, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
        let _ = Technology::BioStims;
        drive_one(gs, player, script, |seat, _| TurnEnd::new(seat))
    }
}

#[cfg(not(feature = "typed-core"))]
mod stub {
    use ti4_model::id::PlayerId;
    use ti4_model::state::GameState;

    use crate::dual::CoreWindow;
    use crate::dual_window::Recording;

    pub fn start_turn(_: &mut GameState, _: &PlayerId, _: &Recording) -> Option<CoreWindow<bool>> {
        None
    }
    pub fn grant(_: &mut GameState, _: &PlayerId, _: &str) -> Option<()> {
        None
    }
    pub fn purge(_: &mut GameState, _: ti4_model::content_types::SourceSet, _: &PlayerId, _: &str) -> Option<bool> {
        None
    }
    pub fn exhaust_text(_: &mut GameState, _: &PlayerId, _: &str) -> Option<bool> {
        None
    }
    pub fn end_turn(_: &mut GameState, _: &PlayerId, _: &Recording) -> Option<CoreWindow<bool>> {
        None
    }
}

#[cfg(all(test, feature = "typed-core"))]
mod tests {
    use ti4_content::ContentStore;
    use ti4_model::content_types::POK;
    use ti4_model::id::{PlanetId, PlayerId, SystemId, TechnologyId};
    use ti4_model::state::GameState;

    use crate::choice::{Scripted, Table};
    use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
    use crate::generated::IdStr;

    fn a() -> PlayerId {
        PlayerId::new("a")
    }

    /// Specialty planets of the base game (a printed specialty), `(system, planet)` ids, in the order of the ids.
    fn specialty_planets(n: usize) -> Vec<(String, String)> {
        use ti4_engine_core::generated::{Planet, Source};
        Planet::ALL
            .iter()
            .filter(|p| !p.data().specialties.is_empty() && p.data().source == Source::Base)
            .filter_map(|p| p.data().system.map(|s| (s.as_str().to_owned(), p.as_str().to_owned())))
            .take(n)
            .collect()
    }

    fn game(techs: &[&str], planets: &[(String, String)]) -> GameState {
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], POK);
        crate::promissory::deal(&mut state, ContentStore::embedded(), POK);
        for t in techs {
            state.player_mut(&a()).unwrap().technologies.insert(TechnologyId::new(*t));
        }
        for (system, planet) in planets {
            state.system_mut(&SystemId::new(system.as_str())).set_control(PlanetId::new(planet.as_str()), a());
        }
        state
    }

    /// Run `body` under `technology=dual`; returns its value and the (compared, bailed) of a window label.
    fn lockstep<T>(label: &str, body: impl FnOnce() -> T) -> (T, (u64, u64), Vec<String>) {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        crate::dual::reset_thread_windows();
        let out = with_mask(AreaMask::STRING.with(Area::Technology, Backend::Dual), body);
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let counts = crate::dual::thread_window_counts(label);
        let bails = r.windows.keys().filter(|k| k.starts_with(label) && k.contains("[bail")).cloned().collect();
        let _ = take_report();
        (out, counts, bails)
    }

    fn run_start(state: &mut GameState, answers: &[&str]) {
        let mut table = Table::with_default(Box::new(Scripted::new(answers.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>())));
        crate::technology::start_turn(state, ContentStore::embedded(), POK, None, &mut table, &a()).expect("answered");
    }

    fn run_end(state: &mut GameState, answers: &[&str]) {
        let mut table = Table::with_default(Box::new(Scripted::new(answers.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>())));
        crate::technology::end_turn(state, ContentStore::embedded(), POK, None, &mut table, &a()).expect("answered");
    }

    #[test]
    fn psychoarchaeology_converts_each_ready_specialty_planet_until_the_owner_declines_in_lockstep() {
        let planets = specialty_planets(3);
        assert_eq!(planets.len(), 3);
        let ((state, goods), counts, bails) = lockstep("technology::start_turn", || {
            let mut state = game(&["pa"], &planets);
            let goods = state.player(&a()).unwrap().trade_goods;
            // Two conversions, then the third is declined.
            run_start(&mut state, &[&planets[0].1, &planets[2].1, "decline"]);
            (state, goods)
        });
        assert_eq!(counts, (1, 0), "the core followed it: {bails:?}");
        assert_eq!(state.player(&a()).unwrap().trade_goods, goods + 2);
        assert!(state.exhausted_planets.contains(&PlanetId::new(planets[0].1.as_str())));
        assert!(state.exhausted_planets.contains(&PlanetId::new(planets[2].1.as_str())));
        assert!(!state.exhausted_planets.contains(&PlanetId::new(planets[1].1.as_str())));
        // The last ready planet converted: nothing more is asked.
        let (_, counts, _) = lockstep("technology::start_turn", || {
            let mut state = game(&["pa"], &planets[..1]);
            run_start(&mut state, &[&planets[0].1]);
            assert_eq!(state.player(&a()).unwrap().trade_goods, game(&["pa"], &[]).player(&a()).unwrap().trade_goods + 1);
        });
        assert_eq!(counts, (1, 0));
    }

    #[test]
    fn a_turn_start_with_nothing_to_offer_asks_nothing_in_lockstep_and_transit_diodes_stay_the_strings() {
        let planets = specialty_planets(1);
        // No Psychoarchaeology: no question, the core window ends at once.
        let (_, counts, _) = lockstep("technology::start_turn", || run_start(&mut game(&[], &planets), &[]));
        assert_eq!(counts, (1, 0));
        // Psychoarchaeology but every specialty planet exhausted: no question either.
        let (_, counts, _) = lockstep("technology::start_turn", || {
            let mut state = game(&["pa"], &planets);
            state.exhausted_planets.insert(PlanetId::new(planets[0].1.as_str()));
            run_start(&mut state, &[]);
        });
        assert_eq!(counts, (1, 0));
        // A ready Transit Diodes (or a Chaos Mapping): the string's, counted with its reason.
        for tech in ["td", "cm"] {
            let (_, counts, bails) = lockstep("technology::start_turn", || run_start(&mut game(&[tech], &planets), &[]));
            assert_eq!(counts, (0, 1), "{tech}");
            assert!(bails.iter().any(|b| b.contains("Transit Diodes or Chaos Mapping")), "{tech}: {bails:?}");
        }
    }

    #[test]
    fn predictive_intelligence_moves_tokens_until_done_and_exhausts_the_card_in_lockstep() {
        let (state, counts, bails) = lockstep("technology::end_turn", || {
            let mut state = game(&["pi"], &[]);
            {
                let seat = state.player_mut(&a()).unwrap();
                (seat.tactic_tokens, seat.fleet_tokens, seat.strategic_tokens) = (3, 2, 1);
            }
            run_end(&mut state, &["tactic|fleet", "fleet|strategy", "done"]);
            state
        });
        assert_eq!(counts, (1, 0), "{bails:?}");
        let seat = state.player(&a()).unwrap();
        assert_eq!((seat.tactic_tokens, seat.fleet_tokens, seat.strategic_tokens), (2, 2, 2));
        assert!(seat.exhausted_technologies.contains(&TechnologyId::new("pi")), "something moved: the card is spent");
        // Declined at once: nothing moved, the card stays ready.
        let (state, counts, _) = lockstep("technology::end_turn", || {
            let mut state = game(&["pi"], &[]);
            run_end(&mut state, &["done"]);
            state
        });
        assert_eq!(counts, (1, 0));
        assert!(!state.player(&a()).unwrap().exhausted_technologies.contains(&TechnologyId::new("pi")));
        // No tokens at all: nothing to move, nothing asked.
        let (_, counts, _) = lockstep("technology::end_turn", || {
            let mut state = game(&["pi"], &[]);
            {
                let seat = state.player_mut(&a()).unwrap();
                (seat.tactic_tokens, seat.fleet_tokens, seat.strategic_tokens) = (0, 0, 0);
            }
            run_end(&mut state, &[]);
        });
        assert_eq!(counts, (1, 0));
    }

    #[test]
    fn the_tokens_a_predictive_intelligence_may_move_are_bounded_by_what_the_owner_holds_in_lockstep() {
        let (state, counts, bails) = lockstep("technology::end_turn", || {
            let mut state = game(&["pi"], &[]);
            {
                let seat = state.player_mut(&a()).unwrap();
                (seat.tactic_tokens, seat.fleet_tokens, seat.strategic_tokens) = (1, 0, 0);
            }
            // One token held: one move, and then the window ends by itself (no `done` is asked).
            run_end(&mut state, &["tactic|strategy"]);
            state
        });
        assert_eq!(counts, (1, 0), "{bails:?}");
        let seat = state.player(&a()).unwrap();
        assert_eq!((seat.tactic_tokens, seat.strategic_tokens), (0, 1));
    }

    #[test]
    fn bio_stims_readies_a_planet_or_a_technology_and_exhausts_itself_in_lockstep() {
        let planets = specialty_planets(2);
        for (answer, check) in [(format!("ready|planet|{}", planets[0].1), 0), ("ready|technology|gd".to_owned(), 1), ("decline".to_owned(), 2)] {
            let (state, counts, bails) = lockstep("technology::end_turn", || {
                let mut state = game(&["bs", "gd"], &planets);
                state.exhausted_planets.insert(PlanetId::new(planets[0].1.as_str()));
                state.exhausted_planets.insert(PlanetId::new(planets[1].1.as_str()));
                state.player_mut(&a()).unwrap().exhausted_technologies.insert(TechnologyId::new("gd"));
                run_end(&mut state, &[&answer]);
                state
            });
            assert_eq!(counts, (1, 0), "{answer}: {bails:?}");
            let seat = state.player(&a()).unwrap();
            match check {
                0 => {
                    assert!(!state.exhausted_planets.contains(&PlanetId::new(planets[0].1.as_str())));
                    assert!(seat.exhausted_technologies.contains(&TechnologyId::new("bs")));
                }
                1 => {
                    assert!(!seat.exhausted_technologies.contains(&TechnologyId::new("gd")));
                    assert!(seat.exhausted_technologies.contains(&TechnologyId::new("bs")));
                }
                _ => assert!(!seat.exhausted_technologies.contains(&TechnologyId::new("bs")), "declined: the card stays ready"),
            }
        }
        // Nothing exhausted: no question.
        let (_, counts, _) = lockstep("technology::end_turn", || run_end(&mut game(&["bs"], &planets), &[]));
        assert_eq!(counts, (1, 0));
    }

    #[test]
    fn predictive_intelligence_then_bio_stims_can_ready_the_card_just_spent() {
        let (state, counts, bails) = lockstep("technology::end_turn", || {
            let mut state = game(&["pi", "bs"], &[]);
            run_end(&mut state, &["tactic|fleet", "done", "ready|technology|pi"]);
            state
        });
        assert_eq!(counts, (1, 0), "{bails:?}");
        let seat = state.player(&a()).unwrap();
        assert!(!seat.exhausted_technologies.contains(&TechnologyId::new("pi")), "Bio-Stims readied it");
        assert!(seat.exhausted_technologies.contains(&TechnologyId::new("bs")));
    }

    #[test]
    fn grant_exhaust_and_purge_write_the_same_position_on_both_sides() {
        let (system, planet) = crate::fixtures::a_placed_planet();
        let (state, counts, _) = lockstep("technology::none", || {
            let mut state = game(&[], &[]);
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &a(), 2);
            let mut by_core = state.clone();
            // Granting, then the upgrade reaches the units (string `apply_unit_upgrades`), then exhausting and purging.
            assert_eq!(super::grant(&mut by_core, &a(), "inf2"), Some(()));
            crate::technology::grant(&mut state, &a(), &TechnologyId::new("inf2"));
            assert_eq!(by_core.player(&a()).unwrap().technologies, state.player(&a()).unwrap().technologies);
            crate::technology::apply_unit_upgrades(&mut state, ContentStore::embedded(), POK, &a());
            crate::technology::apply_unit_upgrades(&mut by_core, ContentStore::embedded(), POK, &a());
            assert_eq!(by_core, state);
            let upgraded = state.system_state(&system).planet_units.get(&PlanetId::new(planet.as_str())).unwrap()[0].type_id.to_string();
            assert!(upgraded.contains('2') || upgraded != "infantry", "the units took the upgrade: {upgraded}");
            // Exhaust: owned, then not owned (Hacan has no Magen).
            let mut core_exhaust = state.clone();
            assert_eq!(super::exhaust_text(&mut core_exhaust, &a(), "inf2"), Some(true));
            assert!(crate::technology::exhaust_technology_text(&mut state, &a(), "inf2"));
            assert_eq!(core_exhaust, state);
            let mut not_owned = state.clone();
            assert_eq!(super::exhaust_text(&mut not_owned, &a(), "pi"), Some(false));
            assert!(!crate::technology::exhaust_technology_text(&mut state, &a(), "pi"));
            assert_eq!(not_owned, state);
            // Purge: the upgrade leaves with its unit form; a card the seat does not own is refused with nothing written.
            let mut purged = state.clone();
            assert_eq!(super::purge(&mut purged, POK, &a(), "inf2"), Some(true));
            assert!(crate::technology::purge(&mut state, ContentStore::embedded(), POK, &a(), &TechnologyId::new("inf2")));
            assert_eq!(purged, state, "the units went back to the form the card replaced");
            assert!(!state.player(&a()).unwrap().exhausted_technologies.contains(&TechnologyId::new("inf2")));
            let mut again = state.clone();
            assert_eq!(super::purge(&mut again, POK, &a(), "inf2"), Some(false));
            assert!(!crate::technology::purge(&mut state, ContentStore::embedded(), POK, &a(), &TechnologyId::new("inf2")));
            assert_eq!(again, state);
            state
        });
        let _ = (state, counts);
    }

    #[test]
    fn a_nekro_seat_that_does_not_own_the_card_may_carry_its_text_on_a_valefar_assimilator_so_the_string_decides() {
        let mut state = crate::fixtures::seated_game(&[("a", "nekro"), ("b", "hacan")], POK);
        crate::promissory::deal(&mut state, ContentStore::embedded(), POK);
        let mut probe = state.clone();
        assert_eq!(super::exhaust_text(&mut probe, &a(), "pi"), None, "the core does not decide for a Nekro");
        assert_eq!(probe, state, "and writes nothing");
        // Any other faction: a card it does not own is simply not exhausted.
        let mut other = game(&[], &[]);
        assert_eq!(super::exhaust_text(&mut other, &a(), "pi"), Some(false));
    }
}
