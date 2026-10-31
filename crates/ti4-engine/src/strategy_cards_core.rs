//! `Area::Strategy` (package B8), the question-asking primary and secondary abilities of Warfare, Diplomacy and
//! Politics as lockstep windows (`strategy_cards::{warfare_primary, diplomacy_primary, diplomacy_secondary,
//! politics_primary, politics_secondary}`). The Trade primary is in [`crate::strategy_core`]. Each core half
//! answers `None` (the string result stands, counted as a bail) where the position holds something the core
//! does not: see `docs/ENGINE_KNOWN_ISSUES.md`, B8.

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

#[cfg(feature = "typed-core")]
mod real {
    use serde_json::{json, Value};
    use ti4_content::ContentStore;
    use ti4_engine_core::choice::strategy::Ask as SAsk;
    use ti4_engine_core::choice::{Ask, Choice as CoreChoice};
    use ti4_engine_core::driver::Driver;
    use ti4_engine_core::generated::StrategyCard;
    use ti4_engine_core::ids::Seat;
    use ti4_engine_core::kernel::stage::{AbilityTable, Window};
    use ti4_engine_core::kernel::{fx, Kernel, Resolver};
    use ti4_engine_core::rules::production::strategy_cards::{Construction, Leadership, Placement, Research, StructureFacts};
    use ti4_engine_core::rules::strategy_primaries::{DMode, Diplomacy, ImperialPrimary, ImperialSecondary, Politics, TeWarfare, WarfarePrimary};
    use ti4_engine_core::state::{CoreState, Slice};
    use ti4_model::content_types::SourceSet;
    use ti4_model::id::{PlayerId, SystemId};
    use ti4_model::state::GameState;

    use crate::choice::Choice;
    use crate::core_sync::{with_core, Ctx, Footprint, Sources};
    use crate::dual::{CoreWindow, Run, Why};
    use crate::dual_window::{drive_with, Driven, Recording};
    use crate::generated::IdStr;
    use crate::preview::{Delta, Preview, Quantity};
    use crate::render::Render;
    use crate::strategy_core::named_tables;

    type Complete<'a> = &'a dyn Fn(&CoreChoice, &CoreState, &mut Choice);

    const WARFARE: Footprint =
        Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Pending], &[Slice::Players, Slice::Board]);
    const DIPLOMACY: Footprint =
        Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Pending], &[Slice::Board]);
    const POLITICS: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Decks, Slice::Agenda, Slice::Pending],
        &[Slice::Meta, Slice::Players, Slice::Decks],
    );

    /// Run `window` through the driver on the lent core state, answering from the recording.
    fn drive_window<W: Window>(
        gs: &mut GameState,
        cx: &mut Ctx,
        r: &Render,
        fp: Footprint,
        window: W,
        seat: Seat,
        script: &Recording,
        complete: Complete<'_>,
    ) -> Option<(W, Driven)> {
        let before = gs.clone();
        let kernel = Kernel::plain();
        let mut driver = crate::window_driver::driver_for(window, Resolver::new(&[seat], Some(seat)), linked(), 1);
        let driven = with_core(gs, cx, Sources::default(), fp, |core, _| drive_with(&mut driver, core, &kernel, r, &before, script, complete))
            .why(crate::dual::reason::NOT_LENDABLE)?
            .why(crate::dual::reason::DRIVER_REFUSED)?;
        Some((driver.window, driven))
    }

    fn finish<W>(window_done: &Driven, _: &W) -> CoreWindow<bool> {
        CoreWindow { run: Run::new(window_done.finished).with_events_vec(Vec::new()), questions: window_done.rendered.clone() }
    }

    // -- command tokens (52.4) -----------------------------------------------------------------------------

    /// `tokens::with_pool_details`: the pools as they stand now and what the question hands out or moves.
    pub(crate) fn pool_details(core: &CoreState, seat: Seat, letnev: bool, mode: &str, to_place: Option<usize>) -> serde_json::Map<String, Value> {
        let (tactic, fleet, strategic) = core
            .players()
            .by_seat(seat)
            .map_or((0, 0, 0), |p| (p.econ.tactic_tokens, p.econ.fleet_tokens, p.econ.strategic_tokens));
        let mut d = serde_json::Map::new();
        d.insert("kind".to_owned(), "command_tokens".into());
        d.insert("mode".to_owned(), mode.into());
        d.insert("pools".to_owned(), json!({ "tactic": tactic, "fleet": fleet, "strategic": strategic }));
        d.insert("reinforcements".to_owned(), fx::tokens_in_reinforcements(core, seat).into());
        if letnev {
            d.insert("fleet_supply_bonus".to_owned(), 2.into());
        }
        match (mode, to_place) {
            ("gain" | "buy", Some(count)) => {
                d.insert("tokens_to_place".to_owned(), count.into());
            }
            ("redistribute" | "restack", _) => {
                d.insert("total".to_owned(), (tactic + fleet + strategic).into());
            }
            _ => {}
        }
        d
    }

    /// The previews of the three pool options (`gain_tokens`): the count each would reach.
    fn pool_previews(core: &CoreState, seat: Seat, rendered: &mut Choice) {
        let (tactic, fleet, strategic) = core
            .players()
            .by_seat(seat)
            .map_or((0, 0, 0), |p| (i64::from(p.econ.tactic_tokens), i64::from(p.econ.fleet_tokens), i64::from(p.econ.strategic_tokens)));
        let quantities = [(Quantity::TacticTokens, tactic), (Quantity::FleetTokens, fleet), (Quantity::StrategicTokens, strategic)];
        for (option, (quantity, have)) in rendered.options.iter_mut().zip(quantities) {
            option.preview = Some(Preview::certain(vec![Delta::new(quantity, have, have + 1)]));
        }
    }

    /// The details and previews of the token questions, given the core question and state.
    pub fn complete_tokens(c: &CoreChoice, core: &CoreState, rendered: &mut Choice, letnev: bool) {
        match c.ask {
            Ask::Strategy(SAsk::GainPool { left }) => {
                pool_previews(core, c.actor, rendered);
                rendered.details.extend(pool_details(core, c.actor, letnev, "gain", Some(usize::from(left))));
            }
            Ask::Strategy(SAsk::WarfareRedistribute { .. } | SAsk::TeWarfareRedistribute { .. }) => {
                rendered.details.extend(pool_details(core, c.actor, letnev, "redistribute", None));
            }
            _ => {}
        }
    }

    // -- Warfare -------------------------------------------------------------------------------------------

    /// The core half of `strategy_cards::warfare_primary`. `None` where the position is outside what the core
    /// holds: a question inside a battle or an invasion, diplomacy on, a nested emission, a command token in a
    /// Mahact fleet pool (the reinforcements differ), a seat the tables do not hold.
    pub fn warfare_primary(gs: &mut GameState, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script).or_else(|| (gs.tokens_held_by_mahact(player) > 0).then_some("a command token in a Mahact fleet pool (the reinforcements differ)")) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let letnev = gs.player(player)?.faction.as_str() == "letnev";
        let window = WarfarePrimary::new(seat, gs.rules_version >= ti4_model::state::RULES_LETNEV_FLEET_POOL);
        let (window, driven) =
            drive_window(gs, &mut cx, &r, WARFARE, window, seat, script, &|c, core, rendered| complete_tokens(c, core, rendered, letnev))?;
        Some(finish(&driven, &window))
    }

    // -- Diplomacy -----------------------------------------------------------------------------------------

    /// `strategy_cards::ready_hints` as the question sees the position: the planets readied so far are not
    /// exhausted any more.
    fn ready_hints_now(before: &GameState, core: &CoreState, content: &ContentStore, sources: SourceSet, player: &PlayerId) -> Value {
        // The planets of the performer as the core holds them now: readied so far, and exhausted by a payment an earlier ability of the
        // same window made (Hesh and Prit performs two secondaries).
        let mut now = before.clone();
        let controlled: Vec<ti4_model::id::PlanetId> = before.controlled_planets(player).into_iter().map(|(_, p)| p.clone()).collect();
        for id in controlled {
            match ti4_engine_core::generated::Planet::from_id(id.as_str()).map(|p| ti4_engine_core::query::board::exhausted(core, p)) {
                Some(true) => {
                    now.exhausted_planets.insert(id);
                }
                Some(false) => {
                    now.exhausted_planets.remove(&id);
                }
                None => {}
            }
        }
        crate::strategy_cards::ready_hints(&now, content, sources, player)
    }

    /// The core half of the two Diplomacy abilities (`secondary`: the follower's). `None` as for Warfare, and
    /// where the string state holds an exhausted planet the core does not know.
    pub fn diplomacy(
        gs: &mut GameState,
        content: &ContentStore,
        sources: SourceSet,
        player: &PlayerId,
        card: &str,
        secondary: bool,
        script: &Recording,
    ) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let base = card == "base2";
        let printed = if base { StrategyCard::DiplomacyBase } else { StrategyCard::DiplomacyCodex };
        let mode = if secondary {
            DMode::Secondary
        } else {
            DMode::Primary { base_printing: base && gs.rules_version >= ti4_model::state::RULES_BASE_DIPLOMACY }
        };
        let before = gs.clone();
        let (window, driven) = drive_window(gs, &mut cx, &r, DIPLOMACY, Diplomacy::new(seat, printed, mode), seat, script, &|c, core, rendered| {
            if matches!(c.ask, Ask::Strategy(SAsk::ReadyPlanet)) {
                rendered.details.insert("ready_hints".to_owned(), ready_hints_now(&before, core, content, sources, player));
            }
        })?;
        // The announcements of the tokens placed are staged on the string state (not a codec slice).
        if let Some(system) = window.chosen() {
            for other in window.placed.as_slice() {
                crate::tokens::stage_command_token_placed(
                    gs,
                    &r.player(*other),
                    &SystemId::new(system.as_str()),
                    crate::factions::hooks_cards::TokenPool::Reinforcements,
                );
            }
        }
        Some(finish(&driven, &window))
    }

    // -- Leadership ----------------------------------------------------------------------------------------

    /// [`drive_with`], rendering each question in its position (the payment options show the worth of a face and
    /// what the payment leaves).
    fn drive_at<W: Window>(
        driver: &mut Driver<W>,
        core: &mut CoreState,
        kernel: &'static Kernel,
        r: &Render,
        gs: &GameState,
        script: &Recording,
        complete: Complete<'_>,
    ) -> Result<Driven, ti4_engine_core::driver::DriverError> {
        use ti4_engine_core::driver::Status;
        let (phase, round) = (crate::render::generic::core_phase(gs.phase), gs.round);
        let mut out = Driven { questions: Vec::new(), rendered: Vec::new(), finished: false };
        let mut status = driver.run(core, kernel)?;
        loop {
            match status {
                Status::Done => {
                    out.finished = true;
                    return Ok(out);
                }
                Status::Asking(choice) => {
                    let position = crate::render::Position { state: std::rc::Rc::new(core.clone()), kernel };
                    let mut rendered = crate::render::Typed::new_at(r, position, choice.clone(), phase, round).rendered;
                    complete(&choice, core, &mut rendered);
                    out.questions.push(choice.clone());
                    out.rendered.push(rendered);
                    let Some(id) = script.questions.get(out.questions.len() - 1).and_then(|q| q.answer.clone()) else {
                        return Ok(out);
                    };
                    let Some(pick) = crate::render::parse_pick(r, &choice.ask, &id) else {
                        return Ok(out);
                    };
                    status = driver.resume(core, kernel, pick)?;
                }
            }
        }
    }

    /// [`drive_window`] for the windows that read the production rules: the production kernel, the questions
    /// rendered in their position, the slices the payment and placement rules read.
    fn run_production<W: Window>(
        gs: &mut GameState,
        cx: &mut Ctx,
        r: &Render,
        window: W,
        seat: Seat,
        script: &Recording,
        complete: Complete<'_>,
    ) -> Option<(W, Driven)> {
        let before = gs.clone();
        let kernel = crate::production_view::kernel();
        let mut driver = crate::window_driver::driver_for(window, Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let fp = Footprint { reads: crate::production_view::READS, writes: ti4_engine_core::state::SliceSet::of(&[Slice::Board, Slice::Players, Slice::Cards, Slice::Marks]) };
        let driven = with_core(gs, cx, Sources::default(), fp, |core, _| drive_at(&mut driver, core, kernel, r, &before, script, complete))
            .why(crate::dual::reason::NOT_LENDABLE)?
            .why(crate::dual::reason::DRIVER_REFUSED)?;
        Some((driver.window, driven))
    }

    /// The string state as a question sees it mid-window: the planets exhausted and the trade goods held now.
    fn string_now(before: &GameState, core: &CoreState, seat: Seat, player: &PlayerId) -> GameState {
        let mut now = before.clone();
        let controlled: Vec<ti4_model::id::PlanetId> = before.controlled_planets(player).into_iter().map(|(_, p)| p.clone()).collect();
        for id in controlled {
            match ti4_engine_core::generated::Planet::from_id(id.as_str()).map(|p| ti4_engine_core::query::board::exhausted(core, p)) {
                Some(true) => {
                    now.exhausted_planets.insert(id);
                }
                Some(false) => {
                    now.exhausted_planets.remove(&id);
                }
                None => {}
            }
        }
        if let (Some(p), Some(held)) = (now.player_mut(player), core.players().by_seat(seat).map(|p| i32::from(p.econ.trade_goods))) {
            p.trade_goods = held;
        }
        now
    }

    /// The core half of the two Leadership abilities (`secondary`: the follower's, its first purchase accepted).
    /// `None` as for Warfare, and where the payment rules are not followed (`production_view::unported_window`:
    /// the Keleres agent and the other faction effects on payment).
    pub fn leadership(
        gs: &mut GameState,
        content: &ContentStore,
        sources: SourceSet,
        player: &PlayerId,
        primary: bool,
        script: &Recording,
    ) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script).or_else(|| (gs.tokens_held_by_mahact(player) > 0).then_some("a command token in a Mahact fleet pool (the reinforcements differ)")).or_else(|| (crate::production_view::unported_window(gs, player)).then_some("a table with a seat the production windows do not carry (Naaz, Nomad, Yssaril, Keleres, Deepwrought)")) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let letnev = gs.player(player)?.faction.as_str() == "letnev";
        let before = gs.clone();
        let gained = std::cell::Cell::new(0_usize);
        let purchase_of = |core: &CoreState| crate::strategy_cards::purchase_details(&string_now(&before, core, seat, player), content, sources, player);
        let (window, driven) = run_production(gs, &mut cx, &r, Leadership::new(seat, primary), seat, script, &|c, core, rendered| {
            complete_tokens(c, core, rendered, letnev);
            match c.ask {
                Ask::Strategy(SAsk::GainPool { .. }) => {
                    // Only the three free tokens of the primary announce the purchase that follows.
                    if primary && gained.get() < 3 {
                        if let Some(purchase) = purchase_of(core) {
                            rendered.details.insert("purchase".to_owned(), purchase);
                        }
                    }
                    gained.set(gained.get() + 1);
                }
                Ask::Strategy(SAsk::LeadershipBuy { more: false, .. }) => {
                    if let Some(purchase) = purchase_of(core) {
                        rendered.details.extend(pool_details(core, c.actor, letnev, "buy", Some(0)));
                        rendered.details.insert("purchase".to_owned(), purchase);
                    }
                }
                _ => {}
            }
        })?;
        Some(finish(&driven, &window))
    }

    // -- Construction ---------------------------------------------------------------------------------------

    /// What a Construction window needs of the position, or `None` where a faction effect changes the placement: the
    /// Titans' alternative to a PDS, Minister of Industry's production after a space dock, a unit the tables do not
    /// name, what `production_view::unported_window` declines.
    pub(crate) fn structure_facts(gs: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId, r: &Render) -> Option<StructureFacts> {
        if crate::production_view::unported_window(gs, player) || crate::laws::industry_produces_on_placement(gs, player) {
            crate::dual::note_bail_here("a seat the production windows do not carry, or Minister of Industry (a production after the dock)");
            return None;
        }
        let alternatives = gs.controlled_planets(player).into_iter().any(|(system, planet)| {
            !crate::factions::hooks_economy::pds_placement_alternatives(gs, content, sources, player, system, planet).is_empty()
        });
        if alternatives {
            crate::dual::note_bail_here("a structure the seat may place instead of a PDS (Titans' Hecatoncheires)");
            return None;
        }
        let placement = |kind: &str| -> Option<Placement> {
            let (unit, in_space) = crate::strategy_cards::construction_placement(gs, content, sources, player, kind);
            if crate::factions::hooks_economy::effect_placement_forbidden(gs, content, sources, player, &unit) {
                return Some(None);
            }
            Some(Some((crate::core_adapter::typed::<ti4_engine_core::generated::Unit>(unit.as_str())?, in_space)))
        };
        let (pds, dock) = (placement("pds")?, placement("spacedock")?);
        Some(StructureFacts { pds, dock, source: r.code("place_structure") })
    }

    /// The core half of the Construction abilities (`primary`: two structures, else the secondary's one).
    /// `None` as for Leadership, and where a faction effect changes the placement ([`structure_facts`]).
    pub fn construction(
        gs: &mut GameState,
        content: &ContentStore,
        sources: SourceSet,
        player: &PlayerId,
        primary: bool,
        script: &Recording,
    ) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let facts = structure_facts(gs, content, sources, player, &r)?;
        let (window, driven) = run_production(gs, &mut cx, &r, Construction::new(seat, primary, facts), seat, script, &|_, _, _| {})?;
        Some(finish(&driven, &window))
    }

    /// The core half of `relics::titan_prototype`, the whole card: the owner names the player, who pays three resources for
    /// a structure or takes a trade good. The value is whether a player was named (`None`: nobody) and whether a structure
    /// was placed. `None` as for Construction, and for a window the table cannot start the build in.
    pub fn titan_prototype(
        gs: &mut GameState,
        content: &ContentStore,
        sources: SourceSet,
        player: &PlayerId,
        script: &Recording,
    ) -> Option<CoreWindow<Option<bool>>> {
        use ti4_engine_core::rules::relic_windows::TitanPrototype;
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        // The builder is the answer of the first question; the structure rules are read for that seat.
        let builder = script.questions.first().and_then(|q| q.answer.clone());
        let facts = match &builder {
            Some(id) => Some(structure_facts(gs, content, sources, &PlayerId::new(id.as_str()), &r)?),
            None => None,
        };
        let window = match facts {
            Some(facts) => TitanPrototype::new(seat).with_build(facts, r.code("relic")),
            None => TitanPrototype::new(seat),
        };
        let before = gs.clone();
        let kernel = crate::production_view::kernel();
        let mut table = AbilityTable::default();
        ti4_engine_core::driver::children::register(&mut table);
        let mut driver = crate::window_driver::driver_for(window, Resolver::new(&[seat], Some(seat)), table, 1);
        let mut reads = crate::production_view::READS;
        reads.insert(Slice::Pending);
        let fp = Footprint { reads, writes: ti4_engine_core::state::SliceSet::of(&[Slice::Board, Slice::Players, Slice::Cards, Slice::Marks, Slice::Pending]) };
        let mut staged = Vec::new();
        let driven = with_core(gs, &mut cx, Sources::default(), fp, |core, _| {
            let driven = drive_at(&mut driver, core, kernel, &r, &before, script, &|_, _, _| {});
            staged = std::mem::take(&mut core.pending_mut().staged);
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        if driver.window.unsupported {
            crate::dual::note_bail_here("the table has no link to start the Titan Prototype's build");
            return None;
        }
        crate::dual_window::apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        let value = driver.window.chosen.map(|_| driver.window.built);
        Some(CoreWindow { run: Run::new(value).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    /// The core half of the primary of the Thunder's Edge Construction (`te4construction`): the choice of a structure
    /// or a production, then the structures. `systems` are the ones that can produce (the string's
    /// `production::capacity`). `None` as for [`construction`], and where the holder chooses the production: a whole
    /// production window nested in the card (the string's).
    pub fn construction_te(
        gs: &mut GameState,
        content: &ContentStore,
        sources: SourceSet,
        player: &PlayerId,
        systems: &[ti4_model::id::SystemId],
        script: &Recording,
    ) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let facts = structure_facts(gs, content, sources, player, &r)?;
        let producing: Vec<ti4_engine_core::generated::System> =
            systems.iter().map(|s| crate::core_adapter::typed::<ti4_engine_core::generated::System>(s.as_str())).collect::<Option<_>>()?;
        let window = Construction::new_te(seat, facts, r.code("te4construction"), &producing);
        let (window, driven) = run_production(gs, &mut cx, &r, window, seat, script, &|_, _, _| {})?;
        if window.produces {
            crate::dual::note_bail("strategy_cards::te4_construction", "the production of the Thunder's Edge Construction (a production window nested in the card)");
            return None;
        }
        Some(finish(&driven, &window))
    }

    // -- Thunder's Edge Warfare -----------------------------------------------------------------------------

    /// The core half of the primary of the Thunder's Edge Warfare (`te6warfare`): the redistribution before the action
    /// and the system of the free tactical action. `systems` are the string's (the map and the systems in play).
    /// The value is the chosen system. `None` as for Warfare's redistribution, inside a battle or an invasion.
    pub fn warfare_te(gs: &mut GameState, player: &PlayerId, systems: &[String], script: &Recording) -> Option<CoreWindow<Option<String>>> {
        // A command token in a Mahact fleet pool is not in the reinforcements: the core's count of them differs (found by the census of
        // `golden-lent.json`, a Mahact at the table: `details.reinforcements` one too many in 8 games of 19).
        if let Some(why) = crate::dual_window::outside_why(gs, script).or_else(|| (gs.tokens_held_by_mahact(player) > 0).then_some("a command token in a Mahact fleet pool (the reinforcements differ)")) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let letnev = gs.player(player)?.faction.as_str() == "letnev";
        let typed: Vec<ti4_engine_core::generated::System> =
            systems.iter().map(|s| crate::core_adapter::typed::<ti4_engine_core::generated::System>(s)).collect::<Option<_>>()?;
        let window = TeWarfare::new(seat, &typed, gs.rules_version >= ti4_model::state::RULES_LETNEV_FLEET_POOL, r.code("te6warfare"));
        let (window, driven) =
            drive_window(gs, &mut cx, &r, WARFARE, window, seat, script, &|c, core, rendered| complete_tokens(c, core, rendered, letnev))?;
        let chosen = window.chosen.map(|system| system.as_str().to_owned());
        Some(CoreWindow { run: Run::new(chosen).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    // -- Trade secondary ------------------------------------------------------------------------------------

    const REPLENISH: Footprint =
        Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Cards, Slice::Pending], &[Slice::Players, Slice::Cards]);

    /// The core half of the Trade secondary (`strategy_cards::replenish` for the follower): the commodities go
    /// to the printed value. `None` where a commodity change causes events the core does not hold (Cabal, a
    /// siphoning plot, a Trade Agreement), where the game's limit is not the faction's printed value, inside a
    /// battle, an invasion or a nested emission, and with diplomacy on.
    pub fn replenish_secondary(gs: &mut GameState, content: &ContentStore, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script).or_else(|| crate::leaders_core::replenish_window_unported(gs)) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let limit = crate::strategy_cards::commodity_limit(gs, content, player);
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let source = r.code("trade_agreement");
        let mut staged = Vec::new();
        let same = with_core(gs, &mut cx, Sources::default(), REPLENISH, |core, _| {
            let printed = core.players().by_seat(seat).map(|p| i32::from(p.faction.data().commodities));
            let done = (printed == Some(limit)).then(|| fx::replenish_announced(core, seat, source));
            staged = std::mem::take(&mut core.pending_mut().staged);
            done
        })
        .why(crate::dual::reason::NOT_LENDABLE)?;
        same.why("the faction's printed commodity limit is not the game's")?;
        crate::dual_window::apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        Some(CoreWindow { run: Run::new(true).with_events_vec(Vec::new()), questions: Vec::new() })
    }

    // -- Technology ----------------------------------------------------------------------------------------

    /// The seats whose own effects on a research are questions or forms the window does not carry: an own Deepwrought
    /// (Doctor Carrina's window opens around every research), Keleres (the agent's offer in the payment) or a Mentak with
    /// The Table's Grace (the Corsair's unit form, Thunder's Edge only: package W6 narrowed it from every Mentak seat, whose
    /// research has no effect of its own). The payment hooks of Naaz, Nomad and Yssaril are the production window's own (tier W).
    fn research_seat_unported(gs: &GameState, player: &PlayerId) -> bool {
        use crate::factions::{deepwrought, keleres};
        let own = [deepwrought::MODULE.alias, keleres::MODULE_M.alias, keleres::MODULE_X.alias, keleres::MODULE_A.alias];
        gs.player(player).is_some_and(|p| own.contains(&p.faction.as_str())) || crate::factions::mentak::corsair_form(gs, player)
    }

    /// Whether the position holds an effect on a research the core `Research` window does not carry: what
    /// [`research_seat_unported`] declines, a technology purged by Ta Zern, a hook the core table answers differently from the string modules (a
    /// faction effect nobody ported), and the questions of the paid research (`strategy_cards::research_asks_more`:
    /// Specialist Compounds, Doctor Sucaban, the Deepwrought commander).
    pub(crate) fn research_unported(gs: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId) -> bool {
        use ti4_engine_core::generated::Technology;
        if research_seat_unported(gs, player) || crate::strategy_cards::research_asks_more(gs, content, sources, player) {
            return true;
        }
        let hooks = crate::technology_core::StringHooks { state: gs, content, sources, player };
        use ti4_engine_core::query::tech::ResearchHooks as _;
        Technology::ALL.iter().any(|t| hooks.purged(*t))
    }

    /// The hooks of the core window against the string modules' answers, over every technology: a mismatch means a
    /// faction effect on a research (a waived prerequisite, a colour, a waiver) the core table does not carry.
    pub(crate) fn research_hooks_agree(core: &CoreState, gs: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId, seat: Seat) -> bool {
        use ti4_engine_core::generated::Technology;
        use ti4_engine_core::query::tech::ResearchHooks as _;
        let kernel = crate::production_view::kernel();
        let core_hooks = ti4_engine_core::rules::production::strategy_cards::KernelHooks::new(core, kernel, seat);
        let string_hooks = crate::technology_core::StringHooks { state: gs, content, sources, player };
        core_hooks.extra_colours() == string_hooks.extra_colours()
            && core_hooks.inheritance_affordable() == string_hooks.inheritance_affordable()
            && Technology::ALL.iter().all(|t| {
                core_hooks.waived_prerequisites(*t) == string_hooks.waived_prerequisites(*t) && core_hooks.waiver_offer(*t) == string_hooks.waiver_offer(*t)
            })
    }

    /// The core half of the two Technology abilities (`primary`: the free research and the six-resource one,
    /// else the secondary's four-resource one). `None` as for Leadership, and where [`research_unported`]
    /// says so, below `RULES_TECH_PAYMENT_CHOICE` (the plan is not asked), where the faction hooks of the core and
    /// the string differ, and where an answer leaves the carried paths (a technology only a faction waiver opens).
    pub fn research(
        gs: &mut GameState,
        content: &ContentStore,
        sources: SourceSet,
        player: &PlayerId,
        primary: bool,
        cost: i64,
        script: &Recording,
    ) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script)
            .or_else(|| (gs.rules_version < ti4_model::state::RULES_TECH_PAYMENT_CHOICE).then_some("a game below the technology payment choice"))
            .or_else(|| research_unported(gs, content, sources, player).then_some("a seat whose own effects on a research are questions or forms (Deepwrought, Keleres, a Mentak with The Table's Grace, Last Bastion, Crimson) or a purged technology"))
        {
            crate::dual::note_bail_here(why);
            return None;
        }
        let roster = *crate::technology_core::roster_for(content, sources).why("the unit roster of the sources is not known")?;
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let scope = crate::technology_core::scope(sources);
        let before = gs.clone();
        let agree = with_core(gs, &mut cx, Sources::default(), Footprint::reads(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Agenda, Slice::Marks, Slice::Strategy, Slice::Cards, Slice::Trade, Slice::Transient, Slice::Explore]), |core, _| {
            research_hooks_agree(core, &before, content, sources, player, seat)
        })
        .why(crate::dual::reason::NOT_LENDABLE)?;
        if !agree {
            crate::dual::note_bail_here("the research hooks of the string and the core disagree (a hook the core does not carry)");
            return None;
        }
        let (window, driven) = run_production(gs, &mut cx, &r, Research::new(seat, primary, cost, scope).with_roster(roster), seat, script, &|c, _, rendered| {
            if matches!(c.ask, Ask::Strategy(SAsk::ResearchTechnology { .. })) {
                for (option, opt) in rendered.options.iter_mut().zip(c.opts.iter()) {
                    if let ti4_engine_core::choice::Opt::Strategy(ti4_engine_core::choice::strategy::Opt::Research(t)) = opt {
                        option.label = crate::technology::name(content, &ti4_model::id::TechnologyId::new(t.as_str()));
                    }
                }
            }
        })?;
        if window.failed {
            crate::dual::note_bail_here("the research window left the path the core carries (a research only a faction waiver opens)");
            return None;
        }
        Some(finish(&driven, &window))
    }

    // -- Imperial ------------------------------------------------------------------------------------------

    const SECRET: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Objectives, Slice::Decks, Slice::Pending],
        &[Slice::Players, Slice::Decks],
    );

    /// The core table of a window that runs the hand limit as a child: the links of the areas are installed.
    fn linked() -> AbilityTable {
        crate::reactions_core::linked_table()
    }

    /// The core half of Imperial's secret objective draw (`secrets::draw` called by the secondary and by the
    /// primary of a seat that does not hold Mecatol Rex): the draw and the hand limit's question (which unscored
    /// secret goes back to the deck, 45.4) as one window. `None` where a reserve is replaying draws or the
    /// window is nested in a battle.
    pub fn draw_secret(gs: &mut GameState, _content: &ContentStore, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script).or_else(|| (gs.deck_reserve.is_some()).then_some("deck reserve (a turn redo replays the draws)")) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let before = gs.clone();
        let kernel = Kernel::plain();
        let mut driver = crate::window_driver::driver_for(ImperialSecondary::new(seat), Resolver::new(&[seat], Some(seat)), linked(), 1);
        let driven = with_core(gs, &mut cx, Sources::default(), SECRET, |core, _| drive_with(&mut driver, core, &kernel, &r, &before, script, &|_, _, _| {}))
            .why(crate::dual::reason::NOT_LENDABLE)?
            .why(crate::dual::reason::DRIVER_REFUSED)?;
        if driver.window.unsupported {
            crate::dual::note_bail_here("the table has no link to the hand limit window");
            return None;
        }
        Some(finish(&driven, &driver.window))
    }

    /// The core half of Imperial's primary (`strategy_cards::imperial_primary`). The scoreable objectives are
    /// the string's (`objectives::scoreable_on`); the window scores the one chosen (paying a bought objective's price
    /// by the first plan), and the leaders it unlocks and Yin's breakthrough are the string's tail
    /// (`objectives::award_tail`), run here after the window. `None` for a Keleres seat (Custodian's Favour asks), a
    /// point that would pass the target, an objective that is no public objective of the tables or whose points the
    /// tables read otherwise, a bought objective at a table whose payment the production kernel does not follow
    /// (Bastion, Crimson, ocean cards), a secret draw that is not plain, and a deck reserve.
    pub fn imperial_primary(
        gs: &mut GameState,
        content: &ContentStore,
        sources: SourceSet,
        galaxy: Option<&ti4_content::galaxy::Galaxy>,
        player: &PlayerId,
        script: &Recording,
    ) -> Option<CoreWindow<bool>> {
        use ti4_engine_core::generated::PublicObjective;
        if let Some(why) = crate::dual_window::outside_why(gs, script).or_else(|| (gs.deck_reserve.is_some()).then_some("deck reserve (a turn redo replays the draws)")).or_else(|| (gs.players.iter().any(|p| p.faction.as_str().starts_with("keleres"))).then_some("a Keleres seat (non-lobby)")) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let scoreable = crate::objectives::scoreable_on(gs, content, sources, player, galaxy);
        let typed: Vec<PublicObjective> = scoreable
            .iter()
            .map(|o| crate::core_adapter::typed::<PublicObjective>(o.as_str()))
            .collect::<Option<_>>()
            .why("a scoreable objective the tables do not know")?;
        let mut most = 0;
        let mut priced = false;
        for (alias, objective) in scoreable.iter().zip(&typed) {
            let points = i32::from(ti4_engine_core::query::objectives::points_of(*objective));
            if crate::objectives::points_for(content, alias) != Some(points) {
                crate::dual::note_bail_here("an objective whose points the tables read otherwise");
                return None;
            }
            most = most.max(points);
            priced |= ti4_engine_core::query::objectives::cost_of(*objective).is_some();
        }
        if priced && price_is_string(gs, player) {
            crate::dual::note_bail_here("a bought objective at a table whose payment the production kernel does not follow (ocean cards)");
            return None;
        }
        let controls = gs.controlled_planets(player).into_iter().any(|(system, _)| crate::seating::is_mecatol(system.as_str()));
        if controls {
            if gs.player(player).why(crate::dual::reason::NO_SEAT)?.victory_points + most + 1 > crate::objectives::VICTORY_TARGET {
                crate::dual::note_bail_here("a point that would pass the victory target");
                return None;
            }
        } else {
            if gs.player(player).why(crate::dual::reason::NO_SEAT)?.victory_points + most > crate::objectives::VICTORY_TARGET {
                crate::dual::note_bail_here("a point that would pass the victory target");
                return None;
            }
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let before = gs.clone();
        let held = crate::secrets::held_count(gs, content, player);
        let max = crate::secrets::HAND_LIMIT + crate::relics::secret_objective_bonus(gs, player);
        // The production kernel answers what a trade good pays and which commodities a payment may spend.
        let env = ti4_engine_core::kernel::fx_pay::PayEnv { scope: crate::core_adapter::scope(sources), blanked: crate::laws::blanked(gs) };
        let window = ImperialPrimary::new(seat, typed.into_iter().collect()).with_price(env);
        let mut driver = crate::window_driver::driver_for(window, Resolver::new(&[seat], Some(seat)), linked(), 1);
        let kernel = crate::production_view::kernel();
        let fp = Footprint::new(
            &[
                Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Decks, Slice::Ledger, Slice::Pending, Slice::Objectives,
                Slice::Agenda, Slice::Marks, Slice::Strategy, Slice::Cards, Slice::Trade, Slice::Transient, Slice::Explore,
            ],
            &[Slice::Players, Slice::Board, Slice::Decks, Slice::Ledger, Slice::Objectives],
        );
        let driven = with_core(gs, &mut cx, Sources::default(), fp, |core, _| {
            drive_with(&mut driver, core, kernel, &r, &before, script, &|c, _, rendered| {
                if matches!(c.ask, Ask::Strategy(SAsk::ImperialScore)) {
                    rendered.details.insert("kind".to_owned(), "imperial".into());
                    rendered.details.insert("controls_mecatol".to_owned(), controls.into());
                    rendered.details.insert("secrets_held".to_owned(), held.into());
                    rendered.details.insert("secrets_max".to_owned(), max.into());
                }
            })
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        if driver.window.unsupported {
            crate::dual::note_bail_here("the table has no link to the hand limit window");
            return None;
        }
        if let Some(objective) = driver.window.scored {
            crate::objectives::award_tail(gs, content, sources, player.clone(), &ti4_model::id::ObjectiveId::new(objective.as_str()));
        }
        gs.finished = crate::objectives::winner(gs).is_some();
        Some(finish(&driven, &driver.window))
    }

    /// Where the price of a bought objective stays with the string: a table with a Last Bastion or a Crimson seat
    /// (their economy hooks are not in the production kernel's reads), and a payer holding Deepwrought ocean cards.
    fn price_is_string(gs: &GameState, payer: &PlayerId) -> bool {
        use crate::factions::{bastion, crimson};
        gs.players.iter().any(|p| [bastion::MODULE.alias, crimson::MODULE.alias].contains(&p.faction.as_str()))
            || !crate::factions::deepwrought::spendable_oceans(gs, payer).is_empty()
    }

    // -- Politics ------------------------------------------------------------------------------------------

    /// Why a faction hook or a law changes `player`'s draw of `count` action cards (`None`: the core window follows it, with
    /// the hand limit's discard question when the hand ends over its limit, package W6). Yssaril's Scheming (a card more, then
    /// a discard) and Crafty (no limit) are the drawer's own, the Deepgloom Executable lets another seat's Scheming be borrowed
    /// (Thunder's Edge), a test hook may add a card, and a turn with blank laws (Law's Order) lifts Sanctions' cap.
    pub(crate) fn draw_hook(gs: &GameState, content: &ContentStore, player: &PlayerId, count: usize) -> Option<&'static str> {
        if crate::laws::blanked(gs) {
            Some("a turn in which the laws are blank (Law's Order)")
        } else if gs.players.iter().any(|p| p.faction.as_str() == "ralnel") {
            Some("a Ral Nel seat: its Data Skimmer could take the discard (non-lobby)")
        } else if gs.player(player).is_some_and(|p| p.faction.as_str() == "yssaril") {
            Some("Yssaril's Scheming and Crafty change the draw and the limit")
        } else if gs.players.iter().any(|p| crate::breakthroughs::holds(gs, &p.id, "yssarilbt")) {
            Some("the Deepgloom Executable lets another seat borrow Scheming")
        } else if crate::factions::hooks_economy::action_card_draw_bonus(gs, content, player, count) != 0 {
            Some("a bonus card with the draw")
        } else if crate::factions::hooks_economy::action_card_limit(gs, content, player, crate::laws::action_card_limit(gs, crate::action_cards::HAND_LIMIT))
            != crate::laws::action_card_limit(gs, crate::action_cards::HAND_LIMIT)
        {
            Some("a faction changes the action card hand limit")
        } else {
            None
        }
    }

    /// Whether `player` draws `count` action cards with no hook: the core's draw window follows it (the hand limit's discard
    /// question included).
    pub(crate) fn plain_draw(gs: &GameState, content: &ContentStore, player: &PlayerId, count: usize) -> bool {
        draw_hook(gs, content, player, count).is_none()
    }

    /// [`plain_draw`] and the hand stays within its limit: no question is asked (the draws that are not windows of their own,
    /// Exploration Probe's Lost Crew).
    pub(crate) fn plain_draw_within_limit(gs: &GameState, content: &ContentStore, player: &PlayerId, count: usize) -> bool {
        plain_draw(gs, content, player, count) && gs.player(player).is_some_and(|p| p.action_cards.len() + count <= crate::action_cards::hand_limit(gs, content, player))
    }

    /// The display facts of the Politics questions: the seat of each speaker option and the printed agenda.
    fn complete_politics(r: &Render, content: &ContentStore, c: &CoreChoice, rendered: &mut Choice) {
        match c.ask {
            Ask::Strategy(SAsk::PoliticsSpeaker) => {
                let mut seats = serde_json::Map::new();
                for opt in &rendered.options {
                    if let Some(found) = (0..r.seats.len()).filter_map(|i| u8::try_from(i).ok()).map(Seat).find(|s| r.faction_text(*s) == opt.id) {
                        seats.insert(opt.id.clone(), Value::from(r.seat_text(found)));
                    }
                }
                rendered.details.insert("seats".to_owned(), Value::Object(seats));
            }
            Ask::Strategy(SAsk::PoliticsAgenda { agenda }) => {
                rendered.details.insert("agenda".to_owned(), crate::strategy_cards::agenda_details(content, agenda.as_str()));
            }
            _ => {}
        }
    }

    /// The core half of the two Politics abilities. `None` as for Warfare, with two seats of one faction (the
    /// speaker options are named by faction) and where the draw is not plain.
    pub fn politics(gs: &mut GameState, content: &ContentStore, player: &PlayerId, primary: bool, script: &Recording) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script).or_else(|| (gs.deck_reserve.is_some()).then_some("deck reserve (a turn redo replays the draws)")).or_else(|| draw_hook(gs, content, player, 2)) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = named_tables(gs)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let (window, driven) = drive_window(gs, &mut cx, &r, POLITICS, Politics::new(seat, primary, gs.rules_version >= ti4_model::state::RULES_OFFICIAL_ACTION_DECK), seat, script, &|c, _, rendered| {
            complete_politics(&r, content, c, rendered);
        })?;
        if window.unsupported {
            crate::dual::note_bail_here("the table has no link to the draw window or no printed-name groups");
            return None;
        }
        Some(finish(&driven, &window))
    }

    // -- Performed by an action card (Strategize, Overrule) ------------------------------------------------

    /// The display facts of the questions a strategy card's ability asks when an action card performs it, which only the string
    /// position knows (the closures of the windows above, for the card the holder picked): the seat that holds each card on offer,
    /// the purchase the free tokens of the Leadership primary announce, the readying hints of Diplomacy, the seats and agendas of
    /// Politics, the commodities of Trade and the names of the technologies. The pool details and previews of the token questions are
    /// [`complete_tokens`], applied by the caller.
    pub(crate) struct Performing<'a> {
        pub(crate) before: &'a GameState,
        pub(crate) content: &'a ContentStore,
        pub(crate) sources: SourceSet,
        pub(crate) player: &'a PlayerId,
        pub(crate) seat: Seat,
        pub(crate) primary: bool,
        pub(crate) r: &'a Render,
        /// The free tokens of the Leadership primary that still announce the purchase that follows them (the only ability that starts
        /// a token gain at three; the Warfare primary gains one).
        pub(crate) free: std::cell::Cell<u8>,
    }

    impl Performing<'_> {
        pub(crate) fn complete(&self, c: &CoreChoice, core: &CoreState, rendered: &mut Choice) {
            use ti4_engine_core::choice::cards::{Ask as CAsk, Opt as COpt, What};
            let letnev = self.before.player(self.player).is_some_and(|p| p.faction.as_str() == "letnev");
            let purchase_of = || {
                crate::strategy_cards::purchase_details(&string_now(self.before, core, self.seat, self.player), self.content, self.sources.clone(), self.player)
            };
            match c.ask {
                Ask::Cards(CAsk::Pick { what: What::Perform, .. }) => {
                    let mut holders = serde_json::Map::new();
                    for opt in c.opts.iter() {
                        if let ti4_engine_core::choice::Opt::Cards(COpt::Perform { card, holder }) = opt {
                            holders.insert(card.as_str().to_owned(), holder.map_or(Value::Null, |seat| Value::from(self.r.seat_text(seat))));
                        }
                    }
                    rendered.details.insert("holders".to_owned(), Value::Object(holders));
                }
                Ask::Strategy(SAsk::GainPool { left }) => {
                    if self.primary && u32::from(left) == crate::strategy_cards::LEADERSHIP_TOKENS {
                        self.free.set(u8::try_from(crate::strategy_cards::LEADERSHIP_TOKENS).unwrap_or(3));
                    }
                    if self.free.get() > 0 {
                        self.free.set(self.free.get() - 1);
                        if let Some(purchase) = purchase_of() {
                            rendered.details.insert("purchase".to_owned(), purchase);
                        }
                    }
                }
                Ask::Strategy(SAsk::LeadershipBuy { more: false, .. }) => {
                    if let Some(purchase) = purchase_of() {
                        rendered.details.extend(pool_details(core, c.actor, letnev, "buy", Some(0)));
                        rendered.details.insert("purchase".to_owned(), purchase);
                    }
                }
                Ask::Strategy(SAsk::ReadyPlanet) => {
                    rendered.details.insert("ready_hints".to_owned(), ready_hints_now(self.before, core, self.content, self.sources.clone(), self.player));
                }
                Ask::Strategy(SAsk::PoliticsSpeaker | SAsk::PoliticsAgenda { .. }) => complete_politics(self.r, self.content, c, rendered),
                Ask::Strategy(SAsk::TradeReplenish) => crate::strategy_core::complete_trade(self.r, core, rendered),
                Ask::Strategy(SAsk::ResearchTechnology { .. }) => {
                    for (option, opt) in rendered.options.iter_mut().zip(c.opts.iter()) {
                        if let ti4_engine_core::choice::Opt::Strategy(ti4_engine_core::choice::strategy::Opt::Research(t)) = opt {
                            option.label = crate::technology::name(self.content, &ti4_model::id::TechnologyId::new(t.as_str()));
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(not(feature = "typed-core"))]
mod stub {
    use ti4_content::ContentStore;
    use ti4_model::content_types::SourceSet;
    use ti4_model::id::PlayerId;
    use ti4_model::state::GameState;

    use crate::dual::CoreWindow;
    use crate::dual_window::Recording;

    pub fn warfare_primary(_: &mut GameState, _: &PlayerId, _: &Recording) -> Option<CoreWindow<bool>> {
        None
    }

    pub fn diplomacy(
        _: &mut GameState,
        _: &ContentStore,
        _: SourceSet,
        _: &PlayerId,
        _: &str,
        _: bool,
        _: &Recording,
    ) -> Option<CoreWindow<bool>> {
        None
    }

    pub fn politics(_: &mut GameState, _: &ContentStore, _: &PlayerId, _: bool, _: &Recording) -> Option<CoreWindow<bool>> {
        None
    }

    pub fn draw_secret(_: &mut GameState, _: &ContentStore, _: &PlayerId, _: &Recording) -> Option<CoreWindow<bool>> {
        None
    }

    pub fn research(
        _: &mut GameState,
        _: &ContentStore,
        _: SourceSet,
        _: &PlayerId,
        _: bool,
        _: i64,
        _: &Recording,
    ) -> Option<CoreWindow<bool>> {
        None
    }

    pub fn replenish_secondary(_: &mut GameState, _: &ContentStore, _: &PlayerId, _: &Recording) -> Option<CoreWindow<bool>> {
        None
    }

    pub fn imperial_primary(
        _: &mut GameState,
        _: &ContentStore,
        _: SourceSet,
        _: Option<&ti4_content::galaxy::Galaxy>,
        _: &PlayerId,
        _: &Recording,
    ) -> Option<CoreWindow<bool>> {
        None
    }

    pub fn construction(
        _: &mut GameState,
        _: &ContentStore,
        _: SourceSet,
        _: &PlayerId,
        _: bool,
        _: &Recording,
    ) -> Option<CoreWindow<bool>> {
        None
    }

    pub fn titan_prototype(_: &mut GameState, _: &ContentStore, _: SourceSet, _: &PlayerId, _: &Recording) -> Option<CoreWindow<Option<bool>>> {
        None
    }

    pub fn construction_te(
        _: &mut GameState,
        _: &ContentStore,
        _: SourceSet,
        _: &PlayerId,
        _: &[ti4_model::id::SystemId],
        _: &Recording,
    ) -> Option<CoreWindow<bool>> {
        None
    }

    pub fn warfare_te(_: &mut GameState, _: &PlayerId, _: &[String], _: &Recording) -> Option<CoreWindow<Option<String>>> {
        None
    }

    pub fn leadership(
        _: &mut GameState,
        _: &ContentStore,
        _: SourceSet,
        _: &PlayerId,
        _: bool,
        _: &Recording,
    ) -> Option<CoreWindow<bool>> {
        None
    }
}

#[cfg(all(test, feature = "typed-core"))]
mod tests {
    use ti4_content::ContentStore;
    use ti4_model::content_types::{DEFAULT, POK};
    use ti4_model::id::{ObjectiveId, PlanetId, PlayerId, SecretObjectiveId, SystemId};
    use ti4_model::state::GameState;

    use crate::choice::{Scripted, Table};
    use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};

    /// The windows of `label` this thread ran: the shared report also holds what other test threads ran under a
    /// global `TI4_AREAS`.
    fn counts(label: &str) -> (u64, u64) {
        crate::dual::thread_window_counts(label)
    }

    const F3: [(&str, &str); 3] = [("a", "sol"), ("b", "hacan"), ("c", "arborec")];

    /// Run `act` on a seated game under the `Dual` strategy backend; returns (compared, bailed) of `label` and
    /// asserts the two sides agreed.
    fn run(
        label: &str,
        factions: &[(&str, &str)],
        setup: impl FnOnce(&mut GameState),
        answers: &[&str],
        act: impl FnOnce(&mut GameState, &mut Table),
    ) -> (u64, u64) {
        run_in(POK, label, factions, setup, answers, act)
    }

    fn run_in(
        sources: ti4_model::content_types::SourceSet,
        label: &str,
        factions: &[(&str, &str)],
        setup: impl FnOnce(&mut GameState),
        answers: &[&str],
        act: impl FnOnce(&mut GameState, &mut Table),
    ) -> (u64, u64) {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        crate::dual::reset_thread_windows();
        with_mask(AreaMask::STRING.with(Area::Strategy, Backend::Dual), || {
            let mut state = crate::fixtures::seated_game(factions, sources);
            // The notes are dealt to the factions once they are seated (the lobby does this; the typed cards
            // slice has no generic faction).
            crate::promissory::deal(&mut state, ContentStore::embedded(), sources);
            setup(&mut state);
            let mut table = Table::with_default(Box::new(Scripted::new(answers.iter().map(|a| (*a).to_owned()))));
            act(&mut state, &mut table);
        });
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let c = counts(label);
        let _ = take_report();
        c
    }

    fn a() -> PlayerId {
        PlayerId::new("a")
    }

    fn primary(card: &'static str) -> impl FnOnce(&mut GameState, &mut Table) {
        move |state, table| {
            crate::strategy_cards::primary(state, ContentStore::embedded(), POK, None, table, &a(), card).unwrap();
        }
    }

    fn secondary(card: &'static str) -> impl FnOnce(&mut GameState, &mut Table) {
        secondary_in(POK, card)
    }

    fn secondary_in(sources: ti4_model::content_types::SourceSet, card: &'static str) -> impl FnOnce(&mut GameState, &mut Table) {
        move |state, table| {
            crate::strategy_cards::secondary(state, ContentStore::embedded(), sources, None, table, &a(), card).unwrap();
        }
    }

    fn token_in(state: &mut GameState, system: &str) {
        state.system_mut(&SystemId::new(system)).command_tokens.insert(a());
    }

    fn exhausted_planet(state: &mut GameState, system: &str, planet: &str) {
        state.system_mut(&SystemId::new(system)).set_control(PlanetId::new(planet), a());
        state.exhausted_planets.insert(PlanetId::new(planet));
    }

    #[test]
    fn warfare_recalls_gains_and_redistributes_in_lockstep() {
        let label = "strategy_cards::warfare_primary";
        let setup = |s: &mut GameState| {
            token_in(s, "26");
            token_in(s, "27");
        };
        // The token goes to the fleet pool, then the sheet is split anew.
        assert_eq!(run(label, &F3, setup, &["26", "fleet_tokens", "2|2|5"], primary("pok6warfare")).0, 1);
        assert_eq!(run(label, &F3, setup, &["27", "tactic_tokens", "0|9|0"], primary("pok6warfare")).0, 1);
        assert_eq!(run(label, &F3, setup, &["27", "strategic_tokens", "4|2|3"], primary("pok6warfare")).0, 1);
        // No command token on the board: no question at all.
        assert_eq!(run(label, &F3, |_| {}, &[], primary("pok6warfare")).0, 1);
        // A Letnev may empty the fleet pool.
        let letnev = [("a", "letnev"), ("b", "hacan")];
        assert_eq!(run(label, &letnev, setup, &["26", "tactic_tokens", "0|0|9"], primary("pok6warfare")).0, 1);
    }

    #[test]
    fn warfare_declines_with_a_token_in_a_mahact_pool() {
        let label = "strategy_cards::warfare_primary";
        let held = |s: &mut GameState| {
            token_in(s, "26");
            s.faction_marks.insert("mahact:fleet:b".to_owned(), "a".to_owned());
        };
        assert_eq!(run(label, &F3, held, &["26", "tactic_tokens", "3|3|3"], primary("pok6warfare")), (0, 1));
    }

    #[test]
    fn diplomacy_places_tokens_and_readies_up_to_two_planets() {
        let label = "strategy_cards::diplomacy_primary";
        let setup = |s: &mut GameState| {
            s.exhausted_planets.insert(PlanetId::new("jord"));
            exhausted_planet(s, "26", "lodor");
        };
        assert_eq!(run(label, &F3, setup, &["26", "lodor", "jord"], primary("pok2diplomacy")).0, 1);
        assert_eq!(run(label, &F3, setup, &["01", "jord", "lodor"], primary("pok2diplomacy")).0, 1);
        // Nothing exhausted: the system question, then no planet.
        assert_eq!(run(label, &F3, |_| {}, &["01"], primary("pok2diplomacy")).0, 1);
        // The base printing readies every exhausted planet of the chosen system, asking nothing more.
        assert_eq!(run(label, &F3, setup, &["26"], primary("base2")).0, 1);
        // The secondary readies up to two planets, nothing else.
        let label = "strategy_cards::diplomacy_secondary";
        assert_eq!(run(label, &F3, setup, &["lodor", "jord"], secondary("pok2diplomacy")).0, 1);
        assert_eq!(run(label, &F3, |_| {}, &[], secondary("pok2diplomacy")).0, 1);
    }

    #[test]
    fn politics_elects_a_speaker_draws_and_puts_the_agendas_back() {
        let label = "strategy_cards::politics_primary";
        assert_eq!(run(label, &F3, |_| {}, &["arborec", "top", "bottom"], primary("pok3politics")).0, 1);
        assert_eq!(run(label, &F3, |_| {}, &["hacan", "bottom", "bottom"], primary("pok3politics")).0, 1);
        // An empty agenda deck: nothing to place.
        assert_eq!(run(label, &F3, |s| s.agenda_deck.clear(), &["hacan"], primary("pok3politics")).0, 1);
        let label = "strategy_cards::politics_secondary";
        assert_eq!(run(label, &F3, |_| {}, &[], secondary("pok3politics")).0, 1);
    }

    #[test]
    fn politics_declines_where_a_hook_changes_the_draw() {
        let label = "strategy_cards::politics_secondary";
        // Scheming is the drawer's own: a Yssaril holder of Politics draws and discards the string's way.
        let yssaril = [("a", "yssaril"), ("b", "sol")];
        assert_eq!(run(label, &yssaril, |_| {}, &[], secondary("pok3politics")).1, 1);
        // Another seat's Yssaril changes nothing for a Sol drawer: the core follows it (W6; it was a bail).
        let beside = [("a", "sol"), ("b", "yssaril")];
        assert_eq!(run(label, &beside, |_| {}, &[], secondary("pok3politics")), (1, 0));
        // A Ral Nel seat (never seated in the lobby): a hand-limit discard could go to its Data Skimmer.
        let ralnel = [("a", "sol"), ("b", "ralnel")];
        assert_eq!(run(label, &ralnel, |_| {}, &[], secondary("pok3politics")).1, 1);
        // Two seats of one faction: the speaker options would clash.
        let twins = [("a", "sol"), ("b", "hacan"), ("c", "hacan")];
        assert_eq!(run("strategy_cards::politics_primary", &twins, |_| {}, &["hacan"], primary("pok3politics")).1, 1);
        // Law's Order: a turn with blank laws lifts Sanctions' cap, which the window does not read from the string marks.
        let blank = |s: &mut GameState| {
            s.phase = ti4_model::state::Phase::Action;
            crate::laws::blank_for_turn(s);
        };
        assert_eq!(run(label, &F3, blank, &[], secondary("pok3politics")), (0, 1));
    }

    /// W6 regression: a Politics draw that ends over the action card hand limit was a bail ("a draw that is not plain", 38 + 32 of
    /// 91 + 71 per 20 golden games); the core window asks which card goes (one option per printed name, in the order of the names).
    #[test]
    fn politics_discards_over_the_action_card_hand_limit_in_the_core_window() {
        let full = |s: &mut GameState| {
            let cards: Vec<_> = s.action_card_deck.drain(..6).collect();
            s.player_mut(&a()).unwrap().action_cards = cards;
        };
        assert_eq!(run("strategy_cards::politics_secondary", &F3, full, &["0"], secondary("pok3politics")), (1, 0));
        // The primary draws between the speaker and the agendas: speaker, discard, then the two agendas.
        let answers = ["hacan", "0", "top", "top"];
        assert_eq!(run("strategy_cards::politics_primary", &F3, full, &answers, primary("pok3politics")), (1, 0));
        // Sanctions caps the hand at three: five cards and two drawn leave four to discard.
        let sanctions = |s: &mut GameState| {
            let cards: Vec<_> = s.action_card_deck.drain(..3).collect();
            s.player_mut(&a()).unwrap().action_cards = cards;
            s.enact_law("sanctions", "a");
        };
        assert_eq!(run("strategy_cards::politics_secondary", &F3, sanctions, &["0", "0"], secondary("pok3politics")), (1, 0));
    }

    fn goods(n: i32) -> impl Fn(&mut GameState) {
        move |s| s.player_mut(&a()).unwrap().trade_goods = n
    }

    #[test]
    fn leadership_gains_three_tokens_and_buys_more_in_lockstep() {
        let label = "strategy_cards::leadership_primary";
        let free = ["tactic_tokens", "tactic_tokens", "fleet_tokens"];
        // Two purchases: a planet and a trade good, then three trade goods.
        let two = [&free[..], &["yes", "exhaust|jord", "fleet_tokens", "yes", "strategic_tokens"]].concat();
        assert_eq!(run(label, &F3, goods(4), &two, primary("pok1leadership")).0, 1);
        // The holder stops after the free tokens.
        let none = [&free[..], &["no"]].concat();
        assert_eq!(run(label, &F3, goods(4), &none, primary("pok1leadership")).0, 1);
        // Nothing to pay with (the home planet spent, no goods): the free tokens only.
        let broke = |s: &mut GameState| {
            s.player_mut(&a()).unwrap().trade_goods = 0;
            s.exhausted_planets.insert(PlanetId::new("jord"));
        };
        assert_eq!(run(label, &F3, broke, &free, primary("pok1leadership")).0, 1);
        // Paid by goods alone, one by one (each lone option is taken without a question).
        let goods_only = |s: &mut GameState| {
            goods(3)(s);
            s.exhausted_planets.insert(PlanetId::new("jord"));
        };
        let answers = [&free[..], &["yes", "tactic_tokens"]].concat();
        assert_eq!(run(label, &F3, goods_only, &answers, primary("pok1leadership")).0, 1);
    }

    #[test]
    fn leadership_secondary_pays_the_accepted_purchase_then_asks_again() {
        let label = "strategy_cards::leadership_secondary";
        let answers = ["trade_good", "trade_good", "exhaust|jord", "fleet_tokens", "no"];
        assert_eq!(run(label, &F3, goods(4), &answers, secondary("pok1leadership")).0, 1);
        // Too poor to pay the first purchase: nothing at all.
        let broke = |s: &mut GameState| {
            s.player_mut(&a()).unwrap().trade_goods = 0;
            s.exhausted_planets.insert(PlanetId::new("jord"));
        };
        assert_eq!(run(label, &F3, broke, &[], secondary("pok1leadership")).0, 1);
    }

    #[test]
    fn leadership_declines_where_payment_is_not_followed() {
        let label = "strategy_cards::leadership_primary";
        let free = ["tactic_tokens", "tactic_tokens", "fleet_tokens", "no"];
        // Argent's agent is an ability of the core production window since the reactions package: the
        // table with an Argent seat is followed, not declined.
        let argent = [("a", "sol"), ("b", "argent")];
        assert_eq!(run(label, &argent, goods(4), &free, primary("pok1leadership")), (1, 0));
        // Package W5: the Naaz, Nomad and Yssaril seats (own or at the table) are followed too: the payment window runs them in lockstep.
        for table in [[("a", "sol"), ("b", "yssaril")], [("a", "sol"), ("b", "nomad")], [("a", "naaz"), ("b", "sol")], [("a", "yssaril"), ("b", "nomad")], [("a", "mentak"), ("b", "sol")]] {
            assert_eq!(run(label, &table, goods(4), &free, primary("pok1leadership")), (1, 0), "{table:?}");
        }
        // Keleres' agent offer inside the payment stays string (the faction is dropped by decision).
        let keleres = [("a", "keleresm"), ("b", "sol")];
        assert_eq!(run(label, &keleres, goods(4), &free, primary("pok1leadership")), (0, 1));
    }

    #[test]
    fn imperial_draws_a_secret_objective_in_lockstep() {
        let label = "strategy_cards::imperial_secret";
        assert_eq!(run(label, &F3, |_| {}, &[], secondary("pok8imperial")).0, 1);
        // The primary of a seat without Mecatol Rex draws one too (nothing is scoreable at the start).
        assert_eq!(run("strategy_cards::imperial_primary", &F3, |_| {}, &[], primary("pok8imperial")).0, 1);
        // A full hand: the draw ends in a question to hand one back, which the core window asks too (W6: it was a bail).
        assert_eq!(run(label, &F3, full_hand, &["eap"], secondary("pok8imperial")), (1, 0));
        // The primary of a seat without Mecatol Rex draws one as well and returns one.
        assert_eq!(run("strategy_cards::imperial_primary", &F3, full_hand, &["eap"], primary("pok8imperial")), (1, 0));
        // Scored secrets count towards the limit and cannot go back: one in hand, two scored, one drawn.
        let scored = |s: &mut GameState| {
            s.player_mut(&a()).unwrap().secret_objectives = vec![SecretObjectiveId::new("eap")];
            s.secret_deck.insert(0, SecretObjectiveId::new("ctr"));
            s.record_score(&a(), ObjectiveId::new("fwm"));
            s.record_score(&a(), ObjectiveId::new("gamf"));
        };
        assert_eq!(run(label, &F3, scored, &["eap"], secondary("pok8imperial")), (1, 0));
        // Every one scored: nothing to return, no question.
        let all_scored = |s: &mut GameState| {
            s.player_mut(&a()).unwrap().secret_objectives.clear();
            s.secret_deck.insert(0, SecretObjectiveId::new("ctr"));
            for o in ["fwm", "gamf", "faa"] {
                s.record_score(&a(), ObjectiveId::new(o));
            }
        };
        assert_eq!(run(label, &F3, all_scored, &[], secondary("pok8imperial")), (1, 0));
    }

    /// Three unscored secrets in hand (`eap`, `fwm`, `gamf`) and `ctr` on top of the deck.
    fn full_hand(s: &mut GameState) {
        s.player_mut(&a()).unwrap().secret_objectives = ["eap", "fwm", "gamf"].map(SecretObjectiveId::new).to_vec();
        s.secret_deck.insert(0, SecretObjectiveId::new("ctr"));
    }

    /// W6 regression: the hand limit's question inside Imperial's draw was a bail ("the hand would pass its limit", 146 of 290
    /// secondaries per 20 golden games). The core window asks it; a wrong answer is the string's refusal on both sides, the
    /// Obsidian's lifted limit asks nothing, and a seat without a link to the hand-limit window is declined, not silently wrong.
    #[test]
    fn imperial_returns_a_secret_over_the_hand_limit_in_the_core_window() {
        let label = "strategy_cards::imperial_secret";
        let (compared, bailed) = run(label, &F3, full_hand, &["fwm"], secondary("pok8imperial"));
        assert_eq!((compared, bailed), (1, 0));
        // The Obsidian lifts the limit by one: no question (the Scripted decider would fail on an unexpected one only if asked).
        let obsidian = |s: &mut GameState| {
            full_hand(s);
            s.player_mut(&a()).unwrap().relics = vec![ti4_model::id::RelicId::new("obsidian")];
        };
        assert_eq!(run(label, &F3, obsidian, &[], secondary("pok8imperial")), (1, 0));
    }

    fn lodor(s: &mut GameState) {
        s.system_mut(&SystemId::new("26")).set_control(PlanetId::new("lodor"), a());
    }

    #[test]
    fn construction_places_structures_in_lockstep() {
        let label = "strategy_cards::construction_primary";
        // Two structures: a PDS, then another PDS (the second may only be a PDS).
        assert_eq!(run(label, &F3, lodor, &["pds|01|jord", "pds|26|lodor"], primary("pok4construction")).0, 1);
        assert_eq!(run(label, &F3, lodor, &["spacedock|26|lodor", "pds|26|lodor"], primary("pok4construction")).0, 1);
        // The holder passes on the first and takes the second.
        assert_eq!(run(label, &F3, lodor, &["decline", "pds|01|jord"], primary("pok4construction")).0, 1);
        let label = "strategy_cards::construction_secondary";
        assert_eq!(run(label, &F3, lodor, &["spacedock|26|lodor"], secondary("pok4construction")).0, 1);
        assert_eq!(run(label, &F3, lodor, &["decline"], secondary("pok4construction")).0, 1);
        // No planet controlled but the home planet, whose dock is there: only a PDS can go.
        assert_eq!(run(label, &F3, |_| {}, &["pds|01|jord"], secondary("pok4construction")).0, 1);
    }

    /// Package W5: the Naaz, Nomad, Yssaril and Mentak seats (own or at the table) no longer bar the structure windows
    /// (`production_view::unported_window` lists the dropped Keleres and Deepwrought seats only). Fails on the pre-W5 guard
    /// (`(0, 1)`: the window was declined).
    #[test]
    fn construction_runs_at_tables_with_the_tier_w_seats() {
        for table in [
            [("a", "sol"), ("b", "yssaril")],
            [("a", "sol"), ("b", "nomad")],
            [("a", "naaz"), ("b", "sol")],
            [("a", "yssaril"), ("b", "nomad")],
            [("a", "mentak"), ("b", "naaz")],
        ] {
            assert_eq!(run("strategy_cards::construction_primary", &table, lodor, &["spacedock|26|lodor", "pds|26|lodor"], primary("pok4construction")), (1, 0), "{table:?}");
            assert_eq!(run("strategy_cards::construction_secondary", &table, lodor, &["spacedock|26|lodor"], secondary("pok4construction")), (1, 0), "{table:?}");
        }
        // The Keleres seat stays string.
        assert_eq!(run("strategy_cards::construction_secondary", &[("a", "keleresm"), ("b", "sol")], lodor, &["spacedock|26|lodor"], secondary("pok4construction")), (0, 1));
    }

    #[test]
    fn construction_declines_where_a_faction_effect_changes_the_placement() {
        let label = "strategy_cards::construction_secondary";
        // Minister of Industry: a space dock opens production.
        let minister = |s: &mut GameState| {
            lodor(s);
            s.enact_law("minister_industry", "a");
        };
        assert_eq!(run(label, &F3, minister, &["spacedock|26|lodor"], secondary("pok4construction")), (0, 1));
    }

    #[test]
    fn a_token_gain_stops_at_the_reinforcements_and_mecatol_is_no_diplomacy_system() {
        // Eight tokens on the sheet and eight on the board: nothing is left to gain, the questions are asked.
        let spent = |s: &mut GameState| {
            for system in ["19", "20", "21", "22", "23", "24", "25", "27"] {
                token_in(s, system);
            }
            s.player_mut(&a()).unwrap().trade_goods = 4;
        };
        let answers = ["tactic_tokens", "tactic_tokens", "fleet_tokens", "no"];
        assert_eq!(run("strategy_cards::leadership_primary", &F3, spent, &answers, primary("pok1leadership")).0, 1);
        // Mecatol Rex is not a system Diplomacy may choose.
        let mecatol = |s: &mut GameState| {
            s.system_mut(&SystemId::new("18")).set_control(PlanetId::new("mr"), a());
            s.exhausted_planets.insert(PlanetId::new("mr"));
        };
        assert_eq!(run("strategy_cards::diplomacy_primary", &F3, mecatol, &["01", "mr"], primary("pok2diplomacy")).0, 1);
    }

    #[test]
    fn diplomacy_lists_each_system_once_stops_after_two_and_knows_both_mecatols() {
        let label = "strategy_cards::diplomacy_primary";
        // Two planets of one system: the system is offered once; three exhausted planets: two are readied.
        let twins = |s: &mut GameState| {
            exhausted_planet(s, "11", "lisisii");
            exhausted_planet(s, "11", "ragh");
            s.exhausted_planets.insert(PlanetId::new("jord"));
        };
        assert_eq!(run(label, &F3, twins, &["11", "lisisii", "ragh"], primary("pok2diplomacy")).0, 1);
        assert_eq!(run("strategy_cards::diplomacy_secondary", &F3, twins, &["lisisii", "ragh"], secondary("pok2diplomacy")).0, 1);
        // The Thunder's Edge Mecatol Rex is not a choice either.
        let te = |s: &mut GameState| {
            s.system_mut(&SystemId::new("112")).set_control(PlanetId::new("mrte"), a());
            s.exhausted_planets.insert(PlanetId::new("mrte"));
        };
        assert_eq!(run(label, &F3, te, &["01", "mrte"], primary("pok2diplomacy")).0, 1);
    }

    #[test]
    fn leadership_carries_an_overpayment_into_the_next_purchase_and_declines_a_mahact_pool() {
        let label = "strategy_cards::leadership_primary";
        // Mecatol pays six for a bill of three: the three left pay the next token, "0 more" influence.
        let mecatol = |s: &mut GameState| {
            s.system_mut(&SystemId::new("18")).set_control(PlanetId::new("mr"), a());
            s.player_mut(&a()).unwrap().trade_goods = 0;
        };
        let answers = ["tactic_tokens", "tactic_tokens", "fleet_tokens", "yes", "exhaust|mr", "strategic_tokens", "yes", "fleet_tokens"];
        assert_eq!(run(label, &F3, mecatol, &answers, primary("pok1leadership")).0, 1);
        let held = |s: &mut GameState| {
            s.faction_marks.insert("mahact:fleet:b".to_owned(), "a".to_owned());
        };
        assert_eq!(run(label, &F3, held, &["tactic_tokens", "tactic_tokens", "fleet_tokens", "no"], primary("pok1leadership")), (0, 1));
    }

    #[test]
    fn a_deck_reserve_and_a_draw_hook_keep_the_draws_on_the_string_side() {
        let reserve = |s: &mut GameState| {
            s.deck_reserve = Some(ti4_model::deck_reserve::DeckReserve::new(
                std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(ti4_model::deck_reserve::IDLE)),
                Vec::new(),
            ));
        };
        assert_eq!(run("strategy_cards::imperial_secret", &F3, reserve, &[], secondary("pok8imperial")), (0, 1));
        assert_eq!(run("strategy_cards::politics_secondary", &F3, reserve, &[], secondary("pok3politics")), (0, 1));
        // A bonus card with the draw (Titans' Slumberstate) is the string's.
        let bonus = crate::factions::hooks_economy::EconomyHooks {
            action_card_draw_bonus: Some(|_, _, _, _| 1),
            ..crate::factions::hooks_economy::EconomyHooks::NONE
        };
        let drawn = crate::factions::hooks_economy::with_test_hooks(bonus, || {
            run("strategy_cards::politics_secondary", &F3, |_| {}, &[], secondary("pok3politics"))
        });
        assert_eq!(drawn, (0, 1));
    }

    #[test]
    fn construction_follows_the_box_the_floating_factory_and_the_pds_alternatives() {
        let label = "strategy_cards::construction_secondary";
        // Six PDS on the board: none is left in the box, only a dock can go.
        let empty_box = |s: &mut GameState| {
            lodor(s);
            for system in ["19", "20", "21", "22", "23", "24"] {
                s.system_mut(&SystemId::new(system)).units.push(ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new("pds"), a()));
            }
        };
        assert_eq!(run(label, &F3, empty_box, &["spacedock|26|lodor"], secondary("pok4construction")).0, 1);
        // Saar's Floating Factory goes into the space area.
        let saar = [("a", "saar"), ("b", "hacan")];
        let strip = |s: &mut GameState| {
            for system in s.board.values_mut() {
                for units in system.planet_units.values_mut() {
                    units.retain(|unit| !unit.type_id.as_str().contains("spacedock"));
                }
            }
        };
        let probe = {
            let mut s = crate::fixtures::seated_game(&saar, DEFAULT);
            strip(&mut s);
            crate::strategy_cards::structure_options(&s, ContentStore::embedded(), DEFAULT, &a(), false)
                .into_iter()
                .find(|o| o.id.starts_with("spacedock|"))
                .expect("a dock spot")
                .id
        };
        assert_eq!(run_in(DEFAULT, label, &saar, strip, &[probe.as_str()], secondary_in(DEFAULT, "pok4construction")).0, 1);
        // The factory ignores the dock the planet already holds.
        let held = {
            let s = crate::fixtures::seated_game(&saar, DEFAULT);
            crate::strategy_cards::structure_options(&s, ContentStore::embedded(), DEFAULT, &a(), false)
                .into_iter()
                .find(|o| o.id.starts_with("spacedock|"))
                .expect("a dock spot beside the dock")
                .id
        };
        assert_eq!(run_in(DEFAULT, label, &saar, |_| {}, &[held.as_str()], secondary_in(DEFAULT, "pok4construction")).0, 1);
        // A PDS alternative a faction offers is the string's.
        let alt = crate::factions::hooks_economy::EconomyHooks {
            pds_placement_alternative: Some(|_, _, _, _, _, _| vec![crate::choice::ChoiceOption::labelled("x:alt", "build", "alt")]),
            ..crate::factions::hooks_economy::EconomyHooks::NONE
        };
        let declined = crate::factions::hooks_economy::with_test_hooks(alt, || {
            run(label, &F3, lodor, &["decline"], secondary("pok4construction"))
        });
        assert_eq!(declined, (0, 1));
    }

    #[test]
    fn imperial_primary_scores_nothing_gains_a_point_for_mecatol_and_scores_in_lockstep() {
        let label = "strategy_cards::imperial_primary";
        let mecatol = |s: &mut GameState| {
            s.system_mut(&SystemId::new("18")).set_control(PlanetId::new("mr"), a());
        };
        assert_eq!(run(label, &F3, mecatol, &[], primary("pok8imperial")).0, 1);
        // The ninth point and the tenth: the game is over.
        let ninth = |s: &mut GameState| {
            mecatol(s);
            s.player_mut(&a()).unwrap().victory_points = 9;
        };
        assert_eq!(run(label, &F3, ninth, &[], primary("pok8imperial")).0, 1);
        // An objective the holder can score (three tokens): the question is asked, a decline goes on.
        let scoreable = |s: &mut GameState| s.revealed_objectives.push(ti4_model::id::ObjectiveId::new("lead"));
        assert_eq!(run(label, &F3, scoreable, &["decline"], primary("pok8imperial")).0, 1);
        // Scoring it is the core's since B8c-3 (`strategy_te_tests` reads the position after).
        assert_eq!(run(label, &F3, scoreable, &["lead"], primary("pok8imperial")), (1, 0));
        // Past the target and a Keleres seat are the string's too (a full secret hand is the core's since W6).
        let tenth = |s: &mut GameState| {
            mecatol(s);
            s.player_mut(&a()).unwrap().victory_points = 10;
        };
        assert_eq!(run(label, &F3, tenth, &[], primary("pok8imperial")), (0, 1));
        let keleres = [("a", "sol"), ("b", "keleresm")];
        assert_eq!(run(label, &keleres, |_| {}, &[], primary("pok8imperial")), (0, 1));
        assert_eq!(run(label, &F3, full_hand, &["eap"], primary("pok8imperial")), (1, 0));
    }

    #[test]
    fn the_trade_secondary_replenishes_in_lockstep_and_declines_what_causes_events() {
        let label = "strategy_cards::trade_secondary";
        assert_eq!(run(label, &F3, |_| {}, &[], secondary("pok5trade")).0, 1);
        let spent = |s: &mut GameState| s.player_mut(&a()).unwrap().commodities = 1;
        assert_eq!(run(label, &F3, spent, &[], secondary("pok5trade")).0, 1);
        // A Cabal at the table: the replenish is announced to it (a staged event, since package W10), compared in lockstep.
        let cabal = [("a", "sol"), ("b", "cabal")];
        assert_eq!(run(label, &cabal, spent, &[], secondary("pok5trade")), (1, 0));
        // A Trade Agreement of the follower held by another seat: the commodities become that seat's trade goods and the note goes home.
        let lent = |s: &mut GameState| {
            s.player_mut(&a()).unwrap().commodities = 1;
            s.promissory_notes.insert("ta:sol".to_owned(), PlayerId::new("b"));
        };
        assert_eq!(run(label, &F3, lent, &[], secondary("pok5trade")), (1, 0));
        // Both at once, and the note of another faction's owner (the follower is not the owner): nothing to pay.
        let both = |s: &mut GameState| {
            lent(s);
            s.promissory_notes.insert("ta:hacan".to_owned(), PlayerId::new("a"));
        };
        let table4 = [("a", "sol"), ("b", "hacan"), ("c", "cabal")];
        assert_eq!(run(label, &table4, both, &[], secondary("pok5trade")), (1, 0));
        // Two seats of one faction (the note is keyed by faction): the string stands.
        let twins = [("a", "sol"), ("b", "sol")];
        assert_eq!(run(label, &twins, spent, &[], secondary("pok5trade")), (0, 1));
    }

    /// The outcome of the Trade secondary with a Trade Agreement lent out: pinned on the string state itself, so a core half
    /// that skipped the payment would show here and as a difference (the dual compares the position, the staged events included).
    #[test]
    fn the_trade_secondary_pays_a_lent_trade_agreement_to_its_holder() {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        with_mask(AreaMask::STRING.with(Area::Strategy, Backend::Dual), || {
            let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan"), ("c", "cabal")], POK);
            crate::promissory::deal(&mut state, ContentStore::embedded(), POK);
            state.promissory_notes.insert("ta:sol".to_owned(), PlayerId::new("b"));
            state.player_mut(&a()).unwrap().commodities = 1;
            let goods = state.player(&PlayerId::new("b")).unwrap().trade_goods;
            let mut table = Table::with_default(Box::new(Scripted::new(Vec::<String>::new())));
            crate::strategy_cards::secondary(&mut state, ContentStore::embedded(), POK, None, &mut table, &a(), "pok5trade").unwrap();
            assert_eq!(state.player(&a()).unwrap().commodities, 0);
            assert_eq!(state.player(&PlayerId::new("b")).unwrap().trade_goods, goods + 4);
            assert_eq!(state.promissory_notes.get("ta:sol"), Some(&a()), "the note went home");
            let staged = crate::supply::staged_event_types(&state);
            assert_eq!(staged, vec!["COMMODITIES_REPLENISHED".to_owned(), "TRADE_GOODS_GAINED".to_owned()], "Cabal first, then the Trade Agreement's gain");
        });
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        assert_eq!(counts("strategy_cards::trade_secondary"), (1, 0), "the core half ran");
        let _ = take_report();
    }

    /// The technologies a fresh seat `a` can research that need nothing and are no unit upgrade.
    fn plain_techs(factions: &[(&str, &str)]) -> Vec<String> {
        let state = crate::fixtures::seated_game(factions, POK);
        let content = ContentStore::embedded();
        crate::technology::researchable(&state, content, POK, &a())
            .into_iter()
            .filter(|t| crate::technology::prerequisites(content, t).is_empty() && !crate::technology::is_unit_upgrade(content, t))
            .map(|t| t.to_string())
            .collect()
    }

    #[test]
    fn technology_researches_and_pays_in_lockstep() {
        let label = "strategy_cards::technology_primary";
        let techs = plain_techs(&F3);
        assert!(techs.len() >= 3, "{techs:?}");
        // The free research, then the paid one: a planet and trade goods pay six.
        let rich = goods(4);
        assert_eq!(run(label, &F3, &rich, &[&techs[0], &techs[1], "exhaust|jord"], primary("pok7technology")).0, 1);
        // The holder passes on the paid one.
        assert_eq!(run(label, &F3, &rich, &[&techs[0], "decline"], primary("pok7technology")).0, 1);
        // Too poor for the second: it is not offered.
        let broke = |s: &mut GameState| {
            s.player_mut(&a()).unwrap().trade_goods = 0;
            s.exhausted_planets.insert(PlanetId::new("jord"));
        };
        assert_eq!(run(label, &F3, broke, &[&techs[0]], primary("pok7technology")).0, 1);
        // The secondary: one research for four resources.
        let label = "strategy_cards::technology_secondary";
        // Exactly four to spend: the home planet alone pays (a lone option is taken without a question).
        let exact = |s: &mut GameState| s.player_mut(&a()).unwrap().trade_goods = 0;
        assert_eq!(run(label, &F3, exact, &[&techs[2]], secondary("pok7technology")).0, 1);
        // Everything researched already: the free research is not asked, the paid one has nothing to offer.
        let everything = |s: &mut GameState| {
            goods(4)(s);
            // Not Inheritance Systems (another faction's, and its owner leaves the plain path).
            for t in ti4_engine_core::generated::Technology::ALL {
                let id = <ti4_engine_core::generated::Technology as crate::generated::IdStr>::as_str(t);
                if id != "is" {
                    s.player_mut(&a()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new(id));
                }
            }
        };
        assert_eq!(run("strategy_cards::technology_primary", &F3, everything, &[], primary("pok7technology")).0, 1);
        assert_eq!(run(label, &F3, &rich, &[&techs[2], "exhaust|jord"], secondary("pok7technology")).0, 1);
        assert_eq!(run(label, &F3, &rich, &["decline"], secondary("pok7technology")).0, 1);
        assert_eq!(run(label, &F3, broke, &[], secondary("pok7technology")).0, 1);
    }

    #[test]
    fn technology_declines_what_the_core_window_does_not_carry() {
        let label = "strategy_cards::technology_primary";
        let techs = plain_techs(&F3);
        let rich = goods(4);
        // Since B8c-2 the Nekro seat, the Anti-Intellectual Revolution, a unit upgrade and Inheritance Systems are
        // carried (`strategy_research_tests` plays each in lockstep). A Jol-Nar at the table is not: its readied
        // agent, Doctor Sucaban, offers every researcher a discount for infantry (a question the window does not carry).
        let jolnar = [("a", "sol"), ("b", "jolnar")];
        assert_eq!(run(label, &jolnar, &rich, &[&techs[0], "decline", "decline"], primary("pok7technology")), (0, 1));
        let nekro = [("a", "sol"), ("b", "nekro")];
        assert_eq!(run(label, &nekro, &rich, &[&techs[0], "decline"], primary("pok7technology")), (1, 0));
        let law = |s: &mut GameState| {
            goods(4)(s);
            s.enact_law("revolution", "for");
        };
        assert_eq!(run(label, &F3, law, &[&techs[0], "decline"], primary("pok7technology")), (1, 0));
        // Below the rules version that asks which planets pay, the plan is chosen for the player.
        let old_rules = |s: &mut GameState| {
            goods(4)(s);
            s.rules_version = ti4_model::state::RULES_TECH_PAYMENT_CHOICE - 1;
        };
        assert_eq!(run(label, &F3, old_rules, &[&techs[0], &techs[1]], primary("pok7technology")), (0, 1));
        // A faction effect that offers to waive prerequisites.
        let waiver = crate::factions::hooks_strategy::StrategyHooks {
            research_waiver_offer: Some(|_, _, _, _| {
                Some(crate::factions::hooks_strategy::ResearchWaiver { id: "t".to_owned(), label: "t".to_owned(), payments: Vec::new() })
            }),
            ..crate::factions::hooks_strategy::StrategyHooks::NONE
        };
        let declined = crate::factions::hooks_strategy::with_test_hooks(waiver, || {
            run(label, &F3, goods(4), &[&techs[0], "decline"], primary("pok7technology"))
        });
        assert_eq!(declined, (0, 1));
    }

    /// Regression: under a global `TI4_AREAS=strategy=dual` the dual-suite step runs game tests on other threads
    /// that play the same cards; their windows were counted into the exact counts of these tests
    /// (`imperial_primary_scores_nothing...` saw `(0, 2)` for `(0, 1)`). The counts are per thread now.
    #[test]
    fn a_window_another_thread_runs_does_not_change_the_counts_of_this_thread() {
        let label = "strategy_cards::politics_secondary";
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        crate::dual::reset_thread_windows();
        std::thread::spawn(|| {
            with_mask(AreaMask::STRING.with(Area::Strategy, Backend::Dual), || {
                let mut state = crate::fixtures::seated_game(&F3, POK);
                crate::promissory::deal(&mut state, ContentStore::embedded(), POK);
                let mut table = Table::with_default(Box::new(Scripted::new(Vec::<String>::new())));
                crate::strategy_cards::secondary(&mut state, ContentStore::embedded(), POK, None, &mut table, &a(), "pok3politics").unwrap();
            });
        })
        .join()
        .unwrap();
        // The serial lock holder is the report's focus (`dual::FOCUS`): a window another thread runs reaches
        // neither the shared report nor this thread's counts.
        let shared = report().windows.get(label).map_or(0, |w| w.compared + w.bailed);
        assert_eq!(shared, 0, "the shared report is focused on this thread: the other thread's window is not in it");
        assert_eq!(counts(label), (0, 0), "this thread ran none");
        let _ = take_report();
    }
}
