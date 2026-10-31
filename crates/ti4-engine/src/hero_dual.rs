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
    use ti4_engine_core::generated::{Planet, System, Technology};
    use ti4_engine_core::kernel::stage::AbilityTable;
    use ti4_engine_core::kernel::{Kernel, Resolver};
    use ti4_engine_core::rules::hero_windows::{ArgentHero, CabalHero, Devourable, KyverHero, MigrationFacts, NaazHero, NekroHero, Stranded, WinnuHero};
    use ti4_engine_core::state::Slice;
    use ti4_model::content_types::SourceSet;
    use ti4_model::id::PlayerId;
    use ti4_model::state::GameState;

    use super::FlayeshTarget;
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
        let mut driver = crate::window_driver::driver_for(NekroHero::new(seat, r.code("nekrohero"), planets), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), FLAYESH, |core, _| {
            let driven = crate::dual_window::drive(&mut driver, core, &kernel, &r, &before, script);
            staged = std::mem::take(&mut core.pending_mut().staged);
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        if driver.window.unsupported {
            crate::dual::note_bail_here("a galvanized ground force fell (the staged row cannot hold it)");
            return None;
        }
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
        let mut driver = crate::window_driver::driver_for(KyverHero::new(seat, limit == plain), Resolver::new(&seats, Some(seat)), table, 1).with_seeds(seeds);
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
        let mut driver = crate::window_driver::driver_for(CabalHero::new(seat, true, pieces, stranded), Resolver::new(&seats, Some(seat)), table, 1).with_seeds(seeds);
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

    /// The facts of the ability a hero performs for a strategy card (`kernel::fx_perform`), on the lent position.
    fn perform_facts(
        core: &ti4_engine_core::state::CoreState,
        gs: &GameState,
        sources: &SourceSet,
        player: &PlayerId,
        seat: ti4_engine_core::ids::Seat,
        r: &crate::render::Render,
    ) -> ti4_engine_core::kernel::fx_perform::Facts {
        ti4_engine_core::kernel::fx_perform::Facts {
            carried: crate::action_cards_view::carried_of(core, gs, sources, player, seat, r),
            replenish_events: crate::leaders_core::replenish_causes_events(gs),
            rules_version: gs.rules_version,
            roster: crate::technology_core::roster_for(ti4_content::ContentStore::embedded(), sources.clone()),
            scope: crate::core_adapter::scope(sources.clone()),
            mahact_held: gs.tokens_held_by_mahact(player),
        }
    }

    /// The ability table of a hero that performs a strategy card: the links to the abilities' windows and the card groups of a draw.
    fn performing_table() -> AbilityTable {
        let mut table = AbilityTable::default();
        ti4_engine_core::driver::children::register(&mut table);
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
        script: &Recording,
    ) -> Option<CoreWindow<bool>> {
        use ti4_engine_core::generated::StrategyCard;
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let list: Vec<StrategyCard> = cards.iter().map(|card| typed::<StrategyCard>(card.as_str())).collect::<Option<_>>().why("a strategy card the typed tables do not know")?;
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let before = gs.clone();
        let kernel = crate::production_view::kernel();
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
        let driven = crate::core_sync::with_core(
            gs,
            &mut cx,
            Sources::default(),
            crate::action_cards_view::PLAY.union(crate::action_cards_view::PERFORM),
            |core, _| {
                let facts = perform_facts(core, &before, sources, player, seat, &r);
                let mut driver = crate::window_driver::driver_for(NaazHero::new(seat, facts, list), Resolver::new(&[seat], Some(seat)), performing_table(), 1);
                let driven = crate::dual_window::drive_with(&mut driver, core, kernel, &r, &before, script, &details);
                staged = std::mem::take(&mut core.pending_mut().staged);
                out = Some((driver.window.used, driver.window.unsupported));
                driven
            },
        )
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        let (used, unsupported) = out?;
        if unsupported {
            crate::dual::note_bail_here("an ability the core does not carry (the Warfare secondary, a changed draw, a research a waiver opens)");
            return None;
        }
        crate::dual_window::apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        Some(CoreWindow { run: Run::new(used).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    /// The core half of Winnu's Mathis Mathinus (`factions::winnu::use_leader_strategy_primary`; the value: the card chosen and whether its
    /// primary resolved plainly). `cards` are the strategy cards the string offers, in its order.
    pub fn winnu_hero(
        gs: &mut GameState,
        player: &PlayerId,
        cards: &[ti4_model::id::StrategyCardId],
        sources: &SourceSet,
        script: &Recording,
    ) -> Option<CoreWindow<(Option<String>, bool)>> {
        use ti4_engine_core::generated::StrategyCard;
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let list: Vec<StrategyCard> = cards.iter().map(|card| typed::<StrategyCard>(card.as_str())).collect::<Option<_>>().why("a strategy card the typed tables do not know")?;
        // The primaries name their options by faction: two seats of one faction would clash.
        let (mut cx, r) = crate::strategy_core::named_tables(gs)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let before = gs.clone();
        let kernel = crate::production_view::kernel();
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
            crate::action_cards_view::PLAY.union(crate::action_cards_view::PERFORM),
            |core, _| {
                let facts = perform_facts(core, &before, sources, player, seat, &r);
                let mut driver = crate::window_driver::driver_for(WinnuHero::new(seat, facts, list), Resolver::new(&[seat], Some(seat)), performing_table(), 1);
                let driven = crate::dual_window::drive_with(&mut driver, core, kernel, &r, &before, script, &details);
                staged = std::mem::take(&mut core.pending_mut().staged);
                out = Some((driver.window.chosen, driver.window.tokens.clone(), driver.window.unsupported));
                driven
            },
        )
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        let (chosen, tokens, unsupported) = out?;
        if unsupported {
            crate::dual::note_bail_here("an ability the core does not carry (the Imperial primary, a Thunder's Edge primary, a changed draw, a research a waiver opens)");
            return None;
        }
        crate::dual_window::apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        crate::action_cards_view::stage_placed_tokens(gs, &r, tokens.map(|(system, seats)| (system, seats.as_slice().to_vec())));
        let card = chosen.map(|card| crate::generated::IdStr::as_str(card).to_owned());
        Some(CoreWindow { run: Run::new((card, true)).with_events_vec(Vec::new()), questions: driven.rendered })
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
        let mut driver = crate::window_driver::driver_for(ArgentHero::new(seat, MigrationFacts { order, on_map, enterable }), Resolver::new(&seats, Some(seat)), AbilityTable::default(), 1);
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
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }

    pub fn winnu_hero(
        _: &mut GameState,
        _: &PlayerId,
        _: &[ti4_model::id::StrategyCardId],
        _: &ti4_model::content_types::SourceSet,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<(Option<String>, bool)>> {
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
}
