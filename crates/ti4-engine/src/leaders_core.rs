//! `Area::LeadersAbilities` (package B7), the mutation half: the status writes of `leaders.rs`
//! (`ready_all`, `ready`, `exhaust`, `purge`, the write of `check_unlocks`) as `ti4_engine_core::rules::leaders`
//! functions over the Players slice. Every function answers `None` when the position is outside what the core
//! holds (more than 16 owners, a leader id the tables do not know, a value the codecs refuse): the caller
//! keeps the string write. The side effects the core cannot do (Nomad's agent hooks, the staged event) stay
//! in the string callers, after the status write.

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

#[cfg(feature = "typed-core")]
mod real {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::ids::Seat;
    use ti4_engine_core::rules::leaders as rules;
    use ti4_engine_core::state::{CoreState, Slice};
    use ti4_model::id::{LeaderId, PlayerId};
    use ti4_model::state::GameState;

    use ti4_engine_core::driver::Driver;
    use ti4_engine_core::kernel::stage::AbilityTable;
    use ti4_engine_core::kernel::{Kernel, Resolver};
    use ti4_engine_core::rules::leader_windows::{HacanAgent, Helmsman, RinHero, XxchaAgent};
    use ti4_model::content_types::SourceSet;

    use crate::core_adapter::typed;
    use crate::dual::{CoreWindow, Run, Why};
    use crate::dual_window::Recording;
    use crate::generated::IdStr;
    use crate::core_sync::{Footprint, Sources};

    const WRITES: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players], &[Slice::Players]);

    fn lend<T>(gs: &mut GameState, player: &PlayerId, f: impl FnOnce(&mut CoreState, Seat) -> T) -> Option<T> {
        crate::dual_window::lend(gs, WRITES, Some(player), |core, who| f(core, who.expect("a seated call")))
    }

    fn ids(list: &[Leader]) -> Vec<LeaderId> {
        list.iter().map(|l| LeaderId::new(l.as_str())).collect()
    }

    /// `leaders::ready_all`.
    pub fn ready_all(gs: &mut GameState, player: &PlayerId) -> Option<Vec<LeaderId>> {
        lend(gs, player, |core, who| ids(rules::ready_all(core, who).as_slice()))
    }

    /// `leaders::ready`.
    pub fn ready(gs: &mut GameState, player: &PlayerId, leader: &LeaderId) -> Option<bool> {
        let l = typed::<Leader>(leader.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
        lend(gs, player, |core, who| rules::ready(core, who, l))
    }

    /// `leaders::exhaust`.
    pub fn exhaust(gs: &mut GameState, player: &PlayerId, leader: &LeaderId) -> Option<bool> {
        let l = typed::<Leader>(leader.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
        lend(gs, player, |core, who| rules::exhaust(core, who, l))
    }

    /// `leaders::purge`.
    pub fn purge(gs: &mut GameState, player: &PlayerId, leader: &LeaderId) -> Option<bool> {
        let l = typed::<Leader>(leader.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
        lend(gs, player, |core, who| rules::purge(core, who, l))
    }

    /// Evelyn DeLouis / Viscount Unlenn: the extra die of this combat round. `base` is the unit type the
    /// string side resolved (a form the tables do not know bails).
    pub fn extra_die_agent(gs: &mut GameState, player: &PlayerId, base: &str) -> Option<bool> {
        let unit = typed::<ti4_engine_core::generated::Unit>(base)?;
        lend(gs, player, |core, who| rules::extra_die_agent(core, who, unit))
    }

    /// Darktalon Treilla's activation.
    pub fn activate_letnev_hero(gs: &mut GameState, player: &PlayerId) -> Option<bool> {
        lend(gs, player, |core, who| rules::activate_letnev_hero(core, who))
    }

    /// Hacan's hero activation.
    pub fn activate_hacan_hero(gs: &mut GameState, player: &PlayerId) -> Option<bool> {
        lend(gs, player, |core, who| rules::activate_hacan_hero(core, who))
    }

    /// `leaders::end_of_round`.
    pub fn end_of_round(gs: &mut GameState) -> Option<Vec<PlayerId>> {
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seats = crate::core_sync::with_core(gs, &mut cx, Sources::default(), WRITES, |core, _| {
            rules::end_of_round(core).as_slice().to_vec()
        })
        .ok()?;
        Some(seats.into_iter().map(|s| PlayerId::new(r.seat_text(s))).collect())
    }


    /// With a map the Map slice is loaded (the string galaxy lends it); without one it cannot be, and the window
    /// never reads it.
    const XXCHA: Footprint =
        Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Map, Slice::Pending], &[Slice::Board]);
    const XXCHA_NO_MAP: Footprint =
        Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Pending], &[Slice::Board]);
    /// Xxekir Grom places units (a Naaz mech is announced: the staged event waits in the pending slice).
    const XXCHA_HERO: Footprint =
        Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Pending], &[Slice::Board, Slice::Pending]);
    const HACAN: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Cards, Slice::Pending], &[Slice::Players, Slice::Cards]);

    const BOARD: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board], &[Slice::Board]);
    /// [`BOARD`] for the writes that announce a Naaz mech (the staged event waits in the pending slice).
    const BOARD_STAGED: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Pending], &[Slice::Board, Slice::Pending]);

    /// Jace X. 4th Air Legion.
    pub fn sol_hero(gs: &mut GameState, player: &PlayerId) -> Option<bool> {
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let who = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        crate::core_sync::with_core(gs, &mut cx, Sources::default(), BOARD, |core, _| rules::sol_hero(core, who)).ok()
    }

    /// Claire Gibson: the placement of `count` units of `unit` (resolved by the string side) on the planet.
    pub fn sol_commander(
        gs: &mut GameState,
        player: &PlayerId,
        system: &ti4_model::id::SystemId,
        planet: &ti4_model::id::PlanetId,
        unit: &str,
        count: usize,
    ) -> Option<bool> {
        let unit = typed::<ti4_engine_core::generated::Unit>(unit)?;
        let (system, planet) = (
            typed::<ti4_engine_core::generated::System>(system.as_str())?,
            typed::<ti4_engine_core::generated::Planet>(planet.as_str())?,
        );
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let who = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        crate::core_sync::with_core(gs, &mut cx, Sources::default(), BOARD, |core, _| rules::sol_commander(core, who, system, planet, unit, count)).ok()
    }

    /// I48S: the swap of one infantry of `target` for `mech` (a unit form without a core `Unit` and a favour that keeps a
    /// diplomacy promise stay with the string arm; a Naaz mech is announced by the core rule since package W3).
    pub fn l1z1x_agent(
        gs: &mut GameState,
        sources: SourceSet,
        player: &PlayerId,
        target: &PlayerId,
        system: &ti4_model::id::SystemId,
        mech: &str,
    ) -> Option<bool> {
        if gs.diplomacy.enabled && target != player {
            crate::dual::note_bail_here("a favour that keeps a diplomacy promise");
            return None;
        }
        let (mech, system) = (typed::<ti4_engine_core::generated::Unit>(mech)?, typed::<ti4_engine_core::generated::System>(system.as_str())?);
        let scope = ti4_engine_core::tables::Scope(u32::try_from(crate::core_adapter::scope(sources).0).ok()?);
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(target.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let mut staged = Vec::new();
        let swapped = crate::core_sync::with_core(gs, &mut cx, Sources::default(), BOARD_STAGED, |core, _| {
            let swapped = rules::l1z1x_agent(core, scope, seat, system, mech);
            staged = std::mem::take(&mut core.pending_mut().staged);
            swapped
        })
        .ok()?;
        crate::dual_window::apply_staged(gs, &r, &staged)?;
        Some(swapped)
    }


    /// Whether a commodity change at this table causes events the core does not hold: a Cabal (its replenish
    /// note), a siphoning Firmament plot, a Trade Agreement in play.
    pub(crate) fn replenish_causes_events(gs: &GameState) -> bool {
        gs.players.iter().any(|p| {
            p.faction.as_str() == "cabal"
                || crate::factions::firmament_plots::siphon_watches(gs, &p.id)
                || crate::promissory::holder_of(gs, "ta", &p.id).is_some()
        })
    }

    /// Why the core windows that replenish (the Trade primary and secondary, Carth of Golden Sands) cannot follow a table: a plot that
    /// listens to commodity gains (Siphon: a Firmament/Obsidian seat, never seated in the lobby), or two seats of one faction (the Trade
    /// Agreement and the option ids are keyed by faction). Cabal's `COMMODITIES_REPLENISHED` and the Trade Agreement are the core's
    /// since package W10 (`fx::replenish_announced`).
    pub(crate) fn replenish_window_unported(gs: &GameState) -> Option<&'static str> {
        if gs.players.iter().any(|p| crate::factions::firmament_plots::siphon_watches(gs, &p.id)) {
            return Some("a siphoning plot listens to the commodities a replenish gains");
        }
        let factions: Vec<&str> = gs.players.iter().map(|p| p.faction.as_str()).collect();
        if factions.iter().enumerate().any(|(i, f)| factions[..i].contains(f)) {
            return Some("two seats of one faction (the Trade Agreement is keyed by faction)");
        }
        None
    }

    /// The kernel of a leader window: no hooks, the game's source scope.
    fn kernel(sources: SourceSet) -> Option<Kernel> {
        let mut kernel = Kernel::plain();
        kernel.sources = ti4_engine_core::kernel::Sources(u8::try_from(crate::core_adapter::scope(sources).0).ok()?);
        Some(kernel)
    }

    /// The core half of `leaders::xxcha_agent`. `None` when the position is outside what the core holds (see
    /// [`outside`]; a map on one side only).
    pub fn xxcha_agent(gs: &mut GameState, player: &PlayerId, galaxy: Option<&ti4_content::galaxy::Galaxy>, sources: SourceSet, script: &Recording) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let kernel = kernel(sources)?;
        let before = gs.clone();
        let has_map = galaxy.is_some();
        let mut driver = Driver::new(XxchaAgent::new(seat, has_map), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources { galaxy, rng: None }, if has_map { XXCHA } else { XXCHA_NO_MAP }, |core, _| {
            if has_map && core.galaxy().is_none() {
                return None;
            }
            Some(crate::dual_window::drive(&mut driver, core, &kernel, &r, &before, script))
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
        Some(CoreWindow { run: Run::new(driver.window.used).with_events_vec(events), questions: driven.rendered })
    }

    /// The core half of Xxekir Grom (`xxchahero-te`): up to four PDS or mechs onto controlled planets, each planet
    /// readied. `None` as for [`xxcha_agent`], and for a unit form the tables do not know (a Naaz mech is announced by the
    /// window since package W3).
    pub fn xxcha_hero_te(gs: &mut GameState, player: &PlayerId, facts: &crate::leaders::HeroTeFacts, script: &Recording) -> Option<CoreWindow<bool>> {
        use ti4_engine_core::generated::{Planet, System, Unit};
        use ti4_engine_core::rules::leader_windows::XxchaHeroTe;
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let form = |f: &Option<(ti4_model::id::UnitTypeId, usize)>| -> Option<Option<(Unit, usize)>> {
            match f {
                None => Some(None),
                Some((unit, left)) => Some(Some((typed::<Unit>(unit.as_str()).why(crate::dual::reason::UNKNOWN_ID)?, (*left).min(usize::from(XxchaHeroTe::MOST))))),
            }
        };
        let (pds, mech) = (form(&facts.pds)?, form(&facts.mech)?);
        let planets: Vec<(System, Planet)> = facts
            .planets
            .iter()
            .map(|(s, p)| Some((typed::<System>(s.as_str()).why(crate::dual::reason::UNKNOWN_ID)?, typed::<Planet>(p.as_str()).why(crate::dual::reason::UNKNOWN_ID)?)))
            .collect::<Option<_>>()?;
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let kernel = Kernel::plain();
        let before = gs.clone();
        let mut driver = Driver::new(XxchaHeroTe::new(seat, &planets, pds, mech), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let mut staged = Vec::new();
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), XXCHA_HERO, |core, _| {
            let driven = crate::dual_window::drive(&mut driver, core, &kernel, &r, &before, script);
            staged = std::mem::take(&mut core.pending_mut().staged);
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        crate::dual_window::apply_staged(gs, &r, &staged)?;
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
        Some(CoreWindow { run: Run::new(driver.window.used).with_events_vec(events), questions: driven.rendered })
    }

    /// The core half of `leaders::l1z1x_hero` (The Helmsman). `None` as for [`xxcha_agent`].
    pub fn l1z1x_hero(gs: &mut GameState, player: &PlayerId, galaxy: Option<&ti4_content::galaxy::Galaxy>, sources: SourceSet, script: &Recording) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let kernel = kernel(sources)?;
        let before = gs.clone();
        let has_map = galaxy.is_some();
        let mut driver = Driver::new(Helmsman::new(seat, has_map), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources { galaxy, rng: None }, if has_map { XXCHA } else { XXCHA_NO_MAP }, |core, _| {
            if has_map && core.galaxy().is_none() {
                return None;
            }
            Some(crate::dual_window::drive(&mut driver, core, &kernel, &r, &before, script))
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
        Some(CoreWindow { run: Run::new(driver.window.used).with_events_vec(events), questions: driven.rendered })
    }

    const PLAYERS: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Pending], &[Slice::Players]);

    /// The core half of `leaders::jolnar_hero` (Rin). `None` where `apply_unit_upgrades` would change the
    /// position (the units are not in the form the owned upgrades demand: the string re-forms them after a swap
    /// and the core does not), and as [`xxcha_agent`] otherwise.
    pub fn jolnar_hero(gs: &mut GameState, content: &ti4_content::ContentStore, player: &PlayerId, sources: SourceSet, script: &Recording) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let mut formed = gs.clone();
        crate::technology::apply_unit_upgrades(&mut formed, content, sources, player);
        if !formed.identical(gs) {
            return None;
        }
        // The corpus file's order of the technologies (the string lists replacements in it).
        let order: Vec<_> = content
            .from_sources(ti4_model::content_types::ContentType::Technologies, sources)
            .filter_map(|r| r.text("alias").and_then(typed::<ti4_engine_core::generated::Technology>))
            .collect();
        let mut by_alias: Vec<_> = ti4_engine_core::generated::Technology::ALL.to_vec();
        by_alias.sort_by_key(|t| t.as_str());
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let kernel = kernel(sources)?;
        let before = gs.clone();
        let mut driver = Driver::new(RinHero::new(seat, order, by_alias), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), PLAYERS, |core, _| {
            crate::dual_window::drive(&mut driver, core, &kernel, &r, &before, script)
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
        Some(CoreWindow { run: Run::new(driver.window.used).with_events_vec(events), questions: driven.rendered })
    }

    /// The core half of `leaders::hacan_agent`. `None` where a commodity change causes events the core does not
    /// hold: a siphoning Firmament plot, a Cabal at the table, a Trade Agreement in play.
    pub fn hacan_agent(gs: &mut GameState, content: &ti4_content::ContentStore, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        if let Some(why) = replenish_window_unported(gs) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let before = gs.clone();
        let kernel = Kernel::plain();
        let mut driver = Driver::new(HacanAgent::with_source(seat, r.code("trade_agreement")), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let mut staged = Vec::new();
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), HACAN, |core, _| {
            let driven = crate::dual_window::drive_with(&mut driver, core, &kernel, &r, &before, script, &|_, _, rendered| {
                rendered.details.insert("commodities".to_owned(), crate::strategy_cards::commodity_standing(&before, content, player));
            });
            staged = std::mem::take(&mut core.pending_mut().staged);
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        crate::dual_window::apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
        Some(CoreWindow { run: Run::new(driver.window.used).with_events_vec(events), questions: driven.rendered })
    }

    /// The status write of `leaders::check_unlocks`.
    pub fn unlock(gs: &mut GameState, player: &PlayerId, leaders: &[LeaderId]) -> Option<()> {
        let list: Option<Vec<Leader>> = leaders.iter().map(|l| typed::<Leader>(l.as_str())).collect();
        let list = list?;
        lend(gs, player, |core, who| rules::unlock(core, who, &list))
    }
}

#[cfg(not(feature = "typed-core"))]
mod stub {
    use ti4_model::id::{LeaderId, PlayerId};
    use ti4_model::state::GameState;

    pub fn ready_all(_: &mut GameState, _: &PlayerId) -> Option<Vec<LeaderId>> {
        None
    }
    pub fn ready(_: &mut GameState, _: &PlayerId, _: &LeaderId) -> Option<bool> {
        None
    }
    pub fn exhaust(_: &mut GameState, _: &PlayerId, _: &LeaderId) -> Option<bool> {
        None
    }
    pub fn purge(_: &mut GameState, _: &PlayerId, _: &LeaderId) -> Option<bool> {
        None
    }
    pub fn unlock(_: &mut GameState, _: &PlayerId, _: &[LeaderId]) -> Option<()> {
        None
    }
    pub fn extra_die_agent(_: &mut GameState, _: &PlayerId, _: &str) -> Option<bool> {
        None
    }
    pub fn activate_letnev_hero(_: &mut GameState, _: &PlayerId) -> Option<bool> {
        None
    }
    pub fn activate_hacan_hero(_: &mut GameState, _: &PlayerId) -> Option<bool> {
        None
    }
    pub fn end_of_round(_: &mut GameState) -> Option<Vec<PlayerId>> {
        None
    }
    pub fn sol_commander(
        _: &mut GameState,
        _: &PlayerId,
        _: &ti4_model::id::SystemId,
        _: &ti4_model::id::PlanetId,
        _: &str,
        _: usize,
    ) -> Option<bool> {
        None
    }
    pub fn sol_hero(_: &mut GameState, _: &PlayerId) -> Option<bool> {
        None
    }
    pub fn l1z1x_agent(
        _: &mut GameState,
        _: ti4_model::content_types::SourceSet,
        _: &PlayerId,
        _: &PlayerId,
        _: &ti4_model::id::SystemId,
        _: &str,
    ) -> Option<bool> {
        None
    }
    pub fn l1z1x_hero(
        _: &mut GameState,
        _: &PlayerId,
        _: Option<&ti4_content::galaxy::Galaxy>,
        _: ti4_model::content_types::SourceSet,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }
    pub fn jolnar_hero(
        _: &mut GameState,
        _: &ti4_content::ContentStore,
        _: &PlayerId,
        _: ti4_model::content_types::SourceSet,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }
    pub fn xxcha_agent(
        _: &mut GameState,
        _: &PlayerId,
        _: Option<&ti4_content::galaxy::Galaxy>,
        _: ti4_model::content_types::SourceSet,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }
    pub fn xxcha_hero_te(
        _: &mut GameState,
        _: &PlayerId,
        _: &crate::leaders::HeroTeFacts,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }
    pub fn hacan_agent(
        _: &mut GameState,
        _: &ti4_content::ContentStore,
        _: &PlayerId,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }
}
