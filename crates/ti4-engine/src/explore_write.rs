//! `Area::ExplorationRelics` (package B5), the mutation half: the writes of `exploration.rs` and `relics.rs`
//! (an exploration card drawn, a fragment gained, three purged for a relic, a relic gained, exhausted or
//! purged, an attachment put on a planet, the frontier tokens, the wormhole token) as
//! `ti4_engine_core::rules::explore` functions over the slices they touch, and the exploration questions
//! (`rules::explore_windows`) as lockstep windows.
//!
//! Every function answers `None` when the position is outside what the core holds (more than 16 owners, an
//! id the tables do not know, a deck reserve of a turn redo, a value the codecs refuse): the caller keeps
//! the string write. The string callers keep what the core cannot do (the log record, the events, the
//! effects that reach other areas); see `docs/ENGINE_KNOWN_ISSUES.md`, B5.

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

#[cfg(feature = "typed-core")]
mod real {
    use ti4_engine_core::choice::explore::{Opt as EOpt, Step};
    use ti4_engine_core::driver::Driver;
    use ti4_engine_core::generated::{Attachment, Explore, ExploreType, Planet, PublicObjective, Relic, System, Technology, Wormhole};
    use ti4_engine_core::ids::Seat;
    use ti4_engine_core::kernel::stage::AbilityTable;
    use ti4_engine_core::kernel::{Kernel, Resolver};
    use ti4_engine_core::rules::explore as rules;
    use ti4_engine_core::rules::relic_windows::{Codex, GainTechnology, Neuraloop, StellarConverter, TitanPrototype};
    use ti4_engine_core::rules::explore_windows::{InInvasion, ChooseDeck, CrownExplore, DistantSuns, DominusOrb, IonStorm, Reward};
    use ti4_engine_core::state::{CoreState, Slice};
    use ti4_model::content_types::SourceSet;
    use ti4_model::id::{PlanetId, PlayerId, RelicId, SystemId};
    use ti4_model::state::GameState;

    use crate::core_adapter::typed;
    use crate::core_sync::conv::{explore_key, explore_type};
    use crate::core_sync::{Footprint, Sources};
    use crate::dual::{CoreWindow, Run};
    use crate::dual_window::Recording;
    use crate::generated::IdStr;

    /// The mutations: the players' cards, the decks, the relic log and the vp ledger, the board's attachments,
    /// the frontier tokens, the ion storm (Meta).
    const WRITES: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Decks, Slice::Explore, Slice::Ledger],
        &[Slice::Meta, Slice::Players, Slice::Board, Slice::Decks, Slice::Explore, Slice::Ledger],
    );

    fn lend<T>(gs: &mut GameState, player: Option<&PlayerId>, f: impl FnOnce(&mut CoreState, Option<Seat>) -> T) -> Option<T> {
        // A turn redo draws through a reserve (`deck_reserve`): the draw is the string engine's.
        if gs.deck_reserve.is_some() {
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let who = match player {
            Some(p) => Some(r.seat(p.as_str())?),
            None => None,
        };
        crate::core_sync::with_core(gs, &mut cx, Sources::default(), WRITES, |core, _| f(core, who)).ok()
    }

    fn seat_of(who: Option<Seat>) -> Seat {
        who.expect("a seated call")
    }

    /// `exploration::draw`. The outer `None` is a bail.
    pub fn draw(gs: &mut GameState, deck: &str) -> Option<Option<String>> {
        let kind = explore_type(Slice::Decks, deck).ok()?;
        lend(gs, None, |core, _| rules::draw(core, kind).map(|c| c.as_str().to_owned()))
    }

    /// `exploration::gain_fragment`.
    pub fn gain_fragment(gs: &mut GameState, player: &PlayerId, trait_name: &str) -> Option<()> {
        let kind = explore_type(Slice::Players, &trait_name.to_ascii_uppercase()).ok()?;
        lend(gs, Some(player), |core, who| rules::gain_fragment(core, seat_of(who), kind))
    }

    /// `exploration::purge_for_relic`.
    pub fn purge_for_relic(gs: &mut GameState, player: &PlayerId, trait_name: &str) -> Option<Option<RelicId>> {
        let kind = explore_type(Slice::Players, &trait_name.to_ascii_uppercase()).ok()?;
        lend(gs, Some(player), |core, who| rules::purge_for_relic(core, seat_of(who), kind).map(|r| RelicId::new(r.as_str())))
    }

    /// `relics::gain`.
    pub fn relic_gain(gs: &mut GameState, player: &PlayerId) -> Option<Option<RelicId>> {
        lend(gs, Some(player), |core, who| rules::relic_gain(core, seat_of(who)).map(|r| RelicId::new(r.as_str())))
    }

    /// `relics::exhaust`.
    pub fn relic_exhaust(gs: &mut GameState, player: &PlayerId, relic: &str) -> Option<bool> {
        let r = typed::<Relic>(relic)?;
        lend(gs, Some(player), |core, who| rules::exhaust_relic(core, seat_of(who), r))
    }

    /// `relics::purge`.
    pub fn relic_purge(gs: &mut GameState, player: &PlayerId, relic: &RelicId) -> Option<()> {
        let r = typed::<Relic>(relic.as_str())?;
        lend(gs, Some(player), |core, who| rules::purge_relic(core, seat_of(who), r))
    }

    /// The attachment write of `explore_drawn`'s Attach arm.
    pub fn attach(gs: &mut GameState, planet: &PlanetId, attachment: &str) -> Option<()> {
        let (p, a) = (typed::<Planet>(planet.as_str())?, typed::<Attachment>(attachment)?);
        lend(gs, None, |core, _| rules::attach(core, p, a))
    }

    /// `state.frontier_tokens.remove(system)`.
    pub fn take_frontier(gs: &mut GameState, system: &SystemId) -> Option<bool> {
        let s = typed::<System>(system.as_str())?;
        lend(gs, None, |core, _| rules::take_frontier(core, s))
    }

    /// `exploration::place_frontier_tokens`: the token set.
    pub fn place_frontier_tokens(gs: &mut GameState, systems: &[SystemId]) -> Option<()> {
        let list: Option<Vec<System>> = systems.iter().map(|s| typed::<System>(s.as_str())).collect();
        let list = list?;
        lend(gs, None, |core, _| rules::place_frontier_tokens(core, &list))
    }

    /// The gamma token of Gamma Wormhole and Gamma Relay.
    pub fn place_gamma_token(gs: &mut GameState, system: &SystemId) -> Option<()> {
        let s = typed::<System>(system.as_str())?;
        lend(gs, None, |core, _| rules::place_wormhole_token(core, Wormhole::Gamma, s))
    }

    /// `relics::crown_of_emphidia_point`.
    pub fn crown_point(gs: &mut GameState, player: &PlayerId) -> Option<bool> {
        lend(gs, Some(player), |core, who| rules::crown_point(core, seat_of(who)))
    }

    // -- the windows -----------------------------------------------------------------------------------

    const WINDOW: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Decks, Slice::Pending],
        &[Slice::Meta, Slice::Decks],
    );

    /// The relic windows read the board (command tokens, units, control) and write the seat's cards.
    const RELIC_WINDOW: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Pending],
        &[Slice::Players],
    );

    /// The Codex moves cards between the discard pile and the hand; the Stellar Converter destroys a planet.
    const CODEX_WINDOW: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Cards, Slice::Pending],
        &[Slice::Players, Slice::Cards],
    );
    const STELLAR_WINDOW: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Pending],
        &[Slice::Board],
    );

    /// What a window of this file cannot follow: a question asked inside a battle or an invasion (its context
    /// carries the battle), diplomacy promises a favour keeps, a nested emission, a question nobody answered
    /// yet (the string window unwound; its value is not an answer).
    fn outside(gs: &GameState, script: &Recording) -> bool {
        gs.deck_reserve.is_some()
            || script.nested
            || gs.active_space_combat.is_some()
            || gs.diplomacy.enabled
            || script.questions.iter().any(|q| q.answer.is_none())
    }

    fn drive_window<W: ti4_engine_core::kernel::stage::Window + Clone + InInvasion, T>(
        gs: &mut GameState,
        fp: Footprint,
        player: &PlayerId,
        window: impl FnOnce(Seat) -> W,
        script: &Recording,
        details: &dyn Fn(&ti4_engine_core::choice::Choice, &CoreState, &mut crate::choice::Choice),
        value: impl FnOnce(&W) -> Option<T>,
    ) -> Option<CoreWindow<T>> {
        if outside(gs, script) {
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let seat = r.seat(player.as_str())?;
        let before = gs.clone();
        let kernel = Kernel::plain();
        let mut driver = Driver::new(window(seat).in_invasion(gs.active_invasion.as_ref().map(|a| a.seq)), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), fp, |core, _| {
            crate::dual_window::drive_with(&mut driver, core, &kernel, &r, &before, script, details)
        })
        .ok()?
        .ok()?;
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>()?;
        Some(CoreWindow { run: Run::new(value(&driver.window)?).with_events_vec(events), questions: driven.rendered })
    }

    fn no_details(_: &ti4_engine_core::choice::Choice, _: &CoreState, _: &mut crate::choice::Choice) {}

    /// The core half of `exploration::choose_deck` (more than one trait asks; the value is the deck).
    pub fn choose_deck(gs: &mut GameState, player: &PlayerId, traits: &[String], script: &Recording) -> Option<CoreWindow<Option<String>>> {
        let kinds: Option<Vec<ExploreType>> = traits.iter().map(|t| explore_type(Slice::Decks, t).ok()).collect();
        let kinds = kinds?;
        drive_window(gs, WINDOW, player, |s| ChooseDeck::new(s, &kinds), script, &no_details, |w| Some(w.chosen.map(explore_key)))
    }

    /// The core half of Distant Suns' pick: draws the second card, asks, the value is the card to resolve.
    pub fn distant_suns(gs: &mut GameState, player: &PlayerId, deck: &str, first: &str, script: &Recording) -> Option<CoreWindow<String>> {
        let deck = explore_type(Slice::Decks, deck).ok()?;
        let first = typed::<Explore>(first)?;
        drive_window(gs, WINDOW, player, |s| DistantSuns::new(s, deck, first), script, &no_details, |w| Some(w.chosen.as_str().to_owned()))
    }

    /// The core half of the ion storm card's question: asks the side, places the token; the value is the side.
    pub fn ion_storm(gs: &mut GameState, player: &PlayerId, system: &SystemId, script: &Recording) -> Option<CoreWindow<String>> {
        let system = typed::<System>(system.as_str())?;
        drive_window(
            gs,
            WINDOW,
            player,
            |s| IonStorm::new(s, system),
            script,
            &no_details,
            |w| w.face.map(|f| if f == Wormhole::Beta { "BETA" } else { "ALPHA" }.to_owned()),
        )
    }

    /// The core half of `relics::offer_dominus_orb` (the value: the card was purged).
    pub fn dominus_orb(gs: &mut GameState, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
        drive_window(gs, RELIC_WINDOW, player, DominusOrb::new, script, &no_details, |w| Some(w.used))
    }

    /// The core half of the Crown of Emphidia's choice of a planet (the value: the planet chosen and the Crown
    /// exhausted; exploring it is the string caller's).
    pub fn crown_explore(
        gs: &mut GameState,
        player: &PlayerId,
        sources: SourceSet,
        script: &Recording,
    ) -> Option<CoreWindow<Option<String>>> {
        let scope = crate::core_adapter::scope(sources);
        drive_window(gs, RELIC_WINDOW, player, |s| CrownExplore::new(s, scope), script, &no_details, |w| {
            Some(w.planet.map(|p| p.as_str().to_owned()))
        })
    }

    /// The core half of the Neuraloop's question (the value: the relic named, `None` for a decline).
    pub fn neuraloop(gs: &mut GameState, holder: &PlayerId, revealed: &str, script: &Recording) -> Option<CoreWindow<Option<String>>> {
        let revealed = typed::<PublicObjective>(revealed)?;
        drive_window(gs, RELIC_WINDOW, holder, |s| Neuraloop::new(s, revealed), script, &no_details, |w| {
            Some(w.chosen.map(|r| r.as_str().to_owned()))
        })
    }

    /// The core half of `relics::codex` (up to three cards from the discard pile into the hand).
    pub fn codex(gs: &mut GameState, player: &PlayerId, script: &Recording) -> Option<CoreWindow<()>> {
        drive_window(gs, CODEX_WINDOW, player, Codex::new, script, &no_details, |_| Some(()))
    }

    /// The core half of the Titan Prototype's question (the value: the player named).
    pub fn titan_prototype(gs: &mut GameState, player: &PlayerId, script: &Recording) -> Option<CoreWindow<Option<String>>> {
        let (_, r) = crate::dual_window::tables(gs)?;
        drive_window(gs, RELIC_WINDOW, player, TitanPrototype::new, script, &no_details, |w| {
            Some(w.chosen.map(|seat| r.seat_text(seat).to_owned()))
        })
    }

    /// The core half of the Stellar Converter's choice and destruction (the value: the planet destroyed).
    pub fn stellar_converter(
        gs: &mut GameState,
        player: &PlayerId,
        targets: &[(SystemId, PlanetId)],
        script: &Recording,
    ) -> Option<CoreWindow<Option<String>>> {
        let list: Option<Vec<(System, Planet)>> =
            targets.iter().map(|(s, p)| Some((typed::<System>(s.as_str())?, typed::<Planet>(p.as_str())?))).collect();
        let list = list?;
        drive_window(gs, STELLAR_WINDOW, player, |s| StellarConverter::new(s, &list), script, &no_details, |w| {
            Some(w.destroyed.map(|p| p.as_str().to_owned()))
        })
    }

    /// The core half of "gain which technology" (the value: the technology named).
    pub fn gain_technology(
        gs: &mut GameState,
        player: &PlayerId,
        relic: &str,
        options: &[String],
        script: &Recording,
    ) -> Option<CoreWindow<Option<String>>> {
        let relic = typed::<Relic>(relic)?;
        let list: Option<Vec<Technology>> = options.iter().map(|t| typed::<Technology>(t)).collect();
        let list = list?;
        drive_window(gs, RELIC_WINDOW, player, |s| GainTechnology::new(s, relic, &list), script, &no_details, |w| {
            Some(w.chosen.map(|t| t.as_str().to_owned()))
        })
    }

    /// The core half of a reward question of an instant card. `options` are the ids the string side offers, in
    /// its order; `details` completes the rendered question (the commodity standing of the four commodity cards).
    pub fn reward(
        gs: &mut GameState,
        player: &PlayerId,
        card: &str,
        options: &[&str],
        skip_single: bool,
        commodities: Option<serde_json::Value>,
        script: &Recording,
    ) -> Option<CoreWindow<Option<String>>> {
        let step = crate::render::explore::step_of(card)?;
        if matches!(step, Step::Exploration | Step::IonStorm) {
            return None;
        }
        let ask = ti4_engine_core::choice::explore::Ask::Reward(step);
        let parsed: Option<Vec<EOpt>> = options
            .iter()
            .map(|id| <crate::render::explore::Fam as crate::render::Family>::parse(&crate::render::Render::new(Vec::new()), &ask, id))
            .collect();
        let parsed = parsed?;
        let details = move |_: &ti4_engine_core::choice::Choice, _: &CoreState, rendered: &mut crate::choice::Choice| {
            if let Some(text) = &commodities {
                rendered.details.insert("commodities".to_owned(), text.clone());
            }
        };
        drive_window(
            gs,
            WINDOW,
            player,
            |s| Reward::new(s, step, &parsed, skip_single),
            script,
            &details,
            |w| {
                Some(w.chosen.map(|o| {
                    <crate::render::explore::Fam as crate::render::Family>::option(&crate::render::Render::new(Vec::new()), &ask, &o).id
                }))
            },
        )
    }
}

#[cfg(not(feature = "typed-core"))]
mod stub {
    use ti4_model::id::{PlanetId, PlayerId, RelicId, SystemId};
    use ti4_model::state::GameState;

    use crate::dual::CoreWindow;
    use crate::dual_window::Recording;

    pub fn draw(_: &mut GameState, _: &str) -> Option<Option<String>> {
        None
    }
    pub fn gain_fragment(_: &mut GameState, _: &PlayerId, _: &str) -> Option<()> {
        None
    }
    pub fn purge_for_relic(_: &mut GameState, _: &PlayerId, _: &str) -> Option<Option<RelicId>> {
        None
    }
    pub fn relic_gain(_: &mut GameState, _: &PlayerId) -> Option<Option<RelicId>> {
        None
    }
    pub fn relic_exhaust(_: &mut GameState, _: &PlayerId, _: &str) -> Option<bool> {
        None
    }
    pub fn relic_purge(_: &mut GameState, _: &PlayerId, _: &RelicId) -> Option<()> {
        None
    }
    pub fn attach(_: &mut GameState, _: &PlanetId, _: &str) -> Option<()> {
        None
    }
    pub fn take_frontier(_: &mut GameState, _: &SystemId) -> Option<bool> {
        None
    }
    pub fn place_frontier_tokens(_: &mut GameState, _: &[SystemId]) -> Option<()> {
        None
    }
    pub fn place_gamma_token(_: &mut GameState, _: &SystemId) -> Option<()> {
        None
    }
    pub fn crown_point(_: &mut GameState, _: &PlayerId) -> Option<bool> {
        None
    }
    pub fn dominus_orb(_: &mut GameState, _: &PlayerId, _: &Recording) -> Option<CoreWindow<bool>> {
        None
    }
    pub fn crown_explore(
        _: &mut GameState,
        _: &PlayerId,
        _: ti4_model::content_types::SourceSet,
        _: &Recording,
    ) -> Option<CoreWindow<Option<String>>> {
        None
    }
    pub fn choose_deck(_: &mut GameState, _: &PlayerId, _: &[String], _: &Recording) -> Option<CoreWindow<Option<String>>> {
        None
    }
    pub fn distant_suns(_: &mut GameState, _: &PlayerId, _: &str, _: &str, _: &Recording) -> Option<CoreWindow<String>> {
        None
    }
    pub fn ion_storm(_: &mut GameState, _: &PlayerId, _: &SystemId, _: &Recording) -> Option<CoreWindow<String>> {
        None
    }
    pub fn neuraloop(_: &mut GameState, _: &PlayerId, _: &str, _: &Recording) -> Option<CoreWindow<Option<String>>> {
        None
    }
    pub fn codex(_: &mut GameState, _: &PlayerId, _: &Recording) -> Option<CoreWindow<()>> {
        None
    }
    pub fn titan_prototype(_: &mut GameState, _: &PlayerId, _: &Recording) -> Option<CoreWindow<Option<String>>> {
        None
    }
    pub fn stellar_converter(
        _: &mut GameState,
        _: &PlayerId,
        _: &[(SystemId, PlanetId)],
        _: &Recording,
    ) -> Option<CoreWindow<Option<String>>> {
        None
    }
    pub fn gain_technology(
        _: &mut GameState,
        _: &PlayerId,
        _: &str,
        _: &[String],
        _: &Recording,
    ) -> Option<CoreWindow<Option<String>>> {
        None
    }
    pub fn reward(
        _: &mut GameState,
        _: &PlayerId,
        _: &str,
        _: &[&str],
        _: bool,
        _: Option<serde_json::Value>,
        _: &Recording,
    ) -> Option<CoreWindow<Option<String>>> {
        None
    }
}

#[cfg(all(test, feature = "typed-core"))]
mod tests {
    use ti4_model::content_types::POK;
    use ti4_model::id::{PlanetId, PlayerId, RelicId, SystemId};
    use ti4_model::state::GameState;

    use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};
    use crate::{exploration, relics};

    fn me() -> PlayerId {
        PlayerId::new("a")
    }

    fn seated() -> GameState {
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], POK);
        state.promissory_notes.clear();
        state.promissory_faceup.clear();
        state.round = 2;
        state.exploration_decks.clear();
        state.exploration_decks.insert("CULTURAL".to_owned(), vec!["minent".into(), "majent".into(), "ent".into()]);
        state.exploration_decks.insert("FRONTIER".to_owned(), Vec::new());
        state.relic_deck = ["thetriad", "shard", "dominusorb", "emphidia"].map(RelicId::new).to_vec();
        state
    }

    /// Every write of the mutation half runs on the core under `exploration=dual` and agrees with the string
    /// write: draws, fragments, the purge for a relic, a relic gained (the Shard scores), exhausted, purged,
    /// attachments, frontier tokens, the gamma token.
    #[test]
    fn the_writes_run_in_lockstep_with_the_core() {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::ExplorationRelics, Backend::Dual);
        with_mask(mask, || {
            let mut state = seated();
            assert!(super::draw(&mut state.clone(), "CULTURAL").is_some(), "the core half runs");
            assert_eq!(exploration::draw(&mut state, "CULTURAL").as_deref(), Some("minent"));
            assert_eq!(exploration::draw(&mut state, "HAZARDOUS"), None, "no such deck");
            assert_eq!(exploration::draw(&mut state, "FRONTIER"), None, "an empty deck");
            assert_eq!(state.exploration_decks["CULTURAL"].len(), 2);

            // Fragments: three cultural make a relic; a frontier fragment makes up a shortfall.
            for _ in 0..2 {
                exploration::gain_fragment(&mut state, &me(), "cultural");
            }
            assert!(exploration::purge_for_relic(&mut state, &me(), "CULTURAL").is_none(), "two are not enough");
            exploration::gain_fragment(&mut state, &me(), "FRONTIER");
            assert!(super::purge_for_relic(&mut state.clone(), &me(), "CULTURAL").is_some(), "the core half runs");
            assert_eq!(exploration::purge_for_relic(&mut state, &me(), "CULTURAL"), Some(RelicId::new("thetriad")));
            let seat = state.player(&me()).unwrap();
            assert_eq!(seat.relic_fragments.get("CULTURAL"), Some(&0));
            assert_eq!(seat.relic_fragments.get("FRONTIER"), Some(&0));
            assert_eq!(seat.relics, vec![RelicId::new("thetriad")]);

            // Three matching fragments are spent first; the frontier one stays.
            let mut full = seated();
            for _ in 0..4 {
                exploration::gain_fragment(&mut full, &me(), "CULTURAL");
            }
            exploration::gain_fragment(&mut full, &me(), "FRONTIER");
            assert_eq!(exploration::purge_for_relic(&mut full, &me(), "CULTURAL"), Some(RelicId::new("thetriad")));
            let seat = full.player(&me()).unwrap();
            assert_eq!((seat.relic_fragments.get("CULTURAL"), seat.relic_fragments.get("FRONTIER")), (Some(&1), Some(&1)));

            // A relic gained: the Shard scores at once, the deck runs out at last.
            let before = state.player(&me()).unwrap().victory_points;
            assert!(super::relic_gain(&mut state.clone(), &me()).is_some());
            assert_eq!(relics::gain(&mut state, &me()), Some(RelicId::new("shard")));
            assert_eq!(state.player(&me()).unwrap().victory_points, before + 1);
            assert!(relics::gain(&mut state, &me()).is_some());
            assert!(relics::gain(&mut state, &me()).is_some());
            assert_eq!(relics::gain(&mut state, &me()), None, "73.2a");
            assert_eq!(state.relic_log.len(), 4);

            // At the victory target the Shard's point is capped.
            let mut capped = seated();
            capped.player_mut(&me()).unwrap().victory_points = 10;
            capped.relic_deck = vec![RelicId::new("shard")];
            assert_eq!(relics::gain(&mut capped, &me()), Some(RelicId::new("shard")));

            // Exhaust and purge.
            assert!(super::relic_exhaust(&mut state.clone(), &me(), "dominusorb").is_some());
            assert!(relics::exhaust(&mut state, &me(), "dominusorb"));
            assert!(!relics::exhaust(&mut state, &me(), "dominusorb"), "exhausted already");
            assert!(!relics::exhaust(&mut state, &me(), "heartofixth"), "not held");
            assert!(super::relic_purge(&mut state.clone(), &me(), &RelicId::new("dominusorb")).is_some());
            let id = RelicId::new("dominusorb");
            relics::purge(&mut state, &me(), &id);
            assert!(!state.player(&me()).unwrap().relics.contains(&id));

            // Attachments, frontier tokens, the gamma token.
            let planet = PlanetId::new("lazulrex");
            assert!(super::attach(&mut state.clone(), &planet, "tombofemphidia").is_some());
            let attachment = exploration::attachment_for(ti4_content::ContentStore::embedded(), "toe", &planet);
            assert!(super::attach(&mut state.clone(), &planet, &attachment).is_some());
            exploration::attach(&mut state, &planet, &attachment);
            exploration::attach(&mut state, &planet, &attachment);
            assert_eq!(state.planet_attachments[&planet].len(), 2);
            let (a, b) = (SystemId::new("19"), SystemId::new("20"));
            exploration::set_frontier_tokens(&mut state, &[b.clone(), a.clone()]);
            assert!(state.frontier_tokens.contains(&a) && state.frontier_tokens.contains(&b));
            assert!(super::take_frontier(&mut state.clone(), &a).is_some());
            exploration::take_frontier_token(&mut state, &a);
            assert!(!state.frontier_tokens.contains(&a));
            exploration::place_gamma_token(&mut state, &b);
            assert_eq!(state.wormhole_tokens.get("GAMMA"), Some(&b));
        });
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        assert!(r.total_calls() >= 20, "the core halves ran: {r:?}");
        let _ = take_report();
    }

    /// The documented bails of the writes: a trait or a deck the tables do not know, an attachment they do
    /// not know, a deck reserve (turn redo). The string write answers alone; nothing differs.
    #[test]
    fn the_writes_bail_where_the_core_holds_less() {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::ExplorationRelics, Backend::Dual);
        with_mask(mask, || {
            let mut state = seated();
            assert!(super::draw(&mut state.clone(), "WEIRD").is_none());
            assert!(super::gain_fragment(&mut state.clone(), &me(), "WEIRD").is_none());
            let unknown = "x".repeat(3);
            assert!(super::attach(&mut state.clone(), &PlanetId::new("lazulrex"), &unknown).is_none());
            assert!(super::relic_exhaust(&mut state.clone(), &me(), "notarelic").is_none());
            // The string write still runs.
            state.exploration_decks.insert("WEIRD".to_owned(), vec!["minent".into()]);
            assert_eq!(exploration::draw(&mut state, "WEIRD").as_deref(), Some("minent"));
            exploration::gain_fragment(&mut state, &me(), "weird");
            assert_eq!(state.player(&me()).unwrap().relic_fragments.get("WEIRD"), Some(&1));
            // A turn redo holds a deck reserve: the codec refuses the position.
            let mut redo = seated();
            redo.deck_reserve = Some(ti4_model::deck_reserve::DeckReserve::new(
                std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(ti4_model::deck_reserve::IDLE)),
                Vec::new(),
            ));
            assert!(super::draw(&mut redo.clone(), "CULTURAL").is_none());
            assert_eq!(exploration::draw(&mut redo, "CULTURAL").as_deref(), Some("minent"));
        });
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }
}
