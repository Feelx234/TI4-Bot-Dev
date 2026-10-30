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
    use ti4_engine_core::rules::strategy_primaries::{DMode, Diplomacy, ImperialPrimary, Politics, TeWarfare, WarfarePrimary};
    use ti4_engine_core::state::{CoreState, Slice};
    use ti4_model::content_types::SourceSet;
    use ti4_model::id::{PlayerId, SystemId};
    use ti4_model::state::GameState;

    use crate::choice::Choice;
    use crate::core_sync::{with_core, Ctx, Footprint, Sources};
    use crate::dual::{CoreWindow, Run};
    use crate::dual_window::{drive_with, Driven, Recording};
    use crate::generated::IdStr;
    use crate::dual_window::outside;
    use crate::preview::{Delta, Preview, Quantity};
    use crate::render::Render;
    use crate::strategy_core::named_tables;

    type Complete<'a> = &'a dyn Fn(&CoreChoice, &CoreState, &mut Choice);

    const WARFARE: Footprint =
        Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Pending], &[Slice::Players, Slice::Board]);
    const DIPLOMACY: Footprint =
        Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Pending], &[Slice::Board]);
    const POLITICS: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Decks, Slice::Pending],
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
        let mut driver = Driver::new(window, Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = with_core(gs, cx, Sources::default(), fp, |core, _| drive_with(&mut driver, core, &kernel, r, &before, script, complete))
            .ok()?
            .ok()?;
        Some((driver.window, driven))
    }

    fn finish<W>(window_done: &Driven, _: &W) -> CoreWindow<bool> {
        CoreWindow { run: Run::new(window_done.finished).with_events_vec(Vec::new()), questions: window_done.rendered.clone() }
    }

    // -- command tokens (52.4) -----------------------------------------------------------------------------

    /// `tokens::with_pool_details`: the pools as they stand now and what the question hands out or moves.
    fn pool_details(core: &CoreState, seat: Seat, letnev: bool, mode: &str, to_place: Option<usize>) -> serde_json::Map<String, Value> {
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
        if outside(gs, script) || gs.tokens_held_by_mahact(player) > 0 {
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let seat = r.seat(player.as_str())?;
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
        let mut now = before.clone();
        now.exhausted_planets.retain(|p| {
            ti4_engine_core::generated::Planet::from_id(p.as_str()).is_none_or(|planet| ti4_engine_core::query::board::exhausted(core, planet))
        });
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
        if outside(gs, script) {
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let seat = r.seat(player.as_str())?;
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
        let mut driver = Driver::new(window, Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let fp = Footprint { reads: crate::production_view::READS, writes: ti4_engine_core::state::SliceSet::of(&[Slice::Board, Slice::Players, Slice::Cards, Slice::Marks]) };
        let driven = with_core(gs, cx, Sources::default(), fp, |core, _| drive_at(&mut driver, core, kernel, r, &before, script, complete))
            .ok()?
            .ok()?;
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
        if outside(gs, script) || gs.tokens_held_by_mahact(player) > 0 || crate::production_view::unported_window(gs, player) {
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let seat = r.seat(player.as_str())?;
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
            return None;
        }
        let alternatives = gs.controlled_planets(player).into_iter().any(|(system, planet)| {
            !crate::factions::hooks_economy::pds_placement_alternatives(gs, content, sources, player, system, planet).is_empty()
        });
        if alternatives {
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
        if outside(gs, script) {
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let seat = r.seat(player.as_str())?;
        let facts = structure_facts(gs, content, sources, player, &r)?;
        let (window, driven) = run_production(gs, &mut cx, &r, Construction::new(seat, primary, facts), seat, script, &|_, _, _| {})?;
        Some(finish(&driven, &window))
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
        if outside(gs, script) {
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let seat = r.seat(player.as_str())?;
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
        if outside(gs, script) {
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let seat = r.seat(player.as_str())?;
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
        Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Pending], &[Slice::Players]);

    /// The core half of the Trade secondary (`strategy_cards::replenish` for the follower): the commodities go
    /// to the printed value. `None` where a commodity change causes events the core does not hold (Cabal, a
    /// siphoning plot, a Trade Agreement), where the game's limit is not the faction's printed value, inside a
    /// battle, an invasion or a nested emission, and with diplomacy on.
    pub fn replenish_secondary(gs: &mut GameState, content: &ContentStore, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
        if outside(gs, script) || crate::leaders_core::replenish_causes_events(gs) {
            return None;
        }
        let limit = crate::strategy_cards::commodity_limit(gs, content, player);
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let seat = r.seat(player.as_str())?;
        let same = with_core(gs, &mut cx, Sources::default(), REPLENISH, |core, _| {
            let printed = core.players().by_seat(seat).map(|p| i32::from(p.faction.data().commodities));
            (printed == Some(limit)).then(|| fx::replenish_commodities(core, seat))
        })
        .ok()?;
        same?;
        Some(CoreWindow { run: Run::new(true).with_events_vec(Vec::new()), questions: Vec::new() })
    }

    // -- Technology ----------------------------------------------------------------------------------------

    /// The seats whose own effects on a research are questions or forms the window does not carry: an own Deepwrought
    /// (Doctor Carrina's window opens around every research), Keleres (the agent's offer in the payment) or Mentak (the
    /// Corsair's unit form), and the economy the production kernel does not follow (`unported_economy`: Last Bastion,
    /// Crimson). The payment hooks of Naaz, Nomad and Yssaril are the production window's own (tier W).
    fn research_seat_unported(gs: &GameState, player: &PlayerId) -> bool {
        use crate::factions::{deepwrought, keleres, mentak};
        let own = [deepwrought::MODULE.alias, keleres::MODULE_M.alias, keleres::MODULE_X.alias, keleres::MODULE_A.alias, mentak::MODULE.alias];
        crate::production_view::unported_economy(gs) || gs.player(player).is_some_and(|p| own.contains(&p.faction.as_str()))
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
        if outside(gs, script)
            || gs.rules_version < ti4_model::state::RULES_TECH_PAYMENT_CHOICE
            || research_unported(gs, content, sources, player)
        {
            return None;
        }
        let roster = *crate::technology_core::roster_for(content, sources)?;
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let seat = r.seat(player.as_str())?;
        let scope = crate::technology_core::scope(sources);
        let before = gs.clone();
        let agree = with_core(gs, &mut cx, Sources::default(), Footprint::reads(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Agenda, Slice::Marks, Slice::Strategy, Slice::Cards, Slice::Trade, Slice::Transient, Slice::Explore]), |core, _| {
            research_hooks_agree(core, &before, content, sources, player, seat)
        })
        .ok()?;
        if !agree {
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
            return None;
        }
        Some(finish(&driven, &window))
    }

    // -- Imperial ------------------------------------------------------------------------------------------

    const SECRET: Footprint =
        Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Decks, Slice::Pending], &[Slice::Players, Slice::Decks]);

    /// The core half of Imperial's secret objective draw (`secrets::draw` called by the secondary and by the
    /// primary of a seat that does not hold Mecatol Rex). `None` where the draw is not plain: the hand would go
    /// over its limit (a question to return one), a reserve is replaying draws, a nested emission.
    pub fn draw_secret(gs: &mut GameState, content: &ContentStore, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
        if outside(gs, script) || gs.deck_reserve.is_some() {
            return None;
        }
        let limit = crate::secrets::HAND_LIMIT + crate::relics::secret_objective_bonus(gs, player);
        if crate::secrets::held_count(gs, content, player) + 1 > limit {
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let seat = r.seat(player.as_str())?;
        with_core(gs, &mut cx, Sources::default(), SECRET, |core, _| fx::draw_secret(core, seat)).ok()?;
        Some(CoreWindow { run: Run::new(true).with_events_vec(Vec::new()), questions: Vec::new() })
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
        if outside(gs, script) || gs.deck_reserve.is_some() || gs.players.iter().any(|p| p.faction.as_str().starts_with("keleres")) {
            return None;
        }
        let scoreable = crate::objectives::scoreable_on(gs, content, sources, player, galaxy);
        let typed: Vec<PublicObjective> = scoreable
            .iter()
            .map(|o| crate::core_adapter::typed::<PublicObjective>(o.as_str()))
            .collect::<Option<_>>()?;
        let mut most = 0;
        let mut priced = false;
        for (alias, objective) in scoreable.iter().zip(&typed) {
            let points = i32::from(ti4_engine_core::query::objectives::points_of(*objective));
            if crate::objectives::points_for(content, alias) != Some(points) {
                return None;
            }
            most = most.max(points);
            priced |= ti4_engine_core::query::objectives::cost_of(*objective).is_some();
        }
        if priced && price_is_string(gs, player) {
            return None;
        }
        let controls = gs.controlled_planets(player).into_iter().any(|(system, _)| crate::seating::is_mecatol(system.as_str()));
        if controls {
            if gs.player(player)?.victory_points + most + 1 > crate::objectives::VICTORY_TARGET {
                return None;
            }
        } else {
            if gs.player(player)?.victory_points + most > crate::objectives::VICTORY_TARGET {
                return None;
            }
            let limit = crate::secrets::HAND_LIMIT + crate::relics::secret_objective_bonus(gs, player);
            if crate::secrets::held_count(gs, content, player) + 1 > limit {
                return None;
            }
        }
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let seat = r.seat(player.as_str())?;
        let before = gs.clone();
        let held = crate::secrets::held_count(gs, content, player);
        let max = crate::secrets::HAND_LIMIT + crate::relics::secret_objective_bonus(gs, player);
        // The production kernel answers what a trade good pays and which commodities a payment may spend.
        let env = ti4_engine_core::kernel::fx_pay::PayEnv { scope: crate::core_adapter::scope(sources), blanked: crate::laws::blanked(gs) };
        let window = ImperialPrimary::new(seat, typed.into_iter().collect()).with_price(env);
        let mut driver = Driver::new(window, Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
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
        .ok()?
        .ok()?;
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

    /// Whether `player` draws `count` action cards plainly: no faction hook changes the draw (Scheming, a bonus
    /// card), the hand stays within its limit (no discard question) and the unofficial cards need not leave the
    /// deck first.
    pub(crate) fn plain_draw(gs: &GameState, content: &ContentStore, player: &PlayerId, count: usize) -> bool {
        gs.players.iter().all(|p| p.faction.as_str() != "yssaril")
            && crate::factions::hooks_economy::action_card_draw_bonus(gs, content, player, count) == 0
            && gs.player(player).is_some_and(|p| p.action_cards.len() + count <= crate::action_cards::hand_limit(gs, content, player))
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
        if outside(gs, script) || gs.deck_reserve.is_some() || !plain_draw(gs, content, player, 2) {
            return None;
        }
        let (mut cx, r) = named_tables(gs)?;
        let seat = r.seat(player.as_str())?;
        let (window, driven) = drive_window(gs, &mut cx, &r, POLITICS, Politics::new(seat, primary, gs.rules_version >= ti4_model::state::RULES_OFFICIAL_ACTION_DECK), seat, script, &|c, _, rendered| {
            complete_politics(&r, content, c, rendered);
        })?;
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
    use ti4_model::id::{PlanetId, PlayerId, SystemId};
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
    fn politics_declines_where_the_draw_is_not_plain() {
        let label = "strategy_cards::politics_secondary";
        // A full hand: the draw would end in a discard question.
        let full = |s: &mut GameState| {
            let cards: Vec<_> = s.action_card_deck.drain(..6).collect();
            s.player_mut(&a()).unwrap().action_cards = cards;
        };
        assert_eq!(run(label, &F3, full, &[], secondary("pok3politics")), (0, 1));
        // Scheming.
        let yssaril = [("a", "sol"), ("b", "yssaril")];
        assert_eq!(run(label, &yssaril, |_| {}, &[], secondary("pok3politics")).1, 1);
        // Two seats of one faction: the speaker options would clash.
        let twins = [("a", "sol"), ("b", "hacan"), ("c", "hacan")];
        assert_eq!(run("strategy_cards::politics_primary", &twins, |_| {}, &["hacan"], primary("pok3politics")).1, 1);
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
        // Yssaril's borrowed agents are still the string's: that table is declined.
        let yssaril = [("a", "sol"), ("b", "yssaril")];
        assert_eq!(run(label, &yssaril, goods(4), &free, primary("pok1leadership")), (0, 1));
    }

    #[test]
    fn imperial_draws_a_secret_objective_in_lockstep() {
        let label = "strategy_cards::imperial_secret";
        assert_eq!(run(label, &F3, |_| {}, &[], secondary("pok8imperial")).0, 1);
        // The primary of a seat without Mecatol Rex draws one too (nothing is scoreable at the start).
        assert_eq!(run("strategy_cards::imperial_primary", &F3, |_| {}, &[], primary("pok8imperial")).0, 1);
        // A full hand: the draw would end in a question to hand one back.
        let full = |s: &mut GameState| {
            let cards: Vec<_> = s.secret_deck.drain(..3).collect();
            s.player_mut(&a()).unwrap().secret_objectives = cards;
        };
        assert_eq!(run(label, &F3, full, &[], secondary("pok8imperial")), (0, 1));
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
        // Past the target, a Keleres seat, a full secret hand and a reserve are the string's too.
        let tenth = |s: &mut GameState| {
            mecatol(s);
            s.player_mut(&a()).unwrap().victory_points = 10;
        };
        assert_eq!(run(label, &F3, tenth, &[], primary("pok8imperial")), (0, 1));
        let keleres = [("a", "sol"), ("b", "keleresm")];
        assert_eq!(run(label, &keleres, |_| {}, &[], primary("pok8imperial")), (0, 1));
        let full = |s: &mut GameState| {
            let cards: Vec<_> = s.secret_deck.drain(..3).collect();
            s.player_mut(&a()).unwrap().secret_objectives = cards;
        };
        assert_eq!(run(label, &F3, full, &[], primary("pok8imperial")), (0, 1));
    }

    #[test]
    fn the_trade_secondary_replenishes_in_lockstep_and_declines_what_causes_events() {
        let label = "strategy_cards::trade_secondary";
        assert_eq!(run(label, &F3, |_| {}, &[], secondary("pok5trade")).0, 1);
        let spent = |s: &mut GameState| s.player_mut(&a()).unwrap().commodities = 1;
        assert_eq!(run(label, &F3, spent, &[], secondary("pok5trade")).0, 1);
        // A Cabal at the table: the replenish is announced to it.
        let cabal = [("a", "sol"), ("b", "cabal")];
        assert_eq!(run(label, &cabal, spent, &[], secondary("pok5trade")), (0, 1));
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
