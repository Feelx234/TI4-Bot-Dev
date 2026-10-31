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
    use ti4_engine_core::rules::technology_windows::{ProductionBiomes, ProductionUsed, TurnEnd, TurnStart};
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
        drive_valued(gs, player, script, make, |_, finished| finished)
    }

    /// [`drive_one`] with the value read from the window (and whether it finished).
    fn drive_valued<W: Window + Clone>(
        gs: &mut GameState,
        player: &PlayerId,
        script: &Recording,
        make: impl FnOnce(ti4_engine_core::ids::Seat, &crate::render::Render) -> W,
        value: impl FnOnce(&W, bool) -> bool,
    ) -> Option<CoreWindow<bool>> {
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
        Some(CoreWindow { run: Run::new(value(&driver.window, driven.finished)).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    /// The core half of Production Biomes (`faction_abilities::production_biomes`; the value: whether the action resolved). The card
    /// must be the player's own: a Nekro seat may carry its text on a Valefar Assimilator, which the string decides.
    pub fn production_biomes(gs: &mut GameState, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
        drive_valued(gs, player, script, |seat, r| ProductionBiomes::new(seat, r.code("production_biomes")), |w, _| w.done)
    }

    /// The core half of Cabal's Vortex (`factions::cabal::vortex_perform`; the value: the action resolved). The card must be the player's
    /// own (a Nekro seat may carry its text on a Valefar Assimilator, which the string decides). `has_map` says the string arm had a map
    /// (its neighbours are the map's plain adjacency, which the lent map carries).
    pub fn vortex(gs: &mut GameState, player: &PlayerId, galaxy: Option<&ti4_content::galaxy::Galaxy>, sources: ti4_model::content_types::SourceSet, script: &Recording) -> Option<CoreWindow<bool>> {
        use ti4_engine_core::rules::technology_windows::Vortex;
        // (Only a Cabal seat uses Vortex, and only with the card itself: a Valefar Assimilator never carries it in the string.)
        if !owned_ready(gs, player, "vtx", false) {
            crate::dual::note_bail_here("Vortex carried by something other than the player's own card (a Valefar Assimilator)");
            return None;
        }
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let scope = ti4_engine_core::tables::Scope(u32::try_from(crate::core_adapter::scope(sources).0).ok()?);
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let neutral = r.seat(crate::neutral_units::NEUTRAL);
        let has_map = galaxy.is_some();
        let before = gs.clone();
        let kernel = Kernel::plain();
        let mut driver = Driver::new(Vortex::new(seat, has_map, neutral, scope), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let footprint = if has_map {
            Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Map, Slice::Marks, Slice::Pending], &[Slice::Players])
        } else {
            Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Marks, Slice::Pending], &[Slice::Players])
        };
        let driven = with_core(gs, &mut cx, Sources { galaxy, rng: None }, footprint, |core, _| {
            if has_map && core.galaxy().is_none() {
                return None;
            }
            Some(drive_with(&mut driver, core, &kernel, &r, &before, script, &|_, _, _| {}))
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        Some(CoreWindow { run: Run::new(driver.window.done).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    /// What the Wormhole Generator reads (the card, the tokens) and writes (the card, the token).
    const WORMHOLE: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Explore, Slice::Pending], &[Slice::Players, Slice::Explore]);

    /// The core half of Ghost's Wormhole Generator (`factions::ghost::perform_component`; the value: the action resolved). `destinations`
    /// are the string's read of the map (`tokens::wormhole_generator_destinations`); the card must be the player's own.
    pub fn wormhole_generator(gs: &mut GameState, player: &PlayerId, destinations: &[(&str, ti4_model::id::SystemId)], script: &Recording) -> Option<CoreWindow<bool>> {
        use ti4_engine_core::rules::technology_windows::WormholeGenerator;
        if !owned_ready(gs, player, "wg", false) {
            crate::dual::note_bail_here("Wormhole Generator carried by something other than the player's own card");
            return None;
        }
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let destinations: Vec<(ti4_engine_core::generated::Wormhole, ti4_engine_core::generated::System)> = destinations
            .iter()
            .map(|(kind, system)| Some((crate::core_adapter::typed(kind)?, crate::core_adapter::typed(system.as_str())?)))
            .collect::<Option<_>>()
            .why(crate::dual::reason::UNKNOWN_ID)?;
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let before = gs.clone();
        let kernel = Kernel::plain();
        let mut driver = Driver::new(WormholeGenerator::new(seat, destinations), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = with_core(gs, &mut cx, Sources::default(), WORMHOLE, |core, _| drive_with(&mut driver, core, &kernel, &r, &before, script, &|_, _, _| {}))
            .why(crate::dual::reason::NOT_LENDABLE)?
            .why(crate::dual::reason::DRIVER_REFUSED)?;
        Some(CoreWindow { run: Run::new(driver.window.done).with_events_vec(Vec::new()), questions: driven.rendered })
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
            // The card that carries the text: the technology, or the Valefar Assimilator whose token sits on it.
            Some(ti4_engine_core::kernel::fx_strategy::exhaust_technology_text(core, seat, tech))
        })
    }

    /// The core half of `technology::start_turn`. Chaos Mapping's single unit is a child window of the core (`Spawn::ProduceOne`); the
    /// adapter hands the window the systems with a PRODUCTION capacity and the facts of the unit. A seat that carries the card on a
    /// Valefar Assimilator, a table the single-unit production does not carry and a unit that ends over the fleet supply or the capacity
    /// of its system are the string's.
    pub fn start_turn(gs: &mut GameState, content: &ti4_content::ContentStore, sources: ti4_model::content_types::SourceSet, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
        use ti4_engine_core::rules::technology_windows::ChaosFacts;
        let has_text = crate::technology::has_technology_text(gs, player, "cm");
        if !has_text {
            return drive_one(gs, player, script, |seat, r| TurnStart::new(seat, r.code("psychoarchaeology")));
        }
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        if crate::production_view::unported_triggered(gs, player) {
            crate::dual::note_bail_here("a table whose seats the single-unit production does not carry (Keleres, Deepwrought)");
            return None;
        }
        let systems: Vec<ti4_engine_core::generated::System> = gs
            .board
            .keys()
            .filter(|system| crate::production::capacity(gs, content, sources, player, system) > 0)
            .map(|system| crate::core_adapter::typed::<ti4_engine_core::generated::System>(system.as_str()))
            .collect::<Option<_>>()
            .why(crate::dual::reason::UNKNOWN_ID)?;
        let buildable = crate::production_window_dual::buildable_for_window(gs, content, sources, player)?;
        let fleet_limit = crate::production_window_dual::enforcement_limit(gs, content, player)?;
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let facts = ChaosFacts { systems, one: ti4_engine_core::kernel::children::OneFacts { buildable, fleet_limit, name: None, alias: None } };
        let before = gs.clone();
        let kernel = crate::production_view::kernel();
        let mut table = AbilityTable::default();
        ti4_engine_core::driver::children::register(&mut table);
        let mut driver = Driver::new(TurnStart::new(seat, r.code("psychoarchaeology")).with_chaos_mapping(facts), Resolver::new(&[seat], Some(seat)), table, 1);
        let footprint = Footprint {
            reads: crate::production_view::READS.union(ti4_engine_core::state::SliceSet::of(&[Slice::Pending])),
            writes: ti4_engine_core::state::SliceSet::of(&[Slice::Board, Slice::Players, Slice::Marks, Slice::Pending]),
        };
        let mut staged = Vec::new();
        let driven = with_core(gs, &mut cx, Sources::default(), footprint, |core, _| {
            let driven = crate::dual_window::drive_positioned(&mut driver, core, kernel, Some(kernel), &r, &before, script, &|_, _, _| {});
            staged = std::mem::take(&mut core.pending_mut().staged);
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        if driver.window.unsupported() {
            crate::dual::note_bail_here("Chaos Mapping's unit stands over the fleet supply or the capacity of its system (the string asks which units go)");
            return None;
        }
        apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        Some(CoreWindow { run: Run::new(driven.finished).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    /// The core half of `technology::production_used`.
    pub fn production_used(gs: &mut GameState, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
        const WRITES_TRANSIENT: Footprint =
            Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Transient, Slice::Pending], &[Slice::Players, Slice::Transient]);
        let _ = WINDOW;
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let before = gs.clone();
        let kernel = Kernel::plain();
        let mut driver = Driver::new(ProductionUsed::new(seat), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = with_core(gs, &mut cx, Sources::default(), WRITES_TRANSIENT, |core, _| drive_with(&mut driver, core, &kernel, &r, &before, script, &|_, _, _| {}))
            .why(crate::dual::reason::NOT_LENDABLE)?
            .why(crate::dual::reason::DRIVER_REFUSED)?;
        Some(CoreWindow { run: Run::new(driven.finished).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    /// The core half of `technology::self_assembly_after_production`.
    pub fn self_assembly(gs: &mut GameState, player: &PlayerId, system: &ti4_model::id::SystemId, sources: ti4_model::content_types::SourceSet, script: &Recording) -> Option<CoreWindow<bool>> {
        const SAR: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Marks, Slice::Cards, Slice::Pending], &[Slice::Players, Slice::Board]);
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let system = crate::core_adapter::typed::<ti4_engine_core::generated::System>(system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let before = gs.clone();
        let mut kernel = Kernel::plain();
        kernel.sources = ti4_engine_core::kernel::Sources(u8::try_from(crate::core_adapter::scope(sources).0).ok()?);
        let mut driver = Driver::new(ti4_engine_core_factions::self_assembly::SelfAssembly::new(seat, system), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = with_core(gs, &mut cx, Sources::default(), SAR, |core, _| drive_with(&mut driver, core, &kernel, &r, &before, script, &|_, _, _| {}))
            .why(crate::dual::reason::NOT_LENDABLE)?
            .why(crate::dual::reason::DRIVER_REFUSED)?;
        Some(CoreWindow { run: Run::new(driven.finished).with_events_vec(Vec::new()), questions: driven.rendered })
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

    pub fn start_turn(_: &mut GameState, _: &ti4_content::ContentStore, _: ti4_model::content_types::SourceSet, _: &PlayerId, _: &Recording) -> Option<CoreWindow<bool>> {
        None
    }
    pub fn production_biomes(_: &mut GameState, _: &PlayerId, _: &Recording) -> Option<CoreWindow<bool>> {
        None
    }
    pub fn wormhole_generator(_: &mut GameState, _: &PlayerId, _: &[(&str, ti4_model::id::SystemId)], _: &Recording) -> Option<CoreWindow<bool>> {
        None
    }
    pub fn vortex(
        _: &mut GameState,
        _: &PlayerId,
        _: Option<&ti4_content::galaxy::Galaxy>,
        _: ti4_model::content_types::SourceSet,
        _: &Recording,
    ) -> Option<CoreWindow<bool>> {
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
    pub fn production_used(_: &mut GameState, _: &PlayerId, _: &Recording) -> Option<CoreWindow<bool>> {
        None
    }
    pub fn self_assembly(_: &mut GameState, _: &PlayerId, _: &ti4_model::id::SystemId, _: ti4_model::content_types::SourceSet, _: &Recording) -> Option<CoreWindow<bool>> {
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
    fn a_turn_start_with_nothing_to_offer_asks_nothing_in_lockstep() {
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
        // A Chaos Mapping with no system that can produce: the core window asks nothing either.
        let (_, counts, bails) = lockstep("technology::start_turn", || run_start(&mut game(&["cm"], &planets), &[]));
        assert_eq!(counts, (1, 0), "{bails:?}");
    }

    /// Chaos Mapping (package W12, lane 7): the choice of the system, then one unit as a child window of the core (`ProduceOne`). The
    /// core half agrees with the string for a destroyer paid with a trade good, a decline of the system and of the unit, and it ran each time.
    #[test]
    fn chaos_mapping_chooses_a_system_and_produces_one_unit_in_lockstep() {
        let planets = specialty_planets(1);
        let (system, planet) = (SystemId::new(planets[0].0.as_str()), PlanetId::new(planets[0].1.as_str()));
        let make = || {
            let mut state = game(&["cm"], &planets);
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "spacedock", &a(), 1);
            state.player_mut(&a()).unwrap().trade_goods = 1;
            state.exhausted_planets.insert(planet.clone());
            state
        };
        let built = |state: &GameState| state.system_state(&system).units_of(&a()).iter().any(|unit| unit.type_id.as_str() == "destroyer");
        let (state, counts, bails) = lockstep("technology::start_turn", || {
            let mut state = make();
            run_start(&mut state, &[system.as_str(), "build|destroyer|1", "trade_good"]);
            state
        });
        assert_eq!(counts, (1, 0), "{bails:?}");
        assert!(built(&state));
        let (state, counts, bails) = lockstep("technology::start_turn", || {
            let mut state = make();
            run_start(&mut state, &["decline"]);
            state
        });
        assert_eq!(counts, (1, 0), "{bails:?}");
        assert!(!built(&state));
        let (state, counts, bails) = lockstep("technology::start_turn", || {
            let mut state = make();
            run_start(&mut state, &[system.as_str(), "decline"]);
            state
        });
        assert_eq!(counts, (1, 0), "{bails:?}");
        assert!(!built(&state));
    }

    /// Transit Diodes (package W12): ground forces move to planets the seat controls, up to four times, a planet that received one
    /// cannot send one out; the card is spent when anything moved. The options are compared whole (order, ids, labels).
    #[test]
    fn transit_diodes_redeploy_ground_forces_and_exhaust_the_card_in_lockstep() {
        let planets = specialty_planets(2);
        let (system_a, planet_a) = (SystemId::new(planets[0].0.as_str()), PlanetId::new(planets[0].1.as_str()));
        let (system_b, planet_b) = (SystemId::new(planets[1].0.as_str()), PlanetId::new(planets[1].1.as_str()));
        let make = || {
            let mut state = game(&["td"], &planets);
            crate::fixtures::put_on_planet(&mut state, &system_a, &planet_a, "infantry", &a(), 2);
            crate::fixtures::put(&mut state, &system_a, "infantry", &a(), 1);
            state
        };
        let id = |from: &str, unit: &str| format!("transit|{system_a}|{from}|{unit}|{system_b}|{planet_b}");
        // One move from a planet, one from the space area, then decline: the card is spent.
        let (state, counts, bails) = lockstep("technology::start_turn", || {
            let mut state = make();
            run_start(&mut state, &[&id(planet_a.as_str(), "infantry"), &id("space", "infantry"), "decline"]);
            state
        });
        assert_eq!(counts, (1, 0), "{bails:?}");
        let there = state.system_state(&system_b).planet_units.get(&planet_b).map_or(0, Vec::len);
        assert_eq!(there, 2);
        assert!(state.player(&a()).unwrap().exhausted_technologies.contains(&TechnologyId::new("td")));
        // Declined at once: nothing moved, the card stays ready.
        let (state, counts, _) = lockstep("technology::start_turn", || {
            let mut state = make();
            run_start(&mut state, &["decline"]);
            state
        });
        assert_eq!(counts, (1, 0));
        assert!(!state.player(&a()).unwrap().exhausted_technologies.contains(&TechnologyId::new("td")));
        // No ground force anywhere: nothing is asked.
        let (_, counts, _) = lockstep("technology::start_turn", || run_start(&mut game(&["td"], &planets), &[]));
        assert_eq!(counts, (1, 0));
    }

    /// Found by the end-of-D1 gate run (`dual_run ... all=dual`, golden game 16): the options of Transit Diodes list the ground
    /// forces of a system by planet, in the string's order (the planet id text), not in the order the map names the planets.
    #[test]
    fn transit_diodes_list_the_planets_of_a_system_in_the_order_of_their_ids_in_lockstep() {
        use ti4_engine_core::generated::{Planet, System};
        let system = System::ALL
            .iter()
            .copied()
            .find(|sys| {
                let p = sys.data().planets;
                p.len() >= 2 && p.windows(2).any(|w| w[0].as_str() > w[1].as_str()) && p.iter().all(|x| x.data().source == ti4_engine_core::generated::Source::Base)
            })
            .expect("a base system whose planets are not named in id order");
        let there: Vec<Planet> = system.data().planets.to_vec();
        let destination = specialty_planets(8).into_iter().find(|(sys, _)| sys != system.as_str()).expect("a planet elsewhere");
        let system_id = SystemId::new(system.as_str());
        let (state, counts, bails) = lockstep("technology::start_turn", || {
            let mut state = game(&["td"], std::slice::from_ref(&destination));
            for planet in &there {
                crate::fixtures::put_on_planet(&mut state, &system_id, &PlanetId::new(planet.as_str()), "infantry", &a(), 1);
            }
            run_start(&mut state, &["decline"]);
            state
        });
        assert_eq!(counts, (1, 0), "{bails:?}");
        let _ = state;
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
    fn a_nekro_seat_that_does_not_own_the_card_may_carry_its_text_on_a_valefar_assimilator_and_the_core_exhausts_the_carrier() {
        let mut state = crate::fixtures::seated_game(&[("a", "nekro"), ("b", "hacan")], POK);
        crate::promissory::deal(&mut state, ContentStore::embedded(), POK);
        // No assimilator carries it: nothing is exhausted (the string says false too).
        let mut probe = state.clone();
        assert_eq!(super::exhaust_text(&mut probe, &a(), "pi"), Some(false));
        assert_eq!(probe, state, "and writes nothing");
        // The assimilator X stands for it while another seat owns it (package W10): the carrier is the card exhausted.
        let mut carried = state.clone();
        carried.player_mut(&PlayerId::new("b")).unwrap().technologies.insert(TechnologyId::new("pi"));
        carried.player_mut(&a()).unwrap().technologies.insert(TechnologyId::new("vax"));
        carried.player_mut(&a()).unwrap().assimilated_technologies.insert("vax".to_owned(), TechnologyId::new("pi"));
        assert_eq!(super::exhaust_text(&mut carried, &a(), "pi"), Some(true));
        let exhausted = &carried.player(&a()).unwrap().exhausted_technologies;
        assert!(exhausted.contains(ti4_model::id::text(&"vax")) && !exhausted.contains(ti4_model::id::text(&"pi")));
        // Any other faction: a card it does not own is simply not exhausted.
        let mut other = game(&[], &[]);
        assert_eq!(super::exhaust_text(&mut other, &a(), "pi"), Some(false));
    }

    fn run_production_used(state: &mut GameState, answers: &[&str]) {
        let mut table = Table::with_default(Box::new(Scripted::new(answers.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>())));
        crate::technology::production_used(state, ContentStore::embedded(), POK, None, &mut table, &a()).expect("answered");
    }

    #[test]
    fn sarween_tools_and_ai_development_algorithm_set_the_discount_of_a_use_in_lockstep() {
        let label = "technology::production_used";
        // Nothing owned: the field is written zero (a value left over from an earlier use is overwritten).
        let (state, counts, bails) = lockstep(label, || {
            let mut state = game(&[], &[]);
            state.production_discount_remaining = 5;
            run_production_used(&mut state, &[]);
            state
        });
        assert_eq!((counts, state.production_discount_remaining), ((1, 0), 0), "{bails:?}");
        // Sarween alone: a flat 1, no question.
        let (state, counts, _) = lockstep(label, || {
            let mut state = game(&["st"], &[]);
            run_production_used(&mut state, &[]);
            state
        });
        assert_eq!((counts, state.production_discount_remaining), ((1, 0), 1));
        // Both and two unit upgrades: exhaust for 1 + 2; or decline for 1; the card is spent only when used.
        for (answer, discount, spent) in [("exhaust", 3, true), ("decline", 1, false)] {
            let (state, counts, bails) = lockstep(label, || {
                let mut state = game(&["st", "aida", "inf2", "ff2"], &[]);
                run_production_used(&mut state, &[answer]);
                state
            });
            assert_eq!(counts, (1, 0), "{answer}: {bails:?}");
            assert_eq!(state.production_discount_remaining, discount, "{answer}");
            assert_eq!(state.player(&a()).unwrap().exhausted_technologies.contains(&TechnologyId::new("aida")), spent, "{answer}");
        }
        // No unit upgrade owned, or the card already spent: no question.
        for (techs, exhausted) in [(&["aida"][..], false), (&["aida", "inf2"][..], true)] {
            let (state, counts, _) = lockstep(label, || {
                let mut state = game(techs, &[]);
                if exhausted {
                    state.player_mut(&a()).unwrap().exhausted_technologies.insert(TechnologyId::new("aida"));
                }
                run_production_used(&mut state, &[]);
                state
            });
            assert_eq!((counts, state.production_discount_remaining), ((1, 0), 0));
        }
    }

    fn run_sar(state: &mut GameState, system: &SystemId, answers: &[&str]) {
        let mut table = Table::with_default(Box::new(Scripted::new(answers.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>())));
        let (mut dice, mut rng) = (crate::dice::Dice::new(), crate::rng::GameRng::new(0));
        let mut ctx = crate::choice::Resolving { content: ContentStore::embedded(), sources: POK, dice: &mut dice, rng: &mut rng, table: &mut table, timing: None };
        crate::technology::self_assembly_after_production(state, &mut ctx, &a(), system).expect("answered");
    }

    #[test]
    fn self_assembly_routines_places_a_mech_after_production_in_lockstep() {
        let label = "technology::self_assembly";
        let planets = specialty_planets(1);
        let system = SystemId::new(planets[0].0.as_str());
        let mechs = |state: &GameState| state.system_state(&system).planet_units.get(&PlanetId::new(planets[0].1.as_str())).map_or(0, |u| u.iter().filter(|u| u.type_id.as_str().contains("mech")).count());
        // The card, a planet of the system, a mech in the box: asked; used (exhausted, one mech) or declined.
        for (answer, placed) in [(planets[0].1.as_str(), 1), ("decline", 0)] {
            let (state, counts, bails) = lockstep(label, || {
                let mut state = game(&["sar"], &planets);
                run_sar(&mut state, &system, &[answer]);
                state
            });
            assert_eq!(counts, (1, 0), "{answer}: {bails:?}");
            assert_eq!(mechs(&state), placed, "{answer}");
            assert_eq!(state.player(&a()).unwrap().exhausted_technologies.contains(&TechnologyId::new("sar")), placed == 1);
        }
        // No planet of the seat in the system, the card exhausted, or no card: nothing is asked.
        for (techs, planets, exhausted) in [(&["sar"][..], false, false), (&["sar"][..], true, true), (&[][..], true, false)] {
            let (state, counts, _) = lockstep(label, || {
                let held = if planets { specialty_planets(1) } else { Vec::new() };
                let mut state = game(techs, &held);
                if exhausted {
                    state.player_mut(&a()).unwrap().exhausted_technologies.insert(TechnologyId::new("sar"));
                }
                run_sar(&mut state, &system, &[]);
                state
            });
            assert_eq!(counts, (1, 0));
            assert_eq!(mechs(&state), 0);
        }
        // Below the rules version of the card: nothing.
        let (_, counts, _) = lockstep(label, || {
            let mut state = game(&["sar"], &planets);
            state.rules_version = ti4_model::state::RULES_SELF_ASSEMBLY - 1;
            run_sar(&mut state, &system, &[]);
        });
        assert_eq!(counts, (1, 0));
    }
}
