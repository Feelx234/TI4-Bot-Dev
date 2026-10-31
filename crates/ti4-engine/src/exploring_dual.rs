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
    use ti4_engine_core::state::{CoreState, EDeck, EHow, EWhy, Slice};
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

    /// The words of the log row of a card: the string's `row_words` over what the core window read.
    #[must_use]
    pub fn words(r: &RowIn) -> RowOut {
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
        RowOut { outcome: w.text, result: w.result, gains: w.gains.into_iter().collect(), placed: w.placed }
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
        let mut driver = crate::window_driver::driver_for(window(seat), Resolver::new(&[seat], Some(seat)), table, 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), crate::dual_window::nested_fp(FP, script), |core, _| {
            let driven = crate::dual_window::drive_with(&mut driver, core, kernel, &r, &before, script, &details);
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
        run_window(gs, content, sources, player, |seat| Exploring::frontier(seat, system, announce, true), script)
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
