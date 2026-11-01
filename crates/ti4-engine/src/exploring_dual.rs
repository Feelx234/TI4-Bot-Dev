//! `Area::ExplorationRelics` (package X): the core half of one exploration (`exploration::explore_with` for a planet, `explore_frontier` for a
//! frontier token), `ti4_engine_core::rules::explore_windows::Exploring`, and what the core window needs of the adapter: the event texts,
//! the words of a log row (the string's `row_words`, the only owner of those words) and the ability table.
//!
//! The string runs first; the window replays it on a copy of the position with the answers the string was given, the questions are
//! compared one by one, then the position, the events and the value (`Option<Explored>`). A position the window does not carry
//! (Freelancers, Titans' Terragenesis, a table with no texts) is a counted bail and the string stands.

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

#[cfg(feature = "typed-core")]
mod real {
    use ti4_engine_core::generated::{Explore, ExploreType, Planet, System, Wormhole};
    use ti4_engine_core::kernel::ability::ExploreCodes;
    use ti4_engine_core::kernel::fx_explore::{RowIn, RowOut};
    use ti4_engine_core::kernel::stage::AbilityTable;
    use ti4_engine_core::kernel::Resolver;
    use ti4_engine_core::rules::explore_windows::Exploring;
    use ti4_engine_core::state::{CoreState, EDeck, EHow, ETarget, EWhy, Slice};
    use ti4_model::content_types::SourceSet;
    use ti4_model::id::{PlanetId, PlayerId, SystemId};
    use ti4_model::state::GameState;

    use crate::core_adapter::typed;
    use crate::core_sync::conv::{explore_key, explore_type};
    use crate::core_sync::{Footprint, Sources};
    use crate::dual::{CoreWindow, Run, Why};
    use crate::dual_window::Recording;
    use crate::exploration::{Explored, Holdings};
    use crate::generated::IdStr;
    use crate::render::Render;

    /// What the window reads (the players' cards, the decks, the board, the explore slice, the transient slice for the invasion it may be
    /// asked in, the pending slice for staged events) and writes.
    pub const FP: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Decks, Slice::Cards, Slice::Agenda, Slice::Marks, Slice::Transient, Slice::Pending, Slice::Strategy, Slice::Ledger, Slice::Objectives, Slice::Explore],
        &[Slice::Meta, Slice::Players, Slice::Board, Slice::Decks, Slice::Cards, Slice::Marks, Slice::Pending, Slice::Ledger, Slice::Explore],
    );

    /// The words of the log row of a card: the string's `row_words` over what the core window read, minted in the position's text table.
    #[must_use]
    pub fn words(core: &mut CoreState, r: &RowIn) -> Option<RowOut> {
        let planet = r.planet.map(|p| PlanetId::new(IdStr::as_str(p)));
        let card = IdStr::as_str(r.card).to_owned();
        let outcome = match r.how {
            EHow::Fragment(deck) => Explored::Fragment { trait_name: explore_key(deck) },
            EHow::Attached => Explored::Attached { card },
            EHow::Resolved => Explored::Resolved { card },
            EHow::Unresolved => Explored::Unresolved { card },
            EHow::Discarded => Explored::Discarded { card },
        };
        let mut tokens = Vec::new();
        if let Some(system) = r.gamma {
            tokens.push(format!("wormhole:GAMMA:{}", IdStr::as_str(system)));
        }
        if let Some((system, face)) = r.ion {
            tokens.push(format!("ion_storm:{}:{}", IdStr::as_str(system), if face == Wormhole::Alpha { "ALPHA" } else { "BETA" }));
        }
        let kept = r.kept.map(IdStr::as_str);
        let attached = r.attached.map(IdStr::as_str);
        let w = crate::exploration::row_words(&Holdings::from_core(r.before), &Holdings::from_core(r.after), planet.as_ref(), &outcome, tokens, kept, attached);
        let mut mint = |text: &str| -> Option<ti4_engine_core::state::Code> {
            core.texts.codes.intern(text.as_bytes(), ti4_engine_core::state::CODE_CAPACITY).and_then(|i| u16::try_from(i).ok()).map(ti4_engine_core::state::Code)
        };
        let outcome = mint(&w.text)?;
        // The string writes an empty `result` as the empty text and the core keeps `None` for it.
        let result = if w.result.is_empty() { None } else { Some(mint(&w.result)?) };
        let gains = w.gains.iter().map(|(key, n)| Some((mint(key)?, *n))).collect::<Option<Vec<_>>>()?;
        let placed = w.placed.iter().map(|text| mint(text)).collect::<Option<Vec<_>>>()?;
        Some(RowOut { outcome, result, gains, placed })
    }

    /// The event texts of an exploration, in the render table.
    #[must_use]
    pub fn codes(r: &Render) -> ExploreCodes {
        let mut codes = ExploreCodes { filled: true, goods: r.code("exploration"), trade_agreement: r.code("trade_agreement"), ..ExploreCodes::default() };
        for deck in ExploreType::ALL {
            codes.decks[deck as usize] = r.code(&explore_key(deck));
        }
        for card in Explore::ALL {
            codes.cards[card as usize] = r.code(IdStr::as_str(card));
        }
        codes.freelancers = r.code("Freelancers");
        codes
    }

    /// What the window of an exploration needs in the table of its driver: the links to the windows of other areas it runs, the printed
    /// names of the action cards, the unit forms of the scope, the event texts and the words.
    #[must_use]
    pub fn table(r: &Render, content: &ti4_content::ContentStore, sources: SourceSet) -> AbilityTable {
        let mut table = AbilityTable::default();
        ti4_engine_core::driver::children::register(&mut table);
        table.facts.card_groups = crate::reactions_core::card_groups();
        table.facts.roster = crate::technology_core::roster_for(content, sources);
        table.facts.explore_words = Some(words);
        table.codes.explore = codes(r);
        table
    }

    /// Why a position is outside what the window follows (the census text), or `None`.
    fn outside(gs: &GameState, script: &Recording) -> Option<&'static str> {
        if gs.deck_reserve.is_some() {
            Some("deck reserve (a turn redo replays the draws)")
        } else if gs.active_space_combat.is_some() {
            Some("a question asked inside a space battle (its context carries the battle)")
        } else if gs.diplomacy.enabled {
            Some("diplomacy on")
        } else {
            let _ = script;
            None
        }
    }

    fn unsupported_text(why: EWhy) -> &'static str {
        match why {
            EWhy::NoTexts => "an exploration whose texts the table does not carry",
            EWhy::NoLink => "an exploration whose child window or unit forms the table does not carry",
            EWhy::LinkedAsks => "a window of another area that asks a question inside the exploration",
            EWhy::Freelancers => "Freelancers (a unit produced with influence)",
            EWhy::Terragenesis => "Titans' Terragenesis (after you explore a planet)",
            EWhy::Replenish => "a replenishment the core does not follow (two seats of one faction)",
            EWhy::Draw => "an exploration draw the core does not carry",
        }
    }

    /// The value of a finished window, as the string's `explore_in` returns it.
    fn value(window: &Exploring) -> Option<Explored> {
        let run = window.run();
        if !window.explored() {
            return None;
        }
        let card = IdStr::as_str(run.card?).to_owned();
        Some(match run.how {
            EHow::Fragment(deck) => Explored::Fragment { trait_name: explore_key(deck) },
            EHow::Attached => Explored::Attached { card },
            EHow::Resolved => Explored::Resolved { card },
            EHow::Unresolved => Explored::Unresolved { card },
            EHow::Discarded => Explored::Discarded { card },
        })
    }

    /// The display facts only the string knows: the commodity standing of the four commodity cards (`COMMODITY_REWARDS`) and the offer card of
    /// the infantry that pays (`pay_offer`).
    fn question_details(c: &ti4_engine_core::choice::Choice, before: &GameState, content: &ti4_content::ContentStore, player: &PlayerId, rendered: &mut crate::choice::Choice) {
        use ti4_engine_core::choice::explore::{Ask as EAsk, PayReward, Step};
        match c.ask {
            ti4_engine_core::choice::Ask::Explore(EAsk::Reward(Step::AbandonedWarehouses | Step::MerchantStation | Step::FunctioningBase | Step::LocalFabricators)) => {
                rendered.details.insert("commodities".to_owned(), crate::strategy_cards::commodity_standing(before, content, player));
            }
            ti4_engine_core::choice::Ask::Explore(EAsk::PayInfantry { card, planet, reward }) => {
                let planet_id = PlanetId::new(IdStr::as_str(planet));
                let system = crate::exploration::system_of(before, &planet_id).map(|s| s.to_string()).unwrap_or_default();
                let reward = match reward {
                    PayReward::TradeGood => "gain 1 trade good",
                    PayReward::ReadyPlanet => "ready this planet",
                    PayReward::CommandToken => "gain 1 command token",
                };
                let offer = crate::exploration::pay_offer(content, IdStr::as_str(card), &planet_id, &system, reward);
                *rendered = offer(rendered.clone());
            }
            _ => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn run_window(
        gs: &mut GameState,
        content: &ti4_content::ContentStore,
        sources: SourceSet,
        player: &PlayerId,
        window: impl FnOnce(ti4_engine_core::ids::Seat) -> Exploring,
        script: &Recording,
    ) -> Option<CoreWindow<Option<Explored>>> {
        if let Some(why) = outside(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        crate::dual_window::nested_check(script)?;
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        // (A draw a faction hook changes: Scheming, Crafty, blank laws, Ral Nel's seat, is the string's.)
        if !crate::strategy_cards_core::plain_draw(gs, content, player, 2) {
            crate::dual::note_bail_here("an exploration whose action card draw a faction hook changes");
            return None;
        }
        let before = gs.clone();
        let kernel = crate::production_view::kernel_of(sources);
        let letnev = gs.player(player).why(crate::dual::reason::NO_SEAT)?.faction.as_str() == "letnev";
        let details = |c: &ti4_engine_core::choice::Choice, core: &CoreState, rendered: &mut crate::choice::Choice| {
            question_details(c, &before, content, player, rendered);
            crate::strategy_cards_core::complete_tokens(c, core, rendered, letnev);
        };
        let mut table = table(&r, content, sources);
        ti4_engine_core_factions::self_assembly::register_links(&mut table);
        let mut staged = Vec::new();
        let window = window(seat);
        // Terragenesis is a rule of the core table (the live path's registry has it); this driver has no registry, so the string's window for it
        // is not followed.
        if window.run().announce && matches!(window.run().target, ETarget::Planet(_)) && gs.player(player).is_some_and(|p| p.faction.as_str() == "titans") {
            crate::dual::note_bail_here(unsupported_text(EWhy::Terragenesis));
            return None;
        }
        let mut driver = crate::window_driver::driver_for(window, Resolver::new(&[seat], Some(seat)), table, 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), crate::dual_window::nested_fp(FP, script), |core, _| {
            let driven = crate::dual_window::drive_positioned(&mut driver, core, kernel, Some(kernel), &r, &before, script, &details);
            staged = std::mem::take(&mut core.pending_mut().staged);
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        if let Some(why) = driver.window.unsupported() {
            crate::dual::note_bail_here(unsupported_text(why));
            return None;
        }
        crate::dual_window::apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
        Some(CoreWindow { run: Run::new(value(&driver.window)).with_events_vec(events), questions: driven.rendered })
    }

    /// The core half of `exploration::explore_with` for a planet: `deck` is the deck the caller chose (asked before, by its own window).
    /// `announce`: the string explores with a timing handle (the goods and `PLANET_EXPLORED` are announced).
    #[allow(clippy::too_many_arguments)]
    pub fn explore_planet(
        gs: &mut GameState,
        content: &ti4_content::ContentStore,
        sources: SourceSet,
        player: &PlayerId,
        deck: &str,
        planet: &PlanetId,
        announce: bool,
        script: &Recording,
    ) -> Option<CoreWindow<Option<Explored>>> {
        let deck = explore_type(Slice::Decks, deck).ok().filter(|d| *d != ExploreType::Frontier).why(crate::dual::reason::UNKNOWN_ID)?;
        let planet = typed::<Planet>(planet.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
        run_window(gs, content, sources, player, |seat| Exploring::planet(seat, planet, EDeck::Fixed(deck), announce), script)
    }

    /// The core half of `exploration::explore_frontier` (after its checks): the token is spent by the window.
    pub fn explore_frontier(
        gs: &mut GameState,
        content: &ti4_content::ContentStore,
        sources: SourceSet,
        player: &PlayerId,
        system: &SystemId,
        announce: bool,
        script: &Recording,
    ) -> Option<CoreWindow<Option<Explored>>> {
        let system = typed::<System>(system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
        run_window(gs, content, sources, player, |seat| Exploring::frontier(seat, system, announce, true, true), script)
    }
}

#[cfg(not(feature = "typed-core"))]
mod stub {
    use ti4_model::content_types::SourceSet;
    use ti4_model::id::{PlanetId, PlayerId, SystemId};
    use ti4_model::state::GameState;

    use crate::dual::CoreWindow;
    use crate::dual_window::Recording;
    use crate::exploration::Explored;

    #[allow(clippy::too_many_arguments)]
    pub fn explore_planet(_: &mut GameState, _: &ti4_content::ContentStore, _: SourceSet, _: &PlayerId, _: &str, _: &PlanetId, _: bool, _: &Recording) -> Option<CoreWindow<Option<Explored>>> {
        None
    }

    pub fn explore_frontier(_: &mut GameState, _: &ti4_content::ContentStore, _: SourceSet, _: &PlayerId, _: &SystemId, _: bool, _: &Recording) -> Option<CoreWindow<Option<Explored>>> {
        None
    }
}

#[cfg(all(test, feature = "typed-core"))]
mod tests {
    use ti4_model::content_types::{FULL, POK};
    use ti4_model::id::PlayerId;
    use ti4_model::state::GameState;

    use crate::choice::{Choice, ChoiceOption, Decider, IllegalChoice, Resolving, Scripted, Table};
    use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};
    use crate::exploration::explore_with;

    fn me() -> PlayerId {
        PlayerId::new("a")
    }

    /// Always the last option: the branch the first-option policy does not take.
    struct LastOption;

    impl Decider for LastOption {
        fn choose(&mut self, c: &Choice) -> Result<ChoiceOption, IllegalChoice> {
            c.options.last().cloned().ok_or_else(|| IllegalChoice::NoOptions { player: c.player.clone(), prompt: c.prompt.clone() })
        }
    }

    /// Seat `a` (`faction`) controls a planet with an infantry and a mech on it, goods and commodities in hand; the decks are empty.
    fn table(faction: &str) -> (GameState, ti4_model::id::PlanetId) {
        let mut state = crate::fixtures::seated_game(&[("a", faction), ("b", if faction == "sol" { "jolnar" } else { "sol" })], POK);
        state.promissory_notes.clear();
        state.promissory_faceup.clear();
        state.exploration_decks.clear();
        let (system, planet) = crate::fixtures::a_placed_planet();
        state.system_mut(&system).set_control(planet.clone(), me());
        crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &me(), 2);
        let seat = state.player_mut(&me()).unwrap();
        seat.trade_goods = 2;
        seat.commodities = 1;
        (state, planet)
    }

    fn ctx_with<T>(last: bool, f: impl FnOnce(&mut Resolving<'_>) -> T) -> T {
        let fallback: Box<dyn Decider> = if last { Box::new(LastOption) } else { Box::new(crate::choice::FirstOption) };
        let mut table = Table::with_default(Box::new(Scripted::with_fallback(Vec::<String>::new(), fallback)));
        let mut dice = crate::dice::Dice::new();
        let mut rng = crate::rng::GameRng::new(0);
        let mut ctx = Resolving { content: ti4_content::ContentStore::embedded(), sources: FULL, dice: &mut dice, rng: &mut rng, table: &mut table, timing: Some(crate::choice::TimingHandle { resolver: &mut crate::timing::Resolver::new(vec![me(), PlayerId::new("b")], Some(me()), Table::default()), sequence: &mut crate::event::EventSequence::new(), galaxy: None }) };
        f(&mut ctx)
    }

    /// Every card of every deck, explored from a planet by a seat of three kinds (a plain one, Naaz with a mech, the Titans) with both policies
    /// of answering: the string and the core window agree on the questions, the position, the events and the value; the only bails are the
    /// ones the window names (Freelancers, Terragenesis).
    #[test]
    fn every_card_explored_from_a_planet_runs_in_lockstep_and_only_the_named_gaps_bail() {
        use ti4_engine_core::generated::Explore;
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mut bailed_cards = Vec::new();
        with_mask(AreaMask::STRING.with(Area::ExplorationRelics, Backend::Dual), || {
            for faction in ["hacan", "naaz", "titans"] {
                for card in Explore::ALL {
                    let id = crate::generated::IdStr::as_str(card);
                    for last in [false, true] {
                        let (mut state, planet) = table(faction);
                        if faction == "naaz" {
                            let (system, _) = crate::fixtures::a_placed_planet();
                            crate::fixtures::put_on_planet(&mut state, &system, &planet, "mech", &me(), 1);
                        }
                        state.exploration_decks.insert("CULTURAL".to_owned(), vec![id.to_owned(), "ent".to_owned()]);
                        state.relic_deck = ["thetriad", "dominusorb"].map(ti4_model::id::RelicId::new).to_vec();
                        let before = report().windows.get("exploration::explore").map_or(0, |w| w.bailed);
                        ctx_with(last, |ctx| explore_with(&mut state, ctx, &me(), "CULTURAL", Some(&planet)));
                        let after = report().windows.get("exploration::explore").map_or(0, |w| w.bailed);
                        if after > before {
                            bailed_cards.push((faction, id));
                        }
                    }
                }
            }
        });
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        // Only the Titans' planet explorations bail here (the dual driver has no registry for Terragenesis; the live Env has) and a Freelancers unit
        // that stands over the fleet supply or the capacity of its system (the string asks which units go); nothing else does.
        let unexpected: Vec<_> = bailed_cards.iter().filter(|(faction, id)| *faction != "titans" && !id.starts_with("frln")).collect();
        assert!(unexpected.is_empty(), "{unexpected:?}: {:?}", r.windows);
        assert!(r.windows.get("exploration::explore").map_or(0, |w| w.compared) > 300, "{:?}", r.windows);
        let _ = take_report();
    }

    /// Every card as a frontier token's draw (no planet): the planet-less arms (a gamma token needs an active system, Mirage names the token's
    /// system, an attachment is discarded) in lockstep with the string, with and without a timing handle.
    #[test]
    fn every_card_explored_from_a_frontier_token_runs_in_lockstep() {
        use ti4_engine_core::generated::Explore;
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        with_mask(AreaMask::STRING.with(Area::ExplorationRelics, Backend::Dual), || {
            for faction in ["hacan", "naaz"] {
                for card in Explore::ALL {
                    let id = crate::generated::IdStr::as_str(card);
                    for (last, active) in [(false, true), (true, false)] {
                        let (mut state, planet) = table(faction);
                        let (system, _) = crate::fixtures::a_placed_planet();
                        let _ = planet;
                        crate::fixtures::put(&mut state, &system, "carrier", &me(), 1);
                        state.frontier_tokens.insert(system.clone());
                        state.active_system = active.then(|| system.clone());
                        state.exploration_decks.insert("FRONTIER".to_owned(), vec![id.to_owned(), "ent".to_owned()]);
                        state.relic_deck = ["thetriad", "dominusorb"].map(ti4_model::id::RelicId::new).to_vec();
                        let out = ctx_with(last, |ctx| crate::exploration::explore_frontier(&mut state, ctx, &me(), &system));
                        assert!(out.is_some(), "{id}");
                    }
                }
            }
        });
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let w = r.windows.get("exploration::explore_frontier").copied();
        assert!(w.is_some_and(|w| w.compared > 150), "{:?}", r.windows);
        let bailed: Vec<_> = r.windows.iter().filter(|(label, w)| label.starts_with("exploration::explore_frontier [bail") && w.bailed > 0).collect();
        assert!(bailed.is_empty(), "{bailed:?}");
        let _ = take_report();
    }
}
