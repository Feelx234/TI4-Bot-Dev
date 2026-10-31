//! `Area::LeadersAbilities` (package W10 of lent-bodies lane 8): the core halves of the heroes composed from windows
//! (`ti4_engine_core::rules::hero_windows`), beside their string bodies in `factions/*.rs`.
//!
//! A core half answers `None` (the string result stands, counted as a bail with its reason) where the position holds more than the body:
//! a unit type the typed tables do not know, a question inside a battle or an invasion, a nested emission.

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

/// One planet UNIT.DSGN.FLAYESH can devour as the string read it: the system, the planet, its resources and influence now and the
/// technologies of its colours the seat may gain (content order).
#[derive(Debug, Clone)]
pub struct FlayeshTarget {
    pub system: ti4_model::id::SystemId,
    pub planet: ti4_model::id::PlanetId,
    pub goods: i64,
    pub techs: Vec<ti4_model::id::TechnologyId>,
}

#[cfg(feature = "typed-core")]
mod real {
    use ti4_engine_core::driver::Driver;
    use ti4_engine_core::generated::{Planet, System, Technology};
    use ti4_engine_core::kernel::stage::AbilityTable;
    use ti4_engine_core::kernel::{Kernel, Resolver};
    use ti4_engine_core::rules::hero_windows::{ArgentHero, CabalHero, Devourable, EmpyreanHero, FrontierFacts, KyverHero, MigrationFacts, NaazHero, NekroHero, Stranded, WinnuHero};
    use ti4_engine_core::state::Slice;
    use ti4_model::content_types::SourceSet;
    use ti4_model::id::PlayerId;
    use ti4_model::state::GameState;

    use super::FlayeshTarget;
    use crate::generated::IdStr;
    use crate::core_adapter::typed;
    use crate::core_sync::{Footprint, Sources};
    use crate::dual::{CoreWindow, Run, Why};
    use crate::dual_window::Recording;

    fn kernel(sources: SourceSet) -> Option<Kernel> {
        let mut kernel = Kernel::plain();
        kernel.sources = ti4_engine_core::kernel::Sources(u8::try_from(crate::core_adapter::scope(sources).0).ok()?);
        Some(kernel)
    }

    /// What UNIT.DSGN.FLAYESH reads (the units of the planets, the trade goods) and writes (the units, the goods, the technology, the staged
    /// ground row and the staged event).
    const FLAYESH: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Marks, Slice::Pending],
        &[Slice::Players, Slice::Board, Slice::Marks, Slice::Pending],
    );

    /// The core half of Nekro's UNIT.DSGN.FLAYESH (`factions::nekro_units::flayesh`; the value: a planet was devoured). `targets` are the
    /// planets the string offers, in its order (empty: the hero is not playable); the facts are the string's reads.
    pub fn nekro_hero(
        gs: &mut GameState,
        content: &ti4_content::ContentStore,
        player: &PlayerId,
        targets: &[FlayeshTarget],
        sources: SourceSet,
        script: &Recording,
    ) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let planets: Vec<Devourable> = targets
            .iter()
            .map(|t| {
                Some(Devourable {
                    system: typed::<System>(t.system.as_str())?,
                    planet: typed::<Planet>(t.planet.as_str())?,
                    goods: i32::try_from(t.goods).ok()?,
                    techs: t.techs.iter().map(|alias| typed::<Technology>(alias.as_str())).collect::<Option<_>>()?,
                })
            })
            .collect::<Option<_>>()
            .why("a planet or technology the typed tables do not know")?;
        let kernel = kernel(sources)?;
        let before = gs.clone();
        let mut staged = Vec::new();
        let mut driver = Driver::new(NekroHero::new(seat, r.code("nekrohero"), planets), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), FLAYESH, |core, _| {
            let driven = crate::dual_window::drive(&mut driver, core, &kernel, &r, &before, script);
            staged = std::mem::take(&mut core.pending_mut().staged);
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        crate::dual_window::apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        // The string re-forms the units after the gain (`technology::gain`): a position it would change is outside the window.
        let mut formed = gs.clone();
        crate::technology::apply_unit_upgrades(&mut formed, content, sources, player);
        if !formed.identical(gs) {
            crate::dual::note_bail_here("the unit upgrades would re-form units after the gain");
            return None;
        }
        Some(CoreWindow { run: Run::new(driver.window.used).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    /// What Kyver reads (the hands, the card groups) and writes (the hands, the staged cards, the stream).
    const KYVER: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Decks, Slice::Agenda, Slice::Marks, Slice::Cards, Slice::Pending, Slice::Rng],
        &[Slice::Players, Slice::Decks, Slice::Marks, Slice::Cards, Slice::Rng],
    );

    /// The core half of Yssaril's Kyver, Blade and Key (`factions::yssaril::use_leader`; the value: some card was shown). `rng_before` is where
    /// the streams stood before the string rolled; the streams after are compared. A hand limit a faction hook changes is not carried
    /// (the child window reads the printed rule and the laws): the string's.
    pub fn yssaril_hero(
        gs: &mut GameState,
        content: &ti4_content::ContentStore,
        player: &PlayerId,
        rng_before: &crate::rng::RngPositions,
        script: &Recording,
    ) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        if crate::laws::blanked(gs) {
            crate::dual::note_bail_here("a turn in which the laws are blank (Law's Order)");
            return None;
        }
        let plain = crate::laws::action_card_limit(gs, crate::action_cards::HAND_LIMIT);
        let limit = crate::factions::hooks_economy::action_card_limit(gs, content, player, plain);
        if limit != plain && limit != usize::MAX {
            crate::dual::note_bail_here("a faction changes the action card hand limit");
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let seats: Vec<ti4_engine_core::ids::Seat> = r.seats.iter().filter_map(|text| r.seat(text)).collect();
        let before = gs.clone();
        let kernel = Kernel::plain();
        let mut table = AbilityTable::default();
        table.facts.card_groups = crate::reactions_core::card_groups();
        ti4_engine_core::driver::children::register(&mut table);
        let seeds = std::rc::Rc::new(crate::core_sync::codec::rng::TextSeeds::of(&cx));
        let mut driver = Driver::new(KyverHero::new(seat, limit == plain), Resolver::new(&seats, Some(seat)), table, 1).with_seeds(seeds);
        let mut after = None;
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources { galaxy: None, rng: Some(rng_before) }, KYVER, |core, _| {
            let driven = crate::dual_window::drive(&mut driver, core, &kernel, &r, &before, script);
            after = Some(core.rng().clone());
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        if driver.window.unsupported {
            crate::dual::note_bail_here("the table has no link to the hand limit window or no printed-name groups");
            return None;
        }
        let positions = crate::core_sync::codec::rng::positions(&cx, &after?).why("the random streams could not be read back")?;
        let mut run = Run::new(driver.window.used).with_events_vec(Vec::new());
        run.rng = Some(positions);
        Some(CoreWindow { run, questions: driven.rendered })
    }

    /// What It Feeds on Carrion reads (the ships, the cargo, the fleet limits) and writes (the captured ships and cargo, the board, the stream).
    const CARRION: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Rng],
        &[Slice::Players, Slice::Board, Slice::Rng],
    );

    /// The core half of Cabal's It Feeds on Carrion (`factions::cabal::use_leader`; the value: the card resolved). `targets` are the ships the
    /// string rolls for, in its order; the enforcement limits of each owner and system are the string's reads (`tactical_dual::limits_of`).
    /// `rng_before` is where the streams stood before the string rolled; the streams after are compared.
    pub fn cabal_hero(
        gs: &mut GameState,
        content: &ti4_content::ContentStore,
        player: &PlayerId,
        targets: &[(ti4_model::id::SystemId, ti4_model::units::Unit)],
        rng_before: &crate::rng::RngPositions,
        sources: SourceSet,
        script: &Recording,
    ) -> Option<CoreWindow<bool>> {
        use ti4_engine_core::kernel::children::SeeingLimits;
        use ti4_engine_core::state::Piece;
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let seats: Vec<ti4_engine_core::ids::Seat> = r.seats.iter().filter_map(|text| r.seat(text)).collect();
        let pieces: Vec<(System, Piece)> = targets
            .iter()
            .map(|(system, unit)| {
                let piece = Piece::new(typed::<ti4_engine_core::generated::Unit>(unit.type_id.as_str())?, r.seat(unit.owner.as_str())?)
                    .with_sustained(unit.sustained_damage)
                    .with_galvanized(unit.galvanized);
                Some((typed::<System>(system.as_str())?, piece))
            })
            .collect::<Option<_>>()
            .why("a ship, a system or an owner the typed tables do not know")?;
        // The owners and systems a capture can strand cargo in, in the order of the string's set of (owner, system) texts.
        let mut pairs: std::collections::BTreeSet<(String, String)> = std::collections::BTreeSet::new();
        for (system, unit) in targets {
            pairs.insert((unit.owner.to_string(), system.to_string()));
        }
        let mut stranded = Vec::new();
        for (owner, system) in &pairs {
            let owner_id = PlayerId::new(owner.clone());
            let system_id = ti4_model::id::SystemId::new(system.clone());
            let limits = match crate::tactical_dual::limits_of(gs, content, &owner_id, &system_id) {
                Ok(limits) => limits,
                Err(why) => {
                    crate::dual::note_bail_here(why);
                    return None;
                }
            };
            stranded.push(Stranded {
                owner: r.seat(owner)?,
                system: typed::<System>(system)?,
                limits: SeeingLimits { enforce: limits.enforce, shown: limits.shown, halves: limits.halves },
            });
        }
        let before = gs.clone();
        let kernel = crate::core_adapter::kernel_for(sources);
        let mut table = AbilityTable::default();
        ti4_engine_core::driver::children::register(&mut table);
        let seeds = std::rc::Rc::new(crate::core_sync::codec::rng::TextSeeds::of(&cx));
        let mut driver = Driver::new(CabalHero::new(seat, true, pieces, stranded), Resolver::new(&seats, Some(seat)), table, 1).with_seeds(seeds);
        let mut after = None;
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources { galaxy: None, rng: Some(rng_before) }, CARRION, |core, _| {
            let driven = crate::dual_window::drive(&mut driver, core, kernel, &r, &before, script);
            after = Some(core.rng().clone());
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        if driver.window.unsupported {
            crate::dual::note_bail_here("the table has no link to the enforcement window");
            return None;
        }
        let positions = crate::core_sync::codec::rng::positions(&cx, &after?).why("the random streams could not be read back")?;
        let mut run = Run::new(driver.window.used).with_events_vec(Vec::new());
        run.rng = Some(positions);
        Some(CoreWindow { run, questions: driven.rendered })
    }

    /// Stage the announcement the string stages at the end of the use of PRODUCTION at `system` when it made something (`UNITS_PRODUCED`, no
    /// timing handle).
    fn announce_use(gs: &mut GameState, player: &PlayerId, system: System, made: &ti4_engine_core::kernel::children::Made) {
        if made.as_slice().is_empty() {
            return;
        }
        let units: Vec<serde_json::Value> = made
            .as_slice()
            .iter()
            .map(|(unit, planet)| serde_json::json!({ "unit_type": unit.as_str(), "place": planet.map_or(crate::production::SPACE, |p| p.as_str()) }))
            .collect();
        let mut payload = std::collections::BTreeMap::new();
        payload.insert("player".to_owned(), player.to_string().into());
        payload.insert("system".to_owned(), system.as_str().into());
        payload.insert("source".to_owned(), "production".into());
        payload.insert("count".to_owned(), made.as_slice().len().into());
        payload.insert("units".to_owned(), serde_json::Value::Array(units));
        crate::supply::stage_event(gs, "UNITS_PRODUCED", &payload);
    }

    /// The facts of the ability a hero performs for a strategy card (`kernel::fx_perform`), on the lent position.
    fn perform_facts(
        core: &ti4_engine_core::state::CoreState,
        gs: &GameState,
        sources: &SourceSet,
        galaxy: Option<&ti4_content::galaxy::Galaxy>,
        player: &PlayerId,
        seat: ti4_engine_core::ids::Seat,
        r: &crate::render::Render,
        cards: &[ti4_engine_core::generated::StrategyCard],
    ) -> ti4_engine_core::kernel::fx_perform::Facts {
        let content = ti4_content::ContentStore::embedded();
        ti4_engine_core::kernel::fx_perform::Facts {
            carried: crate::action_cards_view::carried_of(core, gs, sources, player, seat, r),
            replenish_events: crate::leaders_core::replenish_causes_events(gs),
            rules_version: gs.rules_version,
            roster: crate::technology_core::roster_for(content, sources.clone()),
            scope: crate::core_adapter::scope(sources.clone()),
            mahact_held: gs.tokens_held_by_mahact(player),
            home: home_of(gs, player),
            imperial: crate::strategy_cards_core::imperial_facts(gs, content, sources.clone(), galaxy, player).ok().map(|(facts, _)| facts),
            free: free_systems(gs, galaxy, r, cards),
            te_uses: te_uses(gs, content, sources, player, seat, r, cards),
        }
    }

    /// The systems the free tactical action of the Thunder's Edge Warfare primary is offered in (`strategy_cards::te6_warfare`): the printed
    /// map and every system in play beside it, in the string's order; none outside the action phase or without a galaxy. `None`: a system
    /// the typed tables do not know, or more than the facts hold (only asked for when the card is among `cards`).
    pub(crate) fn free_systems(
        gs: &GameState,
        galaxy: Option<&ti4_content::galaxy::Galaxy>,
        r: &crate::render::Render,
        cards: &[ti4_engine_core::generated::StrategyCard],
    ) -> Option<ti4_engine_core::kernel::children::FreeSystems> {
        use ti4_engine_core::kernel::children::FreeSystems;
        let source = r.code("te6warfare");
        let mut ids: Vec<String> = Vec::new();
        if let (true, Some(galaxy), true) = (cards.contains(&ti4_engine_core::generated::StrategyCard::WarfareThundersEdge), galaxy, gs.phase == ti4_model::state::Phase::Action) {
            ids = galaxy.system_ids_holding_things().into_iter().map(ToOwned::to_owned).collect();
            for system in gs.board.keys() {
                if !galaxy.is_hyperlane(system.as_str()) && !ids.iter().any(|s| s == system.as_str()) {
                    ids.push(system.to_string());
                }
            }
        }
        let systems: Vec<System> = ids.iter().map(|id| typed::<System>(id)).collect::<Option<_>>()?;
        FreeSystems::new(source, &systems)
    }

    /// The uses of PRODUCTION the Thunder's Edge Construction primary may choose (`strategy_cards::te4_construction`): the facts of the use in
    /// each system of the board that can produce, in the order of the board. `None`: a table the production windows do not carry, or more
    /// systems than the facts hold (only asked for when the card is among `cards`).
    pub(crate) fn te_uses(
        gs: &GameState,
        content: &ti4_content::ContentStore,
        sources: &SourceSet,
        player: &PlayerId,
        seat: ti4_engine_core::ids::Seat,
        r: &crate::render::Render,
        cards: &[ti4_engine_core::generated::StrategyCard],
    ) -> Option<ti4_engine_core::kernel::children::TeUses> {
        use ti4_engine_core::kernel::children::TeUses;
        let source = r.code("te4construction");
        let mut uses = Vec::new();
        if cards.contains(&ti4_engine_core::generated::StrategyCard::ConstructionThundersEdge) {
            for system in gs.board.keys().filter(|system| crate::production::capacity(gs, content, sources.clone(), player, system) > 0) {
                match crate::leaders_core::home_facts(gs, content, sources.clone(), seat, player, system) {
                    Ok(facts) => uses.push(facts),
                    Err(why) => {
                        crate::dual::note_bail_here(why);
                        return None;
                    }
                }
            }
        }
        TeUses::new(source, &uses)
    }

    /// What the Warfare secondary finds at the seat's home system (`strategy_cards::home_production`): nothing without a home system or
    /// capacity, a use of PRODUCTION with the facts the string window opens with, or a table the production windows do not carry.
    fn home_of(gs: &GameState, player: &PlayerId) -> ti4_engine_core::kernel::fx_perform::Home {
        use ti4_engine_core::kernel::fx_perform::Home;
        let Some(home) = gs.player(player).and_then(|p| p.home_system.clone()) else { return Home::Nothing };
        // What the table can follow does not change within the run; the facts do (a secondary performed before the Warfare secondary), so the
        // core reads them when the use starts.
        if crate::leaders_core::home_declined(gs, player).is_some() {
            return Home::Unsupported;
        }
        match typed::<System>(home.as_str()) {
            Some(system) => Home::At(system),
            None => Home::Unsupported,
        }
    }

    /// The ability table of a hero that performs a strategy card: the links to the abilities' windows and the card groups of a draw.
    fn performing_table() -> AbilityTable {
        let mut table = AbilityTable::default();
        ti4_engine_core::driver::children::register(&mut table);
        ti4_engine_core_factions::self_assembly::register_links(&mut table);
        table.facts.card_groups = crate::reactions_core::card_groups();
        table
    }

    /// The core half of Naaz's Hesh and Prit (`factions::naaz::use_leader`; the value: the card resolved). `cards` are the strategy cards the
    /// string offers, in its order (the relic deck is read on the position).
    pub fn naaz_hero(
        gs: &mut GameState,
        player: &PlayerId,
        cards: &[ti4_model::id::StrategyCardId],
        sources: &SourceSet,
        galaxy: Option<&ti4_content::galaxy::Galaxy>,
        script: &Recording,
    ) -> Option<CoreWindow<bool>> {
        use ti4_engine_core::generated::StrategyCard;
        if let Some(why) = crate::dual_window::outside_following(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        crate::dual_window::nested_check(script)?;
        let list: Vec<StrategyCard> = cards.iter().map(|card| typed::<StrategyCard>(card.as_str())).collect::<Option<_>>().why("a strategy card the typed tables do not know")?;
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let before = gs.clone();
        let kernel = crate::production_view::kernel_of(sources.clone());
        let performing = crate::strategy_cards_core::Performing {
            before: &before,
            content: ti4_content::ContentStore::embedded(),
            sources: sources.clone(),
            player,
            seat,
            primary: false,
            r: &r,
            free: std::cell::Cell::new(0),
        };
        let letnev = gs.player(player).why(crate::dual::reason::NO_SEAT)?.faction.as_str() == "letnev";
        let details = |c: &ti4_engine_core::choice::Choice, core: &ti4_engine_core::state::CoreState, rendered: &mut crate::choice::Choice| {
            crate::strategy_cards_core::complete_tokens(c, core, rendered, letnev);
            performing.complete(c, core, rendered);
        };
        let mut staged = Vec::new();
        let mut out = None;
        let mut home_system = None;
        let driven = crate::core_sync::with_core(
            gs,
            &mut cx,
            Sources::default(),
            crate::dual_window::nested_fp(crate::action_cards_view::PLAY.union(crate::action_cards_view::PERFORM).union(Footprint::new(&[Slice::Objectives], &[Slice::Objectives])), script),
            |core, _| {
                let facts = perform_facts(core, &before, sources, galaxy, player, seat, &r, &list);
                home_system = match facts.home {
                    ti4_engine_core::kernel::fx_perform::Home::At(system) => Some(system),
                    _ => None,
                };
                let mut driver = Driver::new(NaazHero::new(seat, facts, list), Resolver::new(&[seat], Some(seat)), performing_table(), 1);
                let driven = crate::dual_window::drive_positioned(&mut driver, core, kernel, Some(kernel), &r, &before, script, &details);
                staged = std::mem::take(&mut core.pending_mut().staged);
                out = Some((driver.window.used, driver.window.unsupported, driver.window.home_made.clone(), driver.window.home_staged));
                driven
            },
        )
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        let (used, unsupported, home_made, home_staged) = out?;
        if unsupported {
            crate::dual::note_bail_here("an ability the core does not carry (the Warfare secondary at a table the production windows do not carry or after a secondary that changes what a production reads, a changed draw, a research a waiver opens)");
            return None;
        }
        // The announcement of the home production goes where the core finished it: after the events the secondaries before it staged, before the
        // ones the secondaries after it stage.
        let at = home_staged.min(staged.len());
        crate::dual_window::apply_staged(gs, &r, &staged[..at]).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        if let (Some(made), Some(system)) = (home_made, home_system) {
            announce_use(gs, player, system, &made);
        }
        crate::dual_window::apply_staged(gs, &r, &staged[at..]).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        Some(CoreWindow { run: Run::new(used).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    /// The core half of Winnu's Mathis Mathinus (`factions::winnu::use_leader_strategy_primary`; the value: the card chosen and whether its
    /// primary resolved plainly, the system of the free tactical action the Thunder's Edge Warfare primary leaves to the game). `cards` are the strategy cards the string offers, in its order.
    pub fn winnu_hero(
        gs: &mut GameState,
        player: &PlayerId,
        cards: &[ti4_model::id::StrategyCardId],
        sources: &SourceSet,
        galaxy: Option<&ti4_content::galaxy::Galaxy>,
        script: &Recording,
    ) -> Option<CoreWindow<(Option<String>, bool, Option<String>)>> {
        use ti4_engine_core::generated::StrategyCard;
        if let Some(why) = crate::dual_window::outside_following(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        crate::dual_window::nested_check(script)?;
        let list: Vec<StrategyCard> = cards.iter().map(|card| typed::<StrategyCard>(card.as_str())).collect::<Option<_>>().why("a strategy card the typed tables do not know")?;
        // The primaries name their options by faction: two seats of one faction would clash.
        let (mut cx, r) = crate::strategy_core::named_tables(gs)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let before = gs.clone();
        let kernel = crate::production_view::kernel_of(sources.clone());
        let performing = crate::strategy_cards_core::Performing {
            before: &before,
            content: ti4_content::ContentStore::embedded(),
            sources: sources.clone(),
            player,
            seat,
            primary: true,
            r: &r,
            free: std::cell::Cell::new(0),
        };
        let letnev = gs.player(player).why(crate::dual::reason::NO_SEAT)?.faction.as_str() == "letnev";
        let details = |c: &ti4_engine_core::choice::Choice, core: &ti4_engine_core::state::CoreState, rendered: &mut crate::choice::Choice| {
            crate::strategy_cards_core::complete_tokens(c, core, rendered, letnev);
            performing.complete(c, core, rendered);
        };
        let mut staged = Vec::new();
        let mut out = None;
        let driven = crate::core_sync::with_core(
            gs,
            &mut cx,
            Sources::default(),
            crate::dual_window::nested_fp(crate::action_cards_view::PLAY.union(crate::action_cards_view::PERFORM).union(Footprint::new(&[Slice::Objectives], &[Slice::Objectives])), script),
            |core, _| {
                let facts = perform_facts(core, &before, sources, galaxy, player, seat, &r, &list);
                let mut driver = Driver::new(WinnuHero::new(seat, facts, list), Resolver::new(&[seat], Some(seat)), performing_table(), 1);
                let driven = crate::dual_window::drive_positioned(&mut driver, core, kernel, Some(kernel), &r, &before, script, &details);
                staged = std::mem::take(&mut core.pending_mut().staged);
                out = Some((driver.window.chosen, driver.window.tokens.clone(), driver.window.scored, driver.window.unsupported, driver.window.free, driver.window.produced.clone()));
                driven
            },
        )
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        let (chosen, tokens, scored, unsupported, free, produced) = out?;
        if unsupported {
            crate::dual::note_bail_here("an ability the core does not carry (the Imperial primary at a table the scoring tables do not follow, a Thunder's Edge primary at a table whose facts the core could not take, a changed draw, a research a waiver opens)");
            return None;
        }
        let announced = produced.as_ref().is_some_and(|(_, made)| !made.as_slice().is_empty());
        if announced && !staged.is_empty() {
            crate::dual::note_bail_here("staged events of the primary would interleave with the announcement of its production");
            return None;
        }
        crate::dual_window::apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        if let Some((system, made)) = produced {
            announce_use(gs, player, system, &made);
        }
        crate::action_cards_view::stage_placed_tokens(gs, &r, tokens.map(|(system, seats)| (system, seats.as_slice().to_vec())));
        // What a public score sets off (leaders unlocking, Yin's breakthrough) and the winner are the string's tail of the primary.
        if let Some(objective) = scored {
            crate::objectives::award_tail(gs, ti4_content::ContentStore::embedded(), sources.clone(), player.clone(), &ti4_model::id::ObjectiveId::new(objective.as_str()));
        }
        if chosen.is_some_and(|card| card.data().kind == ti4_engine_core::generated::StrategyCardKind::Imperial) {
            gs.finished = crate::objectives::winner(gs).is_some();
        }
        let card = chosen.map(|card| crate::generated::IdStr::as_str(card).to_owned());
        let free = free.map(|system| system.as_str().to_owned());
        Some(CoreWindow { run: Run::new((card, free.is_none(), free)).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    /// What the primary of the Thunder's Edge Construction reads (the production view, the staged events) and writes (the structures and the
    /// units, the payment, the staged events).
    const TE_CONSTRUCTION: Footprint = Footprint {
        reads: crate::production_view::READS.union(ti4_engine_core::state::SliceSet::of(&[Slice::Pending])),
        writes: ti4_engine_core::state::SliceSet::of(&[Slice::Board, Slice::Players, Slice::Cards, Slice::Transient, Slice::Marks, Slice::Pending]),
    };

    /// The core half of the primary of the Thunder's Edge Construction played by its holder (`strategy_cards::te4_construction`; the value: it
    /// resolved): the choice of a structure or a use of PRODUCTION, the use as a child window of the production area, and the structures.
    pub fn te_construction(
        gs: &mut GameState,
        content: &ti4_content::ContentStore,
        sources: SourceSet,
        player: &PlayerId,
        script: &Recording,
    ) -> Option<CoreWindow<bool>> {
        use ti4_engine_core::generated::StrategyCard;
        use ti4_engine_core::rules::production::strategy_cards::Construction;
        if let Some(why) = crate::dual_window::outside_following(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        crate::dual_window::nested_check(script)?;
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let facts = crate::strategy_cards_core::structure_facts(gs, content, sources.clone(), player, &r)?;
        let uses = te_uses(gs, content, &sources, player, seat, &r, &[StrategyCard::ConstructionThundersEdge])?;
        let list: Vec<_> = uses.uses().collect();
        let window = Construction::new_te_producing(seat, facts, uses.source, &list);
        let before = gs.clone();
        let kernel = crate::production_view::kernel_of(sources.clone());
        let mut staged = Vec::new();
        let mut driver = Driver::new(window, Resolver::new(&[seat], Some(seat)), performing_table(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), crate::dual_window::nested_fp(TE_CONSTRUCTION, script), |core, _| {
            let driven = crate::dual_window::drive_positioned(&mut driver, core, kernel, Some(kernel), &r, &before, script, &|_, _, _| {});
            staged = std::mem::take(&mut core.pending_mut().staged);
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        if driver.window.unsupported || driver.window.produces {
            crate::dual::note_bail_here("the use of PRODUCTION of the Thunder's Edge Construction holds something the core does not carry");
            return None;
        }
        let produced = driver.window.produced.clone();
        let announced = produced.as_ref().is_some_and(|(_, made)| !made.as_slice().is_empty());
        if announced && !staged.is_empty() {
            crate::dual::note_bail_here("staged events of the primary would interleave with the announcement of its production");
            return None;
        }
        crate::dual_window::apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        if let Some((system, made)) = produced {
            announce_use(gs, player, system, &made);
        }
        Some(CoreWindow { run: Run::new(true).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    /// What Conservator Procyon reads (the ships, the decks, the seat's holdings) and writes (the frontier tokens, the log, the cards, the
    /// goods, the relic, the board for the Mirage and the staged events).
    const FRONTIER: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Decks, Slice::Cards, Slice::Agenda, Slice::Marks, Slice::Transient, Slice::Pending, Slice::Strategy, Slice::Ledger, Slice::Objectives, Slice::Explore],
        &[Slice::Meta, Slice::Players, Slice::Board, Slice::Decks, Slice::Cards, Slice::Marks, Slice::Pending, Slice::Ledger, Slice::Explore],
    );

    /// The core half of Empyrean's Conservator Procyon (`factions::empyrean::use_leader`; the value: the card resolved, not undone by a frontier
    /// deck that ran out). `placed` are the planetless systems the string puts a frontier token in. The log rows the cards opened are closed with
    /// the words of `exploration::row_words`, as the string closes them.
    pub fn empyrean_hero(
        gs: &mut GameState,
        content: &ti4_content::ContentStore,
        player: &PlayerId,
        placed: &[ti4_model::id::SystemId],
        sources: SourceSet,
        script: &Recording,
    ) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_following(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        crate::dual_window::nested_check(script)?;
        // Every system that can hold a token, in the text order of its name (the string's set).
        let mut named: std::collections::BTreeSet<&str> = placed.iter().map(ti4_model::id::SystemId::as_str).collect();
        named.extend(gs.frontier_tokens.iter().map(ti4_model::id::SystemId::as_str));
        let order: Vec<System> = named.into_iter().map(typed::<System>).collect::<Option<_>>().why("a system the typed tables do not know")?;
        let placed: Vec<System> = placed.iter().map(|id| typed::<System>(id.as_str())).collect::<Option<_>>().why("a system the typed tables do not know")?;
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let facts = FrontierFacts {
            empty: cx.texts.intern(Slice::Explore, "").ok().why("the empty text of an exploration row could not be interned")?,
            rules_version: gs.rules_version,
            mahact_held: gs.tokens_held_by_mahact(player),
            agreement: crate::leaders_core::replenish_window_unported(gs).is_none().then(|| r.code("trade_agreement")),
            draw_plain: crate::strategy_cards_core::plain_draw(gs, content, player, 2),
        };
        let before = gs.clone();
        let start = gs.exploration_log.len();
        let kernel = crate::core_adapter::kernel_for(sources);
        let letnev = gs.player(player).why(crate::dual::reason::NO_SEAT)?.faction.as_str() == "letnev";
        let details = |c: &ti4_engine_core::choice::Choice, core: &ti4_engine_core::state::CoreState, rendered: &mut crate::choice::Choice| {
            crate::action_cards_view::reward_details(c, &before, player, rendered);
            crate::strategy_cards_core::complete_tokens(c, core, rendered, letnev);
        };
        let mut staged = Vec::new();
        let mut out = None;
        let mut driver = Driver::new(EmpyreanHero::new(seat, placed, order, facts), Resolver::new(&[seat], Some(seat)), performing_table(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), crate::dual_window::nested_fp(FRONTIER, script), |core, _| {
            let driven = crate::dual_window::drive_with(&mut driver, core, kernel, &r, &before, script, &details);
            staged = std::mem::take(&mut core.pending_mut().staged);
            out = Some((driver.window.used, driver.window.unsupported, driver.window.rows().to_vec()));
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        let (used, unsupported, rows) = out?;
        if unsupported {
            crate::dual::note_bail_here("a frontier card the core does not carry (a card of another deck, a changed draw, a replenishment with events it does not hold)");
            return None;
        }
        for (at, row) in rows.iter().enumerate() {
            crate::action_cards_view::close_frontier_row(gs, player, start + at, row).why("a log row the core did not open as the string does")?;
        }
        crate::dual_window::apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        Some(CoreWindow { run: Run::new(used).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    /// What Flock Migration reads (the ships, the tokens, the cargo) and writes (the ships and the cargo that move).
    const FLOCK: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board], &[Slice::Board]);

    /// The core half of Argent's Flock Migration (`factions::argent::use_leader`; the value: some ship moved). The map rules the relocation
    /// checks (a system on the map, a destination the seat's movement rules let it enter) are read on the position the card starts from.
    pub fn argent_hero(
        gs: &mut GameState,
        content: &ti4_content::ContentStore,
        player: &PlayerId,
        galaxy: &ti4_content::galaxy::Galaxy,
        sources: SourceSet,
        script: &Recording,
    ) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let seats: Vec<ti4_engine_core::ids::Seat> = r.seats.iter().filter_map(|text| r.seat(text)).collect();
        let mut order = Vec::new();
        let mut on_map = Vec::new();
        let mut enterable = Vec::new();
        let holds_token = gs.systems_with_token(player);
        for id in gs.board.keys() {
            let system = typed::<System>(id.as_str()).why("a system the typed tables do not know")?;
            order.push(system);
            if galaxy.coord_of(id.as_str()).is_some() || galaxy.wormhole_systems().contains(&id.as_str()) {
                on_map.push(system);
            }
            if holds_token.contains(id) {
                let mut rules = crate::movement::MovementRules::with_laws(
                    galaxy,
                    content,
                    sources,
                    id.as_str(),
                    crate::movement::Board::for_player(gs, content, sources, player),
                    Some(gs),
                );
                crate::action_cards::apply_movement_effects(&mut rules, gs, player);
                if rules.can_enter(id.as_str()) {
                    enterable.push(system);
                }
            }
        }
        let before = gs.clone();
        let kernel = crate::core_adapter::kernel_for(sources);
        let mut driver = Driver::new(ArgentHero::new(seat, MigrationFacts { order, on_map, enterable }), Resolver::new(&seats, Some(seat)), AbilityTable::default(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), FLOCK, |core, _| crate::dual_window::drive(&mut driver, core, kernel, &r, &before, script))
            .why(crate::dual::reason::NOT_LENDABLE)?
            .why(crate::dual::reason::DRIVER_REFUSED)?;
        Some(CoreWindow { run: Run::new(driver.window.moved).with_events_vec(Vec::new()), questions: driven.rendered })
    }
}

#[cfg(not(feature = "typed-core"))]
mod stub {
    use ti4_model::id::PlayerId;
    use ti4_model::state::GameState;

    pub fn nekro_hero(
        _: &mut GameState,
        _: &ti4_content::ContentStore,
        _: &PlayerId,
        _: &[super::FlayeshTarget],
        _: ti4_model::content_types::SourceSet,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }

    pub fn yssaril_hero(
        _: &mut GameState,
        _: &ti4_content::ContentStore,
        _: &PlayerId,
        _: &crate::rng::RngPositions,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }

    pub fn cabal_hero(
        _: &mut GameState,
        _: &ti4_content::ContentStore,
        _: &PlayerId,
        _: &[(ti4_model::id::SystemId, ti4_model::units::Unit)],
        _: &crate::rng::RngPositions,
        _: ti4_model::content_types::SourceSet,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }

    pub fn naaz_hero(
        _: &mut GameState,
        _: &PlayerId,
        _: &[ti4_model::id::StrategyCardId],
        _: &ti4_model::content_types::SourceSet,
        _: Option<&ti4_content::galaxy::Galaxy>,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }

    pub fn winnu_hero(
        _: &mut GameState,
        _: &PlayerId,
        _: &[ti4_model::id::StrategyCardId],
        _: &ti4_model::content_types::SourceSet,
        _: Option<&ti4_content::galaxy::Galaxy>,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<(Option<String>, bool, Option<String>)>> {
        None
    }

    pub fn argent_hero(
        _: &mut GameState,
        _: &ti4_content::ContentStore,
        _: &PlayerId,
        _: &ti4_content::galaxy::Galaxy,
        _: ti4_model::content_types::SourceSet,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }

    pub fn empyrean_hero(
        _: &mut GameState,
        _: &ti4_content::ContentStore,
        _: &PlayerId,
        _: &[ti4_model::id::SystemId],
        _: ti4_model::content_types::SourceSet,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }
}
