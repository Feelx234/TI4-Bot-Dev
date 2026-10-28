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
    use ti4_engine_core::rules::strategy_primaries::{DMode, Diplomacy, Leadership, Politics, WarfarePrimary};
    use ti4_engine_core::state::{CoreState, Slice};
    use ti4_model::content_types::SourceSet;
    use ti4_model::id::{PlayerId, SystemId};
    use ti4_model::state::GameState;

    use crate::choice::Choice;
    use crate::core_sync::{with_core, Ctx, Footprint, Sources};
    use crate::dual::{CoreWindow, Run};
    use crate::dual_window::{drive_with, Driven, Recording};
    use crate::generated::IdStr;
    use crate::leaders_core::outside;
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
    fn complete_tokens(c: &CoreChoice, core: &CoreState, rendered: &mut Choice, letnev: bool) {
        match c.ask {
            Ask::Strategy(SAsk::GainPool { left }) => {
                pool_previews(core, c.actor, rendered);
                rendered.details.extend(pool_details(core, c.actor, letnev, "gain", Some(usize::from(left))));
            }
            Ask::Strategy(SAsk::WarfareRedistribute { .. }) => {
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
        let kernel = crate::production_view::kernel();
        let mut driver = Driver::new(Leadership::new(seat, primary), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let gained = std::cell::Cell::new(0_usize);
        let purchase_of = |core: &CoreState| crate::strategy_cards::purchase_details(&string_now(&before, core, seat, player), content, sources, player);
        let fp = Footprint { reads: crate::production_view::READS, writes: ti4_engine_core::state::SliceSet::of(&[Slice::Board, Slice::Players]) };
        let driven = with_core(gs, &mut cx, Sources::default(), fp, |core, _| {
            drive_at(&mut driver, core, kernel, &r, &before, script, &|c, core, rendered| {
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
            })
        })
        .ok()?
        .ok()?;
        Some(finish(&driven, &driver.window))
    }

    // -- Politics ------------------------------------------------------------------------------------------

    /// Whether `player` draws `count` action cards plainly: no faction hook changes the draw (Scheming, a bonus
    /// card), the hand stays within its limit (no discard question) and the unofficial cards need not leave the
    /// deck first.
    fn plain_draw(gs: &GameState, content: &ContentStore, player: &PlayerId, count: usize) -> bool {
        gs.players.iter().all(|p| p.faction.as_str() != "yssaril")
            && crate::factions::hooks_economy::action_card_draw_bonus(gs, content, player, count) == 0
            && gs.player(player).is_some_and(|p| p.action_cards.len() + count <= crate::action_cards::hand_limit(gs, content, player))
    }

    /// The core half of the two Politics abilities. `None` as for Warfare, with two seats of one faction (the
    /// speaker options are named by faction) and where the draw is not plain.
    pub fn politics(gs: &mut GameState, content: &ContentStore, player: &PlayerId, primary: bool, script: &Recording) -> Option<CoreWindow<bool>> {
        if outside(gs, script) || !plain_draw(gs, content, player, 2) {
            return None;
        }
        let (mut cx, r) = named_tables(gs)?;
        let seat = r.seat(player.as_str())?;
        let (window, driven) = drive_window(gs, &mut cx, &r, POLITICS, Politics::new(seat, primary, gs.rules_version >= ti4_model::state::RULES_OFFICIAL_ACTION_DECK), seat, script, &|c, _, rendered| match c.ask {
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
        })?;
        Some(finish(&driven, &window))
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
    use ti4_model::content_types::POK;
    use ti4_model::id::{PlanetId, PlayerId, SystemId};
    use ti4_model::state::GameState;

    use crate::choice::{Scripted, Table};
    use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
    use crate::dual_window::Recording;

    fn counts(label: &str) -> (u64, u64) {
        report().windows.get(label).map_or((0, 0), |w| (w.compared, w.bailed))
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
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        with_mask(AreaMask::STRING.with(Area::Strategy, Backend::Dual), || {
            let mut state = crate::fixtures::seated_game(factions, POK);
            // The notes are dealt to the factions once they are seated (the lobby does this; the typed cards
            // slice has no generic faction).
            crate::promissory::deal(&mut state, ContentStore::embedded(), POK);
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
        move |state, table| {
            crate::strategy_cards::secondary(state, ContentStore::embedded(), POK, None, table, &a(), card).unwrap();
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
        assert_eq!(run(label, &F3, |s| { goods(3)(s); s.exhausted_planets.insert(PlanetId::new("jord")); }, &[&free[..], &["yes", "tactic_tokens"]].concat(), primary("pok1leadership")).0, 1);
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
        let argent = [("a", "sol"), ("b", "argent")];
        let free = ["tactic_tokens", "tactic_tokens", "fleet_tokens", "no"];
        assert_eq!(run(label, &argent, goods(4), &free, primary("pok1leadership")), (0, 1));
    }
}
